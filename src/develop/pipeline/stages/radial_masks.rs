//! Analytic radial masks evaluated in global pixel-center coordinates.
//!
//! Persisted F0 masks are a flat sequence and contain no group/combine field.
//! They are therefore applied as independent local-adjustment layers. The four
//! mask algebra operations are implemented and tested here for a future schema
//! that can represent grouping without guessing from IDs or ordering.

use crate::develop::{
    CpuImage, PipelineError, RgbaPixel,
    settings::{LocalAdjustments, RadialMask, RadialMasksSettings},
};

const REC2020_LUMA: [f32; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];

pub(super) fn supports(_settings: &RadialMasksSettings) -> bool {
    true
}

pub(super) fn apply(
    image: &mut CpuImage,
    settings: &RadialMasksSettings,
) -> Result<(), PipelineError> {
    if settings.is_neutral() {
        return Ok(());
    }
    apply_with_processor(image, settings, &BuiltinLocalProcessor)
}

trait LocalAdjustmentProcessor {
    fn process(
        &self,
        source: &CpuImage,
        adjustments: &LocalAdjustments,
    ) -> Result<CpuImage, PipelineError>;
}

fn apply_with_processor(
    image: &mut CpuImage,
    settings: &RadialMasksSettings,
    processor: &dyn LocalAdjustmentProcessor,
) -> Result<(), PipelineError> {
    for mask in settings.masks.iter().filter(|mask| {
        mask.enabled && mask.opacity > 0.0 && mask.adjustments != LocalAdjustments::default()
    }) {
        let adjusted = processor.process(image, &mask.adjustments)?;
        composite_mask(image, &adjusted, mask);
    }
    Ok(())
}

struct BuiltinLocalProcessor;

impl LocalAdjustmentProcessor for BuiltinLocalProcessor {
    fn process(
        &self,
        source: &CpuImage,
        adjustments: &LocalAdjustments,
    ) -> Result<CpuImage, PipelineError> {
        let mut output = source.clone();
        let exposure = (2.0 * adjustments.brightness / 100.0).exp2();
        let contrast = (adjustments.contrast / 50.0).exp2();
        let saturation = 1.0 + adjustments.saturation / 100.0;
        let warmth = adjustments.temperature / 100.0 * 0.10;
        let tint = adjustments.tint / 100.0 * 0.10;
        for pixel in output.pixels_mut() {
            let input = [pixel.red, pixel.green, pixel.blue];
            let mut rgb = input.map(|channel| (channel * exposure - 0.18) * contrast + 0.18);
            let luma = luminance(rgb);
            rgb = rgb.map(|channel| luma + saturation * (channel - luma));
            rgb[0] += warmth + tint * 0.25;
            rgb[1] -= tint * 0.50;
            rgb[2] -= warmth - tint * 0.25;
            pixel.red = rgb[0];
            pixel.green = rgb[1];
            pixel.blue = rgb[2];
        }
        if adjustments.sharpness != 0.0 {
            output = sharpen_luma(&output, adjustments.sharpness / 100.0)?;
        }
        Ok(output)
    }
}

fn sharpen_luma(source: &CpuImage, amount: f32) -> Result<CpuImage, PipelineError> {
    let mut pixels = source.pixels().to_vec();
    for y in 0..source.height() {
        for x in 0..source.width() {
            let center = source.pixels()[index(source.width(), x, y)];
            let mut neighbor_luma = 0.0;
            for (sample_x, sample_y) in [
                (x.saturating_sub(1), y),
                ((x + 1).min(source.width() - 1), y),
                (x, y.saturating_sub(1)),
                (x, (y + 1).min(source.height() - 1)),
            ] {
                neighbor_luma += luminance(pixel_rgb(
                    source.pixels()[index(source.width(), sample_x, sample_y)],
                ));
            }
            let detail = luminance(pixel_rgb(center)) - neighbor_luma * 0.25;
            let delta = detail * amount;
            let output = &mut pixels[index(source.width(), x, y)];
            output.red += delta;
            output.green += delta;
            output.blue += delta;
        }
    }
    CpuImage::new(source.width(), source.height(), pixels).map_err(PipelineError::InvalidImage)
}

fn composite_mask(destination: &mut CpuImage, adjusted: &CpuImage, mask: &RadialMask) {
    debug_assert_eq!(destination.width(), adjusted.width());
    debug_assert_eq!(destination.height(), adjusted.height());
    let width = destination.width();
    let height = destination.height();
    for y in 0..height {
        for x in 0..width {
            let coverage = radial_coverage(mask, width, height, x, y);
            let position = index(width, x, y);
            let target = adjusted.pixels()[position];
            let pixel = &mut destination.pixels_mut()[position];
            pixel.red += coverage * (target.red - pixel.red);
            pixel.green += coverage * (target.green - pixel.green);
            pixel.blue += coverage * (target.blue - pixel.blue);
            // Local develop operations preserve the source's straight alpha.
        }
    }
}

