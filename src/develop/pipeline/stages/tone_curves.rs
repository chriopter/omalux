use crate::develop::{
    CpuImage, PipelineError,
    settings::{ToneCurve, ToneCurvesSettings},
};

// Grainroom's monotone cubic curve is documented in docs/develop/wp1-math.md.

const LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const MASTER_BLEND_START: f64 = 0.025;
const MASTER_BLEND_END: f64 = 0.05;
const MASTER_CHANGE_BLEND_START: f64 = 0.5;
const MASTER_CHANGE_BLEND_END: f64 = 1.0;

pub(super) fn supports(_settings: &ToneCurvesSettings) -> bool {
    true
}

pub(super) fn apply(
    image: &mut CpuImage,
    settings: &ToneCurvesSettings,
) -> Result<(), PipelineError> {
    if settings.is_neutral() {
        return Ok(());
    }

    // Prepare every curve before touching pixels. A typed allocation failure
    // therefore leaves even the transactional image unchanged at this stage.
    let master = PreparedCurve::try_new(&settings.master)?;
    let red = PreparedCurve::try_new(&settings.red)?;
    let green = PreparedCurve::try_new(&settings.green)?;
    let blue = PreparedCurve::try_new(&settings.blue)?;

    for pixel in image.pixels_mut() {
        let mut rgb = [
            f64::from(pixel.red),
            f64::from(pixel.green),
            f64::from(pixel.blue),
        ];

        if !master.is_identity() {
            let old_luminance = luminance(rgb);
            let new_luminance = master.evaluate(old_luminance);
            rgb = remap_master_luminance(rgb, old_luminance, new_luminance);
        }

        rgb = [
            red.evaluate(rgb[0]),
            green.evaluate(rgb[1]),
            blue.evaluate(rgb[2]),
        ];
        pixel.red = rgb[0] as f32;
        pixel.green = rgb[1] as f32;
        pixel.blue = rgb[2] as f32;
    }
    Ok(())
}

pub(super) fn prepared_heap_bytes(settings: &ToneCurvesSettings) -> Result<u64, PipelineError> {
    [
        &settings.master,
        &settings.red,
        &settings.green,
        &settings.blue,
    ]
    .into_iter()
    .try_fold(0_u64, |total, curve| {
        if curve.points.iter().all(|point| point.x == point.y) {
            return Ok(total);
        }
        let segments = curve
            .points
            .len()
            .checked_sub(1)
            .ok_or(PipelineError::ResourceLimit(
                crate::io::LimitError::ArithmeticOverflow,
            ))?;
        let bytes = segments
            .checked_mul(std::mem::size_of::<PchipSegment>())
            .and_then(|bytes| u64::try_from(bytes).ok())
            .ok_or(PipelineError::ResourceLimit(
                crate::io::LimitError::ArithmeticOverflow,
            ))?;
        total.checked_add(bytes).ok_or(PipelineError::ResourceLimit(
            crate::io::LimitError::ArithmeticOverflow,
        ))
    })
}

fn luminance(rgb: [f64; 3]) -> f64 {
    rgb[0] * LUMA[0] + rgb[1] * LUMA[1] + rgb[2] * LUMA[2]
}

