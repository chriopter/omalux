//! Edge-aware local contrast on signed scene-linear Rec.2020 luminance.
//!
//! A self-guided filter decomposes signed `asinh` luminance into base and
//! detail. Its two box-filter passes use global Reflect101 coordinates and a
//! fixed accumulation order, so arbitrary output tiles are bit-identical to a
//! full-frame pass. The logical source halo is twice [`RADIUS`].

use super::super::spatial::{Rect, finite_f32, reflect101};
use crate::develop::CpuImage;

const REC2020_LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const MIDDLE_GRAY: f64 = 0.18;
const RADIUS: usize = 8;
const WINDOW: f64 = (2 * RADIUS + 1) as f64;
const GUIDED_EPSILON: f64 = 0.04;
const DETAIL_LIMIT: f64 = 0.5;
const TILE_WIDTH: usize = 128;
const TILE_HEIGHT: usize = 64;

pub(super) fn apply(image: &mut CpuImage, amount: f32) {
    if amount == 0.0 {
        return;
    }
    apply_tiled(image, amount, TILE_WIDTH, TILE_HEIGHT);
}

fn apply_tiled(image: &mut CpuImage, amount: f32, tile_width: usize, tile_height: usize) {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let guide = image
        .pixels()
        .iter()
        .map(|pixel| {
            let luminance = REC2020_LUMA[0] * f64::from(pixel.red)
                + REC2020_LUMA[1] * f64::from(pixel.green)
                + REC2020_LUMA[2] * f64::from(pixel.blue);
            finite_f32((luminance / MIDDLE_GRAY).asinh())
        })
        .collect::<Vec<_>>();

    let mut coefficient_a = box_filter_tiled(&guide, width, height, false, tile_width, tile_height);
    let mut coefficient_b = box_filter_tiled(&guide, width, height, true, tile_width, tile_height);
    for (mean, correlation) in coefficient_a.iter_mut().zip(&mut coefficient_b) {
        let mean_value = f64::from(*mean);
        let correlation_value = f64::from(*correlation);
        let variance = (correlation_value - mean_value * mean_value).max(0.0);
        let a = variance / (variance + GUIDED_EPSILON);
        *mean = a as f32;
        *correlation = finite_f32(mean_value * (1.0 - a));
    }

    apply_coefficients_tiled(
        image,
        &guide,
        &coefficient_a,
        &coefficient_b,
        amount,
        tile_width,
        tile_height,
    );
}

fn box_filter_tiled(
    source: &[f32],
    width: usize,
    height: usize,
    square: bool,
    tile_width: usize,
    tile_height: usize,
) -> Vec<f32> {
    let mut output = vec![0.0; width * height];
    let mut scratch = Vec::new();
    for_each_tile(width, height, tile_width, tile_height, |region| {
        box_filter_roi(
            source,
            &mut output,
            width,
            height,
            region,
            square,
            &mut scratch,
        );
    });
    output
}

