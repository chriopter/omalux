# Develop pipeline resource budgets

The bounded pipeline is fail-closed. A caller selects settings; Grainroom
derives a named, reviewed allocation profile from those settings and checks the
entire peak before creating the transactional image. There is no caller-supplied
scratch estimate.

## `PointwiseV1`

`PointwiseV1` supports neutral geometry, neutral curves/mixer/grading/radial
masks, Basics with clarity at zero, and Effects containing only fade, vignette,
and grain. Brightness, contrast, highlights, shadows, whites, blacks,
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
`PointwiseV1` stages. Geometry and radial masks remain fail-closed. The profile
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
memory, but never more. Exact-estimate and peak-minus-one tests exercise every
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

## Allocation audit and gates

| Stage | Current variable allocation | Bounded status |
|---|---|---|
| Geometry | full output/crop/resample image | gated pending geometry profile |
| Basics point operations | none | `PointwiseV1` |
| Clarity | three full f32 planes plus bounded f64 tile scratch | `SpatialV1` |
| Tone curves | fallible exact-reserve segment vectors; fixed stack coefficient work | `ColorV1` |
| Color mixer/grading | fixed stack arrays only | `ColorV1` |
| Radial masks | ROI output and optional sharpness halo/scratch | gated pending ROI/mask profile |
| Bloom/halation | full scalar planes and bounded pyramid levels | `SpatialV1` |
| Sharpness | full luma/blur planes and spatial scratch | `SpatialV1` |
| Fade/vignette/grain | none | `PointwiseV1` |

An active gated stage returns `ResourceProfileUnavailable(stage)` during
preflight, before the transactional copy is allocated and before mutation.
This is an intentional contract boundary, not an optimistic estimate. Future
profiles must account for simultaneous source, transaction, output/ROI planes,
pyramid levels, and tile scratch, and must make all covered heap allocations
fallible before being enabled.

`process_with_context` remains the compatibility API. Its full-image
transaction copy is now fallible, but it does not claim a resource ceiling.
Budget-sensitive jobs must call `process_bounded_with_context`.
