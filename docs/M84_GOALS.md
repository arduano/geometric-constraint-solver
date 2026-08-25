<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 — Optional code/GUI sketch authoring

Status: **active; architecture and acceptance contract frozen, implementation complete and under
final clean qualification; no M84 candidate or human acceptance exists**. Accepted M83 remains
GitHub Pages authority. ADR 0041 is the controlling design.

## Goal

Let managed GUI geometry and externally authored reusable TypeScript patches coexist in one
editable sketch, with typed feature references, deterministic reverse edits and stable keyed
generation, while keeping the native Rust materializer/solver authoritative and the complete code
layer optional.

## Required behavior

### M84-G1 — optional module boundary

- Add `geosolve-sketch-code` and `@geosolve/sketch-code` as adjacent opt-in layers.
- Keep every core/sketch/linkage/intent/editor crate free of code-layer dependencies and concepts;
  plain editor/workspace-v8 deployments remain unchanged.
- Use only public M83 intent/editor APIs plus narrowly neutral keyed-reconciliation and delegated-
  checkpoint seams where required.

### M84-G2 — deterministic managed source

- Parse the exact `"use geosolve managed-v1";` bounded CST subset in pure Rust and preserve
  comments/formatting/unowned spans losslessly.
- Compile managed source to `AuthoringProgram`, bounded equation-free keyed expansion and an
  ordinary `IntentGraph`, then use the unchanged cold materializer, native solver and independent
  validation.
- Rewrite only authenticated source spans for direct fields/references, organization, declared
  edit lenses and explicit overrides. Unsupported source constructs or edits reject without
  changing canonical state.
- Retain valid-but-failed code intent above the previous accepted canvas; retain syntax-invalid
  text only as an editor draft.

### M84-G3 — reusable custom patches

- Keep `patches/*.patch.ts` user/AI-owned and byte-unchanged by GUI edits.
- Compile them only through an explicit caller-owned Node step to canonical data-only artifacts
  containing pinned digests/ABI, schemas, edit lenses and existing-family template DAGs.
- Never execute custom TypeScript during Rust/WASM/browser runtime, workspace load, expansion or
  solving. Validate artifacts as bounded untrusted data.
- Support structural `p.each`/`p.mapRecord` expansion, including Fillet-every-Polyline-corner,
  without adding a new equation or JavaScript constraint.

### M84-G4 — typed semantic references

- Expose project-branded `FeatureRef`, `OutputRef`, fixed named result objects, mapped
  `FeatureRecord` results and keyed/derived collections; raw wire IDs are not code-facing inputs.
- Generate high-level TypeScript output definitions from central Rust declaration descriptors and
  reject schema drift, raw IDs, cross-project references and port-kind mismatch.
- Give rectangles named corner/edge/profile outputs, preserve exact keys for named Fillet records,
  and carry Polyline corner keys through adaptive Fillet collections.

### M84-G5 — keyed identity and one history

- Reconcile generated members by invocation/template/member-key/output path, retaining all
  unaffected logical/native identities across insertion and reorder.
- Allocate new keys above high-water, tombstone removed keys and use a new generation when a
  retired key is reused outside Undo. Never silently cascade or retarget outside dependents.
- Own one `SketchCodeSession` history over project/artifacts/program/expansion/overrides and a
  delegated nested editor checkpoint. One user action is one Undo entry.
- Parse/expand nothing on pointer frames. Terminal drag publication uses the newest authenticated
  accepted native preview, stages the matching managed edit/override and commits exactly once.

### M84-G6 — project UX and persistence

- Add a **Code & reusable patches** sample group backed by genuine code-project sessions, never
  flat-scene imports or bootstraps that merely resemble their output.
- Add managed/custom file tabs, Apply/Revert, line-local diagnostics, artifact status, generated-
  member groups, ownership badges, edit-lens controls, override indicators and Reset to code.
- Treat custom files as read-only in the demo. Do not build a general browser IDE.
- Round-trip all files, artifacts, locks, expansion provenance, overrides, nested accepted intent
  and unified history in a bounded code-project envelope; repro restore is atomic and offline.
- Preserve ordinary M83 selection, Inspector, canvas constraints/dimensions and accepted-scene
  authority for generated outputs.

## Required demonstrations

1. **Rounded polyline · dynamic corners** — six keyed vertices/radius `0.4` yield five spans/four
   Fillets; inserting `crest` yields seven/six/five. Reorder/remove/Undo preserve unaffected
   identities; radius lens, point override/reset, open/closed mode and impossible-radius retained
   failure are visible.
2. **Typed panel · keyed Fillets** — an aligned rectangle exposes typed named outputs;
   `fillets({ lowerLeft, upperRight })` returns exactly those mapped fields. Compile-fail fixtures
   cover misspelling, raw IDs, cross-project and kind mismatch.
3. **Braced frame · GUI → code → GUI** — a GUI-created rectangle feeds `crossBrace(frame)`;
   `brace.diagonals.rising` feeds an ordinary GUI dimension/constraint. Both sides remain draggable,
   source-visible, correctly owned and in one Undo stream.
4. **Mounting plate · reusable AI-authored module** — a custom helper structurally generates a
   rounded profile and `nw/ne/se/sw` holes. Managed inputs remain GUI-editable, the helper stays
   byte-identical and save/repro restores the complete offline project.

## Acceptance summary

- Preserve the milestone-neutral 271-row authoring/scene golden byte-for-byte and add a separate
  reviewed code-project ledger.
- Qualify lossless parsing/exact-span rewrites, artifact compilation/locks, TypeScript type failures,
  keyed reconciliation/tombstones/high-water/Undo, one-transaction expansion, retained failures,
  bounds and save/repro.
- Prove cold/warm and native/WASM/RPC/TypeScript parity, finite geometry, explicit branches and
  normalized Hard residual `<= 1e-9` through existing validation.
- Prove pointer frames do no parsing/expansion and existing drag/terminal performance ceilings
  remain green.
- Pass format, warnings-denied Clippy/Rustdoc, locked all-feature tests, actual WASM, TypeScript,
  golden require-clean, Trunk and the complete clean release gate.
- Freeze one no-rebuild M84 candidate, verify it locally and through retained Tailscale UAT, and
  publish to Pages only after explicit supervising-user approval and exact hosted-byte proof.

## Bounds and non-goals

Managed source is at most 4 MiB, each artifact 16 MiB and the complete code project 64 MiB; existing
node/child/operation/response/depth limits remain. External patch source is trusted build input;
artifacts are untrusted bounded runtime data. No new primitive, constraint, residual, formula,
priority, Offset redesign, arbitrary TypeScript execution, browser `eval`, remote-package runtime,
3D/B-rep feature or general topological-naming claim is included. Generated definitions without an
edit lens are read-only; explicit free-placement overrides are supported and general ejection is
deferred.
