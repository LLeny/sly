struct Globals {
    view_projection: mat4x4<f32>,
    viewport: vec4<f32>,
    effects: vec4<f32>,
    theme_tint: vec4<f32>,
};

struct Object {
    model: mat4x4<f32>,
    tint: vec4<f32>,
    material: vec4<f32>,
    label: vec4<f32>,
    artwork_crop: vec4<f32>,
    top_crop: vec4<f32>,
    artwork_params: vec4<f32>,
    edge_bounds_min: vec4<f32>,
    edge_bounds_size: vec4<f32>,
};

@group(0) @binding(0) var<uniform> globals: Globals;
@group(1) @binding(0) var<uniform> object: Object;
@group(2) @binding(0) var artwork: texture_2d<f32>;
@group(2) @binding(1) var artwork_sampler: sampler;
@group(3) @binding(0) var top_artwork: texture_2d<f32>;
@group(3) @binding(1) var top_sampler: sampler;

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) world: vec3<f32>,
    @location(3) local_normal: vec3<f32>,
    @location(4) local_position: vec3<f32>,
};

@vertex
fn vs(
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
) -> VertexOut {
    let world = object.model * vec4(position, 1.0);

    var out: VertexOut;
    out.clip = globals.view_projection * world;
    out.normal = normalize((object.model * vec4(normal, 0.0)).xyz);
    out.uv = uv;
    out.world = world.xyz;
    out.local_normal = normal;
    out.local_position = position;
    return out;
}

// Undo the printed rotation within the crop.
fn mapped_uv(uv: vec2<f32>, crop: vec4<f32>, turns: f32) -> vec2<f32> {
    var rotated = uv;

    if turns > 2.5 {
        rotated = vec2(1.0 - uv.y, uv.x);
    } else if turns > 1.5 {
        rotated = vec2(1.0 - uv.x, 1.0 - uv.y);
    } else if turns > 0.5 {
        rotated = vec2(uv.y, 1.0 - uv.x);
    }

    return crop.xy + rotated * crop.zw;
}

// Inset half a texel so cropped scans don't bleed.
fn clamp_to_crop(uv: vec2<f32>, crop: vec4<f32>, size: vec2<u32>) -> vec2<f32> {
    let inset = min(0.5 / vec2<f32>(size), crop.zw * 0.5);
    return clamp(uv, crop.xy + inset, crop.xy + crop.zw - inset);
}

fn quiet_checker(position: vec3<f32>, normal: vec3<f32>) -> f32 {
    let n = abs(normal);
    let side_plane = select(position.xz, position.zy, n.x > n.y);
    let plane = select(side_plane, position.xy, n.z >= max(n.x, n.y));
    let cells = plane / 0.006;
    let footprint = max(fwidth(cells).x, fwidth(cells).y);
    let parity = select(-1.0, 1.0, fract((floor(cells.x) + floor(cells.y)) * 0.5) > 0.25);

    return 0.98 + parity * 0.04 * (1.0 - smoothstep(0.3, 0.8, footprint));
}

