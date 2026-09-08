use rayon::prelude::*;

use crate::develop::{
    CpuImage, PipelineError, RgbaPixel,
    settings::{BasicsSettings, LocalAdjustments},
};

mod clarity;

// Omalux's independent WP1 formulas and reference material are documented
// in docs/develop/wp1-math.md.

const LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const MIDDLE_GRAY: f64 = 0.18;
const LUMA_EPSILON: f64 = 1.0e-6;

pub(super) fn supports(_settings: &BasicsSettings) -> bool {
    true
}

pub(super) fn apply(image: &mut CpuImage, settings: &BasicsSettings) -> Result<(), PipelineError> {
    if settings.is_neutral() {
        return Ok(());
    }

    let prepared = PreparedBasics::from_settings(settings);
    if settings.clarity == 0.0 {
        image
            .pixels_mut()
            .par_iter_mut()
            .for_each(|pixel| prepared.apply_pixel(pixel));
        return Ok(());
    }
    image
        .pixels_mut()
        .par_iter_mut()
        .for_each(|pixel| prepared.apply_pre_clarity(pixel));
    clarity::apply(image, settings.clarity)?;
    image
        .pixels_mut()
        .par_iter_mut()
        .for_each(|pixel| prepared.apply_post_clarity(pixel));
    Ok(())
}

/// Prepared normative WP1 point operations shared with local adjustments.
pub(super) struct PreparedBasics {
    white_balance: [[f64; 3]; 3],
    exposure_ev: f64,
    brightness_ev: f64,
    whites: f32,
    blacks: f32,
    highlights: f32,
    shadows: f32,
    contrast: f64,
    saturation: f64,
    vibrance: f64,
}

impl PreparedBasics {
    fn from_settings(settings: &BasicsSettings) -> Self {
        Self {
            white_balance: prepare_temperature_tint_matrix(settings.temperature, settings.tint),
            exposure_ev: f64::from(settings.exposure_ev),
            brightness_ev: f64::from(settings.brightness) / 100.0,
            whites: settings.whites,
            blacks: settings.blacks,
            highlights: settings.highlights,
            shadows: settings.shadows,
            contrast: f64::from(settings.contrast) / 100.0,
            saturation: f64::from(settings.saturation) / 100.0,
            vibrance: f64::from(settings.vibrance) / 100.0,
        }
    }

    pub(super) fn from_local(settings: &LocalAdjustments) -> Self {
        Self {
            white_balance: prepare_temperature_tint_matrix(settings.temperature, settings.tint),
            exposure_ev: f64::from(settings.exposure_ev),
            brightness_ev: f64::from(settings.brightness) / 100.0,
            whites: 0.0,
            blacks: 0.0,
            highlights: 0.0,
            shadows: 0.0,
            contrast: f64::from(settings.contrast) / 100.0,
            saturation: f64::from(settings.saturation) / 100.0,
            vibrance: 0.0,
        }
    }

    pub(super) fn apply_pixel(&self, pixel: &mut RgbaPixel) {
        let mut rgb = [
            f64::from(pixel.red),
            f64::from(pixel.green),
            f64::from(pixel.blue),
        ];
        rgb = multiply_matrix(self.white_balance, rgb);
        rgb = exposure(rgb, self.exposure_ev);
        rgb = exposure(rgb, self.brightness_ev);
        rgb = whites_blacks(rgb, self.whites, self.blacks);
        rgb = highlights_shadows(rgb, self.highlights, self.shadows);
        rgb = contrast_around_middle_gray(rgb, self.contrast);
        rgb = saturation_adjustment(rgb, self.saturation);
        rgb = vibrance_adjustment(rgb, self.vibrance);
        pixel.red = rgb[0] as f32;
        pixel.green = rgb[1] as f32;
        pixel.blue = rgb[2] as f32;
    }

