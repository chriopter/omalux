use crate::io::{EncodeError, MetadataBundle, MetadataKind, MetadataPolicy, ResourceLimits};

/// JPEG APP1 allows 65,533 data bytes, of which `Exif\0\0` consumes six.
pub(crate) const MAX_JPEG_EXIF_TIFF_BYTES: usize = 65_527;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub struct MetadataWriteReport {
    pub exif_input_bytes: u64,
    pub exif_output_bytes: u64,
    pub gps_removed: bool,
    pub orientation_removed: bool,
    pub unsafe_tags_removed: u32,
    pub xmp_dropped: bool,
    pub iptc_dropped: bool,
    pub malformed_exif_dropped: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SanitizedMetadata {
    pub exif: Option<Vec<u8>>,
    pub report: MetadataWriteReport,
}

#[derive(Clone, Copy)]
enum Endian {
    Little,
    Big,
}

#[derive(Clone)]
struct RetainedEntry {
    tag: u16,
    kind: u16,
    count: u32,
    bytes: Vec<u8>,
}

pub(crate) fn sanitize_metadata(
    metadata: &MetadataBundle,
    policy: MetadataPolicy,
    limits: &ResourceLimits,
    cancelled: impl Fn() -> bool,
) -> Result<SanitizedMetadata, EncodeError> {
    limits.validate().map_err(EncodeError::Limit)?;
    if cancelled() {
        return Err(EncodeError::Cancelled);
    }
    let mut report = MetadataWriteReport {
        exif_input_bytes: metadata.exif().map_or(0, |value| value.len() as u64),
        xmp_dropped: metadata.xmp().is_some(),
        iptc_dropped: metadata.iptc().is_some(),
        ..Default::default()
    };
    if policy == MetadataPolicy::StripAll {
        report.unsafe_tags_removed = u32::from(metadata.exif().is_some());
        return Ok(SanitizedMetadata { exif: None, report });
    }
    let Some(raw) = metadata.exif() else {
        return Ok(SanitizedMetadata { exif: None, report });
    };
    limits
        .check_metadata_component(MetadataKind::Exif, raw.len())
        .map_err(EncodeError::Limit)?;
    let parsed = match parse_safe_entries(raw, &mut report, &cancelled) {
        Ok(value) => value,
        Err(EncodeError::Metadata) => None,
        Err(error) => return Err(error),
    };
    let Some((endian, retained)) = parsed else {
        report.malformed_exif_dropped = true;
        return Ok(SanitizedMetadata { exif: None, report });
    };
    if cancelled() {
        return Err(EncodeError::Cancelled);
    }
    let exif = build_exif(endian, retained).ok_or(EncodeError::Metadata)?;
    if exif.len() > MAX_JPEG_EXIF_TIFF_BYTES {
        return Err(EncodeError::Limit(crate::io::LimitError::MetadataBytes {
            kind: MetadataKind::Exif,
            requested: exif.len() as u64,
            maximum: MAX_JPEG_EXIF_TIFF_BYTES as u64,
        }));
    }
    limits
        .check_metadata_component(MetadataKind::Exif, exif.len())
        .map_err(EncodeError::Limit)?;
    report.exif_output_bytes = exif.len() as u64;
    Ok(SanitizedMetadata {
        exif: (!exif.is_empty()).then_some(exif),
        report,
    })
}

fn parse_safe_entries(
    bytes: &[u8],
    report: &mut MetadataWriteReport,
    cancelled: &impl Fn() -> bool,
) -> Result<Option<(Endian, Vec<RetainedEntry>)>, EncodeError> {
    if bytes.len() < 8 {
        return Ok(None);
    }
    let endian = match &bytes[..2] {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return Ok(None),
    };
    if read_u16(bytes, 2, endian) != Some(42) {
        return Ok(None);
    }
    let Some(ifd0) = read_u32(bytes, 4, endian).and_then(|value| usize::try_from(value).ok())
    else {
        return Ok(None);
    };
    let Some(root) = parse_ifd(bytes, ifd0, endian, cancelled)? else {
        return Ok(None);
    };
    let mut retained = Vec::new();
    let mut exif_offset = None;
    collect_entries(
        bytes,
        &root,
        endian,
        &mut retained,
        &mut exif_offset,
        report,
    )?;
    if let Some(offset) = exif_offset {
        let Some(exif_ifd) = parse_ifd(bytes, offset, endian, cancelled)? else {
            return Ok(None);
        };
        collect_entries(bytes, &exif_ifd, endian, &mut retained, &mut None, report)?;
    }
    retained.sort_by_key(|entry| entry.tag);
    if retained.windows(2).any(|pair| pair[0].tag == pair[1].tag) {
        return Ok(None);
    }
    Ok(Some((endian, retained)))
}

struct IfdEntry {
    tag: u16,
    kind: u16,
    count: u32,
    value_field: usize,
}

fn parse_ifd(
    bytes: &[u8],
    offset: usize,
    endian: Endian,
    cancelled: &impl Fn() -> bool,
) -> Result<Option<Vec<IfdEntry>>, EncodeError> {
    let Some(count) = read_u16(bytes, offset, endian).map(usize::from) else {
        return Ok(None);
    };
    let Some(start) = offset.checked_add(2) else {
        return Ok(None);
    };
    let Some(end) = count
        .checked_mul(12)
        .and_then(|size| start.checked_add(size))
        .and_then(|value| value.checked_add(4))
    else {
        return Ok(None);
    };
    if end > bytes.len() {
        return Ok(None);
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| EncodeError::Limit(crate::io::LimitError::Allocation))?;
    for index in 0..count {
        if index % 256 == 0 && cancelled() {
            return Err(EncodeError::Cancelled);
        }
        let at = start + index * 12;
        entries.push(IfdEntry {
            tag: read_u16(bytes, at, endian).expect("validated IFD entry"),
            kind: read_u16(bytes, at + 2, endian).expect("validated IFD entry"),
            count: read_u32(bytes, at + 4, endian).expect("validated IFD entry"),
            value_field: at + 8,
        });
    }
    Ok(Some(entries))
}

fn collect_entries(
    bytes: &[u8],
    entries: &[IfdEntry],
    endian: Endian,
    retained: &mut Vec<RetainedEntry>,
    exif_offset: &mut Option<usize>,
    report: &mut MetadataWriteReport,
) -> Result<(), EncodeError> {
    for entry in entries {
        match entry.tag {
            0x0112 => {
                report.orientation_removed = true;
                report.unsafe_tags_removed = report.unsafe_tags_removed.saturating_add(1);
            }
            0x8825 => {
                report.gps_removed = true;
                report.unsafe_tags_removed = report.unsafe_tags_removed.saturating_add(1);
            }
            0x8769 if exif_offset.is_none() && entry.kind == 4 && entry.count == 1 => {
                *exif_offset = read_u32(bytes, entry.value_field, endian)
                    .and_then(|value| usize::try_from(value).ok());
                if exif_offset.is_none() {
                    return Err(EncodeError::Metadata);
                }
            }
            tag if allowed_shape(tag, entry.kind, entry.count) => {
                let Some(value) = entry_bytes(bytes, entry, endian) else {
                    return Err(EncodeError::Metadata);
                };
                if matches!(entry.kind, 5 | 10)
                    && value.chunks_exact(8).any(|rational| {
                        read_u32(rational, 4, endian).is_none_or(|denominator| denominator == 0)
                    })
                {
                    return Err(EncodeError::Metadata);
                }
                retained.push(RetainedEntry {
                    tag,
                    kind: entry.kind,
                    count: entry.count,
                    bytes: value.to_vec(),
                });
            }
            _ => {
                report.unsafe_tags_removed = report.unsafe_tags_removed.saturating_add(1);
            }
        }
    }
    Ok(())
}

fn allowed_shape(tag: u16, kind: u16, count: u32) -> bool {
    match tag {
        0x829a | 0x829d | 0x9202 | 0x920a => kind == 5 && count == 1,
        0x9201 | 0x9204 => kind == 10 && count == 1,
        0x8827 => matches!(kind, 3 | 4) && count == 1,
        0xa002 | 0xa003 => matches!(kind, 3 | 4) && count == 1,
        0x8822 | 0x8830 | 0x9207 | 0x9208 | 0x9209 | 0xa001 | 0xa402 | 0xa403 | 0xa405 | 0xa406
        | 0xa408 | 0xa409 | 0xa40a => kind == 3 && count == 1,
        0x8832 => kind == 4 && count == 1,
        0xa432 => kind == 5 && count == 4,
        _ => false,
    }
}

fn entry_bytes<'a>(bytes: &'a [u8], entry: &IfdEntry, endian: Endian) -> Option<&'a [u8]> {
    let element = match entry.kind {
        1 | 2 | 7 => 1_usize,
        3 => 2,
        4 | 9 => 4,
        5 | 10 => 8,
        _ => return None,
    };
    let size = element.checked_mul(usize::try_from(entry.count).ok()?)?;
    let start = if size <= 4 {
        entry.value_field
    } else {
        usize::try_from(read_u32(bytes, entry.value_field, endian)?).ok()?
    };
    bytes.get(start..start.checked_add(size)?)
}

