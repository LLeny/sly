#import bevy_render::globals::Globals
#import bevy_ui::ui_vertex_output::UiVertexOutput
#import launcher::contours::contour_field

@group(0) @binding(1) var<uniform> globals: Globals;

@group(1) @binding(0) var<uniform> parameters: vec4<f32>;
@group(1) @binding(1) var<uniform> backdrop_tint: vec4<f32>;
@group(1) @binding(2) var<uniform> backdrop_parameters: vec4<f32>;
@group(1) @binding(3) var shelf_view: texture_2d<f32>;
@group(1) @binding(4) var shelf_sampler: sampler;

fn rounded_box_distance(pixel: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let half_size = size * 0.5;
    let q = abs(pixel - half_size) - (half_size - vec2<f32>(radius));
    return length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

fn generated_background(pixel: vec2<f32>) -> vec3<f32> {
    let time = globals.time;
    let scale = max(backdrop_parameters.y, 1.0);
    let contour = contour_field(pixel, time, scale);
    let distance = abs(contour - round(contour));
    // A broad transition approximates the contour layer after a soft blur.
    let line = 1.0 - smoothstep(0.025, 0.30, distance);
    let alpha = 0.006 + line * 0.34;
    let paper = vec3<f32>(0.902, 0.906, 0.916);
    return mix(paper, backdrop_tint.rgb, alpha);
}

fn shelf_sample(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(
        shelf_view,
        shelf_sampler,
        clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0)),
    );
}

fn blurred_shelf(uv: vec2<f32>, texel: vec2<f32>, radius: f32) -> vec4<f32> {
    const weights = array<f32, 13>(
        0.018544,
        0.034167,
        0.056332,
        0.083109,
        0.109719,
        0.129618,
        0.137023,
        0.129618,
        0.109719,
        0.083109,
        0.056332,
        0.034167,
        0.018544,
    );
    let spacing = texel * vec2<f32>(radius * 1.25, radius) / 6.0;
    var color = vec4<f32>(0.0);
    for (var y = 0; y < 13; y += 1) {
        for (var x = 0; x < 13; x += 1) {
            let offset = (vec2<f32>(f32(x), f32(y)) - 6.0) * spacing;
            color += shelf_sample(uv + offset) * weights[x] * weights[y];
        }
    }
    return color;
}

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let distance = rounded_box_distance(
        in.uv * in.size,
        in.size,
        min(parameters.z, min(in.size.x, in.size.y) * 0.5),
    );
    if distance > 1.0 {
        discard;
    }

    let dimensions = vec2<f32>(textureDimensions(shelf_view));
    let screen_uv = in.position.xy / dimensions;
    let blurred = blurred_shelf(screen_uv, 1.0 / dimensions, parameters.x);
    let background = generated_background(in.position.xy);
    let scene = background * (1.0 - blurred.a) + blurred.rgb;
    var color = mix(scene, vec3<f32>(0.936, 0.945, 0.958), parameters.y);

    let border = smoothstep(-parameters.w - 1.0, -parameters.w + 1.0, distance);
    color = mix(color, vec3<f32>(0.45, 0.42, 0.51), border * 0.32);
    let edge_alpha = 1.0 - smoothstep(-0.5, 1.0, distance);
    return vec4<f32>(color, edge_alpha * 0.965);
}
