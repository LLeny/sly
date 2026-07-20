use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use core_model::Region;

use crate::artwork;
use crate::library::{BoxStyle, CartShell};

pub struct ProcessedArtwork {
    pub title: String,
    pub front: Option<Image>,
    pub back: Option<Image>,
    pub spine: Option<Image>,
    pub edges: Option<Image>,
    pub label: Option<Image>,
    pub dominant_color: Option<Vec3>,
}

pub fn process_artwork(bundle: artwork::ArtworkBundle, region: Region) -> ProcessedArtwork {
    let shell = CartShell::from_region(region);
    let style = BoxStyle::from_region(region);

    let front = decode_rgba(bundle.front.as_deref());
    let label = decode_rgba(bundle.label.as_deref())
        .or_else(|| front.clone())
        .map(|source| cart_label_image(source, shell));
    let front = front.map(|source| orient_box_face(source, style));
    ProcessedArtwork {
        title: bundle.title,
        edges: front
            .as_ref()
            .map(|source| edge_texture_image(source, style)),
        dominant_color: front.as_ref().and_then(dominant_box_color),
        front: front.map(mipmapped_artwork_image),
        back: decode_rgba(bundle.back.as_deref())
            .map(|source| mipmapped_artwork_image(orient_box_face(source, style))),
        spine: decode_rgba(bundle.spine.as_deref())
            .map(|source| mipmapped_artwork_image(orient_spine(source))),
        label,
    }
}

fn orient_spine(source: image::RgbaImage) -> image::RgbaImage {
    if source.height() > source.width() {
        image::imageops::rotate90(&source)
    } else {
        source
    }
}

fn orient_box_face(source: image::RgbaImage, style: BoxStyle) -> image::RgbaImage {
    match style {
        BoxStyle::Western => source,
        BoxStyle::Japanese => image::imageops::rotate90(&source),
    }
}

fn decode_rgba(bytes: Option<&[u8]>) -> Option<image::RgbaImage> {
    Some(image::load_from_memory(bytes?).ok()?.to_rgba8())
}

fn dominant_box_color(source: &image::RgbaImage) -> Option<Vec3> {
    const CLUSTERS: usize = 6;
    const ITERATIONS: usize = 8;
    const HISTOGRAM_SIZE: usize = 8 * 8 * 8;

    let reduced = image::imageops::resize(source, 64, 64, image::imageops::FilterType::Triangle);
    let mut samples = Vec::with_capacity(64 * 64);
    let mut histogram_counts = [0_u32; HISTOGRAM_SIZE];
    let mut histogram_colors = [Vec3::ZERO; HISTOGRAM_SIZE];

    for pixel in reduced.pixels() {
        let color = Vec3::new(
            pixel[0] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[2] as f32 / 255.0,
        );
        let maximum = color.max_element();
        let chroma = maximum - color.min_element();
        let luminance = color.dot(Vec3::new(0.2126, 0.7152, 0.0722));
        if maximum < 0.055 || (luminance > 0.92 && chroma < 0.15) {
            continue;
        }

        samples.push(color);

        let quantized = (color * 7.999).as_uvec3().min(UVec3::splat(7));
        let bin = (quantized.x * 64 + quantized.y * 8 + quantized.z) as usize;
        histogram_counts[bin] += 1;
        histogram_colors[bin] += color;
    }

    if samples.is_empty() {
        return None;
    }

    let first_bin = histogram_counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| *count)
        .map(|(index, _)| index)?;
    let mut centers = vec![histogram_colors[first_bin] / histogram_counts[first_bin].max(1) as f32];
    while centers.len() < CLUSTERS {
        let Some(&candidate) = samples.iter().max_by(|left, right| {
            nearest_color_distance(**left, &centers)
                .total_cmp(&nearest_color_distance(**right, &centers))
        }) else {
            break;
        };
        if nearest_color_distance(candidate, &centers) < 0.0025 {
            break;
        }
        centers.push(candidate);
    }

    let mut sums = vec![Vec3::ZERO; centers.len()];
    let mut counts = vec![0_u32; centers.len()];
    for _ in 0..ITERATIONS {
        sums.fill(Vec3::ZERO);
        counts.fill(0);
        for &sample in &samples {
            let cluster = nearest_color_cluster(sample, &centers);
            sums[cluster] += sample;
            counts[cluster] += 1;
        }
        for (index, center) in centers.iter_mut().enumerate() {
            if counts[index] > 0 {
                *center = sums[index] / counts[index] as f32;
            }
        }
    }

    centers
        .into_iter()
        .zip(counts)
        .max_by(|left, right| {
            dominant_cluster_score(left.0, left.1)
                .total_cmp(&dominant_cluster_score(right.0, right.1))
        })
        .map(|(color, _)| color)
}

