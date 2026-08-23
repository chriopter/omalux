use lcms2::{DisallowCache, Flags, GlobalContext, Intent, PixelFormat, Transform};

use super::{ColorError, RasterChannel, RgbProfile, linear_rec2020_profile, srgb_profile};
use crate::{
    develop::RgbaPixel,
    io::{LimitError, ResourceLimits, SdrRangePolicy, SignalRelation},
};

type RgbaFloatTransform = Transform<[f32; 4], [f32; 4], GlobalContext, DisallowCache>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorWorkingSetProfile {
    RasterToWorking,
    WorkingToRaster,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorWorkingSetEstimate {
    pub pixels: u64,
    pub serialized_profile_bytes: u64,
    pub scratch_bytes: u64,
    pub accounted_bytes: u64,
}

/// Computes the color-stage scratch requirement before allocation.
pub fn estimate_color_working_set(
    pixels: usize,
    profile: ColorWorkingSetProfile,
    serialized_profile_bytes: u64,
    limits: &ResourceLimits,
) -> Result<ColorWorkingSetEstimate, ColorError> {
    limits.validate()?;
    let pixels_u64 = u64::try_from(pixels).map_err(|_| LimitError::ArithmeticOverflow)?;
    let maximum_pixels = limits.max_pixels.min(u64::from(u32::MAX));
    if pixels_u64 > maximum_pixels {
        return Err(LimitError::PixelCount {
            requested: pixels_u64,
            maximum: maximum_pixels,
        }
        .into());
    }
    let scratch_buffers = match profile {
        ColorWorkingSetProfile::RasterToWorking => 1_u64,
        ColorWorkingSetProfile::WorkingToRaster => 2_u64,
    };
    let scratch_bytes = pixels_u64
        .checked_mul(16)
        .and_then(|bytes| bytes.checked_mul(scratch_buffers))
        .ok_or(LimitError::ArithmeticOverflow)?;
    let accounted_bytes = serialized_profile_bytes
        .checked_add(scratch_bytes)
        .ok_or(LimitError::ArithmeticOverflow)?;
    if accounted_bytes > limits.max_working_bytes {
        return Err(LimitError::WorkingBytes {
            requested: accounted_bytes,
            maximum: limits.max_working_bytes,
        }
        .into());
    }
    Ok(ColorWorkingSetEstimate {
        pixels: pixels_u64,
        serialized_profile_bytes,
        scratch_bytes,
        accounted_bytes,
    })
}

/// Converts normalized, encoded straight-alpha raster samples to Grainroom's
/// linearized display-referred Rec.2020/D65 working space.
pub struct RasterToWorkingTransform {
    transform: RgbaFloatTransform,
    serialized_profile_bytes: u64,
}

impl RasterToWorkingTransform {
    pub fn new(source: &RgbProfile, limits: &ResourceLimits) -> Result<Self, ColorError> {
        let working = linear_rec2020_profile(limits)?;
        let serialized_profile_bytes = source
            .icc_provenance()
            .bytes
            .checked_add(working.icc_provenance().bytes)
            .ok_or(LimitError::ArithmeticOverflow)?;
        estimate_color_working_set(
            0,
            ColorWorkingSetProfile::RasterToWorking,
            serialized_profile_bytes,
            limits,
        )?;
        Ok(Self {
            transform: new_transform(source, &working).map_err(|error| match error {
                ColorError::TransformCreation => ColorError::UnsupportedProfile,
                other => other,
            })?,
            serialized_profile_bytes,
        })
    }

    /// Transforms one caller-bounded scanline transactionally.
    ///
    /// Encoded raster RGB and straight alpha must be finite in `[0, 1]`.
    /// Negative/HDR raster samples are rejected rather than silently clipped.
    pub fn transform_scanline(
        &self,
        source: &[[f32; 4]],
        destination: &mut [RgbaPixel],
        limits: &ResourceLimits,
    ) -> Result<ColorTransformReport, ColorError> {
        if source.len() != destination.len() {
            return Err(ColorError::LengthMismatch {
                source: source.len(),
                destination: destination.len(),
            });
        }
        estimate_color_working_set(
            source.len(),
            ColorWorkingSetProfile::RasterToWorking,
            self.serialized_profile_bytes,
            limits,
        )?;
        validate_encoded_raster(source)?;

        let mut scratch = Vec::new();
        scratch
            .try_reserve_exact(source.len())
            .map_err(|_| ColorError::Allocation)?;
        scratch.resize(source.len(), [0.0; 4]);
        self.transform.transform_pixels(source, &mut scratch);

        for (index, (output, input)) in scratch.iter().zip(source).enumerate() {
            validate_finite_rgb(index, output)?;
            debug_assert!(input[3].is_finite() && (0.0..=1.0).contains(&input[3]));
        }
        for ((destination, output), input) in destination.iter_mut().zip(scratch).zip(source) {
            *destination = RgbaPixel::new(output[0], output[1], output[2], input[3])
                .expect("validated LCMS output and encoded alpha");
        }
        Ok(ColorTransformReport::new(
            0,
            SignalRelation::LinearizedDisplayReferred,
        ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ColorTransformReport {
    pub clipped_samples: u64,
    pub lcms_version: u32,
    /// Semantic relation of the linear Rec.2020 working buffer.
    ///
    /// For an output transform this describes its input, not the encoded sRGB
    /// pixels written by the transform.
    pub working_signal_relation: SignalRelation,
}

impl ColorTransformReport {
    fn new(clipped_samples: u64, working_signal_relation: SignalRelation) -> Self {
        Self {
            clipped_samples,
            lcms_version: lcms2::version(),
            working_signal_relation,
        }
    }
}

/// Converts working-space pixels into the generated standard sRGB profile.
pub struct WorkingToSrgbTransform {
    transform: RgbaFloatTransform,
    serialized_profile_bytes: u64,
}

impl WorkingToSrgbTransform {
    pub fn new(limits: &ResourceLimits) -> Result<Self, ColorError> {
        let working = linear_rec2020_profile(limits)?;
        let output = srgb_profile(limits)?;
        let serialized_profile_bytes = working
            .icc_provenance()
            .bytes
            .checked_add(output.icc_provenance().bytes)
            .ok_or(LimitError::ArithmeticOverflow)?;
        estimate_color_working_set(
            0,
            ColorWorkingSetProfile::WorkingToRaster,
            serialized_profile_bytes,
            limits,
        )?;
        Ok(Self {
            transform: new_transform(&working, &output)?,
            serialized_profile_bytes,
        })
    }

    /// Converts a scanline transactionally with an explicit SDR range policy.
    pub fn transform_scanline(
        &self,
        source: &[RgbaPixel],
        destination: &mut [[f32; 4]],
        input_relation: SignalRelation,
        range: SdrRangePolicy,
        limits: &ResourceLimits,
    ) -> Result<ColorTransformReport, ColorError> {
        if input_relation == SignalRelation::SceneRelatedRaw {
            return Err(ColorError::SceneToDisplayRenderingRequired);
        }
        if source.len() != destination.len() {
            return Err(ColorError::LengthMismatch {
                source: source.len(),
                destination: destination.len(),
            });
        }
        estimate_color_working_set(
            source.len(),
            ColorWorkingSetProfile::WorkingToRaster,
            self.serialized_profile_bytes,
            limits,
        )?;
        let mut input = Vec::new();
        input
            .try_reserve_exact(source.len())
            .map_err(|_| ColorError::Allocation)?;
        input.extend(
            source
                .iter()
                .map(|pixel| [pixel.red(), pixel.green(), pixel.blue(), pixel.alpha()]),
        );
        let mut output = Vec::new();
        output
            .try_reserve_exact(source.len())
            .map_err(|_| ColorError::Allocation)?;
        output.resize(source.len(), [0.0; 4]);
        self.transform.transform_pixels(&input, &mut output);

        let mut clipped_samples = 0_u64;
        for (pixel_index, (pixel, source_pixel)) in output.iter_mut().zip(source).enumerate() {
            validate_finite_rgb(pixel_index, pixel)?;
            for (channel_index, channel) in [
                RasterChannel::Red,
                RasterChannel::Green,
                RasterChannel::Blue,
            ]
            .into_iter()
            .enumerate()
            {
                if !(0.0..=1.0).contains(&pixel[channel_index]) {
                    match range {
                        SdrRangePolicy::Reject => {
                            return Err(ColorError::OutputOutOfRange {
                                pixel: pixel_index,
                                channel,
                            });
                        }
                        SdrRangePolicy::ClipAndReport => {
                            pixel[channel_index] = pixel[channel_index].clamp(0.0, 1.0);
                            clipped_samples += 1;
                        }
                    }
                }
            }
            pixel[3] = source_pixel.alpha();
        }
        destination.copy_from_slice(&output);
        Ok(ColorTransformReport::new(clipped_samples, input_relation))
    }
}

fn new_transform(
    source: &RgbProfile,
    destination: &RgbProfile,
) -> Result<RgbaFloatTransform, ColorError> {
    let flags = Flags::NO_CACHE | Flags::NO_OPTIMIZE | Flags::COPY_ALPHA;
    RgbaFloatTransform::new_flags_context(
        GlobalContext::new(),
        &source.inner,
        PixelFormat::RGBA_FLT,
        &destination.inner,
        PixelFormat::RGBA_FLT,
        Intent::RelativeColorimetric,
        flags,
    )
    .map_err(|_| ColorError::TransformCreation)
}

fn validate_encoded_raster(source: &[[f32; 4]]) -> Result<(), ColorError> {
    for (pixel, samples) in source.iter().enumerate() {
        for (channel, sample) in [
            RasterChannel::Red,
            RasterChannel::Green,
            RasterChannel::Blue,
            RasterChannel::Alpha,
        ]
        .into_iter()
        .zip(samples)
        {
            if !sample.is_finite() || !(0.0..=1.0).contains(sample) {
                return Err(ColorError::InvalidRasterSample { pixel, channel });
            }
        }
    }
    Ok(())
}

fn validate_finite_rgb(pixel: usize, output: &[f32; 4]) -> Result<(), ColorError> {
    for (channel, sample) in [
        RasterChannel::Red,
        RasterChannel::Green,
        RasterChannel::Blue,
    ]
    .into_iter()
    .zip(output)
    {
        if !sample.is_finite() {
            return Err(ColorError::NonFiniteOutput { pixel, channel });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lcms2::{Profile, ProfileClassSignature};

    #[test]
    fn a_generated_general_lut_input_profile_is_supported() {
        let srgb = Profile::new_srgb();
        let xyz = Profile::new_xyz();
        let transform: Transform<[f32; 3], [f32; 3]> = Transform::new_flags(
            &srgb,
            PixelFormat::RGB_FLT,
            &xyz,
            PixelFormat::XYZ_FLT,
            Intent::RelativeColorimetric,
            Flags::FORCE_CLUT | Flags::NO_OPTIMIZE,
        )
        .unwrap();
        let mut lut = Profile::new_device_link(&transform, 4.3, Flags::FORCE_CLUT).unwrap();
        lut.set_device_class(ProfileClassSignature::InputClass);
        let bytes = lut.icc().unwrap();
        let resolved =
            crate::io::color::embedded_rgb_profile(&bytes, &ResourceLimits::default()).unwrap();
        assert!(!resolved.profile.is_matrix_shaper());
        let transform =
            RasterToWorkingTransform::new(&resolved.profile, &ResourceLimits::default()).unwrap();
        let mut output = [RgbaPixel::new(0.0, 0.0, 0.0, 1.0).unwrap()];
        transform
            .transform_scanline(
                &[[0.5, 0.25, 0.75, 1.0]],
                &mut output,
                &ResourceLimits::default(),
            )
            .unwrap();
        assert!(output[0].red().is_finite());
    }
}
