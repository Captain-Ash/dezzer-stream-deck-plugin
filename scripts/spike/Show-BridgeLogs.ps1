$root = "$env:LOCALAPPDATA\Deezer"

Write-Output "--- fichier de disponibilite ---"
Get-Content (Join-Path $root "bridge-runtime.json") -Raw

Write-Output "--- fichiers de log ---"
Get-ChildItem (Join-Path $root "logs") -ErrorAction SilentlyContinue |
    Select-Object Name, Length | Format-Table -AutoSize | Out-String

$log = Get-ChildItem (Join-Path $root "logs\*.log") -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1

if ($log) {
    Write-Output "--- dernieres lignes ($($log.Name)) ---"
    Get-Content $log.FullName | Select-Object -Last 12

    Write-Output "--- fuite de token dans les logs ? ---"
    if (Select-String -Path $log.FullName -Pattern "token" -SimpleMatch -Quiet) {
        Write-Output "OUI -> A CORRIGER"
    } else {
        Write-Output "non"
    }
} else {
    Write-Output "aucun fichier de log"
}
