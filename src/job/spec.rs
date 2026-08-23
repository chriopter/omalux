use std::path::PathBuf;

use crate::{
    develop::{ParameterOverride, PresetDocument},
    io::{
        AlphaPolicy, DecodeOptions, EncodeOptions, MetadataPolicy, OutputFormat, OutputProfile,
        SdrRangePolicy,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub enum PresetSelection {
    CatalogId(String),
    Document(Box<PresetDocument>),
}

impl PresetSelection {
    pub fn document(document: PresetDocument) -> Self {
        Self::Document(Box::new(document))
    }
}

/// Publicly constructible output request for the non-exhaustive I/O options.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DevelopOutput {
    format: OutputFormat,
    quality: u8,
    profile: OutputProfile,
    metadata: MetadataPolicy,
    alpha: AlphaPolicy,
    range: SdrRangePolicy,
}

impl DevelopOutput {
    pub const fn new(
        format: OutputFormat,
        quality: u8,
        profile: OutputProfile,
        metadata: MetadataPolicy,
        alpha: AlphaPolicy,
        range: SdrRangePolicy,
    ) -> Self {
        Self {
            format,
            quality,
            profile,
            metadata,
            alpha,
            range,
        }
    }

    pub fn validate(&self) -> Result<(), crate::io::EncodeError> {
        self.as_encode_options().validate()
    }

    pub(crate) const fn as_encode_options(&self) -> EncodeOptions {
        EncodeOptions {
            format: self.format,
            quality: self.quality,
            profile: self.profile,
            metadata: self.metadata,
            alpha: self.alpha,
            range: self.range,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DevelopJob {
    pub input: PathBuf,
    pub output: PathBuf,
    pub decode: DecodeOptions,
    pub output_options: DevelopOutput,
    pub preset: PresetSelection,
    pub overrides: Vec<ParameterOverride>,
}
