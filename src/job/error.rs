use std::fmt;

use serde::Serialize;

use super::{DevelopJobOutcome, DevelopJobReport, JobStage};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobErrorCode {
    InputIo,
    UnsupportedFormat,
    CorruptInput,
    ColorManagement,
    Metadata,
    RawBackend,
    ResourceLimit,
    InvalidOptions,
    Cancelled,
    Encode,
    OutputIo,
    DestinationConflict,
    EncoderBackendUnavailable,
    UnprovenPipelineBudget,
    Internal,
}

impl From<crate::io::ErrorCode> for JobErrorCode {
    fn from(value: crate::io::ErrorCode) -> Self {
        use crate::io::ErrorCode;
        match value {
            ErrorCode::InputIo => Self::InputIo,
            ErrorCode::UnsupportedFormat => Self::UnsupportedFormat,
            ErrorCode::CorruptInput => Self::CorruptInput,
            ErrorCode::ColorManagement => Self::ColorManagement,
            ErrorCode::Metadata => Self::Metadata,
            ErrorCode::RawBackend => Self::RawBackend,
            ErrorCode::ResourceLimit => Self::ResourceLimit,
            ErrorCode::InvalidOptions => Self::InvalidOptions,
            ErrorCode::Cancelled => Self::Cancelled,
            ErrorCode::Encode => Self::Encode,
            ErrorCode::OutputIo => Self::OutputIo,
            ErrorCode::DestinationConflict => Self::DestinationConflict,
            ErrorCode::EncoderBackendUnavailable => Self::EncoderBackendUnavailable,
            ErrorCode::Internal => Self::Internal,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DevelopJobError {
    pub stage: JobStage,
    pub code: JobErrorCode,
}

impl fmt::Display for DevelopJobError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "develop job failed at {:?} ({:?})",
            self.stage, self.code
        )
    }
}

impl std::error::Error for DevelopJobError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DevelopJobFailure {
    pub error: DevelopJobError,
    pub report: Box<DevelopJobReport>,
    /// What actually went wrong, in the words of the stage that failed. The
    /// code above is the stable contract; this is for the person reading the
    /// terminal, because "internal" on its own has cost real diagnosis time.
    pub detail: Option<String>,
}

impl DevelopJobFailure {
    pub(crate) fn new(stage: JobStage, code: JobErrorCode, mut report: DevelopJobReport) -> Self {
        report.outcome = DevelopJobOutcome::Failure { stage, code };
        Self {
            error: DevelopJobError { stage, code },
            report: Box::new(report),
            detail: None,
        }
    }

    pub(crate) fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl fmt::Display for DevelopJobFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl std::error::Error for DevelopJobFailure {}
