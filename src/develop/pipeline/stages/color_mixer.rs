use crate::develop::{
    CpuImage, DevelopStage, PipelineError,
    settings::{ColorBandAdjustment, ColorMixerSettings},
};

#[path = "../../color.rs"]
pub(super) mod color;

#[cfg(test)]
use color::rec2020_luminance;
use color::{
    ColorMathError, Rgb, exposure_target_luminance, linear_rec2020_to_oklab,
    oklab_to_linear_rec2020_preserving_luminance, oklab_to_oklch, oklch_to_oklab, wrap_radians,
};

const BAND_COUNT: usize = 8;
const DEFAULT_SMOOTHING: f32 = 50.0;

struct PreparedColorMixer {
    hue_shift: [f32; BAND_COUNT],
    saturation: [f32; BAND_COUNT],
    luminance: [f32; BAND_COUNT],
    neutral: bool,
}

impl PreparedColorMixer {
    fn new(settings: &ColorMixerSettings) -> Self {
        let bands = bands(settings);
        Self {
            hue_shift: bands.map(|band| band.hue_shift_degrees.to_radians()),
            saturation: bands.map(|band| band.saturation / 100.0),
            luminance: bands.map(|band| band.luminance / 100.0),
            neutral: settings.is_neutral(),
        }
    }

    fn apply(&self, rgb: Rgb) -> Result<Rgb, ColorMathError> {
        if self.neutral {
            return Ok(rgb);
        }

        let lch = oklab_to_oklch(linear_rec2020_to_oklab(rgb));
        if lch[1] <= 1.0e-5 {
            // Hue is undefined for neutral pixels. In particular, a color-band
            // luminance adjustment must not choose an arbitrary band for gray.
            return Ok(rgb);
        }

        let gate = smoothstep(1.0e-5, 5.0e-4, lch[1]);
        let weights = band_weights(lch[2], DEFAULT_SMOOTHING);
        let hue_shift = gate * weighted_circular_mean(weights, self.hue_shift);
        let saturation = gate * weighted_sum(weights, self.saturation);
        let luminance = gate * weighted_sum(weights, self.luminance);

        let adjusted_lch = [
            lch[0],
            lch[1] * (1.0 + saturation).max(0.0),
            wrap_radians(lch[2] + hue_shift),
        ];
        let target_luminance = exposure_target_luminance(rgb, 2.0 * f64::from(luminance))?;
        oklab_to_linear_rec2020_preserving_luminance(oklch_to_oklab(adjusted_lch), target_luminance)
    }
}

pub(super) fn supports(_settings: &ColorMixerSettings) -> bool {
    true
}

pub(super) fn apply(
    image: &mut CpuImage,
    settings: &ColorMixerSettings,
) -> Result<(), PipelineError> {
    let prepared = PreparedColorMixer::new(settings);
    if prepared.neutral {
        return Ok(());
    }
    for pixel in image.pixels_mut() {
        let adjusted = prepared
            .apply([pixel.red, pixel.green, pixel.blue])
            .map_err(|error| PipelineError::NumericFailure {
                stage: DevelopStage::ColorMixer,
                reason: error.reason(),
            })?;
        pixel.red = adjusted[0];
        pixel.green = adjusted[1];
        pixel.blue = adjusted[2];
    }
    Ok(())
}

fn bands(settings: &ColorMixerSettings) -> [&ColorBandAdjustment; BAND_COUNT] {
    [
        &settings.red,
        &settings.orange,
        &settings.yellow,
        &settings.green,
        &settings.aqua,
        &settings.blue,
        &settings.purple,
        &settings.magenta,
    ]
}

/// Returns non-negative cyclic cubic weights for the eight fixed 45° bands.
fn band_weights(hue_radians: f32, smoothing: f32) -> [f32; BAND_COUNT] {
    let position = wrap_radians(hue_radians) / std::f32::consts::TAU * BAND_COUNT as f32;
    let radius = ((smoothing.clamp(0.0, 100.0) - 50.0) / 100.0).exp2();
    let mut result = [0.0; BAND_COUNT];
    for (band, weight) in result.iter_mut().enumerate() {
        let direct = (position - band as f32).abs();
        let distance = direct.min(BAND_COUNT as f32 - direct);
        let q = (1.0 - distance / radius).clamp(0.0, 1.0);
        *weight = q * q * (3.0 - 2.0 * q);
    }
    let sum: f32 = result.iter().sum();
    debug_assert!(sum > 0.0);
    for weight in &mut result {
        *weight /= sum;
    }
    result
}

fn weighted_sum(weights: [f32; BAND_COUNT], values: [f32; BAND_COUNT]) -> f32 {
    weights
        .into_iter()
        .zip(values)
        .map(|(weight, value)| weight * value)
        .sum()
}

