//! Analytic radial masks evaluated in global pixel-center coordinates.
//!
//! Persisted F0 masks are a flat sequence and contain no group/combine field.
//! They are therefore applied as independent local-adjustment layers. The four
//! mask algebra operations are implemented and tested here for a future schema
//! that can represent grouping without guessing from IDs or ordering.

use super::{
    basics::PreparedBasics,
    effects::{add_finite_delta, local_sharpness_delta, local_sharpness_kernel},
};
use crate::develop::{
    CpuImage, DevelopStage, PipelineError, RgbaPixel,
    settings::{LocalAdjustments, RadialMask, RadialMasksSettings},
};
use crate::io::LimitError;

const REC2020_LUMA: [f64; 3] = [0.2627, 0.6780, 0.0593];

pub(super) fn supports(settings: &RadialMasksSettings) -> bool {
    settings
        .masks
        .iter()
        .all(|mask| !mask.enabled || mask.opacity == 0.0 || mask.adjustments.sharpness >= 0.0)
}

pub(super) fn apply(
    image: &mut CpuImage,
    settings: &RadialMasksSettings,
) -> Result<(), PipelineError> {
    if settings.is_neutral() {
        return Ok(());
    }
    if !supports(settings) {
        return Err(PipelineError::StageNotImplemented(
            DevelopStage::RadialMasks,
        ));
    }
    apply_with_processor(image, settings, &BuiltinLocalProcessor)
}

trait LocalAdjustmentProcessor {
    fn process_region(
        &self,
        source: &CpuImage,
        adjustments: &LocalAdjustments,
        region: Region,
    ) -> Result<Vec<RgbaPixel>, PipelineError>;
}

fn apply_with_processor(
    image: &mut CpuImage,
    settings: &RadialMasksSettings,
    processor: &dyn LocalAdjustmentProcessor,
) -> Result<(), PipelineError> {
    for mask in settings.masks.iter().filter(|mask| {
        mask.enabled && mask.opacity > 0.0 && mask.adjustments != LocalAdjustments::default()
    }) {
        let region = mask_region(mask, image.width(), image.height());
        let adjusted = processor.process_region(image, &mask.adjustments, region)?;
        composite_region(image, &adjusted, mask, region);
    }
    Ok(())
}

struct BuiltinLocalProcessor;

impl LocalAdjustmentProcessor for BuiltinLocalProcessor {
    fn process_region(
        &self,
        source: &CpuImage,
        adjustments: &LocalAdjustments,
        region: Region,
    ) -> Result<Vec<RgbaPixel>, PipelineError> {
        let prepared = PreparedBasics::from_local(adjustments);
        let width = source.width() as usize;
        let height = source.height() as usize;
        let mut output = Vec::new();
        output
            .try_reserve_exact(region.pixel_count())
            .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
        let kernel = local_sharpness_kernel();
        let mut scratch = [0.0_f32; 7];
        for y in region.y..region.y + region.height {
            for x in region.x..region.x + region.width {
                let mut target = source.pixels()[index(source.width(), x, y)];
                prepared.apply_pixel(&mut target);
                if adjustments.sharpness > 0.0 {
                    let delta = local_sharpness_delta(
                        [width, height],
                        [x as usize, y as usize],
                        adjustments.sharpness,
                        &kernel,
                        &mut scratch,
                        |sample_x, sample_y| {
                            let mut sample = source.pixels()
                                [index(source.width(), sample_x as u32, sample_y as u32)];
                            prepared.apply_pixel(&mut sample);
                            luminance(pixel_rgb(sample))
                        },
                    );
                    target.red = add_finite_delta(target.red, delta);
                    target.green = add_finite_delta(target.green, delta);
                    target.blue = add_finite_delta(target.blue, delta);
                }
                output.push(target);
            }
        }
        Ok(output)
    }
}

/// Exact requested heap payload for the largest active ROI. Masks are
/// processed sequentially; analytic coverage and the normative 7-tap local
/// sharpness kernel use stack storage and allocate no mask or halo plane.
pub(super) fn scratch_bytes(
    width: u32,
    height: u32,
    settings: &RadialMasksSettings,
) -> Result<u64, PipelineError> {
    let largest_roi_pixels = settings
        .masks
        .iter()
        .filter(|mask| {
            mask.enabled && mask.opacity > 0.0 && mask.adjustments != LocalAdjustments::default()
        })
        .map(|mask| {
            let region = mask_region(mask, width, height);
            u64::from(region.width) * u64::from(region.height)
        })
        .max()
        .unwrap_or(0);
    largest_roi_pixels
        .checked_mul(16)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))
}

