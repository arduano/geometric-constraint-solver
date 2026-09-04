<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Bundled sample assets

Each child directory is one canonical M92 sample and contains `manifest.json`, `sketch.ts`,
`sketch.compiled.json`, `witnesses.json`, and `NOTICE.md` when any provenance record sets
`notice_required`.

The manifest format is `geosolve-bundled-sample-v1`. It owns `ordinal`, `key`, `title`, `summary`,
`category`, `expected`, ordered `groups`, and `provenance`. Categories are `mechanism`,
`product_fabrication`, `reference_lab`, and `scale_study`. Expected mobility has `raw_dof` and
`effective_dof`; raw DOF is both numerical right nullity and equality DOF, while effective DOF is
bidirectional bounded DOF.

Provenance entries use relationship `original`, `dimensions_only`, or `adapted`, plus `name`,
`url`, immutable `revision`, `path`, `licence`, `scope`, and `notice_required`. External
relationships require the URL, revision, and path fields. Original relationships omit them.

The build fails before compiling the library if catalog order/distribution, required files,
source authentication, V3 compiler authority, ordered groups, exactly-once declaration ownership,
mobility, or provenance is invalid. It then emits bounded compressed compiler envelopes and the
Rust registry. To generate or check the frontend projection, run:

```text
cargo run --locked -p geosolve-sketch-code --example generate_frontend_samples -- --write <path>
cargo run --locked -p geosolve-sketch-code --example generate_frontend_samples -- --check <path>
```
