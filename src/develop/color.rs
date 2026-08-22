//! Color-science primitives shared by the color processing stages.
//!
//! The working space is scene-linear Rec.2020 with a D65 white point. The
//! conversions deliberately use signed cube roots so finite negative scene
//! values remain finite instead of being clipped at an intermediate stage.

// `develop::color` is the single shared implementation used by both WP2
// stages. Helpers needed only by colocated and stage unit tests are compiled
// only for test builds rather than suppressing dead-code diagnostics globally.

pub type Rgb = [f32; 3];
pub type Oklab = [f32; 3];
pub type Oklch = [f32; 3];

#[cfg(test)]
pub const REC2020_LUMA: Rgb = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const REC2020_LUMA_F64: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorMathError {
    TargetLuminanceOutOfRange,
    TargetLuminanceNotReached,
}

impl ColorMathError {
    pub const fn reason(self) -> &'static str {
        match self {
            Self::TargetLuminanceOutOfRange => {
                "target luminance is not finite and representable in f32 RGB"
            }
            Self::TargetLuminanceNotReached => {
                "target luminance cannot be reached within the f64 residual tolerance"
            }
        }
    }
}

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

#[cfg(test)]
pub fn rec2020_luminance(rgb: Rgb) -> f32 {
    rgb[0] * REC2020_LUMA[0] + rgb[1] * REC2020_LUMA[1] + rgb[2] * REC2020_LUMA[2]
}

pub fn rec2020_luminance_f64(rgb: Rgb) -> f64 {
    f64::from(rgb[0]) * REC2020_LUMA_F64[0]
        + f64::from(rgb[1]) * REC2020_LUMA_F64[1]
        + f64::from(rgb[2]) * REC2020_LUMA_F64[2]
}

