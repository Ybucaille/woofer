// @name Bokeh
// @description Out-of-focus city lights drifting over your wallpaper; each one breathes with its own slice of the spectrum
// @author pivuche

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

vec3 sat(vec3 c) {
    return clamp(mix(vec3(dot(c, vec3(0.299, 0.587, 0.114))), c, 1.8), 0.0, 1.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution) / iResolution.y;
    float k = uIntensity;
    float t = uFlow * 0.05 + iTime * 0.012;
    vec3 c1 = sat(uColor1.rgb), c2 = sat(uColor2.rgb), c3 = sat(uColor3.rgb);

    vec3 col = vec3(0.0);
    float cover = 0.0;
    for (int l = 0; l < 3; l++) {
        float fl = float(l);
        float scale = 2.2 + fl * 2.4;                  // near layer: big and soft, far: small
        float blur = mix(0.35, 0.12, fl / 2.0);
        vec2 p = uv * scale + vec2(fl * 7.3, t * (1.0 + fl * 0.5) + fl * 3.1);
        vec2 id0 = floor(p);
        for (int j = -1; j <= 1; j++)
        for (int i = -1; i <= 1; i++) {
            vec2 id = id0 + vec2(float(i), float(j));
            float h = hash21(id + fl * 19.0);
            if (h < 0.45) continue;
            float h2 = hash21(id + fl * 19.0 + 5.0);
            vec2 o = vec2(hash21(id + 11.0), hash21(id + 23.0));
            o += 0.15 * vec2(sin(t * 3.0 + h * 30.0), cos(t * 2.3 + h2 * 30.0));
            float band = spec(int(h2 * 15.99));
            float r = (0.22 + 0.2 * h2) * (1.0 + (0.12 * band + 0.08 * uBeat) * k);
            float d = length(p - id - o);
            float disc = smoothstep(r, r - r * blur, d);
            float rim = smoothstep(r - r * blur * 0.2, r - r * blur * 0.7, d) * disc;
            float glow = (0.3 + 0.6 * band * k) * mix(0.55, 1.0, fl / 2.0) * (0.6 + 0.4 * sin(iTime * 0.5 + h * 50.0));
            float v = (disc * 0.75 + rim * 0.5) * glow;
            vec3 c = h2 < 0.45 ? c1 : (h2 < 0.8 ? c2 : c3);
            col += c * v;
            cover += v;
        }
    }
    col *= 1.0 + 0.3 * uBeat * k;
    cover *= 1.0 + 0.3 * uBeat * k;

    float a = clamp(cover, 0.0, 0.9);
    col = min(col, vec3(a));
    // A faint dusk tint at the bottom gives the lights something to sit in.
    float haze = 0.18 * smoothstep(0.3, -0.6, uv.y);
    fragColor = vec4(col + c1 * 0.02 * haze, a + haze * (1.0 - a));
}
