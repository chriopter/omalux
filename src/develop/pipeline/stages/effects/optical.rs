//! Highlight-spread effects in scene-linear light.
//!
//! Conceptual provenance: separable Gaussian filtering, multiscale highlight
//! diffusion, and difference-of-Gaussians halos are standard image-processing
//! techniques also used throughout open-source raw developers such as
//! darktable and RawTherapee. The transfer functions and coefficients here are
//! an independent Grainroom model; no source, LUT, profile, or preset data is
//! copied from another application. Halation is modeled only as the general
//! physical idea of wavelength-biased light spreading in a film-like medium.

use super::spatial::{RgbImage, gaussian_blur, pyramid_blur};
use crate::develop::CpuImage;

const REC2020_LUMA: [f64; 3] = [0.2627, 0.6780, 0.0593];

pub(super) fn apply_bloom(image: &mut CpuImage, amount: f32) {
    if amount == 0.0 {
        return;
    }
    let highlights = highlight_signal(image, 0.68, 0.28);
    let spread = pyramid_blur(&highlights, pyramid_levels(image, 5));
    add_signal(image, &spread, f64::from(amount) * 0.0032, [1.0, 1.0, 1.0]);
}

pub(super) fn apply_halation(image: &mut CpuImage, amount: f32) {
    if amount == 0.0 {
        return;
    }
    let highlights = highlight_signal(image, 0.78, 0.18);
    let narrow = gaussian_blur(&highlights, 1.35);
    let broad = pyramid_blur(&highlights, pyramid_levels(image, 6));
    let halo = RgbImage::from_pixels(
        image.width() as usize,
        image.height() as usize,
        broad
            .pixels()
            .iter()
            .zip(narrow.pixels())
            .map(|(broad, narrow)| {
                [
                    (broad[0] - narrow[0]).max(0.0),
                    (broad[1] - narrow[1]).max(0.0),
                    (broad[2] - narrow[2]).max(0.0),
                ]
            })
            .collect(),
    );
    add_signal(image, &halo, f64::from(amount) * 0.0045, [1.0, 0.32, 0.08]);
}

fn highlight_signal(image: &CpuImage, threshold: f64, knee: f64) -> RgbImage {
    RgbImage::from_cpu(image).map_pixels(|rgb| {
        let positive = [rgb[0].max(0.0), rgb[1].max(0.0), rgb[2].max(0.0)];
        let luminance = dot(positive, REC2020_LUMA);
        if luminance <= 0.0 {
            return [0.0; 3];
        }
        let soft_distance = (luminance - threshold + knee).clamp(0.0, 2.0 * knee);
        let soft_knee = soft_distance * soft_distance / (4.0 * knee.max(f64::EPSILON));
        let extracted = (luminance - threshold).max(soft_knee).max(0.0);
        let scale = extracted / luminance;
        [
            positive[0] * scale,
            positive[1] * scale,
            positive[2] * scale,
        ]
    })
}

fn add_signal(image: &mut CpuImage, signal: &RgbImage, strength: f64, tint: [f64; 3]) {
    debug_assert_eq!(image.width() as usize, signal.width());
    debug_assert_eq!(image.height() as usize, signal.height());
    for (pixel, spread) in image.pixels_mut().iter_mut().zip(signal.pixels()) {
        pixel.red = finite_f32(f64::from(pixel.red) + spread[0] * strength * tint[0]);
        pixel.green = finite_f32(f64::from(pixel.green) + spread[1] * strength * tint[1]);
        pixel.blue = finite_f32(f64::from(pixel.blue) + spread[2] * strength * tint[2]);
    }
}

fn pyramid_levels(image: &CpuImage, maximum: usize) -> usize {
    let shortest = image.width().min(image.height()).max(1);
    ((u32::BITS - shortest.leading_zeros()) as usize).clamp(1, maximum)
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

pub(super) fn finite_f32(value: f64) -> f32 {
    value.clamp(-(f32::MAX as f64), f32::MAX as f64) as f32
}
