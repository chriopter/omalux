//! Import-time EXIF orientation normalization.
//!
//! This API is crate-internal because orientation belongs to decoder runtime
//! state, not to persisted develop settings.

#[cfg(test)]
use super::RgbaPixel;
use super::{CpuImage, PipelineError};

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
    apply_orthogonal_transform(image, turns, flip_horizontal, flip_vertical)
}

pub(crate) fn apply_orthogonal_transform(
    source: &CpuImage,
    quarter_turns_clockwise: u8,
    flip_horizontal: bool,
    flip_vertical: bool,
) -> Result<CpuImage, PipelineError> {
    let turns = quarter_turns_clockwise % 4;
    if turns == 0 && !flip_horizontal && !flip_vertical {
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(source.pixels().len())
            .map_err(|_| PipelineError::ResourceLimit(crate::io::LimitError::Allocation))?;
        pixels.extend_from_slice(source.pixels());
        return CpuImage::new(source.width(), source.height(), pixels)
            .map_err(PipelineError::InvalidImage);
    }
    let (width, height) = if matches!(turns, 0 | 2) {
        (source.width(), source.height())
    } else {
        (source.height(), source.width())
    };
    let count = pixel_count(width, height);
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(count)
        .map_err(|_| PipelineError::ResourceLimit(crate::io::LimitError::Allocation))?;
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

fn pixel_count(width: u32, height: u32) -> usize {
    usize::try_from(u64::from(width) * u64::from(height)).expect("validated image dimensions")
}

fn index(width: u32, x: u32, y: u32) -> usize {
    (u64::from(y) * u64::from(width) + u64::from(x)) as usize
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered() -> CpuImage {
        CpuImage::new(
            3,
            2,
            (0..6)
                .map(|value| RgbaPixel::new(value as f32, 0.0, 0.0, 1.0).unwrap())
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn all_eight_exif_values_match_exact_pixel_permutations() {
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
            let output = apply_exif_orientation(&numbered(), orientation).unwrap();
            assert_eq!(
                output
                    .pixels()
                    .iter()
                    .map(RgbaPixel::red)
                    .collect::<Vec<_>>(),
                expected
            );
        }
    }
}
