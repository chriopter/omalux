// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic CPU film grain in global full-image coordinates.
//!
//! The three-octave fit and photographic-paper model are adapted from
//! darktable and RawTherapee. The 2-D simplex kernel is a scalar Rust port of
//! the MIT-licensed Ashima Arts GLSL formulation. Exact pinned provenance is
//! recorded in `docs/grain-model.md`; the complete MIT notice is retained in
//! `THIRD_PARTY_NOTICES.md`.

use crate::develop::{CpuImage, RgbaPixel, settings::GrainSettings};

const REC2020_LUMA: [f32; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const FREQUENCIES: [f32; 3] = [0.4910, 0.9441, 1.7280];
const AMPLITUDES: [f32; 3] = [0.2340, 0.7850, 1.2150];
const PAPER_DENSITY_MIN: f32 = 0.000_01;
const PAPER_DENSITY_MAX: f32 = 0.999_99;
const EXPOSURE_NOISE_SCALE: f32 = 0.15;
const SIMPLEX_PERIOD: f32 = 289.0;
const MAX_GRAIN_DIMENSION: usize = 1 << 20;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum GrainError {
    EmptyExtent,
    DimensionTooLarge,
    DimensionOverflow,
    RegionOutOfBounds,
    BufferLengthMismatch { expected: usize, actual: usize },
    NonFiniteOutput { pixel_index: usize },
}

/// A seed already resolved from stable image identity by the render context.
///
/// This type deliberately has no filename/path constructor. Renames must not
/// change an edit once the caller has resolved the image identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ResolvedGrainSeed(u64);

impl ResolvedGrainSeed {
    pub(super) const fn from_image_identity(value: u64) -> Self {
        Self(value)
    }

    #[cfg(test)]
    const fn fixed(value: u64) -> Self {
        Self(value)
    }
}

/// One tightly packed output region located in a larger full image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct GrainRegion {
    full_width: usize,
    full_height: usize,
    origin_x: usize,
    origin_y: usize,
    width: usize,
    height: usize,
}

impl GrainRegion {
    pub(super) fn new(
        full_width: usize,
        full_height: usize,
        origin_x: usize,
        origin_y: usize,
        width: usize,
        height: usize,
    ) -> Result<Self, GrainError> {
        if full_width == 0 || full_height == 0 || width == 0 || height == 0 {
            return Err(GrainError::EmptyExtent);
        }
        if full_width > MAX_GRAIN_DIMENSION || full_height > MAX_GRAIN_DIMENSION {
            return Err(GrainError::DimensionTooLarge);
        }
        full_width
            .checked_mul(full_height)
            .ok_or(GrainError::DimensionOverflow)?;
        width
            .checked_mul(height)
            .ok_or(GrainError::DimensionOverflow)?;
        let end_x = origin_x
            .checked_add(width)
            .ok_or(GrainError::DimensionOverflow)?;
        let end_y = origin_y
            .checked_add(height)
            .ok_or(GrainError::DimensionOverflow)?;
        if end_x > full_width || end_y > full_height {
            return Err(GrainError::RegionOutOfBounds);
        }
        Ok(Self {
            full_width,
            full_height,
            origin_x,
            origin_y,
            width,
            height,
        })
    }

    pub(super) const fn full_width(self) -> usize {
        self.full_width
    }

    pub(super) const fn full_height(self) -> usize {
        self.full_height
    }

    pub(super) const fn origin_x(self) -> usize {
        self.origin_x
    }

    pub(super) const fn origin_y(self) -> usize {
        self.origin_y
    }

    pub(super) const fn width(self) -> usize {
        self.width
    }

    pub(super) const fn height(self) -> usize {
        self.height
    }
}

pub(super) fn apply_full_image(
    image: &mut CpuImage,
    settings: &GrainSettings,
    seed: ResolvedGrainSeed,
) -> Result<(), GrainError> {
    let Some(region) =
        full_region_for_dimensions(image.width() as usize, image.height() as usize, settings)?
    else {
        return Ok(());
    };
    apply_region(image.pixels_mut(), region, settings, seed)
}

fn full_region_for_dimensions(
    width: usize,
    height: usize,
    settings: &GrainSettings,
) -> Result<Option<GrainRegion>, GrainError> {
    if settings.amount == 0.0 {
        return Ok(None);
    }
    GrainRegion::new(width, height, 0, 0, width, height).map(Some)
}

