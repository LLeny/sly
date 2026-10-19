struct Presentation {
    opacity: vec4<f32>,
};

@group(0) @binding(0) var framebuffer: texture_2d<f32>;
@group(0) @binding(1) var pixel_sampler: sampler;
@group(1) @binding(0) var<uniform> presentation: Presentation;

@fragment
fn fs(in: Fullscreen) -> @location(0) vec4<f32> {
    var uv = in.uv;
    switch u32(presentation.opacity.y + 0.5) {
        case 1u: { uv = vec2<f32>(uv.y, 1.0 - uv.x); }
        case 2u: { uv = vec2<f32>(1.0 - uv.y, uv.x); }
        default: {}
    }
    let pixel = textureSample(framebuffer, pixel_sampler, uv);
    return vec4(pixel.rgb, pixel.a * presentation.opacity.x);
}
