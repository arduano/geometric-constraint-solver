# M65 implementation: predictable bounded projected dragging

Status: complete and explicitly approved by the supervising human on 2026-08-01. The reduced-scope
implementation, replacement-candidate mechanical qualification and focused retests of
`M65-F004` and `M65-F005` all pass.

## Scope

M65 has one goal: predictable, synchronously bounded projected dragging for the existing editable
mechanism samples.

The selected control should follow the cursor on its current local configuration, mathematically
independent passive controls should remain stationary, and ordinary dragging must not choose a
different assembly branch implicitly. Rejected or exhausted work preserves the complete last
independently accepted preview, and a later valid sample may recover in the same gesture.

Stability and understandable UX take priority over throughput. Performance remains a requirement
through a finite operation envelope, not through a frozen wall-clock or machine-specific work
comparison.

M65 explicitly excludes:

- alternate-assembly branch search, ghost previews, accept/cancel branch UI and branch-only
  samples;
- sample IDs or browser-owned driver/anchor policy;
- new residual families or relaxed Hard/Temporary validation;
- weighted least squares as a priority substitute;
- global root enumeration, worker architecture or a new persistence language.

## 1. Files and APIs added or changed

The implementation is divided by owner:

- `crates/geosolve-core/src/linearization.rs` exposes controlled accepted-hard component
  nullspace evidence without changing rank policy.
- `crates/geosolve-core/src/solver.rs` and `src/session.rs` independently certify publishable
  Hard/Temporary state, reject invalid terminal/audit evidence and protect the complete positive
  Temporary vector on the single-component dense path. Follow-up `M65-F005` adds a fixed-size
  rank-aware `2 x 2` SVD path with stationarity and minimum-norm certification for the common
  rank-one two-coordinate cursor projection.
- `crates/geosolve-core/tests/m5_priority.rs` and `tests/m10.rs` cover positive-row preservation,
  separable Preference motion, invalid terminal/audit publication, exact-state mismatch and
  invalid evaluator certification.
- `crates/geosolve-sketch/src/compiler.rs`, `src/session.rs`, `src/document_session.rs` and
  `src/document_lowering.rs` implement controlled locality planning, exact transient objective
  lowering, accepted-preview continuation and bounded exact release.
- `crates/geosolve-sketch/src/lib.rs` exports the opaque `DocumentDragLocalityPlan`; consumers can
  inspect only point-observable passive-DOF and anchor-count evidence, not solver matrices or
  anchor identities.
- `crates/geosolve-sketch/tests/m34_lifecycle.rs` and `tests/m65_locality.rs` cover frozen accepted
  targets, symmetric continuation, deterministic greedy ordering/minimal cover and the exact
  cursor/anchor objective inventory.
- `crates/geosolve-constraint-editor/src/lib.rs` stamps gesture epochs, preserves circle-handle
  offsets and classifies stale/untracked projected results. Follow-up `M65-F004` gives directly
  draggable point/semantic-curve geometry priority only where it overlaps an annotation, while
  offset labels and non-draggable annotation interaction remain unchanged.
- `crates/geosolve-constraint-editor/src/coordinator.rs` owns press-time planning, exactly one
  controlled attempt per sample, last-valid-preview retention, work evidence and controlled exact
  release. Its inline tests own the representative mechanism, lifecycle, stale-result and literal
  envelope corpus.
- `crates/geosolve-demo-web/src/workbench/mod.rs` routes pointer-down through the coordinator so
  the plan is captured from the exact visible accepted state.
- `crates/geosolve-demo-web/src/workbench/persistence.rs` directly qualifies authoring through
  workspace encode/decode/restore and subsequent editing.
- `crates/geosolve-sketch/src/alpha_scenarios.rs`,
  `crates/geosolve-demo-web/src/workbench/samples.rs`, the related M14/M30 tests and workbench
  HTML/CSS remove the discarded branch-search prototype and its two branch-only samples. The
  ordinary M64 sample catalog otherwise remains unchanged.

The public surface added for M65 is intentionally small: an opaque
`DocumentDragLocalityPlan`, controlled retained-session plan/attempt/release methods, coordinator
pointer-down ownership and typed projected-drag work/rejection evidence. No alternate-branch DTO,
search API, sample policy or persisted field is part of the reduced candidate.

`geosolve-sketch` owns an opaque, gesture-local drag-locality plan derived from the independently
accepted hard nullspace. Its public surface remains small: presentation consumers do not receive
nullspace vectors, solver matrices or a policy they can reinterpret.

The planner:

1. measures the active point's rank in the accepted hard nullspace;
2. computes the point-observable passive mobility not covered by that active rank;
3. considers ordinary accepted points in compile order;
4. chooses anchors by greatest new rank gain, then lower point mobility rank, then compile order;
5. captures those anchor targets from gesture-start accepted visible geometry.

A remaining hard-nullspace direction that changes only scalar curve/contact state is invisible to
every persistent point. It cannot be represented by a point-position Preference and therefore
neither admits nor requires a locality anchor. `passive_degrees_of_freedom` reports only the
point-observable passive cover.

The solve request compiles the cursor point as the sole Temporary target and only the planned
anchors as PreviousState Preferences. Numerical seeds may advance between samples; the locality
targets do not.

`geosolve-constraint-editor` owns the gesture lifecycle:

- pointer ID and monotonically increasing request ID;
- active semantic point, including circle-circumference-to-center mapping and initial pointer
  offset;
- exact design and accepted identity at gesture start;
- the opaque locality plan;
- the complete last independently accepted preview;
- typed outcome/work evidence for the latest non-stale sample.

The web workbench consumes these headless results. It does not select anchors, retry candidates,
inspect solver matrices or duplicate equations.

## 2. Mathematical behavior implemented

