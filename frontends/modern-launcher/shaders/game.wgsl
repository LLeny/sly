struct Presentation {
    opacity: vec4<f32>,
};

@group(0) @binding(0) var framebuffer: texture_2d<f32>;
@group(0) @binding(1) var pixel_sampler: sampler;
@group(1) @binding(0) var<uniform> presentation: Presentation;

@fragment
fn fs(in: Fullscreen) -> @location(0) vec4<f32> {
    let pixel = textureSample(framebuffer, pixel_sampler, in.uv);
    return vec4(pixel.rgb, pixel.a * presentation.opacity.x);
}
