//! Bounded ICC color management based on the safe `lcms2` Rust wrapper.
//!
//! Profiles and transforms are created per operation. There is no global
//! mutable profile or transform cache in Grainroom.

mod error;
mod png;
mod profile;
mod transform;

pub use error::{ColorError, RasterChannel};
pub use png::{
    Chromaticity, PngChromaticities, PngColorDeclarations, PngProfileKind, SynthesizedPngProfile,
    synthesize_png_profile,
};
pub use profile::{
    ResolvedInputProfile, RgbProfile, assumed_srgb_profile, embedded_rgb_profile, lcms_version,
    linear_rec2020_profile, srgb_profile,
};
pub use transform::{
    ColorTransformReport, ColorWorkingSetEstimate, ColorWorkingSetProfile,
    RasterToWorkingTransform, WorkingToSrgbTransform, estimate_color_working_set,
};
