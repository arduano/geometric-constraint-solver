# Third-party licences and attribution

GeoSolve is licensed under `GPL-3.0-or-later`; see `LICENSE`. Its pre-M70B locked native and
WASM dependency graphs were audited on 2026-07-21 with `cargo-license` and rechecked on
2026-07-22 with `cargo-deny`. The pure-Rust M70B additions and their compatible declared
expressions are recorded below; both platform inventories and `cargo deny check licenses` were
re-run successfully on the nominated M70B source on 2026-08-10.

## Declared dependency licences

The locked graphs contain packages under these SPDX expressions:

- `0BSD OR MIT OR Apache-2.0`;
- `Apache-2.0`;
- `Apache-2.0 OR MIT`;
- `Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT`;
- `Apache-2.0 OR MIT OR Zlib`;
- `(Apache-2.0 OR MIT) AND Unicode-3.0`;
- `Apache-2.0 OR BSD-2-Clause OR MIT`;
- `BSD-2-Clause`;
- `BSD-3-Clause`;
- `MIT`;
- `MIT OR Apache-2.0`;
- `MIT OR Zlib OR Apache-2.0`;
- `MIT OR Unlicense`;
- `Zlib`.

Copyright and complete licence texts remain in each dependency's source package.
The exact package names, versions and checksums are fixed by `Cargo.lock`. Release
audits use:

```bash
cargo license --avoid-dev-deps --all-features \
  --filter-platform x86_64-unknown-linux-gnu --tsv
cargo license --avoid-dev-deps --all-features \
  --filter-platform wasm32-unknown-unknown --tsv
cargo deny check licenses
```

## M70B reproduction transport dependencies

The pure-Rust M70B text transport adds these locked packages and declared SPDX expressions:

- `base64 0.23.1` — `MIT OR Apache-2.0`;
- `miniz_oxide 0.9.1` — `MIT OR Zlib OR Apache-2.0`;
- `adler2 2.0.1` — `0BSD OR MIT OR Apache-2.0`.

They implement strict URL-safe text encoding and zlib/Adler stream handling only. They add no
native library, FFI or `unsafe` block to GeoSolve source. Their exact checksums remain locked in
`Cargo.lock`; both platform inventories and `cargo deny check licenses` pass on the nominated
candidate.

## M87 native renderer dependencies and bundled font

M87's pure-Rust native SVG rasterizer adds these locked packages and declared SPDX expressions:

- `arrayref 0.3.9` — `BSD-2-Clause`;
- `tiny-skia 0.12.0` — `BSD-3-Clause`;
- `tiny-skia-path 0.12.0` — `BSD-3-Clause`.

The native renderer also bundles Share Tech Mono Regular for hermetic text rasterization. The font
is Copyright (c) 2012 Carrois Type Design and Ralph du Carrois, uses the Reserved Font Name
"Share", and is distributed under `OFL-1.1`. The exact decoded font SHA-256 is
`9ceab1f87414829af259c0f537573ae03ef7dd3147c0b27a36a1a0beb6732677`; its copyright and complete
licence text are preserved in `crates/geosolve-sketch-render/assets/README.md` and
`crates/geosolve-sketch-render/assets/ShareTechMono-OFL.txt`.

## M87 atomic headless publication dependency

M87's no-clobber generation publisher adds one direct locked dependency:

- `rustix 1.1.4` — `Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT`.

GeoSolve uses its safe filesystem API for the platform's atomic rename-with-no-replace operation.
Unsupported platforms fail closed instead of falling back to an overwriting rename; no `unsafe`
block is added to GeoSolve source.

## M87 Gridfinity profile reference

The standards-informed `gridfinity-1x1x3-section` sketch derives its dimensional constants and
profile stages from `src/core/standard.scad` in
<https://github.com/kennetek/gridfinity-rebuilt-openscad> at commit
`910e22d8607fd7f5f51ad5e5cbc5287a76810bfd`. GeoSolve translates those values into one managed 2D
contour; it does not vendor or execute OpenSCAD. The reference is licensed under MIT and retains
these upstream notices:

Copyright (c) 2023 Kenneth Hodson

Copyright (c) 2023 Zachary Freedman and Voidstar Lab LLC

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction,
including without limitation the rights to use, copy, modify, merge, publish, distribute,
sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or
substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## M88 browser application dependencies

M88's React/Vite browser application is built from the exact dependency graph in
`crates/geosolve-demo-web/frontend/package-lock.json`. Its published runtime graph uses the
following compatible declared SPDX licences only: `0BSD`, `Apache-2.0`, `ISC` and `MIT`.
The direct runtime families are React/React DOM, CodeMirror 6, Radix UI, Lucide React,
`react-resizable-panels`, `class-variance-authority`, `clsx` and `tailwind-merge`; their transitive
runtime packages are locked and mechanically checked by `npm run check:licenses`.

Build, test and accessibility tooling is not shipped in the browser bundle. Its exact graph and
declared licence metadata are also retained in `package-lock.json`. Copyright and complete licence
texts remain in each dependency's source package. This attribution file, the GeoSolve GPL text and
the public API-compatibility document are copied into every Vite release artifact; the corresponding
GeoSolve source retains the locked build instructions.

## `faer` bundled notices

`faer 0.24.4` declares MIT but its source distribution also carries code and
notices under MPL-2.0 and BSD-3-Clause. GeoSolve uses its sparse linear algebra and
therefore preserves these upstream files in source/binary release attribution:

