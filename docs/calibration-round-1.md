# Calibration, round one

The built-in looks were tuned against target renderings of 29 images, 19 of
them used for tuning and 10 held out and judged once at the end. The tools
live in `tools/calibration/`; the data does not live in the repository.

| | ΔE00 mean | SSIM |
|---|---|---|
| before | 9.52 | 0.805 |
| tuning images | 3.27 | 0.967 |
| holdout images | 4.00 | 0.934 |

What it took, on the renderer side: a tone curve driven by a power norm, a
colour table stage with a strength control, noise reduction, colour that
drains towards white in the RAW base rendition, the DNG radial vignette
opcode, the grain paper model on lightness, and a master curve that crushes
rather than inverts below zero. Every look was then re-tuned; four crush
looks needed a neutral starting point instead of their archived values.

Still weak on unseen images: blitz (a high-key look whose target treats
raster and RAW sources differently), kerry, desert-signal, meadow-cross and
kirche-2. On the camera side, per-camera exposure and the vignette opcode on
one phone remain open.
