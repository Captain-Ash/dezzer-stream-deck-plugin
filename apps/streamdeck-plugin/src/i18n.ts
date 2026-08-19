/**
 * Traductions des textes affichés par le plugin.
 *
 * Les libellés vivent dans `<langue>.json` à la racine du paquet, les mêmes fichiers que
 * Stream Deck utilise pour localiser le manifest : une seule source par langue.
 *
 * Les fonctions d'état renvoient des clés plutôt que des phrases, afin de rester pures et
 * testables sans connexion à Stream Deck.
 */

import streamDeck from "@elgato/streamdeck";

export function t(key: string): string {
  return streamDeck.i18n.t(key);
}
