<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M84 — Optional code/GUI sketch authoring

Status: **active and unaccepted; exact clean-qualified immutable M84-F012 is the current Tailscale
UAT candidate; refreshed U1-U16 remain pending**. Exact F011 source
`e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
`015209773f81ec1a254817c65ef2a71b984e3b08`, and no-rebuild snapshot
`/tmp/geosolve-m84-f011-uat.ps736NLh` are historical rollback evidence. Exact F012 source
`84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
`429ed56d2a5b3988d6604079d19e1002f9049d64`, and no-rebuild snapshot
`/tmp/geosolve-m84-f012-uat.nMOymIIM` are current mechanical nomination authority; no UAT acceptance
is claimed. F010 source `cf463838`,
tree `992e587`, and snapshot `/tmp/geosolve-m84-f010-uat.7R5eXQoz` are historical rollback
evidence. F009 source `c74651c`, tree `a904584`, and snapshot
`/tmp/geosolve-m84-f009-uat.q8cKIN3v` are withdrawn historical defect evidence; F007 source
`cc2f05e`, direct-authoring snapshot `41e65a4`, combined F005/F006 source `ff2e142` and all earlier
M84 nominations are likewise historical. Human UAT U1-U16 and explicit approval remain pending.
No UAT row is accepted, M84 is not published or closed, and accepted M83 remains GitHub Pages
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
- Expose the smallest direct host entry point as optional
  `CodeProject::managed_only(ProjectKey, source)`: it admits complete artifact-free managed source
  but publishes no geometry until ordinary expansion/materialization/native validation succeeds.

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
- Preserve the ordinary drafting path's existing inferred Horizontal and Vertical relations.
  Project each as `$.constraint.horizontal` or `$.constraint.vertical` against the owning lexical
  native span, including suppression, and lower it back to the existing Intent constraint kind.
  Never omit those relations merely to make a Fillet scene promotable, and add no new relation or
  residual equation.
- Authenticate accepted geometry against the exact current retained semantic identity before GUI
  projection. Retained-failed intent must keep Code visibly unavailable with no Promote action;
  never serialize a hybrid of prior accepted geometry and current unaccepted wiring.

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
- Publish direct Polyline `vertices` and `segments` as typed keyed root collections as well as the
  existing exact member paths and `filletableCorners`. Each vertex key maps to its native Point
  port; each directed span's starting key maps to its native CurveSpan port. Recorded artifact
  `each`/mapping rules may consume those roots without coordinate copies, raw IDs or ordinal
  identity.

### M84-G5 — keyed identity and one history

- Reconcile generated members by invocation/template/member-key/output path, retaining all
  unaffected logical/native identities across insertion and reorder.
- Allocate new keys above high-water, tombstone removed keys and use a new generation when a
  retired key is reused outside Undo. Never silently cascade or retarget outside dependents.
- Own one `SketchCodeSession` history over project/artifacts/program/expansion/semantic interaction
  overlay and a delegated nested editor checkpoint. One user action is one Undo entry.
- Parse/expand nothing on pointer frames. Terminal drag publication uses the newest authenticated
  accepted native preview, stages the matching overlay and commits exactly once.
- Keep durable GUI placement in a bounded, generation-authenticated semantic overlay keyed by
  project, semantic owner, output path and writable field. It is an equation-free instance-seed
  layer, never a new constraint, residual or priority mechanism. M84 makes only Cartesian points
  overlay-writable; scalar edits continue through authenticated managed-source lenses. Point-seed
  precedence is typed overlay draft > legacy generated override > managed source seed; Reset
  removes the complete semantic edit bundle and exposes the applicable lower tier again.
- Resolve a canvas drag and deletion through accepted expansion provenance, never by decoding a
  hashed `code.*` intent alias. With no selected semantic owner, or with the producer selected,
  ordinary producer ownership keeps referenced consumers attached. A uniquely selected referenced
  consumer detaches only that consumer; its projected Segment may acquire replacement intent/native
  identity while its code owner stays stable and all retained code/GUI dependents rebind. Repeated
  drags use the detached point lens, and Undo/Redo restores the complete attachment and overlay.
  A rectangle corner updates its two canonical seeds atomically. Unknown, stale-generation,
  wrong-type or non-finite drafts reject before publication. Equal duplicate writes in one
  same-tier terminal bundle collapse; unequal writes to one address reject atomically. Multiple
  matching lenses for the selected declaration reject without mutation.
