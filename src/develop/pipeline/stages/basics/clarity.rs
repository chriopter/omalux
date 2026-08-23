//! Edge-aware local contrast on signed scene-linear Rec.2020 luminance.
//!
//! A self-guided filter decomposes signed `asinh` luminance into base and
//! detail. Its two box-filter passes use global Reflect101 coordinates and a
//! fixed accumulation order, so arbitrary output tiles are bit-identical to a
//! full-frame pass. The logical source halo is twice [`RADIUS`].

use super::super::spatial::{Rect, finite_f32, reflect101};
use crate::{
    develop::{CpuImage, PipelineError},
    io::LimitError,
};

const REC2020_LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const MIDDLE_GRAY: f64 = 0.18;
const RADIUS: usize = 8;
const WINDOW: f64 = (2 * RADIUS + 1) as f64;
const GUIDED_EPSILON: f64 = 0.04;
const DETAIL_LIMIT: f64 = 0.5;
const TILE_WIDTH: usize = 128;
const TILE_HEIGHT: usize = 64;
const RESTART_STRIDE: usize = 16;

pub(super) fn apply(image: &mut CpuImage, amount: f32) -> Result<(), PipelineError> {
    if amount == 0.0 {
        return Ok(());
    }
    apply_tiled(image, amount, TILE_WIDTH, TILE_HEIGHT)
}

fn apply_tiled(
    image: &mut CpuImage,
    amount: f32,
    tile_width: usize,
    tile_height: usize,
) -> Result<(), PipelineError> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let mut guide = Vec::new();
    guide
        .try_reserve_exact(image.pixels().len())
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    for pixel in image.pixels() {
        let luminance = REC2020_LUMA[0] * f64::from(pixel.red)
            + REC2020_LUMA[1] * f64::from(pixel.green)
            + REC2020_LUMA[2] * f64::from(pixel.blue);
        guide.push(finite_f32((luminance / MIDDLE_GRAY).asinh()));
    }

    let mut coefficient_a =
        box_filter_tiled(&guide, width, height, false, tile_width, tile_height)?;
    let mut coefficient_b = box_filter_tiled(&guide, width, height, true, tile_width, tile_height)?;
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
    )
}

fn box_filter_tiled(
    source: &[f32],
    width: usize,
    height: usize,
    square: bool,
    tile_width: usize,
    tile_height: usize,
) -> Result<Vec<f32>, PipelineError> {
    let length = width
        .checked_mul(height)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    output.resize(length, 0.0);
    let mut scratch = Vec::new();
    for y in (0..height).step_by(tile_height.max(1)) {
        for x in (0..width).step_by(tile_width.max(1)) {
            let region = Rect {
                x,
                y,
                width: tile_width.max(1).min(width - x),
                height: tile_height.max(1).min(height - y),
            };
            box_filter_roi(
                source,
                &mut output,
                width,
                height,
                region,
                square,
                &mut scratch,
            )?;
        }
    }
    Ok(output)
}

fn box_filter_roi(
    source: &[f32],
    output: &mut [f32],
    width: usize,
    height: usize,
    region: Rect,
    square: bool,
    scratch: &mut Vec<f64>,
) -> Result<(), PipelineError> {
    let first_anchor_y = restart_anchor(region.y);
    let scratch_height = region.y - first_anchor_y + region.height + 2 * RADIUS;
    try_resize_scratch(scratch, region.width, scratch_height)?;
    for scratch_y in 0..scratch_height {
        let global_y = first_anchor_y as isize + scratch_y as isize - RADIUS as isize;
        for_each_restart_chunk(region.x, region.width, |anchor_x, end_x| {
            let mut sum = 0.0;
            for offset in -(RADIUS as isize)..=RADIUS as isize {
                let value = f64::from(sample(
                    source,
                    width,
                    height,
                    anchor_x as isize + offset,
                    global_y,
                ));
                sum += if square { value * value } else { value };
            }
            for global_x in anchor_x..end_x {
                if global_x > anchor_x {
                    let outgoing = f64::from(sample(
                        source,
                        width,
                        height,
                        global_x as isize - RADIUS as isize - 1,
                        global_y,
                    ));
                    let incoming = f64::from(sample(
                        source,
                        width,
                        height,
                        global_x as isize + RADIUS as isize,
                        global_y,
                    ));
                    sum -= if square {
                        outgoing * outgoing
                    } else {
                        outgoing
                    };
                    sum += if square {
                        incoming * incoming
                    } else {
                        incoming
                    };
                }
                if global_x >= region.x {
                    scratch[scratch_y * region.width + global_x - region.x] = sum / WINDOW;
                }
            }
        });
    }
    for local_x in 0..region.width {
        for_each_restart_chunk(region.y, region.height, |anchor_y, end_y| {
            let anchor_in_scratch = anchor_y - first_anchor_y;
            let mut sum = 0.0;
            for offset in 0..2 * RADIUS + 1 {
                sum += scratch[(anchor_in_scratch + offset) * region.width + local_x];
            }
            for global_y in anchor_y..end_y {
                if global_y > anchor_y {
                    let local_y = global_y - first_anchor_y;
                    sum -= scratch[(local_y - 1) * region.width + local_x];
                    sum += scratch[(local_y + 2 * RADIUS) * region.width + local_x];
                }
                if global_y >= region.y {
                    output[global_y * width + region.x + local_x] = finite_f32(sum / WINDOW);
                }
            }
        });
    }
    Ok(())
}

