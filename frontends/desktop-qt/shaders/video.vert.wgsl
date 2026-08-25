struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
}

@vertex
fn main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let positions = array(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );

    let position = positions[index];

    return VertexOutput(
        vec4<f32>(position, 0.0, 1.0),
        position * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5),
    );
}