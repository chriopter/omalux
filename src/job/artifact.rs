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
        let (image, metadata, digest, color, relation, diagnostics) = photo.into_parts();
        match relation {
            SignalRelation::SceneRelatedRaw => Ok(Self::Scene(
                WorkingArtifact::checked(image, metadata, digest, color, diagnostics)
                    .map_err(DecodedPhotoError::Image)?,
            )),
            SignalRelation::LinearizedDisplayReferred => Ok(Self::Display(
                WorkingArtifact::checked(image, metadata, digest, color, diagnostics)
                    .map_err(DecodedPhotoError::Image)?,
            )),
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
        let image_bytes = u64::from(self.image.width())
            .checked_mul(u64::from(self.image.height()))
            .and_then(|pixels| pixels.checked_mul(16))
            .ok_or(crate::io::LimitError::ArithmeticOverflow)?;
        // One copied source row plus SceneToDisplay's transactional row.
        let row_scratch = u64::from(self.image.width())
            .checked_mul(32)
            .ok_or(crate::io::LimitError::ArithmeticOverflow)?;
        let peak = image_bytes
            .checked_add(row_scratch)
            .ok_or(crate::io::LimitError::ArithmeticOverflow)?;
        if peak > limits.max_working_bytes {
            return Err(crate::io::LimitError::WorkingBytes {
                requested: peak,
                maximum: limits.max_working_bytes,
            }
            .into());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{
        Diagnostic, RawBackendName, RawMatrixSource, RawProcessingProvenance,
        WhiteBalanceProvenance,
    };

    fn raw_photo(limits: &ResourceLimits) -> DecodedPhoto {
        let image = CpuImage::new(
            1,
            1,
            vec![crate::develop::RgbaPixel::new(0.18, 0.18, 0.18, 0.5).unwrap()],
        )
        .unwrap();
        DecodedPhoto::new(
            image,
            MetadataBundle::default(),
            SourceDigestV1::from_bytes(b"artifact resource test"),
            ColorProvenance::RawMatrix {
                matrix: RawMatrixSource::CameraDatabase,
                white_balance: WhiteBalanceProvenance::Camera,
                processing: RawProcessingProvenance {
                    backend: RawBackendName::LibRawDcrawEmu,
                    backend_version: Some("test".to_owned()),
                    full_resolution: true,
                    linear_16_bit: true,
                    output_rec2020: true,
                    embedded_matrix_enabled: true,
                    ahd_demosaic: true,
                },
            },
            SignalRelation::SceneRelatedRaw,
            Vec::<Diagnostic>::new(),
            limits,
        )
        .unwrap()
    }

    #[test]
    fn decoded_conversion_moves_the_pixel_allocation_without_cloning() {
        let limits = ResourceLimits::default();
        let photo = raw_photo(&limits);
        let allocation = photo.image().pixels().as_ptr();
        let DecodedArtifact::Scene(artifact) =
            DecodedArtifact::try_from_photo(photo, &limits).unwrap()
        else {
            panic!("expected scene artifact");
        };
        assert_eq!(artifact.image().pixels().as_ptr(), allocation);
    }

    #[test]
    fn scene_render_peak_is_exactly_image_plus_two_scanlines() {
        let exact = ResourceLimits {
            max_working_bytes: 48,
            ..ResourceLimits::default()
        };
        let DecodedArtifact::Scene(artifact) =
            DecodedArtifact::try_from_photo(raw_photo(&exact), &exact).unwrap()
        else {
            panic!("expected scene artifact");
        };
        assert!(
            artifact
                .render_to_display(&SceneToDisplayTransform::new(), &exact)
                .is_ok()
        );

        let below = ResourceLimits {
            max_working_bytes: 47,
            ..ResourceLimits::default()
        };
        // Decode validation needs 16 bytes; only scene rendering exceeds 47.
        let DecodedArtifact::Scene(artifact) =
            DecodedArtifact::try_from_photo(raw_photo(&below), &below).unwrap()
        else {
            panic!("expected scene artifact");
        };
        assert!(matches!(
            artifact.render_to_display(&SceneToDisplayTransform::new(), &below),
            Err(SceneRenderError::Limit(
                crate::io::LimitError::WorkingBytes {
                    requested: 48,
                    maximum: 47
                }
            ))
        ));
    }
}
