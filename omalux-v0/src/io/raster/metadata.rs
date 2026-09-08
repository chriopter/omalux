use super::{RasterCancellation, allocation_error};
use crate::io::{
    DecodeError, Diagnostic, DiagnosticCode, DiagnosticSeverity, MetadataKind, ResourceLimits,
};

pub(super) struct ExifOutcome {
    pub bytes: Option<Vec<u8>>,
    pub orientation: u8,
    pub diagnostics: Vec<Diagnostic>,
}

pub(super) fn normalize_exif(
    raw: Option<&[u8]>,
    limits: &ResourceLimits,
    cancellation: &RasterCancellation,
) -> Result<ExifOutcome, DecodeError> {
    let Some(raw) = raw else {
        return Ok(ExifOutcome {
            bytes: None,
            orientation: 1,
            diagnostics: Vec::new(),
        });
    };
    limits
        .check_metadata_component(MetadataKind::Exif, raw.len())
        .map_err(DecodeError::Limit)?;
    let parsed = parse_ifd0(raw, cancellation).map_err(|error| match error {
        ExifParseError::DuplicateOrientation => DecodeError::Metadata,
        ExifParseError::Cancelled => DecodeError::Cancelled,
    })?;
    let Some((orientation, orientation_value_offset, has_gps)) = parsed else {
        return dropped(1);
    };
    if has_gps {
        return dropped(orientation);
    }
    let mut normalized = Vec::new();
    normalized
        .try_reserve_exact(raw.len())
        .map_err(|_| allocation_error())?;
    for chunk in raw.chunks(64 * 1024) {
        if cancellation.cancelled() {
            return Err(DecodeError::Cancelled);
        }
        normalized.extend_from_slice(chunk);
    }
    if let Some((offset, endian)) = orientation_value_offset {
        write_u16(&mut normalized, offset, 1, endian).ok_or(DecodeError::Metadata)?;
    }
    Ok(ExifOutcome {
        bytes: Some(normalized),
        orientation,
        diagnostics: Vec::new(),
    })
}

