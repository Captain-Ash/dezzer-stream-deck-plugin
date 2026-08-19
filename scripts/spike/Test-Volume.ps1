param([int]$Port = 39230)

$token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
$headers = @{ Authorization = "Bearer $token" }
$base = "http://127.0.0.1:$Port"

function Get-State { (Invoke-RestMethod "$base/v1/state" -Headers $headers).state }

$avant = Get-State
Write-Output "--- etat initial ---"
Write-Output ("piste       : {0} - {1}" -f $avant.artist, $avant.title)
Write-Output ("volume      : {0}" -f $(if ($null -ne $avant.volume) { "$($avant.volume) %" } else { "<absent>" }))
Write-Output ("capacite    : volume={0} seek={1} previous={2}" -f $avant.capabilities.volume, $avant.capabilities.seek, $avant.capabilities.previous)

if (-not $avant.capabilities.volume) {
    Write-Output ""
    Write-Output "Le volume n'est pas annonce comme disponible : aucune session audio Deezer."
    exit 0
}

$origine = $avant.volume
$cible = if ($origine -gt 50) { 35 } else { 75 }

Write-Output ""
Write-Output "--- ecriture : $origine % -> $cible % ---"
$reponse = Invoke-RestMethod "$base/v1/controls/volume" -Method Post -Headers $headers `
    -ContentType 'application/json' -Body (@{ value = $cible } | ConvertTo-Json)
Write-Output ("relu apres commande : {0} %" -f $reponse.state.volume)

Start-Sleep -Seconds 1
Write-Output ("relu apres 1 s      : {0} %" -f (Get-State).volume)

Write-Output ""
Write-Output "--- restauration a $origine % ---"
Invoke-RestMethod "$base/v1/controls/volume" -Method Post -Headers $headers `
    -ContentType 'application/json' -Body (@{ value = $origine } | ConvertTo-Json) | Out-Null
Write-Output ("volume final : {0} %" -f (Get-State).volume)
