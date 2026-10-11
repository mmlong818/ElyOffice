param(
    [string]$Destination = (Join-Path $PSScriptRoot '../engines/tessdata')
)

$ErrorActionPreference = 'Stop'
$Revision = '87416418657359cb625c412a48b6e1d6d41c29bd'
$Files = [ordered]@{
    'LICENSE' = 'cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30'
    'eng.traineddata' = '7d4322bd2a7749724879683fc3912cb542f19906c83bcc1a52132556427170b2'
    'chi_sim.traineddata' = 'a5fcb6f0db1e1d6d8522f39db4e848f05984669172e584e8d76b6b3141e1f730'
    'chi_tra.traineddata' = '529c5b5797d64b126065cd55f2bb4c7fd7b15790798091b1ff259941a829330b'
}
$Destination = [IO.Path]::GetFullPath($Destination)
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
foreach ($item in $Files.GetEnumerator()) {
    $path = Join-Path $Destination $item.Key
    if ((Test-Path $path) -and (Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant() -eq $item.Value) {
        Write-Output "verified $($item.Key)"
        continue
    }
    $partial = "$path.part"
    $url = "https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/$Revision/$($item.Key)"
    Invoke-WebRequest -Uri $url -OutFile $partial -TimeoutSec 30
    $actual = (Get-FileHash $partial -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $item.Value) {
        Remove-Item -LiteralPath $partial -Force
        throw "$($item.Key) SHA-256 was $actual; expected $($item.Value)."
    }
    Move-Item -LiteralPath $partial -Destination $path -Force
    Write-Output "fetched $($item.Key)"
}
