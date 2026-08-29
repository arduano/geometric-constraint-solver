<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 — Focused bug fixes and UAT follow-up

Status: **accepted at milestone level on 2026-08-29; clean-source qualification and public closeout
remain pending**. M86-F001-F003 and the bounded interaction-trace diagnostic are approved by the
supervising user's explicit close decision. M86-U1-U8 are accepted without claiming a separately
logged row-by-row replay. Accepted M85 Pages remains public-byte authority until the approved M86
descendant passes clean qualification, Pages publication and exact hosted-byte verification. The
former F002 snapshot remains withdrawn historical evidence.

## Goal

Complete a focused bug-fix/UAT batch without changing solver mathematics. F001 makes Inspector
editing of code-owned direct dimensions behave like genuine collaborative code/GUI authoring. F002
defines one deterministic Select-mode hit hierarchy in which the compact Fillet radius grip remains
the most specific control, a visible persistent endpoint owned by either Fillet parent remains
reachable through the broad Fillet surface, and unrelated geometry stays below that surface. F003
keeps a valid Typed Panel rectangle terminal at its accepted release position when independently
staged public computed Fillet geometry differs only through causally bounded rectangle-alias
roundoff. A bounded memory-only interaction trace makes a future terminal recurrence diagnosable
without requiring the increasingly large complete reproduction payload.

## Confirmed baseline

On current source `4b69a57`, open the `pc-water-manifold` project and select Inspector target

`code.dimension.2cabcaba35f1866930e2549cbd95d899abeb2656e495bf047909f1d92176218b`.

Accepted expansion provenance authenticates that alias to managed declaration
`topScrewRail3Length`, whose exact source is `target: mm(16)`. Changing the Inspector value to `8`
reports:

`Code-owned edit was not applied: this code-owned placement has changed leaves without semantic GUI draft provenance`

Source, annotation and accepted native target remain `16`, and no outer code-history entry is
created. The reported user need is generic value editing; the exact selected alias happens to own
`16`, so the frozen regression uses `16 -> 8` rather than relying on the initially described
value.

The generic Inspector adapter currently edits the nested Intent scalar first. Generic code-owned
checkpoint publication then rejects the changed leaf because its semantic reconciliation supports
point-placement overlays, not a direct dimension target. This is optional code-project/workbench
composition, not a solver equation, convergence or dimension-residual defect.

On F001 nomination source `bcc5ae4`, Select-mode hover and pointer-down at the persistent shared
corner of two Fillet parents both resolve to `FeatureCorner` whenever the computed radius rail or
arc tolerance also covers that sample. The ordinary native hit surface independently resolves the
same sample to its persistent `Point`. `ConstraintEditor::resolve_select_pointer_target` asks the
blended Fillet resolver before every native point, and that resolver deliberately merges grip,
spoke, continuation rail and arc into one radius surface. This is a headless picking-priority
defect, not an SVG/DOM hitbox or computed-feature evaluation defect.

The first F002 repair and immutable candidate covered that shared source corner. Focused UAT then
expanded the same report: draw one right-angle two-span `Polyline` with both legs length `2`, and
request a Fillet radius of `2`. The exact radius is the evaluator's tangent-at-endpoint fold
boundary; accepted radius `1.99` robustly preserves the reported visible surface while proving that
the broad Fillet hit can cover both remote parent endpoints and make their point cores unreachable.
This is the same Select resolver, symptom and root cause, so it remains M86-F002 and withdraws the
`dbe94da` candidate rather than opening another finding.

M86-F003 reproduces independently in the bundled **Typed Panel · keyed Fillets** project. Drag the
upper-left corner from `[0, 40]` through an accepted preview to `[3, 38]` and release. The native
terminal is valid, but terminal publication reports
`terminal code drag differs from its independently staged native authority in computed features`
and the durable scene returns to the pre-drag location. This is not drafting inference or solver
snapping. It is a retained code-workbench terminal-authority rejection: M84-F010 already admitted
tightly bounded redundant rectangle-alias normalization in design and accepted documents, while
its computed snapshot comparison remained bit-exact even when the same normalization caused
ULP-scale public Fillet DTO differences. M86-F003 is an M84-F010 scope recurrence; the historical
M84 ID remains stable.

## Required behavior

