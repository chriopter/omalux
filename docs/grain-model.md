# Grain model and provenance

Grainroom's normative CPU grain is an adaptation of the film-grain model shared
by darktable and RawTherapee. The existing preview shader was independently
written from the same model; neither upstream application implements it as a
GLSL shader.

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

The CPU kernel and shader use a compact simplex-noise formulation instead of the
upstream CPU permutation table. The simplex algorithm is by Stefan Gustavson;
the scalar Rust operation order and vectorized GLSL are based on the
MIT-licensed Ashima Arts `webgl-noise` snapshot
[`6abed1e77ed1e18b181627c35f688eb30c9fe75e`](https://github.com/ashima/webgl-noise/tree/6abed1e77ed1e18b181627c35f688eb30c9fe75e).
Its full copyright and MIT permission notice is retained verbatim in
[`THIRD_PARTY_NOTICES.md`](../THIRD_PARTY_NOTICES.md). The GPL notices on
the adapted grain-model files remain in place.

## Normative CPU contract

- `ResolvedGrainSeed` is a resolved 64-bit identity value. SplitMix64 expands it
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
- Rec.2020 luminance uses `(0.2627002, 0.6779981, 0.0593017)`. The inverse paper
  response is evaluated on safe density `[0.00001, 0.99999]`. Only the developed
  paper *delta* relative to that safe density is added equally to the original
  scene-linear RGB. Consequently negative and HDR input remain unbounded; there
  is no final RGB clamp, and straight alpha is never touched.
- Luminance accumulation uses `f64` so cancellation among finite negative/HDR
  Rec.2020 channels does not overflow an intermediate `f32`. Active grain on
  any valid finite `f32` RGB either produces finite, unclamped `f32` RGB or
  returns an explicit `NonFiniteOutput` error; it never silently stores NaN or
  infinity. Inputs at both signs of `f32::MAX` are covered by the kernel tests.
- Amount zero returns before seed, coordinate, luminance, or response work and
  is bit-exact.

## Foundation integration gap

The current Foundation `DevelopPipeline::process` has no render context and
therefore cannot carry a resolved image-stable grain seed. WP4 deliberately
keeps non-zero grain unsupported in the Effects stage instead of deriving a
seed from a filename/path or installing a universal placeholder. Integration
must add an explicit render context containing `ResolvedGrainSeed`, thread it
through preflight/process, and only then call the already implemented
`grain::apply_full_image`. Tests may use `ResolvedGrainSeed::fixed` directly.

The legacy preview shader is explicitly non-normative. In addition to
attenuating procedural detail according to the on-screen source-pixel
footprint, it still uses the older `+1.0` permutation polynomial while pinned
Ashima commit `6abed1e…` and the normative CPU use `+10.0`. GPU migration must
port the exact `mod289`, `permute`, skew/unskew, dot-product and accumulation
operation order; provide the six SplitMix64-derived phases and global f64
pixel-center/lattice decomposition; and replace display-RGB luminance, scalar
float seed, and final `[0,1]` clamp with this CPU contract. Independent GLSL
reference vectors and multi-seed 2-D PSD gates must pass before that shader is
called export-parity rendering.
