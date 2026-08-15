<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 focused UAT — Retained authoring semantic consolidation

Status: **M73-F004 is implemented and focused/proportionally editor-qualified; the F001-F003
candidate remains withdrawn and no replacement has been nominated**. Direct Rust/WASM tests remain
authoritative for semantic dispatch, candidate identity, accepted state and mutation-free
rejection. Human review resumes only after the clean replacement release gate and publication pass.

Historical withdrawn candidate source: `efde645345577f44e0d6b691f7ca27eb587c4b53`

Historical candidate tree: `ae1ddaebd75e740c48eafc0b9ef2ad07cd99378b`

Historical endpoint (not current UAT authority): `http://100.94.63.83:8080/`

Historical immutable snapshot: `/tmp/geosolve-m73-uat.5EhWNL`

Historical ordered-manifest aggregate:
`371596d68a75ce4415970d3237f0511426958918b55b1376fc44700735ba2095`

Replacement candidate: **not yet nominated**.

The historical candidate passed the complete clean release gate. All seven files and `/` were
fetched with proxy/cache bypass and identity encoding, returned HTTP 200 and matched the frozen
snapshot byte-for-byte. M73-F004 preserves that evidence but withdraws those bytes from continued
UAT; do not record U1-U4 results against them.

## Completed mechanical prerequisite — M73-F004 span-axis precedence

An eligible live world Horizontal span whose inference policy both adjusts coordinates and
persists the constraint must suppress same-axis `HorizontalPoints` and
`HorizontalPointToMidpoint` candidates and guides. Live world Vertical must symmetrically suppress
`VerticalPoints` and `VerticalPointToMidpoint`. Orthogonal point/native-midpoint plus world-axis
bundles remain available; remembered Parallel/Perpendicular/Collinear behavior and solver
redundancy rejection remain unchanged.

This narrowly supersedes M71-F004's same-axis-alternative rule only for eligible live world-axis
span constraints. Implementation commit `4fb9a7dd67ea86cd268028b5fa5c7842c56f2a88` passes public
regression `m73_f004_span_axis_precedence` 3/3, including finite accepted geometry/residual and
exact retained history. The inference-owner precedence, pre-pair filtering, orthogonal-bundle and
remembered-direction controls; complete editor suite; M71 F003/F004/F005 and transition parity;
warnings-denied Clippy; and unchanged 234/234 golden survey/check/clean gate all pass. Clean
replacement qualification/publication and this focused UAT remain pending.

## M73-U1 — Line and polyline stage continuity

1. Draw an ordinary Line and a multi-segment Polyline with point reuse, H/V inference and one
   remembered-reference inference.
2. Confirm each staged preview, retained relation and final segment refers to the point/span the
   cursor indicated.
3. Undo/Redo the completed Line and Polyline, then cancel a partial Polyline, reactivate the tool
   and redraw it.

Pass when stage ownership, segment numbering, references and history feel unchanged from M71/M72.

## M73-U2 — Contextual relation authoring

Exercise representative point, point/curve, line/line, center-bearing and curve/curve selections,
including Horizontal/Vertical, Coincident, Point on curve, Parallel/Perpendicular, Equal,
Concentric, Collinear, Tangent and Continuity. Confirm compatible selections apply once, invalid or
incomplete selections show their normal typed warning, and Undo/Redo preserves ordinary selection
and accepted-scene behavior.

Pass when the contextual tool surface retains its existing availability, operand order, branch
choices and error presentation. The retired direct Rust compatibility API has no browser control
and should create no visible omission.

## M73-U3 — Compound inference provenance and recovery

In the retained drafting-relations playground, check one line and one polyline endpoint using:

- a point-axis plus perpendicular span-direction bundle;
- Horizontal alignment to one stored point plus Vertical alignment to another;
- an ambiguous alternative followed by a deliberate candidate choice or cursor retreat.

Confirm the preview guides, snapped point and retained relations describe the same choice. Cancel,
Undo and retry; no stale guide or relation may survive.

Pass when compound candidates remain predictable and every rejection/recovery leaves the last
accepted scene intact.

## M73-U4 — Live world-axis precedence

On the replacement candidate, exercise both Line and Polyline endpoints:

1. Wake a stored point on the same Horizontal axis as a live Horizontal span, then repeat with a
   native midpoint. Confirm only the live Horizontal candidate/constraint-backed guide survives;
   no Horizontal point or point-to-midpoint guide remains.
2. Repeat symmetrically for live Vertical with a stored point and native midpoint.
3. Wake a point and midpoint on the orthogonal axis and confirm each still composes with the live
   world-axis span into the expected two-guide, two-relation bundle.
4. Check remembered Parallel, Perpendicular and Collinear alternatives on Cartesian supports, then
   attempt an actually redundant retained relation and confirm ordinary solver rejection remains.
5. Commit, Undo/Redo, cancel and retry representative cases; no suppressed guide, stale relation or
   extra history step may survive.

Pass when live world H/V direction intent clearly owns its same-axis coordinate while orthogonal
bundles, remembered-direction behavior and retained solver authority remain unchanged.

## Approval record

- M73-U1: pending human review on a replacement candidate.
- M73-U2: pending human review on a replacement candidate.
- M73-U3: pending human review on a replacement candidate.
- M73-U4: mechanical prerequisite passed; replacement-candidate human review pending.
- Final M73 approval: pending.
