<#
.SYNOPSIS
    Spike M0 - Enumere les sessions media Windows (GlobalSystemMediaTransportControls)
    et rapporte les capacites reellement exposees, notamment par Deezer Desktop.

.DESCRIPTION
    Repond a la seule question du spike : Deezer Desktop expose-t-il une session media
    utilisable, et quelles capacites fournit-elle ?
    Lecture seule par defaut. Les commandes ne sont testees que via -TestCommands.

.PARAMETER TestCommands
    Execute reellement play/pause, next, previous (et seek/volume si -Aggressive)
    sur la session Deezer detectee, en restaurant l'etat quand c'est possible.

.PARAMETER JsonPath
    Chemin de sortie du rapport JSON anonymise.

.PARAMETER WatchSeconds
    Observe les changements d'etat pendant N secondes (polling 500 ms).
#>
[CmdletBinding()]
param(
    [switch]$TestCommands,
    [switch]$Aggressive,
    [string]$JsonPath,
    [string]$ArtworkPath,
    [int]$WatchSeconds = 0
)

$ErrorActionPreference = 'Stop'

# --- Pont WinRT pour Windows PowerShell 5.1 -----------------------------------
Add-Type -AssemblyName System.Runtime.WindowsRuntime | Out-Null

$asTaskGeneric = ([System.WindowsRuntimeSystemExtensions].GetMethods() |
    Where-Object {
        $_.Name -eq 'AsTask' -and
        $_.GetParameters().Count -eq 1 -and
        $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
    })[0]

function Await {
    param($WinRtTask, [Type]$ResultType)
    $asTask = $asTaskGeneric.MakeGenericMethod($ResultType)
    $netTask = $asTask.Invoke($null, @($WinRtTask))
    if (-not $netTask.Wait(10000)) { throw "Timeout WinRT (AsTask)" }
    $netTask.Result
}

$asTaskAction = ([System.WindowsRuntimeSystemExtensions].GetMethods() |
    Where-Object {
        $_.Name -eq 'AsTask' -and
        $_.GetParameters().Count -eq 1 -and
        $_.GetParameters()[0].ParameterType.FullName -eq 'Windows.Foundation.IAsyncAction'
    })[0]

function AwaitAction {
    param($WinRtAction)
    $netTask = $asTaskAction.Invoke($null, @($WinRtAction))
    if (-not $netTask.Wait(10000)) { throw "Timeout WinRT (AsTask action)" }
}

# Chargement des types WinRT
$null = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager, Windows.Media.Control, ContentType = WindowsRuntime]
$null = [Windows.Media.Control.GlobalSystemMediaTransportControlsSession, Windows.Media.Control, ContentType = WindowsRuntime]
$null = [Windows.Storage.Streams.IRandomAccessStreamReference, Windows.Storage.Streams, ContentType = WindowsRuntime]

$MgrType = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager]
$SessionType = [Windows.Media.Control.GlobalSystemMediaTransportControlsSession]
$MediaPropsType = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionMediaProperties]

# --- Helpers ------------------------------------------------------------------

function Get-SessionManager {
    Await ($MgrType::RequestAsync()) $MgrType
}

function ConvertTo-TimeSpanMs {
    param($TimeSpan)
    if ($null -eq $TimeSpan) { return $null }
    $ms = [double]$TimeSpan.TotalMilliseconds
    if ([double]::IsNaN($ms)) { return $null }
    return [long][math]::Round($ms)
}

