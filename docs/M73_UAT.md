<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 focused UAT — Retained authoring semantic consolidation

Status: **prepared; implementation and mechanical qualification pass, immutable candidate
nomination is in progress**. Direct Rust/WASM tests remain authoritative for semantic dispatch,
candidate identity, accepted state and mutation-free rejection; human review checks that the
behavior-preserving cleanup is genuinely invisible in ordinary use.

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

## Approval record

- M73-U1: pending qualified candidate.
- M73-U2: pending qualified candidate.
- M73-U3: pending qualified candidate.
- Final M73 approval: pending.