    fn apply_pre_clarity(&self, pixel: &mut RgbaPixel) {
        let mut rgb = [
            f64::from(pixel.red),
            f64::from(pixel.green),
            f64::from(pixel.blue),
        ];
        rgb = multiply_matrix(self.white_balance, rgb);
        rgb = exposure(rgb, self.exposure_ev);
        rgb = exposure(rgb, self.brightness_ev);
        rgb = whites_blacks(rgb, self.whites, self.blacks);
        rgb = highlights_shadows(rgb, self.highlights, self.shadows);
        rgb = contrast_around_middle_gray(rgb, self.contrast);
        pixel.red = rgb[0] as f32;
        pixel.green = rgb[1] as f32;
        pixel.blue = rgb[2] as f32;
    }

    fn apply_post_clarity(&self, pixel: &mut RgbaPixel) {
        let mut rgb = [
            f64::from(pixel.red),
            f64::from(pixel.green),
            f64::from(pixel.blue),
        ];
        rgb = saturation_adjustment(rgb, self.saturation);
        rgb = vibrance_adjustment(rgb, self.vibrance);
        pixel.red = rgb[0] as f32;
        pixel.green = rgb[1] as f32;
        pixel.blue = rgb[2] as f32;
    }
}

fn luminance(rgb: [f64; 3]) -> f64 {
    rgb[0] * LUMA[0] + rgb[1] * LUMA[1] + rgb[2] * LUMA[2]
}

fn with_luminance(rgb: [f64; 3], old_luminance: f64, new_luminance: f64) -> [f64; 3] {
    if old_luminance > LUMA_EPSILON {
        let scale = new_luminance / old_luminance;
        [rgb[0] * scale, rgb[1] * scale, rgb[2] * scale]
    } else {
        rgb
    }
}

fn exposure(rgb: [f64; 3], ev: f64) -> [f64; 3] {
    if ev == 0.0 {
        return rgb;
    }
    let gain = ev.exp2();
    [rgb[0] * gain, rgb[1] * gain, rgb[2] * gain]
}

fn tonal_coordinate(luminance: f64) -> f64 {
    luminance / (luminance + MIDDLE_GRAY)
}

fn whites_blacks(rgb: [f64; 3], whites: f32, blacks: f32) -> [f64; 3] {
    if whites == 0.0 && blacks == 0.0 {
        return rgb;
    }
    let old_luminance = luminance(rgb);
    if old_luminance <= LUMA_EPSILON {
        return rgb;
    }
    let tone = tonal_coordinate(old_luminance);
    let black_mask = 1.0 - smoothstep(0.05, 0.40, tone);
    let white_mask = smoothstep(0.60, 0.95, tone);
    let ev =
        2.0 * f64::from(blacks) / 100.0 * black_mask + 2.0 * f64::from(whites) / 100.0 * white_mask;
    with_luminance(rgb, old_luminance, old_luminance * ev.exp2())
}

fn highlights_shadows(rgb: [f64; 3], highlights: f32, shadows: f32) -> [f64; 3] {
    let rgb = masked_tonal_gain(rgb, shadows, |tone| 1.0 - smoothstep(0.10, 0.65, tone));
    masked_tonal_gain(rgb, highlights, |tone| smoothstep(0.35, 0.90, tone))
}

// Shadows and highlights are applied sequentially: each masked gain is a
// monotone map of luminance on its own, so their composition cannot reorder
// tones, while summing both EV terms in one step can turn the combined slope
// negative and invert highlights against lifted shadows.
fn masked_tonal_gain(rgb: [f64; 3], amount: f32, mask: impl Fn(f64) -> f64) -> [f64; 3] {
    if amount == 0.0 {
        return rgb;
    }
    let old_luminance = luminance(rgb);
    if old_luminance <= LUMA_EPSILON {
        return rgb;
    }
    let ev = 2.0 * f64::from(amount) / 100.0 * mask(tonal_coordinate(old_luminance));
    with_luminance(rgb, old_luminance, old_luminance * ev.exp2())
}

fn contrast_around_middle_gray(rgb: [f64; 3], amount: f64) -> [f64; 3] {
    if amount == 0.0 {
        return rgb;
    }
    let old_luminance = luminance(rgb);
    if old_luminance <= LUMA_EPSILON {
        return rgb;
    }
    let slope = amount.exp2();
    let new_luminance = MIDDLE_GRAY * (old_luminance / MIDDLE_GRAY).powf(slope);
    with_luminance(rgb, old_luminance, new_luminance)
}

