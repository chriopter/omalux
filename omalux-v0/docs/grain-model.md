# Grain model and provenance

Omalux's normative CPU grain is an adaptation of the film-grain model shared
by darktable and RawTherapee. Preview and production export both execute this
same CPU implementation; Omalux contains no alternate grain renderer.

Primary references:

- darktable snapshot `943d74a50e5baeecee26005cf20309e32f487949`,
  [`src/iop/grain.c`](https://github.com/darktable-org/darktable/blob/943d74a50e5baeecee26005cf20309e32f487949/src/iop/grain.c) — original film-grain
  implementation, copyright the named authors and darktable developers,
  GPL-3.0-or-later.
- RawTherapee snapshot `498f623784e33fd9a7077fcd8937fe0734033366`,
  [`rtengine/ipgrain.cc`](https://github.com/RawTherapee/RawTherapee/blob/498f623784e33fd9a7077fcd8937fe0734033366/rtengine/ipgrain.cc) — documented port and
  extension, copyright Alberto Griggio, Jacques Desmis, the named darktable
  authors, and RawTherapee contributors, GPL-3.0-or-later.

The adapted model retains these defining properties:

1. Three noise octaves using frequencies `0.4910`, `0.9441`, `1.7280` and
   amplitudes `0.2340`, `0.7850`, `1.2150`, fitted by darktable to the power
   spectrum of real grain scans.
2. Grain scale expressed as an approximate film ISO.
3. A nonlinear photographic-paper response, rather than a linear noise overlay.
4. A mid-tone bias that reduces the effect in shadows and highlights.
5. A deterministic seed resolved by the caller from stable image identity.
   Grain code never receives a filename or path, so renaming a source cannot
   change an already resolved edit.

The CPU kernel uses a compact simplex-noise formulation instead of the upstream
CPU permutation table. The simplex algorithm is by Stefan Gustavson; the scalar
Rust implementation is adapted from the MIT-licensed Ashima Arts `webgl-noise`
snapshot
[`6abed1e77ed1e18b181627c35f688eb30c9fe75e`](https://github.com/ashima/webgl-noise/tree/6abed1e77ed1e18b181627c35f688eb30c9fe75e).
Its full copyright and MIT permission notice is retained verbatim in
[`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md). The GPL notices on
the adapted grain-model files remain in place.

## Normative CPU contract

- `ResolvedGrainSeed` is an opaque resolved 64-bit identity value. SplitMix64 expands it
  into six independent 24-bit-exact f32 phase coordinates in the simplex period
  `[0, 289)`. Evaluation has no shared mutable state.
- Pixel centers use global full-image coordinates divided by the full image's
  short edge. A tile carries its global origin and full extent, so tile order,
  thread count, zoom, and preview subdivision cannot change a sample.
- Grain extents are non-empty and limited to `2^20` pixels on either axis.
  Region construction uses checked area and end-coordinate arithmetic and
  rejects out-of-bounds regions and mismatched buffers in release builds. Each
  normalized pixel center and skewed lattice decomposition are computed in
  `f64`. Cartesian `x/y` are never wrapped: only the resulting integer lattice
  indices are reduced modulo 289 before entering the pinned `f32` permutation
  kernel. At the supported maximum extent, adjacent coordinates remain
  distinct even at the coarsest octave and no axis-aligned period seam exists.
- ISO maps to `(1 + ISO / 2665) / 800`. The three exact frequency/amplitude
  pairs are listed above. Amount maps from 0–100 to 0–1 before the upstream
  exposure-noise factor `0.15`.
- Rec.2020 luminance uses `(0.2627002, 0.6779981, 0.0593017)`. The paper model
  is evaluated on CIE lightness of that luminance, the perceptual density the
  upstream implementations work in, with the inverse paper response taken on
  safe density `[0.00001, 0.99999]`. The developed *delta* relative to that
  safe density moves the lightness, and the resulting luminance change is
  applied to the original scene-linear RGB as a ratio, so a pixel keeps its
  colour and black stays black. Evaluated on linear values instead, deep
  shadows sat where the paper curve is steepest and lifted into grey blotches.
  Consequently negative and HDR input remain unbounded; there is no final RGB
  clamp, and straight alpha is never touched.
- Luminance accumulation uses `f64` so cancellation among finite negative/HDR
  Rec.2020 channels does not overflow an intermediate `f32`. Active grain on
  any valid finite `f32` RGB either produces finite, unclamped `f32` RGB or
  returns an explicit `NonFiniteOutput` error; it never silently stores NaN or
  infinity. Inputs at both signs of `f32::MAX` are covered by the kernel tests.
- Amount zero returns before full-image region/dimension validation, pixel
  buffer access, seed, coordinate, luminance, or response work. It allocates
  nothing and is bit-exact, including for shapes outside the active kernel's
  supported dimension contract. Non-zero grain requires an explicit
  `DevelopRenderContext` and is the final operation in the Effects order:
  Bloom, Halation, Fade, Vignette, Sharpness, Grain.

## Render-context integration

`DevelopRenderContext::from_source_digest` accepts an already computed 32-byte
source-content digest and derives a grain seed with the fixed domain
`org.omalux/grain-seed/v1`. The domain moved to the project's own
namespace once; since then identical inputs keep identical rendered grain. The API performs no IO and has no filename,
path, mtime, or global-default input. Tests and golden fixtures may explicitly
construct `ResolvedGrainSeed::fixed_for_tests`; production callers should
prefer the content-digest constructor. Active grain without a context fails at
preflight with `MissingRenderContext(Effects)` before caller-visible mutation.

The GUI preview and production export both run through `DevelopJobRunner`, the
production decoder and the production encoder. The source-content digest is
therefore computed by the held-source core path and supplies the normative
render context for grain. Preview requests are coalesced and revisioned, while
the resulting private JPEG is only a bounded display artifact; it is never an
input to production export.
