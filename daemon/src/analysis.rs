//! Faithful port of quickshell/analysis.js (same constants, same order of operations, f64 like JS)
//! so a preset reacts identically under the daemon, Quickshell and the web preview.

pub const BANDS: usize = 16;

#[derive(Clone, Debug)]
pub struct State {
    pub spec: [f64; BANDS],
    pub prev_bass: f64,
    pub rise_avg: f64,
    pub peak: f64,
    pub beat: f64,
    pub since_beat: f64,
    pub flow: f64,
    pub bass: f64,
    pub mid: f64,
    pub high: f64,
    pub level: f64,
}

impl Default for State {
    fn default() -> Self {
        State {
            spec: [0.0; BANDS],
            prev_bass: 0.0,
            rise_avg: 0.0,
            peak: 0.2,
            beat: 0.0,
            since_beat: 1.0,
            flow: 0.0,
            bass: 0.0,
            mid: 0.0,
            high: 0.0,
            level: 0.0,
        }
    }
}

fn avg(a: &[f64], from: usize, to: usize) -> f64 {
    a[from..=to].iter().sum::<f64>() / (to - from + 1) as f64
}

// JS Math.round for the non-negative values used here.
fn round(x: f64) -> usize {
    (x + 0.5).floor() as usize
}

impl State {
    /// `raw`: band values 0..1, `dt`: seconds since the previous call.
    pub fn analyse(&mut self, raw: &[f64; BANDS], dt: f64) {
        let dt = dt.clamp(0.001, 0.1);
        let up = 1.0 - (-dt * 30.0).exp();
        let down = 1.0 - (-dt * 7.0).exp();

        let raw_max = raw.iter().fold(0.0f64, |m, &v| m.max(v));
        // Auto gain: follow peaks quickly, forget them slowly, never divide by silence.
        self.peak = raw_max.max(self.peak * (-dt * 0.35).exp()).max(0.08);

        for (s, &r) in self.spec.iter_mut().zip(raw) {
            let v = (r / self.peak).min(1.0);
            *s += (v - *s) * if v > *s { up } else { down };
        }

        let n = BANDS - 1;
        let b1 = round(n as f64 * 0.15);
        let b2 = round(n as f64 * 0.55);
        self.bass = avg(&self.spec, 0, b1);
        self.mid = avg(&self.spec, b1 + 1, b2);
        self.high = avg(&self.spec, b2 + 1, n);
        self.level = avg(&self.spec, 0, n);

        // Kick detection: how fast the bass rises (per second, so frame-rate independent) compared with its
        // running average. A level threshold missed most kicks over a sustained bassline. The 0.3 s refractory
        // period keeps off-beat bass notes from doubling the beat (fine up to ~200 bpm).
        let bass_now = avg(raw, 0, b1) / self.peak;
        let rise = (bass_now - self.prev_bass).max(0.0) / dt;
        self.prev_bass = bass_now;
        self.since_beat += dt;
        if rise > self.rise_avg * 2.5 + 2.0 && self.since_beat > 0.3 {
            self.beat = 1.0;
            self.since_beat = 0.0;
        } else {
            self.beat *= (-dt * 7.0).exp();
        }
        self.rise_avg += (rise - self.rise_avg) * (1.0 - (-dt * 1.0).exp());

        self.flow += dt * (0.3 + 1.6 * self.level);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kick_is_detected_and_decays() {
        let mut s = State::default();
        let quiet = [0.0; BANDS];
        for _ in 0..60 {
            s.analyse(&quiet, 1.0 / 60.0);
        }
        assert!(s.beat < 0.01 && s.level < 0.01);
        let mut kick = [0.05; BANDS];
        kick[..3].fill(0.9);
        s.analyse(&kick, 1.0 / 60.0);
        assert_eq!(s.beat, 1.0);
        for _ in 0..5 {
            s.analyse(&kick, 1.0 / 60.0);
        }
        assert!(s.bass > 0.5 && s.high < s.bass);
        for _ in 0..19 {
            s.analyse(&quiet, 1.0 / 60.0);
        }
        // exp(-7 * 0.4) ≈ 0.06: "decays to 0 in ~0.4 s"
        assert!(s.beat < 0.1, "beat {}", s.beat);
    }

    /// 124 bpm: sustained bass, a kick on every beat and a bass note on every off-beat.
    /// Expect exactly one beat per kick, close to its onset, none on the bassline alone and none in silence after.
    #[test]
    fn one_beat_per_kick_over_a_bassline() {
        const FPS: f64 = 60.0;
        let dt = 1.0 / FPS;
        let period = 60.0 / 124.0;
        let mut s = State::default();
        let mut beats = Vec::new();
        let frames = |t0: f64, t1: f64| ((t0 * FPS) as usize)..((t1 * FPS) as usize);
        let run = |s: &mut State, range: std::ops::Range<usize>, f: &dyn Fn(f64) -> f64, beats: &mut Vec<f64>| {
            for i in range {
                let t = i as f64 * dt;
                let mut raw = [0.02; BANDS];
                raw[..3].fill(f(t));
                s.analyse(&raw, dt);
                if s.since_beat == 0.0 {
                    beats.push(t);
                }
            }
        };
        // Bassline fading in over 1 s (an abrupt start would rightly count as an onset), then 1 s alone: no beats.
        run(&mut s, frames(0.0, 1.0), &|t| 0.3 * t, &mut beats);
        run(&mut s, frames(1.0, 2.0), &|_| 0.3, &mut beats);
        assert!(beats.is_empty(), "beats without kicks: {beats:?}");
        // Kicks from t = 2 s; the off-beat bass notes join after 2 bars (8 beats). Known limits of the reference
        // logic, kept for parity with analysis.js: the first kick after the bassline is missed (auto-gain lifts
        // `peak` to the kick in the same frame), and if an off-beat note ever fires, the next kick falls inside the
        // 0.3 s refractory window, so detection stays locked half a beat late.
        let song = |t: f64| {
            let p = (t - 2.0).rem_euclid(period);
            let kick = 0.9 * (-p / 0.06).exp();
            let notes = t > 2.0 + 8.0 * period;
            let offbeat = if notes && p > period / 2.0 && p < period / 2.0 + 0.2 { 0.6 } else { 0.3 };
            kick.max(offbeat)
        };
        let start = 2.0 + 8.0 * period - 0.5 * dt;
        run(&mut s, frames(2.0, start), &song, &mut beats);
        assert!(beats.len() >= 6, "kicks over a plain bassline: {beats:?}");
        beats.clear();
        let kicks: Vec<f64> = (8..18).map(|k| 2.0 + k as f64 * period).collect();
        let end = 2.0 + 18.0 * period;
        run(&mut s, frames(start, end), &song, &mut beats);
        assert_eq!(beats.len(), kicks.len(), "beats {beats:?}");
        for (b, k) in beats.iter().zip(&kicks) {
            assert!((b - k).abs() < 2.0 * dt, "beat at {b:.3} for kick at {k:.3}");
        }
        // Silence afterwards: the beat fades and never fires.
        let n = beats.len();
        run(&mut s, frames(end, end + 2.0), &|_| 0.0, &mut beats);
        assert_eq!(beats.len(), n);
        assert!(s.beat < 0.01);
    }

    #[test]
    fn band_split_matches_js() {
        // n = 15: bass 0..=2, mid 3..=8, high 9..=15 (JS Math.round(2.25) = 2, Math.round(8.25) = 8)
        assert_eq!(round(15.0 * 0.15), 2);
        assert_eq!(round(15.0 * 0.55), 8);
        assert_eq!(round(2.5), 3);
    }
}
