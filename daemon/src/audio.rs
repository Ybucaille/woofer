//! PipeWire capture of the default output's monitor, and a spectrum front-end that mimics the web preview's
//! WebAudio AnalyserNode (fftSize 4096, Blackman window, smoothing 0.5, -100..-30 dB → byte, max per band, squared)
//! so `raw` bands — and therefore everything downstream in analysis.rs — match the other hosts.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rustfft::num_complex::Complex32;
use realfft::RealToComplex;

use crate::analysis::{BANDS, State};

pub const FFT_SIZE: usize = 4096;
/// Analysis steps per second of captured audio, whatever the render rate: the kick detector sees the same frames at
/// `--fps 30`, at the idle rate and at 60 fps (and the same as the web preview at 60 fps).
pub const ANALYSIS_RATE: u32 = 60;
/// Room for FFT_SIZE samples plus the audio captured between two renders (up to 256 ms of it, so down to ~4 fps).
const RING: usize = 4 * FFT_SIZE;
/// A chunk louder than this (-60 dBFS) counts as sound for the idle logic.
const SOUND_THRESHOLD: f32 = 1e-3;
/// No buffer for this long means the sink is suspended (we are a passive node): treat as silence.
const STALE: Duration = Duration::from_millis(250);

struct Ring {
    buf: [f32; RING],
    pos: usize,
    /// Samples written since the start (the absolute clock analysis steps are placed on).
    written: u64,
    rate: u32,
    last_data: Option<Instant>,
}

pub struct Shared {
    ring: Mutex<Ring>,
    epoch: Instant,
    /// Milliseconds since `epoch` of the last chunk above SOUND_THRESHOLD (0 = never).
    last_sound_ms: AtomicU64,
    /// Set by the renderer while idling; the capture thread pings `wake` once when sound comes back.
    pub idle: AtomicBool,
    wake: Mutex<Option<Box<dyn Fn() + Send>>>,
}

impl Shared {
    pub fn new() -> Arc<Self> {
        Arc::new(Shared {
            ring: Mutex::new(Ring { buf: [0.0; RING], pos: 0, written: 0, rate: 48000, last_data: None }),
            epoch: Instant::now(),
            last_sound_ms: AtomicU64::new(0),
            idle: AtomicBool::new(false),
            wake: Mutex::new(None),
        })
    }

    pub fn set_waker(&self, f: impl Fn() + Send + 'static) {
        *self.wake.lock().unwrap() = Some(Box::new(f));
    }

    /// Time since the last audible chunk.
    pub fn quiet_for(&self, now: Instant) -> Duration {
        let ms = self.last_sound_ms.load(Ordering::Relaxed);
        if ms == 0 {
            return now - self.epoch;
        }
        now.saturating_duration_since(self.epoch + Duration::from_millis(ms))
    }

    pub(crate) fn push(&self, interleaved: &[f32], channels: usize, rate: u32) {
        let mut peak = 0.0f32;
        // try_lock: never block PipeWire's loop on the renderer; dropping one chunk is harmless.
        if let Ok(mut r) = self.ring.try_lock() {
            r.rate = rate;
            r.last_data = Some(Instant::now());
            let inv = 1.0 / channels as f32;
            for frame in interleaved.chunks_exact(channels) {
                let m = frame.iter().sum::<f32>() * inv;
                peak = peak.max(m.abs());
                let p = r.pos;
                r.buf[p] = m;
                r.pos = (p + 1) % RING;
            }
            r.written += (interleaved.len() / channels) as u64;
        } else {
            peak = interleaved.iter().fold(0.0, |a, v| a.max(v.abs()));
        }
        if peak > SOUND_THRESHOLD {
            let ms = (Instant::now() - self.epoch).as_millis() as u64;
            self.last_sound_ms.store(ms.max(1), Ordering::Relaxed);
            if self.idle.swap(false, Ordering::AcqRel)
                && let Some(w) = self.wake.lock().unwrap().as_ref() {
                    w();
                }
        }
    }
}

/// Spawns the capture thread. It retries every few seconds if PipeWire is absent or restarts.
pub fn spawn_capture(shared: Arc<Shared>, target: Option<String>) {
    std::thread::Builder::new()
        .name("woofer-audio".into())
        .spawn(move || {
            pipewire::init();
            loop {
                if let Err(e) = run_capture(&shared, target.as_deref()) {
                    log::warn!("audio capture: {e}");
                }
                std::thread::sleep(Duration::from_secs(3));
            }
        })
        .expect("spawn audio thread");
}