@fragment
fn fs(in: VertexOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var base = object.tint;
    let fold = object.label.y;
    let is_split_label = object.label.x > 0.5 && fold > 0.0;
    let on_fold = is_split_label && in.uv.y < fold;
    let has_top_scan = object.label.z > 0.5;
    let extends_edge = object.material.w > 0.5;

    var front_uv = in.uv;
    var top_uv = in.uv;

    if is_split_label {
        // Without a top scan, stretch the front's top row over the fold.
        front_uv.y = max(0.0, (in.uv.y - fold) / (1.0 - fold));
        top_uv.y = in.uv.y / fold;
    }

    let model_uv = clamp(
        (in.local_position - object.edge_bounds_min.xyz) / object.edge_bounds_size.xyz,
        vec3(0.0),
        vec3(1.0),
    );

    if extends_edge {
        // Narrow-face UVs don't match the print, so map by position.
        front_uv = vec2(model_uv.x, 1.0 - model_uv.y);

        if abs(in.local_normal.x) > abs(in.local_normal.y) {
            front_uv.x = select(0.0, 1.0, in.local_normal.x > 0.0);
        } else {
            front_uv.y = select(1.0, 0.0, in.local_normal.y > 0.0);
        }

        // The back is mirrored left to right.
        top_uv = vec2(1.0 - front_uv.x, front_uv.y);
    }

    var mapped_front = mapped_uv(front_uv, object.artwork_crop, object.artwork_params.x);
    var mapped_top = mapped_uv(top_uv, object.top_crop, object.artwork_params.y);

    if on_fold || extends_edge {
        mapped_front = clamp_to_crop(mapped_front, object.artwork_crop, textureDimensions(artwork));
    }

    if extends_edge {
        mapped_top = clamp_to_crop(mapped_top, object.top_crop, textureDimensions(top_artwork));
    }

    // Sample unconditionally to keep derivatives valid.
    let front_pixel = textureSample(artwork, artwork_sampler, mapped_front);
    let top_pixel = textureSample(top_artwork, top_sampler, mapped_top);
    let front_color = mix(object.tint.rgb, front_pixel.rgb, front_pixel.a);
    let back_color = mix(object.tint.rgb, top_pixel.rgb, top_pixel.a);

    var has_scan = true;

    if on_fold && has_top_scan {
        base = vec4(mix(base.rgb, top_pixel.rgb, top_pixel.a), 1.0);
    } else if object.material.x > 0.5 {
        base = vec4(mix(base.rgb, front_pixel.rgb, front_pixel.a), 1.0);
    } else {
        has_scan = false;
    }

    if extends_edge {
        var edge_color = front_color;

        if object.material.w > 2.5 {
            edge_color = mix(back_color, front_color, smoothstep(0.3, 0.7, model_uv.z));
        } else if object.material.w > 1.5 {
            edge_color = back_color;
        }

        base = vec4(edge_color, 1.0);
    }

    let checker = quiet_checker(in.local_position, in.local_normal);

    if object.material.z > 0.5 && !has_scan {
        base = vec4(base.rgb * checker, 1.0);
    }

    base.a = object.tint.a;

    // Discard back faces of scanned boxes so transparency doesn't darken them.
    if object.artwork_params.w > 0.5 && !front {
        discard;
    }

    if globals.effects.y < 0.5 || object.artwork_params.w > 0.5 {
        return base;
    }

    let n = normalize(in.normal) * select(-1.0, 1.0, front);
    let key = normalize(vec3(-0.45, 0.7, 1.0));
    let diffuse = max(dot(n, key), 0.0);
    let fill = 0.56 + 0.09 * n.y;
    let eye = normalize(vec3(0.0, 0.0, 2.0) - in.world);

    let shell = object.artwork_params.z > 0.5;
    let material_finish = object.artwork_params.z > 1.5;
    let shell_roughness = min(object.material.y, 0.36);
    let roughness = clamp(
        select(object.material.y, shell_roughness, shell && !material_finish),
        0.08,
        0.95,
    );
    let gloss = 1.0 - roughness;
    let exponent = mix(12.0, 150.0, gloss * gloss);
    let half_dot = max(dot(n, normalize(key + eye)), 0.0);

    var specular = pow(half_dot, exponent) * (0.12 + 0.16 * gloss);

    if shell {
        specular = pow(half_dot, select(12.0, exponent, material_finish)) * 0.075;
    }

    let grazing = pow(1.0 - max(dot(n, eye), 0.0), 4.0) * select(0.065, 0.025, shell);

    // Scans already include highlights; keep their reflection faint.
    let coating = select(1.0, 0.22, object.label.x > 0.5 || object.material.x > 0.5);

    // Flatten lighting on plastic so ribs don't produce bright wedges.
    let illumination = select(fill + 0.46 * diffuse, 0.82 + 0.12 * diffuse, shell);
    let lit = base.rgb * illumination + (specular + grazing) * coating;

    return vec4(lit, base.a);
}
