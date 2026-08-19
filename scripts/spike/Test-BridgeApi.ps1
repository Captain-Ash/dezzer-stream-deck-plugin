param([int]$Port = 51715)

$token = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
$headers = @{ Authorization = "Bearer $token" }
$base = "http://127.0.0.1:$Port"

function Show([string]$label, [scriptblock]$block) {
    Write-Output "--- $label ---"
    try { & $block } catch {
        $code = $null
        if ($_.Exception.Response) { $code = [int]$_.Exception.Response.StatusCode }
        Write-Output "HTTP $code : $($_.Exception.Message)"
    }
    Write-Output ""
}

Show "/health" { (Invoke-RestMethod "$base/health" -Headers $headers) | ConvertTo-Json -Compress }
Show "/v1/state" { (Invoke-RestMethod "$base/v1/state" -Headers $headers).state | ConvertTo-Json -Depth 5 }
Show "/v1/capabilities" { (Invoke-RestMethod "$base/v1/capabilities" -Headers $headers) | ConvertTo-Json -Compress }
Show "sans token" { Invoke-RestMethod "$base/v1/state" | Out-Null }
Show "mauvais token" { Invoke-RestMethod "$base/v1/state" -Headers @{ Authorization = "Bearer mauvais" } | Out-Null }
Show "origine hostile" { Invoke-RestMethod "$base/v1/state" -Headers @{ Authorization = "Bearer $token"; Origin = "https://evil.example" } | Out-Null }
Show "host non loopback" { Invoke-RestMethod "$base/v1/state" -Headers @{ Authorization = "Bearer $token"; Host = "attacker.example" } | Out-Null }
Show "volume hors bornes" { Invoke-RestMethod "$base/v1/controls/volume" -Method Post -Headers $headers -ContentType 'application/json' -Body '{"value":900}' | Out-Null }
Show "artwork" {
    $state = (Invoke-RestMethod "$base/v1/state" -Headers $headers).state
    if ($state.artworkUrl) {
        $r = Invoke-WebRequest "$base$($state.artworkUrl)" -Headers $headers -UseBasicParsing
        Write-Output "$($state.artworkUrl) -> $($r.Headers['Content-Type']) $($r.RawContentLength) octets"
    } else { Write-Output "pas de pochette exposee" }
}
Show "position rafraichie" {
    $a = (Invoke-RestMethod "$base/v1/state" -Headers $headers).state
    Start-Sleep -Seconds 2
    $b = (Invoke-RestMethod "$base/v1/state" -Headers $headers).state
    Write-Output "$($a.positionMs) ms -> $($b.positionMs) ms (delta $($b.positionMs - $a.positionMs) ms, sequence $($a.sequence) -> $($b.sequence))"
}
