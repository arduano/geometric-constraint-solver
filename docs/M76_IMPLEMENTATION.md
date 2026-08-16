<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M76 implementation — production-quality constraint annotations

Status: **complete (2026-08-17); final clean-qualified candidate frozen and byte-verified on
Tailscale, scoped human approval recorded, and exact GitHub Pages publication verified**. The
caller reviewed the initial candidate, requested two final feature refinements and explicitly
authorized closure without a separate post-refinement UAT. U1-U4 are accepted under that scoped
approval; this record does not invent individually replayed observations. No later milestone is
active.

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
- [x] Publish the approved candidate to GitHub Pages and verify the exact public artifact.

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

Exact final clean-qualified source `a7769e4107ab6a62b439d3cfaf0b1f779cbdd22b`, tree
`248cba4509a992aeff7a02dd6d57a1a2481380a4`, passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

Editor tests pass 353/353, demo-web tests pass 122/122, M76 parity passes 5/5 natively and 5/5 under
WASM, M75 passes 11/11 in each environment, M74 passes 5/5 in each environment, and the reviewed
golden remains unchanged at 270/270. The 151.76-second sparse crossover, formatting,
warnings-denied Clippy and Rustdoc, locked workspace tests, benchmarks, licensing/package-content
checks and Trunk 0.21.14 release assembly also pass. This exact source includes separate M22
property-oracle test hardening and milestone-neutral shared-runner performance-gate hardening;
neither changes M76 product behavior.

The gate-produced distribution was copied without rebuilding to
`/tmp/geosolve-m76-final-uat.65Y8J1`. The directory is mode `0555`; its exactly seven regular
non-symlink files are mode `0444`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 20,587 | `24934e6d620dc89078ab41c155acd2a31bba4260a82cfe4c37077421cc1ab853` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-55053c1ba5c6df34.js` | 33,221 | `d8974bcac131556374933e638799af9ee854a913e819e5ce492c9c0707547e0a` |
| `geosolve-demo-web-55053c1ba5c6df34_bg.wasm` | 6,273,395 | `ceed21fb5467d43e0ca603521c4c54602458c85f0736f6c129ae63303c01b53b` |
| `index.html` | 28,226 | `92a5e926448e82d05e3d84f1c0044513c6b70125d798374ec03fc205149ae1a5` |
| `styles-c2e1aed7dc61439c.css` | 31,750 | `69e4241bdcafc260ec6248ecc0a94f0cdb6420155419dd103a30d49ee1d467ee` |

Its C-locale ordered-manifest aggregate is
`967f0c1943c16b9c4a9975aeb973ad0cfe2c6e3dbfab45f414d0dac1bb9088f3`. PID `1780608`, retained
command-runner session `30164`, serves the exact snapshot at `http://100.94.63.83:8080/`.

Proxy/cache-bypassed identity requests for `/` and all seven files return HTTP 200 with exact media
types, lengths and bytes, no redirects or content encoding; `/` equals `index.html`, and the fetched
aggregate matches. Evidence is retained at `/tmp/geosolve-m76-final-http-verify.UwoaMK`.

The prior clean-qualified source `9b4e7f72dcacefdf4d7847a22eb675c711068d26`, snapshot
`/tmp/geosolve-m76-uat.ctgYzp` and aggregate
`337b0e6a2ce2b6a9aed979d0a4849e2d0887c092df66efa345d4917929d01dd4` are superseded historical
evidence. Its server PID `1455071` was retired before the final snapshot took the shared endpoint.

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

## Performance-gate disposition

GitHub Pages run `31957299907` failed both attempts solely at the former 180-second elapsed-time
assertion: 209.696267408 seconds in job `95189757773` and 208.757508921 seconds in job
`95194183206`. Every convergence, validity, rank, `SparseQr`, fallback and residual assertion
passed first. This is infrastructure-sensitive timing evidence, not an M76 defect, and no
`M76-Fxxx` ID was assigned.

Commit `a7769e4` leaves 180 seconds as the documented advisory reference target and enforces a
240-second shared-runner release ceiling. Solver behavior, workload, backend, rank and tolerances
are unchanged. The focused test passes locally, the final complete local gate passes at 151.76
seconds, and the successful hosted run passes at 184.090683967 seconds while emitting the expected
advisory.

## Final GitHub Pages publication

Exact source `a7769e4107ab6a62b439d3cfaf0b1f779cbdd22b`, tree
`248cba4509a992aeff7a02dd6d57a1a2481380a4`, passes GitHub Pages run `31961652265`.
Qualify-and-assemble job `95200423007` passes the complete release gate and repository-prefixed
artifact build; deploy job `95204687455` passes through deployment `5933831093` at
`https://arduano.github.io/geometric-constraint-solver/`.

Artifact `9267811418`, name `github-pages`, is 2,164,829 bytes. Its ZIP SHA-256 matches GitHub's
digest at `dba7e2f5e1b7a51390ec1d840e7869d69968114bcf13250e641448a02d0cb60b`; the 6,440,960-byte
inner tar has SHA-256 `be18173d61fef8ead3d00cf2dd560f893a7731eff7fa3bdfc0b81aadab6298e5`.
It contains exactly seven regular files:

| Final hosted artifact file | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 20,587 | `24934e6d620dc89078ab41c155acd2a31bba4260a82cfe4c37077421cc1ab853` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-d5065a13c9f32407.js` | 33,221 | `d8974bcac131556374933e638799af9ee854a913e819e5ce492c9c0707547e0a` |
| `geosolve-demo-web-d5065a13c9f32407_bg.wasm` | 6,273,316 | `238402721486b606f57aa06857479513ed7ebe2db33b40f6cdcc94db1596a05f` |
| `index.html` | 28,366 | `b161c45f32b635a9c424f959ec02ada376d3862c094b2d34fe64e006735d3f7f` |
| `styles-c2e1aed7dc61439c.css` | 31,750 | `69e4241bdcafc260ec6248ecc0a94f0cdb6420155419dd103a30d49ee1d467ee` |

The C-locale seven-file manifest aggregate is
`41e2a69d55a3232702b1ae429611c6d8351fd9041b970391f815a37078e9fa96`. Public root and all seven
paths return HTTP 200 with zero redirects, exact expected media types and artifact-identical bytes;
`/` equals `index.html`, and asset references are repository-prefixed. Verification evidence is
retained at `/tmp/geosolve-m76-pages-verify.ijOz7p` (an independent repeat is at
`/tmp/geosolve-m76-pages-verify.hVSqQJ`). Pages rebuilds with the repository prefix, so byte
identity with the separately frozen Tailscale artifact is neither expected nor claimed.

The unchanged M72 public browser script passes at `1440x900` and `1024x720`. The original retained
M74/M75 scripts time out only because they still require the deliberately removed Origin canvas
ring. M76-adapted temporary copies change only those obsolete Origin-canvas expectations to require
two canvas axes, no Origin ring and an inapplicable axes intersection. Hashes
`4aff982c6a9e10702d7b0179c17682c6904bb6c28362ebefe967705a984c3355` and
`161e96d541dbcc189dbbc23c47da672e3080b7c7646e45c11ef458a5e521a067` pass M74 at both desktop
sizes and M75 6/6. GitHub Pages is final public-byte authority; the Tailscale snapshot remains the
live frozen candidate for easy follow-up through the completed closeout handoff.
