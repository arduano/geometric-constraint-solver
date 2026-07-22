# Third-party licences and attribution

GeoSolve is licensed under `GPL-3.0-or-later`; see `LICENSE`. Its locked native and
WASM dependency graphs were audited on 2026-07-21 with `cargo-license` and rechecked
on 2026-07-22 with `cargo-deny`. No declared dependency licence is incompatible with
GPLv3.

## Declared dependency licences

The locked graphs contain packages under these SPDX expressions:

- `Apache-2.0`;
- `Apache-2.0 OR MIT`;
- `Apache-2.0 OR MIT OR Zlib`;
- `(Apache-2.0 OR MIT) AND Unicode-3.0`;
- `Apache-2.0 OR BSD-2-Clause OR MIT`;
- `MIT`;
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
