# Grain model and provenance

Grainroom's grain shader is a GPU adaptation of the film-grain model shared by
darktable and RawTherapee. It is not copied from an existing GLSL shader: both
applications implement this effect on the CPU.

Primary references:

- darktable, `src/iop/grain.c` — original film-grain implementation, copyright
  darktable developers and licensed GPL-3.0-or-later:
  https://github.com/darktable-org/darktable/blob/master/src/iop/grain.c
- RawTherapee, `rtengine/ipgrain.cc` — its documented port and extension of the
  darktable implementation, copyright Alberto Griggio, Jacques Desmis and the
  named darktable authors, licensed GPL-3.0-or-later:
  https://github.com/RawTherapee/RawTherapee/blob/dev/rtengine/ipgrain.cc

The adapted model retains these defining properties:

1. Three noise octaves using frequencies `0.4910`, `0.9441`, `1.7280` and
   amplitudes `0.2340`, `0.7850`, `1.2150`, fitted by darktable to the power
   spectrum of real grain scans.
2. Grain scale expressed as an approximate film ISO.
3. A nonlinear photographic-paper response, rather than a linear noise overlay.
4. A mid-tone bias that reduces the effect in shadows and highlights.
5. A deterministic seed derived from the filename, following darktable's
   reverse filename hashing idea.

The shader uses a compact GPU simplex-noise formulation instead of the CPU
permutation-table implementation. The simplex algorithm is by Stefan Gustavson;
the vectorized GLSL formulation is based on the MIT-licensed Ashima Arts
`webgl-noise` implementation:
https://github.com/ashima/webgl-noise

Grainroom additionally attenuates procedural detail according to the on-screen
source-pixel footprint. This approximates darktable's zoomed-out filtering and
prevents grain aliasing in fitted previews.
