use std::{path::Path, sync::LazyLock};

#[cfg(feature = "heic")]
use std::fs;

#[cfg(feature = "heic")]
use sha2::{Digest, Sha256};

use omalux::{
    develop::{CpuImage, RgbaPixel},
    io::{
        AtomicOutputOptions, EncodeCancellation, EncodeError, EncodeOptions,
        EncodeWorkingSetProfile, HeicEncodeRequest, JpegEncodeInput, JpegMetadataFootprint,
        LimitError, MetadataBundle, OutputFormat, ResourceLimits, SignalRelation, encode_heic,
        probe_heic_capability,
    },
};

fn image(width: u32, height: u32) -> CpuImage {
    let count = (u64::from(width) * u64::from(height)) as usize;
    CpuImage::new(
        width,
        height,
        (0..count)
            .map(|index| {
                let value = (index % 17) as f32 / 16.0;
                RgbaPixel::new(value, 0.3, 1.0 - value, 1.0).unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn request<'a>(
    image: &'a CpuImage,
    output: &'a Path,
    limits: &'a ResourceLimits,
    cancellation: &'a EncodeCancellation,
) -> HeicEncodeRequest<'a> {
    static METADATA: LazyLock<MetadataBundle> = LazyLock::new(MetadataBundle::default);
    let mut encode = EncodeOptions::default();
    encode.format = OutputFormat::Heic;
    HeicEncodeRequest {
        input: JpegEncodeInput {
            image,
            signal_relation: SignalRelation::LinearizedDisplayReferred,
            metadata: &METADATA,
        },
        destination: output,
        source_identity: None,
        encode,
        atomic: AtomicOutputOptions::default(),
        limits,
        cancellation,
    }
}

#[cfg(feature = "heic")]
fn request_with_metadata<'a>(
    image: &'a CpuImage,
    metadata: &'a MetadataBundle,
    output: &'a Path,
    limits: &'a ResourceLimits,
    cancellation: &'a EncodeCancellation,
) -> HeicEncodeRequest<'a> {
    let mut request = request(image, output, limits, cancellation);
    request.input.metadata = metadata;
    request
}

#[cfg(feature = "heic")]
#[test]
fn x265_capability_and_odd_sized_atomic_encode_work() {
    let capability = probe_heic_capability().unwrap();
    assert!(capability.encoder.to_ascii_lowercase().contains("x265"));
    assert!(capability.eight_bit && capability.ten_bit);

    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("odd.heic");
    let report = encode_heic(request(
        &image(17, 9),
        &output,
        &ResourceLimits::default(),
        &EncodeCancellation::default(),
    ))
    .unwrap();
    let bytes = fs::read(output).unwrap();
    assert_eq!(report.output_bytes, bytes.len() as u64);
    assert!(bytes.windows(4).any(|window| window == b"ftyp"));
}

#[cfg(feature = "heic")]
#[test]
fn production_file_reads_back_pixels_icc_nclx_and_sanitized_exif() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("readback.heic");
    let source = image(7, 5);
    let metadata = MetadataBundle::try_new(
        Some(iso_exif()),
        Some(b"PRIVATE-XMP".to_vec()),
        None,
        true,
        &ResourceLimits::default(),
    )
    .unwrap();
    let limits = ResourceLimits::default();
    let cancellation = EncodeCancellation::default();
    let report = encode_heic(request_with_metadata(
        &source,
        &metadata,
        &output,
        &limits,
        &cancellation,
    ))
    .unwrap();
    assert_eq!(report.bit_depth, 10);
    assert!(!report.libheif_version.is_empty());
    assert_eq!(report.nclx, omalux::io::HeicNclx::SRGB_FULL_RANGE);
    assert!(report.metadata.gps_removed);
    let bytes = fs::read(output).unwrap();
    unsafe { assert_libheif_readback(&bytes, &report) };
}

