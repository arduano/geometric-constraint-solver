<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 implementation — Retained drafting relations

Status: M71-F004 simultaneous endpoint-axis inference is implemented, clean-qualified and
published as a byte-verified immutable replacement. Supervising-human UAT remains pending.

Architecture owner: ADR 0035

Withdrawn pre-F003 candidate source: `ad01912eac28275644dcfc867a2dc70030b5406d`

Withdrawn F003 candidate source: `83bd2b575784c44b618fb3ad144f24e84702d764`

F004 clean product source: `a2e51efba7d79f684d264094ffd7dd0e37a4d089`

F004 clean product tree: `8b73be00a384fe4a36ebe13fa0c06f32a6694a14`

F004 clean release-gate result: **PASS**; log
`/tmp/geosolve-m71-f004-clean-gate.ZGQEKU.log`

Current F004 release distribution: `/tmp/geosolve-m71-f004-uat.SaXMVY`; endpoint
`http://100.94.63.83:8080/`; PID `2848202`; ordered manifest aggregate
`5baf5514f366da60ef9e88d7f53f2e8b0346ff5c5222d8e993529a38272b631b`

Historical F003 clean release-gate result: **PASS**

Withdrawn F003 release distribution, preserved but no longer served:
`/tmp/geosolve-m71-f003-uat.hybK8W`; historical endpoint `http://100.94.63.83:8080/`, ordered
manifest aggregate
`23ab4586acd0f8a86a85e81d7b913ee2736f2524fe81c9913fa3a726496584e0`

Withdrawn Tailscale release distribution (preserved, no longer served and not acceptable for
continued UAT): `/tmp/geosolve-m71-uat.yFBsnX`; historical endpoint
`http://100.94.63.83:8080/`; ordered manifest aggregate
`43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`

Both earlier distributions remain historical evidence. The preserved F003 snapshot remains
unchanged and its former server has exited. The shared Tailscale endpoint now serves only the
verified F004 snapshot above.

## 1. Files and APIs

- `geosolve-sketch::DocumentConstraintDefinition` adds ordinary retained
  `HorizontalPoints`, `VerticalPoints`, `HorizontalPointToMidpoint`,
  `VerticalPointToMidpoint`, `Concentric` and `Collinear` definitions. The midpoint-axis
  definitions carry an explicit stored `DesignPointId` plus a certified native line/polyline
  `CurveSpan`; public `DocumentCenterRef` and `DocumentLineSupportRef` keep semantic center and
  directed affine-support operands explicit.
- `crates/geosolve-sketch/src/document.rs` isolates the frozen canonical-v4 constraint wire DTO.
  Canonical-v4 export returns `DocumentError::UnsupportedM71State` for any M71 definition. The
  explicitly unsupported draft-v5 envelope stores M71 records in an omitted-when-empty side
  section and merges them into the complete embedded source order before final validation.
- Ordinary document lowering, source audit grouping, activation, suppression, deletion,
  dependency closure, retained rejection, prepared work, exact CAS and Undo/Redo use the same
  lifecycle as existing constraints.
- The sketch runtime adds `Sketch::add_horizontal_point_to_midpoint` and
  `Sketch::add_vertical_point_to_midpoint`, backed by one `AxisMidpointResidual` hard row and an
  analytic `[+1, -1/2, -1/2]` Jacobian with deduplicated endpoint-alias incidence.
- `geosolve-constraint-editor` makes Horizontal and Vertical variable-arity over one affine span
  or two stored points, adds explicit Concentric and Collinear intents, and extends the M70
  inference engine with durable remembered-point H/V, native-midpoint-axis H/V, semantic-center
  Concentric and certified affine-support Collinear candidates. Only accepted native line/polyline
  midpoints can produce the new retained axis relations; `FilletDiscarded` and nonlinear
  curve-parameter midpoints remain tracking-only.
