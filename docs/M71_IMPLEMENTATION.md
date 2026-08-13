<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 implementation — Retained drafting relations

Status: implementation, clean integrated release qualification and immutable Tailscale
publication complete on 2026-08-13. Supervising-human UAT is pending.

Architecture owner: ADR 0035

Candidate source: `ad01912eac28275644dcfc867a2dc70030b5406d`

Integrated release-gate result: **PASS**

Tailscale release distribution: `/tmp/geosolve-m71-uat.yFBsnX` at
`http://100.94.63.83:8080/`, ordered manifest aggregate
`43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`

## 1. Files and APIs

- `geosolve-sketch::DocumentConstraintDefinition` adds ordinary retained
  `HorizontalPoints`, `VerticalPoints`, `Concentric` and `Collinear` definitions. Public
  `DocumentCenterRef` and `DocumentLineSupportRef` keep semantic center and directed affine-support
  operands explicit.
- `crates/geosolve-sketch/src/document.rs` isolates the frozen canonical-v4 constraint wire DTO.
  Canonical-v4 export returns `DocumentError::UnsupportedM71State` for any M71 definition. The
  explicitly unsupported draft-v5 envelope stores M71 records in an omitted-when-empty side
  section and merges them into the complete embedded source order before final validation.
- Ordinary document lowering, source audit grouping, activation, suppression, deletion,
  dependency closure, retained rejection, prepared work, exact CAS and Undo/Redo use the same
  lifecycle as existing constraints.
- `geosolve-constraint-editor` makes Horizontal and Vertical variable-arity over one affine span
  or two stored points, adds explicit Concentric and Collinear intents, and extends the M70
  inference engine with durable remembered-point H/V, semantic-center Concentric and certified
  affine-support Collinear candidates.
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
  persistence and native/WASM transition contracts. The milestone-neutral golden authoring/scene
  fixture gains reviewed append-only M71 rows.

## 2. Mathematical behavior

M71 adds no residual, Jacobian, solver priority or implicit branch rule. It selects existing
runtime mathematics:

| Retained definition | Existing lowering | Hard rows |
| --- | --- | ---: |
| `HorizontalPoints` | `Sketch::add_horizontal_points` | 1 |
| `VerticalPoints` | `Sketch::add_vertical_points` | 1 |
| `Concentric` | resolve stored centers, then `Sketch::add_coincident` | 2 |
| `Collinear` | resolve directed native supports, then `Sketch::add_collinear` | 2 |

Point-pair H/V accepts stored point IDs only. Derived midpoints and other transient semantic
anchors remain tracking-only. Concentric uses exact accepted center capability rather than
coordinate proximity. For a centered construction only, exact semantic-center intent outranks the
incidental stored point that owns the same coordinate; ordinary point authoring retains structural
point-identity precedence, and an explicit candidate preference remains authoritative. Collinear
requires certified native affine supporting-line evidence and replaces, rather than bundles with,
a Parallel proposal. Repeated, tautological, unsupported, degenerate, ambiguous, stale or
resource-exhausted operands fail transactionally.

All four relations are commutative in operand order. Reversing either Collinear support direction
does not change its solution set, but direction remains explicit retained state. Every success is
subject to independent finite hard-residual validation; every rejection preserves prior accepted
geometry, history and publication authority.

## 3. Commands and outcomes

Focused direct qualification completed before candidate nomination:

```text
cargo fmt --all -- --check
cargo test --locked -p geosolve-sketch --test m71_relations
cargo test --locked -p geosolve-sketch --test m71_persistence
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
cargo test --locked -p geosolve-constraint-editor --test m71_transition_parity
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
cargo clippy --locked -p geosolve-constraint-editor --all-targets --all-features -- -D warnings
git diff --check
```

Outcomes:

- M71 relation matrix: **11/11 pass**, including every stored-center curve family in both
  Concentric operand orders and retained parent-point edits across all four relations;
