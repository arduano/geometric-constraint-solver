<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M66 implementation: computed 2D Fillet features

Status: active after the 2026-08-07 ADR 0031 pivot. The post-pivot implementation and mechanical
gate are not yet complete; supervising-human UAT remains open.

The qualified but unapproved solver-owned ordinary-UI endpoint, commit `1034afc`, is preserved at
`origin/archive/m66-associative-fillet-2026-08-07`. The earlier three-tool experiment remains at
`origin/archive/m66-three-helper-tools-2026-08-02` (`80d4939`). Neither archive is active
qualification evidence.

## 1. Files and public APIs planned

### Feature domain

- Add `crates/geosolve-sketch-features` as a pure safe-Rust crate depending among workspace crates
  only on `geosolve-sketch` and `geosolve-geometry`.
- Add a separately versioned `ComputedFeatureDocument` with stable document, feature and corner
  identities; allocation high-water; label/suppression state; and a closed
  `ComputedFeatureDefinition::FilletSet`.
- Persist only feature intent. A FilletSet stores one shared positive radius and explicit source
  spans, picked parameters, neighborhoods/winding, normal sides, retained endpoints, output
  endpoint order and sweep. It never stores generated arcs, trimmed fragments or output IDs.
- Add exact-stamped `ComputedFeatureSnapshot` output with evaluation-local `ComputedEdgeId`s,
  stable feature/corner/source-interval provenance, typed issues and variable output cardinality.
- Keep version-one feature inputs limited to native constrained sketch spans. Computed-on-computed
  references are not part of this cut.

### Editor and coordinator

- Replace the ordinary Fillet use of fixed two-pick `OperationAuthoringState` with reusable grouped
  feature authoring. Interior polyline points remain corner targets; repeated corner or curve-pair
  picks accumulate a batch.
- Preview the shared radius from remembered state or `0.1 * model_scale`. Numeric editing and a
  preview arc/radius grip edit that same value. Apply/Enter persists one FilletSet without a final
  canvas radius-confirmation click.
- Extend `RetainedEditorCoordinator` to own sketch session, feature document and current computed
  snapshot. Exact compare-and-swap includes complete sketch input/accepted identity, feature
  revision/digest and evaluator policy.
- Generated-arc selection resolves stable set/corner provenance. Arc/grip drag changes only feature
  radius; arc deletion removes its corner and final-corner deletion removes its set. Set suppression
  is separate from sketch source activation.

### Workbench and persistence

- Add a **Features** tree section, computed geometry rendering/hit metadata and feature/corner/
  source-attributed Problems/canvas markers.
- Keep native source points/spans selectable and draggable. Computed arcs are never offered as
  sketch constraint operands.
- Advance the application workspace envelope from v3 to v4. Store the separately versioned feature
  document next to the unchanged canonical-v4/draft-v5 sketch payload. Workspace v1-v3 migration
  creates an empty feature document bound to the restored sketch.
- Restore/Undo/Redo preserve feature IDs, intent and allocator state, then regenerate fresh output
  IDs.
- When active computed output would make base-only profiles/fills misleading, withhold them with a
  typed “computed geometry not yet included” status.

The normal UI removes Driving/Reference radius choice and does not auto-create a radius scalar,
dimension, constraint, M28 association or `DocumentCurveTrimView`. M27/M28 public Fillet types and
`SketchOperationRequest::AssociativeFillet` remain available for advanced/backward-compatible
callers. Existing documents are not migrated.

## 2. Mathematical behavior planned

All corners in one set evaluate from the same immutable independently accepted sketch snapshot.
Evaluation uses deterministic bounded construction and independently validates:

- finite source and output geometry;
- finite positive shared radius;
- contact domains, winding and explicit neighborhoods;
- normal sides and retained source endpoints;
- tangency and offset regularity/singularity;
- endpoint order and output sweep; and
- explicit local branch state.

M66 supports affine/affine and affine/non-affine corners. Two non-affine parents produce a typed
unsupported feature failure; this does not narrow M28's generic solver-owned API.

Evaluation composes endpoint claims without mutating sketch trim views. Opposite ends of a shared
span may belong to different sets. Duplicate claims, crossed claims or intervals that consume a
span fail every participating set. One invalid corner withholds its complete set; unrelated valid
sets remain publishable. A failed set exposes issues and no stale output.

A valid sketch edit remains acceptable even if it invalidates a feature. Source motion may recover
the same intent. Source deletion leaves a repairable missing-source failure, and Undo recovers the
same feature/corner identities. Changing only feature radius must leave canonical sketch identity,
accepted coordinates, residual evidence, numerical rank and reported DOF unchanged.

Generated IDs are meaningful only inside one exact computed snapshot. Stable provenance, not an
output ID, is the cross-revision identity seam. The output container accepts zero/one/many fragments
so a later Offset evaluator can represent self-intersection cuts and other topology changes without
redesigning persistence. M66 implements no Offset.

## 3. Exact commands and outcomes

No post-pivot mechanical qualification command has been claimed yet. Before nominating a UAT
candidate, run all of the following on one source state:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
cd crates/geosolve-demo-web
nix-shell ../../shell.nix --run 'env NO_COLOR=true trunk build --release'
git diff --check
```

Record exact command outcomes and nominated commit here only after the final post-pivot source
passes. The old `1034afc` qualification belongs solely to the archived architecture.

## 4. Acceptance status

Open. Direct qualification must cover:

- four-point/three-span two-corner batch output and middle-span two-end composition;
- reverse-selection canonicalization and sequential/batch visible parity;
- atomic claim conflict plus recovery;
- exact sketch-state/residual/rank/DOF invariance under shared-radius edits;
- every source-point drag, invalid-feature withholding and source deletion/Undo recovery;
- deleting/suppressing either adjacent set while retaining the other;
- Undo/Redo/reload, stale CAS, cancellation, exhaustion and allocator non-reuse;
- evaluation-local output-ID invalidation and variable output count;
- ordinary-UI absence of M28 associations, trim views, constraints and radius dimensions; and
- M27/M28/M30/M58 backward compatibility.

After those tests and the full native/WASM/release gate pass on one nominated source, restart the
Tailscale UAT service and execute `docs/M66_UAT.md`. M66 stays open until the supervising human
explicitly approves it.

## 5. Known limitations or next blocker

- Production and visual-profile consumers do not include computed output in M66. Misleading
  base-only presentation must be withheld.
- Two non-affine-parent ordinary authoring remains typed unsupported; M28 advanced APIs remain.
- Version-one computed features reference native constrained spans only.
- Computed-on-computed chaining, Offset, Bake/Explode and cross-revision output topological naming
  are deferred.
- No post-pivot implementation source or qualified UAT candidate has yet been recorded in this
  document.
