<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M69 focused UAT — Profile and construction geometry

Status: not started. Implementation and direct/release qualification must pass before this
scorecard is served.

Candidate source: pending

Tailscale endpoint: pending

Release distribution manifest: pending

Use only the ordinary GeoSolve Sketch Workbench. Direct Rust tests are the mathematical,
persistence and interaction-policy authority; this scorecard assesses discoverability,
predictability and visual clarity. M69 does not qualify persistent point roles, canonical sketch
v5, Offset/Mirror UI, computed chaining, Bake/Explode, computed production topology, marquee
selection, mobile behavior or a legacy route.

## M69-U1 — author and convert Construction curves

1. Start a new sketch and activate **Construction** before drawing a line, polyline, rectangle and
   curved primitive.
2. Return to Profile authoring and draw comparable geometry.
3. Select several Profile curves and press **Construction** once; repeat when all are Construction.
4. Select only one span of a polyline and toggle its role, then Undo and Redo.
5. Refresh the page after an accepted role change.

Expected: every curve created by one operation receives one role atomically; standalone points
remain neutral. A selected batch changes in one history step, one polyline span targets the whole
persistent polyline, and mixed selections deterministically become Construction. Geometry,
constraints and accepted coordinates do not move. Persistent roles survive ordinary workspace
reload.

Result: Pending.

Notes:

## M69-U2 — overlap priority and pick scopes

1. Open **Samples → Curves & constructions → Construction and reference geometry**.
2. In **All geometry**, click the exactly overlapping Profile edge/Construction guide several
   times and at several zoom levels.
3. Switch to **Construction only** and click the same location; switch to **Profile only** and
   repeat.
4. Select the Construction diagonal through the canvas and through the tree, then drag or apply a
   representative compatible constraint.

Expected: Profile predictably wins the near-identical overlap in All mode. Construction-only and
the grouped tree provide direct access to the guide; Profile-only excludes it. Scope changes do not
mutate document/history. Construction geometry remains solver-active and editable.

Result: Pending.

Notes:

## M69-U3 — explicit construction visibility and shared points

1. Toggle **Explicit guides** off and on in the canvas Construction control.
2. Inspect rectangle corners shared with the Construction diagonal.
3. Select or drag a shared point under Profile, Construction and All pick scopes.
4. Select a construction-only point/control and compare the three scopes.

Expected: visibility does not alter solver state. Shared points remain accessible in both relevant
scopes because they are one neutral persistent point, not duplicated or assigned an invented role.
Construction-only controls follow Construction visibility/scope, while free points remain
ordinary Profile interaction targets.

Result: Pending.

Notes:

## M69-U4 — Fillet-hidden implicit construction

1. Open **Samples → Curves & constructions → 2D Fillet playground** and author a line-line Fillet.
2. Inspect the source portion between its old corner and new contact.
3. Click that dashed hidden portion, then inspect the tree/Inspector and apply a harmless selection
   action.
4. Toggle **Fillet-hidden portions** off/on and compare All/Profile/Construction pick scopes.
5. Put Fillets on both ends of one bounded span and inspect both discarded complements.

Expected: each discarded portion has a lighter implicit-construction dash. Clicking it selects and
highlights the complete native source rather than creating a fragment row or identity. Profile
scope ignores the discarded occurrence, Construction scope selects it, and hiding it changes only
presentation/interaction. Both-end composition remains stable and finite.

Result: Pending.

Notes:

## M69-U5 — failure, closed loops and ordinary workflow

1. Exercise a conflicting/invalid Fillet preview and suppress an existing Fillet set.
2. Fillet against a full circle or ellipse, then compare with an arc/open curve.
3. Move native source points, adjust Fillet radius/branch actions and use Undo/Redo.
4. Author representative constraints before and after role/visibility changes.

Expected: invalid, conflicting or suppressed output leaves the untouched native source and no
construction ghost. Full-period loops remain whole; open parents expose only valid discarded
complements. Existing Fillet interaction, pointer capture, problem overlays, constraint authoring,
camera controls and workspace history remain usable.

Result: Pending.

Notes:

## Approval

Supervising-human decision: Pending.
