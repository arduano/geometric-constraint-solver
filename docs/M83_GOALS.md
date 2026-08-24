<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 — Projectional sketch design intent

Status: **implementation, M83-F001 through M83-F009 repair, post-F007 architecture hardening,
clean qualification and immutable F008/F009 Tailscale replacement nomination complete; focused
human UAT pending**.
ADR 0040 is the active architecture. The rejected chronological candidate is preserved only on
`archive/m83-chronological-lineage-2026-08-23` at `be62a1c`; it is not a compatibility target.

## Goal

Represent an editable sketch as stable semantic declarations and writable instance values that
can be projected consistently into the canvas, Inspector, structured source, AI patches and
future TypeScript host code, while keeping the existing native Rust solver and independently
validated flat accepted scene authoritative.

## Required product behavior

### M83-G1 — closed order-independent intent model

- Cover all 25 existing geometry recipes and the complete current relation, dimension, operation,
  computed-Fillet, host-parameter and external-reference catalogs without adding a new equation.
- Generate stable ports, children, input-slot bindings, writable leaves and typed native
  reservations from closed Rust schemas. Existing-point operands alias existing point outputs.
- Separate graph, instance, organization and external-input identities. Schedule only by the
  dependency DAG and stable IDs; names and display order are non-semantic.
- Retain never-reused reservations and typed tombstones across suppression, failure, deletion,
  Undo/Redo and reload.

### M83-G2 — deterministic accepted materialization

- Lower through existing public sketch/feature/operation APIs and exact host inputs.
- Retain a stable logical/native ownership map and reverse free-leaf bindings.
- Require existing finite/domain/branch/residual validation before accepted publication.
- Keep the previous accepted materialization visible beneath newer retained invalid intent.
- Reconstruct that exact accepted materialization when retained-invalid migrated/bootstrap v8
  intent is reloaded, while preserving current failed intent, history and Undo.
- Make cold canonical reconstruction the authority and prove warm/cold semantic parity.
- When a computed Fillet is suppressed, retain its declaration and exact identity but publish no
  computed edge/affordance and do not hide its finite native parents. Restore, Undo and Redo must
  preserve the same feature/corner identity.

### M83-G3 — one mutation and history protocol

- Route canvas, Inspector, structured source, AI/RPC and operation deletion through typed unordered
  exact-CAS patches.
- Authenticate a Structured Source token against its exact originating session/revision/digest
  before resolving its numeric ID; stale reordered tokens reject without mutation.
- Publish one composite Undo/Redo entry per accepted user action. History is read-only; Outline and
  cell reorder are organization-only and use explicit before/after insertion slots.
- Delete an Offset/operation by its declaration and exact dependent closure instead of requiring
  manual deletion of every generated object.
- Do not duplicate coordinator and intent histories or use JSON-leaf reverse diffs.

### M83-G4 — predictable fast direct manipulation

- Prepare one reverse binding route at pointer-down, use the retained native solver for coalesced
  previews and commit only the newest authenticated accepted pointer-up sample. A later rejected
  attempt cannot obscure the visible accepted preview, and each capture has one terminal owner.
- Ordinary drags update genuinely free instance leaves only. They never silently rewrite a driving
  dimension, fixed target, branch or arbitrary former action.
- Keep explicit Fillet-radius and Profile-Offset-distance gestures as property edits.
- Pointer frames perform no workspace save, graph serialization/replay or durable panel rebuild.
- Compare terminal preview with cold materialization exactly except for schema-owned recomputable
  line-branch metadata on Polyline and the four rectangle recipes. That metadata may be
  canonicalized only while it stays in the same positive branch cell; Segment and Midpoint Line
  explicit branches, every unrelated field and the independently validated accepted geometry
  remain exact.

### M83-G5 — projectional workbench

- Add `Outline | Structured source | History` tabs beside the existing sketch tree.
- Make Outline declarations selectable, Inspector-editable and organization-reorderable without
  changing materialization.
- Generate deterministic TypeScript-shaped source; recognized token edits create typed patches,
  while whitespace/order edits cannot affect geometry. Do not execute arbitrary TypeScript.
- Project exact stable input-slot-to-port bindings in both Structured Source and Inspector; rebind
  updates both and Inspector presents these references read-only.
- Show retained invalid declarations and their diagnostics while the accepted canvas remains
  usable.
