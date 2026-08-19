# Contributing

Thanks for taking the time to look at Dezzer.

## Reporting a bug

Open an issue with:

- your Windows, Deezer Desktop and Stream Deck versions;
- the **Exportable diagnostic** block from the Property Inspector (it contains no token);
- what you expected, and what happened instead.

Logs live in `%LOCALAPPDATA%\Dezzer\logs`. They never contain the access token, but do
check before pasting anything.

## Development setup

Requirements: Node.js ≥ 20, stable Rust with the MSVC linker, Windows 10/11.

```powershell
npm install
npm test
npm run build
npm run plugin:install
```

`npm run plugin:install` copies the built package into Stream Deck and restarts it.

To work without Deezer running, use the mock adapter:

```powershell
$env:DEZZER_BRIDGE_ADAPTER = "mock"
npm run bridge:dev
```

## Before opening a pull request

- `npm run typecheck` and `npm test` must pass.
- Keep the Rust contract (`apps/bridge/src/contract.rs`) and the TypeScript contract
  (`packages/playback-contract/src/index.ts`) in sync. Any incompatible change bumps
  `SCHEMA_VERSION` on both sides.
- User-facing strings go in `apps/streamdeck-plugin/com.dezzer.deezer.sdPlugin/en.json`
  **and** `fr.json`. A test enforces that both files carry the same keys.
- Comments and commit messages may be in English or French; the codebase currently mixes
  both, with French for internal rationale.

## Scope

Dezzer only drives Deezer through public Windows APIs: media sessions and Core Audio. No
process injection, no UI scraping, no synthetic keystrokes. Contributions that rely on
those techniques will not be merged.
