# WP5 geometry and radial-mask contract

## Coordinates and ordering

Geometry uses image-edge coordinates. An image occupies `[0, width] x
[0, height]`, and pixel `(x, y)` has center `(x + 0.5, y + 0.5)`. Persisted
operations run in this order:

1. exact clockwise quarter-turn;
2. exact horizontal and vertical flips in the turned image;
3. inverse-mapped straighten and perspective homography;
4. normalized crop, whose lower edge is floored and upper edge is ceiled.

Identity and orthogonal transforms copy/permutate pixels without resampling.
General transforms retain the current canvas extent and use separable Lanczos3
with a transparent border. Straight-alpha pixels are premultiplied before
filtering and unpremultiplied afterwards, preventing hidden RGB in transparent
pixels from bleeding into visible pixels. All accumulations use `f64`; stored
pixels retain the Foundation `f32` scene-linear Rec.2020 contract.

EXIF orientation is import-time runtime state rather than a persisted edit. A
central crate-internal helper provides exact mappings for all eight EXIF
orientations without adding an EXIF field to `GeometrySettings`. In the current
QML MVP, Qt `Image.autoTransform` normalizes standard-image EXIF exactly once.
The RAW command deliberately omits `-t`, so LibRaw applies camera orientation;
its generated PPM carries no EXIF orientation for Qt to apply again. The future
CPU decoder must either disable decoder orientation and call the helper, or
accept decoder orientation and pass `Normal`, never both. Production currently
has no Qt/LibRaw-to-scene-linear-Rec.2020 `CpuImage` decode bridge, so pretending
to invoke the CPU helper from the loader would be incorrect.

## Radial masks

Ellipse centers and radii are normalized independently against full image width
and height. Coverage is evaluated analytically at global pixel centers. The
gradient of normalized ellipse distance converts its first-order signed distance
to physical pixels. This gives an exact symmetric one-pixel transition for
circles and ellipse principal axes, and a gradient-based local one-pixel
approximation elsewhere; smooth feather, inversion, and opacity compose on top.
Using global coordinates makes a full-frame evaluation invariant to how a
future scheduler divides the image into tiles.

Local edits are generated through a processor trait, then straight-RGB
composited by analytic coverage while alpha remains unchanged. Scene-linear
Exposure EV, brightness, contrast, saturation, temperature, and tint use the
same prepared WP1 kernels and order as global Basics; Exposure EV runs before
the other local point controls. Positive sharpness uses the same WP3 Gaussian,
threshold, luma coefficients, strength, Reflect101 halo, and finite conversion
as global USM. The RadialMasksV1 local model permits negative sharpness although
WP3 does not define it; active negative local sharpness therefore fails loudly
at preflight instead of inventing blur semantics.

Preset schema v3 introduces local `exposure_ev`. Readers migrate schema v1 and
v2 masks to the neutral value `0`; those older versions reject the field rather
than accepting ambiguous forward data. Schema v3 requires it in every mask.

Non-inverted masks allocate and process only the rotated ellipse bounding box
plus a one-pixel analytic-AA margin. Sharpness reads its three-sigma halo from
the complete input through global Reflect101 coordinates but materializes only
the bounding-box result. Inverted masks necessarily cover the full frame. Peak
temporary storage is `16 * ROI pixels` bytes plus two seven-value sharpness rows;
there is no full-frame clone per small mask. The worst case remains 64 inverted
full-frame masks processed sequentially, `O(64*N)` work but only `O(N)` peak
temporary memory. Flat masks deliberately remain sequential layers.

Soft-coverage Replace, Union, Intersect, and Subtract algebra is implemented and
unit tested. Foundation F0 does **not** persist a group identifier or combine
operator, so public preset masks are applied only as independent sequential
layers. Grainroom deliberately does not infer grouping from consecutive IDs or
array position. Persisted grouping requires a future schema version plus the
corresponding parameter-registry additions.

## Provenance

The implementation and formulas are original Grainroom code. Architectural
behavior was compared conceptually with these pinned public sources:

- darktable `clipping.c` at
  [`943d74a50e5baeecee26005cf20309e32f487949`](https://github.com/darktable-org/darktable/blob/943d74a50e5baeecee26005cf20309e32f487949/src/iop/clipping.c)
  for inverse geometry/ROI concepts;
- darktable `ellipse.c` at the same commit for analytic ellipse-mask concepts:
  [`ellipse.c`](https://github.com/darktable-org/darktable/blob/943d74a50e5baeecee26005cf20309e32f487949/src/develop/masks/ellipse.c);
- RawTherapee's transform engine at
  [`498f623784e33fd9a7077fcd8937fe0734033366`](https://github.com/RawTherapee/RawTherapee/tree/498f623784e33fd9a7077fcd8937fe0734033366/rtengine)
  for comparison of transform-stage separation and resampling choices.

darktable and RawTherapee are GPL-3.0-or-later projects. No implementation,
constants, LUTs, profiles, or source fragments from either project are copied;
the links record conceptual comparison only.