- M71-F004 gives durable point tracking its own `CandidateKey` component and composes it with a
  complementary exact Cartesian new-span direction. One candidate owns the exact intersection,
  both relations and both guides; its singleton subsets are removed without making relation count
  a ranking discriminator. Generation remains streaming and fail-closed at the configured bound.
  Same-axis and oblique directions remain alternatives.
- `ConstructionCommitPlan` adds prospective curve and directed-support slots so a new circle or
  line can participate in its retained relation in the same atomic publication.
- `SceneConstraintEntry`, `constraint_entries` and `EditorScene::constraint_entries` publish
  stable constraint ID, source ID, label, glyph, operands and suppression through the headless
  boundary. Accepted canvas annotations add geometry for the same identities. The workbench tree
  consumes headless entries for current design intent, including rejected design state, while
  canvas positions remain accepted-state authority.
- `geosolve-demo-web` adds explicit palette icons/actions, inference presentation and the ordinary
  editable **Constraints & dimensions → Retained drafting relations** sample. Its workspace-v5
  adapter round-trips exact draft-v5 bytes and keeps canonical-v4 export unsupported.
- `crates/geosolve-sketch/tests/m71_relations.rs`, `m71_persistence.rs` and
  `crates/geosolve-constraint-editor/tests/m71_transition_parity.rs` own the focused relation,
  persistence and native/WASM transition contracts. The focused M71-F003 owner regression is
  `crates/geosolve-constraint-editor/tests/m71_f003_midpoint_axis.rs`; it reproduces the public
  scene/editor-to-retained transition and checks atomic publication plus later endpoint tracking.
  The focused M71-F004 regression is
  `crates/geosolve-constraint-editor/tests/m71_f004_axis_bundle.rs`; it covers the complementary
  line and polyline pairings, exact preview/plan composition, accepted residuals, one-step history
  and later edits.
  The milestone-neutral golden authoring/scene fixture remains the broad compatibility oracle.

## 2. Mathematical behavior

The original four M71 relations select existing runtime mathematics. M71-F003 adds one linear
residual family without adding solver priority or an implicit branch rule:

| Retained definition | Existing lowering | Hard rows |
| --- | --- | ---: |
| `HorizontalPoints` | `Sketch::add_horizontal_points` | 1 |
| `VerticalPoints` | `Sketch::add_vertical_points` | 1 |
| `HorizontalPointToMidpoint` | `Sketch::add_horizontal_point_to_midpoint` | 1 |
| `VerticalPointToMidpoint` | `Sketch::add_vertical_point_to_midpoint` | 1 |
| `Concentric` | resolve stored centers, then `Sketch::add_coincident` | 2 |
| `Collinear` | resolve directed native supports, then `Sketch::add_collinear` | 2 |

Point-pair H/V accepts stored point IDs only. The two explicit midpoint-axis definitions accept a
stored point and certified native line/polyline span. Horizontal constrains Y and Vertical
constrains X through `P[c] - (A[c] + B[c]) / 2`; both may coexist and follow live endpoint edits.
The midpoint is the live endpoint average, not a placement-time coordinate or hidden point. The
analytic Jacobian is `[+1, -1/2, -1/2]`; central finite-difference coverage checks it, the
structured audit descriptor names the point/span/axis semantics, and independent finite hard-
residual validation gates every success-like result. `FilletDiscarded` and nonlinear
curve-parameter midpoint occurrences remain tracking-only.
Concentric uses exact accepted center capability rather than
coordinate proximity. For a centered construction only, exact semantic-center intent outranks the
incidental stored point that owns the same coordinate; ordinary point authoring retains structural
point-identity precedence, and an explicit candidate preference remains authoritative. Collinear
requires certified native affine supporting-line evidence and replaces, rather than bundles with,
a Parallel proposal. Repeated, tautological, unsupported, degenerate, ambiguous, stale or
resource-exhausted operands fail transactionally.

