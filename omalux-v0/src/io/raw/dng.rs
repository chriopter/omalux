//! Lens corrections a DNG carries as opcodes.
//!
//! Phone and drone cameras rely on their processing to undo what their lenses
//! do: the corners of a raw frame can sit close to two stops darker than the
//! centre, because a tiny lens cannot deliver light evenly across the sensor.
//! Their DNGs describe the fix as opcodes, and every camera-style rendering
//! applies them.
//!
//! The decoder this crate drives applies the opcodes that act on the Bayer
//! data before demosaic, gain maps among them. It does not apply the
//! post-demosaic list, and `FixVignetteRadial` lives there: without this
//! module a drone frame rendered its sky dark blue in every corner, sixty
//! counts under the centre, and no adjustment could put that right. The
//! polynomial is applied after colour conversion, which is where the
//! specification places it.

use std::{fs::File, os::unix::fs::FileExt};

use rayon::prelude::*;

use crate::develop::CpuImage;

const TAG_NEW_SUBFILE_TYPE: u16 = 254;
const TAG_WIDTH: u16 = 256;
const TAG_HEIGHT: u16 = 257;
const TAG_ORIENTATION: u16 = 274;
const TAG_SUB_IFDS: u16 = 330;
const TAG_ACTIVE_AREA: u16 = 50829;
const TAG_OPCODE_LIST3: u16 = 51022;

const OPCODE_FIX_VIGNETTE_RADIAL: u32 = 3;

/// Bounds on what is read from the file. An opcode list of a few hundred
/// kilobytes is a large one; anything beyond these is not a lens correction.
const MAX_IFD_ENTRIES: usize = 512;
const MAX_SUB_IFDS: usize = 16;
const MAX_OPCODE_LIST_BYTES: u64 = 4 << 20;
const MAX_OPCODES: u32 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct VignetteRadial {
    coefficients: [f64; 5],
    center_x: f64,
    center_y: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct LensCorrections {
    active_width: u32,
    active_height: u32,
    orientation: u8,
    vignette: VignetteRadial,
}

struct Reader<'a> {
    file: &'a File,
    endian: Endian,
}

impl Reader<'_> {
    fn bytes(&self, offset: u64, length: usize) -> Option<Vec<u8>> {
        let mut buffer = vec![0_u8; length];
        self.file.read_exact_at(&mut buffer, offset).ok()?;
        Some(buffer)
    }

    fn u16_at(&self, bytes: &[u8], offset: usize) -> Option<u16> {
        let value: [u8; 2] = bytes.get(offset..offset + 2)?.try_into().ok()?;
        Some(match self.endian {
            Endian::Little => u16::from_le_bytes(value),
            Endian::Big => u16::from_be_bytes(value),
        })
    }

    fn u32_at(&self, bytes: &[u8], offset: usize) -> Option<u32> {
        let value: [u8; 4] = bytes.get(offset..offset + 4)?.try_into().ok()?;
        Some(match self.endian {
            Endian::Little => u32::from_le_bytes(value),
            Endian::Big => u32::from_be_bytes(value),
        })
    }
}

#[derive(Clone, Debug)]
struct Entry {
    tag: u16,
    kind: u16,
    count: u32,
    /// The four value bytes as stored; an offset when the value does not fit.
    raw: [u8; 4],
}

fn type_size(kind: u16) -> Option<u64> {
    Some(match kind {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => return None,
    })
}

impl Entry {
    fn byte_length(&self) -> Option<u64> {
        type_size(self.kind)?.checked_mul(u64::from(self.count))
    }

    /// The entry's value bytes, wherever they live.
    fn value_bytes(&self, reader: &Reader<'_>, limit: u64) -> Option<Vec<u8>> {
        let length = self.byte_length()?;
        if length > limit {
            return None;
        }
        if length <= 4 {
            return Some(self.raw[..length as usize].to_vec());
        }
        let offset = reader.u32_at(&self.raw, 0)?;
        reader.bytes(u64::from(offset), length as usize)
    }

    /// Unsigned integers of SHORT or LONG type, in order.
    fn integers(&self, reader: &Reader<'_>) -> Option<Vec<u32>> {
        let bytes = self.value_bytes(reader, 4096)?;
        let mut values = Vec::with_capacity(self.count as usize);
        for index in 0..self.count as usize {
            let value = match self.kind {
                1 => u32::from(*bytes.get(index)?),
                3 => u32::from(reader.u16_at(&bytes, index * 2)?),
                4 | 13 => reader.u32_at(&bytes, index * 4)?,
                _ => return None,
            };
            values.push(value);
        }
        Some(values)
    }
}

