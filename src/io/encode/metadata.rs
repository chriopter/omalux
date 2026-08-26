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

#[derive(Clone, Copy)]
struct RetainedEntry {
    tag: u16,
    kind: u16,
    count: u32,
    source_offset: usize,
    byte_len: usize,
}

struct ParsedEntries {
    endian: Endian,
    retained: [Option<RetainedEntry>; MAX_RETAINED_ENTRIES],
    retained_len: usize,
}

const MAX_RETAINED_ENTRIES: usize = 32;

pub(crate) struct MetadataPlan {
    endian: Option<Endian>,
    retained: [Option<RetainedEntry>; MAX_RETAINED_ENTRIES],
    retained_len: usize,
    output_bytes: usize,
    report: MetadataWriteReport,
}

#[cfg(test)]
pub(crate) fn sanitize_metadata(
    metadata: &MetadataBundle,
    policy: MetadataPolicy,
    limits: &ResourceLimits,
    cancelled: impl Fn() -> bool,
) -> Result<SanitizedMetadata, EncodeError> {
    let plan = plan_metadata(metadata, policy, limits, &cancelled)?;
    materialize_metadata(metadata, plan, &cancelled)
}

/// First pass: validates and sizes the allowlisted TIFF without allocating.
pub(crate) fn plan_metadata(
    metadata: &MetadataBundle,
    policy: MetadataPolicy,
    limits: &ResourceLimits,
    cancelled: &impl Fn() -> bool,
) -> Result<MetadataPlan, EncodeError> {
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
        return Ok(empty_plan(report));
    }
    let Some(raw) = metadata.exif() else {
        return Ok(empty_plan(report));
    };
    limits
        .check_metadata_component(MetadataKind::Exif, raw.len())
        .map_err(EncodeError::Limit)?;
    let parsed = match parse_safe_entries(raw, &mut report, cancelled) {
        Ok(value) => value,
        Err(EncodeError::Metadata) => None,
        Err(error) => return Err(error),
    };
    let Some(ParsedEntries {
        endian,
        retained,
        retained_len,
    }) = parsed
    else {
        report.malformed_exif_dropped = true;
        return Ok(empty_plan(report));
    };
    if cancelled() {
        return Err(EncodeError::Cancelled);
    }
    let output_bytes = exif_output_size(&retained[..retained_len]).ok_or(EncodeError::Metadata)?;
    if output_bytes > MAX_JPEG_EXIF_TIFF_BYTES {
        return Err(EncodeError::Limit(crate::io::LimitError::MetadataBytes {
            kind: MetadataKind::Exif,
            requested: output_bytes as u64,
            maximum: MAX_JPEG_EXIF_TIFF_BYTES as u64,
        }));
    }
    limits
        .check_metadata_component(MetadataKind::Exif, output_bytes)
        .map_err(EncodeError::Limit)?;
    report.exif_output_bytes = output_bytes as u64;
    Ok(MetadataPlan {
        endian: Some(endian),
        retained,
        retained_len,
        output_bytes,
        report,
    })
}

pub(crate) fn materialize_metadata(
    metadata: &MetadataBundle,
    plan: MetadataPlan,
    cancelled: &impl Fn() -> bool,
) -> Result<SanitizedMetadata, EncodeError> {
    if cancelled() {
        return Err(EncodeError::Cancelled);
    }
    let exif = match (plan.endian, metadata.exif(), plan.output_bytes) {
        (_, _, 0) => None,
        (Some(endian), Some(raw), output_bytes) => Some(build_exif(
            endian,
            &plan.retained[..plan.retained_len],
            raw,
            output_bytes,
            cancelled,
        )?),
        _ => return Err(EncodeError::Metadata),
    };
    Ok(SanitizedMetadata {
        exif,
        report: plan.report,
    })
}

impl MetadataPlan {
    pub(crate) fn output_bytes(&self) -> u64 {
        self.output_bytes as u64
    }
}

fn empty_plan(report: MetadataWriteReport) -> MetadataPlan {
    MetadataPlan {
        endian: None,
        retained: [None; MAX_RETAINED_ENTRIES],
        retained_len: 0,
        output_bytes: 0,
        report,
    }
}

