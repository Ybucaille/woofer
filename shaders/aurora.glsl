// @name Aurora
// @description Northern-light curtains drifting over your wallpaper, rippling and brightening with the mids
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
    for (int i = 0; i < 4; i++) {
        v += a * vnoise(p);
        p = p * 2.03 + vec2(1.7, 9.2);
        a *= 0.5;
    }
    return v;
}

vec3 sat(vec3 c) {
    return clamp(mix(vec3(dot(c, vec3(0.299, 0.587, 0.114))), c, 1.6), 0.0, 1.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.y;
    uv.x -= 0.5 * iResolution.x / iResolution.y;
    float k = uIntensity;
    float t = uFlow * 0.12 + iTime * 0.015;
    float mid = uMid * k;
    // Theme colours are usually pastel: push their saturation so the light reads as light.
    vec3 c1 = sat(uColor1.rgb), c2 = sat(uColor2.rgb), c3 = sat(uColor3.rgb);

    vec3 col = vec3(0.0);
    for (int l = 0; l < 3; l++) {
        float fl = float(l);
        float depth = 1.0 - fl * 0.25;
        float x = uv.x * (0.8 + fl * 0.3) + fl * 4.7;
        // Base line of the curtain: big slow folds, plus ripples that grow with the mids.
        float h = 0.48 + fl * 0.1
                + 0.15 * sin(x * 1.1 + t * 0.7 + fl * 2.3)
                + 0.2 * (fbm(vec2(x * 0.8 + t * 0.5, fl * 3.1)) - 0.5)
                + (0.015 + 0.03 * mid) * sin(x * 6.0 + t * 6.0 + fl * 2.0);
        float d = uv.y - h;
        float edge = smoothstep(-0.008, 0.004, d);
        // Curtains come and go along the sky instead of spanning the whole width.
        float gap = vnoise(vec2(x * 1.1 - t * 0.4, fl * 5.0));
        gap = 0.15 + 1.6 * gap * gap;
        float xr = x + fbm(vec2(x * 1.5, t * 0.7 + fl)) * 0.2;
        float rays = 0.6 * vnoise(vec2(xr * 40.0, t * 0.8 + fl * 11.0)) + 0.4 * vnoise(vec2(xr * 110.0, t * 1.3 + fl * 7.0));
        // Each ray has its own length, so the top of the curtain is ragged.
        float rise = exp(-max(d, 0.0) * mix(11.0, 3.0, rays) * (1.0 - 0.35 * mid));
        rays = 0.3 + 0.7 * rays * rays;
        float band = edge * rise * rays * gap * depth;
        band *= 1.0 + 0.8 * k * specAt(fract(x * 0.13 + 0.5));
        vec3 c = mix(c1, c2, smoothstep(0.03, 0.25, d));
        c = mix(c, c3, smoothstep(0.15, 0.4, d));
        col += c * band * (1.0 + 1.2 * exp(-max(d, 0.0) * 40.0));
    }
    col *= 1.2 + 0.5 * mid + 0.3 * uBeat * k;
    col = col / (1.0 + col * 0.45);

    float a = clamp(max(col.r, max(col.g, col.b)), 0.0, 0.92);
    col = min(col, vec3(a));
    // Darken the sky a little so the curtains read on bright wallpapers.
    float dim = 0.32 * smoothstep(0.15, 1.0, uv.y);
    fragColor = vec4(col, a + dim * (1.0 - a));
}
