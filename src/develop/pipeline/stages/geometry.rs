//! Deterministic CPU geometry in edge coordinates.
//!
//! Image edges span `[0, width] x [0, height]`; pixel `(x, y)` is sampled at
//! `(x + 0.5, y + 0.5)`. Projective transforms are inverse mapped so every
//! destination pixel has one stable source coordinate.

use crate::develop::{
    CpuImage, PipelineError, RgbaPixel,
    orientation::apply_orthogonal_transform,
    settings::{CropRect, GeometrySettings},
};
use crate::io::LimitError;

const LANCZOS_RADIUS: i32 = 3;

pub(super) fn supports(_settings: &GeometrySettings) -> bool {
    true
}

pub(super) fn apply(
    image: &mut CpuImage,
    settings: &GeometrySettings,
) -> Result<(), PipelineError> {
    if settings.is_neutral() {
        return Ok(());
    }

    let orthogonal_active =
        settings.quarter_turns_clockwise != 0 || settings.flip_horizontal || settings.flip_vertical;
    let mut rendered = if orthogonal_active {
        Some(apply_orthogonal_transform(
            image,
            settings.quarter_turns_clockwise,
            settings.flip_horizontal,
            settings.flip_vertical,
        )?)
    } else {
        None
    };

    if settings.straighten_degrees != 0.0
        || settings.perspective_horizontal != 0.0
        || settings.perspective_vertical != 0.0
    {
        let source = rendered.as_ref().unwrap_or(image);
        rendered = Some(projective_resample(
            source,
            settings.straighten_degrees,
            settings.perspective_horizontal,
            settings.perspective_vertical,
        )?);
    }

    if let Some(crop) = &settings.crop {
        let source = rendered.as_ref().unwrap_or(image);
        let bounds = crop_bounds(source.width(), source.height(), crop);
        if bounds != (0, 0, source.width(), source.height()) {
            rendered = Some(crop_region(source, bounds)?);
        }
    }
    if let Some(rendered) = rendered {
        *image = rendered;
    }
    Ok(())
}

fn crop_bounds(width: u32, height: u32, crop: &CropRect) -> (u32, u32, u32, u32) {
    let left = normalized_edge(crop.x, width, false);
    let top = normalized_edge(crop.y, height, false);
    let right_edge = (f64::from(crop.x) + f64::from(crop.width)) * f64::from(width);
    let bottom_edge = (f64::from(crop.y) + f64::from(crop.height)) * f64::from(height);
    let right = (right_edge.ceil() as u32).min(width).max(left + 1);
    let bottom = (bottom_edge.ceil() as u32).min(height).max(top + 1);
    let output_width = right.min(width) - left;
    let output_height = bottom.min(height) - top;
    (left, top, output_width, output_height)
}

fn crop_region(
    source: &CpuImage,
    (left, top, output_width, output_height): (u32, u32, u32, u32),
) -> Result<CpuImage, PipelineError> {
    let count = pixel_count(output_width, output_height);
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    for y in top..top + output_height {
        let start = index(source.width(), left, y);
        pixels.extend_from_slice(&source.pixels()[start..start + output_width as usize]);
    }
    CpuImage::new(output_width, output_height, pixels).map_err(PipelineError::InvalidImage)
}

fn normalized_edge(value: f32, extent: u32, upper: bool) -> u32 {
    let edge = f64::from(value) * f64::from(extent);
    let rounded = if upper { edge.ceil() } else { edge.floor() };
    rounded.clamp(0.0, f64::from(extent)) as u32
}

