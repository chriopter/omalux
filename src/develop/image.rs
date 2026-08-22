use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RgbaPixel {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl RgbaPixel {
    pub const fn new(red: f32, green: f32, blue: f32, alpha: f32) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
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
        let expected = width as usize * height as usize;
        if width == 0 || height == 0 {
            return Err(ImageError::EmptyDimensions);
        }
        if pixels.len() != expected {
            return Err(ImageError::PixelCount {
                expected,
                actual: pixels.len(),
            });
        }
        Ok(Self {
            width,
            height,
            pixels,
        })
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ImageError {
    EmptyDimensions,
    PixelCount { expected: usize, actual: usize },
}

impl fmt::Display for ImageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyDimensions => write!(formatter, "image dimensions must be non-zero"),
            Self::PixelCount { expected, actual } => {
                write!(formatter, "expected {expected} pixels, received {actual}")
            }
        }
    }
}

impl std::error::Error for ImageError {}