- Delete a managed declaration through its semantic source owner and exact dependent closure. Do
  not delete only the projected intent node, silently cascade ordinary outside dependents, or allow
  a dirty managed draft or retained code failure to choose deletion authority. Authenticate the
  delete token against the exact code-session identity, accepted alias and semantic owner at
  execution time. A generated-child Delete is reversible suppression, not source deletion;
  GUI-owned selections remain on ordinary editor deletion.

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
- Persist current and accepted semantic overlays separately. A structural attempt that removes an
  owner and later fails native publication retains the deterministic owner-pruned attempted
  overlay while the exact accepted overlay/canvas remain authoritative; reload and Undo preserve
  both sides.
- Because the semantic overlay changes the still-unreleased optional persistence contract, use
  explicit incompatible identifiers `geosolve-sketch-code-session-v2` and
  `geosolve-code-workbench-v2`. Reject prototype-v1 payloads instead of silently changing their
  identity digest. Plain M83 workspace-v8 persistence remains unchanged.
- Preserve ordinary M83 selection, Inspector, canvas constraints/dimensions and accepted-scene
  authority for generated outputs.
- On only an exact canonical fresh workspace whose current and accepted semantic identities match
  an independently validated empty native scene, show one **Start from code** action and all nine
  centrally owned genuine sample cards. Nonempty or retained-failed scenes keep their existing
  Preview/Unavailable behavior.
- Starting from code creates a distinct persisted `Authored` origin, an artifact-free editable
  `sketch.ts` and no fabricated ordinary scene or Promote history. Valid Apply, whole-source
  replacement, retained-invalid source/diagnostic, Undo/Redo, reload and repro remain one atomic
  code/editor authority.

### M84-F006 — adversarial authority hardening

- Cap imported session identities before adopting allocator high-water and bound persisted managed
  drafts to the existing 4 MiB source ceiling.
- Use typed canonical Reset/Restore tokens and authenticate direct plus generated owner pruning
  exactly across retained failure.
- Preserve generated-reference provenance through local detachment, direct/generated Segment and
  generated circle-centre replacement, dependent rebinding, cancellation, repeated drag and exact
  Undo/Redo.
- Compare same-tier point seeds by persisted IEEE bits: bit-identical duplicates collapse, while
  distinct encodings such as `+0.0` and `-0.0` reject atomically in either order.
- Treat these as persistence/resource/transaction hardening only. Do not add an equation,
  constraint, solver priority, tolerance or branch rule.

### M84-F007 — one authenticated terminal point lens

- Authenticate exactly one semantic point lens at pointer-down and retain that route across every
  native preview frame. Do not classify a terminal checkpoint by copying every solver-moved
  code-owned point as independently authorized semantic intent.
- With no semantic selection, choose the unique producer lens. Explicit producer selection keeps
  referenced consumers attached. Explicit consumer selection still detaches only that consumer.
  Ambiguous producer/selected lenses reject before preview authority changes.
- Let only the authenticated point lens authorize terminal publication. M84-F010 below supersedes
  the original single-seed durability interpretation while preserving this authorization rule.
- Store the pending route as that exact `CodeSessionIdentity`, pointer and point lens. Only the
  dedicated authenticated terminal publisher for that pointer may consume it; generic/delegated
  saves reject without consumption, and foreign/reentrant preparation or a foreign terminal
  rejects while preserving the original route.
- Every non-pointer durable code action cancels or invalidates the route. No-motion release and
  cancellation are history-neutral. Apply and Undo invalidate a stale terminal without reverting
  the newer accepted session, checkpoint or overlay authority.
- Every durable Outline, Inspector, source, code, history, Apply and equivalent sidebar route must
  cancel pointer capture before deriving or mutating authority. If an unexpected generic save still
  reaches a pending route, it must preserve the live native editor, token, capture, notice, history
  and persistence rather than restoring accepted authority beneath that token.
- Keep ordinary GUI-owned points on the delegated M83 editor path. Add no equation, constraint,
  solver priority, tolerance or branch rule.

### M84-F010 — complete authenticated terminal movement closure

- Reproduce a Compass Rose shared-center drag whose newest native preview is accepted exactly but
  whose durable scene later rematerializes to another valid point. Classify this at retained code-
  workbench terminal publication, not as a timer, worker or core-solver race.