fn projective_resample(
    source: &CpuImage,
    straighten_degrees: f32,
    perspective_horizontal: f32,
    perspective_vertical: f32,
) -> Result<CpuImage, PipelineError> {
    let width = f64::from(source.width());
    let height = f64::from(source.height());
    let center_x = width * 0.5;
    let center_y = height * 0.5;
    let radians = -f64::from(straighten_degrees).to_radians();
    let (sin, cos) = radians.sin_cos();
    let rotate = Homography([
        [cos, -sin, center_x - cos * center_x + sin * center_y],
        [sin, cos, center_y - sin * center_x - cos * center_y],
        [0.0, 0.0, 1.0],
    ]);
    let perspective = Homography([
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [
            0.75 * f64::from(perspective_horizontal) / (100.0 * width),
            0.75 * f64::from(perspective_vertical) / (100.0 * height),
            1.0 - 0.75 * f64::from(perspective_horizontal) * center_x / (100.0 * width)
                - 0.75 * f64::from(perspective_vertical) * center_y / (100.0 * height),
        ],
    ]);
    let inverse = perspective.multiply(rotate).inverse();
    let pixels = projective_region(source, inverse, 0, 0, source.width(), source.height())?;
    CpuImage::new(source.width(), source.height(), pixels).map_err(PipelineError::InvalidImage)
}

/// Renders an ROI using full-frame destination coordinates. A scheduler may
/// split the output at arbitrary boundaries without changing a sample.
fn projective_region(
    source: &CpuImage,
    inverse: Homography,
    output_x: u32,
    output_y: u32,
    width: u32,
    height: u32,
) -> Result<Vec<RgbaPixel>, PipelineError> {
    let count = pixel_count(width, height);
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| PipelineError::ResourceLimit(LimitError::Allocation))?;
    for y in output_y..output_y + height {
        for x in output_x..output_x + width {
            let destination = [f64::from(x) + 0.5, f64::from(y) + 0.5];
            let mapped = inverse.apply(destination);
            pixels.push(sample_lanczos3(source, mapped[0], mapped[1]));
        }
    }
    Ok(pixels)
}

/// Returns post-geometry dimensions and the maximum simultaneous heap payload
/// allocated by this stage while the outer transaction image remains live.
pub(super) fn working_set(
    width: u32,
    height: u32,
    settings: &GeometrySettings,
) -> Result<(u32, u32, u64), PipelineError> {
    let image_bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(16))
        .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
    let orthogonal_active =
        settings.quarter_turns_clockwise != 0 || settings.flip_horizontal || settings.flip_vertical;
    let projective_active = settings.straighten_degrees != 0.0
        || settings.perspective_horizontal != 0.0
        || settings.perspective_vertical != 0.0;
    let (transformed_width, transformed_height) =
        if settings.quarter_turns_clockwise.is_multiple_of(2) {
            (width, height)
        } else {
            (height, width)
        };
    let mut current_payload = if orthogonal_active { image_bytes } else { 0 };
    let mut peak = current_payload;
    if projective_active {
        peak = peak.max(
            current_payload
                .checked_add(image_bytes)
                .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?,
        );
        current_payload = image_bytes;
    }
    let (output_width, output_height, crop_bytes) = if let Some(crop) = &settings.crop {
        let (_, _, crop_width, crop_height) =
            crop_bounds(transformed_width, transformed_height, crop);
        let bytes = u64::from(crop_width)
            .checked_mul(u64::from(crop_height))
            .and_then(|pixels| pixels.checked_mul(16))
            .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?;
        (crop_width, crop_height, bytes)
    } else {
        (transformed_width, transformed_height, image_bytes)
    };
    if crop_bytes != image_bytes {
        peak = peak.max(
            current_payload
                .checked_add(crop_bytes)
                .ok_or(PipelineError::ResourceLimit(LimitError::ArithmeticOverflow))?,
        );
    }
    Ok((output_width, output_height, peak))
}

#[derive(Clone, Copy)]
struct Homography([[f64; 3]; 3]);

impl Homography {
    fn multiply(self, rhs: Self) -> Self {
        let mut result = [[0.0; 3]; 3];
        for (row, values) in result.iter_mut().enumerate() {
            for (column, value) in values.iter_mut().enumerate() {
                *value = (0..3).map(|k| self.0[row][k] * rhs.0[k][column]).sum();
            }
        }
        Self(result)
    }