function Get-SessionSnapshot {
    param($Session, [switch]$IncludeMetadata)

    $sourceAppId = $Session.SourceAppUserModelId
    $info = $Session.GetPlaybackInfo()
    $timeline = $Session.GetTimelineProperties()

    $caps = $null
    if ($info -and $info.Controls) {
        $c = $info.Controls
        $caps = [ordered]@{
            play             = [bool]$c.IsPlayEnabled
            pause            = [bool]$c.IsPauseEnabled
            playPauseToggle  = [bool]$c.IsPlayPauseToggleEnabled
            stop             = [bool]$c.IsStopEnabled
            next             = [bool]$c.IsNextEnabled
            previous         = [bool]$c.IsPreviousEnabled
            fastForward      = [bool]$c.IsFastForwardEnabled
            rewind           = [bool]$c.IsRewindEnabled
            playbackPosition = [bool]$c.IsPlaybackPositionEnabled
            playbackRate     = [bool]$c.IsPlaybackRateEnabled
            shuffle          = [bool]$c.IsShuffleEnabled
            repeat           = [bool]$c.IsRepeatEnabled
            channelUp        = [bool]$c.IsChannelUpEnabled
            channelDown      = [bool]$c.IsChannelDownEnabled
        }
    }

    $status = 'unknown'
    if ($info) { $status = [string]$info.PlaybackStatus }

    $snapshot = [ordered]@{
        sourceAppUserModelId = $sourceAppId
        playbackStatus       = $status
        playbackType         = if ($info -and $info.PlaybackType) { [string]$info.PlaybackType.Value } else { $null }
        autoRepeatMode       = if ($info -and $info.AutoRepeatMode) { [string]$info.AutoRepeatMode.Value } else { $null }
        isShuffleActive      = if ($info -and $null -ne $info.IsShuffleActive) { $info.IsShuffleActive.Value } else { $null }
        controls             = $caps
        timeline             = $null
        metadata             = $null
    }

    if ($timeline) {
        $snapshot.timeline = [ordered]@{
            startTimeMs      = ConvertTo-TimeSpanMs $timeline.StartTime
            endTimeMs        = ConvertTo-TimeSpanMs $timeline.EndTime
            positionMs       = ConvertTo-TimeSpanMs $timeline.Position
            minSeekMs        = ConvertTo-TimeSpanMs $timeline.MinSeekTime
            maxSeekMs        = ConvertTo-TimeSpanMs $timeline.MaxSeekTime
            lastUpdatedAtUtc = if ($timeline.LastUpdatedTime) { $timeline.LastUpdatedTime.UtcDateTime.ToString('o') } else { $null }
        }
    }

    if ($IncludeMetadata) {
        try {
            $props = Await ($Session.TryGetMediaPropertiesAsync()) $MediaPropsType
            $genres = @()
            if ($props.Genres) { $genres = @($props.Genres) }
            $snapshot.metadata = [ordered]@{
                title            = $props.Title
                artist           = $props.Artist
                albumTitle       = $props.AlbumTitle
                albumArtist      = $props.AlbumArtist
                trackNumber      = $props.TrackNumber
                albumTrackCount  = $props.AlbumTrackCount
                subtitle         = $props.Subtitle
                genres           = $genres
                playbackType     = if ($props.PlaybackType) { [string]$props.PlaybackType.Value } else { $null }
                hasThumbnail     = ($null -ne $props.Thumbnail)
            }
        }
        catch {
            $snapshot.metadata = [ordered]@{ error = $_.Exception.Message }
        }
    }

    return $snapshot
}

function Test-IsDeezerSession {
    param([string]$SourceAppUserModelId)
    if ([string]::IsNullOrWhiteSpace($SourceAppUserModelId)) { return $false }
    return $SourceAppUserModelId -match 'deezer'
}

function Get-ThumbnailInfo {
    param($Session, [string]$SavePath)
    try {
        $props = Await ($Session.TryGetMediaPropertiesAsync()) $MediaPropsType
        if ($null -eq $props.Thumbnail) { return [ordered]@{ available = $false } }

        $streamRefType = [Windows.Storage.Streams.IRandomAccessStreamWithContentType, Windows.Storage.Streams, ContentType = WindowsRuntime]
        $stream = Await ($props.Thumbnail.OpenReadAsync()) $streamRefType

        $size = [long]$stream.Size
        $info = [ordered]@{
            available   = $true
            contentType = $stream.ContentType
            sizeBytes   = $size
        }

        if ($size -gt 0) {
            $null = [Windows.Storage.Streams.DataReader, Windows.Storage.Streams, ContentType = WindowsRuntime]
            $reader = [Windows.Storage.Streams.DataReader]::new($stream.GetInputStreamAt(0))
            $loaded = Await ($reader.LoadAsync([uint32]$size)) ([uint32])
            $bytes = New-Object byte[] $loaded
            $reader.ReadBytes($bytes)
            $reader.Dispose()

            $info.readBytes = $loaded
            $info.magic = (($bytes[0..7] | ForEach-Object { $_.ToString('X2') }) -join ' ')
            $info.base64Length = [math]::Ceiling($loaded / 3) * 4

            if ($SavePath) {
                $dir = Split-Path -Parent $SavePath
                if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
                [System.IO.File]::WriteAllBytes($SavePath, $bytes)
                $info.savedTo = $SavePath
            }
        }

        $stream = $null
        return $info
    }
    catch {
        return [ordered]@{ available = $false; error = $_.Exception.Message }
    }
}

