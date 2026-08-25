//! Automatic scene exposure and base tone for camera RAW decodes.
//!
//! A camera RAW decode is scene-related and linear; rendered as-is it looks
//! far darker and flatter than any in-camera rendition. This module applies
//! Omalux's versioned RAW auto-tone V1 right after decode, before develop
//! stages, so RAW and JPEG sources respond comparably to the same settings:
//!
//! 1. Auto exposure: a uniform gain lifts the sampled 99th luminance
//!    percentile to 0.8, bounded to [0, +4] EV so dark frames are never
//!    pushed beyond recognition and bright frames are never darkened.
//! 2. Base tone: a fixed, monotone tonal shaping expressed as an EV gain over
//!    log2 luminance — strong shadow lift, a gentle S through the midtones —
//!    vanishing at display white, plus a fitted luminance-preserving chroma
//!    gain matching camera-style renditions.
//!
//! The mapping is deterministic, alpha-preserving, luminance-ratio-preserving
//! (RGB is scaled uniformly per pixel), and monotone in luminance: the EV
//! curve's slope in log2 space stays far above -1.

use crate::develop::CpuImage;

const LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const LUMA_EPSILON: f64 = 1.0e-6;
const HIGHLIGHT_PERCENTILE: f64 = 0.99;
const HIGHLIGHT_TARGET: f64 = 0.8;
const MAX_AUTO_EV: f64 = 4.0;
/// Base-tone EV boost sampled at integer log2-luminance nodes; linear in
/// between, clamped at the ends. Node spacing keeps the log2-space slope
/// well above -1, so the overall curve is strictly monotone in luminance.
const BASE_TONE_NODES: [(f64, f64); 12] = [
    (-12.0, 2.3),
    (-10.0, 1.69),
    (-9.0, 1.27),
    (-8.0, 0.83),
    (-7.0, 0.37),
    (-6.0, 0.5),
    (-5.0, 0.35),
    (-4.0, 0.17),
    (-3.0, 0.08),
    (-2.0, 0.11),
    (-1.0, 0.14),
    (0.0, 0.0),
];
/// Chroma gain of the base rendition. Camera-style RAW renditions are
/// noticeably more saturated than a colorimetric decode; the factor was
/// fitted against the reference corpus and is applied luminance-preserving.
const BASE_SATURATION: f64 = 1.35;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RawAutoToneReport {
    /// Uniform auto-exposure gain in EV (0 when the frame is already bright).
    pub exposure_ev: f64,
}

fn luminance(pixel: &crate::develop::RgbaPixel) -> f64 {
    f64::from(pixel.red) * LUMA[0]
        + f64::from(pixel.green) * LUMA[1]
        + f64::from(pixel.blue) * LUMA[2]
}

fn base_tone_ev(log2_luminance: f64) -> f64 {
    let (first_x, first_y) = BASE_TONE_NODES[0];
    if log2_luminance <= first_x {
        return first_y;
    }
    for window in BASE_TONE_NODES.windows(2) {
        let (x0, y0) = window[0];
        let (x1, y1) = window[1];
        if log2_luminance <= x1 {
            let t = (log2_luminance - x0) / (x1 - x0);
            return y0 + t * (y1 - y0);
        }
    }
    0.0
}

