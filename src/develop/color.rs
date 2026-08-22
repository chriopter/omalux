//! Color-science primitives shared by the color processing stages.
//!
//! The working space is scene-linear Rec.2020 with a D65 white point. The
//! conversions deliberately use signed cube roots so finite negative scene
//! values remain finite instead of being clipped at an intermediate stage.

// F0 does not yet expose a shared color module, so WP2's two disjoint stages
// include this file independently and each sees a deliberately partial API.
#![allow(dead_code)]

pub type Rgb = [f32; 3];
pub type Oklab = [f32; 3];
pub type Oklch = [f32; 3];

pub const REC2020_LUMA: Rgb = [0.262_700_2, 0.677_998_1, 0.059_301_7];

const REC2020_TO_XYZ: [[f64; 3]; 3] = [
    [0.636_958_048_3, 0.144_616_903_6, 0.168_880_975_2],
    [0.262_700_212_0, 0.677_998_071_5, 0.059_301_716_5],
    [0.0, 0.028_072_693_0, 1.060_985_057_7],
];
const XYZ_TO_LMS: [[f64; 3]; 3] = [
    [
        0.819_022_437_996_703,
        0.361_906_260_052_890_4,
        -0.128_873_781_520_987_9,
    ],
    [
        0.032_983_653_932_388_5,
        0.929_286_861_586_343_4,
        0.036_144_666_350_642_4,
    ],
    [
        0.048_177_189_359_624_2,
        0.264_239_531_752_730_8,
        0.633_547_828_469_430_9,
    ],
];
const LMS_TO_XYZ: [[f64; 3]; 3] = [
    [
        1.226_879_873_374_155_7,
        -0.557_814_996_555_481_3,
        0.281_391_050_177_215_8,
    ],
    [
        -0.040_575_762_624_313_7,
        1.112_286_829_397_059_4,
        -0.071_711_066_661_517,
    ],
    [
        -0.076_372_949_746_721_4,
        -0.421_493_323_962_791_4,
        1.586_924_024_427_242_2,
    ],
];
const XYZ_TO_REC2020: [[f64; 3]; 3] = [
    [1.716_651_188, -0.355_670_784, -0.253_366_281],
    [-0.666_684_352, 1.616_481_237, 0.015_768_546],
    [0.017_639_857, -0.042_770_613, 0.942_103_121],
];

pub fn rec2020_luminance(rgb: Rgb) -> f32 {
    rgb[0] * REC2020_LUMA[0] + rgb[1] * REC2020_LUMA[1] + rgb[2] * REC2020_LUMA[2]
}

pub fn linear_rec2020_to_oklab(rgb: Rgb) -> Oklab {
    let xyz = mul_mat3_vec3(REC2020_TO_XYZ, to_f64(rgb));
    let lms = mul_mat3_vec3(XYZ_TO_LMS, xyz).map(f64::cbrt);
    [
        (0.210_454_255_3 * lms[0] + 0.793_617_785 * lms[1] - 0.004_072_046_8 * lms[2]) as f32,
        (1.977_998_495_1 * lms[0] - 2.428_592_205 * lms[1] + 0.450_593_709_9 * lms[2]) as f32,
        (0.025_904_037_1 * lms[0] + 0.782_771_766_2 * lms[1] - 0.808_675_766 * lms[2]) as f32,
    ]
}

pub fn oklab_to_linear_rec2020(lab: Oklab) -> Rgb {
    let [lightness, a, b] = lab.map(f64::from);
    let l = lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m = lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s = lightness - 0.089_484_177_5 * a - 1.291_485_548 * b;
    let xyz = mul_mat3_vec3(LMS_TO_XYZ, [l * l * l, m * m * m, s * s * s]);
    mul_mat3_vec3(XYZ_TO_REC2020, xyz).map(|value| value as f32)
}

pub fn oklab_to_oklch(lab: Oklab) -> Oklch {
    let chroma = lab[1].hypot(lab[2]);
    let hue = if chroma <= 1.0e-7 {
        0.0
    } else {
        wrap_radians(lab[2].atan2(lab[1]))
    };
    [lab[0], chroma, hue]
}

pub fn oklch_to_oklab(lch: Oklch) -> Oklab {
    let (sin_hue, cos_hue) = lch[2].sin_cos();
    [lch[0], lch[1] * cos_hue, lch[1] * sin_hue]
}

pub fn wrap_radians(hue: f32) -> f32 {
    hue.rem_euclid(std::f32::consts::TAU)
}

