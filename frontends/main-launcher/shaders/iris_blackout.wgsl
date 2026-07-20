#import bevy_ui::ui_vertex_output::UiVertexOutput

@group(1) @binding(0) var<uniform> parameters: vec4<f32>;

fn farthest_corner(center: vec2<f32>, aspect: f32) -> f32 {
    let scale = vec2<f32>(aspect, 1.0);
    let top_left = length((vec2<f32>(0.0, 0.0) - center) * scale);
    let top_right = length((vec2<f32>(1.0, 0.0) - center) * scale);
    let bottom_left = length((vec2<f32>(0.0, 1.0) - center) * scale);
    let bottom_right = length((vec2<f32>(1.0, 1.0) - center) * scale);
    return max(max(top_left, top_right), max(bottom_left, bottom_right));
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let progress = clamp(parameters.x, 0.0, 1.0);
    let center = parameters.yz;
    let aspect = in.size.x / max(in.size.y, 1.0);
    let scale = vec2<f32>(aspect, 1.0);
    let distance = length((in.uv - center) * scale);
    let edge = max(fwidth(distance) * 1.5, 0.0025);
    let open_radius = farthest_corner(center, aspect) + edge * 2.0;
    let radius = mix(open_radius, -edge, progress);
    let outside = smoothstep(radius - edge, radius + edge, distance);
    return vec4<f32>(vec3<f32>(0.002), outside * clamp(parameters.w, 0.0, 1.0));
}
