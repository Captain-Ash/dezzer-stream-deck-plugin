/**
 * Visualiseur de spectre de la barre de progression.
 *
 * Les niveaux viennent du bridge, qui capture le flux audio de Deezer seul et en calcule
 * la transformée de Fourier. Une barre par bande, du grave à l'aigu, et la portion déjà
 * écoutée est peinte à la couleur d'accent : le tracé suit le son, le remplissage suit le
 * temps.
 */

/** Hauteur minimale d'une barre, en fraction de la hauteur disponible. */
const MIN_BAR_RATIO = 0.06;

/** Part de la largeur d'un segment réellement peinte ; le reste fait la gouttière. */
const BAR_FILL_RATIO = 0.66;

/**
 * Lissage d'affichage.
 *
 * Le bridge lisse déjà l'attaque et la retombée, mais il émet à 30 Hz alors que l'overlay
 * dessine à 60 : interpoler ici évite l'effet d'escalier.
 */
const GLIDE = 0.35;

export class Waveform {
  private readonly context: CanvasRenderingContext2D | null;
  private levels: number[] = [];
  private displayed: number[] = [];

  constructor(private readonly canvas: HTMLCanvasElement) {
    this.context = canvas.getContext("2d");
  }

  reset(): void {
    this.levels = this.levels.map(() => 0);
  }

  /** Reçoit une trame de spectre : un niveau de 0 à 1 par bande. */
  push(bands: number[]): void {
    this.levels = bands.map((band) => (Number.isFinite(band) ? Math.min(1, Math.max(0, band)) : 0));
    if (this.displayed.length !== this.levels.length) {
      this.displayed = [...this.levels];
    }
  }

  draw(progressRatio: number, playedColour: string, remainingColour: string): void {
    const context = this.context;
    if (!context || this.levels.length === 0) return;

    const width = this.canvas.clientWidth;
    const height = this.canvas.clientHeight;
    if (width === 0 || height === 0) return;

    const dpr = window.devicePixelRatio || 1;
    const pixelWidth = Math.round(width * dpr);
    const pixelHeight = Math.round(height * dpr);
    if (this.canvas.width !== pixelWidth || this.canvas.height !== pixelHeight) {
      this.canvas.width = pixelWidth;
      this.canvas.height = pixelHeight;
    }

    context.setTransform(dpr, 0, 0, dpr, 0, 0);
    context.clearRect(0, 0, width, height);

    const count = this.levels.length;
    const slot = width / count;
    const barWidth = Math.max(1, slot * BAR_FILL_RATIO);
    const radius = Math.min(barWidth / 2, 3);
    const played = progressRatio * count;

    for (let index = 0; index < count; index += 1) {
      const target = this.levels[index] ?? 0;
      const current = this.displayed[index] ?? target;
      const level = current + (target - current) * GLIDE;
      this.displayed[index] = level;

      const barHeight = Math.max(height * MIN_BAR_RATIO, level * height);
      const x = index * slot + (slot - barWidth) / 2;

      context.fillStyle = index < played ? playedColour : remainingColour;
      context.beginPath();
      context.roundRect(x, height - barHeight, barWidth, barHeight, radius);
      context.fill();
    }
  }
}
