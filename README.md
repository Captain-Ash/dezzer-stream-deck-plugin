# Deezer

[Français](README.fr.md)

A Stream Deck plugin for Deezer Desktop.

End users install **a single Stream Deck plugin**. No Node.js, no terminal, no service to
start: the plugin ships and supervises a small invisible local binary.

> **Windows only.** Deezer control relies on Windows media sessions and Core Audio.

## Features

| Action | Effect |
|---|---|
| **Play / Pause** | Toggles playback. Album artwork is shown as the key background |
| **Next / Previous track** | Skips through the queue |
| **Volume + / -** | Adjusts Deezer's level in the Windows mixer |
| **Now playing** | Artwork, title, artist and elapsed time; press to toggle playback |
| **Deezer status** | Shows the local service state; press to restart it |

Long titles and artist names **scroll automatically** on the keys.

The plugin is available in **English and French**, following the language selected in
Stream Deck.

> **Looking for an OBS overlay?** It is a separate product:
> [Deezer OBS Overlay](https://github.com/Captain-Ash/deezer-obs-overlay). Both run side by
> side without interfering with each other.

## Install

Download `com.dezzer.deezer.streamDeckPlugin` from the
[latest release](https://github.com/Captain-Ash/deezer-stream-deck-plugin/releases) and
open it. Stream Deck asks for confirmation, and that's it.

See the [user guide](docs/user-guide.md) for day-to-day usage.

### Verifying a release

Each release ships a `SHA256SUMS` file (checksum of `com.dezzer.deezer.streamDeckPlugin`)
and its detached GPG signature `SHA256SUMS.asc`. The public key is
[`signing-key.asc`](signing-key.asc) at the root of this repository.

```powershell
gpg --import signing-key.asc
gpg --verify SHA256SUMS.asc SHA256SUMS
Get-FileHash com.dezzer.deezer.streamDeckPlugin -Algorithm SHA256
```

A trustworthy result prints `Good signature from "Captain_Ash"` with fingerprint:

```text
11D8 8CD4 6C6E 2372 84E7  F983 5B49 522A 022D 7C02
```

## Known limitations

- **Shuffle and repeat are not available.** Deezer Desktop reports both capabilities as
  unsupported, and the corresponding Windows calls have no observable effect.
- **Volume is the Windows mixer level**, not Deezer's internal slider — Windows offers no
  way to drive an application's own volume control. Both levels multiply.
- Volume control stays idle until Deezer has produced sound at least once, because Windows
  only creates the audio session at that moment.

## Architecture

```text
Stream Deck ──┐
              │ HTTP + WebSocket on 127.0.0.1, ephemeral port, per-install token
              ▼
        dezzer-bridge (Rust binary, windowless)
              │
              ├──► Windows Global Media Transport Controls  (playback, metadata)
              └──► Windows Core Audio                       (per-app volume)
                          │
                          ▼
                    Deezer Desktop
```

| Folder | Role |
|---|---|
| `apps/bridge` | Local bridge (Rust): adapters, HTTP/WebSocket API |
| `apps/streamdeck-plugin` | Stream Deck plugin (TypeScript) and its Property Inspector |
| `packages/playback-contract` | Shared data contract, mirror of `apps/bridge/src/contract.rs` |
| `scripts` | Spike, icon generation, packaging, installation |

## Development

Requirements: Node.js ≥ 20, stable Rust with the MSVC linker, Windows 10/11, Deezer
Desktop, Stream Deck ≥ 6.5.

```powershell
npm install

npm run build          # icons + plugin + bridge (release) -> .sdPlugin
npm run plugin:install # copy into Stream Deck and restart it
npm run pack           # produce dist/com.dezzer.deezer.streamDeckPlugin

npm test               # TypeScript and Rust tests
npm run typecheck
```

Working without Deezer running:

```powershell
$env:DEEZER_BRIDGE_ADAPTER = "mock"
npm run bridge:dev
```

## Security

- Listens on `127.0.0.1` only, on a fixed unprivileged port.
- 256-bit per-install token required across the whole API, regenerable on demand.
- `Origin` and `Host` headers are verified: a remote page cannot drive the bridge.
- No Deezer credentials are ever requested or stored.
- Logs never contain the token.


## Documentation

- [User guide](docs/user-guide.md) — [version française](docs/user-guide.fr.md)
- [Contributing](CONTRIBUTING.md)

## Trademarks

Deezer is a trademark of Deezer S.A. This project is an independent, unofficial plugin and
is not affiliated with or endorsed by Deezer. The icons shipped in this repository must be
replaced before any submission to the Stream Deck Marketplace.

## Licence

[PolyForm Noncommercial 1.0.0](LICENSE) — free to use, modify and share for any
**noncommercial** purpose.

Commercial use is not granted by this licence. Selling Deezer, bundling it with a paid
product or service, or monetising a derivative work requires prior written permission from
Captain Ash.

---

The SHA256 checksum of each release asset is signed with GPG by **Captain_Ash**
(fingerprint `11D8 8CD4 6C6E 2372 84E7 F983 5B49 522A 022D 7C02`, see
[signing-key.asc](signing-key.asc) and "Verifying a release" above).