# --- Collecte -----------------------------------------------------------------

Write-Host "=== Spike M0 : sessions media Windows ===" -ForegroundColor Cyan
Write-Host ("OS      : {0}" -f [System.Environment]::OSVersion.VersionString)
Write-Host ("Build   : {0}" -f (Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion').DisplayVersion)
Write-Host ("Deezer  : {0}" -f (@(Get-Process -Name '*deezer*' -ErrorAction SilentlyContinue).Count.ToString() + " processus"))
Write-Host ""

$mgr = Get-SessionManager
$sessions = @($mgr.GetSessions())
$current = $mgr.GetCurrentSession()

Write-Host ("Sessions detectees : {0}" -f $sessions.Count) -ForegroundColor Yellow
if ($current) { Write-Host ("Session courante   : {0}" -f $current.SourceAppUserModelId) }
Write-Host ""

$report = [ordered]@{
    generatedAtUtc  = (Get-Date).ToUniversalTime().ToString('o')
    osVersion       = [System.Environment]::OSVersion.VersionString
    sessionCount    = $sessions.Count
    currentSourceId = if ($current) { $current.SourceAppUserModelId } else { $null }
    sessions        = @()
    deezer          = $null
    commandTests    = @()
    watch           = @()
}

foreach ($s in $sessions) {
    $snap = Get-SessionSnapshot -Session $s -IncludeMetadata
    $snap.isDeezer = Test-IsDeezerSession $snap.sourceAppUserModelId
    $report.sessions += $snap

    $marker = if ($snap.isDeezer) { '>>' } else { '  ' }
    $color = if ($snap.isDeezer) { 'Green' } else { 'Gray' }
    Write-Host ("{0} {1}  [{2}]" -f $marker, $snap.sourceAppUserModelId, $snap.playbackStatus) -ForegroundColor $color
    if ($snap.metadata -and $snap.metadata.title) {
        Write-Host ("     titre : {0} - {1} ({2})" -f $snap.metadata.artist, $snap.metadata.title, $snap.metadata.albumTitle)
    }
    if ($snap.timeline) {
        Write-Host ("     temps : {0} / {1} ms" -f $snap.timeline.positionMs, $snap.timeline.endTimeMs)
    }
    if ($snap.controls) {
        $enabled = ($snap.controls.GetEnumerator() | Where-Object { $_.Value } | ForEach-Object { $_.Key }) -join ', '
        Write-Host ("     caps  : {0}" -f $enabled)
    }
    Write-Host ""
}

# --- Selection Deezer ---------------------------------------------------------

$deezerSessions = @($sessions | Where-Object { Test-IsDeezerSession $_.SourceAppUserModelId })
$deezer = $null
if ($deezerSessions.Count -gt 0) {
    $playing = @($deezerSessions | Where-Object { [string]$_.GetPlaybackInfo().PlaybackStatus -eq 'Playing' })
    $deezer = if ($playing.Count -gt 0) { $playing[0] } else { $deezerSessions[0] }
    Write-Host ("Session Deezer retenue : {0}" -f $deezer.SourceAppUserModelId) -ForegroundColor Green
    $report.deezer = [ordered]@{
        found     = $true
        sourceId  = $deezer.SourceAppUserModelId
        snapshot  = Get-SessionSnapshot -Session $deezer -IncludeMetadata
        thumbnail = Get-ThumbnailInfo -Session $deezer -SavePath $ArtworkPath
    }
    Write-Host ("Artwork : {0}" -f ($report.deezer.thumbnail | ConvertTo-Json -Compress))
}
else {
    Write-Host "Aucune session Deezer detectee." -ForegroundColor Red
    $report.deezer = [ordered]@{ found = $false }
}

# --- Tests de commandes -------------------------------------------------------

if ($TestCommands -and $deezer) {
    Write-Host ""
    Write-Host "=== Tests de commandes (mode explicite) ===" -ForegroundColor Cyan

    function Invoke-CommandTest {
        param([string]$Name, [scriptblock]$Action)
        $before = Get-SessionSnapshot -Session $deezer -IncludeMetadata
        $result = [ordered]@{ command = $Name; before = $before.playbackStatus; beforeTitle = $before.metadata.title }
        try {
            $ok = & $Action
            Start-Sleep -Milliseconds 1200
            $after = Get-SessionSnapshot -Session $deezer -IncludeMetadata
            $result.returned = $ok
            $result.after = $after.playbackStatus
            $result.afterTitle = $after.metadata.title
            $result.changed = ($before.playbackStatus -ne $after.playbackStatus) -or ($before.metadata.title -ne $after.metadata.title)
            $result.error = $null
        }
        catch {
            $result.returned = $false
            $result.error = $_.Exception.Message
        }
        Write-Host ("{0,-12} -> returned={1} changed={2} {3}" -f $Name, $result.returned, $result.changed, $result.error)
        return $result
    }

    $report.commandTests += Invoke-CommandTest 'playPause' { Await ($deezer.TryTogglePlayPauseAsync()) ([bool]) }
    $report.commandTests += Invoke-CommandTest 'playPause2' { Await ($deezer.TryTogglePlayPauseAsync()) ([bool]) }
    $report.commandTests += Invoke-CommandTest 'next' { Await ($deezer.TrySkipNextAsync()) ([bool]) }
    $report.commandTests += Invoke-CommandTest 'previous' { Await ($deezer.TrySkipPreviousAsync()) ([bool]) }

    if ($Aggressive) {
        $tl = $deezer.GetTimelineProperties()
        $target = [long]((ConvertTo-TimeSpanMs $tl.Position) + 30000)
        $report.commandTests += Invoke-CommandTest 'seek+30s' {
            Await ($deezer.TryChangePlaybackPositionAsync($target * 10000)) ([bool])
        }
        $report.commandTests += Invoke-CommandTest 'shuffleOn' {
            Await ($deezer.TryChangeShuffleActiveAsync($true)) ([bool])
        }
    }
}

# --- Observation --------------------------------------------------------------

if ($WatchSeconds -gt 0 -and $deezer) {
    Write-Host ""
    Write-Host ("=== Observation pendant {0}s (Ctrl+C pour arreter) ===" -f $WatchSeconds) -ForegroundColor Cyan
    $end = (Get-Date).AddSeconds($WatchSeconds)
    $lastKey = ''
    while ((Get-Date) -lt $end) {
        $snap = Get-SessionSnapshot -Session $deezer -IncludeMetadata
        $key = "{0}|{1}|{2}" -f $snap.playbackStatus, $snap.metadata.title, $snap.metadata.artist
        if ($key -ne $lastKey) {
            $lastKey = $key
            $line = [ordered]@{
                atUtc      = (Get-Date).ToUniversalTime().ToString('o')
                status     = $snap.playbackStatus
                title      = $snap.metadata.title
                artist     = $snap.metadata.artist
                positionMs = if ($snap.timeline) { $snap.timeline.positionMs } else { $null }
            }
            $report.watch += $line
            Write-Host ("[{0}] {1} - {2} / {3}" -f $snap.playbackStatus, $snap.metadata.artist, $snap.metadata.title, $line.positionMs)
        }
        Start-Sleep -Milliseconds 500
    }
}

# --- Sortie -------------------------------------------------------------------

if ($JsonPath) {
    $dir = Split-Path -Parent $JsonPath
    if ($dir -and -not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $report | ConvertTo-Json -Depth 12 | Set-Content -Path $JsonPath -Encoding UTF8
    Write-Host ""
    Write-Host ("Rapport ecrit : {0}" -f $JsonPath) -ForegroundColor Green
}