fn parse_safe_entries(
    bytes: &[u8],
    report: &mut MetadataWriteReport,
    cancelled: &impl Fn() -> bool,
) -> Result<Option<ParsedEntries>, EncodeError> {
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
    let mut collector = EntryCollector {
        bytes,
        endian,
        retained: [None; MAX_RETAINED_ENTRIES],
        retained_len: 0,
        report,
        cancelled,
    };
    let mut exif_offset = None;
    if !collector.collect_entries(ifd0, &mut exif_offset)? {
        return Ok(None);
    }
    if let Some(offset) = exif_offset
        && !collector.collect_entries(offset, &mut None)?
    {
        return Ok(None);
    }
    if cancelled() {
        return Err(EncodeError::Cancelled);
    }
    collector.retained[..collector.retained_len]
        .sort_unstable_by_key(|entry| entry.expect("retained prefix is populated").tag);
    if cancelled() {
        return Err(EncodeError::Cancelled);
    }
    Ok(Some(ParsedEntries {
        endian,
        retained: collector.retained,
        retained_len: collector.retained_len,
    }))
}

fn ifd_bounds(bytes: &[u8], offset: usize, endian: Endian) -> Option<(usize, usize)> {
    let count = read_u16(bytes, offset, endian).map(usize::from)?;
    let start = offset.checked_add(2)?;
    let end = count
        .checked_mul(12)
        .and_then(|size| start.checked_add(size))
        .and_then(|value| value.checked_add(4))?;
    if end > bytes.len() {
        return None;
    }
    Some((start, count))
}

struct EntryCollector<'a> {
    bytes: &'a [u8],
    endian: Endian,
    retained: [Option<RetainedEntry>; MAX_RETAINED_ENTRIES],
    retained_len: usize,
    report: &'a mut MetadataWriteReport,
    cancelled: &'a dyn Fn() -> bool,
}

impl EntryCollector<'_> {
    fn collect_entries(
        &mut self,
        offset: usize,
        exif_offset: &mut Option<usize>,
    ) -> Result<bool, EncodeError> {
        let Some((start, count)) = ifd_bounds(self.bytes, offset, self.endian) else {
            return Ok(false);
        };
        for index in 0..count {
            if index % 64 == 0 && (self.cancelled)() {
                return Err(EncodeError::Cancelled);
            }
            let value_field = start + index * 12 + 8;
            let tag =
                read_u16(self.bytes, value_field - 8, self.endian).expect("validated IFD entry");
            let kind =
                read_u16(self.bytes, value_field - 6, self.endian).expect("validated IFD entry");
            let value_count =
                read_u32(self.bytes, value_field - 4, self.endian).expect("validated IFD entry");
            match tag {
                0x0112 => {
                    self.report.orientation_removed = true;
                    self.report.unsafe_tags_removed =
                        self.report.unsafe_tags_removed.saturating_add(1);
                }
                0x8825 => {
                    self.report.gps_removed = true;
                    self.report.unsafe_tags_removed =
                        self.report.unsafe_tags_removed.saturating_add(1);
                }
                0x8769 if exif_offset.is_none() && kind == 4 && value_count == 1 => {
                    *exif_offset = read_u32(self.bytes, value_field, self.endian)
                        .and_then(|value| usize::try_from(value).ok());
                    if exif_offset.is_none() {
                        return Err(EncodeError::Metadata);
                    }
                }
                tag if allowed_shape(tag, kind, value_count) => {
                    let Some((source_offset, byte_len)) =
                        entry_range(self.bytes, kind, value_count, value_field, self.endian)
                    else {
                        return Err(EncodeError::Metadata);
                    };
                    let value = &self.bytes[source_offset..source_offset + byte_len];
                    if matches!(kind, 5 | 10)
                        && value.as_chunks::<8>().0.iter().any(|rational| {
                            read_u32(rational, 4, self.endian)
                                .is_none_or(|denominator| denominator == 0)
                        })
                    {
                        return Err(EncodeError::Metadata);
                    }
                    if self.retained_len == MAX_RETAINED_ENTRIES
                        || self.retained[..self.retained_len]
                            .iter()
                            .flatten()
                            .any(|existing| existing.tag == tag)
                    {
                        return Ok(false);
                    }
                    self.retained[self.retained_len] = Some(RetainedEntry {
                        tag,
                        kind,
                        count: value_count,
                        source_offset,
                        byte_len,
                    });
                    self.retained_len += 1;
                }
                _ => {
                    self.report.unsafe_tags_removed =
                        self.report.unsafe_tags_removed.saturating_add(1);
                }
            }
        }
        if (self.cancelled)() {
            return Err(EncodeError::Cancelled);
        }
        Ok(true)
    }
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

