@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(tex_coord, 0.25, 1.0);
}