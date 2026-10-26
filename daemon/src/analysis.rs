//! Faithful port of quickshell/analysis.js (same constants, same order of operations, f64 like JS)
//! so a preset reacts identically under the daemon, Quickshell and the web preview.

pub const BANDS: usize = 16;

#[derive(Clone, Debug)]
pub struct State {
    pub spec: [f64; BANDS],
    pub prev_raw: [f64; BANDS],
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
            prev_raw: [0.0; BANDS],
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

        // Kick detection: the fastest rise of any single band in 40–240 Hz (per second, so frame-rate independent)
        // compared with its running average. Not the bass average: a kick's attack sweeps down from ~200 Hz and jumps
        // a band or two above where a bass note sits, while a saw bass rises evenly across all of them, so averaging
        // lets a loud off-beat bass outscore the kick. The rise uses last frame's gain, otherwise a loud kick raises the
        // gain in the same frame and hides its own rise. A 0.3 s refractory period keeps off-beat bass notes from
        // doubling the beat, but a clearly stronger rise inside it replaces the last beat, so the detector can't stay
        // locked on off-beats.
        let kick_to = round(n as f64 * 0.27);
        let rise = raw[..=kick_to].iter().zip(&self.prev_raw).fold(0.0f64, |m, (&r, &p)| m.max(r - p));
        let rise = rise / dt / prev_peak;
        self.prev_raw = *raw;
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
pub(crate) mod tests {
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

    // Mirrors tools/test-analysis.mjs: synthetic bass frames, 124 bpm, analysed at 60 and 30 Hz.
    // Known limit: in these idealised frames (no analysis window), an off-beat bass note at ~80 % of the kick
    // (off_amp 0.6) still beats a kick sampled 30 ms late, so at 30 Hz it can double the beat; it only runs at 60 Hz.
    // At 30 Hz the first kick after a bass-only intro can be lost the same way; the detector relocks on the next one.
    pub const BEAT: f64 = 60.0 / 124.0;
    const RATES: [f64; 2] = [60.0, 30.0];
    /// Off-beat bass at 0.65 of the kick's 0.75, 30 ms attack.
    const STRONG: (f64, f64) = (0.65 * 0.75, 0.03);

    struct Song {
        kicks: bool,
        offbeats: bool,
        kick_from: f64,
        off_amp: f64,
        off_attack: f64,
        silent: bool,
    }

    impl Song {
        fn new(kicks: bool, offbeats: bool) -> Self {
            Song { kicks, offbeats, kick_from: 0.0, off_amp: 0.6, off_attack: 0.02, silent: false }
        }

        fn strong(self) -> Self {
            Song { off_amp: STRONG.0, off_attack: STRONG.1, ..self }
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
                let a = self.off_attack;
                bass += self.off_amp * if p < a { p / a } else { (-(p - a) / 0.1).exp() };
            }
            let mut raw = [0.05; BANDS];
            raw[..3].fill(bass);
            raw
        }

        fn beats(&self, fps: f64, seconds: f64) -> Vec<f64> {
            let mut s = State::default();
            let mut beats = Vec::new();
            for k in 0..(seconds * fps) as usize {
                let t = k as f64 / fps;
                let before = s.since_beat;
                s.analyse(&self.frame(t), 1.0 / fps);
                if s.since_beat == 0.0 && before > 0.0 {
                    beats.push(t);
                }
            }
            beats
        }
    }

    /// On the frame of the kick or within the next 50 ms.
    fn on_beat(t: f64, fps: f64) -> bool {
        let p = t % BEAT;
        p < (3.0 / fps).max(0.05) || p > BEAT - 1.0 / fps
    }

    #[test]
    fn silence_gives_no_beat() {
        for fps in RATES {
            let song = Song { silent: true, ..Song::new(false, false) };
            assert_eq!(song.beats(fps, 3.0), Vec::<f64>::new(), "at {fps} Hz");
        }
    }

