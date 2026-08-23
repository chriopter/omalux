use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

use rustix::fs::{self, FileType, Mode, OFlags};

use crate::io::{DecodeError, DigestError, ResourceLimits, SourceDigestV1, SourceFileIdentity};

pub(super) struct BufferedSource {
    pub bytes: Vec<u8>,
    pub digest: SourceDigestV1,
    pub identity: SourceFileIdentity,
}

pub(super) fn read_once(
    path: &Path,
    limits: &ResourceLimits,
    cancelled: impl Fn() -> bool,
) -> Result<BufferedSource, DecodeError> {
    if cancelled() {
        return Err(DecodeError::Cancelled);
    }
    let fd = fs::open(
        path,
        OFlags::RDONLY | OFlags::NONBLOCK | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        if error == rustix::io::Errno::LOOP {
            DecodeError::UnsupportedFormat
        } else {
            DecodeError::Input(std_error(error))
        }
    })?;
    read_file_once(File::from(fd), limits, cancelled)
}

pub(super) fn read_file_once(
    mut file: File,
    limits: &ResourceLimits,
    cancelled: impl Fn() -> bool,
) -> Result<BufferedSource, DecodeError> {
    let stat = fs::fstat(&file).map_err(|error| DecodeError::Input(std_error(error)))?;
    if !FileType::from_raw_mode(stat.st_mode).is_file() {
        return Err(DecodeError::UnsupportedFormat);
    }
    let advertised = u64::try_from(stat.st_size).map_err(|_| DecodeError::UnsupportedFormat)?;
    let identity = SourceFileIdentity::from_device_inode(stat.st_dev, stat.st_ino);
    limits
        .check_source_bytes(advertised)
        .map_err(DecodeError::Limit)?;
    let initial = usize::try_from(advertised)
        .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(initial)
        .map_err(|_| DecodeError::Limit(crate::io::LimitError::Allocation))?;
    let (digest, written) = SourceDigestV1::copy_from_reader(
        &mut file,
        FallibleVecWriter(&mut bytes),
        limits,
        cancelled,
    )
    .map_err(map_digest)?;
    if usize::try_from(written).ok() != Some(bytes.len()) {
        return Err(DecodeError::CorruptInput);
    }
    Ok(BufferedSource {
        bytes,
        digest,
        identity,
    })
}

struct FallibleVecWriter<'a>(&'a mut Vec<u8>);

impl Write for FallibleVecWriter<'_> {
    fn write(&mut self, chunk: &[u8]) -> io::Result<usize> {
        self.0.try_reserve_exact(chunk.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "source buffer allocation failed",
            )
        })?;
        self.0.extend_from_slice(chunk);
        Ok(chunk.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn map_digest(error: DigestError) -> DecodeError {
    match error {
        DigestError::Read(error) if error.kind() == io::ErrorKind::Interrupted => {
            DecodeError::Cancelled
        }
        DigestError::Read(error) if error.kind() == io::ErrorKind::OutOfMemory => {
            DecodeError::Limit(crate::io::LimitError::Allocation)
        }
        DigestError::Read(error) => DecodeError::Input(error),
        DigestError::Limit(error) => DecodeError::Limit(error),
    }
}

fn std_error(error: rustix::io::Errno) -> io::Error {
    io::Error::from_raw_os_error(error.raw_os_error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::LimitError;

    #[test]
    fn shared_digest_errors_map_to_precise_raster_categories() {
        assert!(matches!(
            map_digest(DigestError::Read(io::Error::from(
                io::ErrorKind::Interrupted
            ))),
            DecodeError::Cancelled
        ));
        assert!(matches!(
            map_digest(DigestError::Read(io::Error::from(
                io::ErrorKind::OutOfMemory
            ))),
            DecodeError::Limit(LimitError::Allocation)
        ));
        assert!(matches!(
            map_digest(DigestError::Limit(LimitError::ArithmeticOverflow)),
            DecodeError::Limit(LimitError::ArithmeticOverflow)
        ));
        assert!(matches!(
            map_digest(DigestError::Read(io::Error::from(
                io::ErrorKind::BrokenPipe
            ))),
            DecodeError::Input(_)
        ));
    }
}
