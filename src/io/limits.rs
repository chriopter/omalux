use super::{LimitError, MetadataKind};

/// Conservative process-wide limits applied before allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct ResourceLimits {
    pub max_source_bytes: u64,
    pub max_pixels: u64,
    pub max_decoded_bytes: u64,
    pub max_working_bytes: u64,
    /// Maximum number of encoded bytes accepted from an output codec.
    pub max_output_bytes: u64,
    pub max_metadata_component_bytes: u64,
    pub max_total_metadata_bytes: u64,
    pub max_icc_bytes: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 1 << 30,
            max_pixels: 100_000_000,
            max_decoded_bytes: 1 << 30,
            // A conservative desktop default; callers may explicitly raise it.
            max_working_bytes: 1 << 30,
            max_output_bytes: 1 << 30,
            max_metadata_component_bytes: 64 << 20,
            max_total_metadata_bytes: 128 << 20,
            // ICC parsing occurs in native LCMS. Keep the default materially
            // below general metadata limits to reduce hostile-profile surface.
            max_icc_bytes: 4 << 20,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct WorkingSetEstimate {
    pub pixels: u64,
    pub decoded_bytes: u64,
    pub cpu_image_bytes: u64,
    pub transactional_copy_bytes: u64,
    pub scratch_bytes: u64,
    pub peak_bytes: u64,
}

/// Audited allocation shapes used while preparing an encoded image.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EncodeWorkingSetProfile {
    /// Resident RGBA-f32 image, RGB8 output and bounded scanline scratch.
    JpegRgb8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct EncodeWorkingSetEstimate {
    pub pixels: u64,
    pub resident_image_bytes: u64,
    pub encoded_rgb_bytes: u64,
    pub scanline_scratch_bytes: u64,
    pub profile_metadata_bytes: u64,
    pub peak_bytes: u64,
}

/// Audited allocation shapes for supported decoder families. Backends must
/// select a named profile instead of supplying optimistic byte counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DecodeWorkingSetProfile {
    RasterRgba8,
    RasterRgba16,
    RawMosaic16FullResolution,
    /// dcraw_emu P6 RGB16 output plus demosaic/profile-conversion scratch.
    RawPpm16FullResolution,
}

impl DecodeWorkingSetProfile {
    const fn bytes(self) -> (u16, u16) {
        match self {
            Self::RasterRgba8 => (4, 8),
            Self::RasterRgba16 => (8, 8),
            // Mosaic plus conservative demosaic/profile-conversion scratch.
            Self::RawMosaic16FullResolution => (2, 32),
            Self::RawPpm16FullResolution => (6, 32),
        }
    }
}

impl ResourceLimits {
    pub fn with_max_output_bytes(mut self, maximum: u64) -> Self {
        self.max_output_bytes = maximum;
        self
    }

    /// Estimates decode storage plus two 16-byte RGBA f32 buffers and caller
    /// supplied scratch. Every operation is checked before an allocation.
    pub fn validate(&self) -> Result<(), LimitError> {
        if self.max_source_bytes == 0
            || self.max_pixels == 0
            || self.max_decoded_bytes == 0
            || self.max_working_bytes == 0
            || self.max_output_bytes == 0
            || self.max_metadata_component_bytes == 0
            || self.max_total_metadata_bytes == 0
            || self.max_icc_bytes == 0
            || self.max_metadata_component_bytes > self.max_total_metadata_bytes
            || self.max_icc_bytes > self.max_total_metadata_bytes
        {
            return Err(LimitError::InvalidConfiguration);
        }
        Ok(())
    }

    pub fn check_source_bytes(&self, requested: u64) -> Result<(), LimitError> {
        if requested > self.max_source_bytes {
            return Err(LimitError::SourceBytes {
                requested,
                maximum: self.max_source_bytes,
            });
        }
        Ok(())
    }