    #[test]
    fn kicks_over_a_bassline_from_the_start() {
        let kicks = 20;
        let cases = [(Song::new(true, true), 60.0), (Song::new(true, true).strong(), 60.0), (Song::new(true, true).strong(), 30.0)];
        for (song, fps) in cases {
            let beats = song.beats(fps, kicks as f64 * BEAT);
            let late: Vec<_> = beats.iter().filter(|&&t| !on_beat(t, fps)).collect();
            assert!((kicks - 1..=kicks).contains(&beats.len()), "{} Hz: {}/{kicks} beats: {beats:?}", fps, beats.len());
            assert!(late.is_empty(), "{fps} Hz: locked onto off-beats: {late:?}");
        }
    }

    #[test]
    fn kicks_entering_after_a_bass_intro() {
        // Kicks enter on a beat after 4 bars of bassline (t = 8 beats ≈ 3.87 s).
        let entry = 8.0 * BEAT;
        let cases = [(Song::new(true, true), 60.0), (Song::new(true, true).strong(), 60.0), (Song::new(true, true).strong(), 30.0)];
        for (song, fps) in cases {
            let song = Song { kick_from: entry - 1e-6, ..song };
            // At 30 Hz the first kick may be seen 30 ms late, barely above the bass it follows; it must relock a beat later.
            let relock = if fps == 60.0 { 0.0 } else { BEAT };
            let from = entry + relock;
            let after: Vec<f64> =
                song.beats(fps, entry + 10.0 * BEAT).into_iter().filter(|&t| t >= from - 1.0 / fps).collect();
            let first_ok = !after.is_empty() && after[0] - from < (3.0 / fps).max(0.05);
            assert!(first_ok, "{fps} Hz: kick missed: {after:?}");
            let all_on = after.iter().all(|&t| on_beat(t, fps));
            let want = if relock > 0.0 { 8 } else { 9 };
            assert!(after.len() >= want && all_on, "{fps} Hz: {} on the beat: {after:?}", after.len());
        }
    }

    /// A house loop as audio (48 kHz mono): a pitch-swept kick on every beat and a bass note on every off-beat.
    /// Same recipe as `house()` in tools/test-analysis.mjs.
    #[derive(Clone, Copy, Debug)]
    pub struct House {
        /// Kick sweep: from k0 to k1 Hz in `sweep` s.
        pub k0: f64,
        pub k1: f64,
        pub sweep: f64,
        /// Off-beat bass: fundamental, peak relative to the kick, attack (s), sine share (rest is saw).
        pub note: f64,
        pub off_amp: f64,
        pub attack: f64,
        pub sine: f64,
    }

    #[derive(Debug)]
    pub struct Score {
        pub kicks: usize,
        pub hit: usize,
        pub off: usize,
        /// Median delay from the kick to the beat, ms.
        pub latency_ms: f64,
    }

    impl House {
        /// Off-beat bass at 0.65 of the kick with a 30 ms attack, over a few kick and bass sounds. Each of these
        /// lost kicks to the bass at 30 Hz with the old detector (the fast-sweep one at 60 Hz too).
        pub fn strong_cases() -> [House; 4] {
            let h = House { k0: 150.0, k1: 45.0, sweep: 0.12, note: 55.0, off_amp: 0.65, attack: 0.03, sine: 0.0 };
            [
                House { note: 41.2, ..h },
                h,
                House { note: 73.4, sine: 1.0, ..h },
                House { k0: 220.0, k1: 50.0, sweep: 0.04, sine: 1.0, ..h },
            ]
        }

