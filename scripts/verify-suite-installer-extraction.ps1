[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $StagePath,
    [Parameter(Mandatory)] [string] $ExtractedPayloadPath,
    [string] $TempRoot
)

 $oldTemp = $env:TEMP
 $oldTmp = $env:TMP
 $tempConfigured = $false
try {
$ErrorActionPreference = 'Stop'
$StagePath = [IO.Path]::GetFullPath($StagePath)
$ExtractedPayloadPath = [IO.Path]::GetFullPath($ExtractedPayloadPath)
if (-not $TempRoot) { $TempRoot = "$ExtractedPayloadPath.verify-tmp" }
$TempRoot = [IO.Path]::GetFullPath($TempRoot)
if ([IO.Path]::GetPathRoot($TempRoot) -eq [IO.Path]::GetPathRoot($env:SystemRoot)) { throw "TempRoot must not use the system drive: $TempRoot" }
if ($TempRoot.Equals($StagePath, [StringComparison]::OrdinalIgnoreCase) -or
    $TempRoot.StartsWith($StagePath.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
    $TempRoot.Equals($ExtractedPayloadPath, [StringComparison]::OrdinalIgnoreCase) -or
    $TempRoot.StartsWith($ExtractedPayloadPath.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
    $StagePath.StartsWith($TempRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) -or
    $ExtractedPayloadPath.StartsWith($TempRoot.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'TempRoot cannot overlap either payload tree.' }
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
$StagePath = [IO.Path]::GetFullPath($StagePath).TrimEnd('\')
$ExtractedPayloadPath = $ExtractedPayloadPath.TrimEnd('\')
if (-not (Test-Path -LiteralPath $StagePath -PathType Container)) { throw "Stage directory is missing: $StagePath" }
if (-not (Test-Path -LiteralPath $ExtractedPayloadPath -PathType Container)) { throw "7-Zip extraction directory is missing: $ExtractedPayloadPath" }
foreach ($root in @($StagePath, $ExtractedPayloadPath)) {
    $rootItem = Get-Item -LiteralPath $root -Force
    if (($rootItem.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw "Verification root is a reparse point: $root" }
    $reparse = Get-ChildItem -LiteralPath $root -Force -Recurse -ErrorAction Stop |
        Where-Object { ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0 } |
        Select-Object -First 1
    if ($reparse) { throw "Verification tree contains a reparse point: $($reparse.FullName)" }
}
$manifestPath = Join-Path $StagePath 'payload.manifest.json'
if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) { throw "Suite manifest is missing: $manifestPath" }
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.status -cne 'READY_FOR_INSTALLER_REVIEW_NOT_INSTALL_TESTED') {
    throw "Refusing to verify a stage with status '$($manifest.status)'."
}

$expected = [Collections.Generic.Dictionary[string, object]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($entry in $manifest.files) {
    $relative = [string] $entry.path
    $segments = $relative -split '\\'
    if ([string]::IsNullOrWhiteSpace($relative) -or $relative.Contains('/') -or
        $relative -match '[\r\n\x00:*?"<>|]' -or [IO.Path]::IsPathRooted($relative) -or
        ($segments -contains '..') -or ($segments -contains '.') -or ($segments -contains '') -or
        ($segments | Where-Object { $_.EndsWith('.') -or $_.EndsWith(' ') })) {
        throw "Unsafe path in manifest: '$relative'"
    }
    if ($expected.ContainsKey($relative)) { throw "Duplicate or case-colliding manifest path: '$relative'" }
    $expected.Add($relative, $entry)
}
$manifestFile = Get-Item -LiteralPath $manifestPath -Force
$expected.Add('payload.manifest.json', [pscustomobject]@{
    path = 'payload.manifest.json'
    length = [long] $manifestFile.Length
    sha256 = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash
})

$actual = [Collections.Generic.Dictionary[string, IO.FileInfo]]::new([StringComparer]::OrdinalIgnoreCase)
$ignoredInstallerMetadata = [Collections.Generic.List[string]]::new()
foreach ($file in Get-ChildItem -LiteralPath $ExtractedPayloadPath -File -Recurse -Force) {
    $relative = $file.FullName.Substring($ExtractedPayloadPath.Length).TrimStart('\')
    # 7-Zip exposes NSIS plugin resources separately. They are installer UI metadata,
    # not files in the suite payload. The uninstaller stub is generated at install time.
    if (-not $expected.ContainsKey($relative) -and
        ($relative -match '^\$PLUGINSDIR\\' -or $relative -ieq 'uninstall.exe')) {
        $ignoredInstallerMetadata.Add($relative)
        continue
    }
    if ($actual.ContainsKey($relative)) { throw "Duplicate or case-colliding extracted path: '$relative'" }
    $actual.Add($relative, $file)
}

$missing = [Collections.Generic.List[string]]::new()
$mismatched = [Collections.Generic.List[string]]::new()
foreach ($path in $expected.Keys) {
    if (-not $actual.ContainsKey($path)) { $missing.Add($path); continue }
    $file = $actual[$path]
    $entry = $expected[$path]
    if ($file.Length -ne [long] $entry.length -or
        (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash -cne [string] $entry.sha256) {
        $mismatched.Add($path)
    }
}
$extra = [Collections.Generic.List[string]]::new()
foreach ($path in $actual.Keys) { if (-not $expected.ContainsKey($path)) { $extra.Add($path) } }
if ($missing.Count -or $extra.Count -or $mismatched.Count) {
    $details = @(
        "Missing payload files ($($missing.Count)): $($missing -join ', ')"
        "Unexpected extracted files ($($extra.Count)): $($extra -join ', ')"
        "Length/hash mismatches ($($mismatched.Count)): $($mismatched -join ', ')"
    )
    throw ($details -join [Environment]::NewLine)
}

$stageManifestHash = (Get-FileHash -LiteralPath $manifestPath -Algorithm SHA256).Hash
Write-Output "Verified suite payload files: $($expected.Count)"
Write-Output "Stage manifest SHA-256: $stageManifestHash"
Write-Output "Ignored NSIS metadata files: $($ignoredInstallerMetadata.Count)"
} finally {
    if ($tempConfigured) { $env:TEMP = $oldTemp; $env:TMP = $oldTmp }
}