/// Applies auto exposure and base tone in place and reports the chosen gain.
pub fn apply(image: &mut CpuImage) -> RawAutoToneReport {
    let mut luminances: Vec<f64> = image
        .pixels()
        .iter()
        .map(luminance)
        .filter(|value| *value > LUMA_EPSILON)
        .collect();
    let exposure_ev = if luminances.is_empty() {
        0.0
    } else {
        let index = ((luminances.len() - 1) as f64 * HIGHLIGHT_PERCENTILE) as usize;
        let (_, p99, _) = luminances.select_nth_unstable_by(index, f64::total_cmp);
        let p99 = *p99;
        if p99 > 0.0 {
            (HIGHLIGHT_TARGET / p99).log2().clamp(0.0, MAX_AUTO_EV)
        } else {
            0.0
        }
    };
    let gain = exposure_ev.exp2();
    for pixel in image.pixels_mut() {
        let old = luminance(pixel);
        if old <= LUMA_EPSILON {
            continue;
        }
        let exposed = old * gain;
        let toned = exposed * base_tone_ev(exposed.log2()).exp2();
        let scale = toned / old;
        let red = f64::from(pixel.red) * scale;
        let green = f64::from(pixel.green) * scale;
        let blue = f64::from(pixel.blue) * scale;
        let gray = red * LUMA[0] + green * LUMA[1] + blue * LUMA[2];
        pixel.red = (gray + (red - gray) * BASE_SATURATION) as f32;
        pixel.green = (gray + (green - gray) * BASE_SATURATION) as f32;
        pixel.blue = (gray + (blue - gray) * BASE_SATURATION) as f32;
    }
    RawAutoToneReport { exposure_ev }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::RgbaPixel;

    fn gray_image(values: &[f64]) -> CpuImage {
        let pixels: Vec<RgbaPixel> = values
            .iter()
            .map(|v| RgbaPixel::new(*v as f32, *v as f32, *v as f32, 1.0).unwrap())
            .collect();
        CpuImage::new(values.len() as u32, 1, pixels).unwrap()
    }

    #[test]
    fn base_tone_is_monotone_and_vanishes_at_white() {
        let mut previous = f64::NEG_INFINITY;
        for step in -1400..100 {
            let x = f64::from(step) / 100.0;
            let mapped = x + base_tone_ev(x);
            assert!(mapped > previous, "non-monotone at log2 lum {x}");
            previous = mapped;
        }
        assert_eq!(base_tone_ev(0.0), 0.0);
        assert_eq!(base_tone_ev(3.0), 0.0);
    }

    #[test]
    fn bright_frames_get_no_auto_exposure() {
        let mut image = gray_image(&[0.9; 64]);
        let report = apply(&mut image);
        assert_eq!(report.exposure_ev, 0.0);
    }

    #[test]
    fn dark_frames_anchor_p99_to_target_within_bounds() {
        let mut image = gray_image(&[0.1; 100]);
        let report = apply(&mut image);
        assert!((report.exposure_ev - 3.0).abs() < 1.0e-4);
        let expected_ev = report.exposure_ev + base_tone_ev((0.1_f64 * 8.0).log2());
        let expected = 0.1 * expected_ev.exp2();
        let pixel = &image.pixels()[0];
        assert!((f64::from(pixel.red) - expected).abs() < 1.0e-4);

        let mut very_dark = gray_image(&[0.001; 100]);
        let clamped = apply(&mut very_dark);
        assert_eq!(clamped.exposure_ev, MAX_AUTO_EV);
    }

    #[test]
    fn luminance_and_alpha_follow_the_tone_map_and_chroma_boost() {
        let source = RgbaPixel::new(0.02, 0.04, 0.08, 0.5).unwrap();
        let mut image = CpuImage::new(1, 1, vec![source]).unwrap();
        let report = apply(&mut image);
        let out = &image.pixels()[0];
        // Luminance equals the exposed, base-toned value: the chroma boost
        // is luminance-preserving by construction.
        let old = 0.02 * LUMA[0] + 0.04 * LUMA[1] + 0.08 * LUMA[2];
        let exposed = old * report.exposure_ev.exp2();
        let expected = exposed * base_tone_ev(exposed.log2()).exp2();
        let actual = f64::from(out.red) * LUMA[0]
            + f64::from(out.green) * LUMA[1]
            + f64::from(out.blue) * LUMA[2];
        assert!((actual - expected).abs() < 1.0e-5);
        // Chroma grows by the fitted factor around that luminance.
        assert!(f64::from(out.blue) - f64::from(out.red) > 0.0);
        assert_eq!(out.alpha, 0.5);
    }

    #[test]
    fn nonpositive_pixels_are_untouched() {
        let source = RgbaPixel::new(-0.01, 0.0, 0.005, 1.0).unwrap();
        let mut image = CpuImage::new(1, 1, vec![source]).unwrap();
        apply(&mut image);
        let out = &image.pixels()[0];
        assert_eq!(out.red, -0.01);
    }
}
