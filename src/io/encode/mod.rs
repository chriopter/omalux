//! Bounded display-referred image encoding.

mod metadata;
mod prepare;

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub use metadata::MetadataWriteReport;
pub use prepare::{JpegEncodeInput, PreparedDisplayRgb, prepare_display_rgb8};

use crate::io::{AtomicOutputOutcome, IccProfileProvenance};

#[derive(Clone, Default, Debug)]
pub struct EncodeCancellation(Arc<AtomicBool>);

impl EncodeCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub(crate) fn cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct JpegEncodeReport {
    pub outcome: AtomicOutputOutcome,
    pub width: u32,
    pub height: u32,
    pub quality: u8,
    pub output_bytes: u64,
    pub icc: IccProfileProvenance,
    pub clipped_samples: u64,
    pub alpha_flattened_pixels: u64,
    pub metadata: MetadataWriteReport,
}