    fn inverse(self) -> Self {
        let m = self.0;
        let determinant = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
        debug_assert!(determinant.abs() > 1.0e-12);
        let reciprocal = determinant.recip();
        Self([
            [
                (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * reciprocal,
                (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * reciprocal,
                (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * reciprocal,
            ],
            [
                (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * reciprocal,
                (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * reciprocal,
                (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * reciprocal,
            ],
            [
                (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * reciprocal,
                (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * reciprocal,
                (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * reciprocal,
            ],
        ])
    }

    fn apply(self, point: [f64; 2]) -> [f64; 2] {
        let denominator = self.0[2][0] * point[0] + self.0[2][1] * point[1] + self.0[2][2];
        if denominator.abs() < 1.0e-12 {
            return [f64::INFINITY; 2];
        }
        [
            (self.0[0][0] * point[0] + self.0[0][1] * point[1] + self.0[0][2]) / denominator,
            (self.0[1][0] * point[0] + self.0[1][1] * point[1] + self.0[1][2]) / denominator,
        ]
    }
}

fn sample_lanczos3(source: &CpuImage, source_x: f64, source_y: f64) -> RgbaPixel {
    if !source_x.is_finite() || !source_y.is_finite() {
        return transparent();
    }
    let center_x = source_x - 0.5;
    let center_y = source_y - 0.5;
    let base_x = center_x.floor() as i64;
    let base_y = center_y.floor() as i64;
    let mut premultiplied = [0.0_f64; 4];
    for offset_y in -(LANCZOS_RADIUS - 1)..=LANCZOS_RADIUS {
        let sample_y = base_y + i64::from(offset_y);
        let weight_y = lanczos(center_y - sample_y as f64);
        for offset_x in -(LANCZOS_RADIUS - 1)..=LANCZOS_RADIUS {
            let sample_x = base_x + i64::from(offset_x);
            let weight = weight_y * lanczos(center_x - sample_x as f64);
            if sample_x < 0
                || sample_y < 0
                || sample_x >= i64::from(source.width())
                || sample_y >= i64::from(source.height())
            {
                continue;
            }
            let pixel = source.pixels()[index(source.width(), sample_x as u32, sample_y as u32)];
            let alpha = f64::from(pixel.alpha);
            premultiplied[0] += weight * f64::from(pixel.red) * alpha;
            premultiplied[1] += weight * f64::from(pixel.green) * alpha;
            premultiplied[2] += weight * f64::from(pixel.blue) * alpha;
            premultiplied[3] += weight * alpha;
        }
    }
    let accumulated_alpha = premultiplied[3];
    if !accumulated_alpha.is_finite() || accumulated_alpha <= 0.0 {
        return transparent();
    }
    let alpha = accumulated_alpha.clamp(0.0, 1.0);
    RgbaPixel {
        red: (premultiplied[0] / accumulated_alpha) as f32,
        green: (premultiplied[1] / accumulated_alpha) as f32,
        blue: (premultiplied[2] / accumulated_alpha) as f32,
        alpha: alpha as f32,
    }
}

fn lanczos(distance: f64) -> f64 {
    let absolute = distance.abs();
    if absolute < 1.0e-12 {
        1.0
    } else if absolute >= f64::from(LANCZOS_RADIUS) {
        0.0
    } else {
        let pi_distance = std::f64::consts::PI * distance;
        (pi_distance.sin() / pi_distance)
            * ((pi_distance / f64::from(LANCZOS_RADIUS)).sin()
                / (pi_distance / f64::from(LANCZOS_RADIUS)))
    }
}

fn transparent() -> RgbaPixel {
    RgbaPixel {
        red: 0.0,
        green: 0.0,
        blue: 0.0,
        alpha: 0.0,
    }
}

fn pixel_count(width: u32, height: u32) -> usize {
    usize::try_from(u64::from(width) * u64::from(height)).expect("validated image dimensions")
}

fn index(width: u32, x: u32, y: u32) -> usize {
    (u64::from(y) * u64::from(width) + u64::from(x)) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered(width: u32, height: u32) -> CpuImage {
        let pixels = (0..width * height)
            .map(|value| RgbaPixel::new(value as f32, 0.0, 0.0, 1.0).unwrap())
            .collect();
        CpuImage::new(width, height, pixels).unwrap()
    }

    #[test]
    fn clockwise_quarter_turn_is_exact_and_swaps_dimensions() {
        let output = apply_orthogonal_transform(&numbered(3, 2), 1, false, false).unwrap();
        assert_eq!((output.width(), output.height()), (2, 3));
        assert_eq!(
            output
                .pixels()
                .iter()
                .map(RgbaPixel::red)
                .collect::<Vec<_>>(),
            vec![3.0, 0.0, 4.0, 1.0, 5.0, 2.0]
        );
    }

    #[test]
    fn homography_inverse_roundtrips_points() {
        let transform = Homography([[1.1, 0.2, 3.0], [-0.1, 0.9, 4.0], [0.003, -0.002, 1.0]]);
        let point = [17.25, 9.75];
        let roundtrip = transform.inverse().apply(transform.apply(point));
        assert!((roundtrip[0] - point[0]).abs() < 1.0e-10);
        assert!((roundtrip[1] - point[1]).abs() < 1.0e-10);
    }

    #[test]
    fn transparent_pixels_do_not_bleed_hidden_rgb() {
        let source = CpuImage::new(
            2,
            1,
            vec![
                RgbaPixel::new(1000.0, -500.0, 2.0, 0.0).unwrap(),
                RgbaPixel::new(0.5, 0.25, 4.0, 1.0).unwrap(),
            ],
        )
        .unwrap();
        let sampled = sample_lanczos3(&source, 1.0, 0.5);
        assert!((sampled.red() - 0.5).abs() < 1.0e-6);
        assert!((sampled.green() - 0.25).abs() < 1.0e-6);
        assert!((sampled.blue() - 4.0).abs() < 1.0e-6);
    }

    #[test]
    fn every_positive_f32_alpha_survives_center_sampling() {
        for alpha in [
            1.0e-8,
            f32::MIN_POSITIVE,
            f32::from_bits(f32::MIN_POSITIVE.to_bits() - 1),
            f32::from_bits(1),
        ] {
            let source =
                CpuImage::new(1, 1, vec![RgbaPixel::new(-3.0, 0.25, 12.0, alpha).unwrap()])
                    .unwrap();
            let sampled = sample_lanczos3(&source, 0.5, 0.5);
            assert_eq!(sampled.alpha().to_bits(), alpha.to_bits());
            assert_eq!(sampled.red(), -3.0);
            assert_eq!(sampled.green(), 0.25);
            assert_eq!(sampled.blue(), 12.0);
        }
    }

    #[test]
    fn fully_transparent_border_has_no_hidden_rgb() {
        let source = CpuImage::new(
            1,
            1,
            vec![RgbaPixel::new(f32::MAX, -f32::MAX, 42.0, 0.0).unwrap()],
        )
        .unwrap();
        assert_eq!(sample_lanczos3(&source, -0.25, 0.5), transparent());
        assert_eq!(sample_lanczos3(&source, 4.0, 4.0), transparent());
    }

    #[test]
    fn projective_roi_splits_are_bit_exact_to_full_frame() {
        let source = numbered(9, 7);
        let inverse = Homography([
            [0.97, 0.08, -0.25],
            [-0.04, 1.02, 0.30],
            [0.001, -0.002, 1.0],
        ]);
        let full = projective_region(&source, inverse, 0, 0, 9, 7).unwrap();
        let mut tiled = Vec::new();
        for y in 0..7 {
            tiled.extend(projective_region(&source, inverse, 0, y, 4, 1).unwrap());
            tiled.extend(projective_region(&source, inverse, 4, y, 5, 1).unwrap());
        }
        assert_eq!(tiled, full);
    }
}
