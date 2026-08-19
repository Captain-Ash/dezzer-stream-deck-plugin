$candidats = 39217, 39218, 39219, 39220, 39221
$occupes = (Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue).LocalPort | Sort-Object -Unique

Write-Output "--- disponibilite des ports candidats ---"
foreach ($p in $candidats) {
    $etat = if ($occupes -contains $p) { "OCCUPE" } else { "libre" }
    Write-Output ("{0} : {1}" -f $p, $etat)
}

Write-Output ""
Write-Output "--- plage ephemere Windows (a eviter) ---"
netsh int ipv4 show dynamicport tcp

Write-Output "--- plages exclues ---"
netsh int ipv4 show excludedportrange protocol=tcp
