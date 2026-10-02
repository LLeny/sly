struct Fullscreen {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) index: u32) -> Fullscreen {
    let uv = vec2(f32((index << 1u) & 2u), f32(index & 2u));

    var out: Fullscreen;
    out.position = vec4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0);
    out.uv = uv;
    return out;
}
