//! Analyse spectrale du signal de Deezer, source du visualiseur de l'overlay.
//!
//! La FFT est écrite ici plutôt qu'importée : 2048 points trente fois par seconde sont
//! négligeables pour la machine, alors qu'une bibliothèque alourdirait un binaire que le
//! plugin embarque.
//!
//! Ce module ne touche ni à Windows ni à l'audio : il transforme un bloc d'échantillons en
//! niveaux de bandes, et rien d'autre.

/// Fenêtre d'analyse. À 44,1 kHz, 2048 points donnent des bins de ~21 Hz, assez fins pour
/// que les basses ne s'écrasent pas toutes dans la première barre.
pub const FFT_SIZE: usize = 2048;

/// Barres affichées par l'overlay.
pub const BANDS: usize = 48;

const MIN_HZ: f32 = 40.0;
const MAX_HZ: f32 = 16_000.0;

/// Plancher de la dynamique représentée.
const FLOOR_DB: f32 = -72.0;

/// Montée franche, redescente lente : c'est ce qui rend un spectre lisible à l'œil.
const ATTACK: f32 = 0.6;
const RELEASE: f32 = 0.12;

#[derive(Clone, Copy, Default)]
struct Complex {
    re: f32,
    im: f32,
}

pub struct Analyzer {
    window: Vec<f32>,
    twiddles: Vec<Complex>,
    scratch: Vec<Complex>,
    bands: Vec<(usize, usize)>,
    levels: [f32; BANDS],
}

impl Analyzer {
    pub fn new(sample_rate: u32) -> Self {
        let window = (0..FFT_SIZE)
            .map(|i| {
                let phase = std::f32::consts::TAU * i as f32 / FFT_SIZE as f32;
                0.5 - 0.5 * phase.cos()
            })
            .collect();

        let twiddles = (0..FFT_SIZE / 2)
            .map(|k| {
                let angle = -std::f32::consts::TAU * k as f32 / FFT_SIZE as f32;
                Complex {
                    re: angle.cos(),
                    im: angle.sin(),
                }
            })
            .collect();

        Self {
            window,
            twiddles,
            scratch: vec![Complex::default(); FFT_SIZE],
            bands: band_edges(sample_rate),
            levels: [0.0; BANDS],
        }
    }

    /// Analyse un bloc de `FFT_SIZE` échantillons mono et retourne les niveaux lissés.
    pub fn analyse(&mut self, samples: &[f32]) -> &[f32; BANDS] {
        if samples.len() < FFT_SIZE {
            self.decay();
            return &self.levels;
        }

        for (slot, (sample, window)) in self
            .scratch
            .iter_mut()
            .zip(samples[samples.len() - FFT_SIZE..].iter().zip(&self.window))
        {
            *slot = Complex {
                re: sample * window,
                im: 0.0,
            };
        }

        fft(&mut self.scratch, &self.twiddles);

        for band in 0..BANDS {
            let (start, end) = self.bands[band];
            let mut peak = 0.0f32;
            for bin in start..end {
                let value = self.scratch[bin];
                peak = peak.max((value.re * value.re + value.im * value.im).sqrt());
            }

            // Hann perd la moitie de l'amplitude ; le facteur 4/N ramene une sinusoide
            // pleine echelle a 0 dB.
            let amplitude = peak * 4.0 / FFT_SIZE as f32;
            let decibels = if amplitude > 0.0 {
                20.0 * amplitude.log10()
            } else {
                FLOOR_DB
            };
            let target = ((decibels - FLOOR_DB) / -FLOOR_DB).clamp(0.0, 1.0);

            let current = self.levels[band];
            let rate = if target > current { ATTACK } else { RELEASE };
            self.levels[band] = current + (target - current) * rate;
        }

        &self.levels
    }

    /// Fait retomber le spectre quand le flux se tait, plutôt que de le figer.
    pub fn decay(&mut self) -> &[f32; BANDS] {
        for level in &mut self.levels {
            *level *= 1.0 - RELEASE;
        }
        &self.levels
    }
}

/// Bandes réparties logarithmiquement : c'est ainsi que l'oreille perçoit les hauteurs.
fn band_edges(sample_rate: u32) -> Vec<(usize, usize)> {
    let nyquist_bin = FFT_SIZE / 2;
    let hz_per_bin = sample_rate as f32 / FFT_SIZE as f32;
    let ratio = MAX_HZ / MIN_HZ;

    (0..BANDS)
        .map(|band| {
            let low = MIN_HZ * ratio.powf(band as f32 / BANDS as f32);
            let high = MIN_HZ * ratio.powf((band + 1) as f32 / BANDS as f32);

            let start = ((low / hz_per_bin).floor() as usize).clamp(1, nyquist_bin - 1);
            let end = ((high / hz_per_bin).ceil() as usize).clamp(start + 1, nyquist_bin);
            (start, end)
        })
        .collect()
}

