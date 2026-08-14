<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 fresh-session handover

Status: **M71-F004 is reproduced, corrected and passes the complete dirty-tree development gate;
clean replacement qualification/publication are next**. The F003 source
`83bd2b575784c44b618fb3ad144f24e84702d764` remains byte-preserved at
`/tmp/geosolve-m71-f003-uat.hybK8W`, but it is withdrawn from continued UAT because it cannot
compose a remembered point axis with a complementary new-span direction. Its former PID has exited
and `http://100.94.63.83:8080/` is offline until this handover records a verified F004 replacement.
M71 remains open through replacement qualification/publication and explicit supervising-human
approval of M71-U1 through M71-U5.

This document is the canonical short restart contract for the M71 correction and replacement
qualification. Read the repository-required project documents first, then this file, ADR 0035,
`docs/M71_GOALS.md`, `docs/M71_IMPLEMENTATION.md` and `docs/M71_UAT.md`. Do not reconstruct M71
from chat history.

## 2026-08-14 checkpoint — M71-F004 simultaneous endpoint-axis inference

### Exact human finding and intended behavior

The supervising human requested one endpoint placement to retain both pieces of obvious Cartesian
intent. For example, while drawing a vertical line, its top endpoint should be able to align
horizontally with a remembered point to the side. The symmetric horizontal-line plus vertical-
point alignment must behave the same way.

The clean-base reproduction used source `603194947a642917b9e44359326708de37f1a1d2`:

1. start a line at `[0, 0]`;
2. hover a stored point at `[-4, 4]` so its axis reference is remembered; and
3. move to the intersection near `[0, 4]`.

Before correction, the exact intersection was `Ambiguous` between singleton `Vertical` and
singleton `HorizontalPoints` candidates. A slight pointer bias selected only one singleton, so one
coordinate remained unsnapped and placement retained only one relation. No candidate represented
the intended conjunction. This is `M71-F004`, a presentation-independent inference defect owned
by `geosolve-constraint-editor`.

### Root cause and corrected contract

`CandidateKey` previously had only anchor and direction components. Durable point tracking used
the direction component to encode its H/V tracking axis, while new-span H/V also needed that same
component. `point_tracking_candidates` consequently generated the two valid relations only as
alternatives after ordinary direction candidates.

The correction gives point tracking its own keyed component and composes it with a complementary
exact Cartesian new-span direction when both behaviors adjust coordinates and persist a relation.
One candidate now owns the exact coordinate intersection, two constraint-backed guides and the
deterministic relation order **endpoint axis first, span direction second**. It replaces its two
singleton subsets without adding relation-count ranking. Both the point-tracking and direction
latches survive through the shared exit band.

This composition is deliberately Cartesian. World Horizontal/Vertical and remembered
Parallel/Perpendicular/Collinear directions whose original source vector is exactly axis-aligned
qualify. Oblique directions and same-axis relations remain alternatives: same-axis sources can
conflict after later edits and do not independently determine both endpoint coordinates. Exact
ties between distinct remembered operands remain `Ambiguous`.

Candidate generation remains bounded and streaming. One bundle fits a one-candidate limit; the
first second unique bundle proves overflow and returns no partial candidate/guide prefix. Compound
angular ranking publishes the worse of the tracking and direction errors. Candidate IDs distinguish
standalone tracking from the same tracking source composed with a direction, so stale preferences
fail closed.

### Regression and current evidence

The focused public regression is
`crates/geosolve-constraint-editor/tests/m71_f004_axis_bundle.rs`. Its line case proves
`HorizontalPoints + Vertical`; its polyline case proves `VerticalPoints + Horizontal`. Together
they check the exact preview intersection, deterministic two-relation plans, two retained
constraints, finite accepted geometry, independently recomputed endpoint equations and accepted
hard residual `<= 1e-9`; the line case additionally proves one retained history step and a
compatible later edit.

