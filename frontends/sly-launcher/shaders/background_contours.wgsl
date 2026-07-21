#import bevy_render::globals::Globals
#import bevy_ui::ui_vertex_output::UiVertexOutput
#import launcher::contours::contour_field

@group(0) @binding(1) var<uniform> globals: Globals;

@group(1) @binding(0) var<uniform> tint: vec4<f32>;
@group(1) @binding(1) var<uniform> parameters: vec4<f32>;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let time = globals.time;
    let scale = max(parameters.y, 1.0);
    let pixel = in.uv * in.size;
    let contour = contour_field(pixel, time, scale);
    let distance = abs(contour - round(contour));

    let antialias = max(fwidth(contour) * 1.25, 0.002);
    let line = 1.0 - smoothstep(0.145 - antialias, 0.145 + antialias, distance);
    let alpha = 0.008 + line * 0.48;
    return vec4<f32>(tint.rgb, alpha);
}
