# Third-party notices

## Optional HEIC backend

With the `heic` feature Omalux uses `libheif-sys` 5.3.1+1.23.1 (MIT) as
generated bindings to the dynamically linked system libheif 1.23.1
(LGPL-3.0-or-later). HEVC encoding is selected explicitly through the system
x265 plugin/library (GPL-2.0-or-later). Omalux does not copy or bundle their
source or binaries. Their complete license texts accompany their source
distributions. Codec copyright licenses do not grant patent rights; HEVC
distribution requires an independent legal review.

## Ashima Arts webgl-noise

The deterministic film-grain kernel contains a scalar Rust adaptation of the
2-D simplex-noise GLSL from Ashima Arts `webgl-noise`, pinned at commit
`6abed1e77ed1e18b181627c35f688eb30c9fe75e`.

```
Copyright (C) 2011 by Ashima Arts (Simplex noise)
Copyright (C) 2011-2016 by Stefan Gustavson (Classic noise and others)
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:
The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
```

## rust-lcms2 safe Rust wrapper

Omalux uses `lcms2` 6.1.1, Copyright (c) Kornel Lesiński, under the MIT
License:

```
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## Little CMS 2

The dynamically linked Little CMS 2 library is MIT licensed, Copyright (c)
2023 Marti Maria Saguer. Its license terms are the MIT terms reproduced in the
`rust-lcms2` section above.

## Raster codec crates

Omalux uses the following Rust crates through their public APIs for bounded
JPEG, PNG, and BMP decoding and JPEG encoding. Omalux does not copy their
codec source:

- `image` 0.25.10 — MIT OR Apache-2.0
- `png` 0.18.1, Copyright (c) 2015 nwin — MIT OR Apache-2.0
- `flate2` 1.1.9, Copyright (c) 2014-2026 Alex Crichton — MIT OR Apache-2.0
- `crc32fast` 1.5.1, Copyright (c) 2018 Sam Rijs, Alex Crichton and contributors — MIT OR Apache-2.0
- `zune-jpeg` 0.5.15 and `zune-core` 0.5.3, Copyright (c) zune-image developers — MIT OR Apache-2.0 OR Zlib
- `fdeflate` 0.3.7 — MIT OR Apache-2.0
- `miniz_oxide` 0.8.9, Copyright 2013-2014 RAD Game Tools and Valve Software,
  Copyright 2010-2014 Rich Geldreich and Tenacious Software LLC, Copyright (c)
  2017 Frommi, Copyright (c) 2017-2024 oyvindln — MIT OR Zlib OR Apache-2.0
- `adler2` 2.0.1, Copyright (c) Jonas Schievink — 0BSD OR MIT OR Apache-2.0
- `simd-adler32` 0.3.10, Copyright (c) 2021 Marvin Countryman — MIT
- `byteorder-lite` 0.1.0, Copyright (c) 2015 Andrew Gallant — Unlicense OR MIT
- `moxcms` 0.8.1, Copyright (c) Radzivon Bartoshyk — BSD-3-Clause OR Apache-2.0
- `pxfm` 0.1.30, Copyright (c) Radzivon Bartoshyk — BSD-3-Clause OR Apache-2.0
- `bytemuck` 1.25.2, Copyright (c) 2019 Daniel "Lokathor" Gee — Zlib OR Apache-2.0 OR MIT
- `num-traits` 0.2.19, Copyright (c) 2014 The Rust Project Developers — MIT OR Apache-2.0
- `bitflags` 2.13.1, Copyright (c) 2014 The Rust Project Developers — MIT OR Apache-2.0
- `cfg-if` 1.0.4, Copyright (c) 2014 Alex Crichton — MIT OR Apache-2.0

The MIT license text is reproduced above. The Apache-2.0 and Zlib alternatives
are available in each crate's published source distribution; Omalux selects
the MIT option where offered and BSD-3-Clause for `moxcms` and `pxfm`.

### moxcms and pxfm BSD-3-Clause license

Copyright (c) Radzivon Bartoshyk. All rights reserved.

Redistribution and use in source and binary forms, with or without modification,
are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software without
   specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## Ctrl-C handling and platform support crates

The Qt-free CLI uses `ctrlc` for cooperative SIGINT cancellation. Its direct
and platform-specific transitive crates are:

- `ctrlc` 3.5.2, authored by Antti Keränen — MIT OR Apache-2.0;
- `nix` 0.31.3, the rust-nix project developers — MIT;
- `cfg_aliases` 0.2.2, authored by Zicklag — MIT;
- `libc` 0.2.189, the Rust libc developers — MIT OR Apache-2.0;
- `dispatch2` 0.3.1 — Zlib OR Apache-2.0 OR MIT (Apple targets only);
- `block2` 0.6.2, `objc2` 0.6.4, and `objc2-encode` 4.1.0, authored by
  Mads Marquart and contributors — MIT (Apple targets only);
- `windows-sys` 0.61.2 and `windows-link` 0.2.1, the windows-rs project —
  MIT OR Apache-2.0 (Windows targets only).

`bitflags` and `cfg-if`, also used by these crates, are already listed in the
raster-codec dependency notice. Omalux selects the MIT option wherever it
is offered; the complete MIT terms are reproduced in the rust-lcms2 section
above. Alternative Apache-2.0 and Zlib terms remain in the published crate
source distributions.

## Feather Icons

The GUI bundles three icons (edit, presets, info) from Feather Icons,
Copyright (c) 2013-2023 Cole Bemis, under the MIT license. The complete
MIT terms are reproduced in the rust-lcms2 section above.
