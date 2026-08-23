//! Full-resolution RAW Phase-A decoding through the installed LibRaw
//! `dcraw_emu` compatibility executable.

mod ppm;
mod process;
mod stage;

use crate::io::{
    ColorProvenance, DecodeError, DecodeOptions, DecodedPhoto, Diagnostic, DiagnosticCode,
    DiagnosticSeverity, MetadataBundle, RawBackendName, RawMatrixSource, RawProcessingProvenance,
    SignalRelation, WhiteBalancePolicy, WhiteBalanceProvenance,
};
pub use process::{RawCancellation, RawCapability, RawExecutionOptions, probe_dcraw_emu};
use std::path::Path;

/// Stages one immutable source byte stream, runs full-resolution dcraw_emu,
/// and returns scene-linear Rec.2020 pixels. No ICC accuracy is claimed.
pub fn decode_raw(
    source: impl AsRef<Path>,
    options: &DecodeOptions,
    execution: &RawExecutionOptions,
    cancellation: &RawCancellation,
) -> Result<DecodedPhoto, DecodeError> {
    options.validate()?;
    execution.validate()?;
    if !options.raw.apply_orientation {
        return Err(DecodeError::InvalidOptions);
    }
    let staged = stage::stage_source(
        source.as_ref(),
        &execution.staging_directory,
        &options.limits,
        cancellation.flag(),
    )?;
    let bytes = process::run_dcraw(
        staged.path(),
        options.raw.white_balance,
        &options.limits,
        execution,
        cancellation,
    )?;
    let image = ppm::parse_ppm16(&bytes, &options.limits)?;
    let (white_balance, mut diagnostics) = match options.raw.white_balance {
        WhiteBalancePolicy::CameraThenDaylight => (
            WhiteBalanceProvenance::Unknown,
            vec![Diagnostic {
                severity: DiagnosticSeverity::Warning,
                code: DiagnosticCode::CameraWhiteBalanceFallbackUnknown,
            }],
        ),
        WhiteBalancePolicy::Daylight => (WhiteBalanceProvenance::DaylightFallback, Vec::new()),
        WhiteBalancePolicy::Explicit(_) => (WhiteBalanceProvenance::Explicit, Vec::new()),
    };
    diagnostics.push(Diagnostic {
        severity: DiagnosticSeverity::Information,
        code: DiagnosticCode::BackendVersionUnavailable,
    });
    diagnostics.push(Diagnostic {
        severity: DiagnosticSeverity::Warning,
        code: DiagnosticCode::UnknownRawMatrix,
    });
    Ok(DecodedPhoto {
        image,
        metadata: MetadataBundle::try_new(None, None, None, true, &options.limits)
            .map_err(DecodeError::Limit)?,
        source_digest: staged.digest,
        color: ColorProvenance::RawMatrix {
            matrix: RawMatrixSource::Unknown,
            white_balance,
            processing: RawProcessingProvenance {
                backend: RawBackendName::LibRawDcrawEmu,
                backend_version: None,
                full_resolution: true,
                linear_16_bit: true,
                output_rec2020: true,
                embedded_matrix_enabled: true,
                ahd_demosaic: true,
            },
        },
        signal_relation: SignalRelation::SceneReferred,
        diagnostics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{fs, path::PathBuf, time::Duration};
    use tempfile::tempdir;
    #[cfg(unix)]
    fn fake(directory: &Path) -> PathBuf {
        let path = directory.join("dcraw_emu");
        fs::write(
            &path,
            "#!/bin/sh\nprintf 'P6\\n1 1\\n65535\\n\\377\\377\\100\\000\\000\\000'\n",
        )
        .unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        path
    }
    #[cfg(unix)]
    #[test]
    fn end_to_end_is_full_resolution_oriented_once_and_cleans_stage() {
        let d = tempdir().unwrap();
        let source = d.path().join("source;touch SHOULD_NOT_EXIST.NEF");
        fs::write(&source, b"synthetic raw bytes").unwrap();
        let stage = d.path().join("stage");
        fs::create_dir(&stage).unwrap();
        let mut execution = RawExecutionOptions::new(fake(d.path())).unwrap();
        execution.staging_directory = stage.clone();
        execution.timeout = Duration::from_secs(1);
        let photo = decode_raw(
            &source,
            &DecodeOptions::default(),
            &execution,
            &RawCancellation::default(),
        )
        .unwrap();
        assert_eq!((photo.image.width(), photo.image.height()), (1, 1));
        assert!(photo.metadata.orientation_consumed());
        assert_eq!(fs::read_dir(stage).unwrap().count(), 0);
        assert!(!d.path().join("SHOULD_NOT_EXIST").exists());
        let digest = photo.source_digest;
        fs::rename(&source, d.path().join("renamed.nef")).unwrap();
        let photo2 = decode_raw(
            d.path().join("renamed.nef"),
            &DecodeOptions::default(),
            &execution,
            &RawCancellation::default(),
        )
        .unwrap();
        assert_eq!(digest, photo2.source_digest);
    }
    #[cfg(unix)]
    #[test]
    fn corrupt_backend_payload_still_cleans_staged_source() {
        let d = tempdir().unwrap();
        let source = d.path().join("source.raw");
        fs::write(&source, b"bytes").unwrap();
        let executable = d.path().join("bad-dcraw");
        fs::write(&executable, "#!/bin/sh\nprintf 'P6 1 1 65535\\n\\000'\n").unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        let stage = d.path().join("stage");
        fs::create_dir(&stage).unwrap();
        let mut execution = RawExecutionOptions::new(executable).unwrap();
        execution.staging_directory = stage.clone();
        assert!(matches!(
            decode_raw(
                &source,
                &DecodeOptions::default(),
                &execution,
                &RawCancellation::default()
            ),
            Err(DecodeError::CorruptInput)
        ));
        assert_eq!(fs::read_dir(stage).unwrap().count(), 0);
    }
    #[test]
    fn real_backend_probe_is_explicit() {
        match probe_dcraw_emu() {
            RawCapability::Available { executable } => assert!(executable.is_absolute()),
            RawCapability::Unavailable => {}
        }
    }
}
