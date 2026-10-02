@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var source_sampler: sampler;

@fragment
fn fs(in: Fullscreen) -> @location(0) vec4<f32> {
    return textureSample(source, source_sampler, in.uv);
}
