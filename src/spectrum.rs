//! Spectrum levels: from CAVA's 8-bit bars to what the deck draws.
//!
//! NPLAY runs CAVA with autosens, so its output is already normalised to the
//! loudest bar. That leaves most bars near the floor (the median bar is about
//! 7 % of full scale on typical music), and a plain power curve kept the
//! display low. Two pure stages fix that here, without touching NPLAY:
//!
//! 1. [`Leveler`]: a decibel scale around a slowly tracked reference (the 85th
//!    percentile of recent frames, fast up, slow down, frozen in silence). The
//!    reference sits at 80 % of the height, 36 dB span the display, and a soft
//!    knee above 85 % keeps loud passages from pinning at the top. Quiet and
//!    loud masters end up with the same typical height.
//! 2. [`Bars`]: a fast attack, a gravity fall (bars drop faster the longer
//!    they fall) and peak caps that hold briefly and then fall the same way.
//!
//! Both are cheap (32 values per frame) and settle on a flat spectrum, so the
//! frame scheduler stops asking for frames in silence or while paused.

/// Decibels shown between the floor and the reference level.
const RANGE_DB: f32 = 36.0;
/// Height of the reference level (0..1).
const REFERENCE_AT: f32 = 0.80;
/// Above this height values are compressed toward 1.0 instead of clipped.
const KNEE: f32 = 0.85;
/// Percentile of a frame's bars that drives the reference.
const REFERENCE_PERCENTILE: f32 = 0.85;
/// Reference time constants (seconds): rise, fall.
const REF_ATTACK: f32 = 0.35;
const REF_RELEASE: f32 = 3.0;
/// Frames whose reference bar is below this are treated as silence.
const SILENCE: f32 = 0.01;
/// The lowest reference (limits the gain on near-silent material).
const MIN_REFERENCE: f32 = 0.03;

/// Bar attack time constant (seconds).
const ATTACK: f32 = 0.03;
/// Bar and cap fall acceleration, in heights per second squared.
const GRAVITY: f32 = 3.2;
/// How long a peak cap stays up before it falls (seconds).
const PEAK_HOLD: f32 = 0.45;
/// Values closer than this count as arrived.
const EPSILON: f32 = 0.004;

#[derive(Clone, Debug, Default)]
pub struct Leveler {
    reference: Option<f32>,
}

impl Leveler {
    /// Map one CAVA frame (0..255 per bar) to display heights (0..1). `dt` is
    /// the time since the previous frame, in seconds.
    pub fn level(&mut self, raw: &[u8], dt: f32) -> Vec<f32> {
        let dt = dt.clamp(0.001, 0.5);
        let r = percentile(raw, REFERENCE_PERCENTILE) as f32 / 255.0;
        if r > SILENCE {
            let reference = self.reference.get_or_insert(r);
            let tau = if r > *reference {
                REF_ATTACK
            } else {
                REF_RELEASE
            };
            *reference += (r - *reference) * (1.0 - (-dt / tau).exp());
        }
        let reference = self.reference.unwrap_or(0.1).max(MIN_REFERENCE);
        raw.iter().map(|b| map_db(*b, reference)).collect()
    }

    pub fn reset(&mut self) {
        self.reference = None;
    }

    #[cfg(test)]
    pub fn reference(&self) -> Option<f32> {
        self.reference
    }
}

fn percentile(raw: &[u8], q: f32) -> u8 {
    if raw.is_empty() {
        return 0;
    }
    let mut counts = [0u16; 256];
    for b in raw {
        counts[*b as usize] += 1;
    }
    let want = ((raw.len() as f32 * q) as usize).min(raw.len() - 1);
    let mut seen = 0usize;
    for (value, n) in counts.iter().enumerate() {
        seen += *n as usize;
        if seen > want {
            return value as u8;
        }
    }
    255
}

fn map_db(b: u8, reference: f32) -> f32 {
    if b == 0 {
        return 0.0;
    }
    let x = b as f32 / 255.0;
    let mut y = REFERENCE_AT + 20.0 * (x / reference).log10() / RANGE_DB;
    if y > KNEE {
        let room = 1.0 - KNEE;
        y = KNEE + room * (1.0 - (-(y - KNEE) / room).exp());
    }
    y.clamp(0.0, 1.0)
}

/// Displayed bars with gravity fall and peak caps.
#[derive(Clone, Debug, Default)]
pub struct Bars {
    pub shown: Vec<f32>,
    pub peaks: Vec<f32>,
    fall: Vec<f32>,
    peak_fall: Vec<f32>,
    hold: Vec<f32>,
}