Inference-owner tests additionally cover stored points and native midpoints in both pairings,
exact axis-aligned remembered directions, finite extreme non-Cartesian provenance, same-axis
non-composition, distinct-operand ambiguity, candidate identity, stale preference, shared
hysteresis, worst-component ranking and candidate-limit failure. Native/WASM M71 transition parity
adds `remembered-axis-bundle=horizontal-points:71000071000000000000000000000002+vertical`; the
updated fixture SHA-256 is `98df37349faab89e7ca7da763d898b84d4f04588a4923539cd790ca673a53442`.

No residual, Jacobian, solver priority, branch rule, persistence format or public API changes.
F004 is a focused inference-composition dimension; the canonical 234-row authoring/scene oracle
does not exercise inference bundles and remains unchanged at SHA-256
`d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`.

The complete provisional development gate passed from the dirty F004 worktree based on HEAD
`603194947a642917b9e44359326708de37f1a1d2`:

```text
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

It passed formatting/diff hygiene, warnings-denied workspace Clippy, all locked all-feature
workspace tests, the unchanged golden, native/WASM M70 and M71 transition parity, demo-web WASM,
warnings-denied rustdoc, benchmark compilation, M14/M32 budgets, licence/package validation and
Trunk 0.21.14 release assembly. Current owner results include 311/311 constraint-editor unit tests
plus every integration/doc test, the 2/2 public F004 regression, 104/104 demo-web unit tests plus
decoder/doc tests, 17/17 M71 sketch relation tests and 7/7 persistence tests. The required
256-moving-body sparse crossover passed in 151.18 seconds. Cargo emitted only the existing
non-failing `license` plus `license-file` advisories. Because the source was dirty, this is
development evidence rather than clean nomination evidence.

### Preserved F003 publication

The immutable seven-file snapshot remains mode `0555` with `0444` regular files and ordered
aggregate
`23ab4586acd0f8a86a85e81d7b913ee2736f2524fe81c9913fa3a726496584e0`. It must remain untouched
until a clean F004 distribution has been frozen and verified. PID `1202735` is absent and no
process listens on `100.94.63.83:8080`; the snapshot alone remains preservation evidence.

### Pre-nomination repository checkpoint

At the dirty-gate checkpoint, the sole worktree was
`/home/arduano/programming/geometric-constraint-solver` on `main`, with local `main` and
`origin/main` both at commit
`603194947a642917b9e44359326708de37f1a1d2`, tree
`1ac8b8ab3aaeeaa2770efc06180e2625cca902a1`. The F004 implementation, focused regression and
transition parity are now committed as
`1f542555d7fcaf98ecf92c69a10b951fbfcc3dff`. On 2026-08-14 the supervising human granted ordinary
reviewable-commit authority for this repository. The exact clean nominated HEAD/tree/status must
be captured after the pre-release documentation commit and before the clean gate; no push has been
made.

## Historical 2026-08-14 checkpoint — M71-U2 midpoint-axis correction

### Exact human finding and authorized behavior

The supervising human reproduced this in the published UI:

1. Draw a line, hover one of its stored endpoints, move right and place a second line. The teal
   constraint-backed guide creates retained `HorizontalPoints`: **pass**.
2. Repeat after hovering the middle of the line. A differently styled dotted guide appears, but
   placement creates no relation: **fail**.

The old M71 contract deliberately made a remembered midpoint tracking-only. After the diagnosis
was explained, the supervising human explicitly rejected that boundary because durable midpoint
axes are essential when centering sketch geometry in a rectangle. The authorized semantic outcome
is now:

- horizontal alignment to a remembered native line/polyline span midpoint creates one durable
  relation tying the constructed point's Y coordinate to the live average of the span endpoints;
- vertical alignment creates the analogous X-coordinate relation;
- both relations may coexist on one point and thereby keep it exactly at the live span midpoint as
  the rectangle/support moves or resizes;
- this is not a one-time coordinate snap, `FixedCoordinate`, zero dimension, or hidden midpoint
  point;
- each axis is one ordinary retained source with one hard row, explicit point-plus-span operands,
  structured audit text, dependency/lifecycle behavior, persistence, and independent residual
  validation;
- the narrow authorized scope is certified native line/polyline span midpoints. Do not silently
  generalize this checkpoint to arbitrary nonlinear curve-parameter midpoints.

For current M71 guidance, this supersedes earlier statements that native line/polyline midpoint
H/V must remain tracking-only. Historical M70/M70B records remain true to their checkpoints and
carry ADR 0035 supersession notes; the active ADR, goals, implementation, UAT, plan, acceptance,
architecture, start-here and scenario records describe the corrected behavior.

### Diagnosis and finding identity

The exact headless cause is known. `DraftInferenceEngine::point_tracking_candidates` accepts both
`PersistentPoint` and `Midpoint` as remembered guide origins, but its durable branch matches only
`PersistentPoint`. A midpoint therefore publishes a standalone `PointTracking` /
`TrackingOnly` guide, leaves the raw coordinate unchanged, resolves no candidate, and eventually
commits geometry without an inferred relation. The browser accurately rendered that old semantic
classification; this is now a **headless contract defect**, not merely a CSS/discoverability issue.

The fresh session independently reproduced this exact public scene/editor-to-retained transition
against clean source `5b29744f445f458cffabd176c123861f39392d12`. It is assigned `M71-F003`.
The focused owner regression is
`crates/geosolve-constraint-editor/tests/m71_f003_midpoint_axis.rs`; it proves both axes publish
through the retained coordinator and that the live relation follows later endpoint edits.

### Historical F003 correction checkpoint

- Both runtime/document definitions, independent validation, draft-v5 side persistence,
  dependency/lifecycle behavior, editor inference/commit DTOs, annotations, workbench presentation,
  native transition parity and focused owner regression are implemented.
- `AxisMidpointResidual` has an analytic `[+1, -1/2, -1/2]` Jacobian and central finite-difference
  checks at model scales `1e-6`, `1` and `1e6`.
- The sketch owner matrix passes 17/17 and persistence passes 7/7. It covers line/polyline spans,
  exact audit metadata, normalization, endpoint alias incidence, both axes, live edits,
  suppression/history/rejection authority, dependencies/deletion, invalid operands and prepared
  CAS.
- Midpoint-specific inference ambiguity, hysteresis, suppression and stale-preference proofs pass.
  Fillet-discarded midpoint occurrences remain tracking-only.
- The public F003 coordinator regression passes 2/2, native transition parity passes, web
  presentation/persistence focused tests pass, and exact annotation-owner tests pass.
- Constraint-editor all-feature tests pass 302/302 unit tests plus every integration/doc-test;
  demo-web passes 104/104 unit tests, its decoder test and doc tests.
- The unchanged canonical golden passes 234/234 `PASS` in survey/check/require-clean modes at
  SHA-256 `d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`.
- Native and WASM M70/M71 transition parity, demo-web WASM, formatting, warnings-denied workspace
  Clippy, locked all-feature workspace tests and Trunk 0.21.14 release assembly pass.
- The complete dirty-tree development gate passed with a 152.53-second sparse crossover. Clean
  candidate `83bd2b575784c44b618fb3ad144f24e84702d764` then passed the complete gate with a
  145.13-second sparse crossover, licensing/package validation and final Trunk assembly.
- Its immutable seven-file snapshot and cache-bypassed served bytes are verified at ordered
  manifest aggregate `23ab4586acd0f8a86a85e81d7b913ee2736f2524fe81c9913fa3a726496584e0`.
  At that historical checkpoint only supervising-human UAT remained open; F004 has since withdrawn
  those bytes from continued UAT.

### Historical F003 repository/worktree state

- Working directory: `/home/arduano/programming/geometric-constraint-solver`.
- At nomination, the sole worktree's `main` was clean at candidate
  `83bd2b575784c44b618fb3ad144f24e84702d764`, two commits ahead of `origin/main` (`0 2`).
- Candidate commits are `c417f79` (`fix(m71): retain native midpoint axis alignment`) and
  `83bd2b5` (`docs(m71): reconcile midpoint correction handoff`); neither had been pushed at
  nomination. Post-publication evidence is recorded in the separate forward commit `eeda588`.
- The direct-`main` UAT workflow publishes these forward commits after qualification. Always
  recheck the resulting local/remote hash rather than treating a documentation commit as the
  source of the frozen product bytes.
- At that checkpoint PID `1202735` had exact argv `python3 -m http.server 8080 --bind
  100.94.63.83 --directory /tmp/geosolve-m71-f003-uat.hybK8W` and listened on
  `100.94.63.83:8080`. It has since exited.

## Repository history at consolidation

- Sole worktree: `/home/arduano/programming/geometric-constraint-solver`.
- Branch: `main`.
- Pre-consolidation base: `4d5bec1d395c37cfdabc8448933db19d3f94f8b8`, one commit ahead of
  live `origin/main` at `8ebe2e171ece7faf95057dc39c9ff2c6c7804c2f`.
- The complete M71 implementation was confined to that worktree. The five formerly untracked
  files were all intentional M71 relation, persistence, parity and implementation records; no
  scratch, reject, backup or dangling untracked file was found.
- Withdrawn pre-F003 candidate `ad01912eac28275644dcfc867a2dc70030b5406d` remains frozen at
  `/tmp/geosolve-m71-uat.yFBsnX` but is no longer served. Its historical ordered aggregate is
  `43cc01534dc8f91985432d365ac013f9410df80ba1b303b7bb3eeee7a980de41`. The historical M70B
  snapshot remains on disk but is no longer served.

Those earlier publications remain historical mechanical evidence only. Continued UAT must wait
for the clean, byte-verified F004 replacement described at the top of this handover.

After this handover is committed, use `git log -5 --oneline --decorate`, `git status --short
--branch`, `git worktree list --porcelain` and `git rev-list --left-right --count
origin/main...main` to establish the exact new checkpoint. Do not assume it has been pushed unless
the latter command and `git ls-remote origin refs/heads/main` agree.

## Implemented scope

M71 promotes six definitions across five relation families into the ordinary retained
document/editor lifecycle:

- stored-point `HorizontalPoints` and `VerticalPoints`;
- stored-point-to-native-span-midpoint `HorizontalPointToMidpoint` and
  `VerticalPointToMidpoint`;
- semantic-center `Concentric`; and
- directed native-support `Collinear`.

The sketch domain owns validation, lowering, audit grouping, suppression, deletion, dependency
closure, retained solve/history and persistence. Canonical sketch v4 is isolated behind a private
frozen wire DTO and rejects M71 state with `UnsupportedM71State`; unsupported draft v5 carries the
new records in an omitted-when-empty side section and transactionally merges them into the complete
source order.

The headless editor owns variable-arity contextual authoring, semantic inference, candidate
ranking, bounds, prospective same-transaction operands, atomic commit plans and presentation
metadata. The browser adapter renders and dispatches those public DTOs and supplies one ordinary
editable **Retained drafting relations** sample. It owns no equations or applicability policy.

The original four definitions lower to existing `add_horizontal_points`, `add_vertical_points`,
center `add_coincident` and `add_collinear` operations. F003 adds one `AxisMidpointResidual`
family with analytic and finite-difference-checked Jacobian. Every path is followed by independent
finite hard-residual validation; no solver priority or implicit branch rule changes.

## Implicit-correctness law

The supervising principle is **implicit correctness**: prefer strong composable semantics over a
tool-specific edge-case table.

The implemented center rule is expressed through one operand capability:

- `CenteredPointOperand` means a stored construction point that will also be the semantic center
  of a prospective curve;
- for that operand only, an exact accepted semantic-center/Concentric candidate outranks incidental
  structural reuse of the stored center point;
- ordinary `PointOperand` retains M70 point-identity precedence;
- midpoint and PointOnCurve remain available;
- an explicit candidate preference remains authoritative; and
- disabling Concentric falls back to ordinary point identity.

This rule covers Circle, counter-clockwise Circular Arc, Ellipse, Elliptical Arc and Hyperbola
without ranking branches named after those tools. Distinct curves that share one stored center are
distinct retained operands and therefore produce an ambiguous choice; repeated scene occurrences
of one curve are deduplicated. Persistent IDs never silently break a semantic tie.

Scene collection is all-or-nothing and bounded before publication. Ordinary and semantic anchors
share one subject-relevant bound; suppression bypasses traversal; overflow publishes no prefix;
scope/visibility filtering, ambiguity and post-overflow reacquisition are directly tested.

## Exact checkpoint evidence

The following post-F003 commands passed on the historical development tree on 2026-08-14:

```text
cargo test --locked -p geosolve-sketch --test m71_relations --test m71_persistence
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-demo-web --all-features
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m70_transition_parity --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m71_transition_parity --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
env NO_COLOR=true GEOSOLVE_ALLOW_DIRTY=1 \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