        pub fn render(&self, secs: f64) -> Vec<f32> {
            const RATE: f64 = 48000.0;
            let n = (secs * RATE) as usize;
            let mut buf = vec![0.0f64; n];
            let mut k = 0;
            while k as f64 * BEAT < secs {
                let t0 = (k as f64 * BEAT * RATE) as usize;
                let mut ph = 0.0;
                for i in 0..(0.35 * RATE) as usize {
                    let Some(v) = buf.get_mut(t0 + i) else { break };
                    let t = i as f64 / RATE;
                    let f = self.k0 * (self.k1 / self.k0).powf((t / self.sweep).min(1.0));
                    ph += std::f64::consts::TAU * f / RATE;
                    *v += ph.sin() * 0.001f64.powf(t / 0.35);
                }
                let b0 = t0 + (BEAT / 2.0 * RATE) as usize;
                for i in 0..(BEAT / 2.0 * RATE) as usize {
                    let Some(v) = buf.get_mut(b0 + i) else { break };
                    let t = i as f64 / RATE;
                    let saw = 2.0 * ((self.note * t) % 1.0) - 1.0;
                    let sine = (std::f64::consts::TAU * self.note * t).sin();
                    let env = (t / self.attack).min(1.0) * 0.01f64.powf(t / (BEAT / 2.0));
                    *v += self.off_amp * env * ((1.0 - self.sine) * saw + self.sine * sine);
                }
                k += 1;
            }
            buf.iter().map(|&v| (v * 0.4) as f32).collect()
        }

        /// A kick is caught by a beat within 80 ms plus `delay` (s); any other beat is off. Starts after 1 s: the
        /// first kick plays before a full analysis window of audio exists, and the detector may take the bass note
        /// after it before it locks on the next kick.
        pub fn score(&self, beats: &[f64], delay: f64, secs: f64) -> Score {
            let tol = 0.08 + delay;
            let kicks: Vec<f64> = (0..).map(|k| k as f64 * BEAT).skip_while(|&t| t < 1.0).take_while(|&t| t < secs).collect();
            let beats: Vec<f64> = beats.iter().copied().filter(|&b| b >= kicks[0] - 0.01).collect();
            let mut lat: Vec<f64> = kicks
                .iter()
                .filter_map(|&k| beats.iter().find(|&&b| b >= k - 0.01 && b < k + tol).map(|&b| b - k))
                .collect();
            let off = beats.iter().filter(|&&b| !kicks.iter().any(|&k| b >= k - 0.01 && b < k + tol)).count();
            lat.sort_by(f64::total_cmp);
            let latency_ms = lat.get(lat.len() / 2).map_or(f64::NAN, |l| (l * 1000.0).round());
            Score { kicks: kicks.len(), hit: lat.len(), off, latency_ms }
        }
    }

    #[test]
    fn kicks_over_a_strong_bassline_audio() {
        // The whole front end (FFT, bands) and the detector, analysed at 60 and 30 Hz. Mirrors the audio cases of
        // tools/test-analysis.mjs.
        use crate::audio::{Shared, Spectrum};
        let secs = 45.0 * BEAT;
        for house in House::strong_cases() {
            let audio = house.render(secs);
            for fps in RATES {
                let shared = Shared::new();
                let mut sp = Spectrum::with_steps(fps as u32);
                let mut s = State::default();
                let hop = (48000.0 / fps) as usize;
                let mut beats = Vec::new();
                for (k, chunk) in audio.chunks_exact(hop).enumerate() {
                    shared.push(chunk, 1, 48000);
                    if sp.advance(&shared, std::time::Instant::now(), 1.0 / fps, &mut s) {
                        beats.push((k + 1) as f64 / fps);
                    }
                }
                let r = house.score(&beats, 1.0 / fps, secs);
                assert!(r.hit == r.kicks && r.off == 0, "{house:?} at {fps} Hz: {r:?}");
            }
        }
    }

    #[test]
    fn band_split_matches_js() {
        // n = 15: bass 0..=2, mid 3..=8, high 9..=15, kick onset 0..=4 (JS Math.round(2.25) = 2, (8.25) = 8, (4.05) = 4)
        assert_eq!(round(15.0 * 0.15), 2);
        assert_eq!(round(15.0 * 0.55), 8);
        assert_eq!(round(15.0 * 0.27), 4);
        assert_eq!(round(2.5), 3);
    }
}
