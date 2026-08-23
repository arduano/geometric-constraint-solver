<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 — Projectional sketch design intent

Status: **implementation in progress**. ADR 0040 is the active architecture. The rejected
chronological candidate is preserved only on `archive/m83-chronological-lineage-2026-08-23` at
`be62a1c`; it is not a compatibility target.

## Goal

Represent an editable sketch as stable semantic declarations and writable instance values that
can be projected consistently into the canvas, Inspector, structured source, AI patches and
future TypeScript host code, while keeping the existing native Rust solver and independently
validated flat accepted scene authoritative.

## Required product behavior

### M83-G1 — closed order-independent intent model

- Cover all 25 existing geometry recipes and the complete current relation, dimension, operation,
  computed-Fillet, host-parameter and external-reference catalogs without adding a new equation.
- Generate stable ports, children, writable leaves and typed native reservations from closed Rust
  schemas. Existing-point operands alias existing point outputs.
- Separate graph, instance, organization and external-input identities. Schedule only by the
  dependency DAG and stable IDs; names and display order are non-semantic.
- Retain never-reused reservations and typed tombstones across suppression, failure, deletion,
  Undo/Redo and reload.

### M83-G2 — deterministic accepted materialization

- Lower through existing public sketch/feature/operation APIs and exact host inputs.
- Retain a stable logical/native ownership map and reverse free-leaf bindings.
- Require existing finite/domain/branch/residual validation before accepted publication.
- Keep the previous accepted materialization visible beneath newer retained invalid intent.
- Make cold canonical reconstruction the authority and prove warm/cold semantic parity.

### M83-G3 — one mutation and history protocol

- Route canvas, Inspector, structured source, AI/RPC and operation deletion through typed unordered
  exact-CAS patches.
- Publish one composite Undo/Redo entry per accepted user action. History is read-only; Outline and
  cell reorder are organization-only.
- Delete an Offset/operation by its declaration and exact dependent closure instead of requiring
  manual deletion of every generated object.
- Do not duplicate coordinator and intent histories or use JSON-leaf reverse diffs.

### M83-G4 — predictable fast direct manipulation

- Prepare one reverse binding route at pointer-down, use the retained native solver for coalesced
  previews and commit only the newest exact pointer-up sample.
- Ordinary drags update genuinely free instance leaves only. They never silently rewrite a driving
  dimension, fixed target, branch or arbitrary former action.
- Keep explicit Fillet-radius and Profile-Offset-distance gestures as property edits.
- Pointer frames perform no workspace save, graph serialization/replay or durable panel rebuild.

### M83-G5 — projectional workbench

- Add `Outline | Structured source | History` tabs beside the existing sketch tree.
- Make Outline declarations selectable, Inspector-editable and organization-reorderable without
  changing materialization.
- Generate deterministic TypeScript-shaped source; recognized token edits create typed patches,
  while whitespace/order edits cannot affect geometry. Do not execute arbitrary TypeScript.
- Show retained invalid declarations and their diagnostics while the accepted canvas remains
  usable.

### M83-G6 — host surface and persistence

- Add DOM-free WASM/RPC parity and a small branded TypeScript package over the closed patch
  vocabulary. Neither layer owns geometry equations.
- Advance the application envelope to workspace v8. Reject the abandoned v7 format. Restore v1-v6
  strictly, then represent their flat content honestly as an opaque native bootstrap declaration
  until explicitly ejected into supported declarations.
- Round-trip graph/instance/organization/external identities, accepted evidence, reservations,
  tombstones and bounded history canonically.

## Acceptance summary

- Inventory-driven native tests cover every schema, dependency permutation, alias, reservation,
  tombstone, retained failure, deletion closure and organization reorder.
- Differential tests compare graph materialization with the existing accepted flat authoring
  corpus, including native Fillet, Profile Offset and host inputs.
- Native and WASM/RPC transcripts are byte-stable and the TypeScript package passes runtime and
  type-level tests.
- Drag performance is measured separately for pointer frames and exact terminal publication; the
  workbench remains visually responsive on the representative sample corpus.
- Formatting, warnings-denied Clippy/Rustdoc, locked all-feature workspace tests, relevant WASM
  builds, the unchanged milestone-neutral golden and complete clean release gate pass.
- The exact no-rebuild candidate is frozen and byte-verified over Tailscale. GitHub Pages remains
  on the accepted M81 product until explicit M83 human approval.

## Non-goals

No new constraint, residual, formula/expression graph, priority, JavaScript solver, arbitrary
TypeScript execution, B-rep topological naming claim, curve family, mobile UI or Pages publication
before UAT is included. M83 does not restore the chronological ledger or infer fictional recipe
history for arbitrary old flat documents.
