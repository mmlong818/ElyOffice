[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $OutputPath,
    [string] $CefStage,
    [string] $PdfStage,
    [Parameter(Mandatory)] [string] $LibreOfficeRoot,
    [Parameter(Mandatory)] [string] $LibreOfficeLicenses,
    [string] $LokAppStage,
    [string] $ReleaseBin,
    [string] $GitRoot,
    [string] $TempRoot
)

 $oldTemp = $env:TEMP
 $oldTmp = $env:TMP
 $tempConfigured = $false
try {
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$work = Split-Path -Parent $repo
$OutputPath = [IO.Path]::GetFullPath($OutputPath)
if (-not $TempRoot) { $TempRoot = "$OutputPath.temp" }
$TempRoot = [IO.Path]::GetFullPath($TempRoot)
if ([IO.Path]::GetPathRoot($TempRoot) -eq [IO.Path]::GetPathRoot($env:SystemRoot)) { throw "TempRoot must not use the system drive: $TempRoot" }
if ($TempRoot.Equals($OutputPath, [StringComparison]::OrdinalIgnoreCase) -or
    $TempRoot.StartsWith($OutputPath.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
    $OutputPath.StartsWith($TempRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'TempRoot cannot overlap the suite stage path.' }
function Assert-NoReparseAncestors([string] $Path) {
    $current = [IO.Path]::GetFullPath($Path)
    while ($current) {
        if (Test-Path -LiteralPath $current) {
            $item = Get-Item -LiteralPath $current -Force
            if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Path ancestor is a reparse point: $current" }
        }
        $parent = [IO.Directory]::GetParent($current)
        if (-not $parent) { break }
        $current = $parent.FullName
    }
}
Assert-NoReparseAncestors $TempRoot
New-Item -ItemType Directory -Path $TempRoot -Force | Out-Null
$env:TEMP = $TempRoot
$env:TMP = $TempRoot
$tempConfigured = $true
if (-not $CefStage) { $CefStage = Join-Path $work 'cef-windows\stage' }
if (-not $PdfStage) { $PdfStage = Join-Path $work 'mupdf-windows\stage\final' }
if (-not $LokAppStage) { $LokAppStage = Join-Path $work 'lok-windows\app-stage' }
if (-not $ReleaseBin) { $ReleaseBin = Join-Path $repo 'target\release' }
if (-not $GitRoot) { $GitRoot = Join-Path $work 'tools\MinGit-stable-2.56.0.2-64-bit' }

if (Test-Path -LiteralPath $OutputPath) {
    throw "Refusing to overwrite existing stage: $OutputPath"
}
$outputParent = Split-Path -Parent $OutputPath
if (-not (Test-Path -LiteralPath $outputParent -PathType Container)) { throw "Output parent directory is missing: $outputParent" }
Assert-NoReparseAncestors $outputParent

function Assert-NoReparseTree([string] $Path) {
    $root = Get-Item -LiteralPath $Path -Force
    if (($root.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Stage source root is a reparse point: $Path"
    }
    $link = Get-ChildItem -LiteralPath $Path -Force -Recurse -ErrorAction Stop |
        Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 } |
        Select-Object -First 1
    if ($link) { throw "Stage source contains a reparse point: $($link.FullName)" }
}

foreach ($source in @($CefStage, $PdfStage, $LibreOfficeRoot, $LibreOfficeLicenses, $LokAppStage, $ReleaseBin, $GitRoot)) {
    if (-not (Test-Path -LiteralPath $source -PathType Container)) {
        throw "Required stage source is missing: $source"
    }
}
if ((Get-Item -LiteralPath $LokAppStage -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
    throw "LOK app stage root cannot be a reparse point: $LokAppStage"
}
foreach ($source in @($CefStage, $PdfStage, $LibreOfficeRoot, $LibreOfficeLicenses, $GitRoot)) {
    Assert-NoReparseTree $source
}

$cefManifest = Join-Path (Split-Path -Parent $CefStage) 'stage-payload.manifest.json'
if (-not (Test-Path -LiteralPath $cefManifest -PathType Leaf) -or
    (Get-FileHash -LiteralPath $cefManifest -Algorithm SHA256).Hash -cne '210C6F12AFEBD1942511B3872AE2CA4557EBDE544BD20259056A30224980D97B') {
    throw 'CEF producer manifest is missing or differs from the reviewed final stage.'
}
$pdfProvenance = Join-Path $PdfStage 'BUILD-PROVENANCE.txt'
if (-not (Test-Path -LiteralPath $pdfProvenance -PathType Leaf) -or
    (Get-FileHash -LiteralPath $pdfProvenance -Algorithm SHA256).Hash -cne '08E762F1F63136A3110955A3622C2C9229D6FE350C2760B5B29E1FB418A35C7D') {
    throw 'PDF provenance is missing or differs from the reviewed final stage.'
}

foreach ($relative in @('program\soffice.exe', 'program\sofficeapp.dll', 'share')) {
    if (-not (Test-Path -LiteralPath (Join-Path $LibreOfficeRoot $relative))) {
        throw "LibreOffice runtime is incomplete; missing $relative"
    }
}
foreach ($name in @('LICENSE', 'LICENSE.html', 'NOTICE', 'CREDITS.fodt')) {
    if (-not (Test-Path -LiteralPath (Join-Path $LibreOfficeLicenses $name) -PathType Leaf)) {
        throw "LibreOffice license source is incomplete; missing $name"
    }
}
foreach ($name in @('ely-doc.exe', 'ely-table.exe', 'ely-show.exe', 'ely-lok-worker.exe')) {
    $app = Join-Path $LokAppStage $name
    if (-not (Test-Path -LiteralPath $app -PathType Leaf)) {
        throw "LOK app stage is missing $name"
    }
    if ((Get-Item -LiteralPath $app -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) {
        throw "LOK executable cannot be a reparse point: $app"
    }
}
foreach ($name in @('ely-tree.exe', 'ely-mail.exe', 'ely-agent.exe')) {
    if (-not (Test-Path -LiteralPath (Join-Path $ReleaseBin $name) -PathType Leaf)) {
        throw "Release build directory is missing $name"
    }
}
if (-not (Test-Path -LiteralPath (Join-Path $GitRoot 'cmd\git.exe') -PathType Leaf)) {
    throw 'Git for Windows runtime is missing cmd\git.exe.'
}

New-Item -ItemType Directory -Path $OutputPath | Out-Null
function Copy-Merge([string] $Source, [string] $DestinationBase, [string] $Label) {
    $sourceBase = [IO.Path]::GetFullPath($Source).TrimEnd('\')
    $destinationBase = [IO.Path]::GetFullPath($DestinationBase).TrimEnd('\')
    foreach ($file in Get-ChildItem -LiteralPath $sourceBase -File -Recurse -Force) {
        $relative = $file.FullName.Substring($sourceBase.Length).TrimStart('\')
        $destination = Join-Path $destinationBase $relative
        New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
        if (Test-Path -LiteralPath $destination -PathType Leaf) {
            $sourceHash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
            $destinationHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
            if ($sourceHash -cne $destinationHash) {
                throw "Different component files collide at '$relative' ($Label)."
            }
            continue
        }
        Copy-Item -LiteralPath $file.FullName -Destination $destination
    }
}

try {
    Copy-Merge $CefStage $OutputPath 'CEF'
    Copy-Merge $PdfStage $OutputPath 'PDF'
    Copy-Merge $LibreOfficeRoot (Join-Path $OutputPath 'engines\libreoffice') 'LibreOffice runtime'
    foreach ($name in @('ely-doc.exe', 'ely-table.exe', 'ely-show.exe', 'ely-lok-worker.exe')) {
        Copy-Item -LiteralPath (Join-Path $LokAppStage $name) -Destination (Join-Path $OutputPath $name)
    }
    foreach ($name in @('ely-tree.exe', 'ely-mail.exe', 'ely-agent.exe')) {
        Copy-Item -LiteralPath (Join-Path $ReleaseBin $name) -Destination (Join-Path $OutputPath $name)
    }
    Copy-Item -LiteralPath $GitRoot -Destination (Join-Path $OutputPath 'git') -Recurse
    foreach ($slug in @('ely-agent', 'ely-doc', 'ely-mail', 'ely-mark', 'ely-pdf', 'ely-show', 'ely-table', 'ely-tree')) {
        Copy-Item -LiteralPath (Join-Path $repo "assets\icons\$slug.ico") -Destination (Join-Path $OutputPath "$slug.ico")
    }
    $head = (& git -C $repo rev-parse HEAD).Trim()
    $marker = "ElyOffice product marker v1`nVersion=0.1.0`nSourceCommit=$head`n"
    [IO.File]::WriteAllText((Join-Path $OutputPath 'elyoffice.product'), $marker, [Text.Encoding]::ASCII)
    New-Item -ItemType Directory -Path (Join-Path $OutputPath 'Licenses') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $repo 'LICENSE') -Destination (Join-Path $OutputPath 'Licenses\ElyOffice-LICENSE')
    Copy-Item -LiteralPath $LibreOfficeLicenses -Destination (Join-Path $OutputPath 'Licenses\LibreOffice') -Recurse

    $requiredFiles = @(
        'ely-doc.exe', 'ely-table.exe', 'ely-show.exe', 'ely-pdf.exe', 'ely-mark.exe',
        'ely-tree.exe', 'ely-mail.exe', 'ely-agent.exe', 'ely-lok-worker.exe',
        'ely-pdf-worker.exe', 'git\cmd\git.exe', 'libcef.dll', 'icudtl.dat',
        'engines\libreoffice\program\soffice.exe', 'engines\libreoffice\program\sofficeapp.dll'
    )
    foreach ($relative in $requiredFiles) {
        if (-not (Test-Path -LiteralPath (Join-Path $OutputPath $relative) -PathType Leaf)) {
            throw "Assembled suite is missing required runtime file: $relative"
        }
    }

    $files = Get-ChildItem -LiteralPath $OutputPath -File -Recurse -Force |
        Where-Object { $_.Name -ne 'payload.manifest.json' } |
        Sort-Object FullName |
        ForEach-Object {
            [ordered]@{
                path = $_.FullName.Substring($OutputPath.Length).TrimStart('\')
                length = [long] $_.Length
                sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
            }
        }
    $manifest = [ordered]@{
        status = 'READY_FOR_INSTALLER_REVIEW_NOT_INSTALL_TESTED'
        sourceCommit = $head
        cefManifestSha256 = (Get-FileHash -LiteralPath $cefManifest -Algorithm SHA256).Hash
        pdfBuildProvenanceSha256 = (Get-FileHash -LiteralPath $pdfProvenance -Algorithm SHA256).Hash
        sources = [ordered]@{
            cef = [IO.Path]::GetFullPath($CefStage)
            pdf = [IO.Path]::GetFullPath($PdfStage)
            libreOffice = [IO.Path]::GetFullPath($LibreOfficeRoot)
            libreOfficeLicenses = [IO.Path]::GetFullPath($LibreOfficeLicenses)
            lokApps = [IO.Path]::GetFullPath($LokAppStage)
            releaseBin = [IO.Path]::GetFullPath($ReleaseBin)
            git = [IO.Path]::GetFullPath($GitRoot)
        }
        files = @($files)
    }
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputPath 'payload.manifest.json') -Encoding utf8
    Write-Output "Assembled suite stage: $OutputPath"
    Write-Output "Source commit: $head"
    Write-Output "Manifest SHA-256: $((Get-FileHash -LiteralPath (Join-Path $OutputPath 'payload.manifest.json') -Algorithm SHA256).Hash)"
} catch {
    throw "Suite stage assembly failed. Partial stage retained for inspection at '$OutputPath'. $($_.Exception.Message)"
}
} finally {
    if ($tempConfigured) { $env:TEMP = $oldTemp; $env:TMP = $oldTmp }
}
