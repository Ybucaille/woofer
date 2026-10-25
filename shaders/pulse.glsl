// @name Pulse
// @description Soft concentric rings that breathe with the bass — the reference preset
// @author pivuche

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution) / iResolution.y;
    float r = length(uv);
    float a = atan(uv.y, uv.x);

    float k = uIntensity;
    float rings = sin(r * 18.0 - uFlow * 2.0 + specAt(fract(a / 6.2831 + 0.5)) * 2.0 * k);
    rings = smoothstep(0.75, 1.0, rings);

    float halo = exp(-r * (3.0 - 1.5 * uBass * k)) * (0.25 + 0.75 * uBeat * k);
    vec3 col = mix(uColor1.rgb, uColor2.rgb, 0.5 + 0.5 * sin(a * 2.0 + uFlow * 0.3));
    col = col * rings * (0.25 + 0.5 * uLevel * k) + uColor3.rgb * halo * 0.6;

    float alpha = clamp(max(rings * 0.35, halo), 0.0, 0.85) * smoothstep(1.3, 0.2, r);
    fragColor = vec4(col * alpha, alpha);
}
