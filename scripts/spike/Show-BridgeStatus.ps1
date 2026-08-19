$root = "$env:LOCALAPPDATA\Dezzer"

Write-Output "--- fichier de disponibilite ---"
Get-Content (Join-Path $root "bridge-runtime.json") -Raw -ErrorAction SilentlyContinue

Write-Output "--- dernieres lignes de log ---"
$log = Get-ChildItem (Join-Path $root "logs\*.log") -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
if ($log) { Get-Content $log.FullName | Select-Object -Last 5 }

Write-Output "--- sockets du bridge ---"
$bridge = Get-Process dezzer-bridge -ErrorAction SilentlyContinue
if ($bridge) {
    netstat -ano | Select-String ("\s{0}$" -f $bridge.Id) | ForEach-Object { $_.Line.Trim() }
} else {
    Write-Output "bridge absent"
}