fn saturation_adjustment(rgb: [f64; 3], amount: f64) -> [f64; 3] {
    if amount == 0.0 {
        return rgb;
    }
    let gray = luminance(rgb);
    let factor = 1.0 + amount;
    [
        gray + (rgb[0] - gray) * factor,
        gray + (rgb[1] - gray) * factor,
        gray + (rgb[2] - gray) * factor,
    ]
}

fn vibrance_adjustment(rgb: [f64; 3], amount: f64) -> [f64; 3] {
    if amount == 0.0 {
        return rgb;
    }
    let maximum = rgb[0].max(rgb[1]).max(rgb[2]);
    let minimum = rgb[0].min(rgb[1]).min(rgb[2]);
    let denominator = maximum.abs().max(minimum.abs()).max(1.0e-4);
    let occupancy = ((maximum - minimum) / denominator).clamp(0.0, 1.0);
    let factor = if amount > 0.0 {
        1.0 + amount * (1.0 - occupancy).powi(2)
    } else {
        1.0 + amount
    };
    let gray = luminance(rgb);
    [
        gray + (rgb[0] - gray) * factor,
        gray + (rgb[1] - gray) * factor,
        gray + (rgb[2] - gray) * factor,
    ]
}

fn smoothstep(edge0: f64, edge1: f64, value: f64) -> f64 {
    let value = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    value * value * (3.0 - 2.0 * value)
}

fn prepare_temperature_tint_matrix(temperature: f32, tint: f32) -> [[f64; 3]; 3] {
    if temperature == 0.0 && tint == 0.0 {
        return identity_matrix();
    }

    let (x, y) = target_white_xy(temperature, tint);

    let source_white = xy_to_xyz(0.3127, 0.3290);
    let target_white = xy_to_xyz(x, y);
    let adaptation = bradford_adaptation(source_white, target_white);
    multiply_matrices(
        REC2020_XYZ_TO_RGB,
        multiply_matrices(adaptation, REC2020_RGB_TO_XYZ),
    )
}

const D65_KELVIN: f64 = 6504.0;
const COOL_KELVIN: f64 = 25_000.0;
const WARM_KELVIN: f64 = 4000.0;
// The extended slider region beyond +-100 continues piecewise linearly in
// mired toward these endpoints at +-150, matching the reach of established
// editors without changing any value inside the original range.
const EXTENDED_COOL_KELVIN: f64 = 50_000.0;
const EXTENDED_WARM_KELVIN: f64 = 2000.0;
const MAX_TINT_DUV: f64 = 0.05;
const MIN_TARGET_LMS: f64 = 0.01;

fn temperature_to_mired(temperature: f32) -> f64 {
    let center = 1_000_000.0 / D65_KELVIN;
    let warm = 1_000_000.0 / WARM_KELVIN;
    let cool = 1_000_000.0 / COOL_KELVIN;
    let amount = f64::from(temperature) / 100.0;
    if amount >= 1.0 {
        warm + (amount - 1.0) * 2.0 * (1_000_000.0 / EXTENDED_WARM_KELVIN - warm)
    } else if amount >= 0.0 {
        center + amount * (warm - center)
    } else if amount > -1.0 {
        center + (-amount) * (cool - center)
    } else {
        cool + (-amount - 1.0) * 2.0 * (1_000_000.0 / EXTENDED_COOL_KELVIN - cool)
    }
}

fn target_white_xy(temperature: f32, tint: f32) -> (f64, f64) {
    let mired = temperature_to_mired(temperature);
    let ((u, v), (normal_u, normal_v)) = daylight_uv_and_magenta_normal(mired);
    if tint == 0.0 {
        return uv_to_xy(u, v);
    }

    let requested_duv = f64::from(tint) / 100.0 * MAX_TINT_DUV;
    let direction = requested_duv.signum();
    let safe_limit = safe_duv_limit(u, v, normal_u * direction, normal_v * direction);
    // `min` is continuous as the requested value or safe boundary changes;
    // unlike rejecting an unsafe corner, it cannot introduce a color jump.
    let magnitude = requested_duv.abs().min(safe_limit);
    uv_to_xy(
        u + normal_u * direction * magnitude,
        v + normal_v * direction * magnitude,
    )
}