    pub fn estimate_working_set(
        &self,
        width: u32,
        height: u32,
        profile: DecodeWorkingSetProfile,
    ) -> Result<WorkingSetEstimate, LimitError> {
        self.validate()?;
        if width == 0 || height == 0 {
            return Err(LimitError::EmptyDimensions);
        }
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or(LimitError::ArithmeticOverflow)?;
        if pixels > self.max_pixels {
            return Err(LimitError::PixelCount {
                requested: pixels,
                maximum: self.max_pixels,
            });
        }
        let mul = |bytes: u64| {
            pixels
                .checked_mul(bytes)
                .ok_or(LimitError::ArithmeticOverflow)
        };
        let (decoded_bytes_per_pixel, scratch_bytes_per_pixel) = profile.bytes();
        let decoded_bytes = mul(u64::from(decoded_bytes_per_pixel))?;
        if decoded_bytes > self.max_decoded_bytes {
            return Err(LimitError::DecodedBytes {
                requested: decoded_bytes,
                maximum: self.max_decoded_bytes,
            });
        }
        let cpu_image_bytes = mul(16)?;
        let transactional_copy_bytes = cpu_image_bytes;
        let scratch_bytes = mul(u64::from(scratch_bytes_per_pixel))?;
        let peak_bytes = decoded_bytes
            .checked_add(cpu_image_bytes)
            .and_then(|v| v.checked_add(transactional_copy_bytes))
            .and_then(|v| v.checked_add(scratch_bytes))
            .ok_or(LimitError::ArithmeticOverflow)?;
        if peak_bytes > self.max_working_bytes {
            return Err(LimitError::WorkingBytes {
                requested: peak_bytes,
                maximum: self.max_working_bytes,
            });
        }
        Ok(WorkingSetEstimate {
            pixels,
            decoded_bytes,
            cpu_image_bytes,
            transactional_copy_bytes,
            scratch_bytes,
            peak_bytes,
        })
    }

    /// Estimates the complete resident input plus encoder preparation storage.
    pub fn estimate_encode_working_set(
        &self,
        width: u32,
        height: u32,
        profile: EncodeWorkingSetProfile,
        profile_metadata_bytes: u64,
    ) -> Result<EncodeWorkingSetEstimate, LimitError> {
        self.validate()?;
        if width == 0 || height == 0 {
            return Err(LimitError::EmptyDimensions);
        }
        let pixels = u64::from(width)
            .checked_mul(u64::from(height))
            .ok_or(LimitError::ArithmeticOverflow)?;
        if pixels > self.max_pixels {
            return Err(LimitError::PixelCount {
                requested: pixels,
                maximum: self.max_pixels,
            });
        }
        let resident_image_bytes = pixels
            .checked_mul(16)
            .ok_or(LimitError::ArithmeticOverflow)?;
        let encoded_rgb_bytes = match profile {
            EncodeWorkingSetProfile::JpegRgb8 => pixels
                .checked_mul(3)
                .ok_or(LimitError::ArithmeticOverflow)?,
        };
        // Caller source/destination rows plus the two RGBA-f32 rows allocated
        // transactionally by WorkingToSrgbTransform.
        let scanline_scratch_bytes = u64::from(width)
            .checked_mul(64)
            .ok_or(LimitError::ArithmeticOverflow)?;
        // Prepared metadata and the image encoder each own a copy while the
        // codec is active.
        let duplicated_profile_metadata = profile_metadata_bytes
            .checked_mul(2)
            .ok_or(LimitError::ArithmeticOverflow)?;
        let peak_bytes = resident_image_bytes
            .checked_add(encoded_rgb_bytes)
            .and_then(|value| value.checked_add(scanline_scratch_bytes))
            .and_then(|value| value.checked_add(duplicated_profile_metadata))
            .ok_or(LimitError::ArithmeticOverflow)?;
        if peak_bytes > self.max_working_bytes {
            return Err(LimitError::WorkingBytes {
                requested: peak_bytes,
                maximum: self.max_working_bytes,
            });
        }
        Ok(EncodeWorkingSetEstimate {
            pixels,
            resident_image_bytes,
            encoded_rgb_bytes,
            scanline_scratch_bytes,
            profile_metadata_bytes,
            peak_bytes,
        })
    }

