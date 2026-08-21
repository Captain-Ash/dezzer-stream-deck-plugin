# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2026-08-21

### Removed

- **The OBS overlay and everything that served it** — widget page, themes, spectrum
  analyser, per-application audio capture, the `/overlay` and `/v1/levels` endpoints, and
  the matching Stream Deck action and Property Inspector settings. The overlay is now a
  standalone product:
  [Deezer OBS Overlay](https://github.com/Captain-Ash/deezer-obs-overlay). This plugin is
  focused solely on controlling Deezer from the keys.

### Changed

- Corrected the plugin's own spelling everywhere it is displayed publicly: the plugin name,
  its Stream Deck category, the documentation and the runtime data folder are now spelled
  **Deezer** (previously misspelled "Dezzer"). This also affects the `%LOCALAPPDATA%` folder
  used by the local bridge and the `DEEZER_BRIDGE_*` environment variables used for
  development.
- The local bridge now listens on an **ephemeral port** chosen by the system instead of a
  fixed one. The fixed port only existed so that the URL pasted into OBS stayed valid; the
  plugin discovers the real port through the runtime file.

## [0.1.0] - 2026-08-19

First public release.

### Added

- Stream Deck actions: **Play/Pause, Next, Previous, Volume +/-, Now playing, Deezer
  status, OBS overlay**. Album artwork on the keys, automatic scrolling for long titles.
- **OBS overlay** with four themes, an accent colour and an optional live **spectrum
  analyser** on the progress bar. The bridge captures Deezer's audio stream alone, through
  per-application loopback capture, and runs a Fourier transform on it. Enable it with
  `waveform=1` or the checkbox in the Property Inspector.
- Local bridge on a fixed port (`127.0.0.1:39217`), protected by a 256-bit per-install
  token, started and supervised by the plugin.
- **English and French localisation**, following the language selected in Stream Deck.
- `npm run pack` produces the distributable `.streamDeckPlugin` file.
- Continuous integration and an automated release workflow.

### Known limitations

- Shuffle and repeat are not exposed: Deezer Desktop reports them as unsupported and the
  matching Windows calls have no observable effect.
- Volume and the spectrum analyser stay idle until Deezer has produced sound at least once,
  because Windows only creates the audio session at that moment.

[0.1.1]: https://github.com/Captain-Ash/deezer-stream-deck-plugin/releases/tag/v0.1.1
[0.1.0]: https://github.com/Captain-Ash/deezer-stream-deck-plugin/releases/tag/v0.1.0
