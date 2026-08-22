//! Deterministic, full-frame spatial primitives for CPU effects.
//!
//! The implementation deliberately uses a fixed traversal and accumulation
//! order. It never splits an image into independently filtered tiles, so large
//! radii cannot introduce tile-boundary seams.

use crate::develop::CpuImage;

#[derive(Clone, Debug)]
pub(super) struct RgbImage {
    width: usize,
    height: usize,
    pixels: Vec<[f64; 3]>,
}

impl RgbImage {
    pub(super) fn from_cpu(image: &CpuImage) -> Self {
        Self {
            width: image.width() as usize,
            height: image.height() as usize,
            pixels: image
                .pixels()
                .iter()
                .map(|pixel| {
                    [
                        f64::from(pixel.red()),
                        f64::from(pixel.green()),
                        f64::from(pixel.blue()),
                    ]
                })
                .collect(),
        }
    }

    pub(super) fn from_pixels(width: usize, height: usize, pixels: Vec<[f64; 3]>) -> Self {
        debug_assert_eq!(pixels.len(), width * height);
        Self {
            width,
            height,
            pixels,
        }
    }

    pub(super) fn width(&self) -> usize {
        self.width
    }

    pub(super) fn height(&self) -> usize {
        self.height
    }

    pub(super) fn pixels(&self) -> &[[f64; 3]] {
        &self.pixels
    }

    pub(super) fn map_pixels(mut self, mut map: impl FnMut([f64; 3]) -> [f64; 3]) -> Self {
        for pixel in &mut self.pixels {
            *pixel = map(*pixel);
        }
        self
    }

    fn pixel(&self, x: usize, y: usize) -> [f64; 3] {
        self.pixels[y * self.width + x]
    }
}

pub(super) fn gaussian_kernel(sigma: f64) -> Vec<f64> {
    if sigma <= f64::EPSILON {
        return vec![1.0];
    }
    let radius = (sigma * 3.0).ceil() as usize;
    let mut kernel = Vec::with_capacity(radius * 2 + 1);
    let denominator = 2.0 * sigma * sigma;
    for offset in -(radius as isize)..=(radius as isize) {
        let distance = offset as f64;
        kernel.push((-distance * distance / denominator).exp());
    }
    let sum: f64 = kernel.iter().sum();
    for weight in &mut kernel {
        *weight /= sum;
    }
    kernel
}

pub(super) fn gaussian_blur(source: &RgbImage, sigma: f64) -> RgbImage {
    let kernel = gaussian_kernel(sigma);
    if kernel.len() == 1 {
        return source.clone();
    }
    let radius = kernel.len() / 2;
    let mut horizontal = vec![[0.0; 3]; source.pixels.len()];
    for y in 0..source.height {
        for x in 0..source.width {
            let mut sum = [0.0; 3];
            for (kernel_index, weight) in kernel.iter().copied().enumerate() {
                let sample_x = reflect101(
                    x as isize + kernel_index as isize - radius as isize,
                    source.width,
                );
                let sample = source.pixel(sample_x, y);
                for channel in 0..3 {
                    sum[channel] += sample[channel] * weight;
                }
            }
            horizontal[y * source.width + x] = sum;
        }
    }

    let horizontal = RgbImage::from_pixels(source.width, source.height, horizontal);
    let mut vertical = vec![[0.0; 3]; source.pixels.len()];
    for y in 0..source.height {
        for x in 0..source.width {
            let mut sum = [0.0; 3];
            for (kernel_index, weight) in kernel.iter().copied().enumerate() {
                let sample_y = reflect101(
                    y as isize + kernel_index as isize - radius as isize,
                    source.height,
                );
                let sample = horizontal.pixel(x, sample_y);
                for channel in 0..3 {
                    sum[channel] += sample[channel] * weight;
                }
            }
            vertical[y * source.width + x] = sum;
        }
    }
    RgbImage::from_pixels(source.width, source.height, vertical)
}

/// Builds a complete-image Gaussian pyramid and reconstructs it at level zero.
/// Coarser levels provide wide support without radius-sized per-pixel work.
pub(super) fn pyramid_blur(source: &RgbImage, levels: usize) -> RgbImage {
    let mut pyramid = vec![source.clone()];
    for _ in 1..levels.max(1) {
        let previous = pyramid.last().expect("level zero exists");
        if previous.width == 1 && previous.height == 1 {
            break;
        }
        pyramid.push(downsample2(&gaussian_blur(previous, 1.0)));
    }

    let mut reconstructed = gaussian_blur(pyramid.last().expect("level zero exists"), 1.2);
    for finer in pyramid.iter().rev().skip(1) {
        reconstructed = upsample_bilinear(&reconstructed, finer.width, finer.height);
        reconstructed = gaussian_blur(&reconstructed, 0.8);
    }
    reconstructed
}

