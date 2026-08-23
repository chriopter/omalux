use super::{LimitError, MetadataKind};

/// Conservative process-wide limits applied before allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct ResourceLimits {
    pub max_source_bytes: u64,
    pub max_pixels: u64,
    pub max_decoded_bytes: u64,
    pub max_working_bytes: u64,
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

/// Audited allocation shapes for supported decoder families. Backends must
/// select a named profile instead of supplying optimistic byte counts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DecodeWorkingSetProfile {
    RasterRgba8,
    RasterRgba16,
    RawMosaic16FullResolution,
}

impl DecodeWorkingSetProfile {
    const fn bytes(self) -> (u16, u16) {
        match self {
            Self::RasterRgba8 => (4, 8),
            Self::RasterRgba16 => (8, 8),
            // Mosaic plus conservative demosaic/profile-conversion scratch.
            Self::RawMosaic16FullResolution => (2, 32),
        }
    }
}

impl ResourceLimits {
    /// Estimates decode storage plus two 16-byte RGBA f32 buffers and caller
    /// supplied scratch. Every operation is checked before an allocation.
    pub fn validate(&self) -> Result<(), LimitError> {
        if self.max_source_bytes == 0
            || self.max_pixels == 0
            || self.max_decoded_bytes == 0
            || self.max_working_bytes == 0
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
}
