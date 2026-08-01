<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M66 implementation: headless CAD helper-operation authoring

Status: implementation and direct qualification in progress. The supervising-human UAT remains
open and is the final M66 gate.

## Scope

M66 adds reusable CAD-like authoring for three equation-free sketch helpers:

- a bounded, local, branch-explicit associative 2D fillet;
- associative offsets of one line or polyline span in exact-translated or supporting-line mode;
- exact single-source mirrors for line/polyline, Bezier and non-rational B-spline families.

The headless editor owns operand collection, applicability, explicit options, warning recovery,
scratch preview and exact publication. The operations companion remains the deterministic public
transaction producer, and the workbench remains a thin renderer/event adapter. General curve or
profile offsets, approximate mirrors, global fillet-root enumeration, multi-source mirror, feature
history, browser E2E and `/#/dev/lab` remain outside this milestone.

## 1. Files and APIs added or changed

- `crates/geosolve-sketch-ops/src/lib.rs` requires accepted geometry from the exact current
  retained attempt input before a geometry operation may be proposed. A same-design accepted state
  from an older or rejected input returns typed
  `SketchOperationIncompleteReason::AcceptedStateForDifferentInput`.
- `crates/geosolve-constraint-editor/src/operation_authoring.rs` adds the separate public
  `OperationAuthoringState`, its closed tool/options/stage/guidance/pick/warning/outcome DTOs and
  bounded local fillet synthesis. It deliberately does not overload M62 constraint authoring.
- `crates/geosolve-constraint-editor/src/coordinator.rs` owns exact accepted-input pick stamping,
  immutable operation snapshots, controlled scratch proposal application, independently accepted
  preview state, opaque preview tokens and exact candidate-bound publication. A successful apply
  records one ordinary history/replay action and selects the primary created curve.
- `crates/geosolve-constraint-editor/src/lib.rs` exports only the presentation-neutral authoring
  and preview surface required by hosts. ADR 0030 authorizes the editor-to-operations dependency.
- `crates/geosolve-demo-web` removes its direct operations-companion dependency. Its sole
  workbench adds a **Modify** palette, text-free Fillet/Line offset/Mirror icons, normalized event
  forwarding, headless guidance/options/warnings, accepted scratch rendering and exact Apply/
  Enter/Escape routing.
- `crates/geosolve-demo-web/src/workbench/samples.rs` adds three ordinary editable sample
  workspaces: **2D fillet workshop**, **Associative line offsets** and
  **Mirror construction workshop**. The fillet leaf uses only ordinary deletable support locks and
  a driving source-circle radius so strict radius/parent edits remain stable; it has no protected
  scenario state.
- `crates/geosolve-demo-web/src/workbench/persistence.rs` directly qualifies that committed
  operation output uses the ordinary versioned workspace envelope and remains editable after
  restore.
- `README.md`, `ARCHITECTURE.md`, `CHANGELOG.md`, `docs/API_COMPATIBILITY.md`, `PLAN.md`,
  `ACCEPTANCE.md`, `docs/SCENARIOS.md`, ADR 0030 and `docs/M66_UAT.md` record the boundary,
  objective gates and focused human scorecard.

The public surface remains intentionally closed to these three tools. No solver equation, generic
curve trait, browser applicability table or persisted feature-operation schema is added.

## 2. Mathematical behavior implemented

Every coordinator-stamped, preview-eligible pick carries its persistent curve span, exact
parameter, finite accepted model position and complete originating retained input. Standalone
public pick construction remains useful for headless synthesis, but only coordinator-bound picks
may reach scratch preview. A complete candidate can be previewed only while all bound picks and the
accepted geometry still match that exact input.

Fillet authoring performs one deterministic bounded local multi-start search around two distinct
picked spans. Before the existing M28 operation is prepared, it materializes:

- both curve spans and contact parameters;
- bounded/periodic neighborhoods and winding;
- explicit first/second normal sides;
- the retained trim endpoint for each parent;
- periodic trim anchors where applicable;
- output endpoint order, counter-clockwise sweep and minor/alternate arc choice;
- a finite positive radius and driving/reference mode.

The default radius is `0.1 * model_scale`. Picked retained portions and a minor arc are the default;
first-side, second-side and alternate-arc corrections are explicit. Duplicate supports,
already-trimmed parents, escaped spans, zero-speed/cusp/pole geometry, singular offsets and tied
materially distinct roots fail typed without mutation or global search.

Line offset authoring creates two target points, one target line, one positive scalar and the
existing driving offset dimension in one proposal. Side and `Same` orientation are explicit.
Exact translated segment is the default; supporting-line mode truthfully retains axial-slide and
length freedom. No nonlinear curve approximation, chain join or cap is implied.

Mirror authoring collects one supported source and one distinct line/polyline axis. Existing
point-symmetry constraints keep line/polyline, quadratic/cubic Bezier and non-rational B-spline
outputs associative. Circle, arc, conic, rational and NURBS families fail typed and allocate
nothing.

For every tool, the coordinator applies the proposal first to bounded scratch retained state. It
exposes a scene only after ordinary independent acceptance for the exact input. Apply repeats the
same public proposal on a live clone and requires its design, prepared input, accepted identity and
accepted document to equal the rendered scratch result before swapping it into live state.
Cancellation, exhaustion, stale input, unsupported/incomplete work or retained rejection cannot
publish partial state or leave a committable preview.

## 3. Exact commands run and outcomes

Focused operations, editor/coordinator and workbench suites are being run throughout
implementation. The final candidate must pass this exact integrated gate from one source state:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
cd crates/geosolve-demo-web
nix-shell ../../shell.nix --run 'env NO_COLOR=true trunk build --release'
git diff --check
```

Final outcomes, exact candidate commit and endpoint will be recorded after the clean gate.

## 4. Acceptance criteria passed

Objective acceptance remains in progress until the integrated gate completes. Direct coverage
owns operation expansion, state transitions, exact picked parameters, local fillet branches,
offset modes/sides, supported/unsupported mirror families, accepted scratch provenance,
token/candidate/confirmation binding, cancellation/exhaustion/staleness, exact scratch/commit
equality, selection, history/replay, workspace round-trip and editable plain samples.

The explicit supervising-human acceptance checkbox remains open. M66 cannot close from mechanical
qualification alone.

## 5. Known limitations or next blocker

- Fillet root synthesis is deliberately local and bounded; an ambiguous or unavailable local root
  is a truthful warning, not a request to search every branch.
- Offsets are limited to one line or polyline span and provide no joins, caps or self-intersection
  healing.
- Mirrors are exact only for point-defined supported families and commit one source at a time.
- The workbench remains a non-authoritative desktop UAT consumer.
- The next blocker after clean qualification is explicit supervising-human approval of
  `docs/M66_UAT.md` against the exact Tailscale candidate.