### G1 — Authenticated managed-dimension route

- Authenticate the Inspector projection/session identity and exact selected semantic alias against
  current accepted code-project authority.
- Resolve the alias through accepted expansion provenance to one managed declaration; never parse
  or decode the opaque `code.*` digest.
- Accept only a direct `dimension.curveLength` or `dimension.diameter` declaration whose edited
  leaf is exactly its scalar target. Wrong ports, fields, indexes, kinds, stale aliases and foreign
  projects fail before mutation.
- Rewrite managed source argument path `target` through the existing scalar-lens machinery. Do not
  edit native document authority or manufacture a point-placement overlay.

### G2 — Atomic materialization and history

- Parse, expand, materialize and independently validate through the ordinary code/Intent/native
  pipeline, then atomically install the returned delegated editor and code session.
- A valid edit changes only the authenticated target token, updates accepted native target and
  geometry, immediately reselects the stable semantic alias, and creates exactly one outer code
  history entry with no visible nested Intent Undo step.
- Undo restores exact prior source, native value and checkpoint; Redo restores the edit and proves
  the same semantic alias remains available. This does not require durable UI selection across the
  history step. Reload and repro preserve the same project and accepted authority.
- A representable but invalid value retains the failed managed source and local diagnostic for
  correction while preserving the complete previous accepted geometry/checkpoint. Undo restores
  exact source and accepted state.

### G3 — Compatibility and failure boundaries

- Ordinary GUI-owned dimensions retain the existing generic Inspector path.
- Dirty source, retained failure, stale/foreign Inspector identity, unsupported code declarations,
  wrong leaves, malformed/non-finite scalars and failed rematerialization cannot partially mutate
  source, delegated editor, selection, history or accepted authority.
- The browser remains a thin dispatcher. The owning code-project regression proves source,
  materialization, acceptance and history; one thin adapter test proves the displayed Inspector
  control reaches that route.

### G4 — Reachable native Fillet source corners

- Select hover and pointer-down use this order: compact Fillet radius grip; visible persistent
  endpoint owned by either current Fillet parent; broad Fillet arc/spoke/continuation-rail surface;
  ordinary geometry. The compact grip remains an explicit radius control even when it overlaps a
  parent endpoint marker.
- Exact shared endpoint identity and distinct endpoints joined by active explicit Coincident
  topology remain one semantic corner. Coordinate proximity alone never creates topology.
- Coincident representative construction is request-local and lazy. Broad Fillet arc/spoke/rail
  motion outside every endpoint halo performs no document-wide topology traversal; overlapping
  Fillets share at most one traversal once an actual endpoint candidate requires equivalence.
- A remote endpoint belonging to only one parent still preempts that Fillet's broad surface. If
  crowded zoom puts distinct opposite-parent endpoint halos under the same sample, the nearer point
  wins; an exact cross-parent distance tie remains with the Fillet rather than inventing topology.
- Unrelated overlapping points and passive native curves remain below the broad Fillet surface.
  Active Fillet authoring and painted-radius reconciliation retain their existing resolver.
- Hover and pointer-down share this headless result: a winning endpoint paints Point hover, selects
  that persistent Point and starts an ordinary Point gesture. No browser adapter reconstructs
  incidence or overrides priority.

### G5 — Typed rectangle terminal keeps causally bounded computed parity

- Preserve M84-F010's authenticated point lens, complete coupled semantic closure, exact rectangle
  anchors and atomic rematerialization. F003 changes only terminal parity after both candidates are
  already independently accepted.
- Design-document and current-accepted-document comparison must both normalize the same nonempty
  set of redundant rectangle alias points under the existing finite 8-ULP/near-zero seed cell.
  If either side is exact, the sets differ, or either comparison fails, computed parity stays exact.
- From that authenticated point set, derive only accepted source curves whose public definitions
  reference a normalized point. Bounded finite scalar parity applies only to public computed edges
  or construction fragments sourced by those curves, including a Fillet arc whose public contacts
  reference one of them. Every unrelated public edge or fragment remains bit-exact.
- More than 8 ULP, an out-of-cell near-zero value, non-finite data, or changed edge identity, role,
  source, sweep, tangent orientation, contact winding, provenance, topology or public evaluation
  mapping rejects parity. Feature definitions, persistent identities, ownership and allocator
  high-water remain exact. Revision/digest and evaluation-lifecycle stamps may refresh during
  canonical rematerialization and are deliberately outside parity.
