// @name Heat
// @description Heat haze rising off your wallpaper, shimmering harder with the highs
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

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 suv = fragCoord / iResolution;
    float aspect = iResolution.x / iResolution.y;
    float k = uIntensity;
    float t = uFlow * 0.8;
    vec2 p = suv * vec2(aspect, 1.0);

    // Tall thin cells scrolling upward, like air rising off hot asphalt.
    float n1 = vnoise(vec2(p.x * 38.0, p.y * 16.0 - t * 3.0));
    float n2 = vnoise(vec2(p.x * 21.0 + 7.0, p.y * 9.0 - t * 2.2));
    vec2 w = vec2(n1 + 0.6 * n2 - 0.8, (n2 - 0.5) * 0.5);
    float rise = 0.35 + 0.65 * smoothstep(1.0, 0.0, suv.y);
    float amp = (0.001 + 0.006 * uHigh * k + 0.004 * uLevel * k) * rise;
    vec2 uv = 0.5 + (suv - 0.5) * 0.99 + w * amp;
    fragColor = vec4(wall(uv).rgb, 1.0);
}
