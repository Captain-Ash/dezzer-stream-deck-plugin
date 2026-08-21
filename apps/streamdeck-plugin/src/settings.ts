/** Réglages persistés par Stream Deck. Aucun credential Deezer n'y figure (§9.5). */

export type NowPlayingFormat = "title" | "artist" | "title-artist";

export type GlobalSettings = {
  /**
   * Jeton d'installation. Généré une seule fois puis réutilisé, pour que le service local
   * reste joignable par le plugin après un redémarrage.
   */
  token?: string;
  nowPlayingFormat?: NowPlayingFormat;
  /** Pochette de l'album en fond des touches Play/Pause et Morceau en cours. */
  showArtworkOnKeys?: boolean;
  /** Pas appliqué par les actions Volume + et Volume -, en pourcentage. */
  volumeStep?: number;
};

export const VOLUME_STEPS = [1, 2, 5, 10] as const;

export function normaliseVolumeStep(value: unknown): number {
  const parsed = typeof value === "number" ? value : Number.parseInt(String(value ?? ""), 10);
  return (VOLUME_STEPS as readonly number[]).includes(parsed) ? parsed : 5;
}