- Keep M76 annotation layout as a disposable presentation cache beside the intent session. It may
  survive an ordinary workspace save, but it is neither declaration/instance state nor part of
  materialization, solver input or composite Undo/Redo history, and it is recomputed safely when
  absent or incompatible.
- Keep New available on the projectional surface. It creates the same canonical empty workspace-v8
  authority as fresh startup, clears authored geometry/declarations/history and transient
  authoring state, returns to Select, resets the camera and saves the new workspace.
- Distinguish legitimate empty accepted authority from scene-composition failure. The latter is a
  visible frame-local canvas status, never a durable stale error, and clears on the next valid
  composition.

### M83-G6 — host surface and persistence

- Add DOM-free WASM/RPC parity and a small branded TypeScript package over the closed patch
  vocabulary. Neither layer owns geometry equations.
- Advance the application envelope to workspace v8. Reject the abandoned v7 format. Restore v1-v6
  strictly, then normalize their flat content honestly into typed per-object native bootstrap
  declarations with exact existing identity bindings until explicitly ejected into supported
  higher-level declarations.
- Round-trip graph/instance/organization/external identities, accepted evidence, reservations,
  tombstones and bounded history canonically.
- Enable Copy/Load repro for projectional workspaces. Copy transports the complete bounded v8
  authority and unified history with manual-selection fallback; Load validates the complete
  snapshot before atomic replacement. Annotation layout remains omitted and recomputable.

### Post-F007 architecture hardening

- Use SHA-256 for canonical graph/session wire-v2 content identity. Accept experimental wire v1
  only after its canonical FNV fingerprint plus every nested identity, checkpoint, reservation and
  materialization authority validate; then emit v2 only.
- Make ordinary session/semantic identity reads cached and constant-time with respect to retained
  history, while import, planning and publication independently recompute and compare identities.
- Bind every Undo/current/Redo checkpoint body to its causal edges, globally chronological unique
  descriptor revision and body-derived semantic descriptor. Validate current and historical
  accepted graphs, instances, host inputs, evidence and reservation-ledger provenance uniformly.
- Derive declaration schema/default/choice/output/edit metadata from one central Rust descriptor;
  expose bounded compact graph/source snapshots without duplicating opaque bootstrap payloads.
- Keep explicit Snapshot as the complete read. Return closed typed receipts for mutations and
  Undo/Redo, bound request/receipt/response bytes before publication, and validate identical
  contracts in Rust, WASM and TypeScript.
- Publish Fillet/Offset Apply and accepted property drops through the exact prepared transaction
  already used for the visible preview. Reject stale or oversized output before changing intent or
  native accepted authority.
- Admit workspace v8 only inside the public 64 MiB reproduction envelope and avoid unbounded
  version/cache trees, duplicate canonical strings, eager legacy hashing and repeated nested intent
  parsing. Bound the disposable annotation-layout string independently at 4 MiB; cache corruption
  or eviction remains non-semantic.

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

The initial `232b83a` nomination, post-F005 source `a621cdd`, post-F007 source `fafea4e` and post-
hardening source `1e70f3f` remain historical evidence after M83-F001 through M83-F009. Current
source `b0de5af`, tree `ff0b29d`, passes the fresh clean gate; its exact no-rebuild snapshot
`/tmp/geosolve-m83-f008-f009-uat.zLfB22EK`, aggregate
`f2092e54b1b014618dcdded21e3bc0907a280fc15aa93b0c18913cf87d9b30d6`, passes 7/7 local browser
checks and 4/4 on both temporary and retained Tailscale listeners. Both temporary/final eight-path
byte ledgers have SHA-256 `b5bef9cc6274258c217f5edf44c7a6ed06b7299c524f5f0ed3ea4aa64d8866b4`;
the candidate is live at `http://100.94.63.83:8080/`, PID `3376452`. M83-U1 through M83-U9 and
F001-F009 human rechecks remain pending evidence; accepted M81 GitHub Pages bytes remain public
authority.

## Non-goals

No new constraint, residual, formula/expression graph, priority, JavaScript solver, arbitrary
TypeScript execution, B-rep topological naming claim, curve family, mobile UI or Pages publication
before UAT is included. M83 does not restore the chronological ledger or infer fictional recipe
history for arbitrary old flat documents.