- Private continuation certificates, including their transverse-orientation metadata, are not part
  of the demo-workbench public computed DTO parity surface. They are not implicated by this Current
  Typed Panel reproduction and are neither relaxed nor promoted into this adapter contract.

### G6 — Bounded gesture evidence without retained authority

- Managed code projects expose one `Copy trace` command for the latest projectional pointer gesture;
  flat/non-code workspaces keep it disabled. A new pointer-down resets only the diagnostic buffer.
- Record raw browser coordinates/timing/buttons/modifiers, coalesced and animation-frame samples,
  native pointer-up, semantic routing, parity decisions, publication, rollback/restore,
  persistence and presentation work. Floating-point evidence includes exact bits and the first
  computed mismatch's field, ULP distance and applicable bound.
- Keep the export memory-only and capped at 128 KiB. It must not enter source, retained history,
  workspace persistence, local storage or reproduction payloads, and must not include managed
  source or complete editor snapshots.
- Preserve the opening pointer-down and newest terminal/rejection/rollback evidence when a long
  gesture overflows. Reuse the read-only reproduction overlay with selected-text fallback when an
  insecure HTTP context blocks clipboard access.

## Qualification

- The exact manifold alias/declaration is frozen by a focused owner regression. It asserts the sole
  source change is `target: mm(16)` to `target: mm(8)`, native target is `8`, geometry is finite,
  normalized Hard residual is independently at most `1e-9`, immediate alias reselection succeeds
  and outer history advances exactly once.
- Exact Undo/Redo, retained-invalid diameter `5 -> 0` plus Undo, and an accepted direct-diameter
  `5 -> 8` fixture pass, so the route is covered across the two supported direct managed dimension
  declarations.
- A thin `geosolve-demo-web` Inspector adapter regression and GUI-owned collateral pass. Do not
  expand or rewrite the milestone-neutral golden because no authoring family, equation or scene-
  authority state is new.
- The expanded F002 parity target contains 18 rows. It includes the exact shared
  corner, active Coincident and coordinate-only controls, post-Apply state, the two-by-two
  near-radius-two remote endpoints, compact-grip precedence, nearer-endpoint selection and a real
  two-Fillet overlap whose nearer broad owner does not own the exposed endpoint.
- The F003 owner regression repeats same-session upper-left terminals to `[3, 38]`, `[5, 37]` and
  `[-2, 36]`.
  Each terminal, publication and reload retain the exact coordinate, retain two canonical drafts and
  one revision, keep keyed Fillets Current, keep geometry finite and independently validate Hard
  residual at most `1e-9`. Focused negative tests freeze causal source scope, matching nonempty
  design/accepted alias sets, the 8-ULP bound and exact public discrete Fillet state. Human UAT
  still owns pointer-feel confirmation of that automated sequential contract.
- F002 native/WASM parity passes 18/18. A focused unit regression proves a real broad arc hit
  outside every point halo leaves the request-local Coincident cache uninitialized. Warnings-denied
  affected-crate Clippy, the full 430-test editor library and unchanged 271-row golden
  survey/check/require-clean pass.
- Focused F003 owner, boundary and Compass Rose collateral tests pass. The complete demo-web run
  passes 307/307 library tests plus binary, integration and doc-test targets. After the diagnostic
  trace was added, the expanded demo-web library passes 316/316; six focused trace-bound tests,
  warnings-denied demo-web Clippy, formatting, the locked WASM check, release Trunk build and the
  unchanged golden `--check` pass.
- The complete provisional dirty-worktree release gate, no-rebuild freeze and exact temporary,
  local and Tailscale served-byte verification pass. This authorizes human UAT of the exact frozen
  patch identity; it does not substitute for a committed clean-source nomination before Pages.

## Bounds and non-goals

M86 adds no primitive, constraint, dimension family, residual, Jacobian, solver priority,
tolerance, branch rule, formula language, persistence format, code grammar or point-overlay
semantics. It does not make computed/generated dimensions editable, generalize arbitrary Intent
leaf reconciliation, decode opaque aliases, add runtime TypeScript, redesign M84 collaborative
authoring, turn coordinate proximity into topology, globally place all native points above computed
Fillets, broadly weaken computed-scene parity, or turn diagnostic evidence into retained project
authority. Additional M86 bug fixes require their own confirmed finding and explicit scope entry.

