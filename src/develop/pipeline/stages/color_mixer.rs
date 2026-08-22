use crate::develop::{
    CpuImage, PipelineError,
    settings::{ColorBandAdjustment, ColorMixerSettings},
};

#[path = "../../color.rs"]
pub(super) mod color;

use color::{
    Rgb, linear_rec2020_to_oklab, oklab_to_linear_rec2020, oklab_to_oklch, oklab_with_luminance,
    oklch_to_oklab, rec2020_luminance, wrap_radians,
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

    fn apply(&self, rgb: Rgb) -> Rgb {
        if self.neutral {
            return rgb;
        }

        let lch = oklab_to_oklch(linear_rec2020_to_oklab(rgb));
        if lch[1] <= 1.0e-5 {
            // Hue is undefined for neutral pixels. In particular, a color-band
            // luminance adjustment must not choose an arbitrary band for gray.
            return rgb;
        }

        let gate = smoothstep(1.0e-5, 5.0e-4, lch[1]);
        let weights = band_weights(lch[2], DEFAULT_SMOOTHING);
        let hue_shift = gate * weighted_sum(weights, self.hue_shift);
        let saturation = gate * weighted_sum(weights, self.saturation);
        let luminance = gate * weighted_sum(weights, self.luminance);

        let adjusted_lch = [
            lch[0],
            lch[1] * (1.0 + saturation).max(0.0),
            wrap_radians(lch[2] + hue_shift),
        ];
        let target_luminance = rec2020_luminance(rgb) * (2.0 * luminance).exp2();
        let adjusted_lab = oklab_with_luminance(oklch_to_oklab(adjusted_lch), target_luminance);
        oklab_to_linear_rec2020(adjusted_lab)
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
        let adjusted = prepared.apply([pixel.red, pixel.green, pixel.blue]);
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
        let source = oklab_to_linear_rec2020(oklch_to_oklab([0.6, 0.1, 0.0]));
        let mut settings = ColorMixerSettings::default();
        settings.red.hue_shift_degrees = 45.0;
        settings.red.saturation = 100.0;
        settings.red.luminance = 100.0;
        let output = PreparedColorMixer::new(&settings).apply(source);
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
            assert_eq!(prepared.apply([gray, gray, gray]), [gray, gray, gray]);
        }
    }
}
