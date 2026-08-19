# Dezzer

[English](README.md)

Plugin Stream Deck pour Deezer Desktop, avec overlay « Now Playing » pour OBS.

L'utilisateur final installe **un seul plugin Stream Deck**. Aucun Node.js, aucun terminal,
aucun service à démarrer : le plugin embarque et supervise un binaire local invisible.

> **Windows uniquement.** Le pilotage de Deezer repose sur les sessions média Windows et
> sur Core Audio.

## Fonctionnalités

| Action | Effet |
|---|---|
| **Play / Pause** | Bascule la lecture. La pochette de l'album sert de fond de touche |
| **Piste suivante / précédente** | Navigue dans la file d'attente |
| **Volume + / -** | Ajuste le niveau de Deezer dans le mixeur Windows |
| **Morceau en cours** | Pochette, titre, artiste et temps écoulé ; appui pour basculer la lecture |
| **État Deezer** | Affiche l'état du service local ; appui pour le redémarrer |
| **Overlay OBS** | Ouvre un aperçu de l'overlay dans le navigateur |

Les titres et noms d'artiste trop longs **défilent automatiquement**, sur les touches comme
dans l'overlay OBS.

L'overlay OBS propose quatre thèmes, une couleur d'accent et un **analyseur de spectre**
optionnel sur la barre de progression, calculé à partir du flux audio de Deezer lui-même.

Le plugin est disponible en **français et en anglais**, selon la langue choisie dans
Stream Deck.

## Installation

Téléchargez `com.dezzer.deezer.streamDeckPlugin` depuis la
[dernière release](https://github.com/Captain-Ash/dezzer-stream-deck-plugin/releases) et
ouvrez-le. Stream Deck demande confirmation, c'est tout.

Voir le [guide utilisateur](docs/user-guide.fr.md) pour l'usage au quotidien.

### Vérifier une release

Chaque release contient un fichier `SHA256SUMS` (empreinte de
`com.dezzer.deezer.streamDeckPlugin`) et sa signature GPG détachée `SHA256SUMS.asc`. La clé
publique est [`signing-key.asc`](signing-key.asc) à la racine de ce dépôt.

```powershell
gpg --import signing-key.asc
gpg --verify SHA256SUMS.asc SHA256SUMS
Get-FileHash com.dezzer.deezer.streamDeckPlugin -Algorithm SHA256
```

Un résultat de confiance affiche `Good signature from "Captain_Ash"` avec l'empreinte :

```text
11D8 8CD4 6C6E 2372 84E7  F983 5B49 522A 022D 7C02
```

## Limites connues

- **Aléatoire et répétition ne sont pas disponibles.** Deezer Desktop déclare ces deux
  capacités comme non prises en charge, et les appels Windows correspondants n'ont aucun
  effet observable.
- **Le volume est celui du mixeur Windows**, pas le curseur interne de Deezer : Windows
  n'offre aucun moyen de piloter le volume propre à une application. Les deux se multiplient.
- Le volume et l'analyseur de spectre restent inactifs tant que Deezer n'a pas produit de
  son au moins une fois, car Windows ne crée la session audio qu'à ce moment-là.

Le bridge écoute sur un **port fixe** (39217), afin que l'URL collée dans OBS reste valable
d'un redémarrage à l'autre.

## Architecture

```text
Stream Deck ──┐
              │ HTTP + WebSocket sur 127.0.0.1:39217, jeton d'installation
OBS ──────────┤
              ▼
        dezzer-bridge (binaire Rust ~1,4 Mo, sans fenêtre)
              │
              ├──► Windows Global Media Transport Controls  (lecture, métadonnées)
              ├──► Windows Core Audio                       (volume par application)
              └──► Capture applicative + FFT                (analyseur de spectre)
                          │
                          ▼
                    Deezer Desktop
```

| Dossier | Rôle |
|---|---|
| `apps/bridge` | Bridge local (Rust) : adapters, API HTTP/WebSocket, service de l'overlay |
| `apps/streamdeck-plugin` | Plugin Stream Deck (TypeScript) et son Property Inspector |
| `apps/overlay` | Widget OBS (TypeScript + CSS, sans dépendance externe) |
| `packages/playback-contract` | Contrat de données partagé, miroir de `apps/bridge/src/contract.rs` |
| `scripts` | Spike, génération d'icônes, packaging, installation |

## Prérequis de développement

- Node.js ≥ 20
- Rust stable (`rustup default stable`) + un éditeur de liens MSVC
- Windows 10/11, Deezer Desktop, Stream Deck ≥ 6.5

## Commandes

```powershell
npm install

npm run build          # icônes + overlay + plugin + bridge (release) -> .sdPlugin
npm run plugin:install # copie dans Stream Deck et le redémarre
npm run pack           # produit dist/com.dezzer.deezer.streamDeckPlugin

npm test               # tests TypeScript et Rust
npm run typecheck

npm run bridge:dev     # bridge seul, jeton de développement affiché sur stdout
npm run spike          # rapport de compatibilité des sessions média Windows
```

Développer sans Deezer :

```powershell
$env:DEZZER_BRIDGE_ADAPTER = "mock"
npm run bridge:dev
```

## Sécurité en bref

- Écoute exclusivement sur `127.0.0.1`, sur un port fixe et non privilégié.
- Jeton de 256 bits propre à l'installation, exigé sur toute l'API, régénérable à la demande.
- En-têtes `Origin` et `Host` vérifiés : une page web distante ne peut pas piloter le bridge.
- Aucun identifiant Deezer n'est demandé ni stocké.
- Les logs ne contiennent jamais le jeton.


## Documentation

- [Guide utilisateur](docs/user-guide.fr.md) — [English version](docs/user-guide.md)
- [Contribuer](CONTRIBUTING.md)

## Marques

Deezer est une marque de Deezer S.A. Ce projet est un plugin indépendant et non officiel,
sans affiliation ni approbation de Deezer. Les icônes livrées dans ce dépôt doivent être
remplacées avant toute soumission au Marketplace Stream Deck.

## Licence

[PolyForm Noncommercial 1.0.0](LICENSE) — libre d'usage, de modification et de partage
pour tout usage **non commercial**.

L'usage commercial n'est pas accordé par cette licence. Vendre Dezzer, l'intégrer à un
produit ou service payant, ou monétiser un travail dérivé exige l'accord écrit préalable de
Captain Ash.

---

L'empreinte SHA256 de chaque fichier de release est signée avec GPG par **Captain_Ash**
(empreinte `11D8 8CD4 6C6E 2372 84E7 F983 5B49 522A 022D 7C02`, voir
[signing-key.asc](signing-key.asc) et « Vérifier une release » ci-dessus).
