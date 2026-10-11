param(
    [Parameter(Mandatory = $true)]
    [string]$App,

    [Parameter(Mandatory = $true)]
    [string]$Worker,

    [Parameter(Mandatory = $true)]
    [string]$MuPDFSource,

    [Parameter(Mandatory = $true)]
    [string]$OpenSSLSource,

    [Parameter(Mandatory = $true)]
    [string]$Tessdata,

    [Parameter(Mandatory = $true)]
    [string]$Destination
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$MuPDFCommit = '8ad45e92f0935d3d87f1db3f873086472a5e1b24'
$OpenSSLCommit = '45e844fa2a14ec92d146bd8f5778ac130b6625fb'
$TessdataCommit = '87416418657359cb625c412a48b6e1d6d41c29bd'
$MuPDFSource = (Resolve-Path -LiteralPath $MuPDFSource).Path
$OpenSSLSource = (Resolve-Path -LiteralPath $OpenSSLSource).Path
$App = (Resolve-Path -LiteralPath $App).Path
$Worker = (Resolve-Path -LiteralPath $Worker).Path
$Tessdata = (Resolve-Path -LiteralPath $Tessdata).Path
$Destination = [IO.Path]::GetFullPath($Destination)
if ((& git -C $MuPDFSource rev-parse HEAD).Trim() -ne $MuPDFCommit) { throw 'MuPDF source pin mismatch.' }
if ((& git -C $OpenSSLSource rev-parse HEAD).Trim() -ne $OpenSSLCommit) { throw 'OpenSSL source pin mismatch.' }

$hashes = [ordered]@{
    'LICENSE' = 'cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30'
    'eng.traineddata' = '7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2'
    'chi_sim.traineddata' = 'a5fcb6f0db1e1d6d8522f39db4e848f05984669172e584e8d76b6b3141e1f730'
    'chi_tra.traineddata' = '529c5b5797d64b126065cd55f2bb4c7fd7b15790798091b1ff259941a829330b'
}
foreach ($name in $hashes.Keys) {
    $file = Join-Path $Tessdata $name
    if (!(Test-Path $file) -or (Get-FileHash $file -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hashes[$name]) {
        throw "Tessdata artifact failed its pinned SHA-256: $name"
    }
}

$licenseDir = Join-Path $Destination 'licenses'
$dataDir = Join-Path $Destination 'tessdata'
New-Item -ItemType Directory -Force -Path $dataDir, $licenseDir, (Join-Path $Destination 'build-repro/scripts'), (Join-Path $Destination 'build-repro/patches/mupdf') | Out-Null
Copy-Item $App (Join-Path $Destination 'ely-pdf.exe') -Force
Copy-Item $Worker (Join-Path $Destination 'ely-pdf-worker.exe') -Force
foreach ($name in $hashes.Keys) { Copy-Item (Join-Path $Tessdata $name) (Join-Path $dataDir $name) -Force }
foreach ($script in 'fetch-tessdata-windows.ps1', 'build-openssl-windows.ps1', 'build-mupdf-windows.ps1', 'stage-mupdf-windows.ps1') {
    Copy-Item (Join-Path $PSScriptRoot $script) (Join-Path $Destination "build-repro/scripts/$script") -Force
}
Copy-Item (Join-Path $repo 'patches/mupdf/*.patch') (Join-Path $Destination 'build-repro/patches/mupdf') -Force

$licenseFiles = @(
    @('MuPDF-COPYING', (Join-Path $MuPDFSource 'COPYING')),
    @('OpenSSL-LICENSE.txt', (Join-Path $OpenSSLSource 'LICENSE.txt')),
    @('Tesseract-LICENSE', (Join-Path $MuPDFSource 'thirdparty/tesseract/LICENSE')),
    @('Leptonica-LICENSE.txt', (Join-Path $MuPDFSource 'thirdparty/leptonica/leptonica-license.txt')),
    @('Droid-font-NOTICE', (Join-Path $MuPDFSource 'resources/fonts/droid/NOTICE')),
    @('SourceHanSerif-font-LICENSE.txt', (Join-Path $MuPDFSource 'resources/fonts/han/LICENSE.txt')),
    @('Noto-font-COPYING', (Join-Path $MuPDFSource 'resources/fonts/noto/COPYING')),
    @('URW-font-OFL.txt', (Join-Path $MuPDFSource 'resources/fonts/urw/OFL.txt')),
    @('FreeType-LICENSE.txt', (Join-Path $MuPDFSource 'thirdparty/freetype/LICENSE.TXT')),
    @('HarfBuzz-COPYING', (Join-Path $MuPDFSource 'thirdparty/harfbuzz/COPYING')),
    @('OpenJPEG-LICENSE', (Join-Path $MuPDFSource 'thirdparty/openjpeg/LICENSE')),
    @('cmark-gfm-COPYING', (Join-Path $MuPDFSource 'thirdparty/cmark-gfm/COPYING')),
    @('zint-LICENSE', (Join-Path $MuPDFSource 'thirdparty/zint/LICENSE')),
    @('ZXing-LICENSE', (Join-Path $MuPDFSource 'thirdparty/zxing-cpp/LICENSE')),
    @('libextract-COPYING', (Join-Path $MuPDFSource 'thirdparty/extract/COPYING')),
    @('zlib-LICENSE', (Join-Path $MuPDFSource 'thirdparty/zlib/LICENSE')),
    @('Brotli-LICENSE', (Join-Path $MuPDFSource 'thirdparty/brotli/LICENSE')),
    @('LCMS-LICENSE', (Join-Path $MuPDFSource 'thirdparty/lcms2/LICENSE')),
    @('jbig2dec-COPYING', (Join-Path $MuPDFSource 'thirdparty/jbig2dec/COPYING')),
    @('MuJS-COPYING', (Join-Path $MuPDFSource 'thirdparty/mujs/COPYING')),
    @('Gumbo-COPYING', (Join-Path $MuPDFSource 'thirdparty/gumbo-parser/COPYING'))
)
foreach ($item in $licenseFiles) {
    if (Test-Path $item[1]) { Copy-Item $item[1] (Join-Path $licenseDir $item[0]) -Force }
}

$buildInfo = Join-Path $Destination 'BUILD-PROVENANCE.txt'
$lines = [Collections.Generic.List[string]]::new()
$lines.Add('ElyOffice Windows PDF engine payload')
$lines.Add("MuPDF source: https://github.com/ArtifexSoftware/mupdf.git at $MuPDFCommit (patched by patches/mupdf/*.patch)")
$lines.Add("OpenSSL source: https://github.com/openssl/openssl.git openssl-3.5.9 at $OpenSSLCommit; built VC-WIN64A no-shared no-tests no-asm")
$lines.Add("OCR source: https://github.com/tesseract-ocr/tessdata_fast at $TessdataCommit")
$lines.Add('Portable Perl: Strawberry Perl 5.42.3.1 ZIP; SHA256 6a081a811781c30aca51dbc036afd93092af91e3297901f02c17043795a10690 (native Perl, extracted only under work).')
$lines.Add('Native build scripts: scripts/fetch-tessdata-windows.ps1, scripts/build-openssl-windows.ps1, and scripts/build-mupdf-windows.ps1')
$lines.Add('Reproduction scripts and all five MuPDF patches are included under build-repro/.')
$lines.Add('Feature macros: HAVE_TESSERACT, HAVE_LEPTONICA, HAVE_LIBCRYPTO; OpenSSL is linked statically.')
$lines.Add('MuPDF embedded font notices are included for Droid, Source Han, Noto, and URW resources in libresources.lib.')
$lines.Add('The PDF app and ely-pdf-worker.exe are staged together at the payload root; the runtime reads payload-root/tessdata.')
$crypto = Join-Path $MuPDFSource 'thirdparty/openssl/x64/lib/libcrypto.lib'
if (Test-Path $crypto) { $lines.Add("Static libcrypto SHA256: $((Get-FileHash $crypto -Algorithm SHA256).Hash.ToLowerInvariant())") }
$lines.Add('Compiler: Visual Studio 2022 Build Tools v143; Windows SDK 10.0.26100.0; Rust target x86_64-pc-windows-msvc.')
$submodules = & git -C $MuPDFSource submodule status --recursive
if ($LASTEXITCODE -ne 0) { throw 'Could not read pinned MuPDF submodule commits.' }
foreach ($submodule in $submodules) { $lines.Add("MuPDF submodule pin: $submodule") }
foreach ($patch in Get-ChildItem (Join-Path $repo 'patches/mupdf') -Filter '*.patch' | Sort-Object Name) {
    $lines.Add("Patch SHA256 $($patch.Name): $((Get-FileHash $patch.FullName -Algorithm SHA256).Hash.ToLowerInvariant())")
}
foreach ($name in $hashes.Keys) { $lines.Add("Tessdata SHA256 ${name}: $($hashes[$name])") }
Set-Content -LiteralPath $buildInfo -Value $lines -Encoding utf8
Write-Output "Staged PDF worker, OCR languages, licenses, and provenance at $Destination"