/// Cooley-Tukey radix-2, en place.
fn fft(buffer: &mut [Complex], twiddles: &[Complex]) {
    let n = buffer.len();

    let mut target = 0usize;
    for index in 1..n {
        let mut bit = n >> 1;
        while target & bit != 0 {
            target ^= bit;
            bit >>= 1;
        }
        target |= bit;
        if index < target {
            buffer.swap(index, target);
        }
    }

    let mut span = 2;
    while span <= n {
        let half = span / 2;
        let stride = n / span;
        let mut start = 0;
        while start < n {
            for offset in 0..half {
                let twiddle = twiddles[offset * stride];
                let odd = buffer[start + offset + half];
                let rotated = Complex {
                    re: odd.re * twiddle.re - odd.im * twiddle.im,
                    im: odd.re * twiddle.im + odd.im * twiddle.re,
                };
                let even = buffer[start + offset];
                buffer[start + offset] = Complex {
                    re: even.re + rotated.re,
                    im: even.im + rotated.im,
                };
                buffer[start + offset + half] = Complex {
                    re: even.re - rotated.re,
                    im: even.im - rotated.im,
                };
            }
            start += span;
        }
        span <<= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

    fn sine(frequency: f32, amplitude: f32) -> Vec<f32> {
        (0..FFT_SIZE)
            .map(|i| amplitude * (std::f32::consts::TAU * frequency * i as f32 / RATE as f32).sin())
            .collect()
    }

    fn band_of(frequency: f32) -> usize {
        let ratio = MAX_HZ / MIN_HZ;
        ((frequency / MIN_HZ).log10() / ratio.log10() * BANDS as f32) as usize
    }

    /// Plusieurs passes : l'attaque est volontairement progressive.
    fn settle(analyzer: &mut Analyzer, samples: &[f32]) -> [f32; BANDS] {
        let mut levels = [0.0; BANDS];
        for _ in 0..40 {
            levels = *analyzer.analyse(samples);
        }
        levels
    }

    #[test]
    fn une_sinusoide_n_allume_que_sa_bande() {
        let mut analyzer = Analyzer::new(RATE);
        let levels = settle(&mut analyzer, &sine(1_000.0, 1.0));

        let expected = band_of(1_000.0);
        let loudest = levels
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(index, _)| index)
            .unwrap();

        assert!(
            loudest.abs_diff(expected) <= 1,
            "bande la plus forte {loudest}, attendue {expected}"
        );
        assert!(levels[loudest] > 0.9, "une sinusoide pleine echelle sature");

        // Deux octaves plus bas, il ne doit rien rester.
        assert!(levels[band_of(250.0)] < 0.2);
    }

    #[test]
    fn le_silence_ne_produit_aucun_niveau() {
        let mut analyzer = Analyzer::new(RATE);
        let levels = settle(&mut analyzer, &vec![0.0; FFT_SIZE]);
        assert!(levels.iter().all(|level| *level < 0.01));
    }

    #[test]
    fn un_signal_plus_faible_donne_un_niveau_plus_bas() {
        let mut analyzer = Analyzer::new(RATE);
        let fort = settle(&mut analyzer, &sine(1_000.0, 1.0))[band_of(1_000.0)];

        let mut analyzer = Analyzer::new(RATE);
        let faible = settle(&mut analyzer, &sine(1_000.0, 0.05))[band_of(1_000.0)];

        assert!(faible < fort, "{faible} devrait etre sous {fort}");
        assert!(faible > 0.0);
    }

    #[test]
    fn le_spectre_retombe_quand_le_flux_se_tait() {
        let mut analyzer = Analyzer::new(RATE);
        let peak = settle(&mut analyzer, &sine(1_000.0, 1.0))[band_of(1_000.0)];

        for _ in 0..60 {
            analyzer.decay();
        }
        assert!(analyzer.levels[band_of(1_000.0)] < peak * 0.1);
    }

    #[test]
    fn les_bandes_sont_croissantes_et_dans_le_spectre_utile() {
        let edges = band_edges(RATE);
        assert_eq!(edges.len(), BANDS);
        for window in edges.windows(2) {
            assert!(window[0].0 <= window[1].0, "bandes non croissantes");
        }
        assert!(edges.last().unwrap().1 <= FFT_SIZE / 2);
    }
}
