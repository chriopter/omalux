//! Sensor noise reduction.
//!
//! Noise is what a photograph has that the scene did not, and above about
//! ISO 1600 it is the first thing anyone notices. It also cannot be dealt with
//! by any of the tonal or colour controls: they are point operations, and a
//! point operation cannot tell a noisy pixel from a detailed one, because the
//! difference is not in the pixel but in how it relates to its neighbours.
//!
//! The filter averages each pixel with the neighbours it plausibly belongs to
//! and ignores the rest — the sigma filter, which is the cheapest way to keep
//! an edge. A blur would remove the noise as well and take the detail with it.
//!
//! Luminance and colour noise are separated because they behave differently
//! and need very different amounts. Colour noise is coarse, ugly, and carries
//! nothing anyone wants to keep, so it can be smoothed hard. Luminance noise
//! sits on the same scale as fine detail, so smoothing it costs texture.

use rayon::prelude::*;

use crate::develop::{CpuImage, RgbaPixel};

const LUMA: [f32; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
/// Half-width of the neighbourhood, in pixels. Seven by seven: noise at the
/// levels this control exists for is coarser than a five-pixel window can
/// average, and the window has to hold enough samples for the mean to settle.
const RADIUS: i32 = 3;
/// Widest difference, relative to the local level, at which a neighbour is
/// still treated as the same subject. Noise varies with brightness, so the
/// threshold is proportional rather than absolute — an absolute one would
/// smooth the shadows into mush while leaving the highlights untouched.
///
/// The window has to be wide enough to hold the noise it is meant to average:
/// set to about one standard deviation it admits only half the neighbours and
/// barely smooths at all. Any real edge — a step far larger than the noise
/// riding on it — stays outside even the widest setting.
/// The window widens with the control. At a low setting only what is
/// unmistakably noise is averaged; at full strength fine texture goes too,
/// which is the trade the control offers and the photographer decides.
const LUMINANCE_TOLERANCE_MIN: f32 = 0.10;
const LUMINANCE_TOLERANCE_MAX: f32 = 0.55;
const COLOUR_TOLERANCE_MIN: f32 = 0.30;
const COLOUR_TOLERANCE_MAX: f32 = 1.20;

fn luminance(pixel: &RgbaPixel) -> f32 {
    pixel.red * LUMA[0] + pixel.green * LUMA[1] + pixel.blue * LUMA[2]
}

/// Smooths noise while leaving edges and detail in place.
///
/// `luminance_amount` and `colour_amount` are percentages. Both are needed:
/// applying one alone leaves the picture either grainy or blotchy.
pub(super) fn apply(image: &mut CpuImage, luminance_amount: f32, colour_amount: f32) {
    let luminance_strength = (luminance_amount / 100.0).clamp(0.0, 1.0);
    let colour_strength = (colour_amount / 100.0).clamp(0.0, 1.0);
    if luminance_strength == 0.0 && colour_strength == 0.0 {
        return;
    }
    let width = image.width() as i32;
    let height = image.height() as i32;
    let source: Vec<RgbaPixel> = image.pixels().to_vec();

    image
        .pixels_mut()
        .par_iter_mut()
        .enumerate()
        .for_each(|(index, pixel)| {
            let x = index as i32 % width;
            let y = index as i32 / width;
            let centre = source[index];
            let centre_luminance = luminance(&centre);

            // The window is placed around the neighbourhood's own level, not
            // around the pixel being filtered. Placed around the pixel, an
            // outlier — a single dark or hot sample, which is what the worst
            // noise looks like — defines a window that admits nothing, so it
            // is the one pixel the filter leaves alone while everything around
            // it is smoothed, and it ends up standing out more than before.
            let mut level_total = 0.0_f32;
            let mut level_count = 0.0_f32;
            for offset_y in -RADIUS..=RADIUS {
                let sample_y = y + offset_y;
                if sample_y < 0 || sample_y >= height {
                    continue;
                }
                for offset_x in -RADIUS..=RADIUS {
                    let sample_x = x + offset_x;
                    if sample_x < 0 || sample_x >= width {
                        continue;
                    }
                    level_total += luminance(&source[(sample_y * width + sample_x) as usize]);
                    level_count += 1.0;
                }
            }
            let level = (level_total / level_count.max(1.0)).max(0.0);

            let luminance_tolerance = LUMINANCE_TOLERANCE_MIN
                + luminance_strength * (LUMINANCE_TOLERANCE_MAX - LUMINANCE_TOLERANCE_MIN);
            let colour_tolerance = COLOUR_TOLERANCE_MIN
                + colour_strength * (COLOUR_TOLERANCE_MAX - COLOUR_TOLERANCE_MIN);
            let luminance_window = luminance_tolerance * (level + 0.05);
            let colour_window = colour_tolerance * (level + 0.05);

            let mut luminance_total = 0.0_f32;
            let mut luminance_weight = 0.0_f32;
            let mut colour_total = [0.0_f32; 3];
            let mut colour_weight = 0.0_f32;

            for offset_y in -RADIUS..=RADIUS {
                let sample_y = y + offset_y;
                if sample_y < 0 || sample_y >= height {
                    continue;
                }
                for offset_x in -RADIUS..=RADIUS {
                    let sample_x = x + offset_x;
                    if sample_x < 0 || sample_x >= width {
                        continue;
                    }
                    let neighbour = source[(sample_y * width + sample_x) as usize];
                    let neighbour_luminance = luminance(&neighbour);
                    let difference = (neighbour_luminance - level).abs();
                    if difference <= luminance_window {
                        luminance_total += neighbour_luminance;
                        luminance_weight += 1.0;
                    }
                    // Colour is gathered over a wider window: a neighbour that
                    // differs in brightness usually still belongs to the same
                    // coloured subject, and colour noise is far coarser than
                    // luminance noise, so it needs a larger sample to average
                    // out at all. Ratios are only meaningful above black.
                    if difference <= colour_window && neighbour_luminance > 1.0e-3 {
                        colour_total[0] += neighbour.red / neighbour_luminance;
                        colour_total[1] += neighbour.green / neighbour_luminance;
                        colour_total[2] += neighbour.blue / neighbour_luminance;
                        colour_weight += 1.0;
                    }
                }
            }

            let mut result = [centre.red, centre.green, centre.blue];

            if colour_weight > 1.0 && colour_strength > 0.0 && centre_luminance.abs() > 1.0e-4 {
                // Colour is smoothed as a ratio to brightness, so the pixel
                // keeps its own luminance and only its hue is drawn towards
                // its neighbours'. Smoothing the channels directly would blur
                // detail through the colour path as well.
                let smoothed = [
                    colour_total[0] / colour_weight * centre_luminance,
                    colour_total[1] / colour_weight * centre_luminance,
                    colour_total[2] / colour_weight * centre_luminance,
                ];
                for channel in 0..3 {
                    result[channel] += colour_strength * (smoothed[channel] - result[channel]);
                }
            }

            if luminance_weight > 1.0 && luminance_strength > 0.0 {
                let smoothed = luminance_total / luminance_weight;
                let current = result[0] * LUMA[0] + result[1] * LUMA[1] + result[2] * LUMA[2];
                let target = current + luminance_strength * (smoothed - current);
                if current > 1.0e-4 {
                    let scale = target / current;
                    for value in &mut result {
                        *value *= scale;
                    }
                } else {
                    // A pixel at or below black has no ratios to scale; it is
                    // simply set to the neighbourhood's level as neutral grey,
                    // which is the only colour a sample carrying no signal can
                    // honestly claim.
                    result = [target.max(0.0); 3];
                }
            }

            pixel.red = result[0];
            pixel.green = result[1];
            pixel.blue = result[2];
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_from(values: &[[f32; 3]], width: u32) -> CpuImage {
        let pixels: Vec<RgbaPixel> = values
            .iter()
            .map(|value| RgbaPixel::new(value[0], value[1], value[2], 1.0).unwrap())
            .collect();
        let height = pixels.len() as u32 / width;
        CpuImage::new(width, height, pixels).unwrap()
    }

    #[test]
    fn noise_on_a_flat_field_is_reduced() {
        // A flat grey with alternating error, which is what sensor noise looks
        // like where the subject has no detail of its own.
        let values: Vec<[f32; 3]> = (0..64)
            .map(|index| {
                let wobble = if index % 2 == 0 { 0.02 } else { -0.02 };
                [0.30 + wobble, 0.30 + wobble, 0.30 + wobble]
            })
            .collect();
        let mut image = image_from(&values, 8);
        let before = spread(&image);
        apply(&mut image, 100.0, 100.0);
        let after = spread(&image);
        assert!(after < before * 0.5, "spread went {before} -> {after}");
    }

    #[test]
    fn an_edge_survives() {
        // Half dark, half light. A blur would pull the two halves together;
        // the filter must leave the step where it is.
        let values: Vec<[f32; 3]> = (0..64)
            .map(|index| {
                let level = if index % 8 < 4 { 0.15 } else { 0.75 };
                [level, level, level]
            })
            .collect();
        let mut image = image_from(&values, 8);
        apply(&mut image, 100.0, 100.0);
        let pixels = image.pixels();
        assert!((pixels[0].red - 0.15).abs() < 0.02, "dark side moved");
        assert!((pixels[7].red - 0.75).abs() < 0.02, "light side moved");
    }

    #[test]
    fn zero_amounts_change_nothing() {
        let values: Vec<[f32; 3]> = (0..16)
            .map(|index| [index as f32 / 16.0, 0.4, 0.7])
            .collect();
        let mut image = image_from(&values, 4);
        let original = image.pixels().to_vec();
        apply(&mut image, 0.0, 0.0);
        assert_eq!(image.pixels(), original.as_slice());
    }

    #[test]
    fn colour_noise_is_removed_without_flattening_brightness() {
        // Same brightness throughout, colour swinging between two casts: the
        // colour must converge while the brightness pattern stays.
        let values: Vec<[f32; 3]> = (0..64)
            .map(|index| {
                if index % 2 == 0 {
                    [0.42, 0.30, 0.18]
                } else {
                    [0.18, 0.30, 0.42]
                }
            })
            .collect();
        let mut image = image_from(&values, 8);
        apply(&mut image, 0.0, 100.0);
        let pixels = image.pixels();
        let first = pixels[0].red - pixels[0].blue;
        let second = pixels[1].red - pixels[1].blue;
        assert!(first.abs() < 0.08, "colour cast remained: {first}");
        assert!(second.abs() < 0.08, "colour cast remained: {second}");
    }

    fn spread(image: &CpuImage) -> f32 {
        let values: Vec<f32> = image.pixels().iter().map(luminance).collect();
        let mean = values.iter().sum::<f32>() / values.len() as f32;
        (values.iter().map(|v| (v - mean) * (v - mean)).sum::<f32>() / values.len() as f32).sqrt()
    }
}