- M71 persistence matrix: **7/7 pass**;
- complete constraint-editor crate: **297/297 unit tests pass**, with every integration suite
  passing, including M71 native parity;
- complete demo-web crate: **102/102 library tests and 1/1 decoder tests pass**;
- canonical authoring/scene oracle: **234/234 `PASS`**, with `--check` and `--require-clean`
  passing at SHA-256
  `d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`;
- formatting, focused warnings-denied Clippy and diff hygiene pass.

Cargo emitted only the repository's longstanding non-failing `license` plus `license-file`
manifest advisories. M71 WASM transition parity and the demo-web WASM check also pass inside
`nix-shell shell.nix`; an ambient-shell attempt lacked `wasm-bindgen-test-runner` and was a harness
invocation error rather than a failed test. Full workspace Clippy/tests, Trunk and the complete
clean release gate are intentionally recorded only after they run on the nominated candidate.

## 4. Acceptance criteria passed

The implemented direct evidence covers frozen-v4 isolation, draft-v5 exact persistence and
corruption rejection, 1/1/2/2 lowering and audit rows, transformations/scales, commutative
operands, invalid/redundant/conflicting behavior, dependency deletion, suppression/reactivation,
prepared CAS, history, explicit and contextual authoring, bounded inference, prospective geometry,
typed headless entries/annotations, workspace restore, editable sample and reviewed golden parity.

`PLAN.md` items are checked only where evidence exists. Release qualification and immutable
publication now pass; human UAT remains open and M71 is not closed.

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

Neither finding changes a residual, Jacobian, solver priority, branch rule or accepted geometry.
They remain focused owner regressions because they expose no missing systemic golden dimension.
Both were independently classified `DEFECT` against source
`95d54581748292ecf2d1fb3687387b2a2a7805f8`. Exact pre-fix reproduction failed each proposed
owner regression; after repair the exact F001 and F002 commands pass 1/1, the complete editor
crate passes 299/299 unit tests plus every integration/doc-test suite. The current demo-web suite
passes 103/103 plus its decoder/doc tests. M71 sketch relation/persistence matrices pass 11/11 and
7/7, inference passes 56/56, native transition parity passes, and the 234-row golden survey,
check, and clean modes remain unchanged.

A complete development-mode release gate passed on the then-current five-file repair snapshot
before the later demo-adapter regression and Clippy-only scene-builder extraction:

```text
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 nix-shell shell.nix --run './scripts/release-gate.sh'
```

That historical run included formatting/diff hygiene, warnings-denied workspace Clippy, all locked
all-feature workspace tests, clean golden, M70/M71 WASM parity, demo-web WASM, warnings-denied
rustdoc, benchmark compilation, M14/M32 performance budgets, the 147.66-second 256-moving-body
sparse crossover, licence/package validation and Trunk 0.21.14 release assembly. It is provisional
development evidence only. It has now been superseded by the clean candidate evidence below.

Clean candidate `ad01912eac28275644dcfc867a2dc70030b5406d` passed:

```text
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

That authoritative gate completed the same full sequence, including the unchanged 234/234 clean
golden, both WASM transition oracles, a 144.08-second sparse crossover and Trunk 0.21.14 release
assembly. Its seven-file release distribution is frozen read-only at
`/tmp/geosolve-m71-uat.yFBsnX`. PID `49116` serves it only at
`http://100.94.63.83:8080/`. Proxy- and cache-bypassed requests for every asset and `/` byte-match
the snapshot; `/` equals `index.html`, and both served and post-fetch ordered aggregates equal
`43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`.

M71 deliberately excludes derived-point H/V operands, M37 catalog consolidation, certified
generic intersections, quadrant anchors, nonlinear tangent/normal inference, equality/symmetry
inference, host axes/grids/increments, persistent wake state, canonical sketch v5, computed-feature
chaining, browser E2E and mobile behavior.

The remaining blocker is explicit supervising-human review of M71-U1 through M71-U5 in
`docs/M71_UAT.md`. Mechanical qualification and publication do not close M71.
