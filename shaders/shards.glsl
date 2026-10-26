// @name Glass Shards
// @description Your wallpaper seen through faceted glass: each shard bends it a little, kicks crack it open, highs glint on the bevels
// @author pivuche

vec2 hash22(vec2 p) {
    vec3 a = fract(vec3(p.xyx) * vec3(123.34, 234.34, 345.65));
    a += dot(a, a + 34.45);
    return fract(vec2(a.x * a.y, a.y * a.z));
}

vec3 sat(vec3 c) {
    return clamp(mix(vec3(dot(c, vec3(0.299, 0.587, 0.114))), c, 1.5), 0.0, 1.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution) / iResolution.y;
    float k = uIntensity;
    float t = uFlow * 0.08 + iTime * 0.01;
    vec2 p = uv * 2.0;

    // Voronoi, pass 1: nearest cell.
    vec2 n = floor(p), f = fract(p);
    vec2 mr = vec2(0.0), mg = vec2(0.0);
    float md = 8.0;
    for (int j = -1; j <= 1; j++)
    for (int i = -1; i <= 1; i++) {
        vec2 g = vec2(float(i), float(j));
        vec2 o = hash22(n + g);
        o = 0.5 + 0.42 * sin(t + 6.2831 * o);
        vec2 r = g + o - f;
        float d = dot(r, r);
        if (d < md) { md = d; mr = r; mg = g; }
    }
    // Pass 2: distance to the nearest edge, and which way that edge faces.
    float ed = 8.0;
    vec2 en = vec2(0.0);
    for (int j = -2; j <= 2; j++)
    for (int i = -2; i <= 2; i++) {
        vec2 g = mg + vec2(float(i), float(j));
        vec2 o = hash22(n + g);
        o = 0.5 + 0.42 * sin(t + 6.2831 * o);
        vec2 r = g + o - f;
        if (dot(mr - r, mr - r) > 1e-5) {
            vec2 dir = normalize(r - mr);
            float e = dot(0.5 * (mr + r), dir);
            if (e < ed) { ed = e; en = dir; }
        }
    }

    vec3 c1 = sat(uColor1.rgb), c2 = sat(uColor2.rgb), c3 = sat(uColor3.rgb);
    vec2 id = n + mg;
    vec2 h = hash22(id + 17.0);
    vec3 tint = mix(c1, c2, h.x);
    tint = mix(tint, c3, step(0.8, h.y));

    // Light comes from a slowly circling direction; bevels facing it catch it, the others go dark.
    float la = 2.2 + 0.6 * sin(t * 0.4);
    vec2 L = vec2(cos(la), sin(la));
    float bevel = smoothstep(0.07, 0.0, ed);
    float facing = dot(en, L);
    float lit = bevel * max(facing, 0.0);
    float shadow = bevel * max(-facing, 0.0);
    float line = smoothstep(0.01, 0.0, ed);

    // Each shard is tilted a little, so it shifts the picture behind it; bevels bend it more.
    vec2 tilt = hash22(id + 3.0) - 0.5;
    float crack = 1.0 + 1.2 * uBeat * k + 0.4 * uBass * k;
    vec2 off = (tilt * 0.022 - en * bevel * 0.018) * crack;
    vec2 suv = fragCoord / iResolution;
    float disp = 0.0022 * (1.0 + 2.0 * bevel) * crack;   // chromatic dispersion
    vec3 pic = vec3(wall(suv + off * (1.0 + disp * 40.0)).r,
                    wall(suv + off).g,
                    wall(suv + off * (1.0 - disp * 40.0)).b);

    float sweep = dot(uv, L) + 0.35 * dot(tilt, L) - 1.3 * sin(t * 0.5);
    float sheen = exp(-sweep * sweep * 5.0);
    float glint = spec(4 + int(h.y * 11.0)) * step(0.6, h.x) * k;

    vec3 col = pic * mix(vec3(1.0), tint, 0.12 + 0.1 * h.y);
    col += tint * (0.05 * sheen + 0.12 * glint);
    col += vec3(1.0) * lit * (0.35 + 0.25 * sheen + 0.5 * uHigh * k);
    col += vec3(1.0) * line * 0.18;
    col *= 1.0 - shadow * 0.45;
    fragColor = vec4(clamp(col, 0.0, 1.0), 1.0);
}