fn downsample2(source: &RgbImage) -> RgbImage {
    let width = source.width.div_ceil(2);
    let height = source.height.div_ceil(2);
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0.0; 3];
            let mut samples = 0.0;
            for offset_y in 0..2 {
                for offset_x in 0..2 {
                    let sample_x = (x * 2 + offset_x).min(source.width - 1);
                    let sample_y = (y * 2 + offset_y).min(source.height - 1);
                    let sample = source.pixel(sample_x, sample_y);
                    for channel in 0..3 {
                        sum[channel] += sample[channel];
                    }
                    samples += 1.0;
                }
            }
            for value in &mut sum {
                *value /= samples;
            }
            pixels.push(sum);
        }
    }
    RgbImage::from_pixels(width, height, pixels)
}

fn upsample_bilinear(source: &RgbImage, width: usize, height: usize) -> RgbImage {
    if source.width == width && source.height == height {
        return source.clone();
    }
    let mut pixels = Vec::with_capacity(width * height);
    let scale_x = source.width as f64 / width as f64;
    let scale_y = source.height as f64 / height as f64;
    for y in 0..height {
        let source_y = ((y as f64 + 0.5) * scale_y - 0.5).max(0.0);
        let y0 = source_y.floor() as usize;
        let y1 = (y0 + 1).min(source.height - 1);
        let fraction_y = source_y - y0 as f64;
        for x in 0..width {
            let source_x = ((x as f64 + 0.5) * scale_x - 0.5).max(0.0);
            let x0 = source_x.floor() as usize;
            let x1 = (x0 + 1).min(source.width - 1);
            let fraction_x = source_x - x0 as f64;
            let top = lerp_rgb(source.pixel(x0, y0), source.pixel(x1, y0), fraction_x);
            let bottom = lerp_rgb(source.pixel(x0, y1), source.pixel(x1, y1), fraction_x);
            pixels.push(lerp_rgb(top, bottom, fraction_y));
        }
    }
    RgbImage::from_pixels(width, height, pixels)
}

fn lerp_rgb(left: [f64; 3], right: [f64; 3], fraction: f64) -> [f64; 3] {
    [
        left[0] + (right[0] - left[0]) * fraction,
        left[1] + (right[1] - left[1]) * fraction,
        left[2] + (right[2] - left[2]) * fraction,
    ]
}

fn reflect101(index: isize, length: usize) -> usize {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reflect101_has_no_repeated_edge_sample() {
        let samples = (-4..=8)
            .map(|index| reflect101(index, 4))
            .collect::<Vec<_>>();
        assert_eq!(samples, vec![2, 3, 2, 1, 0, 1, 2, 3, 2, 1, 0, 1, 2]);
        assert_eq!(reflect101(-100, 1), 0);
    }

    #[test]
    fn gaussian_kernel_is_symmetric_normalized_and_deterministic() {
        let first = gaussian_kernel(1.4);
        let second = gaussian_kernel(1.4);
        assert_eq!(first, second);
        assert!((first.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        for index in 0..first.len() {
            assert_eq!(first[index], first[first.len() - 1 - index]);
        }
    }

    #[test]
    fn gaussian_preserves_constant_and_impulse_energy() {
        let constant = RgbImage::from_pixels(7, 5, vec![[0.3, -0.2, 4.0]; 35]);
        let filtered = gaussian_blur(&constant, 1.6);
        for pixel in filtered.pixels() {
            assert!((pixel[0] - 0.3).abs() < 1e-12);
            assert!((pixel[1] + 0.2).abs() < 1e-12);
            assert!((pixel[2] - 4.0).abs() < 1e-12);
        }

        let mut impulse = vec![[0.0; 3]; 81];
        impulse[40] = [1.0, 1.0, 1.0];
        let filtered = gaussian_blur(&RgbImage::from_pixels(9, 9, impulse), 1.0);
        let energy: f64 = filtered.pixels().iter().map(|pixel| pixel[0]).sum();
        assert!((energy - 1.0).abs() < 1e-12);
    }
}