impl Bars {
    pub fn is_empty(&self) -> bool {
        self.shown.is_empty()
    }

    pub fn len(&self) -> usize {
        self.shown.len()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Start over at `values` (a new bar count, or the first frame).
    pub fn reset(&mut self, values: &[f32]) {
        let n = values.len();
        self.shown = values.to_vec();
        self.peaks = values.to_vec();
        self.fall = vec![0.0; n];
        self.peak_fall = vec![0.0; n];
        self.hold = vec![PEAK_HOLD; n];
    }

    /// Move toward `target` by `dt` seconds. Returns true once bars and caps
    /// have all arrived, so no further frame is needed.
    pub fn step(&mut self, target: &[f32], dt: f32) -> bool {
        if target.len() != self.shown.len() {
            self.reset(target);
            return true;
        }
        let dt = dt.clamp(0.001, 0.2);
        let attack = 1.0 - (-dt / ATTACK).exp();
        let mut settled = true;
        for (i, &t) in target.iter().enumerate() {
            let s = &mut self.shown[i];
            if t >= *s {
                self.fall[i] = 0.0;
                if t - *s < EPSILON {
                    *s = t;
                } else {
                    *s += (t - *s) * attack;
                    settled = false;
                }
            } else {
                self.fall[i] += GRAVITY * dt;
                *s = (*s - self.fall[i] * dt).max(t);
                if *s - t < EPSILON {
                    *s = t;
                    self.fall[i] = 0.0;
                } else {
                    settled = false;
                }
            }
            let shown = *s;
            let p = &mut self.peaks[i];
            if shown >= *p {
                *p = shown;
                self.hold[i] = PEAK_HOLD;
                self.peak_fall[i] = 0.0;
            } else if self.hold[i] > 0.0 {
                self.hold[i] -= dt;
                settled = false;
            } else {
                self.peak_fall[i] += GRAVITY * dt;
                *p = (*p - self.peak_fall[i] * dt).max(shown);
                if *p - shown < EPSILON {
                    *p = shown;
                } else {
                    settled = false;
                }
            }
        }
        settled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(n: usize, f: impl Fn(usize, usize) -> u8) -> Vec<Vec<u8>> {
        (0..n).map(|k| (0..32).map(|i| f(k, i)).collect()).collect()
    }

    fn median_height(leveler: &mut Leveler, frames: &[Vec<u8>]) -> f32 {
        let mut all = Vec::new();
        for f in frames {
            all.extend(leveler.level(f, 1.0 / 30.0));
        }
        all.sort_by(|a, b| a.partial_cmp(b).unwrap());
        all[all.len() / 2]
    }

    #[test]
    fn quiet_and_loud_material_reach_the_same_typical_height() {
        let shape = |k: usize, i: usize| ((i * 37 + k * 11) % 100) as f32 / 100.0;
        let loud = frames(300, |k, i| (40.0 + 200.0 * shape(k, i)) as u8);
        let quiet = frames(300, |k, i| (4.0 + 20.0 * shape(k, i)) as u8);
        let a = median_height(&mut Leveler::default(), &loud);
        let b = median_height(&mut Leveler::default(), &quiet);
        assert!((a - b).abs() < 0.08, "loud {a} quiet {b}");
        assert!(a > 0.4 && a < 0.8, "median {a}");
    }

    #[test]
    fn loud_frames_are_compressed_not_pinned() {
        let mut l = Leveler::default();
        let mut top = 0.0f32;
        for _ in 0..120 {
            let out = l.level(&[255; 32], 1.0 / 30.0);
            top = top.max(out[0]);
        }
        assert!((0.75..1.0).contains(&top), "{top}");
        // A single loud bar among quiet ones is shown high but below the top.
        let mut frame = [30u8; 32];
        frame[3] = 255;
        let out = l.level(&frame, 1.0 / 30.0);
        assert!(out[3] > out[0] && out[3] < 1.0);
    }

    #[test]
    fn silence_keeps_the_reference_and_maps_to_zero() {
        let mut l = Leveler::default();
        for _ in 0..30 {
            l.level(&[60; 32], 1.0 / 30.0);
        }
        let before = l.reference().unwrap();
        for _ in 0..300 {
            assert!(l.level(&[0; 32], 1.0 / 30.0).iter().all(|v| *v == 0.0));
        }
        assert_eq!(l.reference().unwrap(), before);
    }

    #[test]
    fn bars_fall_with_gravity_and_caps_hold_then_settle() {
        let mut b = Bars::default();
        b.reset(&[0.0; 4]);
        assert!(!b.step(&[1.0; 4], 0.2));
        for _ in 0..10 {
            b.step(&[1.0; 4], 1.0 / 30.0);
        }
        assert!(b.shown[0] > 0.99);
        // Release: the bar falls, the cap stays up for the hold time.
        let mut drops = Vec::new();
        let mut last = b.shown[0];
        for _ in 0..6 {
            b.step(&[0.0; 4], 1.0 / 30.0);
            drops.push(last - b.shown[0]);
            last = b.shown[0];
        }
        assert!(
            drops.windows(2).all(|w| w[1] >= w[0]),
            "accelerating {drops:?}"
        );
        assert!(b.peaks[0] > 0.99, "cap holds");
        let mut frames = 0;
        while !b.step(&[0.0; 4], 1.0 / 30.0) {
            frames += 1;
            assert!(frames < 120, "settles");
        }
        assert!(b.shown.iter().chain(b.peaks.iter()).all(|v| *v == 0.0));
    }

    #[test]
    fn percentile_of_a_frame() {
        let frame: Vec<u8> = (0..100).collect();
        assert_eq!(percentile(&frame, 0.85), 85);
        assert_eq!(percentile(&[], 0.5), 0);
    }

    /// Before/after levels on real CAVA captures (raw 8-bit, 32 bars):
    /// `NIRUAMP_VIS_CAPTURES="a.cava b.cava" cargo test spectrum_report -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn spectrum_report() {
        let files = std::env::var("NIRUAMP_VIS_CAPTURES").unwrap_or_default();
        let pct = |v: &mut Vec<f32>, p: f32| {
            v.sort_by(|a, b| a.partial_cmp(b).unwrap());
            v[((v.len() as f32 * p) as usize).min(v.len() - 1)]
        };
        for file in files.split_whitespace() {
            let data = std::fs::read(file).unwrap();
            let frames: Vec<&[u8]> = data.chunks_exact(32).skip(30).collect();
            let dt = 1.0 / 30.0;
            // 0.6: power curve and exponential easing, drawn at 6 % + 94 %.
            let mut old_shown = vec![0.0f32; 32];
            let mut old = Vec::new();
            // 0.7: leveler and gravity bars, drawn at 3 % + 97 %.
            let mut leveler = Leveler::default();
            let mut bars = Bars::default();
            bars.reset(&[0.0; 32]);
            let mut new = Vec::new();
            let (mut old_max, mut new_max) = (Vec::new(), Vec::new());
            for f in &frames {
                let target: Vec<f32> = f.iter().map(|b| (*b as f32 / 255.0).powf(0.62)).collect();
                old_ease(&mut old_shown, &target, dt);
                let h: Vec<f32> = old_shown.iter().map(|v| 0.06 + v * 0.94).collect();
                old_max.push(h.iter().cloned().fold(0.0, f32::max));
                old.extend(h);
                bars.step(&leveler.level(f, dt), dt);
                let h: Vec<f32> = bars.shown.iter().map(|v| 0.03 + v * 0.97).collect();
                new_max.push(h.iter().cloned().fold(0.0, f32::max));
                new.extend(h);
            }
            for (name, v, m) in [
                ("0.6", &mut old, &mut old_max),
                ("0.7", &mut new, &mut new_max),
            ] {
                let pinned = v.iter().filter(|x| **x >= 0.95).count() as f32 / v.len() as f32;
                let floor = v.iter().filter(|x| **x <= 0.08).count() as f32 / v.len() as f32;
                println!(
                    "{file} {name}: height p10 {:.2} p50 {:.2} p90 {:.2}, frame max p50 {:.2}, >=95% {:.1} %, <=8% {:.1} %",
                    pct(v, 0.10),
                    pct(v, 0.50),
                    pct(v, 0.90),
                    pct(m, 0.50),
                    pinned * 100.0,
                    floor * 100.0
                );
            }
        }
    }

    fn old_ease(shown: &mut [f32], target: &[f32], dt: f32) {
        let attack = 1.0 - (-dt / 0.045).exp();
        let release = 1.0 - (-dt / 0.14).exp();
        for (s, t) in shown.iter_mut().zip(target) {
            let d = *t - *s;
            if d.abs() < 0.004 {
                *s = *t;
            } else {
                *s += d * if d >= 0.0 { attack } else { release };
            }
        }
    }
}