M71-F004 changes only candidate composition. For a point-operand endpoint, durable
HorizontalPoints/HorizontalPointToMidpoint may compose with an exact Vertical span direction, and
durable VerticalPoints/VerticalPointToMidpoint may compose with an exact Horizontal span direction.
Exact axis-aligned remembered Parallel/Perpendicular/Collinear directions are included using their
pre-normalization source provenance. The adjusted endpoint is the exact coordinate intersection;
relation order is endpoint axis first and span direction second. Compound angular evidence is the
worse component error, and both latches retain through the exit band. No residual, Jacobian,
solver priority, branch rule, persistence format or public API changes.

The original four relations are commutative in operand order; the point-to-midpoint definitions
are deliberately directional in operand type. Reversing either Collinear support direction
does not change its solution set, but direction remains explicit retained state. Every success is
subject to independent finite hard-residual validation; every rejection preserves prior accepted
geometry, history and publication authority.

## 3. Commands and outcomes

The following focused qualification and complete development gate pass on the post-F004 product.
The dirty command is retained as historical development evidence; the final command is the clean
F004 qualification of the unchanged nominated source:

```text
cargo fmt --all -- --check
cargo test --locked -p geosolve-sketch --test m71_relations
cargo test --locked -p geosolve-sketch --test m71_persistence
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
cargo test --locked -p geosolve-constraint-editor --test m71_f004_axis_bundle
cargo test --locked -p geosolve-constraint-editor --test m71_transition_parity
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m70_transition_parity --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m71_transition_parity --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
cargo clippy --locked -p geosolve-constraint-editor --all-targets --all-features -- -D warnings
git diff --check
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 \
  nix-shell shell.nix --run './scripts/release-gate.sh'
env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

Outcomes:

- M71 relation matrix: **17/17 pass**, including six dedicated midpoint-axis owner proofs plus
  every stored-center curve family in both Concentric operand orders and retained parent-point
  edits;
- M71 persistence matrix: **7/7 pass**;
- AxisMidpointResidual finite-difference test: **1/1 pass**;
- F003 public coordinator regression: **2/2 pass**;
- F004 public coordinator regression: **2/2 pass**;
- constraint editor: **311/311** unit tests plus every integration and doc-test pass;
- demo web: **104/104** library tests, **1/1** decoder test and doc tests pass;
- canonical authoring/scene oracle: **234/234 `PASS`**, with `--check` and `--require-clean`
  passing at SHA-256
  `d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`;
- native and WASM M70/M71 transition parity pass, with the updated M71 fixture at SHA-256
  `98df37349faab89e7ca7da763d898b84d4f04588a4923539cd790ca673a53442`;
- demo-web WASM, formatting, warnings-denied workspace Clippy, locked all-feature workspace tests,
  warnings-denied rustdoc, benchmark compilation, M14/M32 performance budgets, licence/package
  validation and Trunk 0.21.14 assembly pass;
- the 256-moving-body sparse crossover passed in **151.18 seconds** in the historical dirty
  development gate and **125.55 seconds** in the clean F004 gate.

Cargo emitted only the repository's longstanding non-failing `license` plus `license-file`
manifest advisories. The successful WASM checks ran inside `nix-shell shell.nix`. Product source
`a2e51efba7d79f684d264094ffd7dd0e37a4d089` remained at the same tree with empty status before and
after the clean gate; the later publication-document commit is not part of that qualified product.

## 4. Acceptance state

The implemented direct evidence covers frozen-v4 isolation, draft-v5 exact persistence and
corruption rejection, the original 1/1/2/2 rows plus two one-row midpoint-axis definitions,
transformations/scales, commutative
operands, invalid/redundant/conflicting behavior, dependency deletion, suppression/reactivation,
prepared CAS, history, explicit and contextual authoring, bounded inference, prospective geometry,
typed headless entries/annotations, workspace restore, editable sample and reviewed golden parity.

The midpoint-axis evidence includes central finite-difference Jacobian checks, structured audit
descriptors, independent finite hard-residual validation, explicit point-plus-native-span operands,
live endpoint-average behavior, draft-v5 side persistence and frozen-v4 rejection. The historical
F003 development and clean-candidate gates passed and its immutable publication was byte-verified;
F004 has since withdrawn that candidate from continued UAT.

`PLAN.md` items are checked only where evidence exists. The pre-F003 and F003 publications are
withdrawn; the post-F004 clean replacement gate and immutable byte-verified publication pass.
Human UAT remains pending.

## 5. Known limitations and next blocker

### Resolved findings

- `M71-F001` — `EditorScene::from_accepted_for_design` built both accepted annotation geometry
  and current constraint entries from the retained accepted document. A newer rejected design
  relation was therefore absent from the public scene even though the ordinary workbench's
  separate design-document entry query masked that loss. The scene now keeps geometry and
  annotation coordinates accepted-document-owned while publishing constraint entries from the
  supplied design document. The exact owner regression proves the rejected entry is visible,
  receives no unaccepted annotation geometry, leaves the retained accepted document exactly
  unchanged and cannot authenticate the detached historical
  scene for publication. The existing ordinary workbench composition regression additionally
  proves that the thin adapter carries the exact-source design-only entry without creating a
  canvas annotation for it. A focused renderer regression
  also proves that an accepted annotation whose entry was deleted from the newer rejected design
  retains the accepted document's exact-source label rather than disappearing or borrowing newer
  design metadata.
- `M71-F002` — the compatibility `ConstraintEditor::available_constraints` path advertised
  relations for foreign persistent point IDs and for invalid curve-span occurrences, while the
  contextual coordinator rejected the same operands as `MissingObject`. Direct availability now
  applies the coordinator's exact selection-existence predicate before relation-specific
  applicability. Its focused regression covers foreign point-pair M71 relations and invalid-span
  Concentric without removing either public authoring surface.
- `M71-F003` — remembered midpoint anchors entered tracking, but durable H/V construction matched
  only `PersistentPoint`. The exact public `EditorScene → ConstraintEditor →
  RetainedEditorCoordinator` reproducer was frozen in
  `m71_f003_midpoint_axis.rs`. Accepted native line/polyline midpoints now publish the two explicit
  one-row relations atomically; both axes can coexist. Fillet-discarded and nonlinear midpoint
  occurrences remain outside durable scope. Corrected focused, development-gate and clean
  replacement-gate outcomes pass.
- `M71-F004` — remembered point/midpoint axes and a complementary live-span direction were
  generated only as singleton alternatives because `CandidateKey` had no independent
  point-tracking component. Exact intersections were ambiguous and biased samples snapped only one
  coordinate. The correction publishes one deterministic conjunction candidate and suppresses
  only its compatible singleton subsets. The focused public regression proves line
  `HorizontalPoints + Vertical` and polyline `VerticalPoints + Horizontal`; inference-owner tests
  cover both point/midpoint pairings, exact remembered axes, non-Cartesian provenance, same-axis
  alternatives, ambiguity, stable/stale identity, shared hysteresis, conservative ranking and
  bounded failure. This is a focused inference-composition defect, not a new canonical golden
  dimension.

F001, F002 and F004 change no residual, Jacobian, solver priority, branch rule or accepted
geometry. F003 deliberately adds the authorized linear residual and durable retained behavior. All
four remain focused owner regressions because they expose no missing systemic golden dimension.
F001 and F002 were independently classified `DEFECT` against source
`95d54581748292ecf2d1fb3687387b2a2a7805f8`. Exact pre-fix reproduction failed each proposed
owner regression; after repair the exact F001 and F002 commands pass 1/1, the complete editor
crate passed 302/302 unit tests plus every integration/doc-test suite at that checkpoint. The
current demo-web suite passes 104/104 plus its decoder/doc tests, and the current focused F003
sketch relation/persistence matrices pass 17/17 and 7/7, respectively.

A complete development-mode release gate passed on the historical post-F003 tree:

```text
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 nix-shell shell.nix --run './scripts/release-gate.sh'
```

That run included formatting/diff hygiene, warnings-denied workspace Clippy, all locked
all-feature workspace tests, clean golden, M70/M71 WASM parity, demo-web WASM, warnings-denied
rustdoc, benchmark compilation, M14/M32 performance budgets, the 152.53-second 256-moving-body
sparse crossover, licence/package validation and Trunk 0.21.14 release assembly. It remains
provisional development evidence; the clean replacement gate recorded below supplied nomination
evidence.

The historical post-F004 dirty tree passed the same complete development-mode gate with
`GEOSOLVE_ALLOW_DIRTY=1`. It includes the focused 2/2 F004 public regression, 311/311
constraint-editor unit tests plus every integration/doc test, the unchanged 234/234 golden,
native/WASM M70 and M71 parity, demo-web WASM, warnings-denied workspace Clippy and rustdoc, all
locked all-feature workspace tests, benchmark compilation, M14/M32 budgets, the 151.18-second
256-moving-body sparse crossover, licence/package validation and Trunk 0.21.14 release assembly.
That run remains provisional development evidence only.

The clean F004 product source `a2e51efba7d79f684d264094ffd7dd0e37a4d089`, tree
`8b73be00a384fe4a36ebe13fa0c06f32a6694a14`, passed exactly:

```text
env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