fn entry_range(
    bytes: &[u8],
    kind: u16,
    count: u32,
    value_field: usize,
    endian: Endian,
) -> Option<(usize, usize)> {
    let element = match kind {
        1 | 2 | 7 => 1_usize,
        3 => 2,
        4 | 9 => 4,
        5 | 10 => 8,
        _ => return None,
    };
    let size = element.checked_mul(usize::try_from(count).ok()?)?;
    let start = if size <= 4 {
        value_field
    } else {
        usize::try_from(read_u32(bytes, value_field, endian)?).ok()?
    };
    bytes.get(start..start.checked_add(size)?)?;
    Some((start, size))
}

fn exif_output_size(entries: &[Option<RetainedEntry>]) -> Option<usize> {
    if entries.is_empty() {
        return Some(0);
    }
    let ifd0_size = 2 + 12 + 4;
    let exif_ifd_offset = 8_usize.checked_add(ifd0_size)?;
    let exif_ifd_size = 2_usize
        .checked_add(entries.len().checked_mul(12)?)?
        .checked_add(4)?;
    let data_offset = exif_ifd_offset.checked_add(exif_ifd_size)?;
    let external_bytes = entries.iter().try_fold(0_usize, |total, entry| {
        let entry = entry.as_ref()?;
        if entry.byte_len > 4 {
            total.checked_add(entry.byte_len)
        } else {
            Some(total)
        }
    })?;
    data_offset.checked_add(external_bytes)
}