Observed results:

- M71 relation owner matrix: 17/17 pass;
- M71 persistence matrix: 7/7 pass;
- exact AxisMidpointResidual finite-difference test: 1/1 pass;
- public F003 coordinator regression: 2/2 pass;
- constraint editor: 302/302 unit tests plus every integration and doc-test suite pass;
- demo web: 104/104 library tests, 1/1 decoder test and doc tests pass;
- canonical golden: 234/234 `PASS`, SHA-256
  `d009b76bcf584e32829832ec50df59ffc51a2f260003e5eed36a286c63e5dc27`;
- M70 and M71 WASM transition parity: 1/1 each; demo-web WASM check: pass;
- standalone and gate-owned Trunk 0.21.14 release assembly: pass;
- warnings-denied workspace Clippy, locked all-feature workspace tests, rustdoc, benchmark
  compilation, M14/M32 budgets, licensing/package checks and diff hygiene: pass;
- 256-moving-body sparse crossover: pass in 152.53 seconds.

Cargo emitted only the existing non-failing `license` plus `license-file` manifest advisories.
An ambient-shell WASM attempt could not find `wasm-bindgen-test-runner`; it executed no test and is
a harness error, not product evidence. The successful WASM results above ran inside `nix-shell`.
Because `GEOSOLVE_ALLOW_DIRTY=1` was used, the integrated gate is provisional development evidence,
not clean candidate qualification.

