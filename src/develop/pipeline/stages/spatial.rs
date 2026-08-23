//! Shared deterministic bounded-memory spatial primitives.
//!
//! A filter request always carries the full image extent plus an output ROI.
//! Every ROI reads its halo from global full-frame coordinates with Reflect101;
//! tiles therefore produce exactly the same samples as a single full-frame ROI.
//! Pixels are stored as `f32`, while each convolution uses a fixed-order `f64`
//! accumulator. The implementation allocates one full scalar output (4 B/pixel)
//! and reuses a scratch halo of `tile_width * (tile_height + 2*radius) * 4`
//! bytes. Pyramid construction retains only the current level and dimension
//! metadata; the geometric low-resolution tail is bounded below 4/3 of a plane.

use crate::{develop::PipelineError, io::LimitError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Rect {
    pub(super) x: usize,
    pub(super) y: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

#[derive(Debug, PartialEq)]
pub(super) struct Plane {
    width: usize,
    height: usize,
    pixels: Vec<f32>,
}

impl Plane {
    pub(super) fn new(width: usize, height: usize, pixels: Vec<f32>) -> Self {
        debug_assert_eq!(pixels.len(), width * height);
        Self {
            width,
            height,
            pixels,
        }
    }

    pub(super) fn try_filled(
        width: usize,
        height: usize,
        value: f32,
    ) -> Result<Self, PipelineError> {
        let length = width
            .checked_mul(height)
            .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(length)
            .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
        pixels.resize(length, value);
        Ok(Self::new(width, height, pixels))
    }

    pub(super) fn pixels(&self) -> &[f32] {
        &self.pixels
    }

    pub(super) fn try_clone(&self) -> Result<Self, PipelineError> {
        try_clone_plane(self)
    }

    fn sample(&self, x: isize, y: isize) -> f32 {
        let x = reflect101(x, self.width);
        let y = reflect101(y, self.height);
        self.pixels[y * self.width + x]
    }
}

pub(super) fn gaussian_kernel(sigma: f64) -> Vec<f64> {
    if sigma <= f64::EPSILON {
        return vec![1.0];
    }
    let radius = (sigma * 3.0).ceil() as usize;
    let denominator = 2.0 * sigma * sigma;
    let mut kernel = (-(radius as isize)..=(radius as isize))
        .map(|offset| {
            let distance = offset as f64;
            (-distance * distance / denominator).exp()
        })
        .collect::<Vec<_>>();
    let sum: f64 = kernel.iter().sum();
    for weight in &mut kernel {
        *weight /= sum;
    }
    kernel
}

pub(super) fn gaussian_blur(source: &Plane, sigma: f64) -> Result<Plane, PipelineError> {
    gaussian_blur_tiled(source, sigma, 128, 64)
}

pub(super) fn gaussian_blur_tiled(
    source: &Plane,
    sigma: f64,
    tile_width: usize,
    tile_height: usize,
) -> Result<Plane, PipelineError> {
    let kernel = try_gaussian_kernel(sigma)?;
    if kernel.len() == 1 {
        return try_clone_plane(source);
    }
    let mut output = Plane::try_filled(source.width, source.height, 0.0)?;
    let mut scratch = Vec::new();
    let tile_width = tile_width.max(1);
    let tile_height = tile_height.max(1);
    for y in (0..source.height).step_by(tile_height) {
        for x in (0..source.width).step_by(tile_width) {
            let roi = Rect {
                x,
                y,
                width: tile_width.min(source.width - x),
                height: tile_height.min(source.height - y),
            };
            gaussian_roi(source, &mut output, roi, &kernel, &mut scratch)?;
        }
    }
    Ok(output)
}

fn try_gaussian_kernel(sigma: f64) -> Result<Vec<f64>, PipelineError> {
    let radius = if sigma <= f64::EPSILON {
        0
    } else {
        (sigma * 3.0).ceil() as usize
    };
    let length = radius
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let mut kernel = Vec::new();
    kernel
        .try_reserve_exact(length)
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    if radius == 0 {
        kernel.push(1.0);
        return Ok(kernel);
    }
    let denominator = 2.0 * sigma * sigma;
    for offset in -(radius as isize)..=(radius as isize) {
        let distance = offset as f64;
        kernel.push((-distance * distance / denominator).exp());
    }
    let sum: f64 = kernel.iter().sum();
    for weight in &mut kernel {
        *weight /= sum;
    }
    Ok(kernel)
}

fn try_clone_plane(source: &Plane) -> Result<Plane, PipelineError> {
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(source.pixels.len())
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    pixels.extend_from_slice(&source.pixels);
    Ok(Plane::new(source.width, source.height, pixels))
}

/// Filters `roi` against `source`'s full extent. The scratch halo represents
/// raw global y coordinates; Reflect101 is applied only when sampling the full
/// image, making the result independent of ROI boundaries.
fn gaussian_roi(
    source: &Plane,
    output: &mut Plane,
    roi: Rect,
    kernel: &[f64],
    scratch: &mut Vec<f32>,
) -> Result<(), PipelineError> {
    let radius = kernel.len() / 2;
    let scratch_height = roi.height + 2 * radius;
    let scratch_length = roi
        .width
        .checked_mul(scratch_height)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    if scratch_length > scratch.capacity() {
        scratch
            .try_reserve_exact(scratch_length - scratch.len())
            .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    }
    scratch.resize(scratch_length, 0.0);

    for scratch_y in 0..scratch_height {
        let global_y = roi.y as isize + scratch_y as isize - radius as isize;
        for local_x in 0..roi.width {
            let global_x = roi.x + local_x;
            let mut sum = 0.0_f64;
            for (index, weight) in kernel.iter().copied().enumerate() {
                let sample_x = global_x as isize + index as isize - radius as isize;
                sum += f64::from(source.sample(sample_x, global_y)) * weight;
            }
            scratch[scratch_y * roi.width + local_x] = finite_f32(sum);
        }
    }

    for local_y in 0..roi.height {
        for local_x in 0..roi.width {
            let mut sum = 0.0_f64;
            for (index, weight) in kernel.iter().copied().enumerate() {
                sum += f64::from(scratch[(local_y + index) * roi.width + local_x]) * weight;
            }
            output.pixels[(roi.y + local_y) * output.width + roi.x + local_x] = finite_f32(sum);
        }
    }
    Ok(())
}

/// Applies a blur whose sigma is specified in level-zero pixels. The image is
/// reduced until the residual sigma is at most 2.5 pixels, filtered once, then
/// reconstructed through the recorded full-frame dimensions.
pub(super) fn pyramid_blur(source: Plane, sigma_full: f64) -> Result<Plane, PipelineError> {
    if sigma_full <= f64::EPSILON {
        return Ok(source);
    }
    let mut current = source;
    let mut dimensions = Vec::new();
    let mut residual_sigma = sigma_full;
    while residual_sigma > 2.5 && (current.width > 2 || current.height > 2) {
        dimensions
            .try_reserve_exact(1)
            .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
        dimensions.push((current.width, current.height));
        current = downsample2_reflect101(&current)?;
        residual_sigma *= 0.5;
    }
    current = gaussian_blur(&current, residual_sigma.max(0.5))?;
    for (width, height) in dimensions.into_iter().rev() {
        current = upsample_bilinear(&current, width, height)?;
    }
    Ok(current)
}

fn downsample2_reflect101(source: &Plane) -> Result<Plane, PipelineError> {
    let width = source.width.div_ceil(2);
    let height = source.height.div_ceil(2);
    let length = width
        .checked_mul(height)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(length)
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    for y in 0..height {
        for x in 0..width {
            let x0 = (2 * x) as isize;
            let y0 = (2 * y) as isize;
            let sum = f64::from(source.sample(x0, y0))
                + f64::from(source.sample(x0 + 1, y0))
                + f64::from(source.sample(x0, y0 + 1))
                + f64::from(source.sample(x0 + 1, y0 + 1));
            pixels.push(finite_f32(sum * 0.25));
        }
    }
    Ok(Plane::new(width, height, pixels))
}

fn upsample_bilinear(source: &Plane, width: usize, height: usize) -> Result<Plane, PipelineError> {
    let length = width
        .checked_mul(height)
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(length)
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    let scale_x = source.width as f64 / width as f64;
    let scale_y = source.height as f64 / height as f64;
    for y in 0..height {
        let source_y = (y as f64 + 0.5) * scale_y - 0.5;
        let y0 = source_y.floor();
        let fy = source_y - y0;
        for x in 0..width {
            let source_x = (x as f64 + 0.5) * scale_x - 0.5;
            let x0 = source_x.floor();
            let fx = source_x - x0;
            let top = lerp(
                f64::from(source.sample(x0 as isize, y0 as isize)),
                f64::from(source.sample(x0 as isize + 1, y0 as isize)),
                fx,
            );
            let bottom = lerp(
                f64::from(source.sample(x0 as isize, y0 as isize + 1)),
                f64::from(source.sample(x0 as isize + 1, y0 as isize + 1)),
                fx,
            );
            pixels.push(finite_f32(lerp(top, bottom, fy)));
        }
    }
    Ok(Plane::new(width, height, pixels))
}

fn lerp(left: f64, right: f64, fraction: f64) -> f64 {
    left + (right - left) * fraction
}

pub(super) fn reflect101(index: isize, length: usize) -> usize {
    if length <= 1 {
        return 0;
    }
    let period = 2 * (length - 1) as isize;
    let wrapped = index.rem_euclid(period);
    if wrapped < length as isize {
        wrapped as usize
    } else {
        (period - wrapped) as usize
    }
}

pub(super) fn finite_f32(value: f64) -> f32 {
    value.clamp(-(f32::MAX as f64), f32::MAX as f64) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflect101_and_odd_downsample_do_not_repeat_edges() {
        let samples = (-4..=8)
            .map(|index| reflect101(index, 4))
            .collect::<Vec<_>>();
        assert_eq!(samples, vec![2, 3, 2, 1, 0, 1, 2, 3, 2, 1, 0, 1, 2]);
        let odd = Plane::new(3, 1, vec![1.0, 2.0, 9.0]);
        assert_eq!(downsample2_reflect101(&odd).unwrap().pixels(), &[1.5, 5.5]);
    }

    #[test]
    fn kernel_is_symmetric_normalized_and_deterministic() {
        let first = gaussian_kernel(1.4);
        assert_eq!(first, gaussian_kernel(1.4));
        assert!((first.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        for index in 0..first.len() {
            assert_eq!(first[index], first[first.len() - 1 - index]);
        }
    }

    #[test]
    fn split_tiles_are_bit_identical_to_full_roi_for_boundaries_and_radii() {
        let pixels = (0..37 * 23)
            .map(|index| ((index * 37 % 101) as f32 - 50.0) / 13.0)
            .collect();
        let source = Plane::new(37, 23, pixels);
        for sigma in [0.55, 1.3, 3.8, 7.0] {
            let full = gaussian_blur_tiled(&source, sigma, 37, 23).unwrap();
            for (tile_width, tile_height) in [(1, 1), (7, 5), (16, 8), (36, 22)] {
                assert_eq!(
                    gaussian_blur_tiled(&source, sigma, tile_width, tile_height).unwrap(),
                    full,
                    "sigma={sigma}, tile={tile_width}x{tile_height}"
                );
            }
        }
    }

    #[test]
    fn gaussian_preserves_constant_and_impulse_energy() {
        let constant = Plane::try_filled(31, 17, 0.3).unwrap();
        assert_eq!(gaussian_blur(&constant, 4.0).unwrap(), constant);

        let mut impulse = vec![0.0; 41 * 41];
        impulse[20 * 41 + 20] = 1.0;
        let filtered = gaussian_blur(&Plane::new(41, 41, impulse), 2.0).unwrap();
        let energy: f64 = filtered
            .pixels()
            .iter()
            .map(|value| f64::from(*value))
            .sum();
        assert!((energy - 1.0).abs() < 2e-6);
    }
}