fn build_exif(
    endian: Endian,
    entries: &[Option<RetainedEntry>],
    source: &[u8],
    total: usize,
    cancelled: &impl Fn() -> bool,
) -> Result<Vec<u8>, EncodeError> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let ifd0_size = 2 + 12 + 4;
    let exif_ifd_offset = 8_usize
        .checked_add(ifd0_size)
        .ok_or(EncodeError::Metadata)?;
    let exif_ifd_size = 2_usize
        .checked_add(entries.len().checked_mul(12).ok_or(EncodeError::Metadata)?)
        .and_then(|value| value.checked_add(4))
        .ok_or(EncodeError::Metadata)?;
    let mut data_offset = exif_ifd_offset
        .checked_add(exif_ifd_size)
        .ok_or(EncodeError::Metadata)?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(total)
        .map_err(|_| EncodeError::Limit(crate::io::LimitError::Allocation))?;
    output.resize(total, 0);
    output[..2].copy_from_slice(match endian {
        Endian::Little => b"II",
        Endian::Big => b"MM",
    });
    write_u16(&mut output, 2, 42, endian).ok_or(EncodeError::Metadata)?;
    write_u32(&mut output, 4, 8, endian).ok_or(EncodeError::Metadata)?;
    write_u16(&mut output, 8, 1, endian).ok_or(EncodeError::Metadata)?;
    write_u16(&mut output, 10, 0x8769, endian).ok_or(EncodeError::Metadata)?;
    write_u16(&mut output, 12, 4, endian).ok_or(EncodeError::Metadata)?;
    write_u32(&mut output, 14, 1, endian).ok_or(EncodeError::Metadata)?;
    write_u32(
        &mut output,
        18,
        u32::try_from(exif_ifd_offset).map_err(|_| EncodeError::Metadata)?,
        endian,
    )
    .ok_or(EncodeError::Metadata)?;
    write_u16(
        &mut output,
        exif_ifd_offset,
        u16::try_from(entries.len()).map_err(|_| EncodeError::Metadata)?,
        endian,
    )
    .ok_or(EncodeError::Metadata)?;
    for (index, entry) in entries.iter().enumerate() {
        if cancelled() {
            return Err(EncodeError::Cancelled);
        }
        let entry = entry.as_ref().ok_or(EncodeError::Metadata)?;
        let at = exif_ifd_offset + 2 + index * 12;
        write_u16(&mut output, at, entry.tag, endian).ok_or(EncodeError::Metadata)?;
        write_u16(&mut output, at + 2, entry.kind, endian).ok_or(EncodeError::Metadata)?;
        write_u32(&mut output, at + 4, entry.count, endian).ok_or(EncodeError::Metadata)?;
        let value_end = entry
            .source_offset
            .checked_add(entry.byte_len)
            .ok_or(EncodeError::Metadata)?;
        let value = source
            .get(entry.source_offset..value_end)
            .ok_or(EncodeError::Metadata)?;
        if entry.byte_len <= 4 {
            output[at + 8..at + 8 + entry.byte_len].copy_from_slice(value);
        } else {
            write_u32(
                &mut output,
                at + 8,
                u32::try_from(data_offset).map_err(|_| EncodeError::Metadata)?,
                endian,
            )
            .ok_or(EncodeError::Metadata)?;
            output[data_offset..data_offset + entry.byte_len].copy_from_slice(value);
            data_offset += entry.byte_len;
        }
        if cancelled() {
            return Err(EncodeError::Cancelled);
        }
    }
    Ok(output)
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
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        thread,
    };

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
        let ParsedEntries {
            retained,
            retained_len,
            ..
        } = parse_safe_entries(&exif, &mut MetadataWriteReport::default(), &|| false)
            .unwrap()
            .unwrap();
        assert_eq!(
            retained[..retained_len]
                .iter()
                .flatten()
                .map(|entry| entry.tag)
                .collect::<Vec<_>>(),
            [0x829a, 0x8827]
        );
        assert!(sanitized.report.gps_removed);
        assert!(sanitized.report.orientation_removed);
        assert!(sanitized.report.xmp_dropped);
        assert!(sanitized.report.iptc_dropped);
        assert_eq!(sanitized.report.unsafe_tags_removed, 5);
    }

    #[test]
    fn concurrent_cancellation_preempts_a_large_ifd_scan() {
        let count = u16::MAX;
        let mut source = vec![0_u8; 8 + 2 + usize::from(count) * 12 + 4];
        source[..8].copy_from_slice(b"II*\0\x08\0\0\0");
        write_u16(&mut source, 8, count, Endian::Little).unwrap();
        for index in 0..usize::from(count) {
            let at = 10 + index * 12;
            write_u16(&mut source, at, 0xc000, Endian::Little).unwrap();
            write_u16(&mut source, at + 2, 3, Endian::Little).unwrap();
            write_u32(&mut source, at + 4, 1, Endian::Little).unwrap();
        }
        let metadata =
            MetadataBundle::try_new(Some(source), None, None, true, &ResourceLimits::default())
                .unwrap();
        let polls = Arc::new(AtomicUsize::new(0));
        let cancelled = Arc::new(AtomicBool::new(false));
        let worker_polls = Arc::clone(&polls);
        let worker_cancelled = Arc::clone(&cancelled);
        let worker = thread::spawn(move || {
            while worker_polls.load(Ordering::Acquire) < 2 {
                thread::yield_now();
            }
            worker_cancelled.store(true, Ordering::Release);
        });
        let result = sanitize_metadata(
            &metadata,
            MetadataPolicy::StripLocation,
            &ResourceLimits::default(),
            || {
                polls.fetch_add(1, Ordering::AcqRel);
                cancelled.load(Ordering::Acquire)
            },
        );
        worker.join().unwrap();
        assert!(matches!(result, Err(EncodeError::Cancelled)));
        assert!(polls.load(Ordering::Acquire) >= 3);
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
