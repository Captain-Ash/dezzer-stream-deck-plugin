# Guide utilisateur — Deezer

[English version](user-guide.md)

## Installation

1. Ouvrez le fichier `com.dezzer.deezer.streamDeckPlugin` (ou installez le plugin depuis le
   Marketplace Stream Deck).
2. Confirmez l'installation dans Stream Deck.

Il n'y a rien d'autre à installer : ni Node.js, ni Python, ni service à démarrer.

Le plugin suit la langue choisie dans Stream Deck. Le français et l'anglais sont
disponibles.

## Premier usage

1. Ouvrez **Deezer Desktop** et lancez une piste.
2. Dans Stream Deck, ouvrez la catégorie **Deezer** et déposez l'action **Play / Pause** sur
   une touche.
3. La touche affiche l'état réel de la lecture dans la seconde.

Si la touche affiche `Deezer OFF`, c'est que Deezer Desktop n'est pas lancé ou qu'aucune
piste n'a encore été jouée depuis son démarrage.

## Actions disponibles

| Action | Effet |
|---|---|
| **Play / Pause** | Bascule la lecture. La pochette de l'album s'affiche en fond |
| **Piste suivante** | Passe au titre suivant |
| **Piste précédente** | Revient au titre précédent |
| **Volume +** / **Volume -** | Ajuste le volume de Deezer |
| **Morceau en cours** | Pochette, titre, artiste et temps écoulé ; l'appui bascule la lecture |
| **État Deezer** | Affiche l'état du service local ; l'appui le redémarre |

Un titre ou un artiste trop long **défile automatiquement** sur la touche.

> **Un widget « Now Playing » pour OBS ?** Ce plugin ne s'en occupe plus. Installez le
> produit dédié : [Deezer OBS Overlay](https://github.com/Captain-Ash/deezer-obs-overlay).
> Il se gère entièrement depuis OBS et peut tourner en même temps que ce plugin.

### À savoir sur le volume

Windows ne permet pas de piloter le curseur interne d'une application. Les actions Volume
agissent donc sur le volume de Deezer **dans le mixeur Windows** — le même que celui du
clic droit sur l'icône haut-parleur.

Conséquences :

- Le curseur affiché **dans** Deezer ne bouge pas.
- Les deux volumes se combinent : 50 % dans Deezer et 50 % dans le mixeur donnent 25 % de son.
- Les touches restent inactives tant que Deezer n'a pas joué de son depuis son lancement,
  car Windows ne crée la session audio qu'à ce moment-là.

Le pas d'incrément (1, 2, 5 ou 10 %) se règle dans les paramètres de n'importe quelle
action Deezer.

### Aléatoire et répétition

Ces deux commandes ne sont **pas disponibles**. Deezer Desktop les déclare comme non prises
en charge, et les appels Windows correspondants renvoient un succès sans rien changer.
Elles ne sont donc pas proposées, plutôt que d'offrir des touches sans effet.

## Si quelque chose ne marche pas

| Message sur la touche | Que faire |
|---|---|
| `Démarrage…` | Patientez quelques secondes, le service local se lance |
| `Deezer OFF` | Ouvrez Deezer Desktop et lancez une piste |
| `Bridge OFF` | Appuyez sur la touche **État Deezer** pour relancer le service |
| `Bridge KO` | Ouvrez les réglages, section État, puis **Redémarrer le service** |
| `Non pris en charge` | Cette version de Deezer ou de Windows n'expose pas ce contrôle |

Le Property Inspector affiche en permanence l'état du service local, celui de Deezer et la
liste des capacités réellement disponibles. Le bloc **Diagnostic exportable** peut être copié
tel quel dans un rapport de bug : il ne contient aucun jeton.

## Vie privée

- Tout fonctionne en local. Le service n'ouvre aucune connexion vers Internet.
- Aucun identifiant Deezer n'est demandé.
- Les métadonnées de lecture ne quittent jamais votre machine.
- Aucun flux audio n'est capturé ni analysé.
- Les journaux techniques sont conservés localement dans
  `%LOCALAPPDATA%\Deezer\logs`, avec une rotation sur 5 jours.

## Désinstallation

Désinstallez le plugin depuis Stream Deck. Le service local s'arrête automatiquement avec
lui. Le dossier `%LOCALAPPDATA%\Deezer` peut être supprimé manuellement : il ne contient que
des journaux et un fichier d'état.
