<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M66 implementation: polished associative 2D Fillet authoring

Status: active after the 2026-08-02 Fillet-only pivot. Replacement build-source commit `ff15c78`
passes the complete mechanical gate including `M66-F012`; supervising-human UAT remains open.

## Scope and preserved history

M66 now has one goal: exceptionally predictable, reusable authoring of an associative 2D Fillet.
The constraint-editor layer owns the reusable flow: its headless authoring state owns parent
collection, applicability, branch/radius options and warning recovery, while its retained
coordinator owns shared hover/click acquisition, scratch preview and exact publication. The
operations companion remains the deterministic public transaction producer, and the workbench
remains a thin renderer/event adapter.

The superseded unapproved Fillet/Offset/Mirror candidate is preserved at
`origin/archive/m66-three-helper-tools-2026-08-02`, commit `80d4939`. Active main removes only that
candidate's Offset/Mirror authoring states, palette actions, options, samples and authoring tests,
plus its M66-only single-span and joined-chain line-offset request APIs. This pivot does **not**
remove or deprecate:

- M25's separately named signed supporting-line and exact-translated-segment Offset constraints;
  or
- M58's exact supported-family Mirror operation-companion API and history.

Global Fillet-root enumeration, a persistent feature tree, browser E2E, `/#/dev/lab` and mobile
behavior remain outside M66.

## UAT remediation scope

The Fillet-relevant earlier findings remain active regression obligations:

- `M66-F002`: one unambiguous open-polyline interior corner must resolve to its ordered adjacent
  spans and explicit `End`/`Start` trim ownership.
- `M66-F003`: pointer radius placement defaults to Reference, Driving is explicit, and a free
  accepted arc remains draggable through semantic center metadata even after deleting its radius
  dimension.
- `M66-F004`: preview-only foreground geometry blocks hidden source geometry without becoming a
  live parent or stealing a Fillet placement click.
- `M66-F005`: curve acquisition uses the inclusive 12-pixel nearest/tie policy.

The post-pivot remediation adds six findings:

- `M66-F007` — **canvas-overlay controls:** move Fillet options outside the scrolling palette and
  place them as a viewport-clamped overlay over the canvas. Pure placement tests own all viewport
  edges and resize.
- `M66-F008` — **recoverable invalid hover:** an invalid unconfirmed radius hover clears only the
  transient candidate/preview, retaining both parents and the radius-placement stage. Confirmed
  terminal failure may still re-arm collection mode.
- `M66-F009` — **shared headless hover/click acquisition:** operation hover and click use the same
  preview-aware headless hit test, including the exact inclusive 12-pixel boundary, nearest
  distance, stable identity ties and preview-only barrier semantics.
- `M66-F010` — **current-publication eligibility after point edits:** sketch-owned compatibility
  ignores only the transient `candidate_request` while matching design, publication request,
  solver policy, activation, parameters, external snapshots, accepted identity and attempt
  identity. Exact proposal compare-and-swap remains unchanged.
- `M66-F011` — **certified post-creation contact bounds:** affine line/polyline contacts use full
  `Interior` support. For a pair with exactly one non-affine parent, its accepted curved contact
  uses a strict `Local` cell certified with outward-rounded tangent intervals over the complete
  bounded support or one explicit unwrapped period. The cell cannot cross a tangent-parallel
  barrier relative to the fixed affine-parent direction. Two non-affine-parent authoring is typed
  unsupported until pairwise continuation exists; the underlying M28 generic Fillet API remains
  unchanged.
- `M66-F012` — **immediate accepted-arc interaction:** after successful publication, a shared
  host completion reducer notifies the headless state, exits Fillet collection and explicitly
  restores ordinary Select, allowing the selected Reference arc to resize immediately with its
  non-driving dimension still present. A failed Apply attempt clears the terminal candidate and
  re-arms collection.

The superseded Offset findings `M66-F001` and `M66-F006` are archived with the three-tool candidate
and are not active M66 UAT checks.

## 1. Files and APIs being reconciled

