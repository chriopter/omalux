use std::path::Path;

use super::{EncodeCancellation, JpegEncodeInput, MetadataWriteReport};
use crate::io::{
    AtomicOutputOptions, AtomicOutputOutcome, EncodeError, EncodeOptions, IccProfileProvenance,
    ResourceLimits, SourceFileIdentity,
};

pub struct HeicEncodeRequest<'a> {
    pub input: JpegEncodeInput<'a>,
    pub destination: &'a Path,
    pub source_identity: Option<SourceFileIdentity>,
    pub encode: EncodeOptions,
    pub atomic: AtomicOutputOptions,
    pub limits: &'a ResourceLimits,
    pub cancellation: &'a EncodeCancellation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeicCapability {
    pub libheif_version: String,
    pub encoder: String,
    pub eight_bit: bool,
    pub ten_bit: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeicEncodeReport {
    pub outcome: AtomicOutputOutcome,
    pub width: u32,
    pub height: u32,
    pub quality: u8,
    pub output_bytes: u64,
    pub bit_depth: u8,
    pub libheif_version: String,
    pub encoder: String,
    pub nclx: HeicNclx,
    pub icc: IccProfileProvenance,
    pub clipped_samples: u64,
    pub alpha_flattened_pixels: u64,
    pub metadata: MetadataWriteReport,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeicNclx {
    pub color_primaries: u16,
    pub transfer_characteristics: u16,
    pub matrix_coefficients: u16,
    pub full_range: bool,
}

impl HeicNclx {
    pub const SRGB_FULL_RANGE: Self = Self {
        color_primaries: 1,
        transfer_characteristics: 13,
        matrix_coefficients: 1,
        full_range: true,
    };
}

#[cfg(not(feature = "heic"))]
pub fn encode_heic(_request: HeicEncodeRequest<'_>) -> Result<HeicEncodeReport, EncodeError> {
    Err(EncodeError::HeicBackendNotBuilt)
}

#[cfg(not(feature = "heic"))]
pub fn probe_heic_capability() -> Result<HeicCapability, EncodeError> {
    Err(EncodeError::HeicBackendNotBuilt)
}

#[cfg(feature = "heic")]
mod backend {
    use std::{
        ffi::{CStr, c_void},
        fs::File,
        io::{self, Write},
        ptr,
        sync::{Mutex, MutexGuard, OnceLock},
    };

    use libheif_sys as heif;

    use super::*;
    use crate::io::{AtomicOutputError, LimitError, OutputFormat, write_atomic_output_for_source};

    pub fn encode(request: HeicEncodeRequest<'_>) -> Result<HeicEncodeReport, EncodeError> {
        let prepared = super::super::prepare::prepare_display_rgb8_for(
            request.input,
            &request.encode,
            OutputFormat::Heic,
            request.limits,
            request.cancellation,
        )?;
        let mut failure = None;
        let mut output_bytes = 0;
        let mut codec_report = None;
        let publication = write_atomic_output_for_source(
            request.destination,
            request.source_identity,
            request.atomic,
            |file| {
                let result = encode_to_file(
                    file,
                    &prepared,
                    request.encode.quality,
                    request.limits.max_output_bytes,
                    request.cancellation,
                );
                match result {
                    Ok(report) => {
                        output_bytes = report.output_bytes;
                        codec_report = Some(report);
                        Ok(())
                    }
                    Err(error) => {
                        failure = Some(error);
                        Err(io::Error::other("HEIC encode callback failed"))
                    }
                }
            },
        );
        let outcome = publication_outcome(publication, failure)?;
        let codec_report = codec_report.ok_or(EncodeError::Encode)?;
        Ok(HeicEncodeReport {
            outcome,
            width: prepared.width,
            height: prepared.height,
            quality: request.encode.quality,
            output_bytes,
            bit_depth: codec_report.bit_depth,
            libheif_version: codec_report.libheif_version,
            encoder: codec_report.encoder,
            nclx: HeicNclx::SRGB_FULL_RANGE,
            icc: prepared.icc_provenance,
            clipped_samples: prepared.clipped_samples,
            alpha_flattened_pixels: prepared.alpha_flattened_pixels,
            metadata: prepared.metadata_report,
        })
    }

    pub fn probe() -> Result<HeicCapability, EncodeError> {
        unsafe {
            let _library = LibraryGuard::new()?;
            let version = CStr::from_ptr(heif::heif_get_version())
                .to_string_lossy()
                .into_owned();
            let context = Context::new()?;
            let encoder = Encoder::x265(context.0)?;
            let name = encoder.name();
            let eight_bit = probe_depth(8)?;
            let ten_bit = probe_depth(10)?;
            Ok(HeicCapability {
                libheif_version: version,
                encoder: name,
                eight_bit,
                ten_bit,
            })
        }
    }

    unsafe fn probe_depth(depth: u8) -> Result<bool, EncodeError> {
        const PROBE_ICC: &[u8] = b"grainroom-heic-capability-probe-v1";
        let context = unsafe { Context::new()? };
        let encoder = unsafe { Encoder::x265(context.0)? };
        check(unsafe { heif::heif_encoder_set_lossy_quality(encoder.0, 90) })?;
        let image = unsafe { Image::synthetic(depth) }.map_err(map_inner)?;
        unsafe { image.attach_profiles(PROBE_ICC) }.map_err(map_inner)?;
        let options = unsafe { EncodingOptions::new() }.map_err(map_inner)?;
        let mut encoded_handle = ptr::null_mut();
        check(unsafe {
            heif::heif_context_encode_image(
                context.0,
                image.0,
                encoder.0,
                options.0,
                &mut encoded_handle,
            )
        })?;
        if encoded_handle.is_null() {
            return Ok(false);
        }
        let _encoded_handle = Handle(encoded_handle);
        let mut bytes: Vec<u8> = Vec::new();
        let mut writer = heif::heif_writer {
            writer_api_version: 1,
            write: Some(vec_write_callback),
        };
        check(unsafe {
            heif::heif_context_write(context.0, &mut writer, (&mut bytes as *mut Vec<u8>).cast())
        })?;

        let reader = unsafe { Context::new()? };
        check(unsafe {
            heif::heif_context_read_from_memory_without_copy(
                reader.0,
                bytes.as_ptr().cast(),
                bytes.len(),
                ptr::null(),
            )
        })?;
        let mut handle = ptr::null_mut();
        check(unsafe { heif::heif_context_get_primary_image_handle(reader.0, &mut handle) })?;
        if handle.is_null() {
            return Ok(false);
        }
        let handle = Handle(handle);
        let bits = unsafe { heif::heif_image_handle_get_luma_bits_per_pixel(handle.0) };
        let icc_size = unsafe { heif::heif_image_handle_get_raw_color_profile_size(handle.0) };
        if icc_size != PROBE_ICC.len() {
            return Ok(false);
        }
        let mut icc = Vec::new();
        icc.try_reserve_exact(icc_size)
            .map_err(|_| EncodeError::Limit(LimitError::Allocation))?;
        icc.resize(icc_size, 0);
        check(unsafe {
            heif::heif_image_handle_get_raw_color_profile(handle.0, icc.as_mut_ptr().cast())
        })?;
        let mut nclx = ptr::null_mut();
        check(unsafe { heif::heif_image_handle_get_nclx_color_profile(handle.0, &mut nclx) })?;
        if nclx.is_null() {
            return Ok(false);
        }
        let nclx_ok = unsafe {
            (*nclx).color_primaries == 1
                && (*nclx).transfer_characteristics == 13
                && (*nclx).matrix_coefficients == 1
                && (*nclx).full_range_flag == 1
        };
        unsafe { heif::heif_nclx_color_profile_free(nclx) };
        let decoded_samples_ok = unsafe { decoded_probe_samples(handle.0, depth) };
        Ok(bits == i32::from(depth) && icc == PROBE_ICC && nclx_ok && decoded_samples_ok)
    }

    unsafe fn decoded_probe_samples(handle: *mut heif::heif_image_handle, depth: u8) -> bool {
        let chroma = if depth == 8 {
            heif::heif_chroma_heif_chroma_interleaved_RGB
        } else {
            heif::heif_chroma_heif_chroma_interleaved_RRGGBB_LE
        };
        let mut decoded = ptr::null_mut();
        let result = unsafe {
            heif::heif_decode_image(
                handle,
                &mut decoded,
                heif::heif_colorspace_heif_colorspace_RGB,
                chroma,
                ptr::null(),
            )
        };
        if result.code != heif::heif_error_code_heif_error_Ok || decoded.is_null() {
            return false;
        }
        let decoded = Image(decoded);
        let mut stride = 0usize;
        let plane = unsafe {
            heif::heif_image_get_plane_readonly2(
                decoded.0,
                heif::heif_channel_heif_channel_interleaved,
                &mut stride,
            )
        };
        let bytes_per_sample = if depth == 8 { 1 } else { 2 };
        let row_bytes = 3 * 3 * bytes_per_sample;
        if plane.is_null() || stride < row_bytes {
            return false;
        }
        let mut minimum = u16::MAX;
        let mut maximum = 0_u16;
        for y in 0..2usize {
            for sample in 0..9usize {
                let offset = y * stride + sample * bytes_per_sample;
                let value = if depth == 8 {
                    u16::from(unsafe { *plane.add(offset) })
                } else {
                    u16::from_le_bytes([unsafe { *plane.add(offset) }, unsafe {
                        *plane.add(offset + 1)
                    }])
                };
                minimum = minimum.min(value);
                maximum = maximum.max(value);
            }
        }
        maximum <= ((1_u16 << depth) - 1) && minimum < maximum
    }

    unsafe extern "C" fn vec_write_callback(
        _context: *mut heif::heif_context,
        data: *const c_void,
        size: usize,
        userdata: *mut c_void,
    ) -> heif::heif_error {
        if userdata.is_null() {
            return callback_error();
        }
        if size == 0 {
            return ok();
        }
        if data.is_null() {
            return callback_error();
        }
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };
        let output = unsafe { &mut *userdata.cast::<Vec<u8>>() };
        const MAX_PROBE_BYTES: usize = 1 << 20;
        if output
            .len()
            .checked_add(size)
            .is_none_or(|total| total > MAX_PROBE_BYTES)
        {
            return callback_error();
        }
        if output.try_reserve_exact(size).is_err() {
            return callback_error();
        }
        output.extend_from_slice(bytes);
        ok()
    }

    fn encode_to_file(
        file: &mut File,
        prepared: &super::super::PreparedDisplayRgb,
        quality: u8,
        maximum: u64,
        cancellation: &EncodeCancellation,
    ) -> Result<CodecReport, InnerFailure> {
        unsafe {
            if cancellation.cancelled() {
                return Err(InnerFailure::Cancelled);
            }
            let _library = LibraryGuard::new().map_err(InnerFailure::from)?;
            let libheif_version = CStr::from_ptr(heif::heif_get_version())
                .to_string_lossy()
                .into_owned();
            let context = Context::new().map_err(InnerFailure::from)?;
            let encoder = Encoder::x265(context.0).map_err(InnerFailure::from)?;
            check(heif::heif_encoder_set_lossy_quality(
                encoder.0,
                i32::from(quality),
            ))
            .map_err(InnerFailure::from)?;
            let image = Image::rgb10_from_rgb8(prepared.width, prepared.height, &prepared.rgb8)?;
            image.attach_profiles(&prepared.icc)?;
            let options = EncodingOptions::new()?;
            let mut handle = ptr::null_mut();
            check(heif::heif_context_encode_image(
                context.0,
                image.0,
                encoder.0,
                options.0,
                &mut handle,
            ))
            .map_err(InnerFailure::from)?;
            if handle.is_null() {
                return Err(InnerFailure::Codec);
            }
            let handle = Handle(handle);
            if let Some(exif) = &prepared.exif {
                let size = i32::try_from(exif.len())
                    .map_err(|_| InnerFailure::Limit(LimitError::ArithmeticOverflow))?;
                check(heif::heif_context_add_exif_metadata(
                    context.0,
                    handle.0,
                    exif.as_ptr().cast(),
                    size,
                ))
                .map_err(InnerFailure::from)?;
            }
            let mut state = WriterState {
                file,
                maximum,
                written: 0,
                attempted: 0,
                cancellation,
                failure: None,
            };
            let mut writer = heif::heif_writer {
                writer_api_version: 1,
                write: Some(write_callback),
            };
            let result = heif::heif_context_write(
                context.0,
                &mut writer,
                (&mut state as *mut WriterState<'_>).cast(),
            );
            if let Some(failure) = state.failure {
                return Err(failure);
            }
            check(result).map_err(InnerFailure::from)?;
            if cancellation.cancelled() {
                return Err(InnerFailure::Cancelled);
            }
            Ok(CodecReport {
                output_bytes: state.written,
                // The input plane and selected capability are fixed to 10 bit;
                // the public capability probe independently reopens a coded
                // result and verifies that depth survives the backend.
                bit_depth: 10,
                libheif_version,
                encoder: encoder.name(),
            })
        }
    }

    struct CodecReport {
        output_bytes: u64,
        bit_depth: u8,
        libheif_version: String,
        encoder: String,
    }

    unsafe extern "C" fn write_callback(
        _context: *mut heif::heif_context,
        data: *const c_void,
        size: usize,
        userdata: *mut c_void,
    ) -> heif::heif_error {
        // libheif's writer contract invokes this synchronously and does not
        // retain userdata. `WriterState` therefore remains live and uniquely
        // borrowed for the complete `heif_context_write` call.
        if userdata.is_null() {
            return callback_error();
        }
        let state = unsafe { &mut *userdata.cast::<WriterState<'_>>() };
        if state.failure.is_some() {
            return callback_error();
        }
        if state.cancellation.cancelled() {
            state.failure = Some(InnerFailure::Cancelled);
            return callback_error();
        }
        if size == 0 {
            return ok();
        }
        if data.is_null() {
            state.failure = Some(InnerFailure::Io);
            return callback_error();
        }
        let requested = state.written.saturating_add(size as u64);
        state.attempted = requested;
        if requested > state.maximum {
            state.failure = Some(InnerFailure::Limit(LimitError::OutputBytes {
                requested,
                maximum: state.maximum,
            }));
            return callback_error();
        }
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };
        if state.file.write_all(bytes).is_err() {
            state.failure = Some(InnerFailure::Io);
            return callback_error();
        }
        state.written = requested;
        ok()
    }

    struct WriterState<'a> {
        file: &'a mut File,
        maximum: u64,
        written: u64,
        attempted: u64,
        cancellation: &'a EncodeCancellation,
        failure: Option<InnerFailure>,
    }

    #[derive(Clone)]
    enum InnerFailure {
        Cancelled,
        Limit(LimitError),
        Codec,
        Backend,
        Io,
    }

    impl From<EncodeError> for InnerFailure {
        fn from(error: EncodeError) -> Self {
            match error {
                EncodeError::Limit(error) => Self::Limit(error),
                EncodeError::HeicBackendNotBuilt | EncodeError::HeicBackendUnavailable => {
                    Self::Backend
                }
                _ => Self::Codec,
            }
        }
    }

    fn map_atomic(error: AtomicOutputError, failure: Option<InnerFailure>) -> EncodeError {
        match (error, failure) {
            (AtomicOutputError::Write(_), Some(InnerFailure::Cancelled)) => EncodeError::Cancelled,
            (AtomicOutputError::Write(_), Some(InnerFailure::Limit(error))) => {
                EncodeError::Limit(error)
            }
            (AtomicOutputError::Write(_), Some(InnerFailure::Backend)) => {
                EncodeError::HeicBackendUnavailable
            }
            (AtomicOutputError::Write(_), Some(InnerFailure::Codec)) => EncodeError::Encode,
            (error, _) => EncodeError::Output(error),
        }
    }

    fn publication_outcome(
        result: Result<AtomicOutputOutcome, AtomicOutputError>,
        failure: Option<InnerFailure>,
    ) -> Result<AtomicOutputOutcome, EncodeError> {
        match result {
            Ok(outcome) => Ok(outcome),
            Err(AtomicOutputError::PublishedButNotDurable(_)) if failure.is_none() => {
                Ok(AtomicOutputOutcome::PublishedButNotDurable)
            }
            Err(error) => Err(map_atomic(error, failure)),
        }
    }

    struct Context(*mut heif::heif_context);

    static LIBHEIF_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    struct LibraryGuard {
        _lock: MutexGuard<'static, ()>,
    }
    impl LibraryGuard {
        unsafe fn new() -> Result<Self, EncodeError> {
            // Serialize init, every context operation, callbacks, and deinit.
            // This is deliberately stronger than libheif's optional build-time
            // multithreading contract and also protects plugin lifecycle state.
            let lock = LIBHEIF_LOCK
                .get_or_init(|| Mutex::new(()))
                .lock()
                .map_err(|_| EncodeError::HeicBackendUnavailable)?;
            check(unsafe { heif::heif_init(ptr::null_mut()) })?;
            Ok(Self { _lock: lock })
        }
    }
    impl Drop for LibraryGuard {
        fn drop(&mut self) {
            unsafe { heif::heif_deinit() }
        }
    }

    impl Context {
        unsafe fn new() -> Result<Self, EncodeError> {
            let value = unsafe { heif::heif_context_alloc() };
            (!value.is_null())
                .then_some(Self(value))
                .ok_or(EncodeError::Limit(LimitError::Allocation))
        }
    }
    impl Drop for Context {
        fn drop(&mut self) {
            unsafe { heif::heif_context_free(self.0) }
        }
    }

    struct Encoder(*mut heif::heif_encoder);
    impl Encoder {
        unsafe fn x265(context: *mut heif::heif_context) -> Result<Self, EncodeError> {
            let mut descriptor = ptr::null();
            let count = unsafe {
                heif::heif_get_encoder_descriptors(
                    heif::heif_compression_format_heif_compression_HEVC,
                    c"x265".as_ptr(),
                    &mut descriptor,
                    1,
                )
            };
            if count != 1 || descriptor.is_null() {
                return Err(EncodeError::HeicBackendUnavailable);
            }
            let mut encoder = ptr::null_mut();
            check(unsafe { heif::heif_context_get_encoder(context, descriptor, &mut encoder) })?;
            if encoder.is_null() {
                return Err(EncodeError::HeicBackendUnavailable);
            }
            Ok(Self(encoder))
        }
        unsafe fn name(&self) -> String {
            let name = unsafe { heif::heif_encoder_get_name(self.0) };
            if name.is_null() {
                "x265".into()
            } else {
                unsafe { CStr::from_ptr(name) }
                    .to_string_lossy()
                    .into_owned()
            }
        }
    }
    impl Drop for Encoder {
        fn drop(&mut self) {
            unsafe { heif::heif_encoder_release(self.0) }
        }
    }

    struct Image(*mut heif::heif_image);
    impl Image {
        unsafe fn synthetic(depth: u8) -> Result<Self, InnerFailure> {
            let chroma = if depth == 8 {
                heif::heif_chroma_heif_chroma_interleaved_RGB
            } else {
                heif::heif_chroma_heif_chroma_interleaved_RRGGBB_LE
            };
            let mut image = ptr::null_mut();
            check(unsafe {
                heif::heif_image_create(
                    3,
                    2,
                    heif::heif_colorspace_heif_colorspace_RGB,
                    chroma,
                    &mut image,
                )
            })
            .map_err(InnerFailure::from)?;
            let image = Self(image);
            check(unsafe {
                heif::heif_image_add_plane(
                    image.0,
                    heif::heif_channel_heif_channel_interleaved,
                    3,
                    2,
                    i32::from(depth),
                )
            })
            .map_err(InnerFailure::from)?;
            let mut stride = 0usize;
            let plane = unsafe {
                heif::heif_image_get_plane2(
                    image.0,
                    heif::heif_channel_heif_channel_interleaved,
                    &mut stride,
                )
            };
            let bytes_per_sample = if depth == 8 { 1 } else { 2 };
            let row = 3 * 3 * bytes_per_sample;
            if plane.is_null() || stride < row {
                return Err(InnerFailure::Codec);
            }
            for y in 0..2usize {
                for x in 0..3usize {
                    for channel in 0..3usize {
                        let value = if depth == 8 {
                            ((x + y + channel) * 37 % 256) as u16
                        } else {
                            ((x + y + channel) * 149 % 1024) as u16
                        };
                        let offset = y * stride + (x * 3 + channel) * bytes_per_sample;
                        if depth == 8 {
                            unsafe { *plane.add(offset) = value as u8 };
                        } else {
                            let encoded = value.to_le_bytes();
                            unsafe {
                                *plane.add(offset) = encoded[0];
                                *plane.add(offset + 1) = encoded[1];
                            }
                        }
                    }
                }
            }
            Ok(image)
        }
        unsafe fn rgb10_from_rgb8(
            width: u32,
            height: u32,
            rgb: &[u8],
        ) -> Result<Self, InnerFailure> {
            let w = i32::try_from(width).map_err(|_| InnerFailure::Codec)?;
            let h = i32::try_from(height).map_err(|_| InnerFailure::Codec)?;
            let mut image = ptr::null_mut();
            check(unsafe {
                heif::heif_image_create(
                    w,
                    h,
                    heif::heif_colorspace_heif_colorspace_RGB,
                    heif::heif_chroma_heif_chroma_interleaved_RRGGBB_LE,
                    &mut image,
                )
            })
            .map_err(InnerFailure::from)?;
            let image = Self(image);
            check(unsafe {
                heif::heif_image_add_plane(
                    image.0,
                    heif::heif_channel_heif_channel_interleaved,
                    w,
                    h,
                    10,
                )
            })
            .map_err(InnerFailure::from)?;
            let mut stride = 0usize;
            let plane = unsafe {
                heif::heif_image_get_plane2(
                    image.0,
                    heif::heif_channel_heif_channel_interleaved,
                    &mut stride,
                )
            };
            let input_row = usize::try_from(width)
                .ok()
                .and_then(|v| v.checked_mul(3))
                .ok_or(InnerFailure::Codec)?;
            let output_row = input_row.checked_mul(2).ok_or(InnerFailure::Codec)?;
            if plane.is_null()
                || stride < output_row
                || rgb.len()
                    != input_row
                        .checked_mul(height as usize)
                        .ok_or(InnerFailure::Codec)?
            {
                return Err(InnerFailure::Codec);
            }
            for y in 0..height as usize {
                for (sample, value) in rgb[y * input_row..(y + 1) * input_row]
                    .iter()
                    .copied()
                    .enumerate()
                {
                    let value = ((u32::from(value) * 1023 + 127) / 255) as u16;
                    let value = value.to_le_bytes();
                    let offset = y * stride + sample * 2;
                    unsafe {
                        *plane.add(offset) = value[0];
                        *plane.add(offset + 1) = value[1];
                    }
                }
            }
            Ok(image)
        }
        unsafe fn attach_profiles(&self, icc: &[u8]) -> Result<(), InnerFailure> {
            check(unsafe {
                heif::heif_image_set_raw_color_profile(
                    self.0,
                    c"prof".as_ptr(),
                    icc.as_ptr().cast(),
                    icc.len(),
                )
            })
            .map_err(InnerFailure::from)?;
            let profile = unsafe { heif::heif_nclx_color_profile_alloc() };
            if profile.is_null() {
                return Err(InnerFailure::Limit(LimitError::Allocation));
            }
            unsafe {
                (*profile).color_primaries = 1;
                (*profile).transfer_characteristics = 13;
                (*profile).matrix_coefficients = 1;
                (*profile).full_range_flag = 1;
            }
            let result = check(unsafe { heif::heif_image_set_nclx_color_profile(self.0, profile) });
            unsafe { heif::heif_nclx_color_profile_free(profile) };
            result.map_err(InnerFailure::from)
        }
    }
    impl Drop for Image {
        fn drop(&mut self) {
            unsafe { heif::heif_image_release(self.0) }
        }
    }

    struct Handle(*mut heif::heif_image_handle);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe { heif::heif_image_handle_release(self.0) }
        }
    }
    struct EncodingOptions(*mut heif::heif_encoding_options);
    impl EncodingOptions {
        unsafe fn new() -> Result<Self, InnerFailure> {
            let value = unsafe { heif::heif_encoding_options_alloc() };
            if value.is_null() {
                return Err(InnerFailure::Limit(LimitError::Allocation));
            }
            unsafe {
                (*value).save_two_colr_boxes_when_ICC_and_nclx_available = 1;
            }
            Ok(Self(value))
        }
    }
    impl Drop for EncodingOptions {
        fn drop(&mut self) {
            unsafe { heif::heif_encoding_options_free(self.0) }
        }
    }

    fn check(error: heif::heif_error) -> Result<(), EncodeError> {
        if error.code == heif::heif_error_code_heif_error_Ok {
            Ok(())
        } else if error.code == heif::heif_error_code_heif_error_Memory_allocation_error {
            Err(EncodeError::Limit(LimitError::Allocation))
        } else {
            Err(EncodeError::Encode)
        }
    }
    fn map_inner(error: InnerFailure) -> EncodeError {
        match error {
            InnerFailure::Cancelled => EncodeError::Cancelled,
            InnerFailure::Limit(error) => EncodeError::Limit(error),
            InnerFailure::Backend => EncodeError::HeicBackendUnavailable,
            InnerFailure::Codec | InnerFailure::Io => EncodeError::Encode,
        }
    }
    fn ok() -> heif::heif_error {
        heif::heif_error {
            code: 0,
            subcode: 0,
            message: ptr::null(),
        }
    }
    fn callback_error() -> heif::heif_error {
        heif::heif_error {
            code: heif::heif_error_code_heif_error_Encoding_error,
            subcode: heif::heif_suberror_code_heif_suberror_Cannot_write_output_data,
            message: c"HEIC writer failed".as_ptr(),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn callbacks_accept_null_with_zero_size_without_forming_a_slice() {
            let mut output = Vec::<u8>::new();
            let vector_result = unsafe {
                vec_write_callback(
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                    (&mut output as *mut Vec<u8>).cast(),
                )
            };
            assert_eq!(vector_result.code, heif::heif_error_code_heif_error_Ok);

            let directory = tempfile::tempdir().unwrap();
            let mut file = File::create(directory.path().join("empty.bin")).unwrap();
            let cancellation = EncodeCancellation::default();
            let mut state = WriterState {
                file: &mut file,
                maximum: 0,
                written: 0,
                attempted: 0,
                cancellation: &cancellation,
                failure: None,
            };
            let file_result = unsafe {
                write_callback(
                    ptr::null_mut(),
                    ptr::null(),
                    0,
                    (&mut state as *mut WriterState<'_>).cast(),
                )
            };
            assert_eq!(file_result.code, heif::heif_error_code_heif_error_Ok);
            assert_eq!(state.written, 0);
            assert!(state.failure.is_none());
        }

        #[test]
        fn visible_but_not_durable_publication_is_a_non_retryable_success() {
            let result = publication_outcome(
                Err(AtomicOutputError::PublishedButNotDurable(io::Error::other(
                    "fault injection",
                ))),
                None,
            );
            assert_eq!(result.unwrap(), AtomicOutputOutcome::PublishedButNotDurable);
        }

        #[test]
        fn writer_callback_appends_chunks_and_rejects_the_first_byte_over_limit() {
            let directory = tempfile::tempdir().unwrap();
            let path = directory.path().join("chunks.bin");
            let mut file = File::create(&path).unwrap();
            let cancellation = EncodeCancellation::default();
            let mut state = WriterState {
                file: &mut file,
                maximum: 5,
                written: 0,
                attempted: 0,
                cancellation: &cancellation,
                failure: None,
            };
            for chunk in [&b"ab"[..], &b"cde"[..]] {
                let result = unsafe {
                    write_callback(
                        ptr::null_mut(),
                        chunk.as_ptr().cast(),
                        chunk.len(),
                        (&mut state as *mut WriterState<'_>).cast(),
                    )
                };
                assert_eq!(result.code, heif::heif_error_code_heif_error_Ok);
            }
            let result = unsafe {
                write_callback(
                    ptr::null_mut(),
                    b"f".as_ptr().cast(),
                    1,
                    (&mut state as *mut WriterState<'_>).cast(),
                )
            };
            assert_ne!(result.code, heif::heif_error_code_heif_error_Ok);
            assert_eq!(std::fs::read(path).unwrap(), b"abcde");
        }
    }
}

#[cfg(feature = "heic")]
pub fn encode_heic(request: HeicEncodeRequest<'_>) -> Result<HeicEncodeReport, EncodeError> {
    backend::encode(request)
}

#[cfg(feature = "heic")]
pub fn probe_heic_capability() -> Result<HeicCapability, EncodeError> {
    backend::probe()
}
