# WP1 development math

WP1 operates on straight-alpha, linear Rec.2020 RGB. It never clamps RGB, so
negative scene values and highlights above 1 remain representable. Alpha is
carried through unchanged. All computations below use `f64` internally and are
stored back as `f32`.

## Basic adjustments

The canonical order is white balance, exposure, whites/blacks,
highlights/shadows, contrast, saturation, then vibrance.

- Brightness is exposure: slider value `b` maps to `EV = b / 100`, and every
  RGB channel is multiplied by `2^EV`.
- Contrast uses 18% as its fulcrum. For luminance `Y > 0`, slider value `c`
  gives `Y' = 0.18 * (Y / 0.18)^(2^(c/100))`. RGB is scaled by `Y'/Y`.
- Whites, blacks, highlights, and shadows form smoothstep masks in the bounded
  coordinate `t = Y / (Y + 0.18)`. Their masked slider value is an exposure
  gain. This keeps the adjustment scene-referred and unclipped.
- Saturation interpolates each channel away from Rec.2020 luminance using the
  factor `1 + s/100`.
- Vibrance uses the same luminance-preserving interpolation, but attenuates a
  positive adjustment by `(1 - occupancy)^2`, where `occupancy` is the
  normalized RGB range. A negative adjustment is linear.

### Temperature and tint

Temperature is piecewise linear in reciprocal megakelvin (mired), with the
whole slider active:

| slider | white point |
| ---: | ---: |
| -100 | 25,000 K |
| 0 | D65, 6,504 K |
| +100 | 4,000 K |

Thus positive temperature is warmer. The CIE daylight-locus approximation is
translated by a constant, tiny xy offset so its 6,504 K point is exactly D65
`(0.3127, 0.3290)` while the locus remains continuous.

Tint is a signed CIE 1960 Duv displacement along the local unit normal of that
daylight locus. Slider endpoints map to `Duv = +/-0.05`; positive tint selects
the normal below the locus, toward magenta. The resulting xy white point is
converted to XYZ and a Bradford chromatic-adaptation matrix is composed between
the Rec.2020 RGB/XYZ matrices. A neutral temperature and tint bypasses this
calculation with an exact identity matrix.

At combined temperature/tint extremes, the requested Duv is continuously
capped at the closest safe point found by a fixed-iteration line search. Safe
means every target Bradford LMS component is finite and at least `0.01`. Values
inside the safe interval remain unchanged; `min(requested, safe_limit)` joins
continuously at its boundary. This prevents a negative LMS scale without
flipping or rejecting an extreme slider combination.

## Tone curves

Each master or channel curve is a monotone piecewise-cubic Hermite interpolant.
For points `(x_i, y_i)`, segment widths and secants are:

```text
h_i = x_(i+1) - x_i
d_i = (y_(i+1) - y_i) / h_i
```

An internal derivative is zero beside a flat segment; otherwise it is the
weighted harmonic mean of its neighboring secants. Endpoint derivatives use a
one-sided, shape-preserving estimate limited to three times the endpoint
secant. For a segment and normalized coordinate `t = (x - x_i) / h_i`, WP1
precomputes the exact polynomial:

```text
p(t) = a*t^3 + b*t^2 + c*t + y_i
a = 2*(y_i - y_(i+1)) + h_i*(m_i + m_(i+1))
b = 3*(y_(i+1) - y_i) - h_i*(2*m_i + m_(i+1))
c = h_i*m_i
```

Evaluation finds the segment by binary search. Stored nodes are returned
directly, including nodes closer together than a uniform lookup-table cell.
Below 0 and above 1, the curve extends linearly using its first or last endpoint
derivative.

The master curve operates on Rec.2020 luminance. Stability is measured relative
to the pixel's chroma scale as `r = abs(Y) / max(abs(R), abs(G), abs(B))`, not by
an absolute scene value. For `r <= 0.025`, including exact or chromatic
cancellation black, it adds `Y' - Y` equally to all channels. For `r >= 0.05`,
it scales RGB by `Y'/Y`, preserving channel ratios. Between those boundaries a
smoothstep crossfade joins the two candidates. Both candidates have luminance
`Y'`, so every point in the crossfade does too. A second stability weight uses
`q = abs(Y' - Y) / max(abs(R), abs(G), abs(B))`: ratio weight is one for
`q <= 0.5`, zero for `q >= 1`, and a reversed smoothstep between them. This
suppresses ratio scaling when a lifted black is larger than the source pixel;
therefore all chromatic paths converge to the same additive result at RGB
origin. A final equal-channel correction removes only floating-point
cancellation residue. The ratio candidate is never evaluated below `r = 0.025`,
bounding its channel magnitude by `abs(Y') / 0.025`, and its contribution fades
out before the source scale vanishes. This policy is continuous across all blend
boundaries and through positive or negative zero. Individual red, green, and
blue curves run afterward.

## Unrepresentable HDR output

Finite scene-linear input is intentionally unbounded, but the normative CPU
storage type is `f32`. A sufficiently steep endpoint extrapolation can therefore
produce a mathematically finite `f64` result outside the finite `f32` range.
WP1 does not silently clamp it. The pipeline renders into a private clone,
validates the complete result, returns `PipelineError::InvalidImage` for a
non-finite stored channel, and leaves the caller's image byte-for-byte unchanged.
This is the Foundation transaction contract applied explicitly to HDR overflow.

## Provenance and upstream reading

These are Grainroom-owned formulas and code, not source ports. The following
pinned upstream files were read for terminology, pipeline context, and examples
of production raw-processing behavior:

- darktable snapshot `943d74a`: [exposure.c](https://github.com/darktable-org/darktable/blob/943d74a/src/iop/exposure.c), [colorbalancergb.c](https://github.com/darktable-org/darktable/blob/943d74a/src/iop/colorbalancergb.c), [temperature.c](https://github.com/darktable-org/darktable/blob/943d74a/src/iop/temperature.c), [rgbcurve.c](https://github.com/darktable-org/darktable/blob/943d74a/src/iop/rgbcurve.c), and [curve_tools.c](https://github.com/darktable-org/darktable/blob/943d74a/src/common/curve_tools.c).
- RawTherapee snapshot `498f623`: [curves.cc](https://github.com/RawTherapee/RawTherapee/blob/498f623/rtengine/curves.cc) and [diagonalcurves.cc](https://github.com/RawTherapee/RawTherapee/blob/498f623/rtengine/diagonalcurves.cc).

The upstream curve implementations have their own interpolation, limiting, and
lookup strategies. WP1's weighted-harmonic PCHIP coefficients, exact segment
evaluation, slider normalization, Rec.2020 luminance handling, and golden tests
are independently specified above and implemented locally.
