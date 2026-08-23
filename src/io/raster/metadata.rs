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
    let parsed = parse_ifd0(raw);
    let Some((orientation, orientation_value_offset, has_gps)) = parsed else {
        return Ok(dropped(1));
    };
    if has_gps {
        return Ok(dropped(orientation));
    }
    let mut normalized = raw.to_vec();
    if let Some((offset, endian)) = orientation_value_offset {
        write_u16(&mut normalized, offset, 1, endian).ok_or(DecodeError::Metadata)?;
    }
    Ok(ExifOutcome {
        bytes: Some(normalized),
        orientation,
        diagnostics: Vec::new(),
    })
}

fn dropped(orientation: u8) -> ExifOutcome {
    ExifOutcome {
        bytes: None,
        orientation,
        diagnostics: vec![Diagnostic {
            severity: DiagnosticSeverity::Warning,
            code: DiagnosticCode::MetadataDropped,
        }],
    }
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

type OrientationLocation = Option<(usize, Endian)>;

fn parse_ifd0(bytes: &[u8]) -> Option<(u8, OrientationLocation, bool)> {
    if bytes.len() < 8 {
        return None;
    }
    let endian = match &bytes[..2] {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    if read_u16(bytes, 2, endian)? != 42 {
        return None;
    }
    let ifd = usize::try_from(read_u32(bytes, 4, endian)?).ok()?;
    let count = usize::from(read_u16(bytes, ifd, endian)?);
    let entries_start = ifd.checked_add(2)?;
    let entries_bytes = count.checked_mul(12)?;
    let ifd_end = entries_start.checked_add(entries_bytes)?.checked_add(4)?;
    if ifd_end > bytes.len() {
        return None;
    }
    let mut orientation = 1_u8;
    let mut location = None;
    let mut has_gps = false;
    for index in 0..count {
        let entry = entries_start.checked_add(index.checked_mul(12)?)?;
        let tag = read_u16(bytes, entry, endian)?;
        if tag == 0x8825 {
            has_gps = true;
        }
        if tag == 0x0112 {
            let kind = read_u16(bytes, entry + 2, endian)?;
            let values = read_u32(bytes, entry + 4, endian)?;
            if kind != 3 || values != 1 {
                return None;
            }
            let value = read_u16(bytes, entry + 8, endian)?;
            orientation = u8::try_from(value)
                .ok()
                .filter(|value| (1..=8).contains(value))?;
            location = Some((entry + 8, endian));
        }
    }
    Some((orientation, location, has_gps))
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
        let normalized = normalize_exif(Some(&exif(6, false)), &ResourceLimits::default()).unwrap();
        assert_eq!(normalized.orientation, 6);
        assert_eq!(normalized.bytes.unwrap()[18], 1);
        let gps = normalize_exif(Some(&exif(8, true)), &ResourceLimits::default()).unwrap();
        assert_eq!(gps.orientation, 8);
        assert!(gps.bytes.is_none());
        assert_eq!(gps.diagnostics[0].code, DiagnosticCode::MetadataDropped);
    }
}
