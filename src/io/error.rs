use std::{fmt, io};

/// Stable process-facing error categories. Numeric values are append-only.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
#[non_exhaustive]
pub enum ErrorCode {
    InputIo = 10,
    UnsupportedFormat = 11,
    CorruptInput = 12,
    ColorManagement = 13,
    Metadata = 14,
    RawBackend = 15,
    ResourceLimit = 16,
    InvalidOptions = 17,
    Cancelled = 18,
    Encode = 30,
    OutputIo = 31,
    DestinationConflict = 32,
    Internal = 70,
}

pub trait StableErrorCode {
    fn error_code(&self) -> ErrorCode;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MetadataKind {
    Exif,
    Xmp,
    Iptc,
    Icc,
    Total,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LimitError {
    InvalidConfiguration,
    EmptyDimensions,
    ArithmeticOverflow,
    PixelCount {
        requested: u64,
        maximum: u64,
    },
    SourceBytes {
        requested: u64,
        maximum: u64,
    },
    DecodedBytes {
        requested: u64,
        maximum: u64,
    },
    WorkingBytes {
        requested: u64,
        maximum: u64,
    },
    MetadataBytes {
        kind: MetadataKind,
        requested: u64,
        maximum: u64,
    },
}

impl fmt::Display for LimitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration => {
                f.write_str("resource limits are internally inconsistent")
            }
            Self::EmptyDimensions => f.write_str("image dimensions must be non-zero"),
            Self::ArithmeticOverflow => f.write_str("resource estimate overflowed"),
            Self::PixelCount { requested, maximum } => {
                write!(f, "pixel count {requested} exceeds {maximum}")
            }
            Self::SourceBytes { requested, maximum } => {
                write!(f, "source bytes {requested} exceed {maximum}")
            }
            Self::DecodedBytes { requested, maximum } => {
                write!(f, "decoded bytes {requested} exceed {maximum}")
            }
            Self::WorkingBytes { requested, maximum } => {
                write!(f, "working bytes {requested} exceed {maximum}")
            }
            Self::MetadataBytes {
                kind,
                requested,
                maximum,
            } => write!(f, "{kind:?} bytes {requested} exceed {maximum}"),
        }
    }
}

impl std::error::Error for LimitError {}
impl StableErrorCode for LimitError {
    fn error_code(&self) -> ErrorCode {
        ErrorCode::ResourceLimit
    }
}

#[derive(Debug)]
pub enum DigestError {
    Read(io::Error),
    Limit(LimitError),
}
impl fmt::Display for DigestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(_) => f.write_str("source content could not be read"),
            Self::Limit(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for DigestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Read(e) => Some(e),
            Self::Limit(e) => Some(e),
        }
    }
}
impl StableErrorCode for DigestError {
    fn error_code(&self) -> ErrorCode {
        match self {
            Self::Read(_) => ErrorCode::InputIo,
            Self::Limit(_) => ErrorCode::ResourceLimit,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum DecodeError {
    Input(io::Error),
    UnsupportedFormat,
    CorruptInput,
    ColorManagement,
    Metadata,
    RawBackendUnavailable,
    Limit(LimitError),
    InvalidOptions,
    RawBackendTimedOut,
    Cancelled,
    RawBackendOutputLimit,
    RawBackendCaptureIo(io::Error),
    RawBackendFailed { status: Option<i32> },
}
impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "decode failed ({:?})", self.error_code())
    }
}
impl std::error::Error for DecodeError {}
impl StableErrorCode for DecodeError {
    fn error_code(&self) -> ErrorCode {
        match self {
            Self::Input(_) => ErrorCode::InputIo,
            Self::UnsupportedFormat => ErrorCode::UnsupportedFormat,
            Self::CorruptInput => ErrorCode::CorruptInput,
            Self::ColorManagement => ErrorCode::ColorManagement,
            Self::Metadata => ErrorCode::Metadata,
            Self::RawBackendUnavailable => ErrorCode::RawBackend,
            Self::RawBackendTimedOut
            | Self::RawBackendFailed { .. }
            | Self::RawBackendCaptureIo(_) => ErrorCode::RawBackend,
            Self::RawBackendOutputLimit => ErrorCode::ResourceLimit,
            Self::Cancelled => ErrorCode::Cancelled,
            Self::Limit(_) => ErrorCode::ResourceLimit,
            Self::InvalidOptions => ErrorCode::InvalidOptions,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum EncodeError {
    UnsupportedFormat,
    InvalidOptions,
    OutOfRange,
    ColorManagement,
    Encode,
    Output(io::Error),
    Limit(LimitError),
}
impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "encode failed ({:?})", self.error_code())
    }
}
impl std::error::Error for EncodeError {}
impl StableErrorCode for EncodeError {
    fn error_code(&self) -> ErrorCode {
        match self {
            Self::UnsupportedFormat => ErrorCode::UnsupportedFormat,
            Self::InvalidOptions => ErrorCode::InvalidOptions,
            Self::OutOfRange | Self::ColorManagement => ErrorCode::ColorManagement,
            Self::Encode => ErrorCode::Encode,
            Self::Output(_) => ErrorCode::OutputIo,
            Self::Limit(_) => ErrorCode::ResourceLimit,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum AtomicOutputError {
    InvalidDestination,
    InvalidDestinationType,
    DestinationExists,
    InputOutputCollision,
    InvalidPermissions,
    UnsupportedPlatform,
    Create(io::Error),
    Write(io::Error),
    Sync(io::Error),
    Publish(io::Error),
    Cleanup(io::Error),
    /// The destination name is committed, but directory durability is unknown.
    /// Callers must inspect the destination and must not blindly retry.
    PublishedButNotDurable(io::Error),
}

impl fmt::Display for AtomicOutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "atomic output failed ({:?})", self.error_code())
    }
}

impl std::error::Error for AtomicOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Create(error)
            | Self::Write(error)
            | Self::Sync(error)
            | Self::Publish(error)
            | Self::Cleanup(error)
            | Self::PublishedButNotDurable(error) => Some(error),
            _ => None,
        }
    }
}

impl StableErrorCode for AtomicOutputError {
    fn error_code(&self) -> ErrorCode {
        match self {
            Self::DestinationExists | Self::InvalidDestinationType | Self::InputOutputCollision => {
                ErrorCode::DestinationConflict
            }
            Self::InvalidDestination | Self::InvalidPermissions | Self::UnsupportedPlatform => {
                ErrorCode::InvalidOptions
            }
            Self::Create(_)
            | Self::Write(_)
            | Self::Sync(_)
            | Self::Publish(_)
            | Self::Cleanup(_)
            | Self::PublishedButNotDurable(_) => ErrorCode::OutputIo,
        }
    }
}