- Retain the one F007 pointer-down lens as gesture authorization. Compare the exact authenticated
  origin to the accepted terminal checkpoint and atomically persist every solver-coupled semantic
  Cartesian point seed in that movement closure. Reject any changed leaf outside existing semantic
  provenance/locality. For a referenced-consumer drag, retain the exact post-detachment origin.
- Always require full parity for design and current accepted documents, computed features,
  logical/native ownership and allocators. Read solved authority through
  `accepted_state_for_current_input()` rather than treating design intent as solved state.
- Keep ordinary point aliases bit-exact. For the four rectangle corner lenses over two real seeds,
  make the authenticated corner and diagonal opposite exact anchors; permit only the two redundant
  adjacent aliases to normalize under tightly bounded finite numerical roundoff. Signed-zero,
  non-finite and material conflicts reject atomically.
- Prove exact Compass release through persistence/reload, all spoke starts attached, one history
  entry, finite geometry, Current features and independently validated Hard residual `<= 1e-9`.
  Add no equation, constraint, solver priority, tolerance or branch rule.

### M84-F008 — fitted code scenes and visible sample Fillets

- Fit a newly installed code project to its accepted composed scene instead of resetting the
  camera to the canonical Origin view. Fall back to Origin only when accepted scene authority is
  empty or unavailable.
- Use `mm(4)` for the Rounded Polyline sample so all four valid computed Fillets extend visibly
  beyond the point markers. This is sample/presentation correction only.
- Require native adapter regressions for an off-origin scene inside the fitted viewport and for
  exactly four finite, visibly separated radius-4 Fillet paths. Add no equation, constraint,
  solver priority, tolerance or branch rule.

### M84-F009 — exact multi-output shorthand routing

- Record the exact compiler-selected `result_output` in the data-only template independently of
  renamed/nested public paths. Publish only that authenticated selection, preserve nested root
  collections and never let canonical map order or a generic prefix fallback select an output.
- Require Mounting Plate `plate.profile` to resolve to Profile rather than the alphabetically first
  `ne` Point, while every full output path remains available.
- Cover renamed/nested and mapped multi-output selection; a collection member without recorded
  result provenance must fail closed.
- Exhaustively check every semantic output across all eight demos: its declared reference kind and
  expanded target kind must both equal the reviewed catalog kind.
- Treat this as optional-layer output routing only. Add or change no native geometry, solver
  equation, constraint, priority, tolerance or branch rule.

### M84-F011 — PC water-manifold dogfood amendment

- Add a ninth genuine code-first sample representing a 240 × 120 mm CNC acrylic PC water-cooling
  distribution manifold. Reserve a 60 × 84 mm reservoir bay, route three open channel Polylines,
  surround them with three closed O-ring groove loops and place eight screw circles with solved
  radius 2.5 mm plus driving diameter 5 mm.
- Fully constrain the sketch relationally from exactly one `FixedPoint` and no `FixedCoordinate`.
  Use 21 construction datum spans to locate the reservoir, route starts, seals and screw rails;
  independently require normalized Hard residual `<= 1e-9` and zero numerical, equality and
  bidirectional degrees of freedom.
- Exercise the intended managed/custom hybrid: managed declarations own mechanical dimensions and
  references, while the caller-compiled AI-authored `waterChannel` patch consumes keyed Polyline
  corners through `p.each` and adapts one existing native Fillet per current corner. The three
  routes must produce six Current Fillets; the inexpensive closed O-ring loops remain direct
  managed geometry.
- Extend only the optional managed vocabulary needed for this mechanical slice: circles,
  Coincident, FixedPoint/FixedCoordinate, curve-length and diameter dimensions, keyed Polyline
  member references and profile/construction line roles. The sample deliberately uses no
  FixedCoordinate. Lower all of them to existing Intent/native contracts; add no solver equation,
  residual, priority, tolerance or branch rule.
- Add a presentation-only **Export PNG** action over the authoritative composed SVG. Rasterize in
  the browser to a fixed, self-contained 2000 × 1400 PNG, omit hit/provisional geometry and never
  mutate the document, code session, history or accepted-scene authority.
- Qualify the ninth project, real PNG download and PNG signature/dimensions as M84-U15 and U16.
  Freeze and verify a clean committed F011 candidate on a temporary endpoint before replacing the
  retained F010 Tailscale service. Pages publication, human acceptance and milestone closure remain
  out of bounds until explicit approval.