The sole worktree on `main` had empty status before and after the gate, and HEAD/tree were
unchanged. The complete log is `/tmp/geosolve-m71-f004-clean-gate.ZGQEKU.log`; the gate ran from
2026-08-14 13:04:17 to 13:11:13 AEST, the 256-moving-body sparse crossover passed in 125.55
seconds, and Trunk 0.21.14 produced the authoritative seven-file `dist`. Cargo emitted only the
longstanding non-failing `license` plus `license-file` advisories.

That `dist` was copied without rebuilding, manifest-compared and frozen at
`/tmp/geosolve-m71-f004-uat.SaXMVY` with directory mode `0555` and file mode `0444`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 14165 | `bf7bb1b88a7a6ae55701d10af9b58e2dddbcfaa0f899931d9937c3272f50f239` |
| `LICENSE` | 35148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-4c3212f5ba819fe0.js` | 33327 | `ae66dbea0ce8581e4b0ae2a63a83db2e18a4489f7bfa245627e2c16b757ef22b` |
| `geosolve-demo-web-4c3212f5ba819fe0_bg.wasm` | 6014468 | `f5dfccd077120d4ed0876f318c4cd6a86bfc672a74c40e496a01bd232923a911` |
| `index.html` | 22977 | `98c30dd76cb6f9cd5c33d86b41b3769e5fabbf25fe7f87b612acfbd2d865104c` |
| `styles-36c74d05d21a90c9.css` | 29304 | `49a0d71647856a30e798707860ffa9da4dbdbd1ec2f4faeafa412726f0e69048` |

The C-locale ordered-manifest aggregate is
`5baf5514f366da60ef9e88d7f53f2e8b0346ff5c5222d8e993529a38272b631b`. PID `2848202`, executable
`/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14`, serves only that
snapshot with argv `/run/current-system/sw/bin/python3 -u -m http.server 8080 --bind
100.94.63.83 --directory /tmp/geosolve-m71-f004-uat.SaXMVY` and listens only on
`100.94.63.83:8080`; its log is `/tmp/geosolve-m71-f004-uat.SaXMVY.server.log`.

Proxy-disabled, cache-bypassed, identity-encoded HTTP requests returned status 200 from remote IP
`100.94.63.83` for all seven assets with exact recorded sizes and byte equality. A separate `/`
request equalled `index.html`. The frozen, fetched and post-fetch manifests all reproduced the
same aggregate above. The fetched evidence is at `/tmp/geosolve-m71-f004-fetch.aDfzU7`.

The withdrawn pre-F003 candidate `ad01912eac28275644dcfc867a2dc70030b5406d` passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

That historical gate completed the same full sequence, including the unchanged 234/234 clean
golden, both WASM transition oracles, a 144.08-second sparse crossover and Trunk 0.21.14 release
assembly. Its seven-file release distribution remains frozen read-only at
`/tmp/geosolve-m71-uat.yFBsnX`. At that historical checkpoint, PID `49116` served it only at
`http://100.94.63.83:8080/`; proxy- and cache-bypassed requests for every asset and `/` byte-matched
the snapshot, `/` equalled `index.html`, and both served and post-fetch ordered aggregates equalled
`43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`.

