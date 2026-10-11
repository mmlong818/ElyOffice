param(
    [Parameter(Mandatory = $true)]
    [string]$OpenSSLSource,

    [Parameter(Mandatory = $true)]
    [string]$PerlRoot,

    [Parameter(Mandatory = $true)]
    [string]$OutputRoot,

    [string]$WindowsTargetPlatformVersion = '10.0.26100.0'
)

$ErrorActionPreference = 'Stop'
$OpenSSLCommit = '45e844fa2a14ec92d146bd8f5778ac130b6625fb'
$source = (Resolve-Path -LiteralPath $OpenSSLSource).Path
$perl = (Resolve-Path -LiteralPath (Join-Path $PerlRoot 'perl/bin/perl.exe')).Path
$perlBin = Split-Path -Parent $perl
$output = [IO.Path]::GetFullPath($OutputRoot)
$actualCommit = (& git -C $source rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $actualCommit -ne $OpenSSLCommit) {
    throw "OpenSSL source must be pinned to $OpenSSLCommit (found $actualCommit)."
}
if (!(Test-Path (Join-Path $source 'NOTES-WINDOWS.md'))) {
    throw "Not an OpenSSL source tree: $source"
}

$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$installation = (& $vswhere -latest -products '*' -property installationPath).Trim()
if ($LASTEXITCODE -ne 0 -or !$installation) { throw 'Could not locate Visual Studio with vswhere.' }
$vcvarsall = Join-Path $installation 'VC/Auxiliary/Build/vcvarsall.bat'
$nmake = Join-Path $installation 'VC/Tools/MSVC'
$vsVersion = Get-ChildItem $nmake -Directory | Sort-Object Name -Descending | Select-Object -First 1
if (!$vsVersion -or !(Test-Path (Join-Path $vsVersion.FullName 'bin/Hostx64/x64/nmake.exe'))) {
    throw 'Could not locate the Visual Studio x64 nmake.exe.'
}
$nmake = Join-Path $vsVersion.FullName 'bin/Hostx64/x64/nmake.exe'
$configure = Join-Path $source 'Configure'
$install = $output
New-Item -ItemType Directory -Force -Path $output | Out-Null

# OpenSSL 3.5's official Windows instructions use native Windows Perl and
# nmake. Keep the portable Perl, compiler environment, and install tree local.
$command = 'set "PATH={2};%PATH%" && call "{0}" amd64 {1} >nul && cd /d "{3}" && "{4}" "{5}" VC-WIN64A no-shared no-tests no-asm --prefix="{6}" --openssldir="{6}/ssl" && "{7}" /nologo && "{7}" install_dev' -f `
    $vcvarsall, $WindowsTargetPlatformVersion, $perlBin, $source, $perl, $configure, $install, $nmake
& cmd.exe /d /s /c $command
if ($LASTEXITCODE -ne 0) { throw "OpenSSL Configure/nmake failed with exit code $LASTEXITCODE." }

$destination = Join-Path $output 'x64/lib'
New-Item -ItemType Directory -Force -Path $destination | Out-Null
Copy-Item (Join-Path $install 'lib/libcrypto.lib') (Join-Path $destination 'libcrypto.lib') -Force
if (!(Test-Path (Join-Path $install 'include/openssl/opensslv.h'))) { throw 'OpenSSL headers were not installed.' }
Write-Output "OpenSSL $OpenSSLCommit built statically with VC-WIN64A no-shared no-tests no-asm; output $output."
