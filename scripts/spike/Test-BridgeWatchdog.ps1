<#
.SYNOPSIS
    Verifie que le bridge ne survit jamais au processus qui l'a lance (§8.2).
#>

$bridge = Get-Process dezzer-bridge -ErrorAction SilentlyContinue
if (-not $bridge) {
    Write-Output "aucun bridge en cours : lancez Stream Deck avec une action Deezer visible"
    exit 1
}

$parentId = (Get-CimInstance Win32_Process -Filter "ProcessId = $($bridge.Id)").ParentProcessId
$parent = Get-Process -Id $parentId -ErrorAction SilentlyContinue
Write-Output "bridge pid=$($bridge.Id) parent=$parentId ($($parent.ProcessName))"

Write-Output "arret du processus parent…"
Stop-Process -Id $parentId -Force
Start-Sleep -Seconds 8

if (Get-Process -Id $bridge.Id -ErrorAction SilentlyContinue) {
    Write-Output "ECHEC : le bridge survit a son parent"
} else {
    Write-Output "OK : le bridge s'est arrete avec son parent"
}

$root = "$env:LOCALAPPDATA\Deezer"
Write-Output ("fichier de disponibilite : " + $(if (Test-Path "$root\bridge-runtime.json") { "present (non nettoye)" } else { "supprime" }))
Write-Output ("verrou de processus      : " + $(if (Test-Path "$root\bridge.lock") { "present (non nettoye)" } else { "supprime" }))
