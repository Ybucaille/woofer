// @name Ripple
// @description Each kick drops a stone in your wallpaper: one clean ring travels out from the centre and fades
// @author pivuche

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 suv = fragCoord / iResolution;
    float aspect = iResolution.x / iResolution.y;
    float k = uIntensity;
    vec2 d = (suv - 0.5) * vec2(aspect, 1.0);
    float r = length(d);

    // uBeat decays as exp(-7t): recover the time since the kick to place the ring.
    float age = -log(max(uBeat, 0.002)) / 7.0;
    float radius = 0.08 + age * 1.6;
    float dr = r - radius;
    float fade = smoothstep(0.002, 0.03, uBeat) * exp(-age * 2.0);
    float env = exp(-dr * dr * 60.0);
    float wave = sin(dr * 38.0) * env * fade * k;

    vec2 dir = r > 0.0001 ? d / r : vec2(0.0);
    vec2 uv = suv + dir * wave * 0.02 / vec2(aspect, 1.0);
    vec3 col = wall(uv).rgb;

    // The crest catches a little light, the trough a little shadow.
    float crest = cos(dr * 38.0) * env * fade * k;
    col *= 1.0 + 0.12 * crest;
    col += uColor3.rgb * max(crest, 0.0) * 0.03;
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
