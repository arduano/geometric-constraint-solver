<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 — Focused bug fixes and UAT follow-up

Status: **active and unaccepted; M86-F001 is implemented and human-approved, and M86-F002 is
implemented with replacement qualification pending**. Accepted M85 Pages remains public-byte
authority until explicit M86 approval and publication. The prior F001 immutable candidate remains
historical evidence while F002 is qualified and replaced.

## Goal

Complete a focused bug-fix/UAT batch without changing solver mathematics. F001 makes Inspector
editing of code-owned direct dimensions behave like genuine collaborative code/GUI authoring. F002
makes a persistent line/polyline corner shared by a computed Fillet's two native parents reachable
through the Fillet radius surface, while retaining the established radius grip/rail/arc precedence
over unrelated points and passive native curves.

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

- In Select mode, a finite interactive persistent point which is the shared endpoint of the two
  current Fillet parents wins over that Fillet's radius hit surface. Two distinct stored endpoints
  joined by an active explicit Coincident relation count as the same semantic corner; coordinate
  proximity alone never does.
- The exception is source-specific. An unrelated native point overlapping the computed arc, the
  radius grip/spoke/rail away from the source corner, the computed arc over passive native curves,
  active Fillet authoring and painted-radius reconciliation retain their established precedence.
- Hover and pointer-down use the same headless resolver: the corner paints native point hover,
  selects that point and starts the ordinary Point gesture. No browser adapter reconstructs source
  incidence or overrides the headless result.

## Qualification

- The exact manifold alias/declaration is frozen by a focused owner regression. It asserts the sole source
  change is `target: mm(16)` to `target: mm(8)`, native target is `8`, geometry is finite,
  normalized Hard residual is independently at most `1e-9`, immediate alias reselection succeeds
  and outer history advances exactly once.
- Exact Undo/Redo, retained-invalid diameter `5 -> 0` plus Undo, and an accepted direct-diameter
  `5 -> 8` fixture pass, so the route is covered across the two supported direct managed dimension
  declarations.
- A thin `geosolve-demo-web` Inspector adapter regression and GUI-owned collateral pass. Do not
  expand or rewrite the
  milestone-neutral golden because no authoring family, equation or scene-authority state is new.
- Focused tests, formatting, diff hygiene, warnings-denied affected-crate Clippy, relevant WASM
  parity, unchanged 271-case golden and the complete clean workspace/release gate pass. The exact
  gate-produced distribution is frozen without rebuild and byte-verified locally and on retained
  Tailscale before human UAT.
- The exact F002 overlap regression proves the ordinary native surface and the Fillet radius
  surface both contain one shared source-corner sample, then requires identical native/WASM hover
  and pointer-down ownership by the persistent point. The existing unrelated-point overlap and
  radius affordance regressions remain passing unchanged.

## Bounds and non-goals

M86 adds no primitive, constraint, dimension family, residual, Jacobian, solver priority,
tolerance, branch rule, formula language, persistence format, code grammar or point-overlay
semantics. It does not make computed/generated dimensions editable, generalize arbitrary Intent
leaf reconciliation, decode opaque aliases, add runtime TypeScript, redesign M84 collaborative
authoring, turn coordinate proximity into topology, or globally place all native points above
computed Fillets. Additional M86 bug fixes require their own confirmed finding and explicit scope
entry.

## Release sequence

1. Add each exact failing owner regression and the thin adapter proof where a boundary is crossed.
2. Implement authenticated managed-source routing and the narrow Fillet source-corner precedence.
3. Pass proportional native/WASM/workspace/release gates with unchanged golden authority.
4. Freeze without rebuild, exact-verify local/Tailscale bytes and complete the M86 UAT scorecard.
5. Only after explicit supervising-user approval, publish the accepted descendant to Pages,
   exact-verify hosted bytes, retire the retained candidate services and close M86.

F001 passed those mechanical prerequisites on source `9050424`, tree `65e0925`, and the supervising
user reported that candidate looked good before opening F002. Its immutable snapshot remains live
at `http://127.0.0.1:18101/` and `http://100.94.63.83:8080/` until a qualified F002 replacement is
ready. F002 replacement qualification and its focused human recheck remain pending.
