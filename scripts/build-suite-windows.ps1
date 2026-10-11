[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $StagePath,
    [Parameter(Mandatory)] [string] $OutputPath,
    [Parameter(Mandatory)] [string] $MakeNSIS,
    [string] $TempRoot
)

 $oldTemp = $env:TEMP
 $oldTmp = $env:TMP
 $tempConfigured = $false
try {
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$work = Split-Path -Parent $repo
$StagePath = [IO.Path]::GetFullPath($StagePath)
$OutputPath = [IO.Path]::GetFullPath($OutputPath)
if (-not $TempRoot) { $TempRoot = "$OutputPath.temp" }
$TempRoot = [IO.Path]::GetFullPath($TempRoot)
if ([IO.Path]::GetPathRoot($TempRoot) -eq [IO.Path]::GetPathRoot($env:SystemRoot)) { throw "TempRoot must not use the system drive: $TempRoot" }
if ($TempRoot.Equals($StagePath, [StringComparison]::OrdinalIgnoreCase) -or
    $TempRoot.StartsWith($StagePath.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
    $TempRoot.Equals($OutputPath, [StringComparison]::OrdinalIgnoreCase) -or
    $TempRoot.StartsWith($OutputPath.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
    $StagePath.StartsWith($TempRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
    $OutputPath.StartsWith($TempRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'TempRoot cannot overlap the suite stage or installer output path.' }
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

$MakeNSIS = [IO.Path]::GetFullPath($MakeNSIS)
$nsi = Join-Path $repo 'packaging\windows\installer-suite.nsi'
if (Test-Path -LiteralPath $OutputPath) {
    throw "Refusing to overwrite an existing installer: $OutputPath"
}
foreach ($path in @($StagePath, (Split-Path -Parent $OutputPath), (Split-Path -Parent $MakeNSIS))) {
    if (-not (Test-Path -LiteralPath $path -PathType Container)) {
        throw "Required directory is missing: $path"
    }
}
Assert-NoReparseAncestors (Split-Path -Parent $OutputPath)
if (-not (Test-Path -LiteralPath $MakeNSIS -PathType Leaf)) {
    throw "NSIS compiler is missing: $MakeNSIS"
}
if (Test-Path -LiteralPath (Join-Path $StagePath 'INCOMPLETE-STAGE.txt') -PathType Leaf) {
    throw 'This suite stage is explicitly marked incomplete; installer build refused.'
}
$manifestPath = Join-Path $StagePath 'payload.manifest.json'
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw 'Suite payload manifest is missing; installer build refused.'
}
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.status -cne 'READY_FOR_INSTALLER_REVIEW_NOT_INSTALL_TESTED') {
    throw "Suite payload manifest is not ready for packaging review: $($manifest.status)"
}
$head = (& git -C $repo rev-parse HEAD).Trim()
if ($manifest.sourceCommit -cne $head) {
    throw "Suite manifest source commit $($manifest.sourceCommit) differs from the checked-out source $head."
}
$stageRootItem = Get-Item -LiteralPath $StagePath -Force
if (($stageRootItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw "Suite stage root is a reparse point: $StagePath"
}
$reparse = Get-ChildItem -LiteralPath $StagePath -Force -Recurse -ErrorAction Stop |
    Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 } |
    Select-Object -First 1
if ($reparse) { throw "Suite stage contains a reparse point: $($reparse.FullName)" }
$expectedFiles = @($manifest.files)
$expectedCount = $expectedFiles.Count
$actualFiles = @(Get-ChildItem -LiteralPath $StagePath -File -Recurse -Force |
    Where-Object { $_.FullName -cne $manifestPath })
if ($actualFiles.Count -ne $expectedCount) {
    throw "Suite stage file count $($actualFiles.Count) differs from manifest count $expectedCount."
}
$requiredFiles = @(
    'ely-doc.exe', 'ely-table.exe', 'ely-show.exe', 'ely-pdf.exe', 'ely-mark.exe',
    'ely-tree.exe', 'ely-mail.exe', 'ely-agent.exe', 'ely-lok-worker.exe',
    'ely-pdf-worker.exe', 'git\cmd\git.exe', 'libcef.dll', 'icudtl.dat',
    'ely-doc.ico', 'ely-table.ico', 'ely-show.ico', 'ely-pdf.ico',
    'ely-mark.ico', 'ely-tree.ico', 'ely-mail.ico', 'ely-agent.ico', 'elyoffice.product'
)
foreach ($relative in $requiredFiles) {
    if (-not (Test-Path -LiteralPath (Join-Path $StagePath $relative) -PathType Leaf)) {
        throw "Incomplete suite stage; missing file: $relative"
    }
}
foreach ($relative in @(
    'engines\libreoffice\program\soffice.exe',
    'engines\libreoffice\program\sofficeapp.dll',
    'tessdata',
    'Resources\content\ely-mark',
    'Licenses'
)) {
    if (-not (Test-Path -LiteralPath (Join-Path $StagePath $relative))) {
        throw "Incomplete suite stage; missing runtime or notices: $relative"
    }
}

$seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$deleteFiles = [Collections.Generic.List[string]]::new()
$installGuardLines = [Collections.Generic.List[string]]::new()
$directories = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
function ConvertTo-NsisRelativePath([string] $relative) {
    if ([string]::IsNullOrWhiteSpace($relative) -or $relative.Contains('/') -or
        $relative -match '[\r\n\x00:*?"<>|]' -or [IO.Path]::IsPathRooted($relative)) {
        throw "Unsafe relative path in suite manifest: '$relative'"
    }
    $segments = $relative -split '\\'
    if ($segments -contains '.' -or $segments -contains '..' -or $segments -contains '') {
        throw "Unsafe path segment in suite manifest: '$relative'"
    }
    if ($segments | Where-Object { $_.EndsWith('.') -or $_.EndsWith(' ') }) {
        throw "Non-canonical Windows path segment in suite manifest: '$relative'"
    }
    if (-not $seen.Add($relative)) { throw "Duplicate/case-colliding manifest path: '$relative'" }

    $full = [IO.Path]::GetFullPath((Join-Path $StagePath $relative))
    if (-not $full.StartsWith($StagePath.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
        throw "Manifest path escapes suite stage: '$relative'"
    }
    $file = Get-Item -LiteralPath $full -Force
    if ($file.PSIsContainer -or (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0)) {
        throw "Manifest file is missing, a directory, or a reparse point: '$relative'"
    }
    $escaped = $relative.Replace('$', '$$').Replace('"', '$\"')
    $deleteFiles.Add("Delete `"`$INSTDIR\$escaped`"")
    $installGuardLines.Add("IfFileExists `"`$INSTDIR\$escaped`" install_payload_conflict")

    for ($index = 1; $index -lt $segments.Count; $index++) {
        $directory = $segments[0..($index - 1)] -join '\'
        $directories.Add($directory) | Out-Null
    }
    return $full
}

foreach ($entry in $expectedFiles) {
    $full = ConvertTo-NsisRelativePath ([string] $entry.path)
    $file = Get-Item -LiteralPath $full -Force
    if ($file.Length -ne $entry.length -or
        (Get-FileHash -LiteralPath $full -Algorithm SHA256).Hash -cne $entry.sha256) {
        throw "Suite stage file differs from manifest: $($entry.path)"
    }
}
$manifestHash = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash
if (-not (Test-Path -LiteralPath (Join-Path $StagePath 'payload.manifest.json') -PathType Leaf)) {
    throw 'Manifest itself is missing from the installer stage.'
}
$manifestPathInPackage = ConvertTo-NsisRelativePath 'payload.manifest.json'
$manifestFile = Get-Item -LiteralPath $manifestPathInPackage -Force
if (($manifestFile.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw 'Manifest itself is a reparse point.'
}
$directoryDeleteLines = @($directories | Sort-Object `
    @{ Expression = { ($_ -split '\\').Count }; Descending = $true }, `
    @{ Expression = { $_.Length }; Descending = $true }, `
    @{ Expression = { $_.ToLowerInvariant() }; Descending = $false } |
    ForEach-Object {
        $escaped = $_.Replace('$', '$$').Replace('"', '$\"')
        "RMDir `"`$INSTDIR\$escaped`""
    })

$outputDirectory = Split-Path -Parent $OutputPath
$baseName = [IO.Path]::GetFileNameWithoutExtension($OutputPath)
$uninstallFileInclude = Join-Path $outputDirectory "$baseName.uninstall-files.nsh"
$uninstallDirInclude = Join-Path $outputDirectory "$baseName.uninstall-dirs.nsh"
$installGuardInclude = Join-Path $outputDirectory "$baseName.install-guards.nsh"
function Write-DeterministicInclude([string] $path, [string[]] $lines) {
    $content = ($lines -join "`r`n") + "`r`n"
    if (Test-Path -LiteralPath $path -PathType Leaf) {
        if ([IO.File]::ReadAllText($path) -cne $content) {
            throw "Refusing to overwrite a different generated NSIS include: $path"
        }
        return
    }
    [IO.File]::WriteAllText($path, $content, [Text.UTF8Encoding]::new($false))
}
$deleteFiles.Insert(0, "; generated from payload.manifest.json SHA256 $manifestHash")
$directoryDeleteLines = @("; generated from payload.manifest.json SHA256 $manifestHash") + $directoryDeleteLines
Write-DeterministicInclude $uninstallFileInclude $deleteFiles.ToArray()
Write-DeterministicInclude $uninstallDirInclude $directoryDeleteLines
Write-DeterministicInclude $installGuardInclude $installGuardLines.ToArray()

$stageItem = Get-Item -LiteralPath $StagePath -Force
if (($stageItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
    throw 'Suite stage root cannot be a reparse point.'
}
$OutputPath = [IO.Path]::GetFullPath($OutputPath)
$arguments = @(
    '/INPUTCHARSET', 'UTF8',
    "/DSTAGE=$StagePath",
    "/DOUTFILE=$OutputPath",
    "/DICON=$(Join-Path $StagePath 'ely-mark.ico')",
    "/DUNINSTALL_FILES=$uninstallFileInclude",
    "/DUNINSTALL_DIRS=$uninstallDirInclude",
    "/DINSTALL_GUARDS=$installGuardInclude",
    $nsi
)
& $MakeNSIS @arguments
if ($LASTEXITCODE -ne 0) {
    throw "makensis failed with exit code $LASTEXITCODE"
}
if (-not (Test-Path -LiteralPath $OutputPath -PathType Leaf)) {
    throw "makensis reported success but produced no installer: $OutputPath"
}
Get-Item -LiteralPath $OutputPath | Select-Object FullName, Length, LastWriteTimeUtc
Get-FileHash -LiteralPath $OutputPath -Algorithm SHA256
} finally {
    if ($tempConfigured) { $env:TEMP = $oldTemp; $env:TMP = $oldTmp }
}
