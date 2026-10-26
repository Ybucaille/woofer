// @name Ribbon
// @description A single glowing silk ribbon twisting across the screen, its waves drawn by the spectrum
// @author pivuche

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution) / iResolution.y;
    float aspect = iResolution.x / iResolution.y;
    float k = uIntensity;
    float t = uFlow * 0.35 + iTime * 0.05;
    float px = 1.0 / iResolution.y;
    float xn = uv.x / aspect + 0.5;              // 0..1 across the screen
    float env = smoothstep(0.0, 0.25, xn) * smoothstep(1.0, 0.75, xn);

    // Spectrum drives the amplitude locally along x (mirrored so bass sits in the middle).
    // Squared distance (no cusp in the middle) and a small blur keep the silk smooth.
    float sx = (xn - 0.5) * (xn - 0.5) * 4.0;
    float sp = (specAt(sx - 0.1) + 2.0 * specAt(sx) + specAt(sx + 0.1)) * 0.25;
    float amp = (0.09 + 0.12 * sp * k + 0.04 * uBeat * k) * env;

    vec3 col = vec3(0.0);
    float glow = 0.0;
    for (int i = 0; i < 18; i++) {
        float fi = float(i) / 17.0;
        float tw = (fi - 0.5) * 2.0;               // -1..1 across the ribbon width
        float phase = t + tw * (0.9 + 0.4 * sin(t * 0.3));
        float y = amp * (sin(uv.x * 2.2 + phase) + 0.45 * sin(uv.x * 4.7 - phase * 1.3 + 1.0))
                + 0.05 * tw * cos(uv.x * 1.3 + t * 0.7) - 0.02;
        float d = abs(uv.y - y);
        float line = px * 1.4 / (d + px * 1.4);
        line *= line;
        float soft = exp(-d * 28.0) * 0.06;
        vec3 c = mix(mix(uColor1.rgb, uColor2.rgb, smoothstep(0.1, 0.55, xn)), uColor3.rgb, smoothstep(0.5, 0.95, xn));
        c = mix(c, uColor3.rgb, 0.35 * abs(tw));
        col += c * (line * 0.55 + soft);
        glow += line * 0.55 + soft;
    }
    float bright = (0.75 + 0.5 * uLevel * k) * mix(0.15, 1.0, env);
    col *= bright;
    glow *= bright;

    float a = clamp(glow, 0.0, 0.95);
    col = min(col, vec3(a));
    fragColor = vec4(col, a);
}