fn composite_region(
    destination: &mut CpuImage,
    adjusted: &[RgbaPixel],
    mask: &RadialMask,
    region: Region,
) {
    let width = destination.width();
    let height = destination.height();
    for y in region.y..region.y + region.height {
        for x in region.x..region.x + region.width {
            let coverage = radial_coverage(mask, width, height, x, y);
            let position = index(width, x, y);
            let local = ((y - region.y) * region.width + x - region.x) as usize;
            let target = adjusted[local];
            let pixel = &mut destination.pixels_mut()[position];
            if coverage == 1.0 {
                pixel.red = target.red;
                pixel.green = target.green;
                pixel.blue = target.blue;
            } else if coverage > 0.0 {
                pixel.red += coverage * (target.red - pixel.red);
                pixel.green += coverage * (target.green - pixel.green);
                pixel.blue += coverage * (target.blue - pixel.blue);
            }
            // Local develop operations preserve the source's straight alpha.
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Region {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

impl Region {
    fn pixel_count(self) -> usize {
        usize::try_from(u64::from(self.width) * u64::from(self.height))
            .expect("region lies within a validated image")
    }
}

fn mask_region(mask: &RadialMask, width: u32, height: u32) -> Region {
    if mask.invert {
        return Region {
            x: 0,
            y: 0,
            width,
            height,
        };
    }
    let radians = mask.rotation_degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let radius_x = mask.radius_x * width as f32;
    let radius_y = mask.radius_y * height as f32;
    let extent_x = ((radius_x * cos).powi(2) + (radius_y * sin).powi(2)).sqrt() + 1.0;
    let extent_y = ((radius_x * sin).powi(2) + (radius_y * cos).powi(2)).sqrt() + 1.0;
    let center_x = mask.center_x * width as f32;
    let center_y = mask.center_y * height as f32;
    let left = (center_x - extent_x).floor().max(0.0) as u32;
    let top = (center_y - extent_y).floor().max(0.0) as u32;
    let right = (center_x + extent_x).ceil().min(width as f32) as u32;
    let bottom = (center_y + extent_y).ceil().min(height as f32) as u32;
    Region {
        x: left,
        y: top,
        width: right.saturating_sub(left),
        height: bottom.saturating_sub(top),
    }
}

fn radial_coverage(mask: &RadialMask, width: u32, height: u32, x: u32, y: u32) -> f32 {
    radial_coverage_at(mask, width, height, x as f32 + 0.5, y as f32 + 0.5)
}

fn radial_coverage_at(
    mask: &RadialMask,
    width: u32,
    height: u32,
    pixel_center_x: f32,
    pixel_center_y: f32,
) -> f32 {
    let delta_x = pixel_center_x - mask.center_x * width as f32;
    let delta_y = pixel_center_y - mask.center_y * height as f32;
    let radians = mask.rotation_degrees.to_radians();
    let (sin, cos) = radians.sin_cos();
    let ellipse_x = cos * delta_x + sin * delta_y;
    let ellipse_y = -sin * delta_x + cos * delta_y;
    let radius_x = mask.radius_x * width as f32;
    let radius_y = mask.radius_y * height as f32;
    let distance = ((ellipse_x / radius_x).powi(2) + (ellipse_y / radius_y).powi(2)).sqrt();

    let implicit_gradient_x =
        cos * ellipse_x / radius_x.powi(2) - sin * ellipse_y / radius_y.powi(2);
    let implicit_gradient_y =
        sin * ellipse_x / radius_x.powi(2) + cos * ellipse_y / radius_y.powi(2);
    let implicit_gradient = implicit_gradient_x.hypot(implicit_gradient_y);
    const DISTANCE_EPSILON: f32 = 1.0e-12;
    let distance_gradient = if distance > DISTANCE_EPSILON {
        implicit_gradient / distance
    } else {
        0.0
    };
    let coverage = if distance <= DISTANCE_EPSILON {
        1.0
    } else if mask.feather == 0.0 {
        let signed_distance_pixels = (distance - 1.0) / distance_gradient.max(f32::MIN_POSITIVE);
        1.0 - smoothstep(-0.5, 0.5, signed_distance_pixels)
    } else {
        let normalized = (distance - (1.0 - mask.feather)) / mask.feather;
        let aa = 0.5 * distance_gradient / mask.feather;
        1.0 - smoothstep(-aa, 1.0 + aa, normalized)
    };
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
    (f64::from(rgb[0]) * REC2020_LUMA[0]
        + f64::from(rgb[1]) * REC2020_LUMA[1]
        + f64::from(rgb[2]) * REC2020_LUMA[2]) as f32
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
    fn small_circle_edge_between_centers_is_exact_and_symmetric() {
        let mut circle = mask();
        circle.center_x = 0.5;
        circle.center_y = 0.5;
        circle.radius_x = 2.5 / 21.0;
        circle.radius_y = 2.5 / 21.0;
        circle.feather = 0.0;

        let center_y = 10.5;
        let edge_x = 13.0;
        let inside = radial_coverage_at(&circle, 21, 21, edge_x - 0.5, center_y);
        let edge = radial_coverage_at(&circle, 21, 21, edge_x, center_y);
        let outside = radial_coverage_at(&circle, 21, 21, edge_x + 0.5, center_y);
        assert!((inside - 1.0).abs() < 1.0e-6);
        assert!((edge - 0.5).abs() < 1.0e-6);
        assert!(outside.abs() < 1.0e-6);
        assert!((inside + outside - 1.0).abs() < 1.0e-6);

        circle.feather = 0.4;
        assert_eq!(radial_coverage_at(&circle, 21, 21, 10.5, 10.5), 1.0);
    }

    #[test]
    fn rotated_anisotropic_principal_edge_at_45_degrees_is_symmetric() {
        let mut ellipse = mask();
        ellipse.center_x = 0.5;
        ellipse.center_y = 0.5;
        ellipse.radius_x = 8.0 / 101.0;
        ellipse.radius_y = 3.0 / 101.0;
        ellipse.rotation_degrees = 45.0;
        ellipse.feather = 0.0;

        let direction = std::f32::consts::FRAC_1_SQRT_2;
        let edge_x = 50.5 + 8.0 * direction;
        let edge_y = 50.5 + 8.0 * direction;
        let inside = radial_coverage_at(
            &ellipse,
            101,
            101,
            edge_x - 0.5 * direction,
            edge_y - 0.5 * direction,
        );
        let edge = radial_coverage_at(&ellipse, 101, 101, edge_x, edge_y);
        let outside = radial_coverage_at(
            &ellipse,
            101,
            101,
            edge_x + 0.5 * direction,
            edge_y + 0.5 * direction,
        );
        assert!((inside - 1.0).abs() < 2.0e-5);
        assert!((edge - 0.5).abs() < 2.0e-5);
        assert!(outside.abs() < 2.0e-5);
        assert!((inside + outside - 1.0).abs() < 2.0e-5);
    }

    #[test]
    fn bounding_roi_matches_full_frame_reference_and_is_small() {
        let source = CpuImage::new(
            101,
            79,
            (0..101 * 79)
                .map(|index| {
                    let value = (index % 127) as f32 / 31.0;
                    RgbaPixel::new(value - 1.0, value * 0.4, 3.0 - value, 0.8).unwrap()
                })
                .collect(),
        )
        .unwrap();
        let mut value = mask();
        value.radius_x = 0.08;
        value.radius_y = 0.06;
        value.adjustments.brightness = 30.0;
        value.adjustments.sharpness = 40.0;
        let region = mask_region(&value, source.width(), source.height());
        assert!(region.pixel_count() * 10 < source.pixels().len());

        let settings = RadialMasksSettings {
            masks: vec![value.clone()],
        };
        let mut optimized = source.clone();
        apply_with_processor(&mut optimized, &settings, &BuiltinLocalProcessor).unwrap();

        let full = Region {
            x: 0,
            y: 0,
            width: source.width(),
            height: source.height(),
        };
        let adjusted = BuiltinLocalProcessor
            .process_region(&source, &value.adjustments, full)
            .unwrap();
        let mut reference = source;
        composite_region(&mut reference, &adjusted, &value, full);
        assert_eq!(optimized, reference);
    }

    #[test]
    fn callback_processor_is_used_once_per_active_mask() {
        struct White;
        impl LocalAdjustmentProcessor for White {
            fn process_region(
                &self,
                _source: &CpuImage,
                _adjustments: &LocalAdjustments,
                region: Region,
            ) -> Result<Vec<RgbaPixel>, PipelineError> {
                Ok(vec![
                    RgbaPixel::new(1.0, 1.0, 1.0, 0.7).unwrap();
                    region.pixel_count()
                ])
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