fn daylight_uv_and_magenta_normal(mired: f64) -> ((f64, f64), (f64, f64)) {
    let (base_x, base_y) = daylight_xy_anchored(1_000_000.0 / mired);
    let base_uv = xy_to_uv(base_x, base_y);
    const DERIVATIVE_STEP_MIREDS: f64 = 0.1;
    let cool_mired = 1_000_000.0 / COOL_KELVIN;
    let warm_mired = 1_000_000.0 / WARM_KELVIN;
    let before = (mired - DERIVATIVE_STEP_MIREDS).max(cool_mired);
    let after = (mired + DERIVATIVE_STEP_MIREDS).min(warm_mired);
    let (before_x, before_y) = daylight_xy_anchored(1_000_000.0 / before);
    let (after_x, after_y) = daylight_xy_anchored(1_000_000.0 / after);
    let (u0, v0) = xy_to_uv(before_x, before_y);
    let (u1, v1) = xy_to_uv(after_x, after_y);
    let tangent_u = u1 - u0;
    let tangent_v = v1 - v0;
    let tangent_length = tangent_u.hypot(tangent_v);
    let normal_u = tangent_v / tangent_length;
    let normal_v = -tangent_u / tangent_length;
    (base_uv, (normal_u, normal_v))
}

fn safe_duv_limit(u: f64, v: f64, direction_u: f64, direction_v: f64) -> f64 {
    if target_lms_is_safe(
        u + direction_u * MAX_TINT_DUV,
        v + direction_v * MAX_TINT_DUV,
    ) {
        return MAX_TINT_DUV;
    }

    let mut safe = 0.0;
    let mut unsafe_limit = MAX_TINT_DUV;
    for _ in 0..48 {
        let candidate = (safe + unsafe_limit) * 0.5;
        if target_lms_is_safe(u + direction_u * candidate, v + direction_v * candidate) {
            safe = candidate;
        } else {
            unsafe_limit = candidate;
        }
    }
    safe
}

fn target_lms_is_safe(u: f64, v: f64) -> bool {
    let (x, y) = uv_to_xy(u, v);
    let lms = multiply_matrix(BRADFORD, xy_to_xyz(x, y));
    lms.into_iter()
        .all(|component| component.is_finite() && component >= MIN_TARGET_LMS)
}

fn daylight_xy_anchored(kelvin: f64) -> (f64, f64) {
    const D65_XY: (f64, f64) = (0.3127, 0.3290);
    let (x, y) = daylight_xy(kelvin);
    let (raw_d65_x, raw_d65_y) = daylight_xy(D65_KELVIN);
    (x + D65_XY.0 - raw_d65_x, y + D65_XY.1 - raw_d65_y)
}

fn daylight_xy(kelvin: f64) -> (f64, f64) {
    let inverse = 1.0 / kelvin;
    let x = if kelvin <= 7000.0 {
        0.244_063 + 99.11 * inverse + 2_967_800.0 * inverse.powi(2)
            - 4_607_000_000.0 * inverse.powi(3)
    } else {
        0.237_040 + 247.48 * inverse + 1_901_800.0 * inverse.powi(2)
            - 2_006_400_000.0 * inverse.powi(3)
    };
    (x, -3.0 * x * x + 2.87 * x - 0.275)
}

fn xy_to_uv(x: f64, y: f64) -> (f64, f64) {
    let denominator = -2.0 * x + 12.0 * y + 3.0;
    (4.0 * x / denominator, 6.0 * y / denominator)
}

fn uv_to_xy(u: f64, v: f64) -> (f64, f64) {
    let denominator = 2.0 * u - 8.0 * v + 4.0;
    (3.0 * u / denominator, 2.0 * v / denominator)
}

fn xy_to_xyz(x: f64, y: f64) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

fn bradford_adaptation(source_white: [f64; 3], target_white: [f64; 3]) -> [[f64; 3]; 3] {
    let source_lms = multiply_matrix(BRADFORD, source_white);
    let target_lms = multiply_matrix(BRADFORD, target_white);
    let scale = [
        [target_lms[0] / source_lms[0], 0.0, 0.0],
        [0.0, target_lms[1] / source_lms[1], 0.0],
        [0.0, 0.0, target_lms[2] / source_lms[2]],
    ];
    multiply_matrices(BRADFORD_INVERSE, multiply_matrices(scale, BRADFORD))
}

