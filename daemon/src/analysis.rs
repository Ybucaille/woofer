//! Faithful port of quickshell/analysis.js (same constants, same order of operations, f64 like JS)
//! so a preset reacts identically under the daemon, Quickshell and the web preview.

pub const BANDS: usize = 16;

#[derive(Clone, Debug)]
pub struct State {
    pub spec: [f64; BANDS],
    pub prev_bass: f64,
    pub rise_avg: f64,
    pub beat_rise: f64,
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
            beat_rise: 0.0,
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
        let prev_peak = self.peak;
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
        // running average. The rise uses last frame's gain, otherwise a loud kick raises the gain in the same frame
        // and hides its own rise. A 0.3 s refractory period keeps off-beat bass notes from doubling the beat, but a
        // clearly stronger rise inside it replaces the last beat, so the detector can't stay locked on off-beats.
        let bass_raw = avg(raw, 0, b1);
        let rise = (bass_raw - self.prev_bass).max(0.0) / dt / prev_peak;
        self.prev_bass = bass_raw;
        self.since_beat += dt;
        let onset = rise > self.rise_avg * 2.5 + 2.0;
        if onset && (self.since_beat > 0.3 || rise > self.beat_rise * 1.15) {
            self.beat = 1.0;
            self.since_beat = 0.0;
            self.beat_rise = rise;
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

    // Mirrors tools/test-analysis.mjs: synthetic bass frames at 60 fps, 124 bpm.
    // Known limit: an off-beat bass note louder than ~80 % of the kick (off_amp > 0.6 here) can still steal the
    // first beat after a bass-only intro; the detector then relocks on the kick within a beat.
    const FPS: f64 = 60.0;
    const DT: f64 = 1.0 / FPS;
    const BEAT: f64 = 60.0 / 124.0;

    struct Song {
        kicks: bool,
        offbeats: bool,
        kick_from: f64,
        off_amp: f64,
        silent: bool,
    }

    impl Song {
        fn new(kicks: bool, offbeats: bool) -> Self {
            Song { kicks, offbeats, kick_from: 0.0, off_amp: 0.6, silent: false }
        }

        /// Bass bins 0..=2: sustained bass, optional kicks on the beat, off-beat notes half a beat later.
        fn frame(&self, t: f64) -> [f64; BANDS] {
            if self.silent {
                return [0.0; BANDS];
            }
            let phase = t % BEAT;
            let mut bass = 0.25;
            if self.kicks && t >= self.kick_from {
                bass += 0.75 * (-phase / 0.08).exp();
            }
            if self.offbeats {
                let p = (t + BEAT / 2.0) % BEAT;
                bass += self.off_amp * if p < 0.02 { p / 0.02 } else { (-(p - 0.02) / 0.1).exp() };
            }
            let mut raw = [0.05; BANDS];
            raw[..3].fill(bass);
            raw
        }

        fn beats(&self, seconds: f64) -> Vec<f64> {
            let mut s = State::default();
            let mut beats = Vec::new();
            for k in 0..(seconds * FPS) as usize {
                let t = k as f64 * DT;
                let before = s.since_beat;
                s.analyse(&self.frame(t), DT);
                if s.since_beat == 0.0 && before > 0.0 {
                    beats.push(t);
                }
            }
            beats
        }
    }

    fn on_beat(t: f64) -> bool {
        let p = t % BEAT;
        p < 3.0 * DT || p > BEAT - DT
    }

    #[test]
    fn silence_gives_no_beat() {
        let song = Song { silent: true, ..Song::new(false, false) };
        assert_eq!(song.beats(3.0), Vec::<f64>::new());
    }

    #[test]
    fn kicks_over_a_bassline_from_the_start() {
        let kicks = 20;
        let beats = Song::new(true, true).beats(kicks as f64 * BEAT);
        assert!((kicks - 1..=kicks).contains(&beats.len()), "{}/{kicks} beats: {beats:?}", beats.len());
        let late: Vec<_> = beats.iter().filter(|&&t| !on_beat(t)).collect();
        assert!(late.is_empty(), "locked onto off-beats: {late:?}");
    }

    #[test]
    fn kicks_entering_after_a_bass_intro() {
        // Kicks enter on a beat after 4 bars of bassline (t = 8 beats ≈ 3.87 s).
        let entry = 8.0 * BEAT;
        let song = Song { kick_from: entry - 1e-6, ..Song::new(true, true) };
        let after: Vec<f64> = song.beats(entry + 10.0 * BEAT).into_iter().filter(|&t| t >= entry - DT).collect();
        assert!(!after.is_empty() && after[0] - entry < 3.0 * DT, "first kick missed: {after:?}");
        assert!(after.len() >= 9 && after.iter().all(|&t| on_beat(t)), "{}/10 on the beat: {after:?}", after.len());
    }

    #[test]
    fn band_split_matches_js() {
        // n = 15: bass 0..=2, mid 3..=8, high 9..=15 (JS Math.round(2.25) = 2, Math.round(8.25) = 8)
        assert_eq!(round(15.0 * 0.15), 2);
        assert_eq!(round(15.0 * 0.55), 8);
        assert_eq!(round(2.5), 3);
    }
}
