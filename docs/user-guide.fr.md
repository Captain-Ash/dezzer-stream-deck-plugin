# Guide utilisateur — Dezzer

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
2. Dans Stream Deck, ouvrez la catégorie **Dezzer** et déposez l'action **Play / Pause** sur
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
| **Overlay OBS** | Ouvre un aperçu de l'overlay dans votre navigateur |

Un titre ou un artiste trop long **défile automatiquement** sur la touche, et de la même
manière dans l'overlay OBS.

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
action Dezzer.

### Aléatoire et répétition

Ces deux commandes ne sont **pas disponibles**. Deezer Desktop les déclare comme non prises
en charge, et les appels Windows correspondants renvoient un succès sans rien changer.
Elles ne sont donc pas proposées, plutôt que d'offrir des touches sans effet.

## Overlay dans OBS

1. Sélectionnez n'importe quelle action Dezzer dans Stream Deck pour ouvrir ses réglages.
2. Section **Overlay OBS** : choisissez le thème, la largeur et les éléments affichés.
3. Cliquez sur **Copier l'URL**.
4. Dans OBS : **Sources → + → Navigateur**.
5. Collez l'URL, réglez la largeur sur **720** et la hauteur sur **160**.
6. Validez. Le widget apparaît sur fond transparent.

> **L'URL contient un jeton d'accès.** Ne l'affichez pas en direct et ne la partagez pas.
> Le Property Inspector la masque par défaut ; le bouton de copie fonctionne sans la révéler.
> Si vous l'avez montrée par inadvertance, le bouton **Régénérer le jeton** invalide
> l'ancienne URL — il faudra alors recoller la nouvelle dans OBS.

L'URL utilise un **port fixe** : elle reste valable après un redémarrage de Windows, de
Stream Deck ou du service.

### Réglages de l'overlay

| Réglage | Valeurs | Défaut |
|---|---|---|
| Thème | `minimal`, `glass`, `neon`, `broadcast` | `glass` |
| Largeur | 400 à 1200 px | 720 |
| Couleur d'accent | couleur hexadécimale, ex. `#ff0066` | celle du thème |
| Pochette / Album / Temps | affiché ou non | pochette et temps affichés |
| Waveform | remplace la barre de progression par une forme d'onde en direct | désactivé |
| Waveform | remplace la barre de progression par un spectre audio en direct | désactivé |
| Masquage auto | masque le widget après un délai | désactivé |

Une valeur invalide est ignorée et remplacée par le défaut.

### À savoir sur la waveform

Activée, la barre de progression devient un **analyseur de spectre** : une barre par bande
de fréquence, des graves à gauche aux aigus à droite, qui réagit à la musique en direct.
Les barres déjà écoutées sont à la couleur d'accent : la barre indique donc toujours votre
position dans le morceau.

Le service capture **uniquement le flux audio de Deezer**, grâce à la capture applicative
que Windows propose depuis la version 2004. Discord, les notifications, votre micro et le
reste du système n'apparaissent jamais dans le spectre. Aucun périphérique virtuel ni
pilote n'est installé.

- Le spectre reste plat tant que Deezer n'a pas produit de son depuis son lancement, car
  Windows ne crée la session audio qu'à ce moment-là.
- Rien n'est capturé ni analysé tant qu'aucun overlay n'affiche le spectre.
- L'audio n'est jamais enregistré ni écrit sur le disque : chaque bloc est analysé puis jeté.

### À propos de la waveform

Quand elle est activée, la barre de progression devient une forme d'onde qui réagit à la
musique : les barres sont dessinées à partir du **niveau audio réel de Deezer**, lu dans le
mixeur Windows.

- La waveform se construit au fil de la lecture ; elle n'est pas connue à l'avance.
- Elle est remise à zéro à chaque changement de piste.
- Elle reste plate si Deezer est coupé dans le mixeur Windows, ou n'a pas encore produit de
  son.

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
- L'audio analysé pour le spectre n'est ni enregistré ni transmis : chaque bloc est traité
  puis jeté, et rien n'est capturé hors affichage du spectre.
- Les journaux techniques sont conservés localement dans
  `%LOCALAPPDATA%\Dezzer\logs`, avec une rotation sur 5 jours.

## Désinstallation

Désinstallez le plugin depuis Stream Deck. Le service local s'arrête automatiquement avec
lui. Le dossier `%LOCALAPPDATA%\Dezzer` peut être supprimé manuellement : il ne contient que
des journaux et un fichier d'état.