fn radial_coverage(mask: &RadialMask, width: u32, height: u32, x: u32, y: u32) -> f32 {
    let normalized_x = (x as f32 + 0.5) / width as f32;
    let normalized_y = (y as f32 + 0.5) / height as f32;
    let delta_x = (normalized_x - mask.center_x) * width as f32;
    let delta_y = (normalized_y - mask.center_y) * height as f32;
    let radians = mask.rotation_degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let ellipse_x = cos * delta_x + sin * delta_y;
    let ellipse_y = -sin * delta_x + cos * delta_y;
    let radius_x = mask.radius_x * width as f32;
    let radius_y = mask.radius_y * height as f32;
    let distance = ((ellipse_x / radius_x).powi(2) + (ellipse_y / radius_y).powi(2)).sqrt();

    // One physical pixel of analytic edge AA, expressed in ellipse space.
    let aa = 0.5 * ((1.0 / radius_x).powi(2) + (1.0 / radius_y).powi(2)).sqrt();
    let inner = 1.0 - mask.feather;
    let transition = mask.feather.max(2.0 * aa).max(f32::EPSILON);
    let coverage = 1.0 - smoothstep(inner - aa, inner + transition + aa, distance);
    let coverage = if mask.invert {
        1.0 - coverage
    } else {
        coverage
    };
    (coverage * mask.opacity).clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
enum MaskCombine {
    Replace,
    Union,
    Intersect,
    Subtract,
}

#[allow(dead_code)]
fn combine_coverage(accumulated: f32, incoming: f32, operation: MaskCombine) -> f32 {
    let a = accumulated.clamp(0.0, 1.0);
    let b = incoming.clamp(0.0, 1.0);
    match operation {
        MaskCombine::Replace => b,
        MaskCombine::Union => 1.0 - (1.0 - a) * (1.0 - b),
        MaskCombine::Intersect => a * b,
        MaskCombine::Subtract => a * (1.0 - b),
    }
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn luminance(rgb: [f32; 3]) -> f32 {
    rgb[0] * REC2020_LUMA[0] + rgb[1] * REC2020_LUMA[1] + rgb[2] * REC2020_LUMA[2]
}

fn pixel_rgb(pixel: RgbaPixel) -> [f32; 3] {
    [pixel.red, pixel.green, pixel.blue]
}

fn index(width: u32, x: u32, y: u32) -> usize {
    (u64::from(y) * u64::from(width) + u64::from(x)) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mask() -> RadialMask {
        RadialMask {
            id: "test".into(),
            enabled: true,
            center_x: 0.5,
            center_y: 0.5,
            radius_x: 0.3,
            radius_y: 0.2,
            rotation_degrees: 0.0,
            feather: 0.25,
            opacity: 1.0,
            invert: false,
            adjustments: LocalAdjustments::default(),
        }
    }

    #[test]
    fn ellipse_is_centered_rotated_and_feathered() {
        let mut horizontal = mask();
        let center = radial_coverage(&horizontal, 101, 101, 50, 50);
        let along_x = radial_coverage(&horizontal, 101, 101, 75, 50);
        let along_y = radial_coverage(&horizontal, 101, 101, 50, 75);
        assert!(center > 0.999);
        assert!(along_x > along_y);
        assert!(along_x > 0.0 && along_x < 1.0);

        horizontal.rotation_degrees = 90.0;
        assert!(
            radial_coverage(&horizontal, 101, 101, 50, 75)
                > radial_coverage(&horizontal, 101, 101, 75, 50)
        );
    }

    #[test]
    fn invert_opacity_and_global_coordinates_are_stable() {
        let mut value = mask();
        value.opacity = 0.4;
        let regular = radial_coverage(&value, 80, 60, 20, 17);
        value.invert = true;
        let inverted = radial_coverage(&value, 80, 60, 20, 17);
        assert!((regular + inverted - 0.4).abs() < 1.0e-6);
        assert_eq!(
            regular,
            radial_coverage(&value_with_invert(false), 80, 60, 20, 17)
        );
    }

    fn value_with_invert(invert: bool) -> RadialMask {
        let mut value = mask();
        value.opacity = 0.4;
        value.invert = invert;
        value
    }

    #[test]
    fn all_group_algebra_operations_have_soft_coverage_semantics() {
        assert_eq!(combine_coverage(0.25, 0.75, MaskCombine::Replace), 0.75);
        assert_eq!(combine_coverage(0.25, 0.75, MaskCombine::Union), 0.8125);
        assert_eq!(combine_coverage(0.25, 0.75, MaskCombine::Intersect), 0.1875);
        assert_eq!(combine_coverage(0.25, 0.75, MaskCombine::Subtract), 0.0625);
    }

    #[test]
    fn arbitrary_tile_boundaries_use_full_frame_coordinates() {
        let value = mask();
        let value_ref = &value;
        let full = (0..47)
            .flat_map(|y| (0..73).map(move |x| radial_coverage(value_ref, 73, 47, x, y)))
            .collect::<Vec<_>>();
        let mut tiled = Vec::new();
        for y in 0..47 {
            for (start, end) in [(0, 17), (17, 51), (51, 73)] {
                tiled.extend((start..end).map(|x| radial_coverage(&value, 73, 47, x, y)));
            }
        }
        assert_eq!(tiled, full);
    }

    #[test]
    fn callback_processor_is_used_once_per_active_mask() {
        struct White;
        impl LocalAdjustmentProcessor for White {
            fn process(
                &self,
                source: &CpuImage,
                _adjustments: &LocalAdjustments,
            ) -> Result<CpuImage, PipelineError> {
                CpuImage::new(
                    source.width(),
                    source.height(),
                    source
                        .pixels()
                        .iter()
                        .map(|pixel| RgbaPixel::new(1.0, 1.0, 1.0, pixel.alpha()).unwrap())
                        .collect(),
                )
                .map_err(PipelineError::InvalidImage)
            }
        }
        let mut source =
            CpuImage::new(3, 3, vec![RgbaPixel::new(0.0, 0.0, 0.0, 0.7).unwrap(); 9]).unwrap();
        let mut active = mask();
        active.radius_x = 1.0;
        active.radius_y = 1.0;
        active.feather = 0.0;
        active.adjustments.brightness = 1.0;
        apply_with_processor(
            &mut source,
            &RadialMasksSettings {
                masks: vec![active],
            },
            &White,
        )
        .unwrap();
        assert!(source.pixels()[4].red() > 0.99);
        assert_eq!(source.pixels()[4].alpha(), 0.7);
    }
}
