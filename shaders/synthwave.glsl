// @name Synthwave
// @description Retro sunset over a neon grid, with a mountain range drawn by the spectrum
// @author pivuche

float hash21(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}

float vnoise(float x) {
    float i = floor(x), f = fract(x);
    return mix(hash21(vec2(i, 3.0)), hash21(vec2(i + 1.0, 3.0)), f * f * (3.0 - 2.0 * f));
}

vec3 sat(vec3 c) {
    return clamp(mix(vec3(dot(c, vec3(0.299, 0.587, 0.114))), c, 1.7), 0.0, 1.0);
}

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = (fragCoord - 0.5 * iResolution) / iResolution.y;
    float k = uIntensity;
    float px = 1.0 / iResolution.y;
    float hz = -0.08;
    vec3 col;
    vec3 c1 = sat(uColor1.rgb), c2 = sat(uColor2.rgb), c3 = sat(uColor3.rgb);

    if (uv.y > hz) {
        float y = uv.y - hz;
        // Sky
        col = mix(c3 * c3 * 0.55, c2 * c2 * 0.18, smoothstep(0.0, 0.22, y));
        col = mix(col, c1 * 0.02, smoothstep(0.15, 0.6, y));
        vec2 sg = uv * 90.0;
        float sh = hash21(floor(sg));
        float star = step(0.985, sh) * smoothstep(0.35, 0.0, length(fract(sg) - 0.5));
        col += vec3(star) * smoothstep(0.1, 0.4, y) * (0.4 + 0.3 * sin(iTime * 2.0 + sh * 60.0) + 0.6 * uHigh * k);

        // Sun with horizontal cuts that scroll down
        vec2 sc = vec2(uv.x, y - 0.2);
        float sr = 0.25 * (1.0 + 0.04 * uBass * k);
        float sd = length(sc) - sr;
        float sun = smoothstep(px * 1.5, -px * 1.5, sd);
        float cut = fract((sc.y - uFlow * 0.03) * 22.0);
        float cutW = smoothstep(0.1, -0.2, sc.y) * 0.75 * smoothstep(-0.3, 0.0, -sc.y);
        sun *= smoothstep(cutW - 0.04, cutW + 0.04, cut) + step(0.0, sc.y - 0.02);
        // c*c deepens a pastel theme colour, giving the sun a real sunset gradient.
        vec3 sunC = mix(c2 * c2 * c2 * 1.1, c3 * 0.95, smoothstep(-0.25, 0.05, sc.y));
        sunC = mix(sunC, mix(c3, vec3(1.0), 0.35), smoothstep(0.05, 0.25, sc.y));
        col = mix(col, sunC, sun);
        col += c3 * 0.3 * exp(-max(sd, 0.0) * 8.0) * (0.6 + 0.6 * uBeat * k);

        // Mountains: a quiet ridge plus the spectrum, mirrored around the centre (bass in the middle)
        float ax = abs(uv.x);
        float ridge = 0.05 + 0.06 * vnoise(uv.x * 6.0 + 2.0) + 0.03 * vnoise(uv.x * 17.0);
        float sp = specAt(clamp(ax * 1.1, 0.0, 1.0));
        float mh = ridge + 0.16 * sp * k;
        float md = y - mh;
        float mtn = smoothstep(px * 1.5, -px * 1.5, md);
        vec3 mtnC = c1 * 0.03 + c2 * 0.07 * smoothstep(mh, 0.0, y);
        col = mix(col, mtnC, mtn);
        col += c1 * exp(-abs(md) / (px * 2.0)) * 0.8;
        col += c1 * 0.2 * exp(-abs(md) * 50.0);
    } else {
        // Floor grid in perspective
        float d = hz - uv.y;
        float z = 0.35 / d;
        float x = uv.x * z;
        float zz = z + uFlow * 0.9;
        vec2 gd = abs(fract(vec2(x, zz)) - 0.5);
        float lw = px * z * z / 0.35 * 1.2;
        float lx = smoothstep(px * z * 1.6, 0.0, gd.x) ;
        float lz = smoothstep(lw, 0.0, gd.y) ;
        float grid = max(lx, lz);
        float fog = exp(-z * 0.12);
        col = mix(c1 * 0.015, c2 * 0.06, fog);
        vec3 gc = mix(c2, c3, fog);
        float glowLine = max(exp(-gd.x / (px * z * 6.0)), exp(-gd.y / (lw * 4.0))) * 0.25;
        col += gc * (grid + glowLine) * mix(0.25, 1.0, fog) * (0.8 + 0.8 * uBeat * k + 0.3 * uLevel * k);
        col += c3 * 0.3 * exp(-d * 14.0);
    }

    col *= 1.0 - 0.35 * dot(uv * vec2(0.6, 1.0), uv * vec2(0.6, 1.0));
    col = col / (1.0 + col * 0.3);
    fragColor = vec4(col, 1.0);
}
