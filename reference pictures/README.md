# Reference pictures

Fixed input images for manual visual checks and future pipeline regression comparisons.

- `main.jpg`: AI-generated beach volleyball photograph, 1536 × 1024; generated with the built-in Imagegen tool, then converted to JPEG at quality 98 with 4:4:4 chroma sampling using ImageMagick. It is not a measured color reference. Reuse this file; regenerating the prompt will not reproduce the same pixels.
- `technical/`: six deterministic 16-bit RGB PNGs with an explicit sRGB chunk, the standard-library-only Python generator, and a manifest of values and hashes. Values refer to encoded sRGB, not linear scene radiance. Full-scale endpoints are included where described. There are no labels or overlays inside the test pixels.
- `real pics/`: reserved for real photos and RAW inputs; currently empty.

## Regeneration

From the repository root:

```bash
python3 "reference pictures/technical/generate.py"
```

This deliberately replaces the six technical PNGs and their manifest. Pixel scanline hashes should reproduce; compressed file hashes can depend on the zlib version. The manifest records the exact palette and chart layouts. Technical PNGs are lossless: do not convert them to JPEG for testing. Browser scaling or display color management may alter their appearance; inspect at 100% and measure decoded values when comparing outputs.

No baseline exports or automated regression runner are included yet. These raster inputs do not test the RAW decoder or scene-linear HDR highlights above white. Real photos should be fixed inputs separate from any holdout set used for preset calibration.

## Main image prompt

Generated with the built-in Imagegen tool:

Use case: photorealistic-natural. Asset type: fixed visual regression reference for a photo developer. Generate a high resolution landscape photograph, 1536x1024 or larger, of one large matte multicolored beach volleyball resting on fine natural sand at the shoreline. Clear composition of foreground sand, middle-distance turquoise sea and distant blue sky with a straight horizon and soft white clouds. Ball occupies about 40 percent of image height, entire ball visible, its curved sewn panels show red, orange, yellow, green, cyan, blue, violet and a white panel, as many colors as naturally visible. Realistic fine fabric and sand textures, gentle waves, neutral daylight, defined soft-edged ball shadow with visible shadow detail, smooth sky gradients, subtle bright highlights without extensive clipping. Natural photographic rendering, balanced exposure and moderate saturation, sufficient depth of field for textured sand and sea. No people, text, logos, watermark, frame, collage or color chart. This is an attractive coherent natural beach photograph, not a diagram.