pub fn exposure_target_luminance(rgb: Rgb, exposure_ev: f64) -> Result<f64, ColorMathError> {
    let target = rec2020_luminance_f64(rgb) * exposure_ev.exp2();
    validate_target_luminance(target)?;
    Ok(target)
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
#[cfg(test)]
pub fn force_luminance(rgb: Rgb, target_luminance: f32) -> Rgb {
    let delta = target_luminance - rec2020_luminance(rgb);
    [rgb[0] + delta, rgb[1] + delta, rgb[2] + delta]
}

/// Changes only OKLab lightness until the corresponding Rec.2020 luminance
/// reaches `target_luminance`. This keeps the requested OKLab hue and chroma
/// exact while giving color stages deterministic luminance semantics.
pub fn oklab_with_luminance(mut lab: Oklab, target_luminance: f64) -> Oklab {
    let a = f64::from(lab[1]);
    let b = f64::from(lab[2]);
    let target = target_luminance;
    let l_offset = 0.396_337_777_4 * a + 0.215_803_757_3 * b;
    let m_offset = -0.105_561_345_8 * a - 0.063_854_172_8 * b;
    let s_offset = -0.089_484_177_5 * a - 1.291_485_548 * b;
    let polynomial = LuminancePolynomial::new(l_offset, m_offset, s_offset, target);
    lab[0] = polynomial.closest_root(f64::from(lab[0])) as f32;
    lab
}

/// Converts an adjusted OKLab color while enforcing the requested Rec.2020 Y.
///
/// Solving OKLab lightness retains the requested hue and chroma. The final
/// additive correction is only a defensive fallback for f64-to-f32 rounding or
/// an ill-conditioned cubic; because Rec.2020 luma coefficients sum to one it
/// restores Y without clipping negative or HDR channels.
pub fn oklab_to_linear_rec2020_preserving_luminance(
    lab: Oklab,
    target_luminance: f64,
) -> Result<Rgb, ColorMathError> {
    validate_target_luminance(target_luminance)?;
    let adjusted = oklab_with_luminance(lab, target_luminance);
    let mut rgb = oklab_to_linear_rec2020(adjusted);
    let tolerance = luminance_residual_tolerance(target_luminance);
    for _ in 0..3 {
        let actual_luminance = rec2020_luminance_f64(rgb);
        let residual = target_luminance - actual_luminance;
        if luminance_residual_is_acceptable(actual_luminance, target_luminance, tolerance) {
            return Ok(rgb);
        }
        rgb = rgb.map(|channel| (f64::from(channel) + residual) as f32);
        if !rgb.into_iter().all(f32::is_finite) {
            return Err(ColorMathError::TargetLuminanceNotReached);
        }
    }
    if luminance_residual_is_acceptable(rec2020_luminance_f64(rgb), target_luminance, tolerance) {
        Ok(rgb)
    } else {
        Err(ColorMathError::TargetLuminanceNotReached)
    }
}

fn validate_target_luminance(target_luminance: f64) -> Result<(), ColorMathError> {
    if !target_luminance.is_finite()
        || target_luminance.abs() > f64::from(f32::MAX)
        || (target_luminance != 0.0 && target_luminance.abs() < f64::from(f32::MIN_POSITIVE))
    {
        return Err(ColorMathError::TargetLuminanceOutOfRange);
    }
    Ok(())
}

fn luminance_residual_tolerance(target_luminance: f64) -> f64 {
    64.0 * f64::from(f32::EPSILON) * target_luminance.abs().max(f64::from(f32::MIN_POSITIVE))
}

fn luminance_residual_is_acceptable(actual: f64, target: f64, tolerance: f64) -> bool {
    actual.is_finite() && !(target != 0.0 && actual == 0.0) && (actual - target).abs() <= tolerance
}

#[derive(Clone, Copy)]
struct LuminancePolynomial {
    coefficients: [f64; 4],
    target: f64,
}

impl LuminancePolynomial {
    fn new(l_offset: f64, m_offset: f64, s_offset: f64, target: f64) -> Self {
        const WEIGHTS: [f64; 3] = [
            -0.040_575_762_624_313_7,
            1.112_286_829_397_059_4,
            -0.071_711_066_661_517,
        ];
        let offsets = [l_offset, m_offset, s_offset];
        let cubic = WEIGHTS.iter().sum();
        let quadratic = 3.0
            * WEIGHTS
                .iter()
                .zip(offsets)
                .map(|(weight, offset)| weight * offset)
                .sum::<f64>();
        let linear = 3.0
            * WEIGHTS
                .iter()
                .zip(offsets)
                .map(|(weight, offset)| weight * offset * offset)
                .sum::<f64>();
        let constant = WEIGHTS
            .iter()
            .zip(offsets)
            .map(|(weight, offset)| weight * offset * offset * offset)
            .sum::<f64>()
            - target;
        Self {
            coefficients: [cubic, quadratic, linear, constant],
            target,
        }
    }

    fn value(self, lightness: f64) -> f64 {
        let [cubic, quadratic, linear, constant] = self.coefficients;
        ((cubic * lightness + quadratic) * lightness + linear) * lightness + constant
    }

    fn derivative(self, lightness: f64) -> f64 {
        let [cubic, quadratic, linear, _] = self.coefficients;
        (3.0 * cubic * lightness + 2.0 * quadratic) * lightness + linear
    }

    fn residual_tolerance(self) -> f64 {
        2.0e-12 * (1.0 + self.target.abs())
    }

    /// Finds every real root by splitting the cubic at its stationary points,
    /// then chooses the root nearest the incoming OKLab lightness. Each
    /// monotone interval uses safeguarded Newton steps inside a bracket.
    fn closest_root(self, initial: f64) -> f64 {
        let [cubic, quadratic, linear, constant] = self.coefficients;
        let discriminant = quadratic * quadratic - 3.0 * cubic * linear;
        let mut stationary = Vec::with_capacity(2);
        if discriminant > 0.0 {
            let root = discriminant.sqrt();
            stationary.push((-quadratic - root) / (3.0 * cubic));
            stationary.push((-quadratic + root) / (3.0 * cubic));
        } else if discriminant == 0.0 {
            stationary.push(-quadratic / (3.0 * cubic));
        }

        let mut radius = (1.0 + initial.abs())
            .max(1.0 + (constant / cubic).abs().cbrt())
            .max(
                stationary
                    .iter()
                    .map(|value| 1.0 + value.abs())
                    .fold(1.0, f64::max),
            );
        for _ in 0..128 {
            if self.value(-radius) <= 0.0 && self.value(radius) >= 0.0 {
                break;
            }
            radius *= 2.0;
        }

        let mut bounds = Vec::with_capacity(4);
        bounds.push(-radius);
        bounds.extend(
            stationary
                .into_iter()
                .filter(|value| *value > -radius && *value < radius),
        );
        bounds.push(radius);

        let mut roots = Vec::with_capacity(3);
        let tolerance = self.residual_tolerance();
        for &bound in &bounds {
            if self.value(bound).abs() <= tolerance {
                push_distinct(&mut roots, bound);
            }
        }
        for interval in bounds.windows(2) {
            let lower_value = self.value(interval[0]);
            let upper_value = self.value(interval[1]);
            if lower_value.is_sign_negative() != upper_value.is_sign_negative() {
                push_distinct(
                    &mut roots,
                    self.solve_bracket(interval[0], interval[1], initial),
                );
            }
        }

        roots
            .into_iter()
            .min_by(|left, right| (left - initial).abs().total_cmp(&(right - initial).abs()))
            .unwrap_or(initial)
    }

    fn solve_bracket(self, mut lower: f64, mut upper: f64, initial: f64) -> f64 {
        let mut lower_value = self.value(lower);
        let mut candidate = initial.clamp(lower, upper);
        for _ in 0..96 {
            let value = self.value(candidate);
            if value.abs() <= self.residual_tolerance() {
                return candidate;
            }
            if value.is_sign_negative() == lower_value.is_sign_negative() {
                lower = candidate;
                lower_value = value;
            } else {
                upper = candidate;
            }
            if upper - lower <= 2.0e-13 * (1.0 + candidate.abs()) {
                break;
            }
            let derivative = self.derivative(candidate);
            let newton = candidate - value / derivative;
            candidate = if derivative != 0.0 && newton > lower && newton < upper {
                newton
            } else {
                0.5 * (lower + upper)
            };
        }
        let midpoint = 0.5 * (lower + upper);
        if self.value(candidate).abs() <= self.value(midpoint).abs() {
            candidate
        } else {
            midpoint
        }
    }
}

fn push_distinct(values: &mut Vec<f64>, candidate: f64) {
    if values
        .iter()
        .all(|value| (value - candidate).abs() > 1.0e-10 * (1.0 + candidate.abs()))
    {
        values.push(candidate);
    }
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

    #[test]
    fn bracketed_solver_handles_negative_luminance_counterexamples() {
        for (lab, target) in [
            ([-0.243_762_7, -1.936_353_8, 0.602_729_4], -0.082_114_2),
            ([-0.281_462_2, 0.223_760_8, -1.076_560_4], -0.076_522_1),
        ] {
            let rgb = oklab_to_linear_rec2020_preserving_luminance(lab, target).unwrap();
            assert!((rec2020_luminance_f64(rgb) - target).abs() <= 2.0e-6);
            assert!(rgb.into_iter().all(f32::is_finite));
        }
    }

    #[test]
    fn broad_signed_hdr_luminance_targets_are_reached_or_rejected() {
        let mut state = 0x91e1_0da5_u32;
        for _ in 0..4096 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let sample = |value: u32| value as f32 / u32::MAX as f32;
            let lab = [
                -2.0 + 4.0 * sample(state),
                -3.0 + 6.0 * sample(state.rotate_left(11)),
                -3.0 + 6.0 * sample(state.rotate_left(23)),
            ];
            let target = -16.0 + 32.0 * sample(state.rotate_left(7));
            match oklab_to_linear_rec2020_preserving_luminance(lab, f64::from(target)) {
                Ok(rgb) => {
                    assert!(
                        (rec2020_luminance_f64(rgb) - f64::from(target)).abs()
                            <= luminance_residual_tolerance(f64::from(target))
                    );
                    assert!(rgb.into_iter().all(f32::is_finite));
                }
                Err(ColorMathError::TargetLuminanceNotReached) => {}
                Err(error) => panic!("representable target failed unexpectedly: {error:?}"),
            }
        }
    }

    #[test]
    fn target_range_is_strict_at_positive_and_negative_f32_max() {
        let limit = f64::from(f32::MAX);
        let immediately_below = f64::from_bits(limit.to_bits() - 1);
        let immediately_above = f64::from_bits(limit.to_bits() + 1);
        let previous = f64::from(f32::from_bits(f32::MAX.to_bits() - 1));
        let rounding_interval_above_max = limit + (limit - previous) * 0.25;
        for target in [
            previous,
            immediately_below,
            limit,
            -previous,
            -immediately_below,
            -limit,
        ] {
            assert_eq!(validate_target_luminance(target), Ok(()));
        }
        for target in [
            immediately_above,
            rounding_interval_above_max,
            -immediately_above,
            -rounding_interval_above_max,
        ] {
            assert_eq!(target as f32, target.signum() as f32 * f32::MAX);
            assert_eq!(
                validate_target_luminance(target),
                Err(ColorMathError::TargetLuminanceOutOfRange)
            );
        }

        assert_eq!(
            exposure_target_luminance([f32::MAX, f32::MAX * 0.5, f32::MAX * 0.25], 2.0),
            Err(ColorMathError::TargetLuminanceOutOfRange)
        );
    }

    #[test]
    fn subnormal_targets_are_explicitly_rejected_at_both_signs() {
        let minimum_normal = f64::from(f32::MIN_POSITIVE);
        for target in [minimum_normal, -minimum_normal] {
            assert_eq!(validate_target_luminance(target), Ok(()));
            let rgb = oklab_to_linear_rec2020_preserving_luminance([0.0; 3], target).unwrap();
            let actual = rec2020_luminance_f64(rgb);
            assert_ne!(actual, 0.0);
            assert!(
                (actual - target).abs() <= luminance_residual_tolerance(target),
                "{actual} did not reach {target}"
            );
        }
        for target in [minimum_normal * 0.5, -minimum_normal * 0.5] {
            assert_eq!(
                validate_target_luminance(target),
                Err(ColorMathError::TargetLuminanceOutOfRange)
            );
        }

        let subnormal = f32::from_bits(1);
        assert_eq!(
            exposure_target_luminance([subnormal; 3], -2.0),
            Err(ColorMathError::TargetLuminanceOutOfRange)
        );
    }

    #[test]
    fn exponent_sweep_is_correct_or_explicitly_rejected() {
        let magnitudes = [
            f32::from_bits(1),
            1.0e-30,
            1.0e-10,
            1.0,
            1.0e10,
            1.0e30,
            f32::MAX,
        ];
        for magnitude in magnitudes {
            for sign in [-1.0, 1.0] {
                let red = sign * magnitude;
                let green = -(f64::from(red) * f64::from(REC2020_LUMA[0])
                    / f64::from(REC2020_LUMA[1])) as f32;
                let rgb = [red, green, magnitude * 0.03125];
                for exposure_ev in [-2.0, 0.0, 2.0] {
                    let Ok(target) = exposure_target_luminance(rgb, exposure_ev) else {
                        continue;
                    };
                    assert!(target.is_finite());
                    let mut lab = linear_rec2020_to_oklab(rgb);
                    lab[1] += 0.15;
                    lab[2] -= 0.10;
                    match oklab_to_linear_rec2020_preserving_luminance(lab, target) {
                        Ok(output) => assert!(
                            (rec2020_luminance_f64(output) - target).abs()
                                <= luminance_residual_tolerance(target)
                        ),
                        Err(ColorMathError::TargetLuminanceNotReached) => {}
                        Err(error) => panic!("representable target failed unexpectedly: {error:?}"),
                    }
                }
            }
        }
    }
}
