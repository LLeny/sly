#define_import_path launcher::contours

const TAU: f32 = 6.28318530718;

fn contour_field(pixel: vec2<f32>, time: f32, scale: f32) -> f32 {
    let u = pixel.x / scale * TAU + time * 0.075;
    let v = pixel.y / scale * TAU - time * 0.052;
    let warped_u = u + sin(v * 2.0 + time * 0.035) * 0.72;
    let warped_v = v + sin(u * 2.0 - time * 0.028) * 0.66;
    let height = sin(warped_u)
        + cos(warped_v) * 0.82
        + sin(warped_u * 2.0 + warped_v * 2.0 + time * 0.09) * 0.28;
    return height * 1.18;
}
