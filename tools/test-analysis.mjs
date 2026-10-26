#!/usr/bin/env node
// Checks the shared kick detector (quickshell/analysis.js) at 60 and 30 Hz analysis rates, on synthetic band frames
// and on synthetic audio through a WebAudio AnalyserNode-like front end. Mirrors the tests in daemon/src/analysis.rs.
// Usage: node tools/test-analysis.mjs
// Known limit: in the idealised band frames (no analysis window), an off-beat bass note at ~80 % of the kick
// (offAmp 0.6) still beats a kick sampled 30 ms late, so at 30 Hz it can double the beat; that case runs at 60 Hz.
// At 30 Hz the first kick after a bass-only intro can be lost the same way; the detector relocks on the next one.
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const ctx = {};
vm.runInNewContext(readFileSync(join(root, "quickshell", "analysis.js"), "utf8"), ctx);

const BEAT = 60 / 124, BANDS = 16, RATE = 48000, N = 4096;
// Off-beat bass at 0.65 of the kick's 0.75, 30 ms attack.
const STRONG = { offAmp: 0.65 * 0.75, offAttack: 0.03 };

// Bass bins 0..2 over time: sustained bass, optional kicks on the beat, off-beat bass notes half a beat later.
function frame(t, { kicks, offbeats, kickFrom = 0, offAmp = 0.6, offAttack = 0.02, silent = false }) {
  if (silent) return new Array(BANDS).fill(0);
  const phase = t % BEAT;
  let bass = 0.25;
  if (kicks && t >= kickFrom) bass += 0.75 * Math.exp(-phase / 0.08);
  if (offbeats) {
    const p = (t + BEAT / 2) % BEAT;
    bass += offAmp * (p < offAttack ? p / offAttack : Math.exp(-(p - offAttack) / 0.1));
  }
  const raw = new Array(BANDS).fill(0.05);
  for (let i = 0; i <= 2; i++) raw[i] = bass;
  return raw;
}

function run(fps, seconds, opts) {
  const s = ctx.rdCreateState(BANDS);
  const beats = [];
  for (let k = 0; k < seconds * fps; k++) {
    const t = k / fps;
    const before = s.sinceBeat;
    ctx.rdAnalyse(s, frame(t, opts), 1 / fps);
    if (s.sinceBeat === 0 && before > 0) beats.push(t);
  }
  return beats;
}

// A house loop as audio (48 kHz mono): a pitch-swept kick on every beat, a bass note on every off-beat.
// Same recipe as `House` in daemon/src/analysis.rs.
function house(secs, { k0, k1, sweep, note, offAmp, attack, sine }) {
  const n = Math.floor(secs * RATE), buf = new Float64Array(n);
  for (let k = 0; k * BEAT < secs; k++) {
    const t0 = Math.floor(k * BEAT * RATE);
    let ph = 0;
    for (let i = 0; i < Math.floor(0.35 * RATE) && t0 + i < n; i++) {
      const t = i / RATE, f = k0 * Math.pow(k1 / k0, Math.min(t / sweep, 1));
      ph += 2 * Math.PI * f / RATE;
      buf[t0 + i] += Math.sin(ph) * Math.pow(0.001, t / 0.35);
    }
    const b0 = t0 + Math.floor(BEAT / 2 * RATE);
    for (let i = 0; i < Math.floor(BEAT / 2 * RATE) && b0 + i < n; i++) {
      const t = i / RATE;
      const saw = 2 * ((note * t) % 1) - 1, sn = Math.sin(2 * Math.PI * note * t);
      const env = Math.min(t / attack, 1) * Math.pow(0.01, t / (BEAT / 2));
      buf[b0 + i] += offAmp * env * ((1 - sine) * saw + sine * sn);
    }
  }
  return buf.map((v) => v * 0.4);
}
const H = { k0: 150, k1: 45, sweep: 0.12, note: 55, offAmp: 0.65, attack: 0.03, sine: 0 };
const HOUSES = [{ ...H, note: 41.2 }, H, { ...H, note: 73.4, sine: 1 }, { ...H, k0: 220, k1: 50, sweep: 0.04, sine: 1 }];

