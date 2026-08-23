use crate::{
    develop::{CpuImage, RgbaPixel},
    io::{DecodeError, DecodeWorkingSetProfile, ResourceLimits},
};
use std::io::{self, Read};
const HEADER_LIMIT: usize = 64 * 1024;

pub(super) fn parse_ppm16(
    reader: impl Read,
    limits: &ResourceLimits,
) -> Result<CpuImage, DecodeError> {
    let mut header = HeaderReader::new(reader);
    let magic = header.token()?;
    if magic != b"P6" {
        return Err(DecodeError::CorruptInput);
    }
    let width = parse_u32(&header.token()?)?;
    let height = parse_u32(&header.token()?)?;
    if width == 0 || height == 0 {
        return Err(DecodeError::CorruptInput);
    }
    if parse_u32(&header.token()?)? != 65_535 {
        return Err(DecodeError::CorruptInput);
    }
    limits
        .estimate_working_set(
            width,
            height,
            DecodeWorkingSetProfile::RawPpm16FullResolution,
        )
        .map_err(DecodeError::Limit)?;
    let count = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or(DecodeError::CorruptInput)?;
    let capacity = usize::try_from(count).map_err(|_| DecodeError::CorruptInput)?;
    let mut payload = header.into_payload()?;
    let mut pixels = Vec::with_capacity(capacity);
    let mut rgb = [0_u8; 6];
    for _ in 0..count {
        payload.read_exact(&mut rgb).map_err(map_payload_io)?;
        let c = |i| f32::from(u16::from_be_bytes([rgb[i], rgb[i + 1]])) / 65_535.0;
        pixels.push(RgbaPixel::new(c(0), c(2), c(4), 1.0).map_err(|_| DecodeError::CorruptInput)?);
    }
    let mut trailing = [0_u8; 1];
    match payload.read(&mut trailing) {
        Ok(0) => {}
        Ok(_) => return Err(DecodeError::CorruptInput),
        Err(e) => return Err(DecodeError::RawBackendCaptureIo(e)),
    }
    CpuImage::new(width, height, pixels).map_err(|_| DecodeError::CorruptInput)
}
struct HeaderReader<R> {
    reader: R,
    consumed: usize,
    delimiter: u8,
}
impl<R: Read> HeaderReader<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            consumed: 0,
            delimiter: 0,
        }
    }
    fn token(&mut self) -> Result<Vec<u8>, DecodeError> {
        let mut out = Vec::new();
        loop {
            let byte = self.byte()?;
            if byte.is_ascii_whitespace() {
                continue;
            }
            if byte == b'#' {
                self.comment()?;
                continue;
            }
            out.push(byte);
            break;
        }
        loop {
            let byte = self.byte()?;
            if byte.is_ascii_whitespace() {
                self.delimiter = byte;
                return Ok(out);
            }
            if byte == b'#' {
                return Err(DecodeError::CorruptInput);
            }
            out.push(byte);
        }
    }
    fn comment(&mut self) -> Result<(), DecodeError> {
        loop {
            if self.byte()? == b'\n' {
                return Ok(());
            }
        }
    }
    fn byte(&mut self) -> Result<u8, DecodeError> {
        self.consumed = self
            .consumed
            .checked_add(1)
            .ok_or(DecodeError::CorruptInput)?;
        if self.consumed > HEADER_LIMIT {
            return Err(DecodeError::CorruptInput);
        }
        let mut byte = [0_u8; 1];
        self.reader.read_exact(&mut byte).map_err(map_header_io)?;
        Ok(byte[0])
    }
    fn into_payload(mut self) -> Result<PayloadReader<R>, DecodeError> {
        let prefix = if self.delimiter == b'\r' {
            let byte = self.byte()?;
            (byte != b'\n').then_some(byte)
        } else {
            None
        };
        Ok(PayloadReader {
            reader: self.reader,
            prefix,
        })
    }
}
struct PayloadReader<R> {
    reader: R,
    prefix: Option<u8>,
}
impl<R: Read> Read for PayloadReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if let Some(byte) = self.prefix.take() {
            buffer[0] = byte;
            return Ok(1);
        }
        self.reader.read(buffer)
    }
}
fn parse_u32(token: &[u8]) -> Result<u32, DecodeError> {
    if token.is_empty() || !token.iter().all(u8::is_ascii_digit) {
        return Err(DecodeError::CorruptInput);
    }
    std::str::from_utf8(token)
        .ok()
        .and_then(|s| s.parse().ok())
        .ok_or(DecodeError::CorruptInput)
}
fn map_payload_io(error: io::Error) -> DecodeError {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        DecodeError::CorruptInput
    } else {
        DecodeError::RawBackendCaptureIo(error)
    }
}
fn map_header_io(error: io::Error) -> DecodeError {
    map_payload_io(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn ppm(h: &[u8], p: &[u8]) -> Vec<u8> {
        [h, p].concat()
    }
    #[test]
    fn parses_stream_big_endian() {
        let image = parse_ppm16(
            Cursor::new(ppm(b"P6\n# c\n1 1\n65535\n", &[0xff, 0xff, 0x80, 0, 0, 1])),
            &ResourceLimits::default(),
        )
        .unwrap();
        let p = image.pixels()[0];
        assert_eq!(p.red(), 1.0);
        assert_eq!(p.blue(), 1.0 / 65535.0);
        let crlf = parse_ppm16(
            Cursor::new(ppm(b"P6\r\n1 1\r\n65535\r\n", &[0, 1, 0, 2, 0, 3])),
            &ResourceLimits::default(),
        )
        .unwrap();
        assert_eq!(crlf.pixels()[0].blue(), 3.0 / 65535.0);
    }
    #[test]
    fn rejects_partial_trailing_wrong_max() {
        for b in [
            ppm(b"P6 1 1 255\n", &[0; 6]),
            ppm(b"P6 1 1 65535\n", &[0; 5]),
            ppm(b"P6 1 1 65535\n", &[0; 7]),
        ] {
            assert!(matches!(
                parse_ppm16(Cursor::new(b), &ResourceLimits::default()),
                Err(DecodeError::CorruptInput)
            ));
        }
    }
    #[test]
    fn limits_before_alloc() {
        let b = ppm(b"P6 2 2 65535\n", &[0; 24]);
        let l = ResourceLimits {
            max_pixels: 3,
            ..Default::default()
        };
        assert!(matches!(
            parse_ppm16(Cursor::new(b), &l),
            Err(DecodeError::Limit(crate::io::LimitError::PixelCount { .. }))
        ));
    }
    #[test]
    fn cumulative_header_is_bounded() {
        let mut bytes = b"P6\n".to_vec();
        while bytes.len() <= HEADER_LIMIT {
            bytes.extend_from_slice(b"# comment\n");
        }
        bytes.extend_from_slice(b"1 1\n65535\n");
        assert!(matches!(
            parse_ppm16(Cursor::new(bytes), &ResourceLimits::default()),
            Err(DecodeError::CorruptInput)
        ));
    }
}