fn nearest_color_cluster(color: Vec3, centers: &[Vec3]) -> usize {
    centers
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            color
                .distance_squared(**left)
                .total_cmp(&color.distance_squared(**right))
        })
        .map(|(index, _)| index)
        .unwrap_or_default()
}

fn nearest_color_distance(color: Vec3, centers: &[Vec3]) -> f32 {
    centers
        .iter()
        .map(|center| color.distance_squared(*center))
        .min_by(f32::total_cmp)
        .unwrap_or_default()
}

pub fn color_saturation(color: Vec3) -> f32 {
    let peak = color.max_element();
    (peak - color.min_element()) / peak.max(0.001)
}

fn dominant_cluster_score(color: Vec3, count: u32) -> f32 {
    let saturation = color_saturation(color);
    let chromatic_weight = if saturation < 0.06 {
        0.42
    } else {
        0.75 + saturation * 0.25
    };
    count as f32 * chromatic_weight
}

fn cart_label_image(source: image::RgbaImage, shell: CartShell) -> Image {
    match shell {
        CartShell::Us => us_label_image(&source),
        CartShell::JpPal => jp_pal_label_image(&source),
    }
}

fn fit_cover(source: &image::RgbaImage, width: u32, height: u32) -> image::RgbaImage {
    let scale = (width as f32 / source.width() as f32).max(height as f32 / source.height() as f32);
    let scaled_width = ((source.width() as f32 * scale).round() as u32).max(width);
    let scaled_height = ((source.height() as f32 * scale).round() as u32).max(height);
    let resized = image::imageops::resize(
        source,
        scaled_width,
        scaled_height,
        image::imageops::FilterType::Lanczos3,
    );
    image::imageops::crop_imm(
        &resized,
        (scaled_width - width) / 2,
        (scaled_height - height) / 2,
        width,
        height,
    )
    .to_image()
}

fn us_label_image(source: &image::RgbaImage) -> Image {
    const WIDTH: u32 = 825;
    const FRONT_HEIGHT: u32 = 374;
    const FOLD_HEIGHT: u32 = 86;

    let front = fit_cover(source, WIDTH, FRONT_HEIGHT);
    let mut unfolded = image::RgbaImage::new(WIDTH, FRONT_HEIGHT + FOLD_HEIGHT);
    for y in 0..FOLD_HEIGHT {
        for x in 0..WIDTH {
            unfolded.put_pixel(x, y, *front.get_pixel(x, 0));
        }
    }
    image::imageops::replace(&mut unfolded, &front, 0, FOLD_HEIGHT as i64);

    mipmapped_artwork_image(unfolded)
}

fn jp_pal_label_image(source: &image::RgbaImage) -> Image {
    mipmapped_artwork_image(fit_cover(source, 1040, 361))
}

fn edge_texture_image(source: &image::RgbaImage, style: BoxStyle) -> Image {
    let (width, height) = match style {
        BoxStyle::Western => (12, 192),
        BoxStyle::Japanese => (192, 12),
    };

    let reduced =
        image::imageops::resize(source, width, height, image::imageops::FilterType::Gaussian);
    mipmapped_artwork_image(reduced)
}

fn mipmapped_artwork_image(source: image::RgbaImage) -> Image {
    let width = source.width();
    let height = source.height();
    let mip_level_count = 32 - width.max(height).leading_zeros();

    let mut sampler = ImageSamplerDescriptor::linear();
    sampler.set_anisotropic_filter(8);

    let mut mips = Vec::with_capacity(source.as_raw().len() / 2);
    let mut level_width = width;
    let mut level_height = height;
    while level_width > 1 || level_height > 1 {
        level_width = (level_width / 2).max(1);
        level_height = (level_height / 2).max(1);

        let level = image::imageops::resize(
            &source,
            level_width,
            level_height,
            image::imageops::FilterType::Lanczos3,
        );
        mips.extend_from_slice(level.as_raw());
    }

    let mut image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        source.into_raw(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image
        .data
        .as_mut()
        .expect("newly constructed artwork image has pixel data")
        .extend_from_slice(&mips);
    image.texture_descriptor.mip_level_count = mip_level_count;
    image.sampler = ImageSampler::Descriptor(sampler);
    image
}
