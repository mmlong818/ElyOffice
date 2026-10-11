param(
    [Parameter(Mandatory = $true)]
    [string]$MuPDFSource,

    [Parameter(Mandatory = $true)]
    [string]$OpenSSLRoot,

    [string]$PlatformToolset = 'v143',
    [string]$WindowsTargetPlatformVersion = '10.0.26100.0',
    [int]$MaxCpuCount = 6
)

$ErrorActionPreference = 'Stop'
$MuPDFCommit = '8ad45e92f0935d3d87f1db3f873086472a5e1b24'
$repo = Split-Path -Parent $PSScriptRoot
$source = (Resolve-Path -LiteralPath $MuPDFSource).Path
$openssl = (Resolve-Path -LiteralPath $OpenSSLRoot).Path
$actualCommit = (& git -C $source rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $actualCommit -ne $MuPDFCommit) {
    throw "MuPDF source must be the pinned commit $MuPDFCommit (found $actualCommit)."
}

# Keep the upstream checkout and the five local PDF behavior fixes together,
# while accepting a source tree on which this script has already run.
foreach ($patch in Get-ChildItem (Join-Path $repo 'patches/mupdf') -Filter '*.patch' | Sort-Object Name) {
    & git -C $source apply --check $patch.FullName 2>$null
    if ($LASTEXITCODE -eq 0) {
        & git -C $source apply $patch.FullName
        if ($LASTEXITCODE -ne 0) { throw "Could not apply $($patch.Name)." }
        continue
    }
    & git -C $source apply --reverse --check $patch.FullName 2>$null
    if ($LASTEXITCODE -ne 0) { throw "The source does not match $($patch.Name)." }
}

# MuPDF's Visual Studio project consumes OpenSSL from these legacy-shaped
# paths. The caller supplies an isolated OpenSSL 3.x static MSVC build.
$opensslHeaders = Join-Path $openssl 'include/openssl'
$opensslCrypto = Join-Path $openssl 'x64/lib/libcrypto.lib'
if (!(Test-Path $opensslHeaders) -or !(Test-Path $opensslCrypto)) {
    throw "OpenSSLRoot needs include/openssl and x64/lib/libcrypto.lib: $openssl"
}
$expectedOpenSSL = Join-Path $source 'thirdparty/openssl'
New-Item -ItemType Directory -Force -Path (Join-Path $expectedOpenSSL 'include/openssl'), (Join-Path $expectedOpenSSL 'x64/lib') | Out-Null
$expectedHeaders = (Resolve-Path -LiteralPath (Join-Path $expectedOpenSSL 'include/openssl')).Path
$expectedCrypto = Join-Path $expectedOpenSSL 'x64/lib/libcrypto.lib'
if ($opensslHeaders -ne $expectedHeaders) {
    Copy-Item (Join-Path $opensslHeaders '*') $expectedHeaders -Force
}
if ((Resolve-Path -LiteralPath $opensslCrypto).Path -ne $expectedCrypto) {
    Copy-Item $opensslCrypto $expectedCrypto -Force
}

function Edit-ReleaseX64([string]$Project, [scriptblock]$Edit) {
    $path = Join-Path $source "platform/win32/$Project"
    $text = [IO.File]::ReadAllText($path)
    $marker = '<ItemDefinitionGroup Condition="''$(Configuration)|$(Platform)''==''Release|x64''">'
    $start = $text.IndexOf($marker)
    if ($start -lt 0) { throw "$Project has no Release|x64 configuration." }
    $end = $text.IndexOf('</ItemDefinitionGroup>', $start)
    if ($end -lt 0) { throw "$Project has a malformed Release|x64 configuration." }
    $block = $text.Substring($start, $end - $start)
    $updated = & $Edit $block
    if ($updated -eq $block) { return }
    $text = $text.Substring(0, $start) + $updated + $text.Substring($end)
    [IO.File]::WriteAllText($path, $text, [Text.UTF8Encoding]::new($true))
}