### M84-F012 — annotation visibility shares paint and picking

- Add one **Annotations** checkbox to the existing display options in both the flat and code
  workbenches. It is on for every new browser session and remains session-local presentation state:
  do not place it in the sketch/code document, accepted scene identity, history, persistence,
  Intent IR or repro payload.
- Carry the value on the shared headless scene DTO used by rendering and pointer resolution. When
  false, omit every constraint/dimension annotation and DOM hit corridor and make direct plus
  contextual/corridor hit tests return no annotation, even for selected or problem-forced items.
  Clear any stale annotation hover so the exact underlying geometry or datum owns the next move/
  down. Do not discard or rewrite derived annotation layout or its disposable presentation cache.
- Keep Fillet radius/continuation affordances available and pickable while annotations are hidden;
  they are independent computed-feature interaction authority, not dimension annotations.
- Export the already-composed canvas WYSIWYG: annotations appear in the PNG exactly when currently
  visible. Export-only styling must remove hit targets, errors, drafts, inference guides/candidates
  and all other provisional authoring paint without mutating selection, layout, history, accepted
  authority, code state, IR, persistence or repro.
- Qualify both workbench variants, overlapping annotation/geometry/datum picks, selected/problem
  annotations, Fillet handles, visible/hidden PNGs and provisional-export cleanup. F011 remained
  rollback-only until the clean, frozen, byte/browser-verified F012 replacement existed; U1-U16,
  explicit approval, Pages publication and closure remain pending after that mechanical gate.

## Required demonstrations

The eight bundled demonstrations below are the historical reviewed F009/F010 code-project catalog.
F011 adds the ninth dogfood demonstration below them. A separate, non-bundled entry path starts an
**Untitled code sketch** directly from an editable rectangle plus diagonal whose endpoints are
lexical `frame.corners.*` references.

1. **Rounded polyline · dynamic corners** — six keyed vertices/radius `4` yield five spans/four
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
5. **Lantern garland · adaptive decorations** — seven keyed Polyline vertices feed a bulb at every
   vertex and a Fillet at every interior corner through separate keyed collections and bulb/bend
   scalar lenses.
6. **Suspension bridge · typed structural graph** — three constrained deck spans and two towers
   feed a reusable typed module producing three cable spans and two stays; producer-point drags
   retain attached consumers.
7. **Compass rose · generated semantic lattice** — four editable shared-centre native spokes and
   ordinary Horizontal/Vertical relations feed a generated diamond ring and four marker circles
   with a marker-radius lens.
8. **Neon manifold · explicit native bends** — artifact-free managed TypeScript combines four
   connected axis-constrained native lines with one branch-explicit two-corner direct FilletSet;
   native endpoint dragging recomputes the feature.
9. **PC water manifold · constrained mechanical dogfood** — a managed 240 × 120 mm plate combines
   a 60 × 84 mm reservoir, three open water channels, three closed O-ring loops, eight diameter-5
   screw circles and 21 construction datums. Exactly one fixed point anchors its otherwise
   relational dimensions, and the custom `waterChannel` patch generates six keyed adaptive native
   Fillets across the current channel corners.

## Acceptance summary

- Preserve the milestone-neutral 271-row authoring/scene golden byte-for-byte and extend the
  separate reviewed code-project ledger from the historical eight rows to nine.
- Qualify lossless parsing/exact-span rewrites, artifact compilation/locks, TypeScript type failures,
  keyed reconciliation/tombstones/high-water/Undo, one-transaction expansion, retained failures,
  bounds and save/repro.
- Prove cold/warm and native/WASM/RPC/TypeScript parity, finite geometry, explicit branches and
  normalized Hard residual `<= 1e-9` through existing validation.
