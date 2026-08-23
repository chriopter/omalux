use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

use rustix::fs::{self, FileType, Mode, OFlags};

use crate::io::{DecodeError, ResourceLimits, SourceDigestV1};

pub(super) struct BufferedSource {
    pub bytes: Vec<u8>,
    pub digest: SourceDigestV1,
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
    let stat = fs::fstat(&fd).map_err(|error| DecodeError::Input(std_error(error)))?;
    if !FileType::from_raw_mode(stat.st_mode).is_file() {
        return Err(DecodeError::UnsupportedFormat);
    }
    let advertised = u64::try_from(stat.st_size).map_err(|_| DecodeError::UnsupportedFormat)?;
    limits
        .check_source_bytes(advertised)
        .map_err(DecodeError::Limit)?;
    let initial = usize::try_from(advertised)
        .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(initial)
        .map_err(|_| DecodeError::Input(io::Error::other("source buffer allocation failed")))?;
    let mut file = File::from(fd);
    let mut chunk = [0_u8; 64 * 1024];
    loop {
        if cancelled() {
            return Err(DecodeError::Cancelled);
        }
        let count = file.read(&mut chunk).map_err(DecodeError::Input)?;
        if count == 0 {
            break;
        }
        let next = u64::try_from(bytes.len())
            .map_err(|_| DecodeError::Limit(crate::io::LimitError::ArithmeticOverflow))?
            .checked_add(count as u64)
            .ok_or(DecodeError::Limit(
                crate::io::LimitError::ArithmeticOverflow,
            ))?;
        limits
            .check_source_bytes(next)
            .map_err(DecodeError::Limit)?;
        bytes
            .try_reserve(count)
            .map_err(|_| DecodeError::Limit(crate::io::LimitError::Allocation))?;
        bytes.extend_from_slice(&chunk[..count]);
    }
    let digest = SourceDigestV1::from_bytes(&bytes);
    Ok(BufferedSource { bytes, digest })
}

fn std_error(error: rustix::io::Errno) -> io::Error {
    io::Error::from_raw_os_error(error.raw_os_error())
}