fn restart_anchor(coordinate: usize) -> usize {
    coordinate / RESTART_STRIDE * RESTART_STRIDE
}

fn for_each_restart_chunk(start: usize, length: usize, mut apply: impl FnMut(usize, usize)) {
    let end = start + length;
    let mut anchor = restart_anchor(start);
    while anchor < end {
        apply(anchor, (anchor + RESTART_STRIDE).min(end));
        anchor += RESTART_STRIDE;
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
) -> Result<(), PipelineError> {
    let width = image.width() as usize;
    let height = image.height() as usize;
    let gain = f64::from(amount) / 100.0;
    let mut scratch_a = Vec::new();
    let mut scratch_b = Vec::new();
    for y in (0..height).step_by(tile_height.max(1)) {
        for x in (0..width).step_by(tile_width.max(1)) {
            let region = Rect {
                x,
                y,
                width: tile_width.max(1).min(width - x),
                height: tile_height.max(1).min(height - y),
            };
            let first_anchor_y = restart_anchor(region.y);
            let scratch_height = region.y - first_anchor_y + region.height + 2 * RADIUS;
            try_resize_scratch(&mut scratch_a, region.width, scratch_height)?;
            try_resize_scratch(&mut scratch_b, region.width, scratch_height)?;
            for scratch_y in 0..scratch_height {
                let global_y = first_anchor_y as isize + scratch_y as isize - RADIUS as isize;
                let row_start = scratch_y * region.width;
                for_each_restart_chunk(region.x, region.width, |anchor_x, end_x| {
                    let mut sum_a = 0.0;
                    let mut sum_b = 0.0;
                    for offset in -(RADIUS as isize)..=RADIUS as isize {
                        let x = anchor_x as isize + offset;
                        sum_a += f64::from(sample(coefficient_a, width, height, x, global_y));
                        sum_b += f64::from(sample(coefficient_b, width, height, x, global_y));
                    }
                    for global_x in anchor_x..end_x {
                        if global_x > anchor_x {
                            let outgoing_x = global_x as isize - RADIUS as isize - 1;
                            let incoming_x = global_x as isize + RADIUS as isize;
                            sum_a -= f64::from(sample(
                                coefficient_a,
                                width,
                                height,
                                outgoing_x,
                                global_y,
                            ));
                            sum_a += f64::from(sample(
                                coefficient_a,
                                width,
                                height,
                                incoming_x,
                                global_y,
                            ));
                            sum_b -= f64::from(sample(
                                coefficient_b,
                                width,
                                height,
                                outgoing_x,
                                global_y,
                            ));
                            sum_b += f64::from(sample(
                                coefficient_b,
                                width,
                                height,
                                incoming_x,
                                global_y,
                            ));
                        }
                        if global_x >= region.x {
                            let position = row_start + global_x - region.x;
                            scratch_a[position] = sum_a / WINDOW;
                            scratch_b[position] = sum_b / WINDOW;
                        }
                    }
                });
            }

            for local_x in 0..region.width {
                for_each_restart_chunk(region.y, region.height, |anchor_y, end_y| {
                    let anchor_in_scratch = anchor_y - first_anchor_y;
                    let mut mean_a = 0.0;
                    let mut mean_b = 0.0;
                    for offset in 0..2 * RADIUS + 1 {
                        let position = (anchor_in_scratch + offset) * region.width + local_x;
                        mean_a += scratch_a[position];
                        mean_b += scratch_b[position];
                    }
                    for global_y in anchor_y..end_y {
                        if global_y > anchor_y {
                            let local_y = global_y - first_anchor_y;
                            let outgoing = (local_y - 1) * region.width + local_x;
                            let incoming = (local_y + 2 * RADIUS) * region.width + local_x;
                            mean_a -= scratch_a[outgoing];
                            mean_a += scratch_a[incoming];
                            mean_b -= scratch_b[outgoing];
                            mean_b += scratch_b[incoming];
                        }
                        if global_y >= region.y {
                            let averaged_a = mean_a / WINDOW;
                            let averaged_b = mean_b / WINDOW;
                            let x = region.x + local_x;
                            let index = global_y * width + x;
                            let transformed = f64::from(guide[index]);
                            let base = averaged_a * transformed + averaged_b;
                            let detail = transformed - base;
                            let bounded_detail = bound_detail(detail);
                            let adjusted = transformed + gain * bounded_detail;
                            let luminance_delta =
                                MIDDLE_GRAY * (adjusted.sinh() - transformed.sinh());
                            let pixel = &mut image.pixels_mut()[index];
                            pixel.red = finite_f32(f64::from(pixel.red) + luminance_delta);
                            pixel.green = finite_f32(f64::from(pixel.green) + luminance_delta);
                            pixel.blue = finite_f32(f64::from(pixel.blue) + luminance_delta);
                        }
                    }
                });
            }
        }
    }
    Ok(())
}

fn try_resize_scratch(
    scratch: &mut Vec<f64>,
    width: usize,
    height: usize,
) -> Result<(), PipelineError> {
    let length = width
        .checked_mul(height)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    if length > scratch.len() {
        scratch
            .try_reserve_exact(length - scratch.len())
            .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    }
    scratch.resize(length, 0.0);
    Ok(())
}

fn bound_detail(detail: f64) -> f64 {
    detail / (1.0 + (detail / DETAIL_LIMIT).powi(2)).sqrt()
}

fn sample(source: &[f32], width: usize, height: usize, x: isize, y: isize) -> f32 {
    source[reflect101(y, height) * width + reflect101(x, width)]
}

#[cfg(test)]
fn peak_working_bytes(width: usize, height: usize, tile_width: usize, tile_height: usize) -> usize {
    let planes = width * height * 3 * size_of::<f32>();
    let scratch = 2
        * tile_width.min(width)
        * (tile_height.min(height) + RESTART_STRIDE - 1 + 2 * RADIUS)
        * size_of::<f64>();
    planes + scratch
}

#[cfg(test)]
fn rolling_scalar_accumulations(
    width: usize,
    height: usize,
    tile_width: usize,
    tile_height: usize,
    radius: usize,
) -> usize {
    let tile_width = tile_width.max(1);
    let tile_height = tile_height.max(1);
    let window = 2 * radius + 1;
    let mut operations = 0;
    for y in (0..height).step_by(tile_height) {
        let region_height = tile_height.min(height - y);
        for x in (0..width).step_by(tile_width) {
            let region_width = tile_width.min(width - x);
            let scratch_height = y - restart_anchor(y) + region_height + 2 * radius;
            let mut horizontal = 0;
            for_each_restart_chunk(x, region_width, |anchor, end| {
                horizontal += window + 2 * (end - anchor).saturating_sub(1);
            });
            operations += scratch_height * horizontal;
            let mut vertical = 0;
            for_each_restart_chunk(y, region_height, |anchor, end| {
                vertical += window + 2 * (end - anchor).saturating_sub(1);
            });
            operations += region_width * vertical;
        }
    }
    operations
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

    fn direct_box_reference(source: &[f32], width: usize, height: usize, square: bool) -> Vec<f32> {
        let mut horizontal = vec![0.0_f64; width * (height + 2 * RADIUS)];
        for scratch_y in 0..height + 2 * RADIUS {
            let y = scratch_y as isize - RADIUS as isize;
            for x in 0..width {
                let mut sum = 0.0;
                for offset in -(RADIUS as isize)..=RADIUS as isize {
                    let value = f64::from(sample(source, width, height, x as isize + offset, y));
                    sum += if square { value * value } else { value };
                }
                horizontal[scratch_y * width + x] = sum / WINDOW;
            }
        }
        let mut output = vec![0.0; width * height];
        for y in 0..height {
            for x in 0..width {
                let sum = (0..2 * RADIUS + 1)
                    .map(|offset| horizontal[(y + offset) * width + x])
                    .sum::<f64>();
                output[y * width + x] = finite_f32(sum / WINDOW);
            }
        }
        output
    }

    #[test]
    fn arbitrary_tiles_are_bit_identical_to_full_frame() {
        let source = patterned(37, 23);
        let mut full = source.clone();
        apply_tiled(&mut full, 73.0, 37, 23).unwrap();
        for tile in [(1, 1), (7, 5), (16, 8), (36, 22)] {
            let mut tiled = source.clone();
            apply_tiled(&mut tiled, 73.0, tile.0, tile.1).unwrap();
            assert_eq!(tiled, full, "tile={}x{}", tile.0, tile.1);
        }
    }

    #[test]
    fn adversarial_signed_guides_are_bit_exact_across_caller_partitions() {
        let values = [-20.0_f32, 20.0, -1.0, 1.0, -0.3, 0.3, -1.0e-7, 1.0e-7, 0.0];
        let width = 47;
        let height = 19;
        let guide = (0..width * height)
            .map(|index| values[(index * 17 + index / width * 5) % values.len()])
            .collect::<Vec<_>>();
        for square in [false, true] {
            let canonical = box_filter_tiled(&guide, width, height, square, width, height).unwrap();
            for tile in [(1, 1), (3, 7), (15, 16), (17, 5), (31, 18)] {
                assert_eq!(
                    box_filter_tiled(&guide, width, height, square, tile.0, tile.1).unwrap(),
                    canonical,
                    "square={square}, tile={}x{}",
                    tile.0,
                    tile.1
                );
            }
        }

        let pixels = guide
            .iter()
            .map(|value| {
                let y = finite_f32(MIDDLE_GRAY * f64::from(*value).sinh());
                RgbaPixel::new(y, y, y, 0.37).unwrap()
            })
            .collect();
        let source = CpuImage::new(width as u32, height as u32, pixels).unwrap();
        let mut canonical = source.clone();
        apply_tiled(&mut canonical, 100.0, width, height).unwrap();
        for tile in [(1, 1), (3, 7), (15, 16), (17, 5), (31, 18)] {
            let mut partitioned = source.clone();
            apply_tiled(&mut partitioned, 100.0, tile.0, tile.1).unwrap();
            assert_eq!(partitioned, canonical);
        }
    }

    #[test]
    fn rolling_sum_matches_the_direct_filter_with_tight_numeric_parity() {
        let source = patterned(41, 27);
        let guide = source
            .pixels()
            .iter()
            .map(|pixel| {
                let y = REC2020_LUMA[0] * f64::from(pixel.red)
                    + REC2020_LUMA[1] * f64::from(pixel.green)
                    + REC2020_LUMA[2] * f64::from(pixel.blue);
                finite_f32((y / MIDDLE_GRAY).asinh())
            })
            .collect::<Vec<_>>();
        for square in [false, true] {
            let reference = direct_box_reference(&guide, 41, 27, square);
            for tile in [(1, 1), (7, 5), (16, 8), (41, 27)] {
                let rolling = box_filter_tiled(&guide, 41, 27, square, tile.0, tile.1).unwrap();
                for (actual, expected) in rolling.iter().zip(&reference) {
                    let tolerance = 8.0 * f32::EPSILON * expected.abs().max(1.0);
                    assert!((actual - expected).abs() <= tolerance);
                }
            }
        }
    }

    #[test]
    fn degenerate_extents_and_reflected_halos_are_stable() {
        for (width, height) in [(1, 1), (1, 19), (21, 1), (19, 17)] {
            let mut first = patterned(width, height);
            let mut second = first.clone();
            apply_tiled(&mut first, -100.0, 1, 1).unwrap();
            apply_tiled(&mut second, -100.0, width, height).unwrap();
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
        let expected_scratch = 2 * 128 * (64 + RESTART_STRIDE - 1 + 2 * RADIUS) * 8;
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

    #[test]
    fn rolling_box_work_is_linear_and_its_steady_state_is_radius_independent() {
        let pixels = 6000 * 4000;
        let one_box = rolling_scalar_accumulations(6000, 4000, 128, 64, RADIUS);
        let four_boxes = 4 * one_box;
        let old_direct_work = 4 * pixels * 2 * (2 * RADIUS + 1);
        assert!(four_boxes < 28 * pixels);
        assert!(four_boxes * 5 < old_direct_work);

        let line = 4096;
        let radius_two = (2 * 2 + 1) + 2 * (line - 1);
        let radius_thirty_two = (2 * 32 + 1) + 2 * (line - 1);
        assert_eq!(radius_thirty_two - radius_two, 2 * (32 - 2));
    }

    #[test]
    fn clarity_numeric_golden_pins_the_filter_constants_and_order() {
        let mut image = patterned(5, 3);
        apply_tiled(&mut image, 73.0, TILE_WIDTH, TILE_HEIGHT).unwrap();
        let red_bits = image
            .pixels()
            .iter()
            .map(|pixel| pixel.red().to_bits())
            .collect::<Vec<_>>();
        assert_eq!(
            red_bits,
            vec![
                3_225_341_688,
                3_212_909_635,
                1_064_522_721,
                1_077_579_233,
                3_213_802_183,
                1_062_745_877,
                1_077_133_038,
                3_214_695_121,
                1_060_969_675,
                1_076_687_022,
                3_215_588_063,
                1_059_194_490,
                1_076_241_204,
                3_216_481_872,
                1_057_420_164,
            ]
        );
    }
}
