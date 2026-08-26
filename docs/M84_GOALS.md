<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 — Optional code/GUI sketch authoring

Status: **active and unaccepted; M84-F004 is reproduced and its repair is under focused
qualification**. The clean-qualified M84-F003 snapshot is withdrawn from current UAT by F004 and
is historical evidence only; a replacement Tailscale candidate has not yet been nominated.
M84-U1 through M84-U12 and explicit approval remain pending. Accepted M83 remains GitHub Pages
authority. ADR 0041 is the controlling design.

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
- Keep the data-only M83 source projection as honestly labelled Intent IR. For every supported GUI
  conversion, emit dependency-ordered lexical declarations and member expressions; never expose a
  transport reference object or repeated string identity as authored code.
- Promote the complete supported ordinary sketch atomically into one genuine persisted code
  project. Omit only the exact canonical fresh-workspace document foundation; never discard other
  bootstrap objects. Limit recomputable line-branch normalization to current code-expansion-owned
  Segments so ordinary GUI Segments retain explicit branch authority.
- Project a supported ordinary computed Fillet as direct `$.computed.filletSet` managed source.
  Each corner's exactly two ordered parents must be lexical `NativeCurveSpanRef` values, and every
  persisted parameter, winding, neighborhood, normal-side, retained-endpoint, periodic-anchor,
  endpoint-order, sweep and suppression choice must remain explicit. Generate that native-span
  brand from the central declaration-result descriptors only for `geometry.line.span`, rectangle
  edges and Polyline segments. A computed host Fillet arc is not an Intent-backed native span and
  must reject as a direct FilletSet parent at both the typed boundary and Rust lowering boundary.
  Accept radius only as a positive finite model-unit number or branded `mm(...)`. Lower directly
  to the existing Intent `ComputedFeature::FilletSet`; do not rerun Fillet picking/authoring
  heuristics or introduce a solver path. The result is an opaque `FilletSetFeature`, not a false
  promise that computed child arcs are ordinary native curve-span outputs.

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
- Require dependencies to parse from lexical declaration/member expressions as
  `ManagedValue::Reference`; branded raw strings are insufficient. Reject raw strings, transport
  DTOs, foreign-project and forged reserved-project values, wrong kinds and misspelled members.
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
- Keep the Code surface discoverable for every ordinary sketch. Complete supported scenes show a
  read-only lexical preview and Promote; an unsupported all-or-nothing projection instead shows an
  escaped read-only conversion diagnostic, retains Intent IR as the audit fallback and offers no
  Promote action.
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
- Compile the checked-in managed-v1 two-line/one-Fillet fixture through TypeScript, parse that same
  source in Rust and cold-materialize it through the unchanged accepted-scene path.
- Prove pointer frames do no parsing/expansion and existing drag/terminal performance ceilings
  remain green.
- Pass format, warnings-denied Clippy/Rustdoc, locked all-feature tests, actual WASM, TypeScript,
  golden require-clean, Trunk and the complete clean release gate.
- Freeze one no-rebuild M84 candidate, verify it locally and through retained Tailscale UAT, and
  publish to Pages only after explicit supervising-user approval and exact hosted-byte proof.

Withdrawn nomination record (2026-08-25): exact product source
`79078eca44a5af4de5cccd92bf6fee570c473624`, tree
`05aefb0cbd3972d423f1713df1e58628b24ec216`, passes the complete clean release gate. Its exact
no-rebuild seven-file output is frozen read-only at `/tmp/geosolve-m84-uat.aHw5ePSW`, ordered-
manifest aggregate `99beaf51ebb314aa26689427f970a75a516891efd20f68587c2a33c1b3a64f34`, and is
byte/browser-verified locally and at `http://100.94.63.83:8080/`. Human UAT then opened M84-F003,
so those bytes are historical defect evidence rather than a current candidate.

Withdrawn F003 replacement record (2026-08-26): exact product source
`b9e67bad7f4935b1e0591ea4f149fae478b32675`, tree
`7062806695e1e134c339cfa47903145d321f6350`, passes the complete clean release gate. Its exact
no-rebuild seven-file output `/tmp/geosolve-m84-f003-uat.mO67NI` is frozen at directory/file modes
`0555`/`0444`, ordered-manifest aggregate
`38d356e9f727a4b690c1166dee3a36b1e0a5e59a2ad8bad1ca7243d889c6a617`, and byte/browser-verified
on temporary and retained endpoints. Existing browser checks pass 4/4 and the F003 flow passes 1/1
on both. Retained service PID `3736900` served those exact bytes at
`http://100.94.63.83:8080/`; M84-F004 now withdraws them from current UAT even if the endpoint
remains reachable. No F004 replacement snapshot, clean gate or immutable nomination is claimed
yet. M84-U1 through M84-U12, explicit approval, GitHub Pages publication, service retirement and
closure remain open; accepted M83 remains public authority.

## Bounds and non-goals

Managed source is at most 4 MiB, each artifact 16 MiB and the complete code project 64 MiB; existing
node/child/operation/response/depth limits remain. External patch source is trusted build input;
artifacts are untrusted bounded runtime data. No new primitive, constraint, residual, formula,
priority, Offset redesign, arbitrary TypeScript execution, browser `eval`, remote-package runtime,
3D/B-rep feature or general topological-naming claim is included. Generated definitions without an
edit lens are read-only; explicit free-placement overrides are supported and general ejection is
deferred.
