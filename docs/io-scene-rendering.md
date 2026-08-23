# Scene-related RAW to SDR display rendering

RAW decode and the develop pipeline use finite, unbounded, scene-related linear
Rec.2020/D65 pixels. An sRGB ICC transform cannot decide how that scene should
fit an SDR display. `SceneToDisplayTransform` is the explicit technical output
boundary between those domains. It produces display-referred linear Rec.2020;
`WorkingToSrgbTransform` remains solely responsible for the sRGB transfer curve
and ICC provenance.

This renderer is deliberately not a creative develop control. Raster inputs
already marked `LinearizedDisplayReferred` bypass it, and the renderer rejects
that relation if called accidentally. It is absent from `DevelopSettings` and
preset JSON. A future change to the fixed rendering policy requires a new
`SceneRenderAlgorithm` version rather than silently changing existing output.

## V1 tone curve

For Rec.2020 luminance

```
Y = 0.2627002 R + 0.6779981 G + 0.0593017 B
```

positive luminance is mapped with the stable log-logistic curve

```
z    = logit(0.18) + 1.7 ln(Y / 0.18)
Yout = sigmoid(z)
```

where sigmoid is evaluated with separate positive and negative branches to
avoid exponential overflow. The curve maps scene middle gray 0.18 exactly to
display middle gray 0.18, is monotone, and approaches one without a hard
highlight clip. RGB is initially scaled by `Yout / Y`, preserving channel
ratios. Non-positive luminance has no meaningful logarithmic exposure and is
mapped to display black; the report counts those pixels. All math uses `f64`,
so finite signed/HDR `f32` inputs cannot overflow the calculation.

Creative exposure is upstream. In particular, the existing exposure control
changes the scene value presented to this curve. V1 has no auto-exposure,
histogram dependency, hidden compensation, or per-image state, so preview and
export can produce identical results.

## sRGB-target gamut compression

The toned Rec.2020 RGB is converted by the fixed D65 linear-light
Rec.2020-to-sRGB matrix. If a target channel lies outside `[0, 1]`, chroma is
scaled toward the neutral point `[Yout, Yout, Yout]` by the greatest scalar in
`[0, 1]` that puts every target channel on or inside the cube. Chromatic
boundaries use a fixed `2e-4` inset to cover numerical differences between the
documented matrix and generated LCMS profiles. The local interval always
includes the neutral coordinate, so neutral black and white are not lifted or
lowered. This preserves neutral luminance and hue direction while avoiding
independent RGB clipping. The bounded target value is converted back to linear
Rec.2020. The inverse matrix has positive coefficients, so the returned
Rec.2020 channels are also bounded. The existing LCMS sRGB output transform can
consequently use `SdrRangePolicy::Reject`; clipping after rendering indicates a
bug or a future contract mismatch.

V1 targets sRGB even if another `OutputProfile` is requested. P3/Rec.2020
exports require separately versioned target matrices and policies; they must
not reuse this report while claiming another gamut.

## Transaction, streaming, and memory

`transform_scanline` validates the input relation, length, limits, and every
result before committing anything to the caller's destination. Alpha is copied
bit-for-bit. The method is stateless and pointwise, so arbitrary scanline or
tile partitions are bit-identical. It allocates one fallible transactional
`RgbaPixel` scratch row: exactly 16 bytes per submitted pixel, with O(N) time
and no full-frame histogram or pyramid.

The intended export order is:

1. decode RAW to scene-related linear Rec.2020;
2. apply the creative develop pipeline, including exposure;
3. render each bounded row with `SceneToDisplayTransform`;
4. pass the reported `LinearizedDisplayReferred` relation to
   `WorkingToSrgbTransform` and encode.

A rendered image is an output-stage artifact, not a new `DecodedPhoto` with
RAW provenance: the checked `DecodedPhoto` invariant correctly associates a
RAW matrix with `SceneRelatedRaw`.

## Independent design and comparative provenance

The formula and constants above are an independently specified Grainroom V1
policy. No upstream implementation or parameter set is copied. The separation
of scene-referred creative work, a display-rendering tone operator, gamut
mapping, and output encoding was conceptually compared with these public
GPL-3.0-or-later implementations:

- darktable snapshot `943d74a50e5baeecee26005cf20309e32f487949`,
  [`src/iop/filmicrgb.c`](https://github.com/darktable-org/darktable/blob/943d74a50e5baeecee26005cf20309e32f487949/src/iop/filmicrgb.c)
  and [`src/iop/colorbalancergb.c`](https://github.com/darktable-org/darktable/blob/943d74a50e5baeecee26005cf20309e32f487949/src/iop/colorbalancergb.c);
- RawTherapee snapshot `498f623784e33fd9a7077fcd8937fe0734033366`,
  [`rtengine/iplab2rgb.cc`](https://github.com/RawTherapee/RawTherapee/blob/498f623784e33fd9a7077fcd8937fe0734033366/rtengine/iplab2rgb.cc).

Those sources are comparative provenance only. Grainroom's implementation is
the small formula documented in this file.
