// Shared audio analysis: imported by quickshell/AudioSource.qml (QML JS) and by web/app.js (<script>).
// Keep it plain JS with no module syntax so both runtimes can load it.

function rdCreateState(bands) {
    var s = { bands: bands, spec: [], prevRaw: [], riseAvg: 0, beatRise: 0, peak: 0.2, beat: 0, sinceBeat: 1, flow: 0,
              bass: 0, mid: 0, high: 0, level: 0 };
    for (var i = 0; i < bands; i++) { s.spec.push(0); s.prevRaw.push(0); }
    return s;
}

function rdAvg(a, from, to) {
    var t = 0;
    for (var i = from; i <= to; i++) t += a[i];
    return t / (to - from + 1);
}

// raw: array of `bands` values 0..1, dt: seconds since last call. Mutates and returns state.
function rdAnalyse(s, raw, dt) {
    dt = Math.min(Math.max(dt, 0.001), 0.1);
    var up = 1 - Math.exp(-dt * 30), down = 1 - Math.exp(-dt * 7);

    var rawMax = 0;
    for (var i = 0; i < s.bands; i++) rawMax = Math.max(rawMax, raw[i] || 0);
    // Auto gain: follow peaks quickly, forget them slowly, never divide by silence.
    var prevPeak = s.peak;
    s.peak = Math.max(rawMax, s.peak * Math.exp(-dt * 0.35), 0.08);

    for (var j = 0; j < s.bands; j++) {
        var v = Math.min((raw[j] || 0) / s.peak, 1);
        s.spec[j] += (v - s.spec[j]) * (v > s.spec[j] ? up : down);
    }

    var n = s.bands - 1;
    s.bass = rdAvg(s.spec, 0, Math.round(n * 0.15));
    s.mid = rdAvg(s.spec, Math.round(n * 0.15) + 1, Math.round(n * 0.55));
    s.high = rdAvg(s.spec, Math.round(n * 0.55) + 1, n);
    s.level = rdAvg(s.spec, 0, n);

    // Kick detection: the fastest rise of any single band in 40–240 Hz (per second, so frame-rate independent)
    // compared with its running average. Not the bass average: a kick's attack sweeps down from ~200 Hz and jumps
    // a band or two above where a bass note sits, while a saw bass rises evenly across all of them, so averaging
    // lets a loud off-beat bass outscore the kick. The rise uses last frame's gain, otherwise a loud kick raises the
    // gain in the same frame and hides its own rise. A 0.3 s refractory period keeps off-beat bass notes from
    // doubling the beat, but a clearly stronger rise inside it replaces the last beat, so the detector can't stay
    // locked on off-beats.
    var rise = 0;
    for (var b = 0; b <= Math.round(n * 0.27); b++) rise = Math.max(rise, (raw[b] || 0) - s.prevRaw[b]);
    rise = rise / dt / prevPeak;
    for (var c = 0; c < s.bands; c++) s.prevRaw[c] = raw[c] || 0;
    s.sinceBeat += dt;
    var onset = rise > s.riseAvg * 2.5 + 2.0;
    if (onset && (s.sinceBeat > 0.3 || rise > s.beatRise * 1.15)) {
        s.beat = 1;
        s.sinceBeat = 0;
        s.beatRise = rise;
    } else {
        s.beat *= Math.exp(-dt * 7);
    }
    s.riseAvg += (rise - s.riseAvg) * (1 - Math.exp(-dt * 1));

    s.flow += dt * (0.3 + 1.6 * s.level);
    return s;
}