- Materialize all eight demonstrations through accepted native authority and prove representative
  Lantern vertex, Bridge tower, Compass spoke and Neon shared-endpoint drags keep their generated
  consumers attached and current. Require the eight-row M84 ledger SHA-256
  `bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18` while the milestone-neutral
  271-row golden remains
  `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.
- Materialize the ninth manifold demonstration through the same accepted native authority. Require
  finite geometry, all active features Current, normalized Hard residual `<= 1e-9`, zero numerical/
  equality/bidirectional DOF, exactly one FixedPoint and no FixedCoordinate, eight radius-2.5
  circles with diameter-5 dimensions, three open routes, three closed seals, 21 construction spans
  and six adaptive Fillets. Record the reviewed replacement-ledger digest only after final
  qualification.
- Compile the checked-in managed-v1 two-line/Horizontal/Vertical/one-Fillet fixture through
  TypeScript, parse that same source in Rust and cold-materialize it through the unchanged
  accepted-scene path.
- Prove pointer frames do no parsing/expansion and existing drag/terminal performance ceilings
  remain green.
- Pass format, warnings-denied Clippy/Rustdoc, locked all-feature tests, actual WASM, TypeScript,
  golden require-clean, Trunk and the complete clean release gate.
- Freeze one no-rebuild M84 candidate, verify it locally and through retained Tailscale UAT, and
  publish to Pages only after explicit supervising-user approval and exact hosted-byte proof.
- Exact F010 source `cf463838625e42ba9a0f58fe6e061dd7032c753d` historically passes the complete
  clean gate, no-rebuild freeze, identical temporary/retained eight-path HTTP ledgers, focused
  Compass 1/1 and carried 14/14 browser matrix on both endpoints. F011 withdraws that nomination;
  exact F011 source `e28721a`, tree `0152097`, passes clean qualification, no-rebuild freeze,
  identical temporary/retained eight-path HTTP ledgers and focused manifold/PNG/authority 1/1 on
  both endpoints. F012 withdraws F011; exact source `84dd768`, tree `429ed56`, passes clean
  qualification, no-rebuild freeze, identical temporary/retained eight-path HTTP ledgers and
  focused annotation paint/pick plus visible/hidden PNG 1/1 on both endpoints. Refreshed human UAT
  U1-U16 remains pending.

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
remains reachable.

Historical F004 replacement record (2026-08-26): exact product source
`c2cf160d3a7d5065e582f2ba982881380d2b871c`, tree
`94a178699f9b2e8bd2a6497c9b0334d43cad2b20`, passes the complete clean release gate. Its exact
no-rebuild seven-file output `/tmp/geosolve-m84-f004-hv-uat.FF5RFBZe` is immutable at
directory/file modes `0555`/`0444`, ordered-manifest aggregate
`f34c46ee5876c4bdb458863cc90c6c6b25281cc8e44f89c8d00eba0f16ca5bbc`, and byte/browser-verified
on temporary and retained endpoints. Baseline 4/4, F003 1/1 and F004 2/2 browser suites pass on
both. Historical service PID `3316682` served only those bytes at
`http://100.94.63.83:8080/`. The later direct-authoring amendment withdrew them from current
nomination solely because they predate amended scope; PID `3316682` was retired only after the
direct-authoring replacement passed temporary verification.

Withdrawn direct-authoring nomination (2026-08-26): exact source
`41e65a4f8c92179412ba2e06f44692377cd5fe51`, tree
`d31b805549a29433e157074bc181517bdb50fb67`, passes the complete clean release gate, unchanged
271-row golden and separate four-demo ledger. Its exact no-rebuild output is frozen at
`/tmp/geosolve-m84-authored-uat.ZYQQyBQQ`, directory/files `0555`/`0444`, ordered-manifest aggregate
`6f82bb261057916f737110cb6533da128d1afde72d1b2f5584f937acdd1a54b1`, with complete evidence at
`/tmp/geosolve-m84-authored-freeze-evidence.ngNf7jxZ`. Temporary and retained exact HTTP checks and
direct 3/3, baseline 4/4, F003 1/1 and F004 2/2 browser suites pass against those frozen bytes.
The former PID `4081080` record is historical evidence only. This adds no solver equation, browser
execution, custom artifact or fifth bundled golden row. The later combined F005/F006 source
`ff2e142` and its frozen candidate are withdrawn by M84-F007.

