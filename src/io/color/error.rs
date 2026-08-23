use std::fmt;

use crate::io::LimitError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RasterChannel {
    Red,
    Green,
    Blue,
    Alpha,
}

#[derive(Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ColorError {
    Limit(LimitError),
    EmptyProfile,
    MalformedProfile,
    UnsupportedColorSpace,
    UnsupportedProfile,
    ProfileGeneration,
    TransformCreation,
    LengthMismatch {
        source: usize,
        destination: usize,
    },
    InvalidRasterSample {
        pixel: usize,
        channel: RasterChannel,
    },
    NonFiniteOutput {
        pixel: usize,
        channel: RasterChannel,
    },
    Allocation,
    IncompletePngDeclaration,
    ConflictingPngDeclaration,
    InvalidPngGamma,
    InvalidPngChromaticity,
}

impl fmt::Display for ColorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limit(error) => error.fmt(formatter),
            Self::EmptyProfile => formatter.write_str("ICC profile is empty"),
            Self::MalformedProfile => formatter.write_str("ICC profile is malformed or truncated"),
            Self::UnsupportedColorSpace => formatter.write_str("ICC profile is not an RGB profile"),
            Self::UnsupportedProfile => {
                formatter.write_str("ICC profile cannot be used as an RGB input")
            }
            Self::ProfileGeneration => formatter.write_str("RGB profile generation failed"),
            Self::TransformCreation => formatter.write_str("RGB color transform creation failed"),
            Self::LengthMismatch {
                source,
                destination,
            } => write!(
                formatter,
                "source pixel count {source} does not match destination pixel count {destination}"
            ),
            Self::InvalidRasterSample { pixel, channel } => write!(
                formatter,
                "encoded raster pixel {pixel} has an invalid {channel:?} sample"
            ),
            Self::NonFiniteOutput { pixel, channel } => write!(
                formatter,
                "color transform produced a non-finite {channel:?} sample at pixel {pixel}"
            ),
            Self::Allocation => formatter.write_str("color transform scratch allocation failed"),
            Self::IncompletePngDeclaration => {
                formatter.write_str("PNG gAMA and cHRM declarations must be supplied together")
            }
            Self::ConflictingPngDeclaration => {
                formatter.write_str("PNG sRGB declaration conflicts with gAMA or cHRM")
            }
            Self::InvalidPngGamma => formatter.write_str("PNG gAMA value is invalid"),
            Self::InvalidPngChromaticity => {
                formatter.write_str("PNG cHRM chromaticities are invalid")
            }
        }
    }
}

impl std::error::Error for ColorError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Limit(error) => Some(error),
            _ => None,
        }
    }
}

impl From<LimitError> for ColorError {
    fn from(error: LimitError) -> Self {
        Self::Limit(error)
    }
}