Every non-stale pointer sample executes exactly one retained attempt. The first sample starts from
the authoritative accepted state; later samples continue from the complete last independently
accepted preview. Rejection and operation exhaustion do not replace that preview. A request whose
ID is stale or out of order is a no-op and cannot overwrite newer geometry.

The core retains strict Hard → Temporary → Preference semantics:

- a success-like result independently validates every Hard row;
- on the single-component dense path, a positive Temporary attainment captures and independently
  re-evaluates the complete normalized residual vector;
- Preference processing on that path may publish only a finite candidate that preserves every
  entry of the certified vector within
  `max(min(normalized_residual_tolerance, normalized_step_tolerance), 8 * f64::EPSILON)`;
- the `8 * f64::EPSILON` floor is solely the machine reproducibility band for comparing an
  already attained positive Temporary vector after Preference work; Hard validation and
  Temporary attainment keep their configured tolerances unchanged;
- coupled-priority solving remains unchanged and continues to protect scalar attained Temporary
  levels;
- failure to preserve the applicable vector or scalar level retains the independently certified
  attained state or rejects; raw post-Temporary numerical drift is never authoritative;
- accepted and no-motion report reconstruction rejects invalid-geometry or numerical-failure
  termination and requires successful audit-row evaluation; truthfully non-optimal secondary
  termination remains separate from independently valid Hard geometry.

The retained rank-aware pointer solve uses only the minimal `2 × N` active-point response needed
for locality planning; it does not broaden the priority architecture.

Release independently validates and publishes the exact last preview as one ordinary history edit.
Cancel publishes nothing. Undo/Redo operate on the resulting ordinary edit. Transient cursor and
anchor objectives never enter persisted design intent.

### Projected-sample operation envelope

Every projected sample uses the same finite limits:

| Operation counter | Limit |
| --- | ---: |
| Document validation items | 16,384 |
| Document dependency items | 16,384 |
| Document lowering items | 16,384 |
| Nonlinear iterations | 256 |
| Factorizations | 256 |
| Rank kernels | 256 |
| Rejected trials | 512 |
| Component linearizations | 1,024 |
| Dense kernel rows | 256 |
| Dense kernel columns | 256 |
| Diagnostic candidates | 512 |
| Diagnostic trials | 1,024 |

Crossing any limit is a controlled rejection. It preserves the last valid preview and must not
partially publish design, accepted geometry, history or audit state.

### Direct coverage

The mechanically qualified candidate directly covers:

- both twin rollers across horizontal, vertical, diagonal and reversal paths, with passive-center
  movement `<= 1e-8`;
- a difficult twin-roller rejection followed by valid same-gesture recovery;
- pantograph input, guide, output and center;
- natural off-manifold pantograph-guide projection with reciprocal passive-arm stability;
- Scotch-yoke dragging after deletion of its horizontal guide, including reversals;
- scissor jack and five-stage tower continuity;
- semantic circle-center dragging with a nonzero pointer offset;
- symmetric `MotionCam` center/circumference pointer routing through the left driving-radius
  overlap while its offset label remains selectable;
- release, cancel, Undo and Redo;
- late, duplicate and out-of-order queued results;
- ordinary constraint authoring plus workspace save/reload;
- core rejection of invalid Hard/Temporary publication and invalid accepted/no-motion reports.

No exact work totals or percentage improvement become a compatibility baseline. The finite envelope
is normative; wall-clock and detailed counters are diagnostic characterization only.

## 3. Exact commands run and outcomes

The integrated reduced candidate ran:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
cd crates/geosolve-demo-web
nix-shell ../../shell.nix --run 'env NO_COLOR=true trunk build --release'
git diff --check
```

Outcome (2026-08-01): all commands pass. The locked all-feature workspace suite passes with only
the pre-existing explicitly ignored manual measurement/release-performance tests; all executed
unit, integration and doc tests pass. Trunk 0.21.14 emits a successful optimized distribution.
The Cargo manifest warning that both `license` and `license-file` are present is pre-existing
metadata advice and is not a warnings-denied Rust/Clippy failure.

Qualified replacement code source: `b6433d1`. At review time the exact code was served for focused
UAT at `http://100.94.63.83:8080/`; that historical endpoint is not a continuing service.

## 4. Acceptance criteria passed

The following objective acceptance areas pass:

- deterministic gesture-start locality planning and exact transient objective ownership;
- single-attempt continuation, rejection/exhaustion retention, recovery and stale-result no-ops;
- symmetric offset-preserving twin-roller interaction and representative mechanism paths;
- independently certified Hard/positive-Temporary publication and invalid-report rejection;
- the literal synchronous operation envelope, including press planning and exact release;
- release/cancel/Undo/Redo plus authoring/workspace/editability lifecycle;
- formatting, warnings-denied Clippy, native tests, WASM, release Trunk and diff hygiene.

The unaffected M65-U1 and M65-U4 areas retain the supervising-human closure-review result. M65-U2
and M65-U3 also passed focused human retest of the replacement candidate. The discarded prototype
does not supply acceptance evidence for this candidate.

## 5. Known limitations or next blocker

- M65 guarantees local predictable behavior only for the existing editable mechanism sample
  surface and directly covered ordinary authoring/lifecycle flows.
- It does not provide an explicit command for changing assembly branch.
- Bounded rejection is an expected truthful outcome for a target that cannot be reached inside
  the current local configuration and operation envelope.
- The withdrawn broader prototype remains recoverable only on
  `recovery/m65-f003-overbuilt-20260731`; none of its branch-search UI or samples is in `main`.
- `M65-F004` and `M65-F005` have direct tested dispositions and approved focused human retests.
- Alternate-assembly exploration remains future scope rather than an implicit part of projected
  dragging.
