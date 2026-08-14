<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M71 fresh-session handover

Status: **M71-F005/F006 are corrected and focused-development-qualified, but not clean-qualified or
published**. Implementation source `4f5339fa0de6b12794647835ac9066af5520887e` composes Horizontal
and Vertical point axes from two distinct remembered stored points and narrows the default capture
envelope. The clean F004 source `a2e51efba7d79f684d264094ffd7dd0e37a4d089`, tree
`8b73be00a384fe4a36ebe13fa0c06f32a6694a14`, and byte-verified snapshot
`/tmp/geosolve-m71-f004-uat.SaXMVY` remain historical evidence but are withdrawn from continued
UAT. PID `2848202` still serves those historical bytes at `http://100.94.63.83:8080/`; their
ordered manifest aggregate remains
`5baf5514f366da60ef9e88d7f53f2e8b0346ff5c5222d8e993529a38272b631b`. Clean replacement
qualification/publication, M71-U1 through M71-U5 and explicit approval remain pending.

This document is the canonical short restart contract for the M71 correction and replacement
qualification. Read the repository-required project documents first, then this file, ADR 0035,
`docs/M71_GOALS.md`, `docs/M71_IMPLEMENTATION.md` and `docs/M71_UAT.md`. Do not reconstruct M71
from chat history.

## 2026-08-14 checkpoint — M71-F005/F006 cross-axis points and tighter capture

### Exact human finding and intended behavior

After F004, one remembered point/native-midpoint axis could compose with a complementary exact
direction of a new line or polyline span. The remaining nitpick was to snap a constructed endpoint
to complementary point axes at once: one distinct remembered stored point supplies its Horizontal
Y coordinate while another supplies its Vertical X coordinate. Both must be visible in one exact
preview and retained atomically through either the line or polyline path.

The focused F005 reproducer remembers stored points at `[-4, 4]` and `[3, -4]`, then approaches
`[3.04, 4.05]`. Before correction, candidate identity and confirmed-reference handoff represented
only one point-tracking component, so no one semantic candidate could own both remembered axes and
polyline continuation could retain only one positional reference. Separately, the default capture
thresholds were still the broader historical M70 values—8/12 px for points/midpoints, 10/14 px for
curves and 4/6 degrees for directions—which made inference feel too eager. These are M71-F005 and
M71-F006, presentation-independent interaction defects owned by `geosolve-constraint-editor`.

### Root cause and corrected contract

F005 gives `CandidateKey` an independent `secondary_point_tracking` component. Candidate
generation pairs Horizontal and Vertical tracking work from distinct semantic anchors before
publishing singleton alternatives. Horizontal supplies Y, Vertical supplies X, and the canonical
H-then-V candidate owns `[vertical.x, horizontal.y]`, two terminating constraint-backed guides,
both remembered references and one atomic two-relation plan. Confirmed positional-reference
collection now retains each distinct reference represented by the relations, so both survive the
ordinary polyline stage handoff.

One semantic anchor cannot pair its own two axes because that would disguise point identity as two
redundant retained relations. Equal competing pairings remain `Ambiguous`; both tracking latches
retain only through their shared exit band; the first candidate-limit overflow returns raw
coordinates without a candidate/guide prefix; and F004 point-axis-plus-span-direction bundles
remain explicit alternatives where they encode different retained intent.

F006 changes only `DraftInferenceTolerances::default()`. Stored points, semantic centers and native
midpoints now use inclusive 6/9 px enter/leave thresholds; curves use 8/12 px; and world,
remembered and point-tracking directions use 3/5 degrees. Valid caller-supplied policy values,
validation, resource bounds, suppression and hysteresis transitions remain authoritative and
unchanged. Neither F005 nor F006 changes a residual, Jacobian, solver priority, branch rule,
persistence format or browser-owned policy.

### Regression and focused evidence

Committed implementation source is `4f5339fa0de6b12794647835ac9066af5520887e`. The public
regression `crates/geosolve-constraint-editor/tests/m71_f005_cross_axis.rs` proves line and polyline
preview/commit paths, exact H-then-V relation order, two guides, two retained constraints, one-step
line history, finite accepted coordinates, independently recomputed endpoint equations, hard
residual `<= 1e-9`, later reference edits and both positional references surviving polyline stage
handoff.

Inference-owner tests prove stable pair identity, exact competing-pair ambiguity, same-anchor
exclusion, one-pair and overflowing candidate bounds, both-axis exit hysteresis and coexistence
with F004 point-axis-plus-span-direction alternatives. The F006 regression
`m71_f006_tighter_default_capture_envelope_excludes_old_only_entry_samples` rejects a seven-pixel
point, nine-pixel curve and 3.5-degree direction in a fresh default engine; the boundary matrix
keeps comparisons inclusive at the new thresholds.