    pub fn check_output_bytes(&self, requested: u64) -> Result<(), LimitError> {
        if requested > self.max_output_bytes {
            return Err(LimitError::OutputBytes {
                requested,
                maximum: self.max_output_bytes,
            });
        }
        Ok(())
    }

    pub fn check_metadata_component(
        &self,
        kind: MetadataKind,
        bytes: usize,
    ) -> Result<(), LimitError> {
        let requested = u64::try_from(bytes).map_err(|_| LimitError::ArithmeticOverflow)?;
        let maximum = if kind == MetadataKind::Icc {
            self.max_icc_bytes
        } else {
            self.max_metadata_component_bytes
        };
        if requested > maximum {
            return Err(LimitError::MetadataBytes {
                kind,
                requested,
                maximum,
            });
        }
        Ok(())
    }

    pub fn check_metadata_total(&self, bytes: u64) -> Result<(), LimitError> {
        if bytes > self.max_total_metadata_bytes {
            return Err(LimitError::MetadataBytes {
                kind: MetadataKind::Total,
                requested: bytes,
                maximum: self.max_total_metadata_bytes,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_estimate_is_conservative() {
        assert_eq!(ResourceLimits::default().max_icc_bytes, 4 << 20);
        let e = ResourceLimits::default()
            .estimate_working_set(10, 20, DecodeWorkingSetProfile::RasterRgba16)
            .unwrap();
        assert_eq!(e.peak_bytes, 9_600);
    }
    #[test]
    fn rejects_pixels_before_allocation() {
        let l = ResourceLimits {
            max_pixels: 3,
            ..Default::default()
        };
        assert!(matches!(
            l.estimate_working_set(2, 2, DecodeWorkingSetProfile::RasterRgba8),
            Err(LimitError::PixelCount { .. })
        ));
    }
    #[test]
    fn checked_math_rejects_overflow() {
        let l = ResourceLimits {
            max_pixels: u64::MAX,
            max_decoded_bytes: u64::MAX,
            max_working_bytes: u64::MAX,
            ..Default::default()
        };
        assert_eq!(
            l.estimate_working_set(
                u32::MAX,
                u32::MAX,
                DecodeWorkingSetProfile::RawMosaic16FullResolution
            ),
            Err(LimitError::ArithmeticOverflow)
        );
    }
    #[test]
    fn source_and_configuration_are_bounded() {
        let l = ResourceLimits::default();
        assert!(matches!(
            l.check_source_bytes(l.max_source_bytes + 1),
            Err(LimitError::SourceBytes { .. })
        ));
        let invalid = ResourceLimits {
            max_icc_bytes: l.max_total_metadata_bytes + 1,
            ..l
        };
        assert_eq!(invalid.validate(), Err(LimitError::InvalidConfiguration));
    }

    #[test]
    fn jpeg_encode_estimate_and_output_are_bounded() {
        let limits = ResourceLimits::default();
        let estimate = limits
            .estimate_encode_working_set(10, 20, EncodeWorkingSetProfile::JpegRgb8, 128)
            .unwrap();
        assert_eq!(estimate.pixels, 200);
        assert_eq!(estimate.resident_image_bytes, 3_200);
        assert_eq!(estimate.encoded_rgb_bytes, 600);
        assert_eq!(estimate.scanline_scratch_bytes, 640);
        assert_eq!(estimate.peak_bytes, 4_696);
        assert!(matches!(
            limits.check_output_bytes(limits.max_output_bytes + 1),
            Err(LimitError::OutputBytes { .. })
        ));
    }
}
