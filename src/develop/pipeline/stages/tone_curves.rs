use crate::develop::{
    CpuImage, PipelineError,
    settings::{ToneCurve, ToneCurvesSettings},
};

// Grainroom's monotone cubic curve is documented in docs/develop/wp1-math.md.

const LUMA: [f64; 3] = [0.262_700_2, 0.677_998_1, 0.059_301_7];
const LUMA_EPSILON: f64 = 1.0e-6;

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

    let master = PreparedCurve::new(&settings.master);
    let red = PreparedCurve::new(&settings.red);
    let green = PreparedCurve::new(&settings.green);
    let blue = PreparedCurve::new(&settings.blue);

    for pixel in image.pixels_mut() {
        let mut rgb = [
            f64::from(pixel.red),
            f64::from(pixel.green),
            f64::from(pixel.blue),
        ];

        if !master.is_identity() {
            let old_luminance = luminance(rgb);
            let new_luminance = master.evaluate(old_luminance);
            if old_luminance.abs() > LUMA_EPSILON {
                let scale = new_luminance / old_luminance;
                rgb = [rgb[0] * scale, rgb[1] * scale, rgb[2] * scale];
            } else {
                // Scaling is singular at zero luminance. An equal additive
                // offset changes Rec.2020 luminance by exactly the requested
                // delta because the luminance coefficients sum to one.
                let delta = new_luminance - old_luminance;
                rgb = [rgb[0] + delta, rgb[1] + delta, rgb[2] + delta];
            }
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

fn luminance(rgb: [f64; 3]) -> f64 {
    rgb[0] * LUMA[0] + rgb[1] * LUMA[1] + rgb[2] * LUMA[2]
}

enum PreparedCurve {
    Identity,
    Pchip {
        segments: Box<[PchipSegment]>,
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

impl PreparedCurve {
    fn new(curve: &ToneCurve) -> Self {
        if curve.points.iter().all(|point| point.x == point.y) {
            return Self::Identity;
        }

        let points: Vec<(f64, f64)> = curve
            .points
            .iter()
            .map(|point| (f64::from(point.x), f64::from(point.y)))
            .collect();
        let slopes = pchip_slopes(&points);
        let segments = points
            .windows(2)
            .enumerate()
            .map(|(index, pair)| {
                let (x0, y0) = pair[0];
                let (x1, y1) = pair[1];
                let width = x1 - x0;
                let first_tangent = width * slopes[index];
                let second_tangent = width * slopes[index + 1];
                PchipSegment {
                    x0,
                    x1,
                    y0,
                    y1,
                    cubic: 2.0 * (y0 - y1) + first_tangent + second_tangent,
                    quadratic: 3.0 * (y1 - y0) - 2.0 * first_tangent - second_tangent,
                    linear: first_tangent,
                }
            })
            .collect();
        Self::Pchip {
            segments,
            first_y: points[0].1,
            first_slope: slopes[0],
            last_y: points[points.len() - 1].1,
            last_slope: slopes[slopes.len() - 1],
        }
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

fn pchip_slopes(points: &[(f64, f64)]) -> Vec<f64> {
    let segment_count = points.len() - 1;
    let widths: Vec<f64> = points
        .windows(2)
        .map(|pair| pair[1].0 - pair[0].0)
        .collect();
    let secants: Vec<f64> = points
        .windows(2)
        .zip(&widths)
        .map(|(pair, width)| (pair[1].1 - pair[0].1) / width)
        .collect();
    if segment_count == 1 {
        return vec![secants[0], secants[0]];
    }

    let mut slopes = vec![0.0; points.len()];
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