fn weighted_circular_mean(weights: [f32; BAND_COUNT], values: [f32; BAND_COUNT]) -> f32 {
    let (sine, cosine) =
        weights
            .into_iter()
            .zip(values)
            .fold((0.0, 0.0), |(sine, cosine), (weight, value)| {
                let (value_sine, value_cosine) = value.sin_cos();
                (sine + weight * value_sine, cosine + weight * value_cosine)
            });
    if sine.abs() + cosine.abs() <= 1.0e-7 {
        0.0
    } else {
        sine.atan2(cosine)
    }
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

    #[test]
    fn weights_are_normalized_and_cyclic() {
        for smoothing in [0.0, 25.0, 50.0, 75.0, 100.0] {
            for sample in 0..4096 {
                let hue = std::f32::consts::TAU * sample as f32 / 4096.0;
                let weights = band_weights(hue, smoothing);
                assert!(weights.into_iter().all(|weight| weight >= 0.0));
                assert_close(weights.into_iter().sum(), 1.0, 2.0e-6);
            }
        }
        let below_zero = band_weights(-1.0e-5, 50.0);
        let below_tau = band_weights(std::f32::consts::TAU - 1.0e-5, 50.0);
        for band in 0..BAND_COUNT {
            assert_close(below_zero[band], below_tau[band], 2.0e-6);
        }
    }

    #[test]
    fn default_smoothing_interpolates_exact_band_centers() {
        for band in 0..BAND_COUNT {
            let center = band as f32 * std::f32::consts::TAU / BAND_COUNT as f32;
            let weights = band_weights(center, 50.0);
            assert_close(weights[band], 1.0, 1.0e-6);
            assert_close(weights.into_iter().sum(), 1.0, 1.0e-6);

            let midpoint = center + std::f32::consts::TAU / 16.0;
            let weights = band_weights(midpoint, 50.0);
            assert_close(weights[band], 0.5, 2.0e-6);
            assert_close(weights[(band + 1) % BAND_COUNT], 0.5, 2.0e-6);
        }
    }

    #[test]
    fn red_band_has_normative_hue_chroma_and_luminance_semantics() {
        let source = color::oklab_to_linear_rec2020(oklch_to_oklab([0.6, 0.1, 0.0]));
        let mut settings = ColorMixerSettings::default();
        settings.red.hue_shift_degrees = 45.0;
        settings.red.saturation = 100.0;
        settings.red.luminance = 100.0;
        let output = PreparedColorMixer::new(&settings).apply(source).unwrap();
        let output_lch = oklab_to_oklch(linear_rec2020_to_oklab(output));
        assert_close(output_lch[2], 45.0_f32.to_radians(), 2.0e-4);
        assert_close(output_lch[1], 0.2, 2.0e-4);
        assert_close(
            rec2020_luminance(output),
            rec2020_luminance(source) * 4.0,
            3.0e-6,
        );
    }

    #[test]
    fn gray_is_unchanged_even_with_non_neutral_bands() {
        let mut settings = ColorMixerSettings::default();
        settings.red.hue_shift_degrees = 180.0;
        settings.red.saturation = 100.0;
        settings.red.luminance = 100.0;
        let prepared = PreparedColorMixer::new(&settings);
        for gray in [0.0, 0.01, 0.18, 1.0, 8.0] {
            assert_eq!(
                prepared.apply([gray, gray, gray]).unwrap(),
                [gray, gray, gray]
            );
        }
    }

    #[test]
    fn hue_offsets_interpolate_across_the_signed_wrap_without_cancellation() {
        let mut weights = [0.0; BAND_COUNT];
        weights[0] = 0.5;
        weights[1] = 0.5;
        for (left, right) in [(180.0_f32, -180.0_f32), (179.0, -179.0)] {
            let mut values = [0.0; BAND_COUNT];
            values[0] = left.to_radians();
            values[1] = right.to_radians();
            let result = weighted_circular_mean(weights, values);
            assert!((result.abs() - std::f32::consts::PI).abs() <= 2.0e-4);
        }

        let midpoint = std::f32::consts::TAU / 16.0;
        let epsilon = 1.0e-5;
        let mut values = [0.0; BAND_COUNT];
        values[0] = 179.0_f32.to_radians();
        values[1] = -179.0_f32.to_radians();
        let below = weighted_circular_mean(band_weights(midpoint - epsilon, 50.0), values);
        let above = weighted_circular_mean(band_weights(midpoint + epsilon, 50.0), values);
        assert!(wrap_radians(above - below).min(wrap_radians(below - above)) <= 1.0e-4);
    }
}