## Release sequence

1. Preserve each report at its smallest public owner and keep the broad golden unchanged unless it
   exposes a systemic matrix gap.
2. Complete expanded F002 and F003 focused, collateral, native/WASM and affected-crate checks.
3. Pass the fresh full demo-web/workspace/release gates with unchanged golden authority.
4. Freeze the gate output without rebuild and exact-verify replacement local/Tailscale bytes before
   replacing any historical candidate service for UAT.
5. Complete M86-U6 through M86-U8. The supervising user's 2026-08-29 close decision supplies
   explicit approval; commit and clean-qualify the accepted descendant, publish it to Pages,
   exact-verify hosted bytes, retire candidate services and close M86.

F001 passed its mechanical prerequisites on source `9050424`, tree `65e0925`, and the supervising
user reported that candidate looked good before opening F002. Historical F002 source `dbe94da`, tree
`77f86c0`, and snapshot `/tmp/geosolve-m86-f002-uat.CPfe9QD8` passed their then-current gate and byte
verification, but the expanded F002 report withdraws them from current UAT. They remain immutable
historical evidence.

The provisional served-build identity is the saved pre-gate 160,117-byte, 2,976-line binary patch
over HEAD `4730e156e17cf3df88b9681a22961d41b686c2ff` and tree
`23a76c3b7141f10064d899113b97135932d23033`: patch SHA-256
`feafcc2a717a9c1bf9ff7a708b705903b2e18c6ef67327f533d66819a8784a57`, status SHA-256
`945ef3534016a5735c42c6fedaf72e66be2acc41ce9dc764db6e5896c3636b5a`. Saved pre/post-gate patch
and status files are byte-identical. Subsequent documentation-only worktree edits are outside that
served-build patch and do not alter the frozen seven-file candidate. The complete dirty-tree gate
exits with pipeline `0 0`; log
`/tmp/geosolve-m86-f002-f003-gate.yDrJlI8n/release-gate.log` has SHA-256
`93b645c2a2f1850f589b406943f3618da4fc833a42ff6066884602ec3e31ddb6`. Its exact output is frozen
without rebuild at `/tmp/geosolve-m86-f002-f003-uat.yGY3Nvly`, aggregate
`8f5a4ffcd96819b986ba81a9467d0c83a64365b2d21338cd134e164fa4444ce4`; complete freeze/HTTP
evidence is `/tmp/geosolve-m86-f002-f003-freeze-evidence.EsMzxE2v`. Temporary, local and Tailscale
eight-path ledgers are identical at SHA-256
`dca3e6eeba66e12c873ba4b5ba9b6cadd489060f0e4c7d5ce1070ed3344ec96f`. Temporary PID/invocation
`965128`/`a06c89f580744568b0d39677ee776da1` passed on `127.0.0.1:18102` and is stopped. The combined
candidate was then served at `http://100.94.63.83:8080/`; those listeners were later replaced by
the trace-enabled services below. This is provisional UAT evidence, not a clean committed-source
nomination.

The accepted trace-enabled descendant is frozen at `/tmp/geosolve-m86-trace-uat.U1C0QPSf`, with
exactly seven regular files, zero symlinks, directory/files `0555`/`0444` and ordered-manifest
aggregate `f5f429f70e42e3b39a8f22696c19ff81f358cfb10c43f7910baf386c9d82fd44`. Local and Tailscale
PIDs/invocations are `2433761`/`3f829abfff0a46eba586c07fed507d8e` and
`2433763`/`5f2c7c3eddac43119f380ec2e87b47c8`. `/` and all seven files match on both endpoints; evidence
`/tmp/geosolve-m86-trace-http-verify.fXS06h` has results SHA-256
`b2de59e63fc30a2dcbef108e671b1038103983bb95fea53d7410bb5799d080f3`. The supervising user's
2026-08-29 close decision accepts this descendant and M86-U1-U8. It remains UAT rather than clean-
source or Pages authority until the mechanical closeout completes.