Historical F007 nomination (2026-08-27): exact product source
`cc2f05ed97500f4bae4c0da6839362dbbc8c2e53`, tree
`6b8fc417ac9464843ac14fb350a8e5f794b1cdb1`, passes the complete clean gate. The 6,194-line,
419,126-byte log `/tmp/geosolve-m84-f007-release-gate.fn6ZHE.log` has SHA-256
`40c73a8856df0905e85e2b877a82db0e0e81da583764ccc18d729e268f01763b`. The unchanged 271-row
golden and four-demo M84 ledger retain SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`73b25bd00344229e33971a71c8025e4c6e17ff970106f7e1afafd3b3179dcf7e`.
Without rebuilding, the exact seven-file output is frozen at
`/tmp/geosolve-m84-f007-uat.KgW8fpLf`, directory/files `0555`/`0444`, ordered-manifest aggregate
`8f03810911b1ff96c4f825e005125250db804f463389953e937005ec505b7ab9`, with complete evidence at
`/tmp/geosolve-m84-f007-freeze-evidence.rP5rQcTG`. Temporary `:18087` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`efa609c6bac127753336c3634730b81bed04699a25c6394ab039c7f06b0b2b64`; all 14 browser cases pass
on both endpoints. Historical PID `4081080` and temporary PID `34895` are retired. PID `62376`
served only that immutable snapshot at `http://100.94.63.83:8080/`. The later F009 replacement
superseded it before F010 withdrew that nomination; PID `62376` is retired.

Withdrawn F009 replacement nomination (2026-08-27): exact product source
`c74651cc82506e31926042df65a1eeec08a6af9d`, tree
`a904584410ca9a8cd3112d17ad70c0e84c29e8d9`, passes the complete clean release gate. Its
6,218-line, 421,590-byte log has SHA-256
`c9b743c8f95d6df7706b04e2d820ac67426f1b11ec447d2bffdc08cd1fe0f6f1`. The unchanged 271-row
golden and expanded eight-demo M84 ledger have SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`.

Without rebuilding, the exact seven-file output is frozen at
`/tmp/geosolve-m84-f009-uat.q8cKIN3v`, directory/files `0555`/`0444`, ordered-manifest aggregate
`23f2f839f2a3be6b722ae26cb548f0a19ce2f3d6afac90d5f913938a042d1c1f`, with complete evidence at
`/tmp/geosolve-m84-f009-freeze-evidence.3FoVTQ6m`. Temporary `:18089` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`add827e88d17735cfb6cb0bbecec885f5680db0bd11b67bb591673d566b90676`; all 14 browser cases pass
on both endpoints. Temporary PID `3943194` and superseded F007 PID `62376` are retired.
M84-F010 withdraws this nomination. Historical PID `3965271` and the temporary F010 development
PID `238809` are retired; the immutable F009 snapshot is preserved.

Historical F010 replacement nomination (2026-08-27): exact product source
`cf463838625e42ba9a0f58fe6e061dd7032c753d`, tree
`992e587609e61768a9af76af193df2fad8325829`, passed the clean release gate from
15:58:23.055857854 through 16:17:18.303733569 AEST, exit 0. The 6,209-line, 420,425-byte log
`/tmp/geosolve-m84-f010-gate.9NvAi3z5/release-gate.log` has SHA-256
`bf57345266005a85b6da20f1105c3cf126d2492ba91e413ef0f07c5d38d3b28a` and ends in Trunk
success. The unchanged 271-row golden and eight-demo M84 ledger have SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`.

Without rebuilding, the exact seven-file output is frozen at
`/tmp/geosolve-m84-f010-uat.7R5eXQoz`, directory/files `0555`/`0444`, ordered-manifest aggregate
`ca2302e0e0a1f08525be98202d593c72de1af303b664f6ff7a64a70727e7f72e`, with complete evidence at
`/tmp/geosolve-m84-f010-freeze-evidence.sXWXNG0Z`. Temporary `:18091` and retained `:8080`
eight-path ledgers are byte-identical at SHA-256
`57f2f4c2b47a11db8fc76a7f6a2e3d30555cb36e96b191081454a4c47fb85cbe`; every path has exact
bytes and MIME. Focused Compass 1/1 and the carried 14/14 browser matrix pass on both endpoints.
The focused scenario performs six drags and proves the center exact at release and
+50/+250/+500/+1000 ms, four spokes attached, finite accepted authority and exact reload. Its spec
and config have SHA-256 `4b97f570d5122a353b4ee104b26ea46427ca1c3b79aa5a8d35fee7875302dab0`
and `c0900c1132352ed9471321a2cf5727baf2c004d8eebf1a1bcd8a43146289df9c`.
Historical F010 PID `650971`, F009 PID `3965271` and temporary F010 PIDs `238809`/`621532` are
retired; their snapshots remain preserved. M84-F011 withdraws F010 from current nomination.

