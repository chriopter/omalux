# Develop pipeline resource budgets

The bounded pipeline is fail-closed. A caller selects settings; Grainroom
derives a named, reviewed allocation profile from those settings and checks the
entire peak before creating the transactional image. There is no caller-supplied
scratch estimate.

## `PointwiseV1`

`PointwiseV1` is the always-present base family. With all optional families
inactive it supports Basics with clarity at zero and Effects containing only
fade, vignette, and grain. Brightness, contrast, highlights, shadows, whites, blacks,
saturation, vibrance, temperature, and tint are pointwise and supported.

The exact requested image-payload peak is:

```
source RGBA-f32 image       16 * width * height
transactional RGBA-f32     16 * width * height
stage image scratch         0
------------------------------------------------
peak                       32 * width * height bytes
```

`RgbaPixel` is compile-time pinned to 16 bytes. The estimate charges Rust heap
payload requested by this operation. Allocator bookkeeping and size-class
rounding are implementation-owned and neither observable nor controllable
through Rust's allocation API; `ResourceLimits` consistently excludes that
allocator-internal overhead. Existing caller-owned settings, mask IDs, and
source storage outside `CpuImage` are not newly allocated by this operation.

## `SpatialV1`

`SpatialV1` adds global clarity, bloom, halation, and sharpness to the
`PointwiseV1` stages. The profile
charges the source and transaction above plus the largest sequential spatial
stage; spatial stages do not run concurrently.

Every covered stage retains at most three full `f32` scalar planes
(`12 * pixels`). Clarity additionally retains two `f64` tile halos:

```
clarity auxiliary = min(width, 128) * (min(height, 64) + 16) * 16
```

The effects auxiliary bound covers a radius-eight Gaussian tile, its
17-value `f64` kernel, and 32 pyramid dimension pairs:

```
effects auxiliary = min(width, 128) * (min(height, 64) + 16) * 4
                  + 17 * 8 + 32 * 16
```

Thus `stage_scratch_bytes` is `12 * pixels` plus the maximum active auxiliary
term. The clarity term is exact for its canonical 128x64 tiling. The effects
term is conservative: smaller residual kernels and pyramid levels may use less
memory, but never more. Declared-estimate and peak-minus-one tests exercise every
admitted family and verify that the gate runs before the transaction or any
pixel mutation.

Successful settings validation is allocation-free: dynamic diagnostic paths
are built only after a validation failure, and duplicate mask IDs use the
schema's maximum-64 bounded pairwise scan instead of a `HashSet`. Bounded
processing validates settings once.

Small fixed stack state is not charged as image working memory. Grain's render
context is fixed-size and grain does not allocate a noise plane. The source
remains resident while stages operate on the transactional copy. The copy is
allocated with `try_reserve_exact`; allocation failure is a typed pipeline
error. Commit remains a final move, so limit, allocation, stage, or numeric
failure leaves the caller's image unchanged.

## `ColorV1`

`ColorV1` is `PointwiseV1` plus active master/R/G/B tone curves, the eight-band
color mixer, and three-way color grading. Curve documents have at most 32
points per curve. Preparation uses fixed 31/32-element stack arrays for PCHIP
widths, secants, and slopes, then one fallible, exact-reserve segment vector per
active non-identity curve. A segment is compile-time pinned to seven `f64`
values (56 bytes). All four curves are prepared before pixels are touched.

The exact requested peak is:

```
source + transactional images       32 * width * height
prepared PCHIP segments             56 * sum(active_curve_points - 1)
color mixer/grading heap scratch      0
----------------------------------------------------------------
peak                                  sum of the rows above
```

The maximum curve payload is `4 * 31 * 56 = 6,944` bytes. Identity curves do
not allocate. Mixer and grading preparation consists only of fixed-size stack
arrays. Allocation failure while preparing any curve is typed and precedes the
first pixel write; the outer transactional image provides the same atomicity
for later stage/numeric failures. As for `PointwiseV1`, allocator-internal
rounding is outside the requested-payload contract.

## Explicit family composition

The selected profile is an explicit component record: `pointwise_v1` is always
true and `color_v1`, `spatial_v1`, `geometry_v1`, and `radial_masks_v1` state
which reviewed optional families are active. This avoids an ambiguous packed
tag and composes all family combinations without inventing a variant for each
one. Color and spatial stages execute sequentially, so their scratch payloads
are never resident together. Their declared peak is:

```
source + transactional images       32 * width * height
stage scratch                        max(ColorV1 scratch, SpatialV1 scratch)
----------------------------------------------------------------------------
peak                                  sum of the rows above
```

Tests cover both a color-dominant and a spatial-dominant union and reject
peak-minus-one before the transaction, pixel mutation, or encoder call.

## `GeometryV1`

Geometry uses fallible exact-reserve output buffers. Orthogonal transforms and
projective resampling each request one full RGBA-f32 image; when both are
active, the orthogonal result and projective output overlap for a two-image
stage peak. A following crop overlaps its exact pixel-aligned ROI output with
the preceding result. Crop-only settings request only that ROI. The estimator
uses the same edge rounding and post-rotation dimensions as the renderer.

Geometry runs while the original source and outer transaction are resident.
After it commits, later-family estimates use the cropped dimensions. The
reported job peak is the maximum of these sequential phases.

## `RadialMasksV1`

Active masks are processed sequentially. Coverage is analytic in global pixel
coordinates, so there is no heap mask plane. Each mask requests one exact ROI
RGBA-f32 output. Non-inverted masks use their clipped rotated-ellipse bound;
inverted masks use the full frame. The normative seven-tap local-sharpness
kernel and its seven-value horizontal scratch are fixed stack arrays. Thus the
heap scratch is exactly `16 * max(active ROI pixels)`. Negative local sharpness
remains unsupported and fails before mutation. Local Exposure EV is a heap-free
point operation and does not change the ROI estimate.

## Allocation audit and gates

| Stage | Current variable allocation | Bounded status |
|---|---|---|
| Geometry | fallible full output/crop/resample images | `GeometryV1` |
| Basics point operations | none | `PointwiseV1` |
| Clarity | three full f32 planes plus bounded f64 tile scratch | `SpatialV1` |
| Tone curves | fallible exact-reserve segment vectors; fixed stack coefficient work | `ColorV1` |
| Color mixer/grading | fixed stack arrays only | `ColorV1` |
| Radial masks | fallible exact ROI output; kernel/scratch on stack | `RadialMasksV1` |
| Bloom/halation | full scalar planes and bounded pyramid levels | `SpatialV1` |
| Sharpness | full luma/blur planes and spatial scratch | `SpatialV1` |
| Fade/vignette/grain | none | `PointwiseV1` |

Unsupported negative local sharpness returns a typed capability/profile error
before the transactional copy is allocated and before mutation. Every covered
heap allocation uses fallible reservation and maps allocation failure to the
resource-limit category.

`process_with_context` remains the compatibility API. Its full-image
transaction copy is now fallible, but it does not claim a resource ceiling.
Budget-sensitive jobs must call `process_bounded_with_context`.
