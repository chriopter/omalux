//! Global tonal and detail effects in the normative scene-linear working space.

use super::{optical::finite_f32, spatial::RgbImage, spatial::gaussian_blur};
use crate::develop::CpuImage;

const REC2020_LUMA: [f64; 3] = [0.2627, 0.6780, 0.0593];

pub(super) fn apply_fade(image: &mut CpuImage, amount: f32) {
    if amount == 0.0 {
        return;
    }
    let normalized = f64::from(amount) / 100.0;
    let contrast = 1.0 - 0.18 * normalized;
    let lift = 0.035 * normalized;
    for pixel in image.pixels_mut() {
        pixel.red = finite_f32(f64::from(pixel.red) * contrast + lift);
        pixel.green = finite_f32(f64::from(pixel.green) * contrast + lift);
        pixel.blue = finite_f32(f64::from(pixel.blue) * contrast + lift);
    }
}

/// Applies one vignette over full-image normalized coordinates. The result is
/// independent of how a future renderer partitions work internally.
pub(super) fn apply_vignette(image: &mut CpuImage, amount: f32) {
    if amount == 0.0 {
        return;
    }
    let width = image.width() as usize;
    let height = image.height() as usize;
    let normalized = f64::from(amount) / 100.0;
    for (index, pixel) in image.pixels_mut().iter_mut().enumerate() {
        let x = index % width;
        let y = index / width;
        let normalized_x = 2.0 * (x as f64 + 0.5) / width as f64 - 1.0;
        let normalized_y = 2.0 * (y as f64 + 0.5) / height as f64 - 1.0;
        let radius = (normalized_x * normalized_x + normalized_y * normalized_y).sqrt()
            / std::f64::consts::SQRT_2;
        let edge = smoothstep(0.32, 1.0, radius);
        let gain = if normalized >= 0.0 {
            1.0 - 0.72 * normalized * edge
        } else {
            1.0 + 0.72 * -normalized * edge
        };
        pixel.red = finite_f32(f64::from(pixel.red) * gain);
        pixel.green = finite_f32(f64::from(pixel.green) * gain);
        pixel.blue = finite_f32(f64::from(pixel.blue) * gain);
    }
}

pub(super) fn apply_sharpness(image: &mut CpuImage, amount: f32) {
    if amount == 0.0 {
        return;
    }
    let width = image.width() as usize;
    let height = image.height() as usize;
    let luminance = RgbImage::from_pixels(
        width,
        height,
        image
            .pixels()
            .iter()
            .map(|pixel| {
                let value = f64::from(pixel.red) * REC2020_LUMA[0]
                    + f64::from(pixel.green) * REC2020_LUMA[1]
                    + f64::from(pixel.blue) * REC2020_LUMA[2];
                [value; 3]
            })
            .collect(),
    );
    let blurred = gaussian_blur(&luminance, 1.0);
    let strength = f64::from(amount) * 0.015;
    let threshold = 0.003;
    for ((pixel, source), low_pass) in image
        .pixels_mut()
        .iter_mut()
        .zip(luminance.pixels())
        .zip(blurred.pixels())
    {
        let detail = source[0] - low_pass[0];
        let thresholded = detail.signum() * (detail.abs() - threshold).max(0.0);
        let adjustment = thresholded * strength;
        pixel.red = finite_f32(f64::from(pixel.red) + adjustment);
        pixel.green = finite_f32(f64::from(pixel.green) + adjustment);
        pixel.blue = finite_f32(f64::from(pixel.blue) + adjustment);
    }
}

fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    let normalized = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    normalized * normalized * (3.0 - 2.0 * normalized)
}