/// Applies grain to a tightly packed tile using global image coordinates.
///
/// Pixels are independent, so tiles may be evaluated in any thread/order and
/// remain bit-identical to a single full-image pass.
pub(super) fn apply_region(
    pixels: &mut [RgbaPixel],
    region: GrainRegion,
    settings: &GrainSettings,
    seed: ResolvedGrainSeed,
) -> Result<(), GrainError> {
    let expected = region
        .width
        .checked_mul(region.height)
        .ok_or(GrainError::DimensionOverflow)?;
    if pixels.len() != expected {
        return Err(GrainError::BufferLengthMismatch {
            expected,
            actual: pixels.len(),
        });
    }
    if settings.amount == 0.0 {
        return Ok(());
    }

    let amount = settings.amount / 100.0;
    let bias = settings.midtone_response / 100.0;
    let scale = iso_scale(settings.size_iso);
    let phases = octave_phases(seed);
    let short_edge = region.full_width.min(region.full_height);

    for (index, pixel) in pixels.iter_mut().enumerate() {
        let local_x = index % region.width;
        let local_y = index / region.width;
        let global_x = region.origin_x + local_x;
        let global_y = region.origin_y + local_y;
        let noise = film_noise_at_pixel(global_x, global_y, short_edge, scale, phases);
        let luminance = f64::from(pixel.red) * f64::from(REC2020_LUMA[0])
            + f64::from(pixel.green) * f64::from(REC2020_LUMA[1])
            + f64::from(pixel.blue) * f64::from(REC2020_LUMA[2]);

        // The paper response is defined on density [0,1], but only its change
        // is added to scene-linear RGB. Negative and HDR source values are not
        // replaced by the bounded paper density and are never finally clamped.
        let density =
            luminance.clamp(f64::from(PAPER_DENSITY_MIN), f64::from(PAPER_DENSITY_MAX)) as f32;
        let exposure = inverse_paper_response(density, bias);
        let developed = paper_response(exposure + noise * amount * EXPOSURE_NOISE_SCALE, bias);
        let delta = developed - density;
        let red = pixel.red + delta;
        let green = pixel.green + delta;
        let blue = pixel.blue + delta;
        if !(red.is_finite() && green.is_finite() && blue.is_finite()) {
            return Err(GrainError::NonFiniteOutput { pixel_index: index });
        }
        pixel.red = red;
        pixel.green = green;
        pixel.blue = blue;
    }
    Ok(())
}

fn iso_scale(size_iso: f32) -> f32 {
    (1.0 + size_iso.clamp(20.0, 6400.0) / 2665.0) / 800.0
}

fn paper_delta(bias: f32) -> f32 {
    2.0 * (bias.clamp(0.0, 1.0) * 0.0001_f32.ln()).exp()
}

fn paper_response(exposure: f32, bias: f32) -> f32 {
    let delta = paper_delta(bias);
    (1.0 + 2.0 * delta) / (1.0 + (4.0 * (0.5 - exposure) / (1.0 + 2.0 * delta)).exp()) - delta
}

fn inverse_paper_response(density: f32, bias: f32) -> f32 {
    let delta = paper_delta(bias);
    let density = density.clamp(PAPER_DENSITY_MIN, PAPER_DENSITY_MAX);
    -((1.0 + 2.0 * delta) / (density + delta) - 1.0).ln() * (1.0 + 2.0 * delta) / 4.0 + 0.5
}

fn film_noise_at_pixel(
    global_x: usize,
    global_y: usize,
    short_edge: usize,
    scale: f32,
    phases: [[f32; 2]; 3],
) -> f32 {
    let mut total = 0.0;
    for octave in 0..3 {
        let point = octave_point(
            global_x,
            global_y,
            short_edge,
            FREQUENCIES[octave],
            scale,
            phases[octave],
        );
        total += simplex_noise_large(point) * AMPLITUDES[octave];
    }
    total
}

