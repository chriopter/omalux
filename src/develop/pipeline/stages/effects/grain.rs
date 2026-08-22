// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic CPU film grain in global full-image coordinates.
//!
//! The three-octave fit and photographic-paper model are adapted from
//! darktable and RawTherapee. The 2-D simplex kernel is a scalar Rust port of
//! the MIT-licensed Ashima Arts GLSL formulation. Exact pinned provenance is
//! recorded in `docs/grain-model.md`.

use crate::develop::{CpuImage, RgbaPixel, settings::GrainSettings};

const REC2020_LUMA: [f32; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const FREQUENCIES: [f32; 3] = [0.4910, 0.9441, 1.7280];
const AMPLITUDES: [f32; 3] = [0.2340, 0.7850, 1.2150];
const PAPER_DENSITY_MIN: f32 = 0.000_01;
const PAPER_DENSITY_MAX: f32 = 0.999_99;
const EXPOSURE_NOISE_SCALE: f32 = 0.15;
const SIMPLEX_PERIOD: f32 = 289.0;

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
    pub(super) full_width: usize,
    pub(super) full_height: usize,
    pub(super) origin_x: usize,
    pub(super) origin_y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

impl GrainRegion {
    pub(super) fn full(image: &CpuImage) -> Self {
        Self {
            full_width: image.width() as usize,
            full_height: image.height() as usize,
            origin_x: 0,
            origin_y: 0,
            width: image.width() as usize,
            height: image.height() as usize,
        }
    }
}

pub(super) fn apply_full_image(
    image: &mut CpuImage,
    settings: &GrainSettings,
    seed: ResolvedGrainSeed,
) {
    let region = GrainRegion::full(image);
    apply_region(image.pixels_mut(), region, settings, seed);
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
) {
    debug_assert!(region.full_width > 0 && region.full_height > 0);
    debug_assert!(region.origin_x + region.width <= region.full_width);
    debug_assert!(region.origin_y + region.height <= region.full_height);
    debug_assert_eq!(pixels.len(), region.width * region.height);
    if settings.amount == 0.0 {
        return;
    }

    let amount = settings.amount / 100.0;
    let bias = settings.midtone_response / 100.0;
    let scale = iso_scale(settings.size_iso);
    let phases = octave_phases(seed);
    let short_edge = region.full_width.min(region.full_height) as f32;

    for (index, pixel) in pixels.iter_mut().enumerate() {
        let local_x = index % region.width;
        let local_y = index / region.width;
        let global_x = region.origin_x + local_x;
        let global_y = region.origin_y + local_y;
        let position = [
            (global_x as f32 + 0.5) / short_edge,
            (global_y as f32 + 0.5) / short_edge,
        ];
        let noise = film_noise(position, scale, phases);
        let luminance = pixel.red * REC2020_LUMA[0]
            + pixel.green * REC2020_LUMA[1]
            + pixel.blue * REC2020_LUMA[2];

        // The paper response is defined on density [0,1], but only its change
        // is added to scene-linear RGB. Negative and HDR source values are not
        // replaced by the bounded paper density and are never finally clamped.
        let density = luminance.clamp(PAPER_DENSITY_MIN, PAPER_DENSITY_MAX);
        let exposure = inverse_paper_response(density, bias);
        let developed = paper_response(exposure + noise * amount * EXPOSURE_NOISE_SCALE, bias);
        let delta = developed - density;
        pixel.red += delta;
        pixel.green += delta;
        pixel.blue += delta;
    }
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

fn film_noise(position: [f32; 2], scale: f32, phases: [[f32; 2]; 3]) -> f32 {
    let mut total = 0.0;
    for octave in 0..3 {
        let point = [
            position[0] * FREQUENCIES[octave] / scale + phases[octave][0],
            position[1] * FREQUENCIES[octave] / scale + phases[octave][1],
        ];
        total += simplex_noise(point) * AMPLITUDES[octave];
    }
    total
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

// Scalar f32 port of Ashima Arts' 2-D simplex noise. Operation ordering follows
// the GLSL source to keep a straightforward future CPU/GPU parity path.
fn simplex_noise(point: [f32; 2]) -> f32 {
    const C_X: f32 = 0.211_324_87;
    const C_Y: f32 = 0.366_025_42;
    const C_Z: f32 = -0.577_350_26;
    const C_W: f32 = 0.024_390_243;

    let skew = (point[0] + point[1]) * C_Y;
    let mut cell = [(point[0] + skew).floor(), (point[1] + skew).floor()];
    let unskew = (cell[0] + cell[1]) * C_X;
    let local0 = [point[0] - cell[0] + unskew, point[1] - cell[1] + unskew];
    let corner = if local0[0] > local0[1] {
        [1.0, 0.0]
    } else {
        [0.0, 1.0]
    };
    let local1 = [local0[0] + C_X - corner[0], local0[1] + C_X - corner[1]];
    let local2 = [local0[0] + C_Z, local0[1] + C_Z];

    cell = [mod289(cell[0]), mod289(cell[1])];
    let permutation = [
        permute(permute(cell[1]) + cell[0]),
        permute(permute(cell[1] + corner[1]) + cell[0] + corner[0]),
        permute(permute(cell[1] + 1.0) + cell[0] + 1.0),
    ];
    let locals = [local0, local1, local2];
    let mut contribution = [0.0; 3];
    for index in 0..3 {
        let radius =
            (0.5 - locals[index][0] * locals[index][0] - locals[index][1] * locals[index][1])
                .max(0.0);
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
    mod289(((value * 34.0) + 1.0) * value)
}

fn mod289(value: f32) -> f32 {
    value - (value / SIMPLEX_PERIOD).floor() * SIMPLEX_PERIOD
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

    #[test]
    fn amount_zero_is_bit_exact_for_unbounded_rgb_and_alpha() {
        let mut pixels = vec![
            pixel([-3.0, 0.5, 12.0], 0.25),
            pixel([f32::MAX, -f32::MAX, 1.0], 0.75),
        ];
        let original = pixels.clone();
        apply_region(
            &mut pixels,
            GrainRegion {
                full_width: 2,
                full_height: 1,
                origin_x: 0,
                origin_y: 0,
                width: 2,
                height: 1,
            },
            &settings(0.0, 4000.0, 100.0),
            ResolvedGrainSeed::fixed(7),
        );
        assert_eq!(pixels, original);
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
    fn simplex_and_three_octave_noise_match_f32_goldens() {
        assert_eq!(simplex_noise([0.0, 0.0]).to_bits(), 0.0_f32.to_bits());
        assert_eq!(simplex_noise([0.125, -0.75]).to_bits(), 3_201_403_381);
        let phases = octave_phases(ResolvedGrainSeed::fixed(42));
        assert_eq!(
            film_noise([0.25, 0.75], iso_scale(4000.0), phases).to_bits(),
            3_214_978_724
        );
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
            GrainRegion {
                full_width: width,
                full_height: height,
                origin_x: 0,
                origin_y: 0,
                width,
                height,
            },
            &grain,
            seed,
        );

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
                GrainRegion {
                    full_width: width,
                    full_height: height,
                    origin_x,
                    origin_y,
                    width: tile_width,
                    height: tile_height,
                },
                &grain,
                seed,
            );
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
                GrainRegion {
                    full_width: 16,
                    full_height: 16,
                    origin_x: 0,
                    origin_y: 0,
                    width: 16,
                    height: 16,
                },
                &settings(50.0, 4000.0, 100.0),
                seed,
            );
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
        apply_region(
            &mut low,
            GrainRegion {
                full_width: 33,
                full_height: 33,
                origin_x: 0,
                origin_y: 0,
                width: 33,
                height: 33,
            },
            &grain,
            seed,
        );
        apply_region(
            &mut high,
            GrainRegion {
                full_width: 99,
                full_height: 99,
                origin_x: 0,
                origin_y: 0,
                width: 99,
                height: 99,
            },
            &grain,
            seed,
        );
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
            GrainRegion {
                full_width: 1,
                full_height: 1,
                origin_x: 0,
                origin_y: 0,
                width: 1,
                height: 1,
            },
            &settings(100.0, 4000.0, 100.0),
            ResolvedGrainSeed::fixed(9),
        );
        let changed = pixels[0];
        assert_eq!(changed.alpha.to_bits(), original.alpha.to_bits());
        let red_delta = changed.red - original.red;
        assert!(((changed.green - original.green) - red_delta).abs() < 1.0e-6);
        assert!(((changed.blue - original.blue) - red_delta).abs() < 1.0e-6);
        assert!(changed.red < 0.0 && changed.blue > 1.0);
    }

    #[test]
    fn synthetic_noise_passes_mean_variance_centroid_and_isotropy_gates() {
        let size = 96;
        let phases = octave_phases(ResolvedGrainSeed::fixed(0x5eed));
        let scale = iso_scale(4000.0);
        let field = (0..size * size)
            .map(|index| {
                let x = index % size;
                let y = index / size;
                film_noise([x as f32 * 0.000_75, y as f32 * 0.000_75], scale, phases)
            })
            .collect::<Vec<_>>();
        let mean = field.iter().copied().sum::<f32>() / field.len() as f32;
        let variance = field
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f32>()
            / field.len() as f32;
        assert!(mean.abs() < 0.25, "mean={mean}");
        assert!((0.05..4.0).contains(&variance), "variance={variance}");

        let mut horizontal = 0.0;
        let mut vertical = 0.0;
        for y in 0..size - 1 {
            for x in 0..size - 1 {
                let center = field[y * size + x];
                horizontal += (field[y * size + x + 1] - center).powi(2);
                vertical += (field[(y + 1) * size + x] - center).powi(2);
            }
        }
        let isotropy = horizontal / vertical;
        assert!((0.70..1.43).contains(&isotropy), "isotropy={isotropy}");

        let row = &field[(size / 2) * size..(size / 2 + 1) * size];
        let mut weighted_frequency = 0.0_f64;
        let mut total_power = 0.0_f64;
        for frequency in 1..size / 2 {
            let mut real = 0.0_f64;
            let mut imaginary = 0.0_f64;
            for (index, sample) in row.iter().enumerate() {
                let angle = std::f64::consts::TAU * frequency as f64 * index as f64 / size as f64;
                real += f64::from(*sample) * angle.cos();
                imaginary -= f64::from(*sample) * angle.sin();
            }
            let power = real * real + imaginary * imaginary;
            total_power += power;
            weighted_frequency += frequency as f64 * power;
        }
        let centroid = weighted_frequency / total_power / (size as f64 / 2.0);
        assert!((0.08..0.85).contains(&centroid), "centroid={centroid}");
    }
}
