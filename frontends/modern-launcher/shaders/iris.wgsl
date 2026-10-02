struct Iris {
    size_center: vec4<f32>,
    radius_fade: vec4<f32>,
};

@group(0) @binding(0) var<uniform> iris: Iris;

@fragment
fn fs(in: Fullscreen) -> @location(0) vec4<f32> {
    let size = iris.size_center.xy;
    let center = iris.size_center.zw * size;

    let radius = iris.radius_fade.x * length(max(center, size - center));
    let edge = smoothstep(radius - 1.5, radius + 1.5, distance(in.position.xy, center));

    return vec4(0.0, 0.0, 0.0, max(edge, iris.radius_fade.y));
}