fn octave_point(
    global_x: usize,
    global_y: usize,
    short_edge: usize,
    frequency: f32,
    scale: f32,
    phase: [f32; 2],
) -> [f64; 2] {
    let divisor = short_edge as f64;
    let frequency_scale = f64::from(frequency) / f64::from(scale);
    [
        (global_x as f64 + 0.5) / divisor * frequency_scale + f64::from(phase[0]),
        (global_y as f64 + 0.5) / divisor * frequency_scale + f64::from(phase[1]),
    ]
}

fn octave_phases(seed: ResolvedGrainSeed) -> [[f32; 2]; 3] {
    let mut state = seed.0;
    let mut phases = [[0.0; 2]; 3];
    for phase in &mut phases {
        phase[0] = phase_component(splitmix64(&mut state));
        phase[1] = phase_component(splitmix64(&mut state));
    }
    phases
}

fn phase_component(word: u64) -> f32 {
    let unit = ((word >> 40) as u32) as f32 * (1.0 / 16_777_216.0);
    unit * SIMPLEX_PERIOD
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

// Scalar f32 port of Ashima Arts' 2-D simplex noise at pinned commit
// 6abed1e77ed1e18b181627c35f688eb30c9fe75e. Constants, the +10 permutation,
// and operation ordering follow noise2D.glsl for a direct CPU/GPU parity path.
fn simplex_noise(point: [f32; 2]) -> f32 {
    const C_X: f32 = 0.211_324_87;
    const C_Y: f32 = 0.366_025_42;

    let skew = point[0] * C_Y + point[1] * C_Y;
    let mut cell = [(point[0] + skew).floor(), (point[1] + skew).floor()];
    let unskew = cell[0] * C_X + cell[1] * C_X;
    let local0 = [point[0] - cell[0] + unskew, point[1] - cell[1] + unskew];
    cell = [mod289(cell[0]), mod289(cell[1])];
    simplex_noise_from_lattice(local0, cell)
}

// Large-coordinate entry point. Cartesian input is never wrapped: Simplex
// noise is periodic in its skewed integer lattice, not independently in x/y.
// The global coordinate and lattice decomposition stay in f64, and only the
// integer lattice indices are reduced before entering the pinned f32 kernel.
fn simplex_noise_large(point: [f64; 2]) -> f32 {
    const C_X: f64 = 0.211_324_865_405_187;
    const C_Y: f64 = 0.366_025_403_784_439;

    let skew = point[0] * C_Y + point[1] * C_Y;
    let cell_x = (point[0] + skew).floor() as i64;
    let cell_y = (point[1] + skew).floor() as i64;
    let unskew = cell_x as f64 * C_X + cell_y as f64 * C_X;
    let local0 = [
        (point[0] - cell_x as f64 + unskew) as f32,
        (point[1] - cell_y as f64 + unskew) as f32,
    ];
    let cell = [
        cell_x.rem_euclid(SIMPLEX_PERIOD as i64) as f32,
        cell_y.rem_euclid(SIMPLEX_PERIOD as i64) as f32,
    ];
    simplex_noise_from_lattice(local0, cell)
}

fn simplex_noise_from_lattice(local0: [f32; 2], cell: [f32; 2]) -> f32 {
    const C_X: f32 = 0.211_324_87;
    const C_Z: f32 = -0.577_350_26;
    const C_W: f32 = 0.024_390_243;

    let corner = if local0[0] > local0[1] {
        [1.0, 0.0]
    } else {
        [0.0, 1.0]
    };
    let local1 = [local0[0] + C_X - corner[0], local0[1] + C_X - corner[1]];
    let local2 = [local0[0] + C_Z, local0[1] + C_Z];

    let permutation = [
        permute(permute(cell[1]) + cell[0]),
        permute(permute(cell[1] + corner[1]) + cell[0] + corner[0]),
        permute(permute(cell[1] + 1.0) + cell[0] + 1.0),
    ];
    let locals = [local0, local1, local2];
    let mut contribution = [0.0; 3];
    for index in 0..3 {
        let squared_radius =
            locals[index][0] * locals[index][0] + locals[index][1] * locals[index][1];
        let radius = (0.5 - squared_radius).max(0.0);
        let mut weight = radius * radius;
        weight *= weight;
        let gradient_x = 2.0 * fract(permutation[index] * C_W) - 1.0;
        let gradient_h = gradient_x.abs() - 0.5;
        let gradient_offset = (gradient_x + 0.5).floor();
        let gradient = gradient_x - gradient_offset;
        weight *= 1.792_842_9 - 0.853_734_73 * (gradient * gradient + gradient_h * gradient_h);
        contribution[index] =
            weight * (gradient * locals[index][0] + gradient_h * locals[index][1]);
    }
    130.0 * (contribution[0] + contribution[1] + contribution[2])
}

fn permute(value: f32) -> f32 {
    mod289(((value * 34.0) + 10.0) * value)
}

fn mod289(value: f32) -> f32 {
    value - (value * (1.0 / SIMPLEX_PERIOD)).floor() * SIMPLEX_PERIOD
}

fn fract(value: f32) -> f32 {
    value - value.floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(amount: f32, iso: f32, midtones: f32) -> GrainSettings {
        GrainSettings {
            amount,
            size_iso: iso,
            midtone_response: midtones,
        }
    }

    fn pixel(value: [f32; 3], alpha: f32) -> RgbaPixel {
        RgbaPixel::new(value[0], value[1], value[2], alpha).unwrap()
    }

    fn region(
        full_width: usize,
        full_height: usize,
        origin_x: usize,
        origin_y: usize,
        width: usize,
        height: usize,
    ) -> GrainRegion {
        GrainRegion::new(full_width, full_height, origin_x, origin_y, width, height).unwrap()
    }

    #[test]
    fn amount_zero_is_bit_exact_for_unbounded_rgb_and_alpha() {
        let mut pixels = vec![
            pixel([-3.0, 0.5, 12.0], 0.25),
            pixel([f32::MAX, -f32::MAX, 1.0], 0.75),
        ];
        let original = pixels.clone();
        apply_region(
            &mut pixels,
            region(2, 1, 0, 0, 2, 1),
            &settings(0.0, 4000.0, 100.0),
            ResolvedGrainSeed::fixed(7),
        )
        .unwrap();
        assert_eq!(pixels, original);
    }

    #[test]
    fn neutral_full_image_contract_precedes_oversized_region_validation() {
        let oversized_width = MAX_GRAIN_DIMENSION + 1;
        assert_eq!(
            full_region_for_dimensions(oversized_width, 1, &settings(0.0, 4000.0, 100.0),),
            Ok(None)
        );
        assert_eq!(
            full_region_for_dimensions(oversized_width, 1, &settings(1.0, 4000.0, 100.0),),
            Err(GrainError::DimensionTooLarge)
        );

        let mut image = CpuImage::new(
            2,
            1,
            vec![
                pixel([-3.0, 0.5, 12.0], 0.25),
                pixel([f32::MAX, -f32::MAX, 1.0], 0.75),
            ],
        )
        .unwrap();
        let original = image.clone();
        apply_full_image(
            &mut image,
            &settings(0.0, 4000.0, 100.0),
            ResolvedGrainSeed::fixed(7),
        )
        .unwrap();
        assert_eq!(image, original);
    }

    #[test]
    fn splitmix_and_seed_phases_match_goldens() {
        let mut state = 0;
        assert_eq!(splitmix64(&mut state), 0xe220_a839_7b1d_cdaf);
        assert_eq!(splitmix64(&mut state), 0x6e78_9e6a_a1b9_65f4);
        let bits = octave_phases(ResolvedGrainSeed::fixed(0x1234_5678_9abc_def0))
            .map(|phase| phase.map(f32::to_bits));
        assert_eq!(
            bits,
            [
                [1_103_598_331, 1_128_518_212],
                [1_114_594_563, 1_116_986_266],
                [1_127_055_116, 1_121_104_651],
            ]
        );
    }

    #[test]
    fn simplex_matches_independent_pinned_glsl_reference_vectors() {
        // Generated independently from the pinned GLSL source with every
        // scalar rounded to IEEE-754 binary32 after each GLSL operation.
        // The points cover cells, simplex boundaries, negative coordinates,
        // and the 289-cell permutation boundary.
        let vectors = [
            ([0.0, 0.0], 0_u32),
            ([0.125, -0.75], 3_200_011_663),
            ([0.000_000_1, -0.000_000_1], 3_041_743_548),
            ([0.211_324_87, 0.366_025_42], 1_053_690_685),
            ([12.999_99, 13.000_01], 3_192_682_416),
            ([-288.999_97, 288.999_97], 3_107_932_565),
            ([289.0, 289.0], 1_049_637_202),
            ([289.000_03, -289.000_03], 3_109_614_572),
            ([123.456, -78.9], 1_017_503_219),
            ([-1000.25, 2048.5], 1_050_861_999),
        ];
        for (point, expected) in vectors {
            assert_eq!(simplex_noise(point).to_bits(), expected, "{point:?}");
        }
    }

    #[test]
    fn f64_lattice_decomposition_preserves_normal_coordinate_kernel() {
        for point in [
            [0.0_f32, 0.0],
            [0.125, -0.75],
            [12.999_99, 13.000_01],
            [-288.999_97, 288.999_97],
            [123.456, -78.9],
        ] {
            let pinned = simplex_noise(point);
            let decomposed = simplex_noise_large([f64::from(point[0]), f64::from(point[1])]);
            assert!(
                (pinned - decomposed).abs() <= 2.0e-5,
                "{point:?}: pinned={pinned}, decomposed={decomposed}"
            );
        }
    }

    #[test]
    fn three_octave_noise_matches_golden() {
        let phases = octave_phases(ResolvedGrainSeed::fixed(42));
        assert_eq!(
            film_noise_at_pixel(0, 1, 2, iso_scale(4000.0), phases).to_bits(),
            3_211_355_459
        );
    }

    #[test]
    fn region_contract_rejects_invalid_extents_bounds_overflow_and_buffers() {
        assert_eq!(
            GrainRegion::new(1, 1, 0, 0, 0, 1),
            Err(GrainError::EmptyExtent)
        );
        assert_eq!(
            GrainRegion::new(MAX_GRAIN_DIMENSION + 1, 1, 0, 0, 1, 1),
            Err(GrainError::DimensionTooLarge)
        );
        assert_eq!(
            GrainRegion::new(10, 10, 9, 0, 2, 1),
            Err(GrainError::RegionOutOfBounds)
        );
        assert_eq!(
            GrainRegion::new(10, 10, usize::MAX, 0, 1, 1),
            Err(GrainError::DimensionOverflow)
        );

        let valid = region(10, 8, 2, 3, 4, 5);
        assert_eq!(valid.full_width(), 10);
        assert_eq!(valid.full_height(), 8);
        assert_eq!(valid.origin_x(), 2);
        assert_eq!(valid.origin_y(), 3);
        assert_eq!(valid.width(), 4);
        assert_eq!(valid.height(), 5);
        let mut wrong_length = vec![pixel([0.18; 3], 1.0); 19];
        assert_eq!(
            apply_region(
                &mut wrong_length,
                valid,
                &settings(0.0, 4000.0, 50.0),
                ResolvedGrainSeed::fixed(1),
            ),
            Err(GrainError::BufferLengthMismatch {
                expected: 20,
                actual: 19,
            })
        );
    }

    #[test]
    fn f64_coordinates_keep_adjacent_supported_pixels_distinct_without_wrapping() {
        let scale = iso_scale(6400.0);
        let phases = octave_phases(ResolvedGrainSeed::fixed(u64::MAX));
        for octave in 0..3 {
            let before = octave_point(
                MAX_GRAIN_DIMENSION - 2,
                MAX_GRAIN_DIMENSION - 2,
                MAX_GRAIN_DIMENSION,
                FREQUENCIES[octave],
                scale,
                phases[octave],
            );
            let after_x = octave_point(
                MAX_GRAIN_DIMENSION - 1,
                MAX_GRAIN_DIMENSION - 2,
                MAX_GRAIN_DIMENSION,
                FREQUENCIES[octave],
                scale,
                phases[octave],
            );
            let after_y = octave_point(
                MAX_GRAIN_DIMENSION - 2,
                MAX_GRAIN_DIMENSION - 1,
                MAX_GRAIN_DIMENSION,
                FREQUENCIES[octave],
                scale,
                phases[octave],
            );
            assert_ne!(before[0], after_x[0]);
            assert_ne!(before[1], after_y[1]);
            assert_ne!(simplex_noise_large(before), simplex_noise_large(after_x));
            assert_ne!(simplex_noise_large(before), simplex_noise_large(after_y));

            // Extreme aspect ratio: the long-axis coordinate remains f64 and
            // advances at the supported boundary without cartesian wrapping.
            let long_before = octave_point(
                MAX_GRAIN_DIMENSION - 2,
                0,
                1,
                FREQUENCIES[octave],
                scale,
                phases[octave],
            );
            let long_after = octave_point(
                MAX_GRAIN_DIMENSION - 1,
                0,
                1,
                FREQUENCIES[octave],
                scale,
                phases[octave],
            );
            assert_ne!(long_before[0], long_after[0]);
            assert!(long_before[0] > SIMPLEX_PERIOD as f64);
            assert_ne!(
                simplex_noise_large(long_before),
                simplex_noise_large(long_after)
            );
        }
    }

    #[test]
    fn cartesian_289_offsets_are_not_falsely_assumed_periodic() {
        for point in [[0.125, -0.75], [17.25, 41.5], [-1000.25, 2048.5]] {
            let original = simplex_noise_large(point);
            let shifted_x = simplex_noise_large([point[0] + 289.0, point[1]]);
            let shifted_y = simplex_noise_large([point[0], point[1] + 289.0]);
            assert_ne!(original.to_bits(), shifted_x.to_bits(), "{point:?}");
            assert_ne!(original.to_bits(), shifted_y.to_bits(), "{point:?}");
        }
    }

    #[test]
    fn noise_is_continuous_across_former_cartesian_wraps_and_lattice_boundaries() {
        const EPSILON: f64 = 0.000_01;
        for fixed in [-731.25, -0.75, 0.125, 83.5] {
            for boundary in [-578.0, -289.0, 0.0, 289.0, 578.0] {
                let left = simplex_noise_large([boundary - EPSILON, fixed]);
                let right = simplex_noise_large([boundary + EPSILON, fixed]);
                assert!((left - right).abs() < 0.002, "x={boundary}, y={fixed}");
                let below = simplex_noise_large([fixed, boundary - EPSILON]);
                let above = simplex_noise_large([fixed, boundary + EPSILON]);
                assert!((below - above).abs() < 0.002, "x={fixed}, y={boundary}");
            }
        }

        // Cross exact boundaries of each skewed lattice coordinate at both
        // ordinary and large cells. The two one-sided limits must meet.
        const C_Y: f64 = 0.366_025_403_784_439;
        for fixed in [-37.25, 0.125, 8192.75] {
            for lattice_index in [-1_000_000_i64, -17, 0, 23, 1_000_000] {
                let x_boundary = (lattice_index as f64 - fixed * C_Y) / (1.0 + C_Y);
                let left = simplex_noise_large([x_boundary - EPSILON, fixed]);
                let right = simplex_noise_large([x_boundary + EPSILON, fixed]);
                assert!((left - right).abs() < 0.002);

                let y_boundary = (lattice_index as f64 - fixed * C_Y) / (1.0 + C_Y);
                let below = simplex_noise_large([fixed, y_boundary - EPSILON]);
                let above = simplex_noise_large([fixed, y_boundary + EPSILON]);
                assert!((below - above).abs() < 0.002);
            }
        }
    }

    #[test]
    fn fitted_octaves_and_iso_scale_match_goldens() {
        assert_eq!(FREQUENCIES, [0.4910, 0.9441, 1.7280]);
        assert_eq!(AMPLITUDES, [0.2340, 0.7850, 1.2150]);
        assert_eq!(iso_scale(20.0).to_bits(), 983_896_527);
        assert_eq!(iso_scale(4000.0).to_bits(), 994_893_944);
        assert_eq!(iso_scale(6400.0).to_bits(), 998_986_579);
    }

    #[test]
    fn paper_response_roundtrips_its_supported_density_domain() {
        for bias in [0.0, 0.25, 0.5, 0.75, 1.0] {
            for density in [0.000_01, 0.01, 0.18, 0.5, 0.9, 0.999_99] {
                let roundtrip = paper_response(inverse_paper_response(density, bias), bias);
                assert!(
                    (roundtrip - density).abs() <= 2.0e-5,
                    "{bias}, {density}: {roundtrip}"
                );
            }
        }
    }

    #[test]
    fn global_coordinates_make_tiles_bit_identical_in_any_order() {
        let width = 37;
        let height = 23;
        let source = (0..width * height)
            .map(|index| {
                let base = index as f32 / (width * height) as f32;
                pixel([base * 2.0 - 0.3, base, 1.5 - base], 0.6)
            })
            .collect::<Vec<_>>();
        let grain = settings(73.0, 4000.0, 80.0);
        let seed = ResolvedGrainSeed::fixed(0xdead_beef);
        let mut full = source.clone();
        apply_region(
            &mut full,
            region(width, height, 0, 0, width, height),
            &grain,
            seed,
        )
        .unwrap();

        let mut assembled = source;
        for (origin_x, origin_y, tile_width, tile_height) in [
            (19, 11, 18, 12),
            (0, 11, 19, 12),
            (19, 0, 18, 11),
            (0, 0, 19, 11),
        ] {
            let mut tile = Vec::with_capacity(tile_width * tile_height);
            for y in origin_y..origin_y + tile_height {
                tile.extend_from_slice(
                    &assembled[y * width + origin_x..y * width + origin_x + tile_width],
                );
            }
            apply_region(
                &mut tile,
                region(width, height, origin_x, origin_y, tile_width, tile_height),
                &grain,
                seed,
            )
            .unwrap();
            for local_y in 0..tile_height {
                let destination = (origin_y + local_y) * width + origin_x;
                assembled[destination..destination + tile_width]
                    .copy_from_slice(&tile[local_y * tile_width..(local_y + 1) * tile_width]);
            }
        }
        assert_eq!(assembled, full);
    }

    #[test]
    fn resolved_seed_input_is_independent_of_a_display_name() {
        fn render_named(_display_name: &str, seed: ResolvedGrainSeed) -> Vec<RgbaPixel> {
            let mut pixels = vec![pixel([0.18; 3], 1.0); 16 * 16];
            apply_region(
                &mut pixels,
                region(16, 16, 0, 0, 16, 16),
                &settings(50.0, 4000.0, 100.0),
                seed,
            )
            .unwrap();
            pixels
        }
        let resolved = ResolvedGrainSeed::fixed(1234);
        assert_eq!(
            render_named("before.raw", resolved),
            render_named("renamed.raw", resolved)
        );
    }

    #[test]
    fn normalized_coordinates_are_export_resolution_independent() {
        let grain = settings(80.0, 4000.0, 70.0);
        let seed = ResolvedGrainSeed::fixed(2026);
        let mut low = vec![pixel([0.18; 3], 1.0); 33 * 33];
        let mut high = vec![pixel([0.18; 3], 1.0); 99 * 99];
        apply_region(&mut low, region(33, 33, 0, 0, 33, 33), &grain, seed).unwrap();
        apply_region(&mut high, region(99, 99, 0, 0, 99, 99), &grain, seed).unwrap();
        for (x, y) in [(0, 0), (3, 7), (16, 16), (31, 20), (32, 32)] {
            let low_pixel = low[y * 33 + x];
            let high_pixel = high[(3 * y + 1) * 99 + 3 * x + 1];
            assert_eq!(high_pixel, low_pixel, "coordinate ({x}, {y})");
        }
    }

    #[test]
    fn grain_preserves_alpha_and_scene_rgb_is_changed_only_by_equal_delta() {
        let original = pixel([-2.0, 0.5, 8.0], 0.375);
        let mut pixels = vec![original];
        apply_region(
            &mut pixels,
            region(1, 1, 0, 0, 1, 1),
            &settings(100.0, 4000.0, 100.0),
            ResolvedGrainSeed::fixed(9),
        )
        .unwrap();
        let changed = pixels[0];
        assert_eq!(changed.alpha.to_bits(), original.alpha.to_bits());
        let red_delta = changed.red - original.red;
        assert!(((changed.green - original.green) - red_delta).abs() < 1.0e-6);
        assert!(((changed.blue - original.blue) - red_delta).abs() < 1.0e-6);
        assert!(changed.red < 0.0 && changed.blue > 1.0);
    }

    #[test]
    fn active_grain_handles_f32_extremes_cancellation_negative_and_hdr() {
        let source = [
            [f32::MAX, f32::MAX, f32::MAX],
            [-f32::MAX, -f32::MAX, -f32::MAX],
            [f32::MAX, -f32::MAX, f32::MAX],
            [-f32::MAX, f32::MAX, -f32::MAX],
            [-1000.0, 0.000_01, 100_000.0],
        ];
        let mut pixels = source.map(|rgb| pixel(rgb, 0.42)).to_vec();
        apply_region(
            &mut pixels,
            region(source.len(), 1, 0, 0, source.len(), 1),
            &settings(100.0, 6400.0, 100.0),
            ResolvedGrainSeed::fixed(0xfeed_face),
        )
        .unwrap();
        for output in pixels {
            assert!(output.red.is_finite());
            assert!(output.green.is_finite());
            assert!(output.blue.is_finite());
            assert_eq!(output.alpha.to_bits(), 0.42_f32.to_bits());
        }
    }

    #[test]
    fn two_dimensional_radial_psd_matches_multi_seed_references() {
        let references = [
            (
                0x5eed_u64,
                [
                    0.004_294, 0.497_019, 0.056_178, 0.317_087, 0.626_735, 1.054_418,
                ],
            ),
            (
                0xdead_beef_u64,
                [
                    0.003_392, 0.468_533, 0.076_231, 0.320_804, 0.602_965, 1.295_413,
                ],
            ),
            (
                0x1234_5678_9abc_def0_u64,
                [
                    -0.015_825, 0.487_551, 0.063_819, 0.338_568, 0.597_613, 1.189_199,
                ],
            ),
        ];
        let actuals = references.map(|(seed, _)| (seed, psd_metrics(seed)));
        let tolerances = [0.005, 0.01, 0.01, 0.015, 0.015, 0.04];
        for ((_, expected), (_, actual)) in references.into_iter().zip(actuals) {
            for (index, ((actual, expected), tolerance)) in
                actual.into_iter().zip(expected).zip(tolerances).enumerate()
            {
                assert!(
                    (actual - expected).abs() < tolerance,
                    "metric {index}: actual={actual}, expected={expected}"
                );
            }
        }
    }

    // Returns mean, variance, low/mid/high radial energy fractions, and the
    // horizontal/vertical angular-sector energy ratio from a true 2-D DFT.
    fn psd_metrics(seed: u64) -> [f64; 6] {
        const SIZE: usize = 32;
        let phases = octave_phases(ResolvedGrainSeed::fixed(seed));
        let scale = iso_scale(4000.0);
        let field = (0..SIZE * SIZE)
            .map(|index| {
                f64::from(film_noise_at_pixel(
                    index % SIZE,
                    index / SIZE,
                    SIZE,
                    scale,
                    phases,
                ))
            })
            .collect::<Vec<_>>();
        let mean = field.iter().sum::<f64>() / field.len() as f64;
        let variance = field
            .iter()
            .map(|sample| (sample - mean).powi(2))
            .sum::<f64>()
            / field.len() as f64;
        let mut bands = [0.0_f64; 3];
        let mut horizontal = 0.0;
        let mut vertical = 0.0;
        for signed_y in -(SIZE as isize / 2)..SIZE as isize / 2 {
            for signed_x in -(SIZE as isize / 2)..SIZE as isize / 2 {
                if signed_x == 0 && signed_y == 0 {
                    continue;
                }
                let radius = ((signed_x * signed_x + signed_y * signed_y) as f64).sqrt()
                    / (SIZE as f64 / 2.0);
                if radius > 1.0 {
                    continue;
                }
                let mut real = 0.0;
                let mut imaginary = 0.0;
                for y in 0..SIZE {
                    for x in 0..SIZE {
                        let angle = std::f64::consts::TAU
                            * (signed_x as f64 * x as f64 + signed_y as f64 * y as f64)
                            / SIZE as f64;
                        let centered = field[y * SIZE + x] - mean;
                        real += centered * angle.cos();
                        imaginary -= centered * angle.sin();
                    }
                }
                let power = real * real + imaginary * imaginary;
                let band = if radius <= 0.25 {
                    0
                } else if radius <= 0.60 {
                    1
                } else {
                    2
                };
                bands[band] += power;
                if signed_x.abs() >= signed_y.abs() {
                    horizontal += power;
                } else {
                    vertical += power;
                }
            }
        }
        let total = bands.iter().sum::<f64>();
        [
            mean,
            variance,
            bands[0] / total,
            bands[1] / total,
            bands[2] / total,
            horizontal / vertical,
        ]
    }
}