# Build the engine and every static dependency with one CRT. This is required
# because Rust's MSVC target links libcmt, while upstream Windows projects
# default to the dynamic CRT and otherwise leave __imp_* CRT imports unresolved.
$staticProjects = @(
    'libmupdf.vcxproj', 'libthirdparty.vcxproj', 'libextract.vcxproj',
    'libharfbuzz.vcxproj', 'libmubarcode.vcxproj', 'libzxing.vcxproj',
    'libtesseract.vcxproj', 'libleptonica.vcxproj', 'libpkcs7.vcxproj'
)
foreach ($project in $staticProjects) {
    Edit-ReleaseX64 $project {
        param($block)
        $old = '<RuntimeLibrary>MultiThreadedDLL</RuntimeLibrary>'
        $new = '<RuntimeLibrary>MultiThreaded</RuntimeLibrary>'
        if (!$block.Contains($old) -and !$block.Contains($new)) {
            throw "The Release|x64 compile settings in $project do not declare RuntimeLibrary."
        }
        $block.Replace($old, $new)
    }
}

# Release already enables Tesseract and Leptonica. Add the OpenSSL-backed PDF
# signature path without enabling ReleaseExtra's optional libarchive inputs.
Edit-ReleaseX64 'libmupdf.vcxproj' {
    param($block)
    $block = $block.Replace(
        '..\..\thirdparty\extract\include;%(AdditionalIncludeDirectories)',
        '..\..\thirdparty\extract\include;..\..\thirdparty\openssl\include;%(AdditionalIncludeDirectories)'
    )
    $block.Replace(
        'HAVE_TESSERACT;HAVE_LIBCRYPTO;WIN64;NDEBUG;OPJ_STATIC',
        'HAVE_TESSERACT;HAVE_LIBCRYPTO;WIN64;NDEBUG;OPJ_STATIC'
    )
}
Edit-ReleaseX64 'libpkcs7.vcxproj' {
    param($block)
    $block = $block.Replace(
        '..\..\include;%(AdditionalIncludeDirectories)',
        '..\..\include;..\..\thirdparty\openssl\include;%(AdditionalIncludeDirectories)'
    )
    $block = $block.Replace(
        'WIN64;NDEBUG;%(PreprocessorDefinitions)',
        'WIN64;NDEBUG;HAVE_LIBCRYPTO;%(PreprocessorDefinitions)'
    )
    $block = $block.Replace(
        '</ClCompile>',
        "</ClCompile>`r`n    <Lib>`r`n      <AdditionalDependencies>libcrypto.lib;%(AdditionalDependencies)</AdditionalDependencies>`r`n      <AdditionalLibraryDirectories>..\..\thirdparty\openssl\x64\lib;%(AdditionalLibraryDirectories)</AdditionalLibraryDirectories>`r`n    </Lib>"
    )
    [regex]::Replace(
        $block,
        '(?s)(<CustomBuildStep>\s*<Command>).*?(</Command>)',
        '$1echo OpenSSL 3.x is statically linked; skip legacy DLL copies$2',
        1
    )
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$installation = (& $vswhere -latest -products '*' -property installationPath).Trim()
if ($LASTEXITCODE -ne 0 -or !$installation) { throw 'Could not locate Visual Studio with vswhere.' }
$vcvarsall = Join-Path $installation 'VC/Auxiliary/Build/vcvarsall.bat'
$msbuild = Join-Path $installation 'MSBuild/Current/Bin/MSBuild.exe'
$solution = Join-Path $source 'platform/win32/mupdf.sln'
$command = 'call "{0}" amd64 {1} >nul && "{2}" "{3}" /nologo /m:{4} /verbosity:minimal /t:libmupdf /p:Configuration=Release /p:Platform=x64 /p:PlatformToolset={5} /p:WindowsTargetPlatformVersion={1}' -f `
    $vcvarsall, $WindowsTargetPlatformVersion, $msbuild, $solution, $MaxCpuCount, $PlatformToolset
& cmd.exe /d /s /c $command
if ($LASTEXITCODE -ne 0) { throw "MuPDF MSBuild failed with exit code $LASTEXITCODE." }

$output = Join-Path $source 'platform/win32/x64/Release'
foreach ($library in @('libmupdf.lib', 'libthirdparty.lib', 'libresources.lib', 'libextract.lib', 'libharfbuzz.lib', 'libmubarcode.lib', 'libzxing.lib', 'libpkcs7.lib', 'libtesseract.lib', 'libleptonica.lib')) {
    if (!(Test-Path (Join-Path $output $library))) { throw "MuPDF output is missing $library." }
}
Write-Output "MuPDF $MuPDFCommit built for x64; set ELY_MUPDF_DIR=$output and ELY_MUPDF_INCLUDE=$(Join-Path $source 'include')."
