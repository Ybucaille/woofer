// @name Glitch
// @description Your wallpaper, untouched until a kick lands: a brief RGB split and a few slipped slices, then calm
// @author pivuche

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 suv = fragCoord / iResolution;
    float k = uIntensity;
    // Only the first instants after a kick glitch; it is fully clean well before the next one.
    float g = uBeat * uBeat * uBeat * k;
    float seed = floor(iTime * 14.0);

    // A few horizontal slices slip sideways.
    float band = floor(suv.y * 28.0);
    float hb = hash21(vec2(band, seed));
    float slip = step(0.86, hash21(vec2(band * 1.7, seed + 3.0))) * (hb - 0.5) * 0.04 * g;
    vec2 uv = vec2(suv.x + slip, suv.y);

    float ca = (0.003 + 0.003 * uBass) * g;
    vec3 col = vec3(wall(uv + vec2(ca, 0.0)).r, wall(uv).g, wall(uv - vec2(ca, 0.0)).b);

    // Faint scanlines only while it glitches.
    col *= 1.0 - 0.05 * g * step(0.5, fract(fragCoord.y * 0.5));
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
