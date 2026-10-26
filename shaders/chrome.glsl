// @name Liquid Chrome
// @description Mirror-polished metal blobs floating over your wallpaper and reflecting it; they merge, split and swell on the bass
// @author pivuche

vec3 sat(vec3 c, float s) {
    return clamp(mix(vec3(dot(c, vec3(0.299, 0.587, 0.114))), c, s), 0.0, 1.0);
}

// Matcap of a chrome sphere in a studio: n.xy is the surface normal seen from the front.
vec3 matcap(vec2 n) {
    vec3 c1 = sat(uColor1.rgb, 1.5), c2 = sat(uColor2.rgb, 1.5), c3 = sat(uColor3.rgb, 1.5);
    float y = n.y + 0.08 * n.x * n.x;
    vec3 sky = mix(c2 * 0.95, c1 * 0.3, smoothstep(-0.3, 0.9, y));
    vec3 ground = mix(c1 * 0.03, c3 * 0.45, smoothstep(-0.55, -1.0, y));
    vec3 c = mix(ground, sky, smoothstep(-0.48, -0.42, y));
    c += c3 * exp(-abs(y + 0.45) * 25.0) * 0.6;
    // Two soft-box reflections.
    vec2 q = n - vec2(-0.35, 0.45);
    c += vec3(1.0) * smoothstep(0.22, 0.12, length(q * vec2(1.0, 1.8)));
    c += vec3(0.9) * 0.5 * smoothstep(0.06, 0.0, abs(n.x - 0.55 - 0.15 * n.y)) * smoothstep(-0.2, 0.3, n.y);
    return c;
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution) / iResolution.y;
    float k = uIntensity;
    float t = uFlow * 0.18 + iTime * 0.02;
    float px = 1.0 / iResolution.y;

    float F = 0.0;
    vec2 g = vec2(0.0);
    for (int i = 0; i < 7; i++) {
        float fi = float(i);
        vec2 c = vec2(sin(t * (0.31 + fi * 0.07) + fi * 2.1) * 0.55,
                      cos(t * (0.27 + fi * 0.05) + fi * 1.3) * 0.28);
        c.x *= iResolution.x / iResolution.y * 0.85;
        float rad = 0.16 + 0.05 * sin(fi * 3.7) + (0.025 * spec(i * 2) + 0.03 * uBeat) * k;
        vec2 d = uv - c;
        float dd = dot(d, d) + 1e-4;
        float w = rad * rad / dd;
        F += w;
        g += -2.0 * w * d / dd;
    }

    float gl = length(g);
    float mask = smoothstep(1.0 - gl * px, 1.0 + gl * px, F);

    // Height h = sqrt(1 - 1/F): vertical at the rim, flat on top of each blob.
    float h = sqrt(max(1.0 - 1.0 / F, 0.0));
    vec2 gh = g / (F * F * 2.0 * max(h, 0.03));
    vec3 n = normalize(vec3(-gh * 0.45, 1.0));
    // Reflect the wallpaper like a chrome sphere would, then add the studio lights on top.
    vec2 suv = fragCoord / iResolution;
    vec3 refl = wall(clamp(vec2(0.5) + n.xy * 0.42 + (suv - 0.5) * 0.25, 0.0, 1.0)).rgb;
    vec3 metal = matcap(n.xy) * 0.55 + refl * mix(vec3(1.0), sat(uColor1.rgb, 1.2), 0.25) * 0.8;
    metal += vec3(1.0) * pow(max(dot(n, normalize(vec3(-0.4, 0.5, 1.0))), 0.0), 80.0) * (0.4 + 1.0 * uHigh * k);

    // Background: the wallpaper itself, dimmed a touch, with a soft shadow and glow under the metal.
    vec3 bg = wall(suv).rgb * 0.8;
    bg *= 1.0 - 0.45 * smoothstep(0.25, 0.95, F);
    bg += sat(uColor3.rgb, 1.5) * 0.1 * smoothstep(0.3, 0.95, F) * (1.0 + 1.5 * uBeat * k);

    vec3 col = mix(bg, metal, mask);
    col = col / (1.0 + col * 0.35);
    fragColor = vec4(col, 1.0);
}
