<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 implementation — production-quality constraint annotations

Status: active. Implementation, clean release qualification and immutable byte-verified Tailscale
nomination pass. Human UAT, explicit approval and GitHub Pages publication remain open.

## Implementation ledger

- [x] Editor-owned annotation layout state and semantic placement forms.
- [x] Exact shared paint/hit primitives and deterministic automatic layout.
- [x] Seven dimension-family renderers and compact/reference formatting.
- [x] Twenty contextual constraint categories plus Display “show all”.
- [x] Select-mode movement, cancellation and selected/global reset.
- [x] Workspace v6 optional cache with v1-v5 migration and fail-soft recovery.
- [x] Native/WASM/demo regressions.
- [x] Complete clean release qualification.
- [x] Freeze and byte-verify an immutable Tailscale nomination without rebuilding.
- [ ] Complete human UAT and receive explicit approval.
- [ ] Publish the exact approved candidate to GitHub Pages and close M76.

## Implemented behavior

- `SceneAnnotationGeometry` owns exact linear/radial/angular paths, witnesses, leaders,
  arrowheads, label bounds and compact-glyph bounds consumed by both painting and picking.
- Automatic layout deterministically reserves points, curve corridors, viewport margins, fixed
  right-angle marks, prior annotations and manual placements. Genuine finite perpendicular corners
  retain fixed squares; other marks receive geometry-derived local rotation and movable leaders.
- All seven dimension families use compact four-significant-digit CAD notation and all twenty
  constraint categories have an ordinary editable sampler. The canvas title/ARIA text and the
  property inspector retain full semantic family, mode, value and unit descriptions.
- Typed layout moves preview after 3 px, commit only on pointer-up, cancel on every interaction
  invalidation path, and remain solve-, revision-, branch- and Sketch Undo/Redo-neutral.
- Workspace v6 stores an optional self-versioned cache. v1-v5 migrate empty; wrong outer types,
  versions, rows, identities, sources, marker indices, kinds, placement forms and non-finite values
  are discarded without rejecting otherwise-valid sketch data.

## Focused evidence

- `cargo test --locked -p geosolve-constraint-editor --lib` — 353 passed.
- `cargo test --locked -p geosolve-constraint-editor --test m76_annotation_parity` — 4 passed.
- `nix-shell shell.nix --run 'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m76_annotation_parity --target wasm32-unknown-unknown'` — 4 passed.
- `cargo test --locked -p geosolve-demo-web --lib` — 122 passed.
- `cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web --all-targets --all-features -- -D warnings` — passed.
- `cargo fmt --all -- --check`, `git diff --check` and
  `./scripts/golden-authoring-scene-oracle.sh --check` — passed; the reviewed 270-row oracle is
  unchanged.

## Clean qualification and immutable nomination

Exact product source `37eade50b566f62905a395655bc80c17d9b6bef4`, tree
`d6ad2f453d672accbcc3848a1a16d2039b3511d1`, passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The clean command exited 0 after 503 seconds. Editor tests pass 353/353, demo-web tests pass
122/122, M76 parity passes 4/4 natively and 4/4 under WASM, M75 passes 11/11 in each environment,
M74 passes 5/5 in each environment, and the reviewed golden remains unchanged at 270/270. The
149.39-second sparse crossover, warnings-denied Rustdoc, licensing/package-content checks,
benchmarks and Trunk 0.21.14 release assembly also pass.

The gate-produced distribution was copied without rebuilding to
`/tmp/geosolve-m76-uat.puiPgO`. The directory is mode `0555`; its seven regular non-symlink files
are mode `0444`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 19,296 | `ab28a050a47a4b64fd20f6b821658246444528eee9b0c4499627af381427b72f` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-bcd530e5985506f3.js` | 33,221 | `d8974bcac131556374933e638799af9ee854a913e819e5ce492c9c0707547e0a` |
| `geosolve-demo-web-bcd530e5985506f3_bg.wasm` | 6,273,503 | `daf31f0a65459d3a6158feaa95d4c91c844417755a5071a9b8195c3ef9e02809` |
| `index.html` | 28,226 | `426d1c517dceebfdd80d48e5efa8a5931010bfd66b41264d655626d84c5d15f1` |
| `styles-c6136ab1b6e8294a.css` | 32,376 | `f1d2d6327d3e2af504d4e3d3a9e371b25040b114e23cf138d2e415a5835b539a` |

Its C-locale ordered-manifest aggregate is
`fb18b7c2387b9cea4bb681cac124f6ef9e63180ff071a734e80d27ac8cd83bdf`. PID `1077092` serves the
snapshot at `http://100.94.63.83:8080/`; retained command-runner session `11404` and log
`/tmp/geosolve-m76-uat.puiPgO.server.log` own the live process. Old M75 PID `37152` was retired.

Proxy/cache-bypassed identity requests for `/` and all seven files return HTTP 200 with exact
media types, lengths and bytes, no redirects or content encoding; `/` equals `index.html`, and the
fetched aggregate matches. Evidence is retained at `/tmp/geosolve-m76-http-verify.noWQO3`.

This is mechanical nomination only. It disposes no item in `docs/M76_UAT.md`; U1-U4, explicit
approval, exact GitHub Pages publication and M76 closure remain open. Accepted M75 remains public
authority.