Historical F011 qualification/nomination (2026-08-27): exact source
`e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
`015209773f81ec1a254817c65ef2a71b984e3b08`, passes the complete clean gate from
18:38:08.068586771 through 18:55:55.205076185 AEST, exit 0. The 6,251-line, 422,664-byte log
`/tmp/geosolve-m84-f011-gate.GvBT6f/release-gate.log` has SHA-256
`ceea545929196981e2790a822b384596642f63a6ef93ecea98f76799cdd7e353` and ends in Trunk success.
The unchanged golden and nine-demo ledger have SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`c610a229e490467f59c9d57f334c96f23f61ea98a713eb2c98daa3c773eab66f`.

The exact no-rebuild seven-file output is frozen at `/tmp/geosolve-m84-f011-uat.ps736NLh`, modes
`0555`/`0444`, aggregate `056193f4af17437da5430dc86059ad4c4b73ec62e959a461935ca29153b10fc2`,
with evidence at `/tmp/geosolve-m84-f011-freeze-evidence.4deymgss`. Temporary `:18093` and
retained `:8080` eight-path ledgers are byte-identical at SHA-256
`9339301ea57feb293a27256795344f88805046426e3659bae0f750b67b251b94`; focused frozen
manifold/PNG/authority runs pass 1/1 in 12.8 and 12.9 seconds and preserve lifecycle, history
length, project title and viewport markup authority. Their exact 233,543-byte PNG has SHA-256
`2378a8c74216524c42fc8910d79e28a6dacc54fc437ac705518ab42122363dc2`, valid signature and
2000 × 1400 IHDR; the screenshot has SHA-256
`2db91740662d19adf9f38518ed25deca91ad92e37022dfa3549caad0d608bcb0`. Only after temporary proof
passed was F010 PID `650971` retired. Retained `geosolve-m84-uat.service`, PID `1485656`, invocation
`f04bc05089d94947b7a24d8ec6a6f26d`, served only F011 from its immutable snapshot working directory
at `http://100.94.63.83:8080/`; temporary `:18093` is retired. M84-F012 withdraws that nomination,
and PID `1485656` was retired only after temporary F012 proof passed.

Current F012 qualification/nomination (2026-08-27): exact source
`84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
`429ed56d2a5b3988d6604079d19e1002f9049d64`, passes the complete clean gate from
20:36:27.840305756 through 21:01:30.826307821 AEST, exit 0. The 6,274-line, 424,393-byte log
`/tmp/geosolve-m84-f012-gate.PmjeGNUa/release-gate.log` has SHA-256
`04e35c73fe92ca3e089b87bd13b5221c60835b72c9eeba5ed38916b51150004a` and ends in Trunk success.
The unchanged golden and nine-demo ledger retain SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
`c610a229e490467f59c9d57f334c96f23f61ea98a713eb2c98daa3c773eab66f`.

The exact seven-regular-file, zero-symlink no-rebuild output is frozen at
`/tmp/geosolve-m84-f012-uat.nMOymIIM`, modes `0555`/`0444`, aggregate
`166abc1298220090ba4c8b0a37a176fb4f945cceae68771efbd601acc1970169`, with evidence at
`/tmp/geosolve-m84-f012-freeze-evidence.qua6ci1b`. Temporary and retained eight-path HTTP ledgers
are byte-identical at SHA-256
`66fcd4c852baab5290605066ec856239af7c4f033cef55a4dfd5fb86058645ba`; focused browser runs pass
1/1 on each endpoint and prove annotation paint/pick removal, underlying-target access, authority
neutrality, exact restoration and WYSIWYG visible/hidden 2000 × 1400 PNG export.

Retained `geosolve-m84-uat.service`, PID `2241323`, invocation
`b621b1a43b8c4ee281f1e8edddf10e57`, serves only F012 from its immutable snapshot working directory
at `http://100.94.63.83:8080/`; the temporary service is retired. M84-U1 through M84-U16, explicit
approval, Pages publication, service retirement and closure remain open.

## Bounds and non-goals

Managed source is at most 4 MiB, each artifact 16 MiB and the complete code project 64 MiB; existing
node/child/operation/response/depth limits remain. External patch source is trusted build input;
artifacts are untrusted bounded runtime data. No new primitive, constraint, residual, formula,
priority, Offset redesign, arbitrary TypeScript execution, browser `eval`, remote-package runtime,
3D/B-rep feature or general topological-naming claim is included. Generated definitions without an
edit lens are read-only; explicit free-placement overrides are supported and general ejection is
deferred.