fn read_ifd(reader: &Reader<'_>, offset: u64) -> Option<Vec<Entry>> {
    let head = reader.bytes(offset, 2)?;
    let count = usize::from(reader.u16_at(&head, 0)?);
    if count == 0 || count > MAX_IFD_ENTRIES {
        return None;
    }
    let body = reader.bytes(offset + 2, count * 12)?;
    let mut entries = Vec::with_capacity(count);
    for index in 0..count {
        let at = index * 12;
        let mut raw = [0_u8; 4];
        raw.copy_from_slice(&body[at + 8..at + 12]);
        entries.push(Entry {
            tag: reader.u16_at(&body, at)?,
            kind: reader.u16_at(&body, at + 2)?,
            count: reader.u32_at(&body, at + 4)?,
            raw,
        });
    }
    Some(entries)
}

fn find(entries: &[Entry], tag: u16) -> Option<&Entry> {
    entries.iter().find(|entry| entry.tag == tag)
}

/// Reads the radial vignette correction of a DNG. Anything that is not a
/// DNG, or carries no such opcode, yields `None`.
pub(super) fn read_lens_corrections(file: &File) -> Option<LensCorrections> {
    let mut header = [0_u8; 8];
    file.read_exact_at(&mut header, 0).ok()?;
    let endian = match &header[..2] {
        b"II" => Endian::Little,
        b"MM" => Endian::Big,
        _ => return None,
    };
    let reader = Reader { file, endian };
    if reader.u16_at(&header, 2)? != 42 {
        return None;
    }
    let first = read_ifd(&reader, u64::from(reader.u32_at(&header, 4)?))?;
    let orientation = find(&first, TAG_ORIENTATION)
        .and_then(|entry| entry.integers(&reader))
        .and_then(|values| values.first().copied())
        .filter(|value| (1..=8).contains(value))
        .map_or(1, |value| value as u8);

    let mut candidates = vec![first.clone()];
    if let Some(subs) = find(&first, TAG_SUB_IFDS).and_then(|entry| entry.integers(&reader)) {
        for offset in subs.into_iter().take(MAX_SUB_IFDS) {
            if let Some(entries) = read_ifd(&reader, u64::from(offset)) {
                candidates.push(entries);
            }
        }
    }
    let raw_ifd = candidates.into_iter().find(|entries| {
        find(entries, TAG_NEW_SUBFILE_TYPE)
            .and_then(|entry| entry.integers(&reader))
            .and_then(|values| values.first().copied())
            == Some(0)
    })?;

    let dimension = |tag: u16| {
        find(&raw_ifd, tag)
            .and_then(|entry| entry.integers(&reader))
            .and_then(|values| values.first().copied())
    };
    let width = dimension(TAG_WIDTH)?;
    let height = dimension(TAG_HEIGHT)?;
    let (active_width, active_height) = match find(&raw_ifd, TAG_ACTIVE_AREA)
        .and_then(|entry| entry.integers(&reader))
        .as_deref()
    {
        Some([top, left, bottom, right]) if bottom > top && right > left => {
            (right - left, bottom - top)
        }
        _ => (width, height),
    };
    if active_width == 0 || active_height == 0 {
        return None;
    }
    let bytes = find(&raw_ifd, TAG_OPCODE_LIST3)
        .and_then(|entry| entry.value_bytes(&reader, MAX_OPCODE_LIST_BYTES))?;
    let vignette = parse_opcode_list(&bytes)?;
    Some(LensCorrections {
        active_width,
        active_height,
        orientation,
        vignette,
    })
}
fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
}

fn be_f64(bytes: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_be_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
}

/// Opcode lists are big-endian regardless of the file's byte order. The last
/// radial vignette in the list wins, as later opcodes act on earlier ones.
fn parse_opcode_list(bytes: &[u8]) -> Option<VignetteRadial> {
    let count = be_u32(bytes, 0)?;
    let mut at = 4;
    let mut vignette = None;
    for _ in 0..count.min(MAX_OPCODES) {
        let (Some(id), Some(size)) = (be_u32(bytes, at), be_u32(bytes, at + 12)) else {
            break;
        };
        let start = at + 16;
        let Some(end) = start.checked_add(size as usize) else {
            break;
        };
        let Some(data) = bytes.get(start..end) else {
            break;
        };
        if id == OPCODE_FIX_VIGNETTE_RADIAL
            && let Some(parsed) = parse_vignette(data)
        {
            vignette = Some(parsed);
        }
        at = end;
    }
    vignette
}

fn parse_vignette(data: &[u8]) -> Option<VignetteRadial> {
    let mut values = [0.0_f64; 7];
    for (index, value) in values.iter_mut().enumerate() {
        *value = be_f64(data, index * 8)?;
        if !value.is_finite() {
            return None;
        }
    }
    Some(VignetteRadial {
        coefficients: [values[0], values[1], values[2], values[3], values[4]],
        center_x: values[5],
        center_y: values[6],
    })
}

