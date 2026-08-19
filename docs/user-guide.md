# User guide — Dezzer

[Version française](user-guide.fr.md)

## Install

1. Open `com.dezzer.deezer.streamDeckPlugin` (or install the plugin from the Stream Deck
   Marketplace).
2. Confirm the installation in Stream Deck.

There is nothing else to install: no Node.js, no Python, no service to start.

The plugin follows the language selected in Stream Deck. English and French are available.

## First use

1. Open **Deezer Desktop** and play a track.
2. In Stream Deck, open the **Dezzer** category and drop the **Play / Pause** action onto a
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
| **OBS overlay** | Opens a preview of the overlay in your browser |

A title or artist that is too long **scrolls automatically** on the key, and the same way in
the OBS overlay.

### About volume

Windows offers no way to drive an application's internal volume slider. The Volume actions
therefore act on Deezer's level **in the Windows mixer** — the same one you reach by
right-clicking the speaker icon.

Consequences:

- The slider shown **inside** Deezer does not move.
- Both levels combine: 50 % in Deezer and 50 % in the mixer give 25 % of the sound.
- The keys stay inactive until Deezer has produced sound since it started, because Windows
  only creates the audio session at that moment.

The step (1, 2, 5 or 10 %) is set in the settings of any Dezzer action.

### Shuffle and repeat

These two controls are **not available**. Deezer Desktop reports them as unsupported, and
the matching Windows calls report success without changing anything. They are therefore not
exposed rather than offered as keys that would do nothing.

## Overlay in OBS

1. Select any Dezzer action in Stream Deck to open its settings.
2. **OBS overlay** section: pick the theme, the width and the elements to display.
3. Click **Copy URL**.
4. In OBS: **Sources → + → Browser**.
5. Paste the URL, set the width to **720** and the height to **160**.
6. Confirm. The widget appears on a transparent background.

> **The URL contains an access token.** Do not show it on stream and do not share it.
> The Property Inspector masks it by default; the copy button works without revealing it.
> If you exposed it by accident, the **Regenerate token** button invalidates the old URL —
> you will then need to paste the new one into OBS.

The URL uses a **fixed port**: it stays valid after restarting Windows, Stream Deck or the
service.

### Overlay settings

| Setting | Values | Default |
|---|---|---|
| Theme | `minimal`, `glass`, `neon`, `broadcast` | `glass` |
| Width | 400 to 1200 px | 720 |
| Accent colour | hexadecimal colour, e.g. `#ff0066` | the theme's own |
| Artwork / Album / Time | shown or not | artwork and time shown |
| Waveform | replaces the progress bar with a live audio spectrum | off |
| Auto-hide | hides the widget after a delay | off |

An invalid value is ignored and replaced by the default.

### About the waveform

When enabled, the progress bar becomes a **spectrum analyser**: one bar per frequency band,
from bass on the left to treble on the right, reacting live to the music. Bars already
played are drawn in the accent colour, so the bar still shows your position in the track.

The bridge captures **Deezer's audio stream only**, using the per-application loopback
capture that Windows has offered since version 2004. Discord, notifications, your
microphone and the rest of the system never appear in the spectrum. No virtual device and
no driver are installed.

- The spectrum stays flat if Deezer has not produced sound since it started, because
  Windows only creates the audio session at that moment.
- Nothing is captured or analysed while no overlay displays the spectrum.
- Audio is never recorded nor written to disk: each block is analysed then discarded.

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
- Audio analysed for the spectrum is never recorded nor transmitted: each block is
  processed then discarded, and nothing is captured while the spectrum is not displayed.
- Technical logs are kept locally in `%LOCALAPPDATA%\Dezzer\logs`, rotated over 5 days.

## Uninstall

Uninstall the plugin from Stream Deck. The local service stops with it. The
`%LOCALAPPDATA%\Dezzer` folder can be deleted manually: it only holds logs and a state file.