fn run_capture(shared: &Arc<Shared>, target: Option<&str>) -> Result<(), pipewire::Error> {
    use pipewire as pw;
    use pw::spa;

    let mainloop = pw::main_loop::MainLoopRc::new(None)?;
    let context = pw::context::ContextRc::new(&mainloop, None)?;
    let core = context.connect_rc(None)?;
    let ml = mainloop.clone();
    let _core_listener = core
        .add_listener_local()
        .error(move |id, _seq, res, msg| {
            log::warn!("pipewire error on {id}: {res} {msg}");
            if id == pw::core::PW_ID_CORE {
                ml.quit();
            }
        })
        .register();

    let mut props = pw::properties::properties! {
        *pw::keys::MEDIA_TYPE => "Audio",
        *pw::keys::MEDIA_CATEGORY => "Capture",
        *pw::keys::MEDIA_ROLE => "Music",
        *pw::keys::STREAM_CAPTURE_SINK => "true",
        // Passive: we never keep the sink awake, so silence with nothing playing costs no wakeups at all.
        *pw::keys::NODE_PASSIVE => "true",
        *pw::keys::NODE_LATENCY => "1024/48000",
        *pw::keys::NODE_NAME => "woofer",
        *pw::keys::APP_NAME => "woofer",
    };
    if let Some(t) = target {
        props.insert("target.object", t);
    }
    let stream = pw::stream::StreamRc::new(core, "woofer", props)?;

    struct Fmt {
        channels: usize,
        rate: u32,
        samples: Vec<f32>,
    }
    let sh = shared.clone();
    let _listener = stream
        .add_local_listener_with_user_data(Fmt { channels: 2, rate: 48000, samples: Vec::with_capacity(8192) })
        .param_changed(|_, fmt, id, param| {
            let Some(param) = param else { return };
            if id != spa::param::ParamType::Format.as_raw() {
                return;
            }
            let mut info = spa::param::audio::AudioInfoRaw::new();
            if info.parse(param).is_ok() {
                fmt.channels = (info.channels() as usize).max(1);
                fmt.rate = info.rate().max(1);
                log::info!("audio: capturing {} Hz, {} channel(s)", fmt.rate, fmt.channels);
            }
        })
        .process(move |stream, fmt| {
            let Some(mut buffer) = stream.dequeue_buffer() else { return };
            let datas = buffer.datas_mut();
            let Some(data) = datas.first_mut() else { return };
            let size = data.chunk().size() as usize;
            let offset = data.chunk().offset() as usize;
            let Some(bytes) = data.data() else { return };
            let end = (offset + size).min(bytes.len());
            let bytes = &bytes[offset.min(end)..end];
            fmt.samples.clear();
            fmt.samples.extend(bytes.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
            sh.push(&fmt.samples, fmt.channels, fmt.rate);
        })
        .register()?;

    let mut info = spa::param::audio::AudioInfoRaw::new();
    info.set_format(spa::param::audio::AudioFormat::F32LE);
    let obj = spa::pod::Object {
        type_: spa::utils::SpaTypes::ObjectParamFormat.as_raw(),
        id: spa::param::ParamType::EnumFormat.as_raw(),
        properties: info.into(),
    };
    let values: Vec<u8> = spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &spa::pod::Value::Object(obj),
    )
    .map_err(|_| pw::Error::CreationFailed)?
    .0
    .into_inner();
    let mut params = [spa::pod::Pod::from_bytes(&values).ok_or(pw::Error::CreationFailed)?];
    stream.connect(
        spa::utils::Direction::Input,
        None,
        pw::stream::StreamFlags::AUTOCONNECT | pw::stream::StreamFlags::MAP_BUFFERS,
        &mut params,
    )?;
    log::info!("audio: connected to PipeWire ({} monitor)", target.unwrap_or("default output"));
    mainloop.run();
    Ok(())
}

/// Turns captured audio into 16 raw bands, one set per analysis step, and feeds them to the analysis.
/// Lives on the render thread; allocates only in `new`.
pub struct Spectrum {
    fft: Arc<dyn RealToComplex<f32>>,
    input: Vec<f32>,
    buf: Vec<Complex32>,
    scratch: Vec<Complex32>,
    window: Vec<f32>,
    smooth: Vec<f32>,
    linear: Box<[f32; RING]>,
    bands: [(usize, usize); BANDS],
    raw: [f64; BANDS],
    rate: u32,
    silent: bool,
    /// Analysis steps per second of audio (ANALYSIS_RATE; tests vary it).
    steps: u32,
    /// Ring position (`Ring::written`) of the last analysis step.
    done: u64,
}

impl Spectrum {
    pub fn new() -> Self {
        Self::with_steps(ANALYSIS_RATE)
    }