impl VignetteRadial {
    fn gain(&self, u: f64, v: f64, width: f64, height: f64) -> f64 {
        let cx = self.center_x * width;
        let cy = self.center_y * height;
        let max_radius = (cx.max(width - cx)).hypot(cy.max(height - cy));
        if max_radius <= 0.0 {
            return 1.0;
        }
        let dx = (u * width - cx) / max_radius;
        let dy = (v * height - cy) / max_radius;
        let r2 = dx * dx + dy * dy;
        let mut gain = 1.0;
        let mut term = r2;
        for coefficient in self.coefficients {
            gain += coefficient * term;
            term *= r2;
        }
        gain.max(0.0)
    }
}

/// Maps a normalized position in the oriented output back onto the sensor.
fn sensor_position(orientation: u8, x: f64, y: f64) -> (f64, f64) {
    match orientation {
        2 => (1.0 - x, y),
        3 => (1.0 - x, 1.0 - y),
        4 => (x, 1.0 - y),
        5 => (y, x),
        6 => (y, 1.0 - x),
        7 => (1.0 - y, 1.0 - x),
        8 => (1.0 - y, x),
        _ => (x, y),
    }
}

/// Multiplies every pixel by the gain the vignette polynomial prescribes for
/// its place on the sensor. Alpha is untouched.
pub(super) fn apply(image: &mut CpuImage, corrections: &LensCorrections) {
    let width = image.width() as usize;
    let height = image.height() as usize;
    if width == 0 || height == 0 {
        return;
    }
    let active_width = f64::from(corrections.active_width);
    let active_height = f64::from(corrections.active_height);
    image
        .pixels_mut()
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(y, row)| {
            let ny = (y as f64 + 0.5) / height as f64;
            for (x, pixel) in row.iter_mut().enumerate() {
                let nx = (x as f64 + 0.5) / width as f64;
                let (u, v) = sensor_position(corrections.orientation, nx, ny);
                let gain = corrections.vignette.gain(u, v, active_width, active_height);
                pixel.red = (f64::from(pixel.red) * gain) as f32;
                pixel.green = (f64::from(pixel.green) * gain) as f32;
                pixel.blue = (f64::from(pixel.blue) * gain) as f32;
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::develop::RgbaPixel;
    use std::io::Write;

    fn vignette_opcode(coefficients: [f64; 5], cx: f64, cy: f64) -> Vec<u8> {
        let mut data = Vec::new();
        for value in coefficients.iter().chain([cx, cy].iter()) {
            data.extend_from_slice(&value.to_be_bytes());
        }
        data
    }

    fn opcode_list(opcodes: &[(u32, Vec<u8>)]) -> Vec<u8> {
        let mut bytes = (opcodes.len() as u32).to_be_bytes().to_vec();
        for (id, data) in opcodes {
            bytes.extend_from_slice(&id.to_be_bytes());
            bytes.extend_from_slice(&1_u32.to_be_bytes());
            bytes.extend_from_slice(&0_u32.to_be_bytes());
            bytes.extend_from_slice(&(data.len() as u32).to_be_bytes());
            bytes.extend_from_slice(data);
        }
        bytes
    }

    /// A little-endian TIFF: IFD0 holds a thumbnail and points at a sub-IFD
    /// that is the raw frame, the way DNGs are laid out.
    fn synthetic_dng(orientation: u16, list3: &[u8]) -> Vec<u8> {
        let mut bytes = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
        // IFD0: NewSubfileType 1, Orientation, SubIFDs -> offset filled later.
        let ifd0_entries: Vec<(u16, u16, u32, u32)> = vec![
            (254, 4, 1, 1),
            (274, 3, 1, u32::from(orientation)),
            (330, 4, 1, 0),
        ];
        let ifd0_at = bytes.len();
        bytes.extend_from_slice(&(ifd0_entries.len() as u16).to_le_bytes());
        for (tag, kind, count, value) in &ifd0_entries {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&kind.to_le_bytes());
            bytes.extend_from_slice(&count.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        // Data blobs for the raw IFD. The four CFA bytes fit inline.
        let cfa_inline = u32::from_le_bytes([1, 2, 0, 1]);
        let active_at = bytes.len();
        for value in [0_u32, 0, 3072, 4080] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        let list3_at = bytes.len();
        bytes.extend_from_slice(list3);
        while bytes.len() % 2 != 0 {
            bytes.push(0);
        }
        let sub_at = bytes.len();
        let mut sub_entries: Vec<(u16, u16, u32, u32)> = vec![
            (254, 4, 1, 0),
            (256, 4, 1, 4080),
            (257, 4, 1, 3072),
            (33421, 3, 2, 2 | (2 << 16)),
            (33422, 1, 4, cfa_inline),
            (50829, 4, 4, active_at as u32),
        ];
        if !list3.is_empty() {
            sub_entries.push((51022, 7, list3.len() as u32, list3_at as u32));
        }
        bytes.extend_from_slice(&(sub_entries.len() as u16).to_le_bytes());
        for (tag, kind, count, value) in &sub_entries {
            bytes.extend_from_slice(&tag.to_le_bytes());
            bytes.extend_from_slice(&kind.to_le_bytes());
            bytes.extend_from_slice(&count.to_le_bytes());
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        // Patch the SubIFDs pointer.
        let pointer_at = ifd0_at + 2 + 2 * 12 + 8;
        bytes[pointer_at..pointer_at + 4].copy_from_slice(&(sub_at as u32).to_le_bytes());
        bytes
    }

    fn temp_file(bytes: &[u8]) -> File {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(bytes).unwrap();
        file
    }

    fn flat_image(width: u32, height: u32) -> CpuImage {
        let pixels = vec![RgbaPixel::new(0.2, 0.2, 0.2, 1.0).unwrap(); (width * height) as usize];
        CpuImage::new(width, height, pixels).unwrap()
    }

    #[test]
    fn a_radial_vignette_is_read_and_lifts_the_corners_not_the_centre() {
        let list3 = opcode_list(&[(
            OPCODE_FIX_VIGNETTE_RADIAL,
            vignette_opcode([0.5, 0.0, 0.0, 0.0, 0.0], 0.5, 0.5),
        )]);
        let file = temp_file(&synthetic_dng(1, &list3));
        let corrections = read_lens_corrections(&file).expect("opcodes are read");
        assert_eq!(
            (corrections.active_width, corrections.active_height),
            (4080, 3072)
        );
        let mut image = flat_image(41, 31);
        apply(&mut image, &corrections);
        let pixels = image.pixels();
        let centre = pixels[15 * 41 + 20].red;
        let corner = pixels[0].red;
        assert!((centre - 0.2).abs() < 1.0e-3, "centre moved to {centre}");
        // The farthest corner sits at r = 1 (less half a pixel): gain 1.5.
        assert!(corner > 0.29 && corner <= 0.3, "corner is {corner}");
        assert_eq!(pixels[0].alpha, 1.0);
    }

    #[test]
    fn the_correction_follows_the_sensor_through_orientation() {
        // Centre pushed to the left edge of the sensor: the right edge sits at
        // r = 1 and receives the full gain, the left edge none.
        let list3 = opcode_list(&[(
            OPCODE_FIX_VIGNETTE_RADIAL,
            vignette_opcode([1.0, 0.0, 0.0, 0.0, 0.0], 0.0, 0.5),
        )]);
        let upright = read_lens_corrections(&temp_file(&synthetic_dng(1, &list3))).unwrap();
        let mut image = flat_image(20, 10);
        apply(&mut image, &upright);
        let left = image.pixels()[5 * 20].green;
        let right = image.pixels()[5 * 20 + 19].green;
        assert!(left < 0.21 && right > 0.35, "left {left} right {right}");

        // Rotated 90° clockwise for display, the sensor's left edge is the
        // top of the output.
        let rotated = read_lens_corrections(&temp_file(&synthetic_dng(6, &list3))).unwrap();
        let mut image = flat_image(10, 20);
        apply(&mut image, &rotated);
        let top = image.pixels()[5].green;
        let bottom = image.pixels()[19 * 10 + 5].green;
        assert!(top < 0.21 && bottom > 0.35, "top {top} bottom {bottom}");
    }

    #[test]
    fn files_without_opcodes_or_without_a_tiff_header_yield_nothing() {
        assert!(read_lens_corrections(&temp_file(&synthetic_dng(1, &[]))).is_none());
        assert!(read_lens_corrections(&temp_file(b"P6\n1 1\n255\n\0\0\0")).is_none());
        assert!(read_lens_corrections(&temp_file(b"")).is_none());
    }

    #[test]
    fn malformed_opcodes_are_ignored_rather_than_trusted() {
        let mut truncated = vignette_opcode([0.5, 0.0, 0.0, 0.0, 0.0], 0.5, 0.5);
        truncated.truncate(40);
        let mut infinite = vignette_opcode([0.5, 0.0, 0.0, 0.0, 0.0], 0.5, 0.5);
        infinite[..8].copy_from_slice(&f64::INFINITY.to_be_bytes());
        let list3 = opcode_list(&[
            (OPCODE_FIX_VIGNETTE_RADIAL, truncated),
            (OPCODE_FIX_VIGNETTE_RADIAL, infinite),
        ]);
        assert!(read_lens_corrections(&temp_file(&synthetic_dng(1, &list3))).is_none());
    }
}
