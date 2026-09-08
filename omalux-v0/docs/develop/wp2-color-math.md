# WP2 color math

This document records the normative CPU behavior of Omalux's color mixer
and three-way color-grading stages. It distinguishes the formulas implemented
here from the public references that informed the design.

## Working-space contract

- Input and output are scene-linear Rec.2020/D65 RGB.
- RGB channels are finite but unbounded; negative and HDR values are valid.
- Alpha is straight, finite, and is never modified by either color stage.
- Rec.2020 luminance is
  `Y = 0.2627002 R + 0.6779981 G + 0.0593017 B`.
- Neutral settings return without touching the pixel buffer.

The primaries and D65 white point come from
[ITU-R BT.2020-2](https://www.itu.int/rec/R-REC-BT.2020-2-201510-I/en).
The RGB/XYZ matrices in `src/develop/color.rs` are the corresponding
high-precision linear transforms.

## Rec.2020 and OKLab

Omalux converts Rec.2020 to XYZ, then uses Björn Ottosson's published OKLab
XYZ/LMS and opponent transforms. The primary source is
[A perceptual color space for image processing](https://bottosson.github.io/posts/oklab/)
(retrieved 2026-08-23).

The published model assumes ordinary non-negative colorimetric values.
Omalux deliberately extends it to its unbounded scene-linear contract by
using a signed real cube root for every LMS component. The inverse cubes the
signed components normally. Tests pin Rec.2020 primary values and round trips
for positive, negative, and HDR samples.

## Exact target luminance

Hue or chroma edits in OKLab can change Rec.2020 Y. For a fixed pair `(a, b)`,
inverse OKLab makes Y a cubic polynomial in OKLab lightness `L`:

```text
Y(L) = wy_l (L + dl)^3 + wy_m (L + dm)^3 + wy_s (L + ds)^3
```

The offsets are the inverse-OKLab contributions of `(a, b)`, and the three
weights are the Y row of the LMS-to-XYZ matrix. Omalux subtracts the target
Y, splits the cubic at its real stationary points, brackets every real root,
and applies safeguarded Newton iterations inside each monotone bracket. If the
cubic has multiple roots, the root nearest the incoming L is selected to avoid
an unnecessary lightness branch jump.

Source Y and the exposure-scaled target Y are calculated in f64. A target that
is non-finite or whose absolute value is strictly larger than `f32::MAX` is
rejected before the pixel is changed, including values inside the cast-to-max
rounding interval. Omalux also has an explicit flush-to-zero boundary:
every non-zero target with `abs(Y) < f32::MIN_POSITIVE` is rejected rather than
silently rounded to zero; `±f32::MIN_POSITIVE` are accepted. After conversion
back to f32 RGB, the residual is checked in f64 against
`64 * f32::EPSILON * max(abs(target_Y), f32::MIN_POSITIVE)`. A non-zero accepted
target can never succeed with an actual Y of zero. An additive neutral correction is a
last-resort rounding/ill-conditioning fallback. Since the Rec.2020 luminance
coefficients sum to one, adding the same delta to R, G, and B restores Y without
clipping. Every fallback is followed by the same definitive finite residual
check. Failure produces `PipelineError::NumericFailure` and the transactional
pipeline leaves the caller's complete image unchanged; an infinite tolerance
is never accepted.

Regression tests include the signed counterexamples that defeated the former
unbounded Newton iteration and deterministic property sweeps through the
actual mixer and grading pipelines.

## Eight-band color mixer

The fixed OKLCh hue centers are red, orange, yellow, green, aqua, blue, purple,
and magenta at 45-degree intervals. Cyclic distance selects neighboring bands;
a cubic smoothstep kernel produces non-negative normalized weights.

- Hue-shift offsets use a weighted circular mean of sine and cosine. Thus
  `+180` and `-180` are treated as the same rotation instead of cancelling.
- Saturation multiplies OKLCh chroma by `max(0, 1 + adjustment / 100)`.
- Luminance is a weighted exposure of up to two stops:
  `target_Y = source_Y * 2^(2 * adjustment / 100)`.
- Exact and near-neutral pixels are gated out because their hue is undefined;
  this prevents an arbitrary band from changing gray pixels.

These weighting and parameter mappings are Omalux formulas, not copied
from another application.

## Three-way color grading

Shadows, midtones, and highlights are smooth normalized masks over OKLab L.
Balance shifts the mask coordinate by up to 0.25; blending changes each
transition width from 0.05 to 0.45. The three selected hues contribute OKLab
`a/b` vectors with maximum magnitude 0.15. Per-range luminance is a weighted
exposure of up to two stops. Chroma-only grading preserves source Rec.2020 Y.

The conceptual separation into smooth shadows/midtones/highlights masks and a
scene-referred color-grading stage was cross-checked against darktable's public
`colorbalancergb.c` at the pinned repository snapshot
[`943d74a50e5baeecee26005cf20309e32f487949`](https://github.com/darktable-org/darktable/blob/943d74a50e5baeecee26005cf20309e32f487949/src/iop/colorbalancergb.c).
Omalux does not reproduce darktable's UCS, gamut mapping, masks, parameter
scales, or implementation; the OKLab model and all formulas above are local.
darktable is licensed GPL-3.0-or-later. Its file is cited for conceptual and
comparative provenance only; no darktable implementation is incorporated here.

## Module integration

`src/develop/mod.rs` declares exactly one shared `color` module. The mixer and
grading stages both import `crate::develop::color`; no path-based or duplicate
compilation of the numerical helpers exists. Consequently `ColorMathError`,
the conversion matrices, and the luminance solver have one implementation.