fn remap_master_luminance(rgb: [f64; 3], old_luminance: f64, new_luminance: f64) -> [f64; 3] {
    let delta = new_luminance - old_luminance;
    let additive = [rgb[0] + delta, rgb[1] + delta, rgb[2] + delta];
    let max_abs_rgb = rgb[0].abs().max(rgb[1].abs()).max(rgb[2].abs());
    if max_abs_rgb == 0.0 {
        return additive;
    }

    // Luminance relative to chroma, rather than an absolute epsilon, detects
    // cancellation colors at every scene-linear scale. A second relative
    // measure suppresses ratio scaling when a black lift is large compared
    // with the pixel itself, keeping every path through RGB origin continuous.
    // Both candidates have the requested luminance.
    let relative_luminance = old_luminance.abs() / max_abs_rgb;
    let relative_weight =
        smooth_range_weight(relative_luminance, MASTER_BLEND_START, MASTER_BLEND_END);
    let relative_change = delta.abs() / max_abs_rgb;
    let change_weight = 1.0
        - smooth_range_weight(
            relative_change,
            MASTER_CHANGE_BLEND_START,
            MASTER_CHANGE_BLEND_END,
        );
    let ratio_weight = relative_weight * change_weight;
    let mut remapped = if ratio_weight == 0.0 {
        additive
    } else {
        let scale = new_luminance / old_luminance;
        let ratio = [rgb[0] * scale, rgb[1] * scale, rgb[2] * scale];
        if ratio_weight == 1.0 {
            ratio
        } else {
            let additive_weight = 1.0 - ratio_weight;
            [
                additive[0] * additive_weight + ratio[0] * ratio_weight,
                additive[1] * additive_weight + ratio[1] * ratio_weight,
                additive[2] * additive_weight + ratio[2] * ratio_weight,
            ]
        }
    };

    // Correct only floating-point cancellation residue. Equal-channel
    // addition changes Rec.2020 luminance by the same amount.
    let residual = new_luminance - luminance(remapped);
    remapped = [
        remapped[0] + residual,
        remapped[1] + residual,
        remapped[2] + residual,
    ];
    remapped
}

fn smooth_range_weight(value: f64, start: f64, end: f64) -> f64 {
    let coordinate = ((value - start) / (end - start)).clamp(0.0, 1.0);
    coordinate * coordinate * (3.0 - 2.0 * coordinate)
}

enum PreparedCurve {
    Identity,
    Pchip {
        segments: Vec<PchipSegment>,
        first_y: f64,
        first_slope: f64,
        last_y: f64,
        last_slope: f64,
    },
}

struct PchipSegment {
    x0: f64,
    x1: f64,
    y0: f64,
    y1: f64,
    cubic: f64,
    quadratic: f64,
    linear: f64,
}

// ColorV1's public memory proof relies on this stable, padding-free payload.
const _: [(); 56] = [(); std::mem::size_of::<PchipSegment>()];

impl PreparedCurve {
    fn try_new(curve: &ToneCurve) -> Result<Self, PipelineError> {
        if curve.points.iter().all(|point| point.x == point.y) {
            return Ok(Self::Identity);
        }

        let slopes = pchip_slopes(curve);
        let segment_count = curve.points.len() - 1;
        let mut segments = Vec::new();
        fail_test_allocation()?;
        segments
            .try_reserve_exact(segment_count)
            .map_err(|_| PipelineError::ResourceLimit(crate::io::LimitError::Allocation))?;
        for (index, pair) in curve.points.windows(2).enumerate() {
            let x0 = f64::from(pair[0].x);
            let y0 = f64::from(pair[0].y);
            let x1 = f64::from(pair[1].x);
            let y1 = f64::from(pair[1].y);
            let width = x1 - x0;
            let first_tangent = width * slopes[index];
            let second_tangent = width * slopes[index + 1];
            segments.push(PchipSegment {
                x0,
                x1,
                y0,
                y1,
                cubic: 2.0 * (y0 - y1) + first_tangent + second_tangent,
                quadratic: 3.0 * (y1 - y0) - 2.0 * first_tangent - second_tangent,
                linear: first_tangent,
            });
        }
        Ok(Self::Pchip {
            segments,
            first_y: f64::from(curve.points[0].y),
            first_slope: slopes[0],
            last_y: f64::from(curve.points[curve.points.len() - 1].y),
            last_slope: slopes[curve.points.len() - 1],
        })
    }

    fn is_identity(&self) -> bool {
        matches!(self, Self::Identity)
    }

