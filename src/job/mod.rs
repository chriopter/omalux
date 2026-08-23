//! Qt-free contracts for one deterministic photo-development job.

mod artifact;
mod error;
mod progress;
mod report;
mod runner;
mod services;
mod spec;

pub use artifact::{DecodedArtifact, DisplayReferred, SceneRelated, WorkingArtifact};
pub use error::{DevelopJobError, DevelopJobFailure, JobErrorCode};
pub use progress::{CancellationToken, JobStage, NoProgress, ProgressSink};
pub use report::{
    DEVELOP_JOB_REPORT_SCHEMA, DEVELOP_JOB_REPORT_VERSION, DevelopJobOutcome, DevelopJobReport,
    ReportSignalRelation, SceneRenderSummary,
};
pub use runner::DevelopJobRunner;
pub use services::{EncodeReceipt, PhotoDecoder, PhotoEncoder};
pub use spec::{DevelopJob, DevelopOutput, PresetSelection};
