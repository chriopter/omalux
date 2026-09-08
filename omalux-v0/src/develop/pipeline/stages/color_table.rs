//! Three-dimensional colour table.
//!
//! The colour mixer divides the hue circle into eight bands and moves each one
//! as a whole. That cannot express a rendering whose treatment of a colour
//! depends on how light it is — a deep sky staying blue while a hazy one turns
//! towards cyan is a single decision in hue but two different ones in
//! lightness. A table indexed by the whole triple can hold both, which is why
//! every renderer of this kind offers one.
//!
//! The table is applied to display-encoded coordinates. Photographic colour
//! renderings are authored that way, and the standard interchange formats
//! carry them that way, so a table exported from another tool means here what
//! it meant there.

use rayon::prelude::*;

use crate::develop::{CpuImage, DevelopStage, PipelineError, settings::ColorTableSettings};

/// Maps a linear working value to the display-encoded coordinate the table is
/// indexed by, and back. Values outside the unit range pass through the table
/// unchanged rather than being clamped into it: a scene-referred highlight
/// still carries detail that a display-referred table has no opinion about.
fn encode(value: f32) -> f32 {
    if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    }
}

fn decode(value: f32) -> f32 {
    if value <= 0.040_45 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

pub(super) fn supports(settings: &ColorTableSettings) -> bool {
    settings.is_neutral() || settings.is_well_formed()
}

pub(super) fn apply(
    image: &mut CpuImage,
    settings: &ColorTableSettings,
) -> Result<(), PipelineError> {
    if settings.is_neutral() {
        return Ok(());
    }
    if !settings.is_well_formed() {
        return Err(PipelineError::NumericFailure {
            stage: DevelopStage::ColorTable,
            reason: "colour table size does not match its entries",
        });
    }
    let size = settings.size as usize;
    let strength = (settings.strength / 100.0).clamp(0.0, 1.0);
    image.pixels_mut().par_iter_mut().for_each(|pixel| {
        let source = [pixel.red, pixel.green, pixel.blue];
        // The working space is scene-referred, so highlights routinely sit
        // above the table's domain. Treating those pixels differently — by
        // skipping them, or by fading the correction out over a margin — puts
        // a boundary into the picture wherever brightness crosses the
        // threshold gradually, and in a sky that boundary follows a line of
        // equal brightness and reads as an arc across the frame.
        //
        // So the lookup simply saturates: a channel above white is looked up
        // as white. The correction is then applied as a ratio rather than a
        // replacement, which keeps a highlight's own brightness intact instead
        // of folding the whole range above white onto one entry. Both the
        // lookup and the ratio are continuous across the boundary, so no edge
        // can appear there.
        let encoded = source.map(|value| encode(value.clamp(0.0, 1.0)));
        let mapped = sample(&settings.entries, size, encoded);
        let blended = [0, 1, 2].map(|channel| {
            let target = decode(encoded[channel] + strength * (mapped[channel] - encoded[channel]));
            let reference = decode(encoded[channel]);
            if reference > 1.0e-6 && source[channel] > 1.0 {
                source[channel] * (target / reference)
            } else {
                target + (source[channel] - reference)
            }
        });
        pixel.red = blended[0];
        pixel.green = blended[1];
        pixel.blue = blended[2];
    });
    Ok(())
}

/// Trilinear lookup. The table is stored red-major, so the red index strides
/// furthest, matching the ordering the interchange formats use.
fn sample(entries: &[f32], size: usize, coordinate: [f32; 3]) -> [f32; 3] {
    let last = size - 1;
    let mut base = [0_usize; 3];
    let mut fraction = [0.0_f32; 3];
    for axis in 0..3 {
        let position = coordinate[axis].clamp(0.0, 1.0) * last as f32;
        let floor = position.floor();
        base[axis] = (floor as usize).min(last.saturating_sub(1));
        fraction[axis] = position - base[axis] as f32;
    }
    let mut result = [0.0_f32; 3];
    for corner in 0..8 {
        let offsets = [corner & 1, (corner >> 1) & 1, (corner >> 2) & 1];
        let mut weight = 1.0_f32;
        let mut index = 0_usize;
        for axis in 0..3 {
            let step = base[axis] + offsets[axis];
            weight *= if offsets[axis] == 1 {
                fraction[axis]
            } else {
                1.0 - fraction[axis]
            };
            index = index * size + step.min(last);
        }
        if weight == 0.0 {
            continue;
        }
        for channel in 0..3 {
            result[channel] += weight * entries[index * 3 + channel];
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::RgbaPixel;

    fn identity(size: u32) -> ColorTableSettings {
        let last = (size - 1) as f32;
        let mut entries = Vec::with_capacity((size as usize).pow(3) * 3);
        for red in 0..size {
            for green in 0..size {
                for blue in 0..size {
                    entries.push(red as f32 / last);
                    entries.push(green as f32 / last);
                    entries.push(blue as f32 / last);
                }
            }
        }
        ColorTableSettings {
            size,
            entries,
            strength: 100.0,
        }
    }

    #[test]
    fn an_identity_table_leaves_every_colour_where_it_was() {
        let settings = identity(9);
        let pixels: Vec<_> = (0..64)
            .map(|index| {
                let unit = index as f32 / 63.0;
                RgbaPixel::new(unit, 1.0 - unit, (unit * 3.0).fract(), 1.0).unwrap()
            })
            .collect();
        let original = pixels.clone();
        let mut image = CpuImage::new(64, 1, pixels).unwrap();
        apply(&mut image, &settings).unwrap();
        for (after, before) in image.pixels().iter().zip(&original) {
            for (a, b) in [
                (after.red, before.red),
                (after.green, before.green),
                (after.blue, before.blue),
            ] {
                assert!((a - b).abs() < 2.0e-3, "{a} moved from {b}");
            }
        }
    }

    #[test]
    fn strength_scales_the_move_and_zero_is_neutral() {
        let mut settings = identity(9);
        // A table that sends everything to mid grey makes the blend visible.
        for entry in settings.entries.iter_mut() {
            *entry = 0.5;
        }
        let source = RgbaPixel::new(0.8, 0.2, 0.4, 1.0).unwrap();

        settings.strength = 0.0;
        let mut image = CpuImage::new(1, 1, vec![source]).unwrap();
        apply(&mut image, &settings).unwrap();
        assert_eq!(image.pixels()[0], source);

        settings.strength = 100.0;
        let mut image = CpuImage::new(1, 1, vec![source]).unwrap();
        apply(&mut image, &settings).unwrap();
        let full = image.pixels()[0];
        assert!((full.red - decode(0.5)).abs() < 1.0e-5);

        settings.strength = 50.0;
        let mut image = CpuImage::new(1, 1, vec![source]).unwrap();
        apply(&mut image, &settings).unwrap();
        let half = image.pixels()[0];
        let expected = decode(encode(0.8) + 0.5 * (0.5 - encode(0.8)));
        assert!((half.red - expected).abs() < 1.0e-5);
    }

    #[test]
    fn a_highlight_above_white_keeps_its_own_brightness() {
        let mut settings = identity(9);
        for entry in settings.entries.iter_mut() {
            *entry = 0.5;
        }
        let source = RgbaPixel::new(2.5, 0.2, 0.2, 0.75).unwrap();

        // An identity table has nothing to say about a highlight, and must
        // leave it where it was rather than clamping it to white.
        let mut image = CpuImage::new(1, 1, vec![source]).unwrap();
        apply(&mut image, &identity(9)).unwrap();
        assert!((image.pixels()[0].red - 2.5).abs() < 5.0e-3);

        // A table that darkens everything darkens the highlight too, but the
        // highlight stays the brightest channel by a wide margin instead of
        // being folded onto the same entry as the others.
        let mut image = CpuImage::new(1, 1, vec![source]).unwrap();
        apply(&mut image, &settings).unwrap();
        let out = image.pixels()[0];
        assert!(out.red < 2.5);
        assert!(out.red > out.green * 2.0, "{} vs {}", out.red, out.green);
        assert_eq!(out.alpha, 0.75);
    }

    #[test]
    fn the_correction_is_continuous_across_the_domain_edge() {
        // A step here draws an arc across any gradient that crosses the
        // boundary, so neighbouring values must stay close together.
        let mut settings = identity(9);
        for entry in settings.entries.iter_mut() {
            *entry = 0.5;
        }
        let mut previous: Option<f32> = None;
        for step in 0..60 {
            let value = 0.90 + step as f32 * 0.01;
            let source = RgbaPixel::new(value, 0.2, 0.2, 1.0).unwrap();
            let mut image = CpuImage::new(1, 1, vec![source]).unwrap();
            apply(&mut image, &settings).unwrap();
            let green = image.pixels()[0].green;
            if let Some(last) = previous {
                assert!(
                    (green - last).abs() < 5.0e-3,
                    "the result jumped at {value}: {last} to {green}"
                );
            }
            previous = Some(green);
        }
    }

    #[test]
    fn a_malformed_table_is_reported_rather_than_applied() {
        let settings = ColorTableSettings {
            size: 9,
            entries: vec![0.0; 10],
            strength: 100.0,
        };
        let mut image =
            CpuImage::new(1, 1, vec![RgbaPixel::new(0.5, 0.5, 0.5, 1.0).unwrap()]).unwrap();
        assert!(apply(&mut image, &settings).is_err());
        assert!(!supports(&settings));
    }
}