- `crates/geosolve-sketch/src/profiles/fillet_branch.rs` owns the public, non-mutating
  `SketchDocument::certify_line_curve_fillet_branch_cell` query and typed failure surface. It
  reuses the outward-rounded private interval/curve-piece kernel without exposing visual-profile
  DTOs or duplicating curve equations.
- `crates/geosolve-sketch/src/document_session.rs` owns publication-compatible attempt-input
  comparison and access to accepted state for the current publication. It must ignore only a
  transient candidate request and must not loosen stale/different-input rejection or proposal CAS.
- `crates/geosolve-constraint-editor/src/operation_authoring.rs` is reduced to a genuinely
  Fillet-only closed state machine. It owns parent picks, explicit Fillet branch/radius state,
  recoverable unconfirmed hover, candidate confirmation and terminal outcomes.
- `crates/geosolve-constraint-editor/src/coordinator.rs` owns immutable operation snapshots,
  publication-compatible accepted-state eligibility, controlled scratch proposal application,
  independently accepted preview state, opaque preview tokens and exact candidate-bound
  publication.
- `crates/geosolve-constraint-editor/src/lib.rs` owns the shared preview-aware operation hit path
  used by both hover and click. Circular arcs retain their stored center as semantic drag metadata.
- `crates/geosolve-sketch-ops/src/lib.rs` retains M58 operations, including exact Mirror and the
  public associative-Fillet integration, while removing only the unreleased M66 line-offset
  request/result additions.
- `crates/geosolve-demo-web` exposes one text-free **Fillet** action. Its controls render as a
  viewport-clamped canvas overlay, its geometry hover consumes headless acquisition, and no M66
  Offset/Mirror action, options, icon, sample or persistence fixture remains.
- `crates/geosolve-demo-web/src/workbench/samples.rs` retains only the ordinary editable **2D
  fillet workshop** for M66. It contains no protected state, guide, scripted action or alternate
  coordinator.
- The application workspace v3 provenance check remains relevant to flexible-Fillet reload. It is
  only a routing hint; the sketch domain still independently exact-certifies restored acceptance.
- `README.md`, `ARCHITECTURE.md`, `CHANGELOG.md`, `docs/API_COMPATIBILITY.md`, `PLAN.md`,
  `ACCEPTANCE.md`, `docs/SCENARIOS.md`, amended ADR 0030 and `docs/M66_UAT.md` record the narrowed
  boundary and open gate.

No solver equation, generic curve trait, browser applicability table or persisted feature-operation
schema is added.

## 2. Mathematical and interaction behavior targeted

Every coordinator-stamped parent pick carries its persistent curve span, exact parameter, finite
accepted model position and complete originating retained input. Only coordinator-bound picks may
reach scratch preview. A complete candidate remains previewable only while its picks and eligible
current accepted publication agree.

Fillet authoring performs deterministic bounded local multi-start search around two different
picked supports or two adjacent affine spans of one open polyline. One unambiguous interior point
expands to the ordered adjacent pair. Before the existing M28 operation is prepared, the request
materializes:

- both curve spans and contact parameters;
- winding and explicit first/second normal sides;
- retained trim endpoints and periodic anchors where applicable;
- output endpoint order, counter-clockwise sweep and minor/alternate arc choice; and
- a finite positive radius with explicit Driving/Reference mode.

The post-creation neighborhood policy is closed and deliberately conservative:

- two affine line/polyline spans use `ContactNeighborhood::Interior` on both parents;
- with exactly one affine parent, that span remains `Interior` and the non-affine parent receives
  a strict `Local` cell containing its selected root; and
- two non-affine parents return typed `UnsupportedFilletPair` authoring feedback until a bounded
  pairwise-continuation contract is implemented.

The sketch-owned branch-cell query examines the curved parent over its complete bounded support or
one explicit unwrapped period. For every accepted half-cell, outward-rounded interval evaluation
proves that `cross(curve_tangent(t), fixed_line_direction)` is finite, excludes zero and retains
the selected sign. Bisection retains only proven-safe endpoints and conservatively approaches the
nearest tangent-parallel barrier without crossing it. This replaces the former seed-centred
fractional window with an auditable branch boundary; it does not promise motion through a
tangent-parallel configuration or authoring for two curved parents.

