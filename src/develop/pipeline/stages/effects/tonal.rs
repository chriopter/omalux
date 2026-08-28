//! Global tonal and detail effects in the normative scene-linear working space.
//!
//! Fade, radial vignette, and thresholded luminance unsharp-mask formulas were
//! independently specified for Omalux from their general photographic and
//! signal-processing definitions. No upstream or proprietary implementation,
//! coefficients, LUT, preset, or camera profile was consulted or copied.

use super::super::spatial::{Plane, finite_f32, gaussian_blur, reflect101};
use crate::{
    develop::{CpuImage, PipelineError},
    io::LimitError,
};

const REC2020_LUMA: [f64; 3] = [0.2627, 0.6780, 0.0593];

/// Matte print fade.
///
/// A faded print has a real density floor and ceiling: its blacks never reach
/// zero and its whites never reach paper white. Those limits are properties of
/// the print, so they are defined in display-referred terms and applied in the
/// display encoding. A scene-linear lift and contrast scale cannot express
/// them: any value above the display maximum still clips to white afterwards,
/// which is exactly the hard, patchy highlight this effect must avoid.
///
/// At full strength the floor rises to 4% and the ceiling drops to 94% of the
/// display range; both move proportionally with the slider. Signed and HDR
/// values pass through the same odd-symmetric encoding, so the mapping stays
/// defined and monotone outside `[0,1]`.
pub(super) fn apply_fade(image: &mut CpuImage, amount: f32) {
    if amount == 0.0 {
        return;
    }
    let normalized = f64::from(amount) / 100.0;
    let floor = 0.04 * normalized;
    let ceiling = 1.0 - 0.06 * normalized;
    let span = ceiling - floor;
    let fade_channel = |value: f32| -> f32 {
        let encoded = srgb_encode_signed(f64::from(value));
        finite_f32(srgb_decode_signed(floor + saturate(encoded) * span))
    };
    for pixel in image.pixels_mut() {
        pixel.red = fade_channel(pixel.red);
        pixel.green = fade_channel(pixel.green);
        pixel.blue = fade_channel(pixel.blue);
    }
}

/// Maps the encoded range `[0, inf)` into `[0, 1)`: identity below the knee,
/// then an exponential approach to one. Without it a scene value far above
/// display white would still leave the ceiling behind and clip, which is the
/// failure this effect exists to prevent. Odd symmetry keeps signed values
/// defined and monotone.
fn saturate(value: f64) -> f64 {
    const KNEE: f64 = 0.7;
    let magnitude = value.abs();
    let saturated = if magnitude <= KNEE {
        magnitude
    } else {
        let headroom = 1.0 - KNEE;
        KNEE + headroom * (1.0 - (-(magnitude - KNEE) / headroom).exp())
    };
    value.signum() * saturated
}

fn srgb_encode_signed(value: f64) -> f64 {
    let magnitude = value.abs();
    let encoded = if magnitude <= 0.003_130_8 {
        12.92 * magnitude
    } else {
        1.055 * magnitude.powf(1.0 / 2.4) - 0.055
    };
    value.signum() * encoded
}

fn srgb_decode_signed(value: f64) -> f64 {
    let magnitude = value.abs();
    let decoded = if magnitude <= 0.040_45 {
        magnitude / 12.92
    } else {
        ((magnitude + 0.055) / 1.055).powf(2.4)
    };
    value.signum() * decoded
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

pub(super) fn apply_sharpness(image: &mut CpuImage, amount: f32) -> Result<(), PipelineError> {
    if amount == 0.0 {
        return Ok(());
    }
    let width = image.width() as usize;
    let height = image.height() as usize;
    let mut luminance_pixels = Vec::new();
    luminance_pixels
        .try_reserve_exact(image.pixels().len())
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    for pixel in image.pixels() {
        let value = f64::from(pixel.red) * REC2020_LUMA[0]
            + f64::from(pixel.green) * REC2020_LUMA[1]
            + f64::from(pixel.blue) * REC2020_LUMA[2];
        luminance_pixels.push(finite_f32(value));
    }
    let luminance = Plane::new(width, height, luminance_pixels);
    let blurred = gaussian_blur(&luminance, 1.0)?;
    let strength = f64::from(amount) * 0.015;
    let threshold = 0.003;
    for ((pixel, source), low_pass) in image
        .pixels_mut()
        .iter_mut()
        .zip(luminance.pixels())
        .zip(blurred.pixels())
    {
        let detail = f64::from(*source) - f64::from(*low_pass);
        let thresholded = detail.signum() * (detail.abs() - threshold).max(0.0);
        let adjustment = thresholded * strength;
        pixel.red = finite_f32(f64::from(pixel.red) + adjustment);
        pixel.green = finite_f32(f64::from(pixel.green) + adjustment);
        pixel.blue = finite_f32(f64::from(pixel.blue) + adjustment);
    }
    Ok(())
}

pub(super) fn sharpness_delta_at<F>(
    extent: [usize; 2],
    point: [usize; 2],
    amount: f32,
    kernel: &[f64],
    horizontal: &mut [f32],
    mut sample_luminance: F,
) -> f64
where
    F: FnMut(usize, usize) -> f32,
{
    let [width, height] = extent;
    let [x, y] = point;
    if amount == 0.0 {
        return 0.0;
    }
    let radius = kernel.len() / 2;
    debug_assert_eq!(horizontal.len(), kernel.len());
    for (offset_y, horizontal_value) in horizontal.iter_mut().enumerate() {
        let sample_y = reflect101(y as isize + offset_y as isize - radius as isize, height);
        let mut sum = 0.0;
        for (offset_x, weight) in kernel.iter().copied().enumerate() {
            let sample_x = reflect101(x as isize + offset_x as isize - radius as isize, width);
            sum += f64::from(sample_luminance(sample_x, sample_y)) * weight;
        }
        *horizontal_value = finite_f32(sum);
    }
    let low_pass = horizontal
        .iter()
        .copied()
        .zip(kernel.iter().copied())
        .map(|(sample, weight)| f64::from(sample) * weight)
        .sum::<f64>();
    let source = f64::from(sample_luminance(x, y));
    let detail = source - low_pass;
    let thresholded = detail.signum() * (detail.abs() - 0.003).max(0.0);
    thresholded * f64::from(amount) * 0.015
}

fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    let normalized = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    normalized * normalized * (3.0 - 2.0 * normalized)
}
