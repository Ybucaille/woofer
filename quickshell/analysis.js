// Shared audio analysis: imported by quickshell/AudioSource.qml (QML JS) and by web/app.js (<script>).
// Keep it plain JS with no module syntax so both runtimes can load it.

function rdCreateState(bands) {
    var s = { bands: bands, spec: [], prevBass: 0, riseAvg: 0, peak: 0.2, beat: 0, sinceBeat: 1, flow: 0,
              bass: 0, mid: 0, high: 0, level: 0 };
    for (var i = 0; i < bands; i++) s.spec.push(0);
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

    // Kick detection: how fast the bass rises (per second, so frame-rate independent) compared with its
    // running average. A level threshold missed most kicks over a sustained bassline. The 0.3 s refractory
    // period keeps off-beat bass notes from doubling the beat (fine up to ~200 bpm).
    var bassNow = rdAvg(raw, 0, Math.round(n * 0.15)) / s.peak;
    var rise = Math.max(0, bassNow - s.prevBass) / dt;
    s.prevBass = bassNow;
    s.sinceBeat += dt;
    if (rise > s.riseAvg * 2.5 + 2.0 && s.sinceBeat > 0.3) {
        s.beat = 1;
        s.sinceBeat = 0;
    } else {
        s.beat *= Math.exp(-dt * 7);
    }
    s.riseAvg += (rise - s.riseAvg) * (1 - Math.exp(-dt * 1));

    s.flow += dt * (0.3 + 1.6 * s.level);
    return s;
}