#[cfg(feature = "heic")]
unsafe fn assert_libheif_readback(bytes: &[u8], report: &omalux::io::HeicEncodeReport) {
    use libheif_sys as heif;
    use std::ptr;

    let context = unsafe { heif::heif_context_alloc() };
    assert!(!context.is_null());
    let read = unsafe {
        heif::heif_context_read_from_memory_without_copy(
            context,
            bytes.as_ptr().cast(),
            bytes.len(),
            ptr::null(),
        )
    };
    assert_eq!(read.code, heif::heif_error_code_heif_error_Ok);
    let mut handle = ptr::null_mut();
    let primary = unsafe { heif::heif_context_get_primary_image_handle(context, &mut handle) };
    assert_eq!(primary.code, heif::heif_error_code_heif_error_Ok);
    assert_eq!(unsafe { heif::heif_image_handle_get_width(handle) }, 7);
    assert_eq!(unsafe { heif::heif_image_handle_get_height(handle) }, 5);

    let icc_size = unsafe { heif::heif_image_handle_get_raw_color_profile_size(handle) };
    let mut icc = vec![0_u8; icc_size];
    let profile =
        unsafe { heif::heif_image_handle_get_raw_color_profile(handle, icc.as_mut_ptr().cast()) };
    assert_eq!(profile.code, heif::heif_error_code_heif_error_Ok);
    assert_eq!(<[u8; 32]>::from(Sha256::digest(&icc)), report.icc.sha256);

    let mut nclx = ptr::null_mut();
    let color = unsafe { heif::heif_image_handle_get_nclx_color_profile(handle, &mut nclx) };
    assert_eq!(color.code, heif::heif_error_code_heif_error_Ok);
    assert_eq!(unsafe { (*nclx).color_primaries }, 1);
    assert_eq!(unsafe { (*nclx).transfer_characteristics }, 13);
    assert_eq!(unsafe { (*nclx).matrix_coefficients }, 1);
    assert_eq!(unsafe { (*nclx).full_range_flag }, 1);
    unsafe { heif::heif_nclx_color_profile_free(nclx) };

    assert_eq!(
        unsafe { heif::heif_image_handle_get_number_of_metadata_blocks(handle, c"Exif".as_ptr()) },
        1
    );
    let mut metadata_id = 0;
    assert_eq!(
        unsafe {
            heif::heif_image_handle_get_list_of_metadata_block_IDs(
                handle,
                c"Exif".as_ptr(),
                &mut metadata_id,
                1,
            )
        },
        1
    );
    let exif_size = unsafe { heif::heif_image_handle_get_metadata_size(handle, metadata_id) };
    let mut exif = vec![0_u8; exif_size];
    let exif_result = unsafe {
        heif::heif_image_handle_get_metadata(handle, metadata_id, exif.as_mut_ptr().cast())
    };
    assert_eq!(exif_result.code, heif::heif_error_code_heif_error_Ok);
    assert!(
        exif.windows(2)
            .any(|window| window == 400_u16.to_le_bytes())
    );
    assert!(!exif.windows(4).any(|window| window == b"PRIV"));

    let mut decoded = ptr::null_mut();
    let decode = unsafe {
        heif::heif_decode_image(
            handle,
            &mut decoded,
            heif::heif_colorspace_heif_colorspace_RGB,
            heif::heif_chroma_heif_chroma_interleaved_RGB,
            ptr::null(),
        )
    };
    assert_eq!(decode.code, heif::heif_error_code_heif_error_Ok);
    let mut stride = 0usize;
    let plane = unsafe {
        heif::heif_image_get_plane_readonly2(
            decoded,
            heif::heif_channel_heif_channel_interleaved,
            &mut stride,
        )
    };
    assert!(!plane.is_null());
    let first = unsafe { std::slice::from_raw_parts(plane, 7 * 3) };
    assert!(first.windows(2).any(|samples| samples[0] != samples[1]));
    unsafe {
        heif::heif_image_release(decoded);
        heif::heif_image_handle_release(handle);
        heif::heif_context_free(context);
    }
}

