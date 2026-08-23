use serde::{Serialize, ser::SerializeStruct};

use crate::{
    develop::DevelopWorkingSetProfile,
    io::{SignalRelation, color::SceneRenderReport},
};

use super::{JobErrorCode, JobStage};

pub const DEVELOP_JOB_REPORT_SCHEMA: &str = "io.omacom.grainroom.develop-job-report";
pub const DEVELOP_JOB_REPORT_VERSION: u32 = 3;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportOutputFormat {
    Jpeg,
    Heic,
}

impl From<crate::io::OutputFormat> for ReportOutputFormat {
    fn from(value: crate::io::OutputFormat) -> Self {
        match value {
            crate::io::OutputFormat::Jpeg => Self::Jpeg,
            crate::io::OutputFormat::Heic => Self::Heic,
        }
    }
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

/// Exact requested image-payload peak for the selected bounded profile.
/// For RAW this is the maximum of develop and later scene-render phases.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DevelopWorkingSetSummary {
    pub estimated_peak_bytes: u64,
}

impl DevelopWorkingSetSummary {
    const fn pending() -> Self {
        Self {
            estimated_peak_bytes: 0,
        }
    }

    pub(crate) const fn from_profile(
        profile: DevelopWorkingSetProfile,
        estimated_peak_bytes: u64,
    ) -> Self {
        debug_assert!(estimated_peak_bytes > 0);
        match profile {
            DevelopWorkingSetProfile::PointwiseV1 => Self {
                estimated_peak_bytes,
            },
        }
    }

    pub const fn profile(self) -> Option<ReportDevelopWorkingSetProfile> {
        if self.estimated_peak_bytes == 0 {
            None
        } else {
            Some(ReportDevelopWorkingSetProfile::PointwiseV1)
        }
    }
}

impl Serialize for DevelopWorkingSetSummary {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut value = serializer.serialize_struct("DevelopWorkingSetSummary", 2)?;
        value.serialize_field("profile", &self.profile())?;
        value.serialize_field("estimated_peak_bytes", &self.estimated_peak_bytes)?;
        value.end()
    }
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "format", rename_all = "snake_case")]
pub enum EncodeSummary {
    Jpeg {
        quality: u8,
        icc_sha256: ReportDigest,
        clipped_samples: u64,
        alpha_flattened_pixels: u64,
    },
    Heic {
        quality: u8,
        bit_depth: u8,
        libheif_version: String,
        encoder: String,
        icc_sha256: ReportDigest,
        nclx: HeicNclxSummary,
        clipped_samples: u64,
        alpha_flattened_pixels: u64,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct HeicNclxSummary {
    pub color_primaries: u16,
    pub transfer_characteristics: u16,
    pub matrix_coefficients: u16,
    pub full_range: bool,
}

/// Stable machine report. It deliberately contains neither paths nor filenames.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DevelopJobReport {
    pub schema: &'static str,
    pub schema_version: u32,
    pub source_digest_v1: Option<ReportDigest>,
    pub input_signal_relation: Option<ReportSignalRelation>,
    pub output_signal_relation: Option<ReportSignalRelation>,
    pub output_format: Option<ReportOutputFormat>,
    pub develop_working_set: DevelopWorkingSetSummary,
    pub scene_render: Option<SceneRenderSummary>,
    pub encoding: Option<Box<EncodeSummary>>,
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
            output_format: None,
            develop_working_set: DevelopWorkingSetSummary::pending(),
            scene_render: None,
            encoding: None,
            outcome: DevelopJobOutcome::Failure {
                stage: JobStage::Validate,
                code: JobErrorCode::Internal,
            },
        }
    }
}
