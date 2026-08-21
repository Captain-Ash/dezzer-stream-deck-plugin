# User guide — Deezer

[Version française](user-guide.fr.md)

## Install

1. Open `com.dezzer.deezer.streamDeckPlugin` (or install the plugin from the Stream Deck
   Marketplace).
2. Confirm the installation in Stream Deck.

There is nothing else to install: no Node.js, no Python, no service to start.

The plugin follows the language selected in Stream Deck. English and French are available.

## First use

1. Open **Deezer Desktop** and play a track.
2. In Stream Deck, open the **Deezer** category and drop the **Play / Pause** action onto a
   key.
3. The key reflects the real playback state within a second.

If the key reads `Deezer OFF`, Deezer Desktop is not running, or no track has been played
since it started.

## Available actions

| Action | Effect |
|---|---|
| **Play / Pause** | Toggles playback. Album artwork is shown as the key background |
| **Next track** | Skips to the next title |
| **Previous track** | Goes back to the previous title |
| **Volume +** / **Volume -** | Adjusts Deezer's volume |
| **Now playing** | Artwork, title, artist and elapsed time; press to toggle playback |
| **Deezer status** | Shows the local service state; press to restart it |

A title or artist that is too long **scrolls automatically** on the key.

> **Looking for a "Now Playing" widget for OBS?** This plugin no longer handles it. Install
> the dedicated product:
> [Deezer OBS Overlay](https://github.com/Captain-Ash/deezer-obs-overlay). It is configured
> entirely from OBS and can run alongside this plugin.

### About volume

Windows offers no way to drive an application's internal volume slider. The Volume actions
therefore act on Deezer's level **in the Windows mixer** — the same one you reach by
right-clicking the speaker icon.

Consequences:

- The slider shown **inside** Deezer does not move.
- Both levels combine: 50 % in Deezer and 50 % in the mixer give 25 % of the sound.
- The keys stay inactive until Deezer has produced sound since it started, because Windows
  only creates the audio session at that moment.

The step (1, 2, 5 or 10 %) is set in the settings of any Deezer action.

### Shuffle and repeat

These two controls are **not available**. Deezer Desktop reports them as unsupported, and
the matching Windows calls report success without changing anything. They are therefore not
exposed rather than offered as keys that would do nothing.

## If something does not work

| Message on the key | What to do |
|---|---|
| `Starting…` | Wait a few seconds, the local service is starting |
| `Deezer OFF` | Open Deezer Desktop and play a track |
| `Bridge OFF` | Press the **Deezer status** key to restart the service |
| `Bridge error` | Open the settings, Status section, then **Restart service** |
| `Not supported` | This version of Deezer or Windows does not expose that control |

The Property Inspector permanently shows the state of the local service, of Deezer, and the
list of capabilities actually available. The **Exportable diagnostic** block can be pasted
as-is into a bug report: it contains no token.

## Privacy

- Everything runs locally. The service opens no connection to the internet.
- No Deezer credentials are requested.
- Playback metadata never leaves your machine.
- No audio stream is captured or analysed.
- Technical logs are kept locally in `%LOCALAPPDATA%\Deezer\logs`, rotated over 5 days.

## Uninstall

Uninstall the plugin from Stream Deck. The local service stops with it. The
`%LOCALAPPDATA%\Deezer` folder can be deleted manually: it only holds logs and a state file.
