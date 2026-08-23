use crate::{
    develop::{CpuImage, RgbaPixel},
    io::{DecodeError, DecodeWorkingSetProfile, ResourceLimits},
};

const HEADER_LIMIT: usize = 64 * 1024;

pub(super) fn parse_ppm16(bytes: &[u8], limits: &ResourceLimits) -> Result<CpuImage, DecodeError> {
    let mut parser = Header { bytes, index: 0 };
    if parser.token()? != b"P6" {
        return Err(DecodeError::CorruptInput);
    }
    let width = parse_u32(parser.token()?)?;
    let height = parse_u32(parser.token()?)?;
    if width == 0 || height == 0 {
        return Err(DecodeError::CorruptInput);
    }
    if parse_u32(parser.token()?)? != 65_535 {
        return Err(DecodeError::CorruptInput);
    }
    let payload = parser.consume_raster_separator()?;
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
    let expected = usize::try_from(count.checked_mul(6).ok_or(DecodeError::CorruptInput)?)
        .map_err(|_| DecodeError::CorruptInput)?;
    if payload.len() != expected {
        return Err(DecodeError::CorruptInput);
    }
    let mut pixels =
        Vec::with_capacity(usize::try_from(count).map_err(|_| DecodeError::CorruptInput)?);
    for rgb in payload.chunks_exact(6) {
        let channel = |i| f32::from(u16::from_be_bytes([rgb[i], rgb[i + 1]])) / 65_535.0;
        pixels.push(
            RgbaPixel::new(channel(0), channel(2), channel(4), 1.0)
                .map_err(|_| DecodeError::CorruptInput)?,
        );
    }
    CpuImage::new(width, height, pixels).map_err(|_| DecodeError::CorruptInput)
}

struct Header<'a> {
    bytes: &'a [u8],
    index: usize,
}
impl<'a> Header<'a> {
    fn token(&mut self) -> Result<&'a [u8], DecodeError> {
        self.skip()?;
        let start = self.index;
        while self.index < self.bytes.len()
            && !self.bytes[self.index].is_ascii_whitespace()
            && self.bytes[self.index] != b'#'
        {
            self.index += 1;
            if self.index > HEADER_LIMIT {
                return Err(DecodeError::CorruptInput);
            }
        }
        if start == self.index {
            return Err(DecodeError::CorruptInput);
        }
        Ok(&self.bytes[start..self.index])
    }
    fn skip(&mut self) -> Result<(), DecodeError> {
        loop {
            while self.index < self.bytes.len() && self.bytes[self.index].is_ascii_whitespace() {
                self.index += 1;
                if self.index > HEADER_LIMIT {
                    return Err(DecodeError::CorruptInput);
                }
            }
            if self.bytes.get(self.index) == Some(&b'#') {
                while self.index < self.bytes.len() && self.bytes[self.index] != b'\n' {
                    self.index += 1;
                    if self.index > HEADER_LIMIT {
                        return Err(DecodeError::CorruptInput);
                    }
                }
            } else {
                return Ok(());
            }
        }
    }
    fn consume_raster_separator(&mut self) -> Result<&'a [u8], DecodeError> {
        let first = *self
            .bytes
            .get(self.index)
            .ok_or(DecodeError::CorruptInput)?;
        if !first.is_ascii_whitespace() {
            return Err(DecodeError::CorruptInput);
        }
        self.index += 1;
        if first == b'\r' && self.bytes.get(self.index) == Some(&b'\n') {
            self.index += 1;
        }
        Ok(&self.bytes[self.index..])
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

#[cfg(test)]
mod tests {
    use super::*;
    fn ppm(header: &[u8], payload: &[u8]) -> Vec<u8> {
        [header, payload].concat()
    }
    #[test]
    fn parses_big_endian_exactly() {
        let image = parse_ppm16(
            &ppm(b"P6\n# c\n1 1\n65535\n", &[0xff, 0xff, 0x80, 0x00, 0, 1]),
            &ResourceLimits::default(),
        )
        .unwrap();
        let p = image.pixels()[0];
        assert_eq!(p.red(), 1.0);
        assert!((p.green() - 32768.0 / 65535.0).abs() < 1e-7);
        assert_eq!(p.blue(), 1.0 / 65535.0);
        assert_eq!(p.alpha(), 1.0);
    }
    #[test]
    fn rejects_wrong_max_partial_trailing_and_huge_header() {
        for bytes in [
            ppm(b"P6 1 1 255\n", &[0; 6]),
            ppm(b"P6 1 1 65535\n", &[0; 5]),
            ppm(b"P6 1 1 65535\n", &[0; 7]),
        ] {
            assert!(matches!(
                parse_ppm16(&bytes, &ResourceLimits::default()),
                Err(DecodeError::CorruptInput)
            ));
        }
        let huge = [b"P6 #".as_slice(), vec![b'x'; HEADER_LIMIT + 1].as_slice()].concat();
        assert!(parse_ppm16(&huge, &ResourceLimits::default()).is_err());
    }
    #[test]
    fn dimensions_are_checked_before_pixel_allocation() {
        let bytes = ppm(b"P6 2 2 65535\n", &[0; 24]);
        let limits = ResourceLimits {
            max_pixels: 3,
            ..Default::default()
        };
        assert!(matches!(
            parse_ppm16(&bytes, &limits),
            Err(DecodeError::Limit(crate::io::LimitError::PixelCount { .. }))
        ));
        assert!(matches!(
            parse_ppm16(b"P6 0 1 65535\n", &ResourceLimits::default()),
            Err(DecodeError::CorruptInput)
        ));
    }
}
