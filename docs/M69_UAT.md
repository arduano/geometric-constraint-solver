<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M69 focused UAT — Profile and construction geometry

Status: ready for focused supervising-human review; approval is pending.

Candidate source: `567141776c78178022f6123cbb399599ba713c62` on `main`

Tailscale endpoint: `http://100.94.63.83:8080/`

Release distribution manifest aggregate:
`1ffc65e4dadee3da240bad502254ea850a1cb9b11e06376572179b0ef1c75ba1`

```text
3dcb87723d1807a9829741aa31f5a53de003a460ecdf5e9a0516a32bb399caee  dist/API_COMPATIBILITY.md
ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e  dist/LICENSE
665e4df98334f5efea3efa83d18ea71198a182825c2d40f96dbf141e43a2a418  dist/THIRD_PARTY_LICENSES.md
cfc925cc92300bef04cefbcd19d0e28f0c40b884ea90f9c27df3ed17012be35e  dist/geosolve-demo-web-2b7cfc5e20c98b47.js
2e21db895e0305c60d983defdd4551f3ce10ae9ff258883a440e5acec071c6d0  dist/geosolve-demo-web-2b7cfc5e20c98b47_bg.wasm
f097939267de41cbb4246c6fb40a70aa5c0a03a273dfa4db5a6a994abb0c6611  dist/index.html
02e29144773da283540f73aabce70f6ce483f3a8be585a4fe7ed026e39b14393  dist/styles-642247db02aebd54.css
```

All seven served assets and `/` were fetched over the Tailscale endpoint and compared byte-for-byte
with the frozen local release distribution. Hard-refresh once before beginning if this browser tab
previously loaded an older candidate from the same endpoint.

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