fn identity_matrix() -> [[f64; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

fn multiply_matrix(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    [
        matrix[0][0] * vector[0] + matrix[0][1] * vector[1] + matrix[0][2] * vector[2],
        matrix[1][0] * vector[0] + matrix[1][1] * vector[1] + matrix[1][2] * vector[2],
        matrix[2][0] * vector[0] + matrix[2][1] * vector[1] + matrix[2][2] * vector[2],
    ]
}

fn multiply_matrices(left: [[f64; 3]; 3], right: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut result = [[0.0; 3]; 3];
    for row in 0..3 {
        for column in 0..3 {
            result[row][column] = (0..3)
                .map(|index| left[row][index] * right[index][column])
                .sum();
        }
    }
    result
}

const BRADFORD: [[f64; 3]; 3] = [
    [0.8951, 0.2664, -0.1614],
    [-0.7502, 1.7135, 0.0367],
    [0.0389, -0.0685, 1.0296],
];
const BRADFORD_INVERSE: [[f64; 3]; 3] = [
    [0.986_992_9, -0.147_054_3, 0.159_962_7],
    [0.432_305_3, 0.518_360_3, 0.049_291_2],
    [-0.008_528_7, 0.040_042_8, 0.968_486_7],
];
const REC2020_RGB_TO_XYZ: [[f64; 3]; 3] = [
    [0.636_958_048_3, 0.144_616_903_6, 0.168_880_975_2],
    [0.262_700_212_0, 0.677_998_071_5, 0.059_301_716_5],
    [0.0, 0.028_072_693_0, 1.060_985_057_7],
];
const REC2020_XYZ_TO_RGB: [[f64; 3]; 3] = [
    [1.716_651_188_0, -0.355_670_783_8, -0.253_366_281_4],
    [-0.666_684_351_8, 1.616_481_236_6, 0.015_768_545_8],
    [0.017_639_857_4, -0.042_770_613_3, 0.942_103_121_2],
];

#[cfg(test)]
mod tests {
    use super::{
        BRADFORD, D65_KELVIN, MAX_TINT_DUV, MIN_TARGET_LMS, daylight_uv_and_magenta_normal,
        multiply_matrix, prepare_temperature_tint_matrix, safe_duv_limit, target_white_xy,
        temperature_to_mired, xy_to_uv, xy_to_xyz,
    };

    #[test]
    fn temperature_endpoints_use_the_full_slider_range() {
        assert_close(1_000_000.0 / temperature_to_mired(-100.0), 25_000.0);
        assert_close(1_000_000.0 / temperature_to_mired(0.0), D65_KELVIN);
        assert_close(1_000_000.0 / temperature_to_mired(100.0), 4000.0);
        assert_ne!(
            temperature_to_mired(96.0).to_bits(),
            temperature_to_mired(100.0).to_bits()
        );
    }

    #[test]
    fn temperature_and_tint_whitepoints_match_goldens() {
        for (temperature, tint, expected) in [
            (-100.0, 0.0, (0.249_839_613_517_353, 0.254_680_365_073_757)),
            (0.0, 0.0, (0.312_700_000_000_000, 0.329_000_000_000_000)),
            (100.0, 0.0, (0.382_329_568_117_353, 0.383_647_161_878_391)),
            (0.0, -100.0, (0.297_387_552_972_594, 0.430_624_537_556_747)),
            (0.0, 100.0, (0.323_673_166_458_433, 0.256_174_084_920_873)),
        ] {
            let actual = target_white_xy(temperature, tint);
            assert_close(actual.0, expected.0);
            assert_close(actual.1, expected.1);
        }
    }

    #[test]
    fn endpoint_adaptation_matrices_match_goldens() {
        let goldens = [
            (
                -100.0,
                0.0,
                [
                    [
                        0.883_832_527_419_965,
                        -0.059_499_126_952_997,
                        0.011_094_946_450_048,
                    ],
                    [
                        -0.003_343_360_290_510,
                        1.006_737_717_470_062,
                        -0.010_248_058_267_705,
                    ],
                    [
                        0.005_702_570_873_192,
                        -0.008_374_447_795_678,
                        1.810_065_092_898_805,
                    ],
                ],
            ),
            (
                0.0,
                0.0,
                [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            ),
            (
                100.0,
                0.0,
                [
                    [
                        1.123_751_312_242_211,
                        0.083_072_563_122_434,
                        -0.006_291_596_980_879,
                    ],
                    [
                        0.004_616_572_868_311,
                        0.951_987_217_128_624,
                        0.005_101_582_271_090,
                    ],
                    [
                        -0.003_510_070_636_723,
                        0.003_869_899_707_082,
                        0.549_128_001_038_774,
                    ],
                ],
            ),
            (
                0.0,
                -100.0,
                [
                    [
                        0.835_532_007_212_340,
                        -0.160_124_006_515_978,
                        -0.005_595_861_289_803,
                    ],
                    [
                        -0.008_799_197_025_495,
                        1.166_926_261_437_867,
                        0.007_904_369_826_139,
                    ],
                    [
                        -0.001_811_712_955_965,
                        0.007_611_835_377_340,
                        0.558_655_485_186_120,
                    ],
                ],
            ),
            (
                0.0,
                100.0,
                [
                    [
                        1.198_121_824_483_883,
                        0.192_888_776_140_963,
                        0.006_741_119_733_829,
                    ],
                    [
                        0.010_599_777_832_078,
                        0.798_917_091_065_992,
                        -0.009_521_967_118_077,
                    ],
                    [
                        0.002_182_382_381_882,
                        -0.009_169_463_457_487,
                        1.531_653_518_302_281,
                    ],
                ],
            ),
        ];
        for (temperature, tint, expected) in goldens {
            let actual = prepare_temperature_tint_matrix(temperature, tint);
            for row in 0..3 {
                for column in 0..3 {
                    assert_close(actual[row][column], expected[row][column]);
                }
            }
        }
    }

    #[test]
    fn extreme_temperature_tint_corners_keep_bradford_lms_positive() {
        for temperature in [-100.0, 100.0] {
            for tint in [-100.0, 100.0] {
                let (x, y) = target_white_xy(temperature, tint);
                let lms = multiply_matrix(BRADFORD, xy_to_xyz(x, y));
                let matrix = prepare_temperature_tint_matrix(temperature, tint);
                assert!(
                    lms.into_iter()
                        .all(|component| component >= MIN_TARGET_LMS - 1.0e-12),
                    "unsafe LMS at temperature {temperature}, tint {tint}: {lms:?}"
                );
                assert!(matrix.into_iter().flatten().all(f64::is_finite));
            }
        }
    }

    #[test]
    fn tint_limit_is_continuous_and_positive_tint_stays_magenta() {
        for temperature in [-100.0, 0.0, 100.0] {
            let mired = temperature_to_mired(temperature);
            let ((base_u, base_v), (normal_u, normal_v)) = daylight_uv_and_magenta_normal(mired);
            for tint in [-100.0, 100.0] {
                let (x, y) = target_white_xy(temperature, tint);
                let (u, v) = xy_to_uv(x, y);
                let signed_duv = (u - base_u) * normal_u + (v - base_v) * normal_v;
                assert_eq!(signed_duv.is_sign_positive(), tint > 0.0);
            }
        }

        let mired = temperature_to_mired(100.0);
        let ((base_u, base_v), (normal_u, normal_v)) = daylight_uv_and_magenta_normal(mired);
        let safe_green_limit = safe_duv_limit(base_u, base_v, -normal_u, -normal_v);
        assert!(safe_green_limit < MAX_TINT_DUV);
        let boundary_tint = -100.0 * safe_green_limit / MAX_TINT_DUV;
        let before = target_white_xy(100.0, (boundary_tint - 0.01) as f32);
        let after = target_white_xy(100.0, (boundary_tint + 0.01) as f32);
        assert!(
            (before.0 - after.0).hypot(before.1 - after.1) < 1.0e-4,
            "tint safety limit introduced a visible discontinuity"
        );
    }

    fn assert_close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= 1.0e-12,
            "expected {expected}, got {actual}"
        );
    }
}
