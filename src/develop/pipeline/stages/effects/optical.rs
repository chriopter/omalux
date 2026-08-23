//! Independently derived highlight-spread effects in scene-linear light.
//!
//! These formulas were written from the general definitions of normalized
//! Gaussian convolution, a continuous quadratic soft knee, and a positive
//! broad-minus-narrow ring. No upstream implementation, LUT, camera profile,
//! preset, or proprietary model was consulted or copied. Names of open-source
//! raw developers in project planning are conceptual context only; this module
//! therefore has no upstream code provenance or inherited license obligation.

use super::super::spatial::{Plane, finite_f32, pyramid_blur};
use crate::{
    develop::{CpuImage, PipelineError},
    io::LimitError,
};

const REC2020_LUMA: [f64; 3] = [0.2627, 0.6780, 0.0593];

pub(super) fn apply_bloom(image: &mut CpuImage, amount: f32) -> Result<(), PipelineError> {
    if amount == 0.0 {
        return Ok(());
    }
    let width = image.width() as usize;
    let height = image.height() as usize;
    let mut extraction = try_f32_buffer(image.pixels().len())?;
    for pixel in image.pixels() {
        let rgb = [
            f64::from(pixel.red()).max(0.0),
            f64::from(pixel.green()).max(0.0),
            f64::from(pixel.blue()).max(0.0),
        ];
        let luminance = dot(rgb, REC2020_LUMA);
        extraction.push(soft_knee_scale(luminance, 0.68, 0.28));
    }
    let sigma = normalized_sigma(width, height, amount, 0.006, 0.018);
    let strength = f64::from(amount) * 0.0032;

    // One color plane at a time bounds full-resolution memory to extraction,
    // source, and output scalar planes (12 B/pixel), plus reusable tile scratch.
    for channel in 0..3 {
        let mut source_pixels = try_f32_buffer(image.pixels().len())?;
        for (pixel, scale) in image.pixels().iter().zip(&extraction) {
            let value = match channel {
                0 => pixel.red(),
                1 => pixel.green(),
                _ => pixel.blue(),
            };
            source_pixels.push(finite_f32(f64::from(value).max(0.0) * f64::from(*scale)));
        }
        let source = Plane::new(width, height, source_pixels);
        let spread = pyramid_blur(source, sigma)?;
        for (pixel, spread) in image.pixels_mut().iter_mut().zip(spread.pixels()) {
            let target = match channel {
                0 => &mut pixel.red,
                1 => &mut pixel.green,
                _ => &mut pixel.blue,
            };
            *target = finite_f32(f64::from(*target) + f64::from(*spread) * strength);
        }
    }
    Ok(())
}

pub(super) fn apply_halation(image: &mut CpuImage, amount: f32) -> Result<(), PipelineError> {
    if amount == 0.0 {
        return Ok(());
    }
    let width = image.width() as usize;
    let height = image.height() as usize;
    // Halation energy is scalar Rec.2020 luminance, not a red-channel proxy.
    // Consequently pure green and blue highlights also produce a warm ring.
    let mut source_pixels = try_f32_buffer(image.pixels().len())?;
    for pixel in image.pixels() {
        let rgb = [
            f64::from(pixel.red()).max(0.0),
            f64::from(pixel.green()).max(0.0),
            f64::from(pixel.blue()).max(0.0),
        ];
        let luminance = dot(rgb, REC2020_LUMA);
        source_pixels.push(finite_f32(
            luminance * f64::from(soft_knee_scale(luminance, 0.78, 0.18)),
        ));
    }
    let source = Plane::new(width, height, source_pixels);
    let narrow_sigma = normalized_sigma(width, height, amount, 0.0035, 0.0045);
    let broad_sigma = normalized_sigma(width, height, amount, 0.016, 0.030);
    let narrow = pyramid_blur(source.try_clone()?, narrow_sigma)?;
    let broad = pyramid_blur(source, broad_sigma)?;
    let strength = f64::from(amount) * 0.0045;
    let warm = [1.0, 0.32, 0.08];
    for ((pixel, broad), narrow) in image
        .pixels_mut()
        .iter_mut()
        .zip(broad.pixels())
        .zip(narrow.pixels())
    {
        let ring = (f64::from(*broad) - f64::from(*narrow)).max(0.0) * strength;
        pixel.red = finite_f32(f64::from(pixel.red) + ring * warm[0]);
        pixel.green = finite_f32(f64::from(pixel.green) + ring * warm[1]);
        pixel.blue = finite_f32(f64::from(pixel.blue) + ring * warm[2]);
    }
    Ok(())
}

fn try_f32_buffer(length: usize) -> Result<Vec<f32>, PipelineError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    Ok(values)
}

fn normalized_sigma(width: usize, height: usize, amount: f32, base: f64, span: f64) -> f64 {
    let short_extent = width.min(height).max(1) as f64;
    short_extent * (base + span * f64::from(amount) / 100.0)
}

fn soft_knee_scale(luminance: f64, threshold: f64, knee: f64) -> f32 {
    if luminance <= 0.0 {
        return 0.0;
    }
    let distance = (luminance - threshold + knee).clamp(0.0, 2.0 * knee);
    let soft = distance * distance / (4.0 * knee.max(f64::EPSILON));
    let extracted = (luminance - threshold).max(soft).max(0.0);
    finite_f32(extracted / luminance)
}

fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
    left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_knee_is_continuous_monotonic_and_zero_below_foot() {
        let samples = (0..=400)
            .map(|index| index as f64 / 200.0)
            .map(|luminance| soft_knee_scale(luminance, 0.68, 0.28))
            .collect::<Vec<_>>();
        assert_eq!(samples[0], 0.0);
        for pair in samples.windows(2) {
            assert!(pair[1] >= pair[0] - 1e-6);
        }
        let left = soft_knee_scale(0.68 - 1e-7, 0.68, 0.28);
        let right = soft_knee_scale(0.68 + 1e-7, 0.68, 0.28);
        assert!((right - left).abs() < 1e-5);
    }
}
