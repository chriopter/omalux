//! Deterministic CPU geometry in edge coordinates.
//!
//! Image edges span `[0, width] x [0, height]`; pixel `(x, y)` is sampled at
//! `(x + 0.5, y + 0.5)`. Projective transforms are inverse mapped so every
//! destination pixel has one stable source coordinate.

use crate::develop::{
    CpuImage, PipelineError, RgbaPixel,
    settings::{CropRect, GeometrySettings},
};

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

    let mut rendered = exact_orthogonal_transform(
        image,
        settings.quarter_turns_clockwise,
        settings.flip_horizontal,
        settings.flip_vertical,
    )?;

    if settings.straighten_degrees != 0.0
        || settings.perspective_horizontal != 0.0
        || settings.perspective_vertical != 0.0
    {
        rendered = projective_resample(
            &rendered,
            settings.straighten_degrees,
            settings.perspective_horizontal,
            settings.perspective_vertical,
        )?;
    }

    if let Some(crop) = &settings.crop {
        rendered = normalized_crop(&rendered, crop)?;
    }
    *image = rendered;
    Ok(())
}

/// EXIF orientation is runtime input, deliberately separate from persisted
/// develop settings. Import code may call the same exact orthogonal mapping
/// before constructing a develop document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub(crate) enum ExifOrientation {
    Normal = 1,
    MirrorHorizontal = 2,
    Rotate180 = 3,
    MirrorVertical = 4,
    Transpose = 5,
    Rotate90Clockwise = 6,
    Transverse = 7,
    Rotate270Clockwise = 8,
}

#[allow(dead_code)]
pub(crate) fn apply_exif_orientation(
    image: &CpuImage,
    orientation: ExifOrientation,
) -> Result<CpuImage, PipelineError> {
    let (turns, flip_horizontal, flip_vertical) = match orientation {
        ExifOrientation::Normal => (0, false, false),
        ExifOrientation::MirrorHorizontal => (0, true, false),
        ExifOrientation::Rotate180 => (2, false, false),
        ExifOrientation::MirrorVertical => (0, false, true),
        ExifOrientation::Transpose => (1, true, false),
        ExifOrientation::Rotate90Clockwise => (1, false, false),
        ExifOrientation::Transverse => (1, false, true),
        ExifOrientation::Rotate270Clockwise => (3, false, false),
    };
    exact_orthogonal_transform(image, turns, flip_horizontal, flip_vertical)
}

fn exact_orthogonal_transform(
    source: &CpuImage,
    quarter_turns_clockwise: u8,
    flip_horizontal: bool,
    flip_vertical: bool,
) -> Result<CpuImage, PipelineError> {
    let turns = quarter_turns_clockwise % 4;
    if turns == 0 && !flip_horizontal && !flip_vertical {
        return Ok(source.clone());
    }
    let (width, height) = if turns % 2 == 0 {
        (source.width(), source.height())
    } else {
        (source.height(), source.width())
    };
    let mut pixels = Vec::with_capacity(pixel_count(width, height));
    for output_y in 0..height {
        for output_x in 0..width {
            let turned_x = if flip_horizontal {
                width - 1 - output_x
            } else {
                output_x
            };
            let turned_y = if flip_vertical {
                height - 1 - output_y
            } else {
                output_y
            };
            let (source_x, source_y) = match turns {
                0 => (turned_x, turned_y),
                1 => (turned_y, source.height() - 1 - turned_x),
                2 => (
                    source.width() - 1 - turned_x,
                    source.height() - 1 - turned_y,
                ),
                3 => (source.width() - 1 - turned_y, turned_x),
                _ => unreachable!(),
            };
            pixels.push(source.pixels()[index(source.width(), source_x, source_y)]);
        }
    }
    CpuImage::new(width, height, pixels).map_err(PipelineError::InvalidImage)
}

fn normalized_crop(source: &CpuImage, crop: &CropRect) -> Result<CpuImage, PipelineError> {
    let width = source.width();
    let height = source.height();
    let left = normalized_edge(crop.x, width, false);
    let top = normalized_edge(crop.y, height, false);
    let right = normalized_edge(crop.x + crop.width, width, true).max(left + 1);
    let bottom = normalized_edge(crop.y + crop.height, height, true).max(top + 1);
    let output_width = right.min(width) - left;
    let output_height = bottom.min(height) - top;
    if left == 0 && top == 0 && output_width == width && output_height == height {
        return Ok(source.clone());
    }
    let mut pixels = Vec::with_capacity(pixel_count(output_width, output_height));
    for y in top..top + output_height {
        let start = index(width, left, y);
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
    let pixels = projective_region(source, inverse, 0, 0, source.width(), source.height());
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
) -> Vec<RgbaPixel> {
    let mut pixels = Vec::with_capacity(pixel_count(width, height));
    for y in output_y..output_y + height {
        for x in output_x..output_x + width {
            let destination = [f64::from(x) + 0.5, f64::from(y) + 0.5];
            let mapped = inverse.apply(destination);
            pixels.push(sample_lanczos3(source, mapped[0], mapped[1]));
        }
    }
    pixels
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
    let alpha = premultiplied[3].clamp(0.0, 1.0);
    if alpha <= f64::from(f32::EPSILON) {
        return transparent();
    }
    RgbaPixel {
        red: (premultiplied[0] / premultiplied[3]) as f32,
        green: (premultiplied[1] / premultiplied[3]) as f32,
        blue: (premultiplied[2] / premultiplied[3]) as f32,
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
        let output = exact_orthogonal_transform(&numbered(3, 2), 1, false, false).unwrap();
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
    fn all_exif_orientations_match_the_normative_exact_mapping() {
        let source = numbered(3, 2);
        for (orientation, expected) in [
            (ExifOrientation::Normal, vec![0., 1., 2., 3., 4., 5.]),
            (
                ExifOrientation::MirrorHorizontal,
                vec![2., 1., 0., 5., 4., 3.],
            ),
            (ExifOrientation::Rotate180, vec![5., 4., 3., 2., 1., 0.]),
            (
                ExifOrientation::MirrorVertical,
                vec![3., 4., 5., 0., 1., 2.],
            ),
            (ExifOrientation::Transpose, vec![0., 3., 1., 4., 2., 5.]),
            (
                ExifOrientation::Rotate90Clockwise,
                vec![3., 0., 4., 1., 5., 2.],
            ),
            (ExifOrientation::Transverse, vec![5., 2., 4., 1., 3., 0.]),
            (
                ExifOrientation::Rotate270Clockwise,
                vec![2., 5., 1., 4., 0., 3.],
            ),
        ] {
            let output = apply_exif_orientation(&source, orientation).unwrap();
            let values = output
                .pixels()
                .iter()
                .map(RgbaPixel::red)
                .collect::<Vec<_>>();
            assert_eq!(values, expected, "{orientation:?}");
        }
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
    fn projective_roi_splits_are_bit_exact_to_full_frame() {
        let source = numbered(9, 7);
        let inverse = Homography([
            [0.97, 0.08, -0.25],
            [-0.04, 1.02, 0.30],
            [0.001, -0.002, 1.0],
        ]);
        let full = projective_region(&source, inverse, 0, 0, 9, 7);
        let mut tiled = Vec::new();
        for y in 0..7 {
            tiled.extend(projective_region(&source, inverse, 0, y, 4, 1));
            tiled.extend(projective_region(&source, inverse, 4, y, 5, 1));
        }
        assert_eq!(tiled, full);
    }
}
