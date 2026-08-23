//! Qt-free contracts for one deterministic photo-development job.

mod artifact;
mod error;
mod production;
mod progress;
mod report;
mod runner;
mod services;
mod spec;

pub use artifact::{
    ArtifactRelation, DecodedArtifact, DisplayReferred, SceneRelated, WorkingArtifact,
};
pub use error::{DevelopJobError, DevelopJobFailure, JobErrorCode};
pub use production::{ProductionJpegEncoder, ProductionPhotoDecoder};
pub use progress::{CancellationToken, JobStage, NoProgress, ProgressSink};
pub use report::{
    DEVELOP_JOB_REPORT_SCHEMA, DEVELOP_JOB_REPORT_VERSION, DevelopJobOutcome, DevelopJobReport,
    ReportDigest, ReportSignalRelation, SceneRenderSummary,
};
pub use runner::DevelopJobRunner;
pub use services::{
    DecodedSource, EncodeReceipt, PhotoDecoder, PhotoEncoder, PublicationRequest, PublicationStatus,
};
pub use spec::{DevelopJob, DevelopOutput, PresetSelection};