// AnalyserNode-like front end (fftSize 4096, Blackman, smoothing 0.5, -100..-30 dB → byte, max per band, squared),
// the same as web/app.js and the daemon's audio.rs.
const WIN = Float64Array.from({ length: N }, (_, i) => {
  const x = 2 * Math.PI * i / N;
  return 0.42 - 0.5 * Math.cos(x) + 0.08 * Math.cos(2 * x);
});
const BAND_BINS = Array.from({ length: BANDS }, (_, b) => {
  const f0 = 40 * Math.pow(350, b / BANDS), f1 = 40 * Math.pow(350, (b + 1) / BANDS);
  const i0 = Math.floor(f0 / (RATE / 2) * (N / 2));
  return [i0, Math.max(i0 + 1, Math.floor(f1 / (RATE / 2) * (N / 2)))];
});
function magnitudes(x) { // iterative radix-2 FFT, |X[k]| for k < N/2
  const re = Float64Array.from(x), im = new Float64Array(N);
  for (let i = 1, j = 0; i < N; i++) {
    let b = N >> 1;
    for (; j & b; b >>= 1) j ^= b;
    j ^= b;
    if (i < j) [re[i], re[j]] = [re[j], re[i]];
  }
  for (let len = 2; len <= N; len <<= 1) {
    const a = -2 * Math.PI / len;
    for (let i = 0; i < N; i += len) {
      for (let k = 0; k < len / 2; k++) {
        const cr = Math.cos(a * k), ci = Math.sin(a * k), p = i + k, q = p + len / 2;
        const vr = re[q] * cr - im[q] * ci, vi = re[q] * ci + im[q] * cr;
        re[q] = re[p] - vr; im[q] = im[p] - vi; re[p] += vr; im[p] += vi;
      }
    }
  }
  return Float64Array.from({ length: N / 2 }, (_, k) => Math.hypot(re[k], im[k]));
}
function runAudio(audio, fps) {
  const s = ctx.rdCreateState(BANDS), smooth = new Float64Array(N / 2), beats = [], hop = RATE / fps;
  for (let k = 1; k * hop <= audio.length; k++) {
    const end = k * hop, x = new Float64Array(N);
    for (let i = 0; i < N; i++) x[i] = (end - N + i >= 0 ? audio[end - N + i] : 0) * WIN[i];
    const m = magnitudes(x);
    for (let i = 0; i < N / 2; i++) smooth[i] = 0.5 * smooth[i] + 0.5 * m[i] / N;
    const raw = BAND_BINS.map(([i0, i1]) => {
      let mx = 0;
      for (let i = i0; i < i1; i++) mx = Math.max(mx, smooth[i]);
      const byte = Math.min(255, Math.max(0, Math.floor(255 / 70 * (20 * Math.log10(Math.max(mx, 1e-12)) + 100))));
      return (byte / 255) ** 2;
    });
    const before = s.sinceBeat;
    ctx.rdAnalyse(s, raw, 1 / fps);
    if (s.sinceBeat === 0 && before > 0) beats.push(end / RATE);
  }
  return beats;
}
// A kick is caught by a beat within 80 ms plus one frame; any other beat is off. Starts after 1 s: the first kick
// plays before a full analysis window of audio exists, and the detector may take the bass note after it before it
// locks on the next kick.
function score(beats, fps, secs) {
  const tol = 0.08 + 1 / fps, kicks = [];
  for (let k = 0; k * BEAT < secs; k++) if (k * BEAT >= 1) kicks.push(k * BEAT);
  beats = beats.filter((b) => b >= kicks[0] - 0.01);
  const near = (b, k) => b >= k - 0.01 && b < k + tol;
  const lat = kicks.map((k) => beats.find((b) => near(b, k))).map((b, i) => b - kicks[i]).filter((l) => l >= -1);
  lat.sort((a, b) => a - b);
  return { kicks: kicks.length, hit: lat.length, off: beats.filter((b) => !kicks.some((k) => near(b, k))).length,
    ms: Math.round(lat[lat.length >> 1] * 1000) };
}

let failed = 0;
function check(name, ok, detail) {
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? `  (${detail})` : ""}`);
  if (!ok) failed++;
}

for (const fps of [60, 30]) {
  const DT = 1 / fps;
  // On the frame of the kick or within the next 50 ms.
  const onBeat = (t) => { const p = t % BEAT; return p < Math.max(3 * DT, 0.05) || p > BEAT - DT; };
  const at = `@${fps} Hz`;
  const basslines = fps === 60 ? [["", {}], [" (strong off-beat bass)", STRONG]] : [[" (strong off-beat bass)", STRONG]];

  const silence = run(fps, 3, { silent: true });
  check(`${at} silence gives no beat`, silence.length === 0, `${silence.length} beats`);

  const kicks = 20;
  for (const [label, bass] of basslines) {
    const both = run(fps, kicks * BEAT, { kicks: true, offbeats: true, ...bass });
    const late = both.filter((t) => !onBeat(t));
    check(`${at} kicks over a bassline from the start${label}: one beat per kick`,
      both.length >= kicks - 1 && both.length <= kicks, `${both.length}/${kicks}`);
    check(`${at} never locks onto the off-beat notes${label}`, late.length === 0, `${late.length} off-beat`);
  }

  // Kicks enter on a beat after 4 bars of bassline (t = 8 beats ≈ 3.87 s).
  const entry = 8 * BEAT;
  for (const [label, bass] of basslines) {
    const delayed = run(fps, entry + 10 * BEAT, { kicks: true, offbeats: true, kickFrom: entry - 1e-6, ...bass });
    // At 30 Hz the first kick may be seen 30 ms late, barely above the bass it follows; it must relock a beat later.
    const relock = fps === 60 ? 0 : BEAT;
    const after = delayed.filter((t) => t >= entry + relock - DT);
    check(`${at} ${relock ? "second" : "first"} kick after a bassline is caught${label}`,
      after.length > 0 && after[0] - entry - relock < Math.max(3 * DT, 0.05),
      after.length ? `first at +${((after[0] - entry) * 1000).toFixed(0)} ms` : "none");
    check(`${at} then one beat per kick, on the beat${label}`, after.length >= 9 - relock / BEAT && after.every(onBeat),
      `${after.length}/${10 - relock / BEAT}, ${after.filter((t) => !onBeat(t)).length} off-beat`);
  }

  // Audio with an off-beat bass at 0.65 of the kick, 30 ms attack: saw at 41 and 55 Hz, sine at 73 Hz, fast-sweep kick.
  const secs = 45 * BEAT;
  for (const h of HOUSES) {
    const r = score(runAudio(house(secs, h), fps), fps, secs);
    const name = `${h.sine ? "sine" : "saw"} ${h.note} Hz bass, kick ${h.k0}→${h.k1} Hz`;
    check(`${at} audio, ${name}: every kick, no off-beat`, r.hit === r.kicks && r.off === 0,
      `${r.hit}/${r.kicks}, ${r.off} off, median ${r.ms} ms after the kick`);
  }
}

process.exit(failed ? 1 : 0);
