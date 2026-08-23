use std::fmt;

// Every resource profile charges the concrete pixel payload, not an assumed
// channel packing. A layout change must fail at compile time and trigger an
// estimator/version review.
const _: () = assert!(std::mem::size_of::<RgbaPixel>() == 16);

/// One straight-alpha pixel in Grainroom's normative CPU working space.
///
/// RGB is linear Rec.2020 with a D65 white point. Whether those values are
/// scene-referred (for example, RAW) or linearized display-referred (for
/// example, JPEG/PNG) is carried separately by the I/O `SignalRelation` and
/// must not be inferred from this pixel container. RGB values are finite but
/// intentionally unbounded: negative values and values above 1.0 are valid
/// and must survive intermediate processing. Alpha is straight (not
/// premultiplied), finite, and constrained to the inclusive range 0.0..=1.0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RgbaPixel {
    pub(crate) red: f32,
    pub(crate) green: f32,
    pub(crate) blue: f32,
    pub(crate) alpha: f32,
}

impl RgbaPixel {
    pub fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Result<Self, PixelError> {
        let pixel = Self {
            red,
            green,
            blue,
            alpha,
        };
        pixel.validate()?;
        Ok(pixel)
    }

    pub const fn red(&self) -> f32 {
        self.red
    }

    pub const fn green(&self) -> f32 {
        self.green
    }

    pub const fn blue(&self) -> f32 {
        self.blue
    }

    pub const fn alpha(&self) -> f32 {
        self.alpha
    }

    fn validate(&self) -> Result<(), PixelError> {
        for (channel, value) in [
            (PixelChannel::Red, self.red),
            (PixelChannel::Green, self.green),
            (PixelChannel::Blue, self.blue),
        ] {
            if !value.is_finite() {
                return Err(PixelError::NonFinite(channel));
            }
        }
        if !self.alpha.is_finite() {
            return Err(PixelError::NonFinite(PixelChannel::Alpha));
        }
        if !(0.0..=1.0).contains(&self.alpha) {
            return Err(PixelError::AlphaOutOfRange);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CpuImage {
    width: u32,
    height: u32,
    pixels: Vec<RgbaPixel>,
}

impl CpuImage {
    pub fn new(width: u32, height: u32, pixels: Vec<RgbaPixel>) -> Result<Self, ImageError> {
        if width == 0 || height == 0 {
            return Err(ImageError::EmptyDimensions);
        }
        let expected = checked_pixel_count(width, height)?;
        if pixels.len() != expected {
            return Err(ImageError::PixelCount {
                expected,
                actual: pixels.len(),
            });
        }
        let image = Self {
            width,
            height,
            pixels,
        };
        image.validate()?;
        Ok(image)
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub fn pixels(&self) -> &[RgbaPixel] {
        &self.pixels
    }

    pub fn pixels_mut(&mut self) -> &mut [RgbaPixel] {
        &mut self.pixels
    }

    /// Defensively verifies render buffers at pipeline boundaries. Stages may
    /// use crate-private channel access for performance, so non-finite output
    /// is rejected before it can be committed to a caller-visible image.
    pub fn validate(&self) -> Result<(), ImageError> {
        let expected = checked_pixel_count(self.width, self.height)?;
        if self.pixels.len() != expected {
            return Err(ImageError::PixelCount {
                expected,
                actual: self.pixels.len(),
            });
        }
        for (index, pixel) in self.pixels.iter().enumerate() {
            pixel
                .validate()
                .map_err(|source| ImageError::InvalidPixel { index, source })?;
        }
        Ok(())
    }
}

fn checked_pixel_count(width: u32, height: u32) -> Result<usize, ImageError> {
    let count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(ImageError::DimensionOverflow { width, height })?;
    if count > isize::MAX as u64 {
        return Err(ImageError::DimensionOverflow { width, height });
    }
    usize::try_from(count).map_err(|_| ImageError::DimensionOverflow { width, height })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PixelChannel {
    Red,
    Green,
    Blue,
    Alpha,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PixelError {
    NonFinite(PixelChannel),
    AlphaOutOfRange,
}

impl fmt::Display for PixelError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonFinite(channel) => write!(formatter, "{channel:?} channel must be finite"),
            Self::AlphaOutOfRange => write!(formatter, "straight alpha must be between 0 and 1"),
        }
    }
}

impl std::error::Error for PixelError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageError {
    EmptyDimensions,
    DimensionOverflow { width: u32, height: u32 },
    PixelCount { expected: usize, actual: usize },
    InvalidPixel { index: usize, source: PixelError },
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDimensions => write!(formatter, "image dimensions must be non-zero"),
            Self::DimensionOverflow { width, height } => {
                write!(
                    formatter,
                    "image dimensions {width}x{height} exceed addressable memory"
                )
            }
            Self::PixelCount { expected, actual } => {
                write!(formatter, "expected {expected} pixels, received {actual}")
            }
            Self::InvalidPixel { index, source } => {
                write!(formatter, "invalid pixel at index {index}: {source}")
            }
        }
    }
}

impl std::error::Error for ImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPixel { source, .. } => Some(source),
            _ => None,
        }
    }
}
