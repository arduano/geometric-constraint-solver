<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M66 implementation: headless CAD helper-operation authoring

Status: replacement mechanically qualified as of 2026-08-01. Supervising-human UAT opened
`M66-F001` through `M66-F003`; their direct dispositions and replacement gate now pass, while the
focused human retest remains pending.

## Scope

M66 adds reusable CAD-like authoring for three equation-free sketch helpers:

- a bounded, local, branch-explicit associative 2D fillet with flexible pointer radius placement;
- associative offsets of one line or polyline span in exact-translated or supporting-line mode,
  plus one bounded explicitly selected one-shot joined-chain form;
- exact single-source mirrors for line/polyline, Bezier and non-rational B-spline families.

The headless editor owns operand collection, applicability, explicit options, warning recovery,
scratch preview and exact publication. The operations companion remains the deterministic public
transaction producer, and the workbench remains a thin renderer/event adapter. Persistent
associative multi-span offsets, automatic chain discovery, general curve/profile offsets,
approximate mirrors, global fillet-root enumeration, multi-source mirror, feature history, browser
E2E and `/#/dev/lab` remain outside this milestone.

## UAT remediation scope

- `M66-F001`: repeated Line offset clicks could not author a joined path. The remediation accepts
  an explicitly clicked ordered chain of at most 32 unique endpoint-connected line/polyline spans
  and emits one atomic mitered ordinary polyline. It is deliberately one-shot and carries no
  persistent distance scalar, dimension or association.
- `M66-F002`: selecting an ordinary open-polyline corner did not resolve the intended two fillet
  parents. The remediation accepts the two adjacent spans directly and expands one unambiguous
  interior point to that ordered pair with explicit `End`/`Start` trim ownership; other
  same-support or ambiguous cases still reject.
- `M66-F003`: the fallback Fillet radius became a driving constraint without direct placement, and
  deleting that dimension did not make the visible arc body draggable. The remediation adds a
  headless pointer-radius stage, defaults its output to Reference unless Driving is explicit, and
  exposes a circular arc's stored center as its semantic canvas drag owner.

## 1. Files and APIs added or changed

- `crates/geosolve-sketch-ops/src/lib.rs` requires accepted geometry from the exact current
  retained attempt input before a geometry operation may be proposed. A same-design accepted state
  from an older or rejected input returns typed
  `SketchOperationIncompleteReason::AcceptedStateForDifferentInput`. The F001 remediation adds a
  closed joined-chain request/result that creates one ordinary mitered polyline without a retained
  offset relation.
- `crates/geosolve-constraint-editor/src/operation_authoring.rs` adds the separate public
  `OperationAuthoringState`, its closed tool/options/stage/guidance/pick/warning/outcome DTOs and
  bounded local fillet synthesis. F001-F003 extend that same state with explicit path collection,
  an atomic polyline-corner shortcut and reference-radius placement; they do not overload M62
  constraint authoring.
- `crates/geosolve-constraint-editor/src/coordinator.rs` owns exact accepted-input pick stamping,
  immutable operation snapshots, controlled scratch proposal application, independently accepted
  preview state, opaque preview tokens and exact candidate-bound publication. A successful apply
  records one ordinary history/replay action and selects the primary created curve.
- `crates/geosolve-constraint-editor/src/lib.rs` exports only the presentation-neutral authoring,
  preview and semantic curve-drag surface required by hosts. Circular arcs map to their stored
  center there; ADR 0030 authorizes the editor-to-operations dependency.
- `crates/geosolve-demo-web` removes its direct operations-companion dependency. Its sole
  workbench adds a **Modify** palette, text-free Fillet/Line offset/Mirror icons, normalized event
  forwarding, headless guidance/options/warnings, accepted scratch rendering and exact Apply/
  Enter/Escape routing. It forwards corner/path/radius pointer events but owns no expansion,
  radius or miter formula.
- `crates/geosolve-demo-web/src/workbench/samples.rs` adds three ordinary editable sample
  workspaces: **2D fillet workshop**, **Associative line offsets** and
  **Mirror construction workshop**. The fillet leaf uses only ordinary deletable support locks and
  a driving source-circle radius so strict radius/parent edits remain stable; it has no protected
  scenario state.
- `crates/geosolve-demo-web/src/workbench/persistence.rs` advances the application envelope to v3,
  migrates v1/v2 conservatively and retains independently checked current-design accepted
  provenance so a flexible fillet reloads bit-for-bit without trusting user-editable storage.
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

