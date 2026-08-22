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

EXIF orientation is import-time runtime state rather than a persisted edit. The
stage therefore provides an exact helper for all eight EXIF orientations but
does not add an EXIF field to `GeometrySettings`.

## Radial masks

Ellipse centers and radii are normalized independently against full image width
and height. Coverage is evaluated analytically at global pixel centers, with a
one-pixel edge antialiasing width, smooth feather, inversion, and opacity.
Using global coordinates makes a full-frame evaluation invariant to how a
future scheduler divides the image into tiles.

Local edits are generated through a processor trait, then straight-RGB
composited by analytic coverage while alpha remains unchanged. The built-in CPU
processor supplies deterministic brightness, contrast, saturation,
temperature, tint, and luma-sharpness behavior for the current flat schema.

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
