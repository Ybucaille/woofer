// @name Nebula
// @description Slow cosmic clouds folding into themselves; they bloom on every kick while stars catch the highs
// @author pivuche

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float vnoise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash21(i), hash21(i + vec2(1.0, 0.0)), u.x),
               mix(hash21(i + vec2(0.0, 1.0)), hash21(i + vec2(1.0, 1.0)), u.x), u.y);
}

float fbm(vec2 p) {
    float v = 0.0, a = 0.5;
    mat2 m = mat2(0.8, 0.6, -0.6, 0.8);
    for (int i = 0; i < 5; i++) {
        v += a * vnoise(p);
        p = m * p * 2.02 + vec2(3.1, 1.7);
        a *= 0.5;
    }
    return v;
}

float stars(vec2 uv, float scale, float seed) {
    vec2 g = uv * scale;
    vec2 id = floor(g);
    float h = hash21(id + seed);
    vec2 pos = vec2(hash21(id + seed + 7.0), hash21(id + seed + 13.0)) * 0.8 + 0.1;
    float d = length(fract(g) - pos);
    float tw = 0.6 + 0.4 * sin(iTime * (1.0 + 2.0 * h) + h * 40.0);
    return step(0.9, h) * smoothstep(0.08, 0.0, d) * tw * (h - 0.9) * 10.0;
}

vec3 sat(vec3 c) {
    return clamp(mix(vec3(dot(c, vec3(0.299, 0.587, 0.114))), c, 2.0), 0.0, 1.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution) / iResolution.y;
    float k = uIntensity;
    float t = uFlow * 0.05 + iTime * 0.006;
    vec2 p = uv * 1.1 + vec2(0.0, t * 0.5);

    // Two levels of domain warping (iq).
    vec2 q = vec2(fbm(p + vec2(0.0, t)), fbm(p + vec2(5.2, 1.3) - t));
    vec2 r = vec2(fbm(p + 2.0 * q + vec2(1.7, 9.2) + 0.5 * t),
                  fbm(p + 2.0 * q + vec2(8.3, 2.8) - 0.4 * t));
    float f = fbm(p + 2.2 * r + 0.15 * uBass * k);

    vec3 c1 = sat(uColor1.rgb), c2 = sat(uColor2.rgb), c3 = sat(uColor3.rgb);
    float dens = smoothstep(0.3, 0.72, f);
    vec3 hue = mix(c1, c2, smoothstep(0.25, 0.75, q.x));
    hue = mix(hue, c3, smoothstep(0.45, 0.8, r.y));
    // Dark dust lanes cut through the glowing gas.
    float dust = smoothstep(0.25, 0.6, r.x);
    vec3 col = hue * (0.05 + 2.4 * pow(dens, 1.5)) * mix(0.3, 1.0, dust);

    // Bright cores bloom on kicks.
    float core = pow(smoothstep(0.45, 0.95, f), 3.0) * 1.6;
    col += mix(c3, vec3(1.0), 0.3) * core * dust * (0.3 + 1.0 * uBeat * k + 0.4 * uLevel * k);
    col *= 0.85 + 0.25 * uLevel * k;

    float s = stars(uv, 60.0, 0.0) + stars(uv + 3.7, 140.0, 5.0) * 0.6;
    col += vec3(0.9, 0.92, 1.0) * s * (0.8 + 1.2 * uHigh * k) * (1.0 - dens * 0.8);

    col *= 1.0 - 0.45 * dot(uv, uv);
    col = col / (1.0 + col * 0.5);
    col = max(mix(vec3(dot(col, vec3(0.299, 0.587, 0.114))), col, 1.3), 0.0);
    fragColor = vec4(col, 1.0);
}