fn build_exif(endian: Endian, entries: Vec<RetainedEntry>) -> Option<Vec<u8>> {
    if entries.is_empty() {
        return Some(Vec::new());
    }
    let ifd0_size = 2 + 12 + 4;
    let exif_ifd_offset = 8_usize.checked_add(ifd0_size)?;
    let exif_ifd_size = 2_usize
        .checked_add(entries.len().checked_mul(12)?)?
        .checked_add(4)?;
    let mut data_offset = exif_ifd_offset.checked_add(exif_ifd_size)?;
    let external_bytes = entries.iter().try_fold(0_usize, |total, entry| {
        if entry.bytes.len() > 4 {
            total.checked_add(entry.bytes.len())
        } else {
            Some(total)
        }
    })?;
    let total = data_offset.checked_add(external_bytes)?;
    let mut output = vec![0_u8; total];
    output[..2].copy_from_slice(match endian {
        Endian::Little => b"II",
        Endian::Big => b"MM",
    });
    write_u16(&mut output, 2, 42, endian)?;
    write_u32(&mut output, 4, 8, endian)?;
    write_u16(&mut output, 8, 1, endian)?;
    write_u16(&mut output, 10, 0x8769, endian)?;
    write_u16(&mut output, 12, 4, endian)?;
    write_u32(&mut output, 14, 1, endian)?;
    write_u32(
        &mut output,
        18,
        u32::try_from(exif_ifd_offset).ok()?,
        endian,
    )?;
    write_u16(
        &mut output,
        exif_ifd_offset,
        u16::try_from(entries.len()).ok()?,
        endian,
    )?;
    for (index, entry) in entries.iter().enumerate() {
        let at = exif_ifd_offset + 2 + index * 12;
        write_u16(&mut output, at, entry.tag, endian)?;
        write_u16(&mut output, at + 2, entry.kind, endian)?;
        write_u32(&mut output, at + 4, entry.count, endian)?;
        if entry.bytes.len() <= 4 {
            output[at + 8..at + 8 + entry.bytes.len()].copy_from_slice(&entry.bytes);
        } else {
            write_u32(
                &mut output,
                at + 8,
                u32::try_from(data_offset).ok()?,
                endian,
            )?;
            output[data_offset..data_offset + entry.bytes.len()].copy_from_slice(&entry.bytes);
            data_offset += entry.bytes.len();
        }
    }
    Some(output)
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
    let encoded = match endian {
        Endian::Little => value.to_le_bytes(),
        Endian::Big => value.to_be_bytes(),
    };
    bytes
        .get_mut(offset..offset.checked_add(2)?)?
        .copy_from_slice(&encoded);
    Some(())
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32, endian: Endian) -> Option<()> {
    let encoded = match endian {
        Endian::Little => value.to_le_bytes(),
        Endian::Big => value.to_be_bytes(),
    };
    bytes
        .get_mut(offset..offset.checked_add(4)?)?
        .copy_from_slice(&encoded);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_exif() -> Vec<u8> {
        // IFD0: orientation, GPS, Exif pointer, camera serial. Exif IFD:
        // exposure time, ISO, MakerNote and UserComment.
        let mut bytes = vec![0_u8; 128];
        bytes[..8].copy_from_slice(b"II*\0\x08\0\0\0");
        write_u16(&mut bytes, 8, 4, Endian::Little).unwrap();
        let entries = [
            (0x0112, 3, 1, 1),
            (0x8825, 4, 1, 90),
            (0x8769, 4, 1, 62),
            (0xa431, 2, 4, u32::from_le_bytes(*b"SER\0")),
        ];
        for (index, (tag, kind, count, value)) in entries.into_iter().enumerate() {
            let at = 10 + index * 12;
            write_u16(&mut bytes, at, tag, Endian::Little).unwrap();
            write_u16(&mut bytes, at + 2, kind, Endian::Little).unwrap();
            write_u32(&mut bytes, at + 4, count, Endian::Little).unwrap();
            write_u32(&mut bytes, at + 8, value, Endian::Little).unwrap();
        }
        write_u16(&mut bytes, 62, 4, Endian::Little).unwrap();
        let exif = [
            (0x829a, 5, 1, 120),
            (0x8827, 3, 1, 400),
            (0x927c, 7, 4, u32::from_le_bytes(*b"MAKE")),
            (0x9286, 7, 4, u32::from_le_bytes(*b"USER")),
        ];
        for (index, (tag, kind, count, value)) in exif.into_iter().enumerate() {
            let at = 64 + index * 12;
            write_u16(&mut bytes, at, tag, Endian::Little).unwrap();
            write_u16(&mut bytes, at + 2, kind, Endian::Little).unwrap();
            write_u32(&mut bytes, at + 4, count, Endian::Little).unwrap();
            write_u32(&mut bytes, at + 8, value, Endian::Little).unwrap();
        }
        write_u32(&mut bytes, 120, 1, Endian::Little).unwrap();
        write_u32(&mut bytes, 124, 125, Endian::Little).unwrap();
        bytes
    }

    #[test]
    fn rebuilds_only_safe_numeric_exif() {
        let source = source_exif();
        let metadata = MetadataBundle::try_new(
            Some(source),
            Some(b"SYNTHETIC_LOCATION_PAYLOAD".to_vec()),
            Some(b"iptc".to_vec()),
            true,
            &ResourceLimits::default(),
        )
        .unwrap();
        let sanitized = sanitize_metadata(
            &metadata,
            MetadataPolicy::StripLocation,
            &ResourceLimits::default(),
            || false,
        )
        .unwrap();
        let exif = sanitized.exif.unwrap();
        assert!(!exif.windows(4).any(|window| window == b"MAKE"));
        assert!(!exif.windows(4).any(|window| window == b"USER"));
        assert!(!exif.windows(3).any(|window| window == b"SER"));
        let (_, retained) =
            parse_safe_entries(&exif, &mut MetadataWriteReport::default(), &|| false)
                .unwrap()
                .unwrap();
        assert_eq!(
            retained.iter().map(|entry| entry.tag).collect::<Vec<_>>(),
            [0x829a, 0x8827]
        );
        assert!(sanitized.report.gps_removed);
        assert!(sanitized.report.orientation_removed);
        assert!(sanitized.report.xmp_dropped);
        assert!(sanitized.report.iptc_dropped);
        assert_eq!(sanitized.report.unsafe_tags_removed, 5);
    }

    #[test]
    fn malformed_is_dropped_and_strip_all_never_parses() {
        let metadata = MetadataBundle::try_new(
            Some(b"not tiff".to_vec()),
            None,
            None,
            false,
            &ResourceLimits::default(),
        )
        .unwrap();
        let dropped = sanitize_metadata(
            &metadata,
            MetadataPolicy::StripLocation,
            &ResourceLimits::default(),
            || false,
        )
        .unwrap();
        assert!(dropped.exif.is_none());
        assert!(dropped.report.malformed_exif_dropped);
        let stripped = sanitize_metadata(
            &metadata,
            MetadataPolicy::StripAll,
            &ResourceLimits::default(),
            || false,
        )
        .unwrap();
        assert!(stripped.exif.is_none());
        assert!(!stripped.report.malformed_exif_dropped);
    }

    #[test]
    fn metadata_cancellation_is_loud() {
        assert!(matches!(
            sanitize_metadata(
                &MetadataBundle::default(),
                MetadataPolicy::PreserveSafe,
                &ResourceLimits::default(),
                || true,
            ),
            Err(EncodeError::Cancelled)
        ));
    }
}
