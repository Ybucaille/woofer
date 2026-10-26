//! Faithful port of quickshell/analysis.js (same constants, same order of operations, f64 like JS)
//! so a preset reacts identically under the daemon, Quickshell and the web preview.

pub const BANDS: usize = 16;

#[derive(Clone, Debug)]
pub struct State {
    pub spec: [f64; BANDS],
    pub slow_bass: f64,
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
            slow_bass: 0.0,
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

        // Kick detection: fast bass rising well above its slow average.
        let fast_bass = avg(raw, 0, b1) / self.peak;
        self.since_beat += dt;
        if fast_bass > self.slow_bass * 1.35 + 0.06 && self.since_beat > 0.18 {
            self.beat = 1.0;
            self.since_beat = 0.0;
        } else {
            self.beat *= (-dt * 7.0).exp();
        }
        self.slow_bass += (fast_bass - self.slow_bass) * (1.0 - (-dt * 2.0).exp());

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

    #[test]
    fn band_split_matches_js() {
        // n = 15: bass 0..=2, mid 3..=8, high 9..=15 (JS Math.round(2.25) = 2, Math.round(8.25) = 8)
        assert_eq!(round(15.0 * 0.15), 2);
        assert_eq!(round(15.0 * 0.55), 8);
        assert_eq!(round(2.5), 3);
    }
}