Development prequalification commands passed on committed implementation HEAD
`4f5339fa0de6b12794647835ac9066af5520887e` while documentation-only changes remained in the
worktree:

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
cargo test --locked -p geosolve-constraint-editor --all-features
cargo test --locked -p geosolve-constraint-editor --test m71_f005_cross_axis
cargo test --locked -p geosolve-demo-web --all-features
./scripts/golden-authoring-scene-oracle.sh --survey
./scripts/golden-authoring-scene-oracle.sh --check
./scripts/golden-authoring-scene-oracle.sh --require-clean
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m70_transition_parity --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner cargo test --locked -p geosolve-constraint-editor --test m71_transition_parity --target wasm32-unknown-unknown'
nix-shell shell.nix --run \
  'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
git diff --check
```

All passed: the editor has 319/319 unit tests plus every integration/doc test, the F005 public
regression passes 2/2, demo-web passes 104 library tests plus its decoder/doc tests, the unchanged
canonical golden remains 234/234 `PASS`, native and WASM M70/M71 transition parity pass 1/1 each,
demo-web WASM passes, and Trunk 0.21.14 emits seven files. This remains development evidence only.
No unchanged-source post-F005/F006 clean release gate, immutable snapshot or served-byte
verification has completed. Do not nominate or test the F004 publication as a current candidate.

### Repository and publication state

The F005/F006 implementation and focused public regression are committed as
`4f5339fa0de6b12794647835ac9066af5520887e` (`fix(m71): compose distinct point axis snaps`). The
milestone records are being reconciled forward from that source. Ordinary reviewable-commit
authority is available, but this handover update itself has not nominated a release product.

Historical F004 source `a2e51efba7d79f684d264094ffd7dd0e37a4d089`, tree
`8b73be00a384fe4a36ebe13fa0c06f32a6694a14`, clean-gate log
`/tmp/geosolve-m71-f004-clean-gate.ZGQEKU.log`, immutable snapshot
`/tmp/geosolve-m71-f004-uat.SaXMVY` and aggregate
`5baf5514f366da60ef9e88d7f53f2e8b0346ff5c5222d8e993529a38272b631b` remain exact historical
evidence. PID `2848202` still serves that snapshot, but it is withdrawn from continued UAT. Do not
stop the server or alter the snapshot merely to express withdrawal; replace UAT authority only
after a clean post-F005/F006 candidate is qualified, frozen and byte-verified.

## Historical 2026-08-14 checkpoint — M71-F004 simultaneous endpoint-axis inference

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

### Regression and checkpoint evidence

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
Trunk 0.21.14 release assembly. Checkpoint owner results included 311/311 constraint-editor unit
tests plus every integration/doc test, the 2/2 public F004 regression, 104/104 demo-web unit tests
plus decoder/doc tests, 17/17 M71 sketch relation tests and 7/7 persistence tests. The required
256-moving-body sparse crossover passed in 151.18 seconds. Cargo emitted only the existing
non-failing `license` plus `license-file` advisories. Because the source was dirty, this is
development evidence rather than clean nomination evidence.

### Historical clean qualification and F004 publication

Clean product source `a2e51efba7d79f684d264094ffd7dd0e37a4d089`, tree
`8b73be00a384fe4a36ebe13fa0c06f32a6694a14`, passed exactly:

```text
env -u GEOSOLVE_ALLOW_DIRTY NO_COLOR=true \
  nix-shell shell.nix --run './scripts/release-gate.sh'