    /// `steps` analysis steps per second of audio.
    pub fn with_steps(steps: u32) -> Self {
        // Real-input FFT (realfft packs it into a half-size rustfft): half the work of a complex transform.
        let fft = realfft::RealFftPlanner::<f32>::new().plan_fft_forward(FFT_SIZE);
        let scratch = fft.make_scratch_vec();
        let n = FFT_SIZE as f32;
        let window = (0..FFT_SIZE)
            .map(|i| {
                let x = std::f32::consts::TAU * i as f32 / n;
                0.42 - 0.5 * x.cos() + 0.08 * (2.0 * x).cos()
            })
            .collect();
        let mut s = Spectrum {
            input: fft.make_input_vec(),
            buf: fft.make_output_vec(),
            fft,
            scratch,
            window,
            smooth: vec![0.0; FFT_SIZE / 2],
            linear: Box::new([0.0; RING]),
            bands: [(0, 1); BANDS],
            raw: [0.0; BANDS],
            rate: 0,
            silent: true,
            steps,
            done: 0,
        };
        s.set_rate(48000);
        s
    }

    fn set_rate(&mut self, rate: u32) {
        self.rate = rate;
        let nyq = rate as f64 / 2.0;
        let (lo, hi) = (40.0f64, 14000.0f64);
        let len = (FFT_SIZE / 2) as f64;
        for (b, band) in self.bands.iter_mut().enumerate() {
            let f0 = lo * (hi / lo).powf(b as f64 / BANDS as f64);
            let f1 = lo * (hi / lo).powf((b + 1) as f64 / BANDS as f64);
            let i0 = (f0 / nyq * len).floor() as usize;
            let i1 = ((f1 / nyq * len).floor() as usize).max(i0 + 1);
            *band = (i0.min(FFT_SIZE / 2 - 1), i1.min(FFT_SIZE / 2));
        }
    }

    /// Brings `state` up to date with the captured audio: one analysis step per 1/ANALYSIS_RATE s of audio since
    /// the last call, each on the FFT_SIZE samples ending there. With no fresh audio, one silent step of `dt`
    /// (seconds since the last call). Returns whether a beat started.
    pub fn advance(&mut self, shared: &Shared, now: Instant, dt: f64, state: &mut State) -> bool {
        let (rate, written, hop, last, mut end, mut step);
        {
            let r = shared.ring.lock().unwrap();
            // No buffers (sink suspended), or only digital silence for a while: nothing to analyse.
            let fresh = r.last_data.is_some_and(|t| now.saturating_duration_since(t) < STALE)
                && shared.quiet_for(now) < STALE;
            if !fresh {
                drop(r);
                if !self.silent {
                    self.smooth.fill(0.0);
                    self.silent = true;
                }
                state.analyse(&[0.0; BANDS], dt);
                return false;
            }
            rate = r.rate;
            written = r.written;
            hop = (rate / self.steps).max(1) as u64;
            last = written / hop * hop;
            if last < FFT_SIZE as u64 {
                return false;
            }
            end = self.done + hop;
            step = hop as f64 / rate as f64;
            // Coming back from silence, a pause or a stall (or the ring no longer holds that window): start again
            // from the latest audio. That one step stands for the whole gap, which also keeps a jump in the bands
            // from looking like a fast rise.
            if self.silent || end + ((RING - FFT_SIZE) as u64) < written {
                end = last;
                step = step.max(dt);
            }
            if end > last {
                return false;
            }
            // Copy only what the pending steps read, oldest first, into the tail of `linear`.
            let n = (written - end) as usize + FFT_SIZE;
            let tail = &mut self.linear[RING - n..];
            if n <= r.pos {
                tail.copy_from_slice(&r.buf[r.pos - n..r.pos]);
            } else {
                let (a, b) = tail.split_at_mut(n - r.pos);
                a.copy_from_slice(&r.buf[RING - (n - r.pos)..]);
                b.copy_from_slice(&r.buf[..r.pos]);
            }
        }
        if rate != self.rate {
            self.set_rate(rate);
        }
        self.silent = false;
        let mut beat = false;
        while end <= last {
            let at = RING - (written - end) as usize - FFT_SIZE;
            self.bands_at(at);
            state.analyse(&self.raw, step);
            beat |= state.since_beat == 0.0;
            self.done = end;
            step = hop as f64 / rate as f64;
            end += hop;
        }
        beat
    }

