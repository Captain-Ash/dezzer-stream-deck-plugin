# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.0]: https://github.com/Captain-Ash/dezzer-stream-deck-plugin/releases/tag/v0.1.0