#[cfg(feature = "heic")]
fn iso_exif() -> Vec<u8> {
    let mut bytes = vec![0_u8; 94];
    bytes[..8].copy_from_slice(b"II*\0\x08\0\0\0");
    bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
    for (index, (tag, kind, count, value)) in [
        (0x0112_u16, 3_u16, 1_u32, 1_u32),
        (0x8825, 4, 1, 90),
        (0x8769, 4, 1, 50),
    ]
    .into_iter()
    .enumerate()
    {
        let at = 10 + index * 12;
        bytes[at..at + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        bytes[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
    }
    bytes[50..52].copy_from_slice(&2_u16.to_le_bytes());
    for (index, (tag, kind, count, value)) in [
        (0x8827_u16, 3_u16, 1_u32, 400_u32),
        (0x9286, 7, 4, u32::from_le_bytes(*b"PRIV")),
    ]
    .into_iter()
    .enumerate()
    {
        let at = 52 + index * 12;
        bytes[at..at + 2].copy_from_slice(&tag.to_le_bytes());
        bytes[at + 2..at + 4].copy_from_slice(&kind.to_le_bytes());
        bytes[at + 4..at + 8].copy_from_slice(&count.to_le_bytes());
        bytes[at + 8..at + 12].copy_from_slice(&value.to_le_bytes());
    }
    bytes
}

#[cfg(feature = "heic")]
#[test]
fn parallel_probe_and_encode_are_process_lifecycle_safe() {
    let workers = (0..3)
        .map(|index| {
            std::thread::spawn(move || {
                let capability = probe_heic_capability().unwrap();
                assert!(capability.eight_bit && capability.ten_bit);
                let directory = tempfile::tempdir().unwrap();
                let output = directory.path().join(format!("parallel-{index}.heic"));
                let source = image(5 + index, 3 + index);
                let limits = ResourceLimits::default();
                let cancellation = EncodeCancellation::default();
                encode_heic(request(&source, &output, &limits, &cancellation)).unwrap();
                assert!(output.is_file());
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
}

#[cfg(feature = "heic")]
#[test]
fn raw_cancel_and_output_limits_are_transactional() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("bounded.heic");
    let source = image(32, 32);
    let limits = ResourceLimits::default().with_max_output_bytes(64);
    assert!(matches!(
        encode_heic(request(
            &source,
            &output,
            &limits,
            &EncodeCancellation::default()
        )),
        Err(EncodeError::Limit(LimitError::OutputBytes { .. }))
    ));
    assert!(!output.exists());

    let default_limits = ResourceLimits::default();
    let default_cancellation = EncodeCancellation::default();
    let mut raw = request(&source, &output, &default_limits, &default_cancellation);
    raw.input.signal_relation = SignalRelation::SceneRelatedRaw;
    assert!(matches!(
        encode_heic(raw),
        Err(EncodeError::SceneToDisplayRenderingRequired)
    ));
    assert!(!output.exists());
}

#[test]
fn native_working_allowance_accepts_exact_peak_and_rejects_peak_minus_one() {
    let metadata = JpegMetadataFootprint {
        input_metadata_bytes: 0,
        output_exif_bytes: 0,
        output_icc_bytes: 588,
        transform_profile_bytes: 1156,
    };
    let estimate = ResourceLimits::default()
        .estimate_encode_working_set(17, 9, EncodeWorkingSetProfile::HeicRgb8X265, metadata)
        .unwrap();
    let exact = ResourceLimits::default().with_max_working_bytes(estimate.peak_bytes);
    assert_eq!(
        exact
            .estimate_encode_working_set(17, 9, EncodeWorkingSetProfile::HeicRgb8X265, metadata)
            .unwrap()
            .peak_bytes,
        estimate.peak_bytes
    );
    let below = ResourceLimits::default().with_max_working_bytes(estimate.peak_bytes - 1);
    assert!(matches!(
        below.estimate_encode_working_set(17, 9, EncodeWorkingSetProfile::HeicRgb8X265, metadata),
        Err(LimitError::WorkingBytes { .. })
    ));
}

#[cfg(not(feature = "heic"))]
#[test]
fn disabled_feature_fails_without_touching_output() {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("disabled.heic");
    assert!(matches!(
        encode_heic(request(
            &image(1, 1),
            &output,
            &ResourceLimits::default(),
            &EncodeCancellation::default()
        )),
        Err(EncodeError::HeicBackendNotBuilt)
    ));
    assert!(!output.exists());
    assert!(matches!(
        probe_heic_capability(),
        Err(EncodeError::HeicBackendNotBuilt)
    ));
}