    /// Fills `self.raw` from the FFT_SIZE samples at `self.linear[at..]`.
    fn bands_at(&mut self, at: usize) {
        for ((c, &x), &w) in self.input.iter_mut().zip(&self.linear[at..at + FFT_SIZE]).zip(&self.window) {
            *c = x * w;
        }
        // Only fails on wrong buffer lengths, which `new` rules out.
        let _ = self.fft.process_with_scratch(&mut self.input, &mut self.buf, &mut self.scratch);
        let inv_n = 1.0 / FFT_SIZE as f32;
        for (s, c) in self.smooth.iter_mut().zip(&self.buf) {
            *s = 0.5 * *s + 0.5 * c.norm() * inv_n;
        }
        // WebAudio getByteFrequencyData: minDecibels -100, maxDecibels -30.
        for (out, &(i0, i1)) in self.raw.iter_mut().zip(&self.bands) {
            let m = self.smooth[i0..i1].iter().fold(0.0f32, |m, &v| m.max(v));
            let db = 20.0 * m.max(1e-12).log10();
            let byte = ((255.0 / 70.0) * (db + 100.0)).floor().clamp(0.0, 255.0);
            *out = (byte as f64 / 255.0).powi(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::tests::{BEAT, House};

    #[test]
    fn sine_lands_in_its_band() {
        let shared = Shared::new();
        // 100 Hz at -6 dBFS, stereo.
        let mut v = Vec::new();
        for i in 0..2 * FFT_SIZE {
            let s = 0.5 * (std::f32::consts::TAU * 100.0 * i as f32 / 48000.0).sin();
            v.push(s);
            v.push(s);
        }
        shared.push(&v, 2, 48000);
        let mut sp = Spectrum::new();
        let mut state = State::default();
        sp.advance(&shared, Instant::now(), 1.0 / 60.0, &mut state);
        let raw = sp.raw;
        let best = (0..BANDS).max_by(|&a, &b| raw[a].total_cmp(&raw[b])).unwrap();
        // 40·(350)^(b/16) contains 100 Hz for b = 2.
        assert_eq!(best, 2, "{raw:?}");
        assert!(raw[best] > 0.5);
        assert!(raw[12] < 0.05);
        assert!(shared.quiet_for(Instant::now()) < Duration::from_secs(1));
    }

    /// The daemon's real path: audio arrives in 1024-frame PipeWire quanta in real time and we render at `fps`.
    /// Returns (beat times as seen by the renderer, analysis steps run).
    fn render(audio: &[f32], fps: f64) -> (Vec<f64>, usize) {
        const QUANTUM: usize = 1024;
        let shared = Shared::new();
        let mut sp = Spectrum::new();
        let mut state = State::default();
        let (mut sent, mut beats, mut steps) = (0, Vec::new(), 0);
        let mut last_done = 0;
        for k in 1.. {
            let t = k as f64 / fps;
            // Every quantum fully captured by time t has been delivered.
            let due = (t * 48000.0) as usize / QUANTUM * QUANTUM;
            if due + QUANTUM > audio.len() {
                break;
            }
            while sent + QUANTUM <= due {
                shared.push(&audio[sent..sent + QUANTUM], 1, 48000);
                sent += QUANTUM;
            }
            if sp.advance(&shared, Instant::now(), 1.0 / fps, &mut state) {
                beats.push(t);
            }
            steps += ((sp.done - last_done) / 800) as usize;
            last_done = sp.done;
        }
        (beats, steps)
    }

    #[test]
    fn kicks_over_a_strong_bassline_at_any_render_rate() {
        // Off-beat bass at 0.65 of the kick, 30 ms attack, from the start. Before analysis ran on the audio clock,
        // --fps 30 missed up to 1 kick in 2 here and fired on the bass instead.
        for house in House::strong_cases() {
            let audio = house.render(45.0 * BEAT);
            let mut seen = Vec::new();
            for fps in [60.0, 30.0, 10.0, 144.0] {
                let (beats, steps) = render(&audio, fps);
                // Shown at the next render after the quantum holding the kick's window arrived.
                let delay = 1.0 / fps + 1024.0 / 48000.0;
                let r = house.score(&beats, delay, audio.len() as f64 / 48000.0);
                assert!(r.hit == r.kicks && r.off == 0, "{house:?} at {fps} fps: {r:?}");
                // Kick to visible beat: the analysis window's lag (~30 ms) plus at most a frame and a quantum.
                assert!(r.latency_ms < 40.0 + 1000.0 * delay, "{house:?} at {fps} fps: {r:?}");
                // Same detector input whatever the render rate: ~60 steps per second of audio.
                let secs = audio.len() as f64 / 48000.0;
                assert!((steps as f64 - secs * 60.0).abs() < 8.0, "{steps} steps at {fps} fps");
                seen.push((fps, r));
            }
            eprintln!("{house:?}: {seen:?}");
        }
    }
}