Those bytes predate M71-F003, are withdrawn from continued UAT and are no longer served.

Withdrawn historical F003 clean source `83bd2b575784c44b618fb3ad144f24e84702d764`
passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The gate completed formatting and diff hygiene, warnings-denied workspace Clippy, all locked
all-feature workspace tests, the unchanged 234/234 clean golden oracle, native/WASM M70 and M71
transition parity, demo-web WASM, warnings-denied rustdoc, benchmark compilation, M14/M32
performance budgets, the 145.13-second 256-moving-body sparse crossover, licence/package checks and
Trunk 0.21.14 release assembly. Cargo emitted only the longstanding non-failing `license` plus
`license-file` advisories.

The exact seven-file gate output was copied without rebuilding, byte-compared and frozen at
`/tmp/geosolve-m71-f003-uat.hybK8W` with directory mode `0555` and file mode `0444`:

| File | SHA-256 |
| --- | --- |
| `API_COMPATIBILITY.md` | `bf7bb1b88a7a6ae55701d10af9b58e2dddbcfaa0f899931d9937c3272f50f239` |
| `LICENSE` | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-f3ecc0dffeb9ce14.js` | `ae66dbea0ce8581e4b0ae2a63a83db2e18a4489f7bfa245627e2c16b757ef22b` |
| `geosolve-demo-web-f3ecc0dffeb9ce14_bg.wasm` | `53bd9bfdc0cec56f9f3520af328c45c8a5dcda3e836c43017d2b1409b48c1a9e` |
| `index.html` | `946d66a5e03e56b22efd3ee99fc157ba9668c10ae4393695b6200274f57aace4` |
| `styles-36c74d05d21a90c9.css` | `49a0d71647856a30e798707860ffa9da4dbdbd1ec2f4faeafa412726f0e69048` |

At the F003 checkpoint PID `1202735` had exact argv `python3 -m http.server 8080 --bind
100.94.63.83 --directory /tmp/geosolve-m71-f003-uat.hybK8W` and listened on
`100.94.63.83:8080`. Proxy-disabled, cache-bypassed requests byte-matched all seven files; `/`
matched `index.html`. Both local and fetched ordered manifest aggregates equalled
`23ab4586acd0f8a86a85e81d7b913ee2736f2524fe81c9913fa3a726496584e0`.
PID `1202735` has since exited and those F003 bytes are no longer served. The shared endpoint is
now owned by the verified F004 server identified above.

M71 deliberately excludes broad derived-point H/V operands beyond explicit native line/polyline
midpoint axes, M37 catalog consolidation, certified generic intersections, quadrant anchors,
nonlinear tangent/normal inference, equality/symmetry inference, host axes/grids/increments,
persistent wake state, canonical sketch v5, computed-feature chaining, browser E2E and mobile
behavior. `FilletDiscarded` and nonlinear curve-parameter midpoint occurrences remain
tracking-only.

The sole remaining blocker is explicit supervising-human review and approval of M71-U1 through
M71-U5. Mechanical qualification and publication do not close M71.
