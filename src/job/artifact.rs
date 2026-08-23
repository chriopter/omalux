use std::marker::PhantomData;

use crate::{
    develop::{CpuImage, ImageError},
    io::{
        ColorProvenance, DecodedPhoto, DecodedPhotoError, Diagnostic, MetadataBundle,
        ResourceLimits, SignalRelation, SourceDigestV1,
        color::{SceneRenderError, SceneRenderReport, SceneToDisplayTransform},
    },
};

mod private {
    pub trait Sealed {}
}

/// Compile-time relation carried by a working artifact.
pub trait ArtifactRelation: private::Sealed {
    const SIGNAL_RELATION: SignalRelation;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SceneRelated;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DisplayReferred;

impl private::Sealed for SceneRelated {}
impl private::Sealed for DisplayReferred {}
impl ArtifactRelation for SceneRelated {
    const SIGNAL_RELATION: SignalRelation = SignalRelation::SceneRelatedRaw;
}
impl ArtifactRelation for DisplayReferred {
    const SIGNAL_RELATION: SignalRelation = SignalRelation::LinearizedDisplayReferred;
}

/// A validated linear-Rec.2020 image whose signal relation is encoded in its type.
///
/// Metadata and decode provenance stay attached without constructing a false
/// display-referred `DecodedPhoto` with RAW provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkingArtifact<R: ArtifactRelation> {
    image: CpuImage,
    metadata: MetadataBundle,
    source_digest: SourceDigestV1,
    color: ColorProvenance,
    diagnostics: Vec<Diagnostic>,
    relation: PhantomData<R>,
}

impl<R: ArtifactRelation> WorkingArtifact<R> {
    fn checked(
        image: CpuImage,
        metadata: MetadataBundle,
        source_digest: SourceDigestV1,
        color: ColorProvenance,
        diagnostics: Vec<Diagnostic>,
    ) -> Result<Self, ImageError> {
        image.validate()?;
        Ok(Self {
            image,
            metadata,
            source_digest,
            color,
            diagnostics,
            relation: PhantomData,
        })
    }

    pub fn image(&self) -> &CpuImage {
        &self.image
    }

    pub(crate) fn image_mut(&mut self) -> &mut CpuImage {
        &mut self.image
    }

    pub fn metadata(&self) -> &MetadataBundle {
        &self.metadata
    }

    pub const fn source_digest(&self) -> SourceDigestV1 {
        self.source_digest
    }

    pub fn color_provenance(&self) -> &ColorProvenance {
        &self.color
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub const fn signal_relation(&self) -> SignalRelation {
        R::SIGNAL_RELATION
    }
}

/// Runtime decode result converted once into a relation-typed artifact.
#[derive(Clone, Debug, PartialEq)]
pub enum DecodedArtifact {
    Scene(WorkingArtifact<SceneRelated>),
    Display(WorkingArtifact<DisplayReferred>),
}

impl DecodedArtifact {
    pub fn try_from_photo(
        photo: DecodedPhoto,
        limits: &ResourceLimits,
    ) -> Result<Self, DecodedPhotoError> {
        photo.validate(limits)?;
        // The canonical pipeline is transactional and therefore needs the
        // decoded image plus one full working copy. Enforce that actual peak
        // before this boundary performs its temporary fallible copy.
        let pixels = u64::from(photo.image().width())
            .checked_mul(u64::from(photo.image().height()))
            .ok_or(DecodedPhotoError::Limit(
                crate::io::LimitError::ArithmeticOverflow,
            ))?;
        let peak = pixels.checked_mul(32).ok_or(DecodedPhotoError::Limit(
            crate::io::LimitError::ArithmeticOverflow,
        ))?;
        if peak > limits.max_working_bytes {
            return Err(DecodedPhotoError::Limit(
                crate::io::LimitError::WorkingBytes {
                    requested: peak,
                    maximum: limits.max_working_bytes,
                },
            ));
        }
        let mut pixels_copy = Vec::new();
        pixels_copy
            .try_reserve_exact(photo.image().pixels().len())
            .map_err(|_| DecodedPhotoError::Limit(crate::io::LimitError::Allocation))?;
        pixels_copy.extend_from_slice(photo.image().pixels());
        let image = CpuImage::new(photo.image().width(), photo.image().height(), pixels_copy)
            .map_err(DecodedPhotoError::Image)?;
        let build = || {
            (
                image,
                photo.metadata().clone(),
                photo.source_digest(),
                photo.color().clone(),
                photo.diagnostics().to_vec(),
            )
        };
        match photo.signal_relation() {
            SignalRelation::SceneRelatedRaw => {
                let (image, metadata, digest, color, diagnostics) = build();
                Ok(Self::Scene(
                    WorkingArtifact::checked(image, metadata, digest, color, diagnostics)
                        .map_err(DecodedPhotoError::Image)?,
                ))
            }
            SignalRelation::LinearizedDisplayReferred => {
                let (image, metadata, digest, color, diagnostics) = build();
                Ok(Self::Display(
                    WorkingArtifact::checked(image, metadata, digest, color, diagnostics)
                        .map_err(DecodedPhotoError::Image)?,
                ))
            }
        }
    }
}

impl WorkingArtifact<SceneRelated> {
    /// Consumes scene-related pixels and returns a separately typed display artifact.
    pub fn render_to_display(
        mut self,
        transform: &SceneToDisplayTransform,
        limits: &ResourceLimits,
    ) -> Result<(WorkingArtifact<DisplayReferred>, SceneRenderReport), SceneRenderError> {
        let width =
            usize::try_from(self.image.width()).map_err(|_| SceneRenderError::Allocation)?;
        let mut total: Option<SceneRenderReport> = None;
        let mut source = Vec::new();
        source
            .try_reserve_exact(width)
            .map_err(|_| SceneRenderError::Allocation)?;
        for row in self.image.pixels_mut().chunks_exact_mut(width) {
            source.clear();
            source.extend_from_slice(row);
            let row = transform.transform_scanline(
                &source,
                row,
                SignalRelation::SceneRelatedRaw,
                limits,
            )?;
            if let Some(report) = &mut total {
                report.tone_mapped_pixels = report
                    .tone_mapped_pixels
                    .checked_add(row.tone_mapped_pixels)
                    .ok_or(SceneRenderError::Allocation)?;
                report.gamut_compressed_pixels = report
                    .gamut_compressed_pixels
                    .checked_add(row.gamut_compressed_pixels)
                    .ok_or(SceneRenderError::Allocation)?;
                report.nonpositive_luminance_pixels = report
                    .nonpositive_luminance_pixels
                    .checked_add(row.nonpositive_luminance_pixels)
                    .ok_or(SceneRenderError::Allocation)?;
            } else {
                total = Some(row);
            }
        }
        let report = total.ok_or(SceneRenderError::Allocation)?;
        let artifact = WorkingArtifact::checked(
            self.image,
            self.metadata,
            self.source_digest,
            self.color,
            self.diagnostics,
        )
        .map_err(|_| SceneRenderError::Allocation)?;
        Ok((artifact, report))
    }
}
