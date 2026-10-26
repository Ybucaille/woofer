// @name Drift
// @description A barely-there breeze moves through your wallpaper; the mids turn it into gusts
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
    for (int i = 0; i < 3; i++) {
        v += a * vnoise(p);
        p = p * 2.1 + vec2(1.7, 9.2);
        a *= 0.5;
    }
    return v;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 suv = fragCoord / iResolution;
    float aspect = iResolution.x / iResolution.y;
    float k = uIntensity;
    float t = uFlow * 0.25;
    vec2 p = suv * vec2(aspect, 1.0) * 2.2;

    // The wind blows mostly sideways: the field scrolls with it and leans horizontal.
    vec2 w = vec2(fbm(p + vec2(-t, 0.3 * t)), fbm(p + vec2(5.2 - t, 1.3))) - 0.5;
    float amp = 0.0025 + 0.012 * uMid * k;
    vec2 uv = 0.5 + (suv - 0.5) * 0.985 + w * vec2(1.4, 0.7) * amp;
    fragColor = vec4(wall(uv).rgb, 1.0);
}