- `COPYING.EIGEN.MPL2`;
- `COPYING.LAPACK.BSD`;
- `COPYING.SUITE_SPARSE.AMD.BSD`;
- `COPYING.SUITE_SPARSE.COLAMD.BSD`.

The authoritative copies are distributed in the `faer 0.24.4` crate source at
<https://codeberg.org/sarah-quinones/faer>. MPL-2.0 and BSD-3-Clause are compatible
with this GPLv3 work; their notices and source obligations remain in force.

## Reference implementations

`REFERENCES.md` records SolveSpace and PlaneGCS as conceptual and differential
oracles. GeoSolve does not vendor, bind or directly translate their source. If a
future change translates reference code, that change must identify the exact
upstream file/revision and preserve its copyright and licence notice here.

## Release policy

Changing `Cargo.lock` requires rerunning both platform inventories and the
licence allowlist. A browser or binary release must provide this file, the GeoSolve
GPL text, the corresponding tagged source and build instructions. Missing metadata
or an unreviewed licence is a release blocker.

## M94 GPU canvas dependencies
The locked PixiJS 8.20.1 renderer uses the following added browser dependency packages. Its
WebGL2 drawing backend adds no solver FFI. The reviewed BSD-3-Clause packages are GPLv3-compatible;
the runtime licence checker now admits that SPDX identifier alongside its existing set.
- `@pixi/colord 2.9.6` — `MIT`;
- `@types/earcut 3.0.0` — `MIT`;
- `@webgpu/types 0.1.72` — `BSD-3-Clause`;
- `@xmldom/xmldom 0.8.15` — `MIT`;
- `earcut 3.2.3` — `ISC`;
- `eventemitter3 5.0.4` — `MIT`;
- `gifuct-js 2.1.2` — `MIT`;
- `ismobilejs 1.1.1` — `MIT`;
- `js-binary-schema-parser 2.0.3` — `MIT`;
- `parse-svg-path 0.2.0` — `MIT`;
- `pixi.js 8.20.1` — `MIT`;
- `tiny-lru 11.4.7` — `BSD-3-Clause`;

The exact registry integrity identities remain in package-lock.json. PixiJS tree-shaking excludes
unused importers, but all newly locked packages retain attribution here. Upstream notices:

### @types/earcut 3.0.0

```text
    MIT License

    Copyright (c) Microsoft Corporation.

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
    SOFTWARE
```

### @webgpu/types 0.1.72

```text
Copyright 2022 WebGPU Developers

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

   1. Redistributions of source code must retain the above copyright notice,
      this list of conditions and the following disclaimer.

   2. Redistributions in binary form must reproduce the above copyright notice,
      this list of conditions and the following disclaimer in the documentation
      and/or other materials provided with the distribution.

   3. Neither the name of the copyright holder nor the names of its
      contributors may be used to endorse or promote products derived from this
      software without specific prior written permission.

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
```

### @xmldom/xmldom 0.8.15

```text
Copyright 2019 - present Christopher J. Brody and other contributors, as listed in: https://github.com/xmldom/xmldom/graphs/contributors
Copyright 2012 - 2017 @jindw <jindw@xidea.org> and other contributors, as listed in: https://github.com/jindw/xmldom/graphs/contributors

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

### earcut 3.2.3

```text
ISC License

Copyright (c) 2026, Mapbox

Permission to use, copy, modify, and/or distribute this software for any purpose
with or without fee is hereby granted, provided that the above copyright notice
and this permission notice appear in all copies.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND
FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS
OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER
TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF
THIS SOFTWARE.
```

### eventemitter3 5.0.4

```text
The MIT License (MIT)

Copyright (c) 2014 Arnout Kazemier

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

### gifuct-js 2.1.2

```text
The MIT License (MIT)

Copyright (c) 2015 Matt Way

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

### ismobilejs 1.1.1

```text
MIT License

Copyright (c) 2019 Kai Mallea

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

### js-binary-schema-parser 2.0.3

```text
The MIT License (MIT)

Copyright (c) 2015 Matt Way

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

### parse-svg-path 0.2.0

```text

The MIT License

Copyright (c) 2013 Jake Rosoman <jkroso@gmail.com>

Permission is hereby granted, free of charge, to any person obtaining
a copy of this software and associated documentation files (the
'Software'), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish,
distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to
the following conditions:

The above copyright notice and this permission notice shall be
included in all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE
SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
```

### pixi.js 8.20.1

```text
The MIT License

Copyright (c) 2013-2023 Mathew Groves, Chad Engler

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

### tiny-lru 11.4.7

```text
Copyright (c) 2026, Jason Mulligan
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

* Redistributions of source code must retain the above copyright notice, this
  list of conditions and the following disclaimer.

* Redistributions in binary form must reproduce the above copyright notice,
  this list of conditions and the following disclaimer in the documentation
  and/or other materials provided with the distribution.

* Neither the name of tiny-lru nor the names of its
  contributors may be used to endorse or promote products derived from
  this software without specific prior written permission.

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
```

### @pixi/colord — upstream MIT notice

Source: https://github.com/omgovich/colord/blob/master/LICENSE.md

```text
MIT License

Copyright (c) 2020 Vlad Shilov omgovich@ya.ru

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
