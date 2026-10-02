struct Globals {
    view_projection: mat4x4<f32>,
    viewport: vec4<f32>,
    effects: vec4<f32>,
    theme_tint: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

@fragment
fn fs(in: Fullscreen) -> @location(0) vec4<f32> {
    let accent = globals.theme_tint.rgb;
    var background = mix(vec3(0.955), accent, 0.075);

    if globals.effects.x > 0.5 {
        let sine = globals.effects.z;
        let cosine = globals.effects.w;
        let drift = vec2(sine * 0.035, (cosine - 1.0) * 0.025);
        let p = in.position.xy / globals.viewport.y + drift;

        let field = sin(p.x * 5.3 + sin(p.y * 5.1 + sine * 0.08))
            + sin(p.y * 6.7 - p.x * 1.3) * 0.72
            + sin(p.x * 11.0 + p.y * 8.0) * 0.23;
        let bands = abs(fract(field * 5.0) - 0.5);
        let aa = max(fwidth(field * 5.0), 0.003);
        let contour = 1.0 - smoothstep(0.035, 0.035 + aa, bands);

        // Fade the contours out behind the footer.
        let legibility = 1.0 - smoothstep(0.86, 0.99, in.uv.y) * 0.85;
        let ink = mix(vec3(0.48), accent, 0.38);
        background = mix(background, ink, contour * 0.24 * legibility);
    }

    return vec4(background, 1.0);
}