fn dropped(orientation: u8) -> Result<ExifOutcome, DecodeError> {
    let mut diagnostics = Vec::new();
    diagnostics
        .try_reserve_exact(1)
        .map_err(|_| allocation_error())?;
    diagnostics.push(Diagnostic {
        severity: DiagnosticSeverity::Warning,
        code: DiagnosticCode::MetadataDropped,
    });
    Ok(ExifOutcome {
        bytes: None,
        orientation,
        diagnostics,
    })
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

type OrientationLocation = Option<(usize, Endian)>;

enum ExifParseError {
    DuplicateOrientation,
    Cancelled,
}

fn parse_ifd0(
    bytes: &[u8],
    cancellation: &RasterCancellation,
) -> Result<Option<(u8, OrientationLocation, bool)>, ExifParseError> {
    if bytes.len() < 8 {
        return Ok(None);
    }
    let endian = match &bytes[..2] {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return Ok(None),
    };
    let Some(signature) = read_u16(bytes, 2, endian) else {
        return Ok(None);
    };
    if signature != 42 {
        return Ok(None);
    }
    let Some(ifd) = read_u32(bytes, 4, endian).and_then(|value| usize::try_from(value).ok()) else {
        return Ok(None);
    };
    let Some(count) = read_u16(bytes, ifd, endian).map(usize::from) else {
        return Ok(None);
    };
    let Some(entries_start) = ifd.checked_add(2) else {
        return Ok(None);
    };
    let Some(entries_bytes) = count.checked_mul(12) else {
        return Ok(None);
    };
    let Some(ifd_end) = entries_start
        .checked_add(entries_bytes)
        .and_then(|v| v.checked_add(4))
    else {
        return Ok(None);
    };
    if ifd_end > bytes.len() {
        return Ok(None);
    }
    let mut orientation = 1_u8;
    let mut location = None;
    let mut has_gps = false;
    let mut orientation_entry = None;
    for index in 0..count {
        if index % 256 == 0 && cancellation.cancelled() {
            return Err(ExifParseError::Cancelled);
        }
        let Some(entry) = index
            .checked_mul(12)
            .and_then(|v| entries_start.checked_add(v))
        else {
            return Ok(None);
        };
        let Some(tag) = read_u16(bytes, entry, endian) else {
            return Ok(None);
        };
        if tag == 0x8825 {
            has_gps = true;
        }
        if tag == 0x0112 && orientation_entry.replace(entry).is_some() {
            return Err(ExifParseError::DuplicateOrientation);
        }
    }
    if let Some(entry) = orientation_entry {
        let Some(kind) = read_u16(bytes, entry + 2, endian) else {
            return Ok(None);
        };
        let Some(values) = read_u32(bytes, entry + 4, endian) else {
            return Ok(None);
        };
        if kind != 3 || values != 1 {
            return Ok(None);
        }
        let Some(value) = read_u16(bytes, entry + 8, endian) else {
            return Ok(None);
        };
        let Some(value) = u8::try_from(value)
            .ok()
            .filter(|value| (1..=8).contains(value))
        else {
            return Ok(None);
        };
        orientation = value;
        location = Some((entry + 8, endian));
    }
    Ok(Some((orientation, location, has_gps)))
}

fn read_u16(bytes: &[u8], offset: usize, endian: Endian) -> Option<u16> {
    let value: [u8; 2] = bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?;
    Some(match endian {
        Endian::Little => u16::from_le_bytes(value),
        Endian::Big => u16::from_be_bytes(value),
    })
}

fn read_u32(bytes: &[u8], offset: usize, endian: Endian) -> Option<u32> {
    let value: [u8; 4] = bytes.get(offset..offset.checked_add(4)?)?.try_into().ok()?;
    Some(match endian {
        Endian::Little => u32::from_le_bytes(value),
        Endian::Big => u32::from_be_bytes(value),
    })
}

fn write_u16(bytes: &mut [u8], offset: usize, value: u16, endian: Endian) -> Option<()> {
    let target = bytes.get_mut(offset..offset.checked_add(2)?)?;
    let encoded = match endian {
        Endian::Little => value.to_le_bytes(),
        Endian::Big => value.to_be_bytes(),
    };
    target.copy_from_slice(&encoded);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exif(orientation: u16, gps: bool) -> Vec<u8> {
        let count = if gps { 2_u16 } else { 1 };
        let mut bytes = b"II*\0\x08\0\0\0".to_vec();
        bytes.extend_from_slice(&count.to_le_bytes());
        bytes.extend_from_slice(&0x0112_u16.to_le_bytes());
        bytes.extend_from_slice(&3_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.extend_from_slice(&orientation.to_le_bytes());
        bytes.extend_from_slice(&[0, 0]);
        if gps {
            bytes.extend_from_slice(&0x8825_u16.to_le_bytes());
            bytes.extend_from_slice(&4_u16.to_le_bytes());
            bytes.extend_from_slice(&1_u32.to_le_bytes());
            bytes.extend_from_slice(&8_u32.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes
    }

    #[test]
    fn orientation_is_normalized_and_gps_causes_safe_drop() {
        let cancellation = RasterCancellation::default();
        let normalized = normalize_exif(
            Some(&exif(6, false)),
            &ResourceLimits::default(),
            &cancellation,
        )
        .unwrap();
        assert_eq!(normalized.orientation, 6);
        assert_eq!(normalized.bytes.unwrap()[18], 1);
        let gps = normalize_exif(
            Some(&exif(8, true)),
            &ResourceLimits::default(),
            &cancellation,
        )
        .unwrap();
        assert_eq!(gps.orientation, 8);
        assert!(gps.bytes.is_none());
        assert_eq!(gps.diagnostics[0].code, DiagnosticCode::MetadataDropped);
    }

    #[test]
    fn duplicate_orientation_is_loudly_rejected() {
        let mut duplicate = exif(6, false);
        duplicate[8..10].copy_from_slice(&2_u16.to_le_bytes());
        let first = duplicate[10..22].to_vec();
        duplicate.splice(22..22, first);
        assert!(matches!(
            normalize_exif(
                Some(&duplicate),
                &ResourceLimits::default(),
                &RasterCancellation::default()
            ),
            Err(DecodeError::Metadata)
        ));
    }
}
