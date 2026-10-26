#!/usr/bin/env python3
"""Writes a 124 bpm kick / bass / hi-hat loop (same recipe as the web preview's demo beat) as a 48 kHz WAV."""
import math, random, struct, sys, wave

out = sys.argv[1] if len(sys.argv) > 1 else "beat.wav"
secs = float(sys.argv[2]) if len(sys.argv) > 2 else 60.0
rate, bpm = 48000, 124.0
beat = 60.0 / bpm
n = int(rate * secs)
buf = [0.0] * n
notes = [55.0, 55.0, 65.4, 49.0]
random.seed(1)
k = 0
while k * beat < secs:
    t0 = int(k * beat * rate)
    phase = 0.0
    for i in range(int(0.35 * rate)):  # kick: 150 → 40 Hz sweep, exponential decay
        if t0 + i >= n: break
        t = i / rate
        f = 150.0 * (40.0 / 150.0) ** min(t / 0.15, 1.0)
        phase += 2 * math.pi * f / rate
        buf[t0 + i] += math.sin(phase) * 0.001 ** (t / 0.35)
    b0, f = t0 + int(beat / 2 * rate), notes[(k // 4) % 4]
    for i in range(int(beat / 2 * rate)):  # off-beat saw bass
        if b0 + i >= n: break
        t = i / rate
        saw = 2.0 * ((f * t) % 1.0) - 1.0
        buf[b0 + i] += 0.25 * saw * min(t / 0.02, 1.0) * 0.001 ** (t / (beat / 2))
    for h in (0.25, 0.75):  # hats: short noise bursts
        h0 = t0 + int(h * beat * rate)
        for i in range(int(0.04 * rate)):
            if h0 + i >= n: break
            buf[h0 + i] += 0.08 * (random.random() * 2 - 1) * (1 - i / (0.04 * rate))
    k += 1
with wave.open(out, "wb") as w:
    w.setnchannels(2); w.setsampwidth(2); w.setframerate(rate)
    w.writeframes(b"".join(struct.pack("<hh", s, s) for s in (int(max(-1, min(1, v * 0.35)) * 32767) for v in buf)))