Fillet authoring performs one deterministic bounded local multi-start search around either two
different picked supports or two adjacent spans of one open polyline. One unambiguous interior
point expands to that ordered pair. Before the existing M28 operation is prepared, it materializes:

- both curve spans and contact parameters;
- bounded/periodic neighborhoods and winding;
- explicit first/second normal sides;
- the retained trim endpoint for each parent;
- periodic trim anchors where applicable;
- output endpoint order, counter-clockwise sweep and minor/alternate arc choice;
- a finite positive radius and driving/reference mode.

After the parents are known, the headless state exposes a distinct pointer-radius placement stage.
`0.1 * model_scale` is only its finite fallback seed. Pointer motion refreshes a non-committable
scratch preview, click confirms the finite positive radius, and Apply remains bound to that exact
candidate. The ordinary radius dimension defaults to Reference, retaining the regular fillet DOF;
Driving is explicit user intent. Picked retained portions and a minor arc remain the default, while
first-side, second-side and alternate-arc corrections are explicit. Unsupported same-support
pairs, endpoint/ambiguous corners, already-trimmed parents, escaped spans, zero-speed/cusp/pole
geometry, singular offsets and tied materially distinct roots fail typed without mutation or
global search.

The accepted output arc stays ordinary geometry. Its stored center is presentation-neutral drag
metadata for the visible arc body. A free/reference fillet therefore uses the existing controlled
projected-point lifecycle to change center, radius, contacts and trim intervals. A driving radius
may truthfully have no remaining mobility, and rejected drags retain prior accepted geometry and
explicit side/order/sweep state.

Single-span line offset authoring creates two target points, one target line, one positive scalar
and the existing driving offset dimension in one proposal. Side and `Same` orientation are
explicit. Exact translated segment is the default; supporting-line mode truthfully retains
axial-slide and length freedom.

Joined offset has separate one-shot semantics. The state accepts only explicit unique picks along
one endpoint-connected ordered path, bounded to 32 spans. The operation offsets every selected
line support by one requested signed distance, intersects adjacent offset supporting lines for
interior miters and atomically emits one ordinary polyline. No offset scalar, dimension,
association, recursive neighboring-span discovery, healing or cap is persisted. Invalid path or
unresolved miter input rejects before allocation. No nonlinear curve/profile approximation is
implied.

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

Replacement code source `92e6ddce1e37d6508b5dd8568078146ac2822aa7` passed the exact
integrated gate on 2026-08-01. Initial source `f913fb46e14308dc66563d1e602d3ae6ed2f7cb1`
is superseded and retained only as historical evidence:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
cd crates/geosolve-demo-web
nix-shell ../../shell.nix --run 'env NO_COLOR=true trunk build --release'
git diff --check
```

Outcomes:

- formatting passed without changes;
- warnings-denied workspace/all-target/all-feature Clippy passed; Cargo emitted only the existing
  non-failing manifest advice about declaring both `license` and `license-file`;
- every executed workspace unit, integration and documentation test passed; the explicitly manual
  or release-performance-only tests remained ignored;
- the all-feature `wasm32-unknown-unknown` check passed;
- Trunk 0.21.14 produced the optimized release distribution successfully; and
- `git diff --check` reported no whitespace errors.

## 4. Acceptance status

The replacement candidate passes its objective implementation and mechanical acceptance
criteria. Direct coverage owns operation expansion, state transitions, exact picked parameters,
local fillet branches, single-span offset modes/sides, bounded joined-chain construction and
invalid classes, adjacent-polyline direct/corner fillets, flexible/locked radius placement,
semantic arc-center drag, accepted scratch provenance, token/candidate/confirmation binding,
cancellation/exhaustion/staleness, exact scratch/commit equality, selection, history/replay,
bit-exact workspace round-trip and editable plain samples. The retained M25/M27/M28 derivative
and independent-validation corpora also pass; M66 adds no residual.

F001-F003 have direct owning-layer regressions and the complete replacement native/WASM/release
gate passes. The explicit supervising-human acceptance checkbox and focused human retest remain
open, so M66 is not yet closed.

## 5. Known limitations or next blocker

- Fillet root synthesis is deliberately local and bounded; an ambiguous or unavailable local root
  is a truthful warning, not a request to search every branch.
- Joined offsets are bounded explicit one-shot polylines. They are not associative, discover no
  neighboring spans and provide no caps or self-intersection healing.
- Mirrors are exact only for point-defined supported families and commit one source at a time.
- The workbench remains a non-authoritative desktop UAT consumer.
- The only remaining gate is focused supervising-human retest and explicit approval of
  `docs/M66_UAT.md`.