This restriction belongs only to the M66 authoring abstraction. M28's existing all-family
`CurveCurveFilletRequest`, persistent association, residual implementation and independent
validation remain public and unchanged for deliberate callers that already own complete branch
state.

After both parents are known, the headless state enters pointer-radius placement. The finite
fallback is only a seed. Pointer motion refreshes a non-committable scratch candidate, click
confirms the radius and Apply remains bound to that exact candidate. Reference is the default;
Driving is explicit. Invalid exploratory pointer positions retain parents and placement mode.

The accepted output arc stays ordinary geometry. Its stored center is presentation-neutral drag
metadata for the visible body. A free Fillet uses normal projected point drag to change center,
radius, contacts and trim intervals; a Driving radius may truthfully have no remaining mobility.
Rejected drag retains prior accepted geometry and explicit branch state.

For every attempt, the coordinator applies the proposal first to bounded scratch retained state.
It exposes a scene only after independent acceptance for the eligible current publication. Apply
repeats the same public proposal on live state and retains exact compare-and-swap equality.
Cancellation, exhaustion, stale input, unsupported/incomplete work or retained rejection cannot
publish partial state or leave a committable preview.

Canvas operation hover and click share an inclusive 12-pixel nearest-geometry hit. A preview-only
foreground hit blocks hidden source geometry but is neither hovered nor forwarded as a parent.
Fillet options are visually independent of palette overflow and clamped to the canvas viewport.

## 3. Exact commands and outcomes

Build-source commit `c1b0336` passed the complete post-pivot gate on 2026-08-02:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
cd crates/geosolve-demo-web
nix-shell ../../shell.nix --run 'env NO_COLOR=true trunk build --release'
git diff --check
```

All six commands completed successfully. Focused operation/editor/sketch/workbench regressions for
F002-F005 and F007-F011 pass inside the locked all-feature workspace suite, including the exact
symmetric-cubic hostile edit, three-sample semantic arc-center gesture beyond the former
quarter-span window, periodic branch certificate, overlay-toggle reflow and M25/M58 preservation.
The only non-failure output was the pre-existing Cargo advisory that workspace crates specify both
`license` and `license-file`.

Replacement build-source commit `ff15c78` passed the same six-command gate. Focused native tests
for `M66-F012` prove the exact workbench success/failure handoff, explicit Select restoration,
retained output selection and process-local option memory. One integrated default-Reference
authoring → publication → immediate arc-body center gesture → accepted resize → dimension deletion
→ second accepted resize lifecycle proves the reported UAT path. The WASM check and optimized
Trunk release build also pass; no tracked or untracked build output was introduced.

## 4. Acceptance status

M66 remains open only for focused supervising-human UAT. Build-source commit `c1b0336` first
qualified symmetric-cubic remote-root separation, an
unwrapped periodic circle cell, affine `Interior` persistence, hostile free-Fillet edit rejection,
multi-sample semantic pointer mobility, overlay reflow, persistence and exact publication.
Replacement `ff15c78` retains those results and closes the post-Apply routing trap. The archived
three-tool candidate is historical evidence only and does not satisfy the active gate.

## 5. Known limitations or next blocker

- Fillet root synthesis remains deliberately local and bounded; ambiguous or unavailable local
  roots warn instead of starting global enumeration.
- M66 authoring accepts affine/affine and affine/non-affine parent pairs. Two non-affine parents
  are typed unsupported until pairwise continuation is implemented; M28's generic all-family
  Fillet API remains available and unchanged below this authoring layer.
- Same-support Fillets remain limited to the explicit adjacent-open-polyline shortcut.
- The workbench remains a non-authoritative desktop UAT consumer.
- Offset/Mirror authoring is not part of active M66, although M25 Offset constraints and the M58
  Mirror operation API remain intact.
- The next and only remaining gate is focused human `docs/M66_UAT.md` approval against build-source
  commit `ff15c78` on the recorded Tailscale endpoint.
