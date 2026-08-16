<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 implementation — production-quality constraint annotations

Status: **approved for scoped closure (2026-08-17); final clean-qualified candidate frozen and
byte-verified on Tailscale**. The caller reviewed the initial candidate, requested two final
feature refinements and explicitly authorized closure without a separate post-refinement UAT.
U1-U4 are accepted under that scoped approval; this record does not invent individually replayed
observations. GitHub Pages publication and exact public-byte verification remain the final standard
closeout step. No later milestone is active.

## Implementation ledger

- [x] Editor-owned annotation layout state and semantic placement forms.
- [x] Exact shared paint/hit primitives and deterministic automatic layout.
- [x] Seven dimension-family renderers and compact/reference formatting.
- [x] Twenty contextual constraint categories plus Display “show all”.
- [x] Select-mode movement, cancellation and selected/global reset.
- [x] Workspace v6 optional cache with v1-v5 migration and fail-soft recovery.
- [x] Native/WASM/demo regressions.
- [x] Put shared-endpoint acute/right angle annotations in the actual finite-ray interior wedge.
- [x] Remove the redundant canvas Origin marker while retaining protected datum semantics.
- [x] Complete clean release qualification.
- [x] Freeze and byte-verify an immutable Tailscale candidate without rebuilding.
- [x] Receive explicit scoped approval, including U1-U4 and no separate follow-up UAT.
- [ ] Publish the approved candidate to GitHub Pages and verify the exact public artifact.

## Implemented behavior

- `SceneAnnotationGeometry` owns exact linear/radial/angular paths, witnesses, leaders,
  arrowheads, label bounds and compact-glyph bounds consumed by both painting and picking.
- Automatic layout deterministically reserves points, curve corridors, viewport margins, fixed
  right-angle marks, prior annotations and manual placements. Genuine finite perpendicular corners
  retain fixed squares; other marks receive geometry-derived local rotation and movable leaders.
- All seven dimension families use compact four-significant-digit CAD notation and all twenty
  constraint categories have an ordinary editable sampler. Canvas title/ARIA text and the property
  inspector retain full semantic family, mode, value and unit descriptions.
- Typed layout moves preview after 3 px, commit only on pointer-up, cancel on every interaction
  invalidation path, and remain solve-, revision-, branch- and Sketch Undo/Redo-neutral.
- Workspace v6 stores an optional self-versioned cache. v1-v5 migrate empty; wrong outer types,
  versions, rows, identities, sources, marker indices, kinds, placement forms and non-finite values
  are discarded without rejecting otherwise-valid sketch data.

## Final feature refinements

Shared-endpoint line angles now choose the wedge formed by the actual finite rays when that wedge
is consistent with the accepted acute or right-angle value. Arc, label, arrowheads and hit geometry
share that choice, and the vertically opposite wedge does not hit. An obtuse finite join continues
to show the accepted acute supporting-line angle. The previous side choice predated M76; this is an
M76 presentation feature refinement, not an `M76-Fxxx` defect.

Origin remains a permanent, protected, selectable and inference-capable intrinsic datum in the
headless scene and remains available through the Reference geometry tree and inspector. The canvas
no longer draws its redundant ring, cross, text or focus target: the intersection of the permanent
X/Y axes communicates zero without a competing annotation.

The feature refinements were committed as
`a9fd6f6a71edf5be9d9fb5856074d291192a898d`, tree
`2627e1d0ffdc500166bbcee50626fc9d65e05b67`.

## Focused and compatibility evidence

- `cargo test --locked -p geosolve-constraint-editor --lib` — 353/353 passed.
- `cargo test --locked -p geosolve-constraint-editor --test m76_annotation_parity` — 5/5 passed.
- The same M76 parity target under `wasm32-unknown-unknown` with
  `wasm-bindgen-test-runner` — 5/5 passed.
- `cargo test --locked -p geosolve-demo-web --lib` — 122/122 passed.
- Focused warnings-denied Clippy passed.
- `cargo fmt --all -- --check`, `git diff --check` and
  `./scripts/golden-authoring-scene-oracle.sh --check` passed; the reviewed 270-row oracle is
  unchanged.

The new parity regression failed against the old endpoint-order behavior before the feature
correction, then passed. It freezes vertex, finite rays, chosen bisector, arc hit geometry,
opposite-wedge exclusion, visible value and arrowheads.

## Final clean qualification and immutable candidate

Exact clean-qualified source `9b4e7f72dcacefdf4d7847a22eb675c711068d26`, tree
`e0591664fbeb2e353bc880dd826dc39ac1caeec9`, passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The clean command exited 0 after 569 seconds. Editor tests pass 353/353, demo-web tests pass
122/122, M76 parity passes 5/5 natively and 5/5 under WASM, M75 passes 11/11 in each environment,
M74 passes 5/5 in each environment, and the reviewed golden remains unchanged at 270/270. The
127.63-second sparse crossover, formatting, warnings-denied Clippy and Rustdoc, locked workspace
tests, benchmarks, licensing/package-content checks and Trunk 0.21.14 release assembly also pass.
This exact source includes separate M22 property-oracle test hardening; it does not change M76
product behavior.

The gate-produced distribution was copied without rebuilding to
`/tmp/geosolve-m76-uat.ctgYzp`. The directory is mode `0555`; its exactly seven regular
non-symlink files are mode `0444`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 19,296 | `ab28a050a47a4b64fd20f6b821658246444528eee9b0c4499627af381427b72f` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-55053c1ba5c6df34.js` | 33,221 | `d8974bcac131556374933e638799af9ee854a913e819e5ce492c9c0707547e0a` |
| `geosolve-demo-web-55053c1ba5c6df34_bg.wasm` | 6,273,395 | `ceed21fb5467d43e0ca603521c4c54602458c85f0736f6c129ae63303c01b53b` |
| `index.html` | 28,226 | `92a5e926448e82d05e3d84f1c0044513c6b70125d798374ec03fc205149ae1a5` |
| `styles-c2e1aed7dc61439c.css` | 31,750 | `69e4241bdcafc260ec6248ecc0a94f0cdb6420155419dd103a30d49ee1d467ee` |

Its C-locale ordered-manifest aggregate is
`337b0e6a2ce2b6a9aed979d0a4849e2d0887c092df66efa345d4917929d01dd4`. PID `1455071`, retained
command-runner session `70653`, serves the exact snapshot at `http://100.94.63.83:8080/`.

Proxy/cache-bypassed identity requests for `/` and all seven files return HTTP 200 with exact media
types, lengths and bytes, no redirects or content encoding; `/` equals `index.html`, and the fetched
aggregate matches. Evidence is retained at `/tmp/geosolve-m76-http-verify.CqEufj`.

The initial nomination at source `37eade50b566f62905a395655bc80c17d9b6bef4`, tree
`d6ad2f453d672accbcc3848a1a16d2039b3511d1`, snapshot
`/tmp/geosolve-m76-uat.puiPgO`, aggregate
`fb18b7c2387b9cea4bb681cac124f6ef9e63180ff071a734e80d27ac8cd83bdf`, is superseded historical
evidence only. Its server PID `1077092` is retired.

## Scoped approval record

The caller reviewed the initial candidate and reported that it looked good, then requested the
angle-side and Origin refinements above. After implementation and automated review, the caller
explicitly authorized closing M76 without a separate follow-up UAT. That authority accepts U1-U4
for scoped closure; it is not represented as a detailed post-refinement hands-on transcript.

GitHub Pages remains on accepted M75 until the approved M76 descendant is published and verified.
The Tailscale snapshot is final candidate evidence, not public-byte authority.