fn box_filter_roi(
    source: &[f32],
    output: &mut [f32],
    width: usize,
    height: usize,
    region: Rect,
    square: bool,
    scratch: &mut Vec<f64>,
) {
    let scratch_height = region.height + 2 * RADIUS;
    scratch.resize(region.width * scratch_height, 0.0);
    for scratch_y in 0..scratch_height {
        let global_y = region.y as isize + scratch_y as isize - RADIUS as isize;
        for local_x in 0..region.width {
            let global_x = region.x + local_x;
            let mut sum = 0.0;
            for offset in -(RADIUS as isize)..=RADIUS as isize {
                let value = f64::from(sample(
                    source,
                    width,
                    height,
                    global_x as isize + offset,
                    global_y,
                ));
                sum += if square { value * value } else { value };
            }
            scratch[scratch_y * region.width + local_x] = sum / WINDOW;
        }
    }
    for local_y in 0..region.height {
        for local_x in 0..region.width {
            let mut sum = 0.0;
            for offset in 0..2 * RADIUS + 1 {
                sum += scratch[(local_y + offset) * region.width + local_x];
            }
            output[(region.y + local_y) * width + region.x + local_x] = finite_f32(sum / WINDOW);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_coefficients_tiled(
    image: &mut CpuImage,
    guide: &[f32],
    coefficient_a: &[f32],
    coefficient_b: &[f32],
    amount: f32,
    tile_width: usize,
    tile_height: usize,
) {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let gain = f64::from(amount) / 100.0;
    let mut scratch_a = Vec::new();
    let mut scratch_b = Vec::new();
    for_each_tile(width, height, tile_width, tile_height, |region| {
        let scratch_height = region.height + 2 * RADIUS;
        scratch_a.resize(region.width * scratch_height, 0.0);
        scratch_b.resize(region.width * scratch_height, 0.0);
        for scratch_y in 0..scratch_height {
            let global_y = region.y as isize + scratch_y as isize - RADIUS as isize;
            for local_x in 0..region.width {
                let global_x = region.x + local_x;
                let mut sum_a = 0.0;
                let mut sum_b = 0.0;
                for offset in -(RADIUS as isize)..=RADIUS as isize {
                    let x = global_x as isize + offset;
                    sum_a += f64::from(sample(coefficient_a, width, height, x, global_y));
                    sum_b += f64::from(sample(coefficient_b, width, height, x, global_y));
                }
                let position = scratch_y * region.width + local_x;
                scratch_a[position] = sum_a / WINDOW;
                scratch_b[position] = sum_b / WINDOW;
            }
        }

        for local_y in 0..region.height {
            for local_x in 0..region.width {
                let mut mean_a = 0.0;
                let mut mean_b = 0.0;
                for offset in 0..2 * RADIUS + 1 {
                    let position = (local_y + offset) * region.width + local_x;
                    mean_a += scratch_a[position];
                    mean_b += scratch_b[position];
                }
                mean_a /= WINDOW;
                mean_b /= WINDOW;

                let x = region.x + local_x;
                let y = region.y + local_y;
                let index = y * width + x;
                let transformed = f64::from(guide[index]);
                let base = mean_a * transformed + mean_b;
                let detail = transformed - base;
                let bounded_detail = bound_detail(detail);
                let adjusted = transformed + gain * bounded_detail;
                let luminance_delta = MIDDLE_GRAY * (adjusted.sinh() - transformed.sinh());
                let pixel = &mut image.pixels_mut()[index];
                pixel.red = finite_f32(f64::from(pixel.red) + luminance_delta);
                pixel.green = finite_f32(f64::from(pixel.green) + luminance_delta);
                pixel.blue = finite_f32(f64::from(pixel.blue) + luminance_delta);
            }
        }
    });
}

fn bound_detail(detail: f64) -> f64 {
    detail / (1.0 + (detail / DETAIL_LIMIT).powi(2)).sqrt()
}

fn sample(source: &[f32], width: usize, height: usize, x: isize, y: isize) -> f32 {
    source[reflect101(y, height) * width + reflect101(x, width)]
}

fn for_each_tile(
    width: usize,
    height: usize,
    tile_width: usize,
    tile_height: usize,
    mut apply: impl FnMut(Rect),
) {
    let tile_width = tile_width.max(1);
    let tile_height = tile_height.max(1);
    for y in (0..height).step_by(tile_height) {
        for x in (0..width).step_by(tile_width) {
            apply(Rect {
                x,
                y,
                width: tile_width.min(width - x),
                height: tile_height.min(height - y),
            });
        }
    }
}

#[cfg(test)]
fn peak_working_bytes(width: usize, height: usize, tile_width: usize, tile_height: usize) -> usize {
    let planes = width * height * 3 * size_of::<f32>();
    let scratch =
        2 * tile_width.min(width) * (tile_height.min(height) + 2 * RADIUS) * size_of::<f64>();
    planes + scratch
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::RgbaPixel;

    fn patterned(width: usize, height: usize) -> CpuImage {
        CpuImage::new(
            width as u32,
            height as u32,
            (0..width * height)
                .map(|index| {
                    let value = ((index * 37 % 113) as f32 - 56.0) / 19.0;
                    RgbaPixel::new(value, 0.3 * value - 0.2, 2.0 - value, 0.7).unwrap()
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn arbitrary_tiles_are_bit_identical_to_full_frame() {
        let source = patterned(37, 23);
        let mut full = source.clone();
        apply_tiled(&mut full, 73.0, 37, 23);
        for tile in [(1, 1), (7, 5), (16, 8), (36, 22)] {
            let mut tiled = source.clone();
            apply_tiled(&mut tiled, 73.0, tile.0, tile.1);
            assert_eq!(tiled, full, "tile={}x{}", tile.0, tile.1);
        }
    }

    #[test]
    fn degenerate_extents_and_reflected_halos_are_stable() {
        for (width, height) in [(1, 1), (1, 19), (21, 1), (19, 17)] {
            let mut first = patterned(width, height);
            let mut second = first.clone();
            apply_tiled(&mut first, -100.0, 1, 1);
            apply_tiled(&mut second, -100.0, width, height);
            assert_eq!(first, second);
            assert!(first.pixels().iter().all(|pixel| {
                pixel.red().is_finite() && pixel.green().is_finite() && pixel.blue().is_finite()
            }));
        }
    }

    #[test]
    fn working_memory_is_three_planes_plus_bounded_tile_scratch() {
        let width = 6000;
        let height = 4000;
        let expected_planes = width * height * 12;
        let expected_scratch = 2 * 128 * (64 + 2 * RADIUS) * 8;
        assert_eq!(
            peak_working_bytes(width, height, 128, 64),
            expected_planes + expected_scratch
        );
        assert!(expected_scratch < 200_000);
    }

    #[test]
    fn transformed_detail_is_odd_monotonic_and_strictly_bounded() {
        let samples = [0.0, 0.1, 0.5, 2.0, 100.0];
        let bounded = samples.map(bound_detail);
        assert!(bounded.windows(2).all(|pair| pair[0] < pair[1]));
        for (input, output) in samples.into_iter().zip(bounded) {
            assert_eq!(bound_detail(-input), -output);
            assert!(output <= DETAIL_LIMIT);
        }
    }
}
