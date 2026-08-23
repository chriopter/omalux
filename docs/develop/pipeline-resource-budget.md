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

The exact image-buffer peak is:

```
source RGBA-f32 image       16 * width * height
transactional RGBA-f32     16 * width * height
stage image scratch         0
------------------------------------------------
peak                       32 * width * height bytes
```

Small fixed stack state is not charged as image working memory. Grain's render
context is fixed-size and grain does not allocate a noise plane. The source
remains resident while stages operate on the transactional copy. The copy is
allocated with `try_reserve_exact`; allocation failure is a typed pipeline
error. Commit remains a final move, so limit, allocation, stage, or numeric
failure leaves the caller's image unchanged.

## Allocation audit and gates

| Stage | Current variable allocation | Bounded status |
|---|---|---|
| Geometry | full output/crop/resample image | gated pending geometry profile |
| Basics point operations | none | `PointwiseV1` |
| Clarity | three full f32 planes plus bounded f64 tile scratch | gated pending spatial profile |
| Tone curves | prepared point/slope/segment vectors | gated pending fallible preparation profile |
| Color mixer/grading | small solver vectors in the per-pixel implementation | gated pending allocation-free solver refactor |
| Radial masks | ROI output and optional sharpness halo/scratch | gated pending ROI/mask profile |
| Bloom/halation | full scalar planes and bounded pyramid levels | gated pending spatial/pyramid profile |
| Sharpness | full luma/blur planes and spatial scratch | gated pending spatial profile |
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