    fn evaluate(&self, value: f64) -> f64 {
        match self {
            Self::Identity => value,
            Self::Pchip {
                segments,
                first_y,
                first_slope,
                last_y,
                last_slope,
            } => {
                if value < 0.0 {
                    return first_y + first_slope * value;
                }
                if value > 1.0 {
                    return last_y + last_slope * (value - 1.0);
                }
                let index = segments
                    .partition_point(|segment| segment.x1 < value)
                    .min(segments.len() - 1);
                let segment = &segments[index];
                if value == segment.x0 {
                    return segment.y0;
                }
                if value == segment.x1 {
                    return segment.y1;
                }
                let t = (value - segment.x0) / (segment.x1 - segment.x0);
                ((segment.cubic * t + segment.quadratic) * t + segment.linear) * t + segment.y0
            }
        }
    }
}

fn pchip_slopes(curve: &ToneCurve) -> [f64; 32] {
    let segment_count = curve.points.len() - 1;
    let mut widths = [0.0_f64; 31];
    let mut secants = [0.0_f64; 31];
    for (index, pair) in curve.points.windows(2).enumerate() {
        widths[index] = f64::from(pair[1].x) - f64::from(pair[0].x);
        secants[index] = (f64::from(pair[1].y) - f64::from(pair[0].y)) / widths[index];
    }
    let mut slopes = [0.0_f64; 32];
    if segment_count == 1 {
        slopes[0] = secants[0];
        slopes[1] = secants[0];
        return slopes;
    }

    for index in 1..segment_count {
        let before = secants[index - 1];
        let after = secants[index];
        if before == 0.0 || after == 0.0 || before.signum() != after.signum() {
            slopes[index] = 0.0;
        } else {
            let weight_before = 2.0 * widths[index] + widths[index - 1];
            let weight_after = widths[index] + 2.0 * widths[index - 1];
            slopes[index] =
                (weight_before + weight_after) / (weight_before / before + weight_after / after);
        }
    }
    slopes[0] = endpoint_slope(widths[0], widths[1], secants[0], secants[1]);
    slopes[segment_count] = endpoint_slope(
        widths[segment_count - 1],
        widths[segment_count - 2],
        secants[segment_count - 1],
        secants[segment_count - 2],
    );
    slopes
}

#[cfg(test)]
thread_local! {
    static FORCE_ALLOCATION_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn fail_test_allocation() -> Result<(), PipelineError> {
    #[cfg(test)]
    if FORCE_ALLOCATION_FAILURE.with(std::cell::Cell::get) {
        return Err(PipelineError::ResourceLimit(
            crate::io::LimitError::Allocation,
        ));
    }
    Ok(())
}

fn endpoint_slope(width: f64, adjacent_width: f64, secant: f64, adjacent_secant: f64) -> f64 {
    let mut slope = ((2.0 * width + adjacent_width) * secant - width * adjacent_secant)
        / (width + adjacent_width);
    if slope.signum() != secant.signum() {
        slope = 0.0;
    } else if secant.signum() != adjacent_secant.signum() && slope.abs() > 3.0 * secant.abs() {
        slope = 3.0 * secant;
    }
    slope
}

#[cfg(test)]
mod allocation_tests {
    use super::*;
    use crate::develop::{CurvePoint, RgbaPixel};

    #[test]
    fn allocation_failure_happens_before_the_first_pixel_change() {
        let mut settings = ToneCurvesSettings::default();
        settings.master.points = vec![
            CurvePoint { x: 0.0, y: 0.0 },
            CurvePoint { x: 0.5, y: 0.7 },
            CurvePoint { x: 1.0, y: 1.0 },
        ];
        let pixel = RgbaPixel::new(0.2, 0.4, 0.8, 0.5).unwrap();
        let mut image = CpuImage::new(2, 1, vec![pixel; 2]).unwrap();
        let original = image.clone();

        FORCE_ALLOCATION_FAILURE.with(|flag| flag.set(true));
        let result = apply(&mut image, &settings);
        FORCE_ALLOCATION_FAILURE.with(|flag| flag.set(false));

        assert_eq!(
            result,
            Err(PipelineError::ResourceLimit(
                crate::io::LimitError::Allocation
            ))
        );
        assert_eq!(image, original);
    }
}
