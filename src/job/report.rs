use serde::Serialize;

use crate::{
    develop::DevelopWorkingSetProfile,
    io::{SignalRelation, color::SceneRenderReport},
};

use super::{JobErrorCode, JobStage};

pub const DEVELOP_JOB_REPORT_SCHEMA: &str = "io.omacom.grainroom.develop-job-report";
pub const DEVELOP_JOB_REPORT_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportDigest(pub [u8; 32]);

impl Serialize for ReportDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = [0_u8; 64];
        for (index, byte) in self.0.iter().copied().enumerate() {
            encoded[index * 2] = HEX[usize::from(byte >> 4)];
            encoded[index * 2 + 1] = HEX[usize::from(byte & 0x0f)];
        }
        let encoded = std::str::from_utf8(&encoded).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(encoded)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportSignalRelation {
    SceneRelatedRaw,
    LinearizedDisplayReferred,
}

impl From<SignalRelation> for ReportSignalRelation {
    fn from(value: SignalRelation) -> Self {
        match value {
            SignalRelation::SceneRelatedRaw => Self::SceneRelatedRaw,
            SignalRelation::LinearizedDisplayReferred => Self::LinearizedDisplayReferred,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportDevelopWorkingSetProfile {
    PointwiseV1,
}

impl From<DevelopWorkingSetProfile> for ReportDevelopWorkingSetProfile {
    fn from(value: DevelopWorkingSetProfile) -> Self {
        match value {
            DevelopWorkingSetProfile::PointwiseV1 => Self::PointwiseV1,
        }
    }
}

/// Exact requested image payload for the selected bounded develop profile.
/// A RAW scene peak follows develop, so `job_peak_bytes` is the maximum of the
/// two phases rather than their sum.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct DevelopWorkingSetSummary {
    pub profile: ReportDevelopWorkingSetProfile,
    pub source_image_bytes: u64,
    pub transactional_image_bytes: u64,
    pub stage_scratch_bytes: u64,
    pub develop_peak_bytes: u64,
    pub post_develop_scene_peak_bytes: Option<u64>,
    pub job_peak_bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SceneRenderSummary {
    pub tone_mapped_pixels: u64,
    pub gamut_compressed_pixels: u64,
    pub nonpositive_luminance_pixels: u64,
}

impl From<SceneRenderReport> for SceneRenderSummary {
    fn from(value: SceneRenderReport) -> Self {
        Self {
            tone_mapped_pixels: value.tone_mapped_pixels,
            gamut_compressed_pixels: value.gamut_compressed_pixels,
            nonpositive_luminance_pixels: value.nonpositive_luminance_pixels,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum DevelopJobOutcome {
    PublishedAndDurable { bytes_written: u64 },
    PublishedButNotDurable { bytes_written: u64 },
    Failure { stage: JobStage, code: JobErrorCode },
}

/// Stable machine report. It deliberately contains neither paths nor filenames.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DevelopJobReport {
    pub schema: &'static str,
    pub schema_version: u32,
    pub source_digest_v1: Option<ReportDigest>,
    pub input_signal_relation: Option<ReportSignalRelation>,
    pub output_signal_relation: Option<ReportSignalRelation>,
    pub develop_working_set: Option<DevelopWorkingSetSummary>,
    pub scene_render: Option<SceneRenderSummary>,
    pub outcome: DevelopJobOutcome,
}

impl DevelopJobReport {
    pub(crate) fn pending() -> Self {
        Self {
            schema: DEVELOP_JOB_REPORT_SCHEMA,
            schema_version: DEVELOP_JOB_REPORT_VERSION,
            source_digest_v1: None,
            input_signal_relation: None,
            output_signal_relation: None,
            develop_working_set: None,
            scene_render: None,
            outcome: DevelopJobOutcome::Failure {
                stage: JobStage::Validate,
                code: JobErrorCode::Internal,
            },
        }
    }
}