## Review before release nomination

The completed audit found no solver or mathematical blocker, but two cleanup questions remain.
They should be resolved by semantic consolidation, not by adding more examples:

1. `construction_point_stage` and `draft_inference_subject` currently classify centered stages
   from closely related exhaustive `EditorTool` matches. Consider replacing that duplicated tool
   knowledge with one construction-stage semantic descriptor. Preserve the current centered-tool,
   coordinate-only-stage and prospective-curve-slot tests.
2. The older direct `available_constraints`/`constraint_edit` path and the contextual
   `resolve_constraint` coordinator path each contain M71 applicability knowledge. Audit whether
   both public surfaces must remain; if so, derive them from one semantic predicate or retain an
   explicit parity law. Do not casually remove compatibility APIs.

The parallel semantic-center vector/latch/candidate pipeline was reviewed as an implementation
shape, but its behavior is now governed by operand capability, retained curve identity, shared
bounds and subject-aware ranking. Do not refactor it merely for visual uniformity unless the
result makes those laws smaller and clearer.

## Next-session sequence

1. Commit the pre-release documentation set under the granted commit authority, then capture the
   exact clean HEAD/tree/status before and after the clean gate. The implementation/test commit is
   `1f542555d7fcaf98ecf92c69a10b951fbfcc3dff`.
2. Run `env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'` without a dirty
   override on that unchanged clean source.
3. Copy that gate-produced seven-file `dist` without rebuilding, byte-compare and freeze a new
   immutable F004 snapshot. Then start a fresh Tailscale-only server, verify the exact listener and
   fetch every asset plus `/` with proxy/cache bypass; PID `1202735` is already absent.
4. Record the clean source, gate, manifest, PID/listener and served-byte evidence in a forward
   documentation commit. Then ask the supervising human to repeat M71-U1 through M71-U5, with U2
   explicitly covering simultaneous endpoint-axis plus span-direction placement.
5. Close M71 only after explicit supervising-human approval. Do not infer approval from mechanical
   evidence.

## Deliberately deferred

Do not expand M71 into broad derived-point H/V operands beyond the two explicit native-span
midpoint-axis definitions, M37 catalog consolidation, generic
intersections, quadrant anchors, nonlinear tangent/normal inference, equality/symmetry inference,
host axes/grids/increments, persistent wake state, canonical sketch v5, computed-feature chaining,
browser E2E, mobile support or legacy UI.
