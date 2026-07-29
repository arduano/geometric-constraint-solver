<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M61 candidate remediation report

Date: 2026-07-29

Status: objective remediation complete; replacement human UAT pending

Implementation source: `1f5fd59`; targeted interaction repair: `1c314e9`

## 1. Files and APIs added

- `geosolve-constraint-editor` adds advanced `EditorTool`, `ConstructionProposal`,
  `ConstructionPreview` and option DTOs for quadratic/cubic Bezier, ellipse, elliptical arc,
  rational quadratic conic, parabola, hyperbola and NURBS construction.
- `geosolve-demo-web` adds the matching sole-workbench tools/options, advanced preview rendering,
  active-scenario interaction routing and ten mechanism scenario definitions.
- `CanvasCamera` provides model-center/scale viewport state, cursor-anchored zoom, pan and scene
  fitting. Recursive scenario flyouts preserve visible overflow at every desktop level.
- Dynamic branch selectors preserve a previous value only when the current headless option set
  still contains it; first use and curve-family changes select the first published default.
- Projected browser pointer moves use a latest-sample animation-frame queue. Pointer-up flushes at
  most the latest pending sample once; cancellation and stale frames cannot replay old positions.
- The host-state card reports accepted geometry-role declarations without running legacy visual
  profile analysis. The production-topology companion remains the sole consumability authority.
- No solver, residual, public schema, operation/topology companion, old route or browser harness
  was added.

## 2. Mathematical behavior implemented

Construction proposals allocate ordinary public sketch points/scalars/curves atomically. Conic
values use the owning document curve definitions. NURBS construction supports clamped open and
periodic uniform topology, positive explicit/unit weights, stable semantic span IDs and an
explicit gauge whose selected weight is exactly one. Invalid values, degree/control count,
weight count, gauge or terminal geometry leave the document unchanged.

Preview code does not duplicate curve equations: it applies a localized proposal to a temporary
public `SketchDocument`, obtains visible intervals and samples public curve jets.

The ten mechanism leaves reuse `alpha_scenario`. Stable diagnostics directly confirm their
initial equality/bounded mobility as `1/1`, except the Bezier bridge (`3/1`) and twin-roller cam
(`2/2`). Each preselects its documented driver. Projected pointer effects resolve and publish
through the active ephemeral coordinator, so dependent geometry moves while ordinary workspace
bytes remain isolated. Reset reconstructs the exact public fixture.

Camera transforms change only the web-supplied editor viewport. They never enter the sketch,
scenario evidence or persistence model.

The supplied pathological retained workspace contains five points, two circles, two lines, one
quadratic Bezier, four contacts, two point-on-circle constraints, one line/Bezier tangency and two
driving line-length dimensions. Replaying retained revision 42/attempt 44 against accepted revision
41 reaches a finite numerical solution but remains correctly rejected as
`AmbiguousContactNeighborhood`. The interaction repair changes only when browser samples are
submitted; it does not alter this mathematical result.

## 3. Commands run and outcomes

Focused native/WASM/release qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --all-features'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
```

Outcome: pass. The editor passes 63 unit and seven M55 integration tests; demo-web passes 45
direct tests. WASM check and release Trunk build pass.

Complete release qualification:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check && cargo clippy --locked --workspace --all-targets --all-features -- -D warnings && cargo test --locked --workspace --all-features && cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown && cargo check --locked -p geosolve-sketch-ops --target wasm32-unknown-unknown && cargo check --locked -p geosolve-sketch-topology --target wasm32-unknown-unknown && cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
```

Outcome: pass. Formatting, warnings-denied workspace Clippy, the complete workspace all-feature
test suite, all three WASM checks and the release Trunk build completed successfully.

Post-candidate `M61-F003` qualification:

```bash
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor --all-features --test m55'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo clippy --locked -p geosolve-constraint-editor -p geosolve-demo-web --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cargo fmt --all -- --check'
```

Outcome: pass. The exact retained-payload regression is one of 12 passing M55 integration tests;
demo-web passes 48 direct tests, including the pointer queue lifecycle regression.

## 4. Acceptance criteria passed

- Every new mechanism reports the documented nonzero mobility and one valid selected driver.
- Repeated bidirectional twin-roller drags include the opposite persistent center as a transient
  stability target and retain that passive center within `1e-9` of its accepted position.
- An authored line endpoint plus circle resolves Coincident to point-on-curve and publishes using
  the untouched periodic contact defaults; empty or obsolete dynamic browser choices cannot
  override those defaults.
- The exact supplied retained contact graph remains rejected for its ambiguous neighborhood, then
  accepts a small projected retry without weakening validation.
- Browser projected drag retains only the latest pending sample per animation frame, drains it at
  most once on pointer-up and invalidates stale callbacks.
- A projected scissor drag advances accepted state and moves dependent geometry; Reset restores
  exact initial geometry/selection.
- Active scenario interaction and camera controls do not publish ordinary workspace persistence.
- Desktop third-level compact/linkage flyouts expand right without nested clipping scroll state.
- Every requested advanced family applies atomically, solves through the retained public session
  and evaluates finite public curve jets.
- Invalid conic/NURBS values and topology retain editor/document state.
- Cursor-anchored zoom, screen-direction pan and fit-to-scene are directly tested.
- No `unsafe`, FFI, equation duplication, weighted-priority substitution, `/#/dev/lab`, old
  playground, CDP or browser E2E was introduced.

No new residual was added, so no new residual Jacobian implementation or finite-difference
Jacobian test is required.

### Post-candidate interaction repair

Human review found `M61-F001`: the generic web projected-drag path omitted the `MotionCam`
fixture's passive stability target, so the other independent roller could jump along its own DOF
and make dragging severely laggy. Repair `1c314e9` adds a small headless coordinator method that
accepts only the passive persistent point identity, obtains its authoritative accepted position
and constructs the temporary stability request. Scenario metadata maps both roller directions to
their opposite passive center. A direct repeated-drag regression checks both directions and
inspects the actual transient request; no solver equation, persistent constraint or browser-owned
coordinate policy was added.

Human review then supplied `M61-F003`, a retained workspace whose coupled contact graph makes some
projected centre solves materially more expensive. The WASM listener previously ran every raw
move synchronously, allowing stale events to queue behind an expensive solve. The workbench now
coalesces to the latest sample per animation frame and performs one terminal drain before
pointer-up. An exact native regression preserves the original accepted/design revision split,
asserts the retained ambiguity rejection and proves a small projected retry can recover. No solver
equation, validation rule or schema changed.

The targeted recheck exposed `M61-F004`: startup restore was not the dominant lock. Direct
`wasm32` replay completes in about 171 ms, while legacy full accepted-profile analysis inside
`host_state_markup` took about 2.3 seconds in optimized native code and reran after every render.
That sidebar content duplicated the separately qualified production-topology surface. It now lists
only accepted profile/construction role declarations, with an explicit non-consumability caveat.
Exact native panel generation falls to about 0.12 ms and production topology remains unchanged.

## 5. Known limitations and next blocker

- The workbench remains a replaceable desktop diagnostic consumer, not a production renderer.
- Advanced construction authors complete curve definitions; property editing after commit remains
  ordinary document/API work rather than a new inspector workflow.
- NURBS creation uses deterministic clamped-open or periodic-uniform knots. Arbitrary user knot
  vectors are not exposed in this UAT control.
- Scenario edits deliberately remain ephemeral and are reset/exit scoped.
- M61 still requires explicit supervising-human approval of the replacement scorecard in
  `docs/M61_UAT.md`.
