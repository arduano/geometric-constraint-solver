<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 implementation — Retained drafting relations

Status: M71-F003 midpoint-axis correction is implemented and the complete dirty-tree development
gate passes. The former four-definition candidate is withdrawn; clean nominated-source
qualification, replacement publication and supervising-human UAT are pending.

Architecture owner: ADR 0035

Withdrawn pre-F003 candidate source: `ad01912eac28275644dcfc867a2dc70030b5406d`

F003 development release-gate result: **pass with `GEOSOLVE_ALLOW_DIRTY=1` (provisional)**

Withdrawn Tailscale release distribution (preserved, not acceptable for continued UAT):
`/tmp/geosolve-m71-uat.yFBsnX` at
`http://100.94.63.83:8080/`, ordered manifest aggregate
`43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`

No replacement source hash, clean release-gate result, immutable distribution or publication has
yet been nominated or recorded.

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

The original four relations are commutative in operand order; the point-to-midpoint definitions
are deliberately directional in operand type. Reversing either Collinear support direction
does not change its solution set, but direction remains explicit retained state. Every success is
subject to independent finite hard-residual validation; every rejection preserves prior accepted
geometry, history and publication authority.

## 3. Commands and outcomes

The following post-F003 qualification completed on the current dirty development tree. The
integrated release-gate run is proportional mechanical evidence, but it is not a clean nominated
candidate gate and must be repeated after commit:

```text
cargo fmt --all -- --check
cargo test --locked -p geosolve-sketch --test m71_relations
cargo test --locked -p geosolve-sketch --test m71_persistence
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
cargo test --locked -p geosolve-constraint-editor --test m71_transition_parity
nix-shell shell.nix --run '<M70 WASM parity; M71 WASM parity; demo-web WASM check>'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
cargo clippy --locked -p geosolve-constraint-editor --all-targets --all-features -- -D warnings
git diff --check
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

Outcomes:

- M71 relation matrix: **17/17 pass**, including six dedicated midpoint-axis owner proofs plus
  every stored-center curve family in both Concentric operand orders and retained parent-point
  edits;
- M71 persistence matrix: **7/7 pass**;
- AxisMidpointResidual finite-difference test: **1/1 pass**;
- F003 public coordinator regression: **2/2 pass**;
- constraint editor: **302/302** unit tests plus every integration and doc-test pass;
- demo web: **104/104** library tests, **1/1** decoder test and doc tests pass;
- canonical authoring/scene oracle: **234/234 `PASS`**, with `--check` and `--require-clean`
  passing at SHA-256
  `d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`;
- native and WASM M70/M71 transition parity, demo-web WASM, formatting, warnings-denied workspace
  Clippy, locked all-feature workspace tests, warnings-denied rustdoc, benchmark compilation,
  M14/M32 performance budgets, licence/package validation and Trunk 0.21.14 assembly pass;
- the 256-moving-body sparse crossover passes in **152.53 seconds**.

Cargo emitted only the repository's longstanding non-failing `license` plus `license-file`
manifest advisories. An ambient-shell attempt lacked `wasm-bindgen-test-runner`, executed no test
and was a harness invocation error; the successful WASM checks ran inside `nix-shell shell.nix`.
The same complete gate must run without `GEOSOLVE_ALLOW_DIRTY` on the clean nominated candidate.

## 4. Acceptance state

The implemented direct evidence covers frozen-v4 isolation, draft-v5 exact persistence and
corruption rejection, the original 1/1/2/2 rows plus two one-row midpoint-axis definitions,
transformations/scales, commutative
operands, invalid/redundant/conflicting behavior, dependency deletion, suppression/reactivation,
prepared CAS, history, explicit and contextual authoring, bounded inference, prospective geometry,
typed headless entries/annotations, workspace restore, editable sample and reviewed golden parity.

The midpoint-axis evidence includes central finite-difference Jacobian checks, structured audit
descriptors, independent finite hard-residual validation, explicit point-plus-native-span operands,
live endpoint-average behavior, draft-v5 side persistence and frozen-v4 rejection. The full
development gate passes; clean candidate qualification and publication remain open.

`PLAN.md` items are checked only where evidence exists. The pre-F003 publication is withdrawn;
replacement release qualification and immutable publication remain open, followed by human UAT.

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
  occurrences remain outside durable scope. Corrected focused and full development-gate outcomes
  pass; a replacement clean release gate remains to be recorded.

F001 and F002 change no residual, Jacobian, solver priority, branch rule or accepted geometry.
F003 deliberately adds the authorized linear residual and durable retained behavior. All three
remain focused owner regressions because they expose no missing systemic golden dimension. F001
and F002 were independently classified `DEFECT` against source
`95d54581748292ecf2d1fb3687387b2a2a7805f8`. Exact pre-fix reproduction failed each proposed
owner regression; after repair the exact F001 and F002 commands pass 1/1, the complete editor
crate passes 302/302 unit tests plus every integration/doc-test suite. The current demo-web suite
passes 104/104 plus its decoder/doc tests. The current focused F003 sketch relation/persistence
matrices pass 17/17 and 7/7, respectively.

A complete development-mode release gate passed on the current post-F003 tree:

```text
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 nix-shell shell.nix --run './scripts/release-gate.sh'
```

That run included formatting/diff hygiene, warnings-denied workspace Clippy, all locked
all-feature workspace tests, clean golden, M70/M71 WASM parity, demo-web WASM, warnings-denied
rustdoc, benchmark compilation, M14/M32 performance budgets, the 152.53-second 256-moving-body
sparse crossover, licence/package validation and Trunk 0.21.14 release assembly. It is provisional
development evidence only and must be repeated on a clean nominated source.

The withdrawn pre-F003 candidate `ad01912eac28275644dcfc867a2dc70030b5406d` passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

That historical gate completed the same full sequence, including the unchanged 234/234 clean
golden, both WASM transition oracles, a 144.08-second sparse crossover and Trunk 0.21.14 release
assembly. Its seven-file release distribution is frozen read-only at
`/tmp/geosolve-m71-uat.yFBsnX`. PID `49116` serves it only at
`http://100.94.63.83:8080/`. Proxy- and cache-bypassed requests for every asset and `/` byte-match
the snapshot; `/` equals `index.html`, and both served and post-fetch ordered aggregates equal
`43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`.

Those bytes predate M71-F003 and are withdrawn from continued UAT. No corrected source hash,
release-gate result, distribution manifest or served endpoint has been recorded.

M71 deliberately excludes broad derived-point H/V operands beyond explicit native line/polyline
midpoint axes, M37 catalog consolidation, certified generic intersections, quadrant anchors,
nonlinear tangent/normal inference, equality/symmetry inference, host axes/grids/increments,
persistent wake state, canonical sketch v5, computed-feature chaining, browser E2E and mobile
behavior. `FilletDiscarded` and nonlinear curve-parameter midpoint occurrences remain
tracking-only.

The immediate blocker is complete clean qualification and byte-verified publication of a
replacement F003 candidate. After that, explicit supervising-human review of M71-U1 through
M71-U5 remains required. Mechanical qualification and publication do not close M71.
