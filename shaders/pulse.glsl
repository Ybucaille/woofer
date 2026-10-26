// @name Pulse
// @description Soft concentric rings that breathe with the bass — the reference preset
// @author pivuche

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution) / iResolution.y;
    float r = length(uv);
    float a = atan(uv.y, uv.x);
    float k = uIntensity;

    // Mirror the angle so the spectrum wraps around the rings without a seam.
    float s = specAt(abs(a) / 3.14159265);
    float rings = smoothstep(0.8, 1.0, sin(r * 18.0 - uFlow * 2.0 + s * 2.0 * k));
    float ringA = rings * (0.08 + 0.4 * uLevel * k) * smoothstep(1.2, 0.15, r);

    // A small glow, not a fog: kept light so the wallpaper stays readable through it.
    float halo = exp(-r * r * (6.0 - 3.0 * uBass * k)) * (0.08 + 0.3 * uBeat * k);

    vec3 ringCol = mix(uColor1.rgb, uColor2.rgb, 0.5 + 0.5 * sin(a * 2.0 + uFlow * 0.3));
    // Premultiplied, mostly additive: light is added on top, little of the wallpaper is covered.
    vec3 rgb = min(ringCol * ringA + uColor3.rgb * halo, vec3(1.0));
    float alpha = clamp(ringA * 0.6 + halo * 0.5, 0.0, 0.6);
    fragColor = vec4(rgb, alpha);
}
