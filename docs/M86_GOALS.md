<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M86 — Focused bug fixes and UAT follow-up

Status: **active and unaccepted; M86-F001 is implemented and focused-qualified; complete clean
qualification, immutable nomination and human UAT are pending**. Accepted M85 product/public
evidence remains authoritative until every M86 gate passes. This initial scope contains exactly one
confirmed defect; later reports enter the batch only after independent reproduction and a recorded
M86 finding.

## Goal

Make Inspector editing of code-owned direct dimensions behave like genuine collaborative
code/GUI authoring. A valid value edit must rewrite the authenticated managed TypeScript
declaration, rematerialize through the existing Intent/native validation path and publish exactly
one outer code transaction. It must not mutate an expanded native leaf and then ask generic
code-checkpoint reconciliation to infer source intent.

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
- Focused tests, formatting, diff hygiene, warnings-denied affected-crate Clippy and the relevant
  WASM target check pass. Unchanged clean golden and proportional workspace/release gates remain
  required before immutable nomination.

## Bounds and non-goals

M86-F001 adds no primitive, constraint, dimension family, residual, Jacobian, solver priority,
tolerance, branch rule, formula language, persistence format, code grammar or point-overlay
semantics. It does not make computed/generated dimensions editable, generalize arbitrary Intent
leaf reconciliation, decode opaque aliases, add runtime TypeScript, or redesign M84 collaborative
authoring. Additional M86 bug fixes require their own confirmed finding and explicit scope entry.

## Release sequence

1. Add the exact failing owner regression and thin adapter proof.
2. Implement authenticated managed-source routing and qualify valid, invalid and Undo/Redo paths.
3. Pass proportional native/WASM/workspace/release gates with unchanged golden authority.
4. Freeze without rebuild, exact-verify local/Tailscale bytes and complete M86-U1 through M86-U5.
5. Only after explicit supervising-user approval, publish the accepted descendant to Pages,
   exact-verify hosted bytes, retire the retained candidate services and close M86.
