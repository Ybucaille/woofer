#!/usr/bin/env node
// Checks the shared kick detector (quickshell/analysis.js) on synthetic band frames.
// Usage: node tools/test-analysis.mjs
// Known limit: an off-beat bass note louder than ~80 % of the kick (offAmp > 0.6 here) can still steal the first
// beat after a bass-only intro; the detector then relocks on the kick within a beat.
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const ctx = {};
vm.runInNewContext(readFileSync(join(root, "quickshell", "analysis.js"), "utf8"), ctx);

const FPS = 60, DT = 1 / FPS, BEAT = 60 / 124, BANDS = 16;

// Bass bins 0..2 over time: sustained bass, optional kicks on the beat, off-beat bass notes half a beat later.
function frame(t, { kicks, offbeats, kickFrom = 0, offAmp = 0.6, silent = false }) {
  if (silent) return new Array(BANDS).fill(0);
  const phase = t % BEAT;
  let bass = 0.25;
  if (kicks && t >= kickFrom) bass += 0.75 * Math.exp(-phase / 0.08);
  if (offbeats) {
    const p = (t + BEAT / 2) % BEAT;
    bass += offAmp * (p < 0.02 ? p / 0.02 : Math.exp(-(p - 0.02) / 0.1));
  }
  const raw = new Array(BANDS).fill(0.05);
  for (let i = 0; i <= 2; i++) raw[i] = bass;
  return raw;
}

function run(seconds, opts) {
  const s = ctx.rdCreateState(BANDS);
  const beats = [];
  for (let k = 0; k < seconds * FPS; k++) {
    const t = k * DT;
    const before = s.sinceBeat;
    ctx.rdAnalyse(s, frame(t, opts), DT);
    if (s.sinceBeat === 0 && before > 0) beats.push(t);
  }
  return beats;
}

let failed = 0;
function check(name, ok, detail) {
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? `  (${detail})` : ""}`);
  if (!ok) failed++;
}
const onBeat = (t) => { const p = t % BEAT; return p < 3 * DT || p > BEAT - DT; };

const silence = run(3, { silent: true });
check("silence gives no beat", silence.length === 0, `${silence.length} beats`);

const kicks = 20;
const both = run(kicks * BEAT, { kicks: true, offbeats: true });
const late = both.filter((t) => !onBeat(t));
check("kicks over a bassline from the start: one beat per kick", both.length >= kicks - 1 && both.length <= kicks,
  `${both.length}/${kicks}`);
check("never locks onto the off-beat notes", late.length === 0, `${late.length} off-beat`);

// Kicks enter on a beat after 4 bars of bassline (t = 8 beats ≈ 3.87 s).
const entry = 8 * BEAT;
const delayed = run(entry + 10 * BEAT, { kicks: true, offbeats: true, kickFrom: entry - 1e-6 });
const after = delayed.filter((t) => t >= entry - DT);
check("first kick after a bassline is caught", after.length > 0 && after[0] - entry < 3 * DT,
  after.length ? `first at +${((after[0] - entry) * 1000).toFixed(0)} ms` : "none");
check("then one beat per kick, on the beat", after.length >= 9 && after.every(onBeat),
  `${after.length}/10, ${after.filter((t) => !onBeat(t)).length} off-beat`);

process.exit(failed ? 1 : 0);