```

The sole worktree on `main` had empty status before and after the gate, and HEAD/tree stayed
unchanged. The gate log is `/tmp/geosolve-m71-f004-clean-gate.ZGQEKU.log`; it ran from 13:04:17 to
13:11:13 AEST on 2026-08-14, passed the 256-moving-body sparse crossover in 125.55 seconds and
completed Trunk 0.21.14 release assembly. Cargo emitted only the longstanding non-failing
`license` plus `license-file` advisories.

The gate-produced seven-file `dist` was copied without rebuilding, byte-compared and frozen at
`/tmp/geosolve-m71-f004-uat.SaXMVY` with directory mode `0555`, file mode `0444` and C-locale
ordered-manifest aggregate
`5baf5514f366da60ef9e88d7f53f2e8b0346ff5c5222d8e993529a38272b631b`. PID `2848202` has exact
argv `/run/current-system/sw/bin/python3 -u -m http.server 8080 --bind 100.94.63.83 --directory
/tmp/geosolve-m71-f004-uat.SaXMVY`, executable
`/nix/store/gxzhl7aaiid7zp3y47jqqiq7zg5mqpwp-python3-3.14.6/bin/python3.14`, and listens only on
`100.94.63.83:8080`; its log is `/tmp/geosolve-m71-f004-uat.SaXMVY.server.log`.

Proxy-disabled, cache-bypassed, identity-encoded requests returned HTTP 200 from remote IP
`100.94.63.83` for every asset with exact size and byte equality. `/` equalled `index.html`; the
snapshot, fetched and post-fetch aggregates all equalled the value above. The fetch evidence is
`/tmp/geosolve-m71-f004-fetch.aDfzU7`.

### Preserved F003 publication

The immutable seven-file snapshot remains mode `0555` with `0444` regular files and ordered
aggregate
`23ab4586acd0f8a86a85e81d7b913ee2736f2524fe81c9913fa3a726496584e0`. It must remain untouched
as historical evidence. PID `1202735` is absent and those bytes are no longer served; the shared
endpoint now serves only the verified F004 snapshot above.

### Historical qualified product and repository checkpoint

At qualification, the sole worktree was
`/home/arduano/programming/geometric-constraint-solver` on `main`, HEAD
`a2e51efba7d79f684d264094ffd7dd0e37a4d089`, tree
`8b73be00a384fe4a36ebe13fa0c06f32a6694a14`, empty status and `origin/main...HEAD` divergence
`0 3`. The F004 implementation/focused regression is commit
`1f542555d7fcaf98ecf92c69a10b951fbfcc3dff`; pre-release documentation is
`ee27de77838a5adb1220c3316ddfcbf4b0380163`; the offline-server correction is
`a2e51efba7d79f684d264094ffd7dd0e37a4d089`. The publication-evidence documentation is distinct
from that qualified product source; its commit ID must be read from repository history. No push
has been made.

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

Those earlier publications remain historical mechanical evidence only. Continued UAT is paused
until a clean, byte-verified post-F005/F006 replacement is published.

At resume, use `git log -5 --oneline --decorate`, `git status --short --branch`, `git worktree
list --porcelain` and `git rev-list --left-right --count origin/main...main` to establish the
current checkpoint. Preserve `a2e51efba7d79f684d264094ffd7dd0e37a4d089` only as the historical
qualified F004 product source. PID `2848202`, its exact argv/listener, snapshot modes and aggregate
may be rechecked as historical evidence, but they do not authorize UAT. Do not assume any commit
has been pushed unless the divergence and `git ls-remote origin refs/heads/main` agree.

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

F004 composes one remembered point/native-midpoint axis with a complementary exact Cartesian new-
span direction. F005 composes complementary axes from two distinct remembered stored points while
retaining both positional references through line/polyline confirmation. F006 tightens only the
default capture envelope to 6/9 px for points/midpoints, 8/12 px for curves and 3/5 degrees for
directions. These inference corrections add no solver equation, branch or persistence format.

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

## Deferred cleanup review

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

1. Re-establish HEAD/status/worktree/divergence and confirm implementation commit
   `4f5339fa0de6b12794647835ac9066af5520887e` plus the reconciled M71 records. Preserve every F004
   snapshot/hash/log fact as historical evidence; do not use its served bytes for UAT.
2. Finish the reviewable documentation commit, then run the focused F005/F006 owner and public
   regressions, relevant collateral, clean golden, formatting, warnings-denied workspace Clippy,
   locked all-feature workspace tests and native/WASM/Trunk checks on one unchanged nominated
   source.
3. Run `./scripts/release-gate.sh` from that clean source without `GEOSOLVE_ALLOW_DIRTY`. Record the
   exact source/tree, complete log and before/after clean-state evidence.
4. Copy the gate-produced `dist` without rebuilding, freeze and manifest-compare an immutable
   post-F005/F006 snapshot, publish it through the Tailscale-only listener and verify every served
   byte plus `/`. Only that verified replacement becomes current UAT authority.
5. Ask the supervising human to perform M71-U1 through M71-U5 from `docs/M71_UAT.md`. U2 must cover
   two distinct stored points contributing Horizontal Y and Vertical X, the F004 point-axis-plus-
   span-direction pairings, native-midpoint pairings, line/polyline paths, exact ties, same-anchor
   exclusion, tighter capture feel and later edits proving both relations survive.
6. Record each human result without inferring unperformed coverage or approval from the mechanical
   gate. Close M71 only after explicit supervising-human approval.

## Deliberately deferred

Do not expand M71 into broad derived-point H/V operands beyond the two explicit native-span
midpoint-axis definitions, M37 catalog consolidation, generic
intersections, quadrant anchors, nonlinear tangent/normal inference, equality/symmetry inference,
host axes/grids/increments, persistent wake state, canonical sketch v5, computed-feature chaining,
browser E2E, mobile support or legacy UI.
