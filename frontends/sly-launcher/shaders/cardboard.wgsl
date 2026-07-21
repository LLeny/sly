#import bevy_pbr::{
    forward_io::{VertexOutput, FragmentOutput},
    pbr_fragment::pbr_input_from_standard_material,
    pbr_functions::{alpha_discard, apply_pbr_lighting, main_pass_post_lighting_processing},
}

struct CardboardExtension {
    parameters: vec4<f32>,
}

@group(#{MATERIAL_BIND_GROUP}) @binding(100)
var<uniform> cardboard: CardboardExtension;

fn hash_21(point: vec2<f32>) -> f32 {
    return fract(sin(dot(point, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn value_noise(point: vec2<f32>) -> f32 {
    let cell = floor(point);
    let local = fract(point);
    let blend = local * local * (3.0 - 2.0 * local);
    let bottom = mix(hash_21(cell), hash_21(cell + vec2<f32>(1.0, 0.0)), blend.x);
    let top = mix(
        hash_21(cell + vec2<f32>(0.0, 1.0)),
        hash_21(cell + vec2<f32>(1.0, 1.0)),
        blend.x,
    );
    return mix(bottom, top, blend.y);
}

fn paper_height(uv: vec2<f32>, seed: f32, detail_fade: f32) -> f32 {
    let broad = value_noise(uv * vec2<f32>(24.0, 19.0) + seed);
    let medium = value_noise(uv * vec2<f32>(91.0, 73.0) - seed * 0.71);
    let fiber_warp = value_noise(uv * vec2<f32>(13.0, 31.0) + seed * 1.93);
    let fiber_phase = uv.y * 430.0 + fiber_warp * 9.0 + seed;
    let fibers = sin(fiber_phase) * 0.68 + sin(fiber_phase * 0.47 + seed) * 0.32;
    let flecks = value_noise(uv * vec2<f32>(370.0, 510.0) + seed * 2.17);

    return (broad - 0.5) * 0.42
        + (medium - 0.5) * 0.20
        + fibers * 0.075 * detail_fade
        + (flecks - 0.5) * 0.14 * detail_fade;
}

fn ellipse_dent(
    uv: vec2<f32>,
    center: vec2<f32>,
    radius: vec2<f32>,
    angle: f32,
) -> f32 {
    let cosine = cos(angle);
    let sine = sin(angle);
    let offset = uv - center;
    let rotated = vec2<f32>(
        offset.x * cosine - offset.y * sine,
        offset.x * sine + offset.y * cosine,
    );
    let distance = length(rotated / radius);
    let bowl = 1.0 - smoothstep(0.0, 1.0, distance);
    let rim = smoothstep(0.68, 0.90, distance)
        * (1.0 - smoothstep(0.90, 1.16, distance));
    return -bowl * bowl + rim * 0.26;
}

fn dent_field(uv: vec2<f32>, seed: f32) -> f32 {
    let center_a = vec2<f32>(
        0.20 + hash_21(vec2<f32>(seed, 1.7)) * 0.60,
        0.20 + hash_21(vec2<f32>(seed, 4.3)) * 0.60,
    );
    let center_b = vec2<f32>(
        0.18 + hash_21(vec2<f32>(seed, 8.1)) * 0.64,
        0.18 + hash_21(vec2<f32>(seed, 11.9)) * 0.64,
    );
    let dent_a = ellipse_dent(
        uv,
        center_a,
        vec2<f32>(0.045, 0.030),
        hash_21(vec2<f32>(seed, 14.2)) * 3.14159,
    );
    let dent_b = ellipse_dent(
        uv,
        center_b,
        vec2<f32>(0.026, 0.052),
        hash_21(vec2<f32>(seed, 18.6)) * 3.14159,
    );

    let crease_center = vec2<f32>(
        0.24 + hash_21(vec2<f32>(seed, 21.4)) * 0.52,
        0.24 + hash_21(vec2<f32>(seed, 25.8)) * 0.52,
    );
    let crease_angle = hash_21(vec2<f32>(seed, 29.1)) * 3.14159;
    let crease_direction = vec2<f32>(cos(crease_angle), sin(crease_angle));
    let crease_normal = vec2<f32>(-crease_direction.y, crease_direction.x);
    let crease_offset = uv - crease_center;
    let along = abs(dot(crease_offset, crease_direction));
    let across = abs(dot(crease_offset, crease_normal));
    let crease = (1.0 - smoothstep(0.002, 0.010, across))
        * (1.0 - smoothstep(0.055, 0.17, along));

    return dent_a * 0.72 + dent_b * 0.48 - crease * 0.16;
}

fn perturb_normal_from_height(
    normal: vec3<f32>,
    world_position: vec3<f32>,
    height: f32,
    strength: f32,
) -> vec3<f32> {
    let position_dx = dpdx(world_position);
    let position_dy = dpdy(world_position);
    let height_dx = dpdx(height);
    let height_dy = dpdy(height);
    let reciprocal_x = cross(position_dy, normal);
    let reciprocal_y = cross(normal, position_dx);
    let determinant = dot(position_dx, reciprocal_x);
    let surface_gradient = sign(determinant)
        * (height_dx * reciprocal_x + height_dy * reciprocal_y)
        / max(abs(determinant), 0.0000001);
    return normalize(normal - surface_gradient * strength);
}

@fragment
fn fragment(
    vertex_output: VertexOutput,
    @builtin(front_facing) is_front: bool,
) -> FragmentOutput {
    var pbr_input = pbr_input_from_standard_material(vertex_output, is_front);
    let uv = vertex_output.uv;
    let seed = cardboard.parameters.w;

    let uv_footprint = max(length(dpdx(uv)), length(dpdy(uv)));
    let detail_fade = 1.0 - smoothstep(0.0015, 0.010, uv_footprint);
    let broad = value_noise(uv * vec2<f32>(24.0, 19.0) + seed);
    let medium = value_noise(uv * vec2<f32>(91.0, 73.0) - seed * 0.71);
    let fine = value_noise(uv * vec2<f32>(330.0, 470.0) + seed * 1.37);

    let edge_x = min(uv.x, 1.0 - uv.x);
    let edge_y = min(uv.y, 1.0 - uv.y);
    let edge_distance = min(edge_x, edge_y);
    let edge_noise = value_noise(uv * vec2<f32>(67.0, 83.0) + seed * 2.1);
    let edge_width = 0.019 + edge_noise * 0.017;
    let edge = 1.0 - smoothstep(0.002, edge_width, edge_distance);
    let corner = 1.0 - smoothstep(0.018, 0.095, length(vec2<f32>(edge_x, edge_y)));

    var height = paper_height(uv, seed, detail_fade);
    let dents = dent_field(uv, seed);
    height += dents * 0.58;
    height -= edge * (0.10 + edge_noise * 0.08);
    height += corner * (medium - 0.5) * 0.12;
    pbr_input.N = perturb_normal_from_height(
        pbr_input.N,
        vertex_output.world_position.xyz,
        height,
        cardboard.parameters.x,
    );
    pbr_input.clearcoat_N = pbr_input.N;

    let luminance = dot(
        pbr_input.material.base_color.rgb,
        vec3<f32>(0.2126, 0.7152, 0.0722),
    );
    let ink_density = 1.0 - luminance;
    let roughness_breakup = (broad - 0.5) * cardboard.parameters.y
        + (medium - 0.5) * 0.052
        + (fine - 0.5) * 0.026 * detail_fade
        + edge * 0.075
        + min(abs(dents), 1.0) * 0.055
        - ink_density * 0.018;
    pbr_input.material.perceptual_roughness = clamp(
        max(pbr_input.material.perceptual_roughness, 0.68) + roughness_breakup,
        0.61,
        0.96,
    );

    let softened_color = pbr_input.material.base_color.rgb
        * (0.986 + broad * 0.016 + medium * 0.006);
    pbr_input.material.base_color = vec4<f32>(
        softened_color,
        pbr_input.material.base_color.a,
    );
    let wear_pattern = smoothstep(0.38, 0.84, value_noise(uv * 113.0 - seed * 1.7));
    let wear = clamp(edge * (0.30 + wear_pattern * 0.70) + corner * 0.24, 0.0, 1.0);
    let worn_luminance = dot(
        pbr_input.material.base_color.rgb,
        vec3<f32>(0.2126, 0.7152, 0.0722),
    );
    let paper_tint = mix(
        vec3<f32>(worn_luminance),
        vec3<f32>(0.74, 0.66, 0.52),
        vec3<f32>(0.18),
    );
    let worn_color = mix(
        pbr_input.material.base_color.rgb,
        paper_tint,
        vec3<f32>(wear * cardboard.parameters.z),
    );
    pbr_input.material.base_color = vec4<f32>(
        worn_color,
        pbr_input.material.base_color.a,
    );
    pbr_input.material.base_color = alpha_discard(
        pbr_input.material,
        pbr_input.material.base_color,
    );

    var out: FragmentOutput;
    out.color = apply_pbr_lighting(pbr_input);
    out.color = main_pass_post_lighting_processing(pbr_input, out.color);
    return out;
}
