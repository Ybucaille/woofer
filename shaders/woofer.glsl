// @name Woofer
// @description Your wallpaper as a speaker cone: the bass pushes it out, every kick makes it ring
// @author pivuche

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 suv = fragCoord / iResolution;
    float aspect = iResolution.x / iResolution.y;
    float k = uIntensity;
    vec2 d = (suv - 0.5) * vec2(aspect, 1.0);
    float r = length(d);

    // uBeat decays as exp(-7t), so this is the time since the last kick.
    float age = -log(max(uBeat, 0.002)) / 7.0;
    // Excursion follows the bass; each kick adds a damped membrane wobble.
    float ring = exp(-age * 6.0) * cos(age * 24.0) * smoothstep(0.002, 0.02, uBeat);
    float push = (0.025 * uBass * uBass + 0.07 * ring) * k;

    // Cone profile: full excursion in the middle, none at the surround (screen edges stay put).
    float R = 0.62 * aspect;
    float x = min(r / R, 1.0);
    float h = 0.5 + 0.5 * cos(3.14159 * x);
    vec2 uv = 0.5 + (suv - 0.5) * (1.0 - push * h);
    vec3 col = wall(uv).rgb;

    // Light from the top-left grazes the bulge, so the push reads as depth, not just zoom.
    float slope = 0.5 * 3.14159 * sin(3.14159 * x) / R;
    vec2 dir = r > 0.0001 ? d / r : vec2(0.0);
    float shade = dot(dir, normalize(vec2(-0.6, 0.8))) * slope * push * 1.4;
    col *= 1.0 + shade;
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
