use rayon::prelude::*;

use crate::develop::{
    CpuImage, DevelopStage, PipelineError,
    color::{
        ColorMathError, Rgb, exposure_target_luminance, linear_rec2020_to_oklab,
        oklab_to_linear_rec2020_preserving_luminance,
    },
    settings::{ColorGradeRange, ColorGradingSettings},
};

#[cfg(test)]
use crate::develop::color::{oklab_to_linear_rec2020, rec2020_luminance};

const MAX_GRADE_CHROMA: f32 = 0.15;

#[derive(Clone, Copy)]
struct PreparedGradeRange {
    ab: [f32; 2],
    luminance: f32,
}

impl PreparedGradeRange {
    fn new(settings: &ColorGradeRange) -> Self {
        let strength = settings.saturation / 100.0 * MAX_GRADE_CHROMA;
        let (sin_hue, cos_hue) = settings.hue_degrees.to_radians().sin_cos();
        Self {
            ab: [strength * cos_hue, strength * sin_hue],
            luminance: settings.luminance / 100.0,
        }
    }
}

struct PreparedColorGrading {
    ranges: [PreparedGradeRange; 3],
    balance: f32,
    transition_width: f32,
    neutral: bool,
}

impl PreparedColorGrading {
    fn new(settings: &ColorGradingSettings) -> Self {
        Self {
            ranges: [
                PreparedGradeRange::new(&settings.shadows),
                PreparedGradeRange::new(&settings.midtones),
                PreparedGradeRange::new(&settings.highlights),
            ],
            balance: settings.balance / 100.0,
            transition_width: 0.05 + 0.40 * (settings.blending / 100.0),
            neutral: settings.is_neutral(),
        }
    }

    fn apply(&self, rgb: Rgb) -> Result<Rgb, ColorMathError> {
        if self.neutral {
            return Ok(rgb);
        }
        let lab = linear_rec2020_to_oklab(rgb);
        let weights = grade_weights(lab[0], self.balance, self.transition_width);
        let mut graded = lab;
        graded[1] += weighted_component(weights, self.ranges.map(|range| range.ab[0]));
        graded[2] += weighted_component(weights, self.ranges.map(|range| range.ab[1]));

        // F0 persists per-range luminance but has no preserve-luminance flag.
        // Chroma grading therefore always preserves Rec.2020 Y, while these
        // explicit luminance controls apply a weighted local exposure of ±2 EV.
        let luminance_adjustment =
            weighted_component(weights, self.ranges.map(|range| range.luminance));
        let target_luminance =
            exposure_target_luminance(rgb, 2.0 * f64::from(luminance_adjustment))?;
        oklab_to_linear_rec2020_preserving_luminance(graded, target_luminance)
    }
}

pub(super) fn supports(_settings: &ColorGradingSettings) -> bool {
    true
}

pub(super) fn apply(
    image: &mut CpuImage,
    settings: &ColorGradingSettings,
) -> Result<(), PipelineError> {
    let prepared = PreparedColorGrading::new(settings);
    if prepared.neutral {
        return Ok(());
    }
    image.pixels_mut().par_iter_mut().try_for_each(|pixel| {
        let adjusted = prepared
            .apply([pixel.red, pixel.green, pixel.blue])
            .map_err(|error| PipelineError::NumericFailure {
                stage: DevelopStage::ColorGrading,
                reason: error.reason(),
            })?;
        pixel.red = adjusted[0];
        pixel.green = adjusted[1];
        pixel.blue = adjusted[2];
        Ok(())
    })
}

fn grade_weights(lightness: f32, balance: f32, transition_width: f32) -> [f32; 3] {
    let value = (lightness + 0.25 * balance).clamp(0.0, 1.0);
    let half_width = transition_width * 0.5;
    let shadow = 1.0 - smoothstep(0.25 - half_width, 0.25 + half_width, value);
    let highlight = smoothstep(0.75 - half_width, 0.75 + half_width, value);
    let midtone = (1.0 - shadow - highlight).max(0.0);
    [shadow, midtone, highlight]
}

fn weighted_component(weights: [f32; 3], values: [f32; 3]) -> f32 {
    weights
        .into_iter()
        .zip(values)
        .map(|(weight, value)| weight * value)
        .sum()
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f32, expected: f32, tolerance: f32) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "{actual} != {expected}"
        );
    }

    fn assert_weights(actual: [f32; 3], expected: [f32; 3]) {
        for index in 0..3 {
            assert_close(actual[index], expected[index], 2.0e-6);
        }
    }

    #[test]
    fn normative_zone_weights_are_smooth_and_normalized() {
        let width = 0.25;
        assert_weights(grade_weights(0.0, 0.0, width), [1.0, 0.0, 0.0]);
        assert_weights(grade_weights(0.25, 0.0, width), [0.5, 0.5, 0.0]);
        assert_weights(grade_weights(0.5, 0.0, width), [0.0, 1.0, 0.0]);
        assert_weights(grade_weights(0.75, 0.0, width), [0.0, 0.5, 0.5]);
        assert_weights(grade_weights(1.0, 0.0, width), [0.0, 0.0, 1.0]);

        for sample in -1000..=2000 {
            let weights = grade_weights(sample as f32 / 1000.0, 0.37, 0.31);
            assert!(weights.into_iter().all(|weight| weight >= 0.0));
            assert_close(weights.into_iter().sum(), 1.0, 2.0e-6);
        }
    }

    #[test]
    fn positive_balance_favors_highlights() {
        assert_weights(
            grade_weights(0.5, 1.0, 0.25),
            grade_weights(0.75, 0.0, 0.25),
        );
        assert_weights(
            grade_weights(0.5, -1.0, 0.25),
            grade_weights(0.25, 0.0, 0.25),
        );
    }

    #[test]
    fn pure_zone_grade_adds_normative_oklab_chroma_and_preserves_y() {
        let source_lab = [0.2, 0.0, 0.0];
        let source = oklab_to_linear_rec2020(source_lab);
        let mut settings = ColorGradingSettings::default();
        settings.shadows.saturation = 100.0;
        settings.shadows.hue_degrees = 0.0;
        let output = PreparedColorGrading::new(&settings).apply(source).unwrap();
        let output_lab = linear_rec2020_to_oklab(output);
        assert_close(output_lab[1], MAX_GRADE_CHROMA, 3.0e-5);
        assert_close(output_lab[2], 0.0, 3.0e-5);
        assert_close(rec2020_luminance(output), rec2020_luminance(source), 3.0e-6);
    }

    #[test]
    fn range_luminance_is_a_weighted_two_stop_adjustment() {
        let source = [0.18, 0.18, 0.18];
        let source_l = linear_rec2020_to_oklab(source)[0];
        let mut settings = ColorGradingSettings::default();
        if source_l < 0.625 {
            settings.midtones.luminance = 100.0;
        } else {
            settings.highlights.luminance = 100.0;
        }
        let prepared = PreparedColorGrading::new(&settings);
        let weights = grade_weights(source_l, prepared.balance, prepared.transition_width);
        let weighted = weighted_component(weights, prepared.ranges.map(|range| range.luminance));
        let output = prepared.apply(source).unwrap();
        assert_close(
            rec2020_luminance(output),
            rec2020_luminance(source) * (2.0 * weighted).exp2(),
            3.0e-6,
        );
    }
}