/// Adds a neutral component so the returned RGB has exactly the requested
/// Rec.2020 luminance without clipping negative or HDR channel values.
pub fn force_luminance(rgb: Rgb, target_luminance: f32) -> Rgb {
    let delta = target_luminance - rec2020_luminance(rgb);
    [rgb[0] + delta, rgb[1] + delta, rgb[2] + delta]
}

/// Changes only OKLab lightness until the corresponding Rec.2020 luminance
/// reaches `target_luminance`. This keeps the requested OKLab hue and chroma
/// exact while giving color stages deterministic luminance semantics.
pub fn oklab_with_luminance(mut lab: Oklab, target_luminance: f32) -> Oklab {
    let a = f64::from(lab[1]);
    let b = f64::from(lab[2]);
    let target = f64::from(target_luminance);
    let l_offset = 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_offset = -0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_offset = -0.089_484_177_5 * a - 1.291_485_548 * b;
    let mut lightness = f64::from(lab[0]);

    for _ in 0..12 {
        let l = lightness + l_offset;
        let m = lightness + m_offset;
        let s = lightness + s_offset;
        let value = -0.040_575_762_624_313_7 * l * l * l + 1.112_286_829_397_059_4 * m * m * m
            - 0.071_711_066_661_517 * s * s * s;
        let derivative = 3.0
            * (-0.040_575_762_624_313_7 * l * l + 1.112_286_829_397_059_4 * m * m
                - 0.071_711_066_661_517 * s * s);
        if derivative.abs() <= 1.0e-12 {
            break;
        }
        let correction = (value - target) / derivative;
        lightness -= correction.clamp(-4.0, 4.0);
        if correction.abs() <= 1.0e-10 {
            break;
        }
    }
    lab[0] = lightness as f32;
    lab
}

fn to_f64(value: Rgb) -> [f64; 3] {
    value.map(f64::from)
}

fn mul_mat3_vec3(matrix: [[f64; 3]; 3], vector: [f64; 3]) -> [f64; 3] {
    matrix.map(|row| row[0] * vector[0] + row[1] * vector[1] + row[2] * vector[2])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_triplet(actual: [f32; 3], expected: [f32; 3], tolerance: f32) {
        for channel in 0..3 {
            assert!(
                (actual[channel] - expected[channel]).abs() <= tolerance,
                "channel {channel}: {:?} != {:?}",
                actual,
                expected
            );
        }
    }

    #[test]
    fn rec2020_primary_goldens_match_oklab_definition() {
        for (rgb, expected) in [
            ([0.0, 0.0, 0.0], [0.0, 0.0, 0.0]),
            ([1.0, 1.0, 1.0], [0.999_999_94, 0.0, 0.000_000_04]),
            ([1.0, 0.0, 0.0], [0.687_089, 0.332_730_4, 0.149_438_26]),
            ([0.0, 1.0, 0.0], [0.829_777_24, -0.415_774_38, 0.215_562_52]),
            (
                [0.0, 0.0, 1.0],
                [0.423_448_26, -0.161_378_43, -0.347_132_44],
            ),
            ([0.18, 0.18, 0.18], [0.564_621_6, 0.0, 0.000_000_02]),
            (
                [-0.01, 0.2, 0.4],
                [0.520_523_2, -0.187_579_29, -0.112_568_18],
            ),
        ] {
            assert_triplet(linear_rec2020_to_oklab(rgb), expected, 3.0e-6);
        }
    }

    #[test]
    fn signed_scene_values_roundtrip_without_clipping() {
        for rgb in [
            [-0.25, 0.0, 16.0],
            [-0.01, 0.2, 0.4],
            [4.0, 2.0, 0.5],
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
        ] {
            assert_triplet(
                oklab_to_linear_rec2020(linear_rec2020_to_oklab(rgb)),
                rgb,
                3.0e-6,
            );
        }
    }

    #[test]
    fn forcing_luminance_is_additive_and_unbounded() {
        let input = [-0.2, 0.4, 3.0];
        let output = force_luminance(input, 2.0);
        assert!((rec2020_luminance(output) - 2.0).abs() <= 3.0e-7);
        assert!((output[1] - output[0] - (input[1] - input[0])).abs() <= 3.0e-7);
    }

    #[test]
    fn solving_oklab_lightness_preserves_hue_and_chroma() {
        let source = [0.6, 0.2, -0.1];
        let adjusted = oklab_with_luminance(source, 2.0);
        assert_eq!(adjusted[1], source[1]);
        assert_eq!(adjusted[2], source[2]);
        assert!((rec2020_luminance(oklab_to_linear_rec2020(adjusted)) - 2.0).abs() <= 3.0e-6);
    }
}
