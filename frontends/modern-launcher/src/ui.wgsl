struct Screen {
    // Physical width and height, then the display scale factor.
    values: vec4<f32>,
};

@group(0) @binding(0) var<uniform> screen: Screen;

struct RectInput {
    @location(0) rect: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) parameters: vec4<f32>,
};

struct RectOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) local: vec2<f32>,
    @location(1) size: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) radius: f32,
};

@vertex
fn vs_main(input: RectInput, @builtin(vertex_index) index: u32) -> RectOutput {
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
        vec2(0.0, 1.0),
        vec2(0.0, 1.0),
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
    );
    let local = corners[index] * input.rect.zw;
    let physical = (input.rect.xy + local) * screen.values.z;

    var output: RectOutput;
    output.position = vec4(
        physical.x / screen.values.x * 2.0 - 1.0,
        1.0 - physical.y / screen.values.y * 2.0,
        0.0,
        1.0,
    );
    output.local = local;
    output.size = input.rect.zw;
    output.color = input.color;
    output.radius = input.parameters.x;
    return output;
}

@fragment
fn fs_main(input: RectOutput) -> @location(0) vec4<f32> {
    let q = abs(input.local - input.size * 0.5) - input.size * 0.5 + input.radius;
    let distance = length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - input.radius;
    let width = max(fwidth(distance), 0.001);
    let coverage = 1.0 - smoothstep(-width * 0.5, width * 0.5, distance);

    return vec4(input.color.rgb, input.color.a * coverage);
}
