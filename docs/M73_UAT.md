<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 focused UAT — Retained authoring semantic consolidation

Status: **qualified immutable candidate nominated; focused human UAT pending**. Direct Rust/WASM
tests remain authoritative for semantic dispatch, candidate identity, accepted state and
mutation-free rejection; human review checks that the behavior-preserving cleanup is genuinely
invisible in ordinary use.

Candidate source: `efde645345577f44e0d6b691f7ca27eb587c4b53`

Candidate tree: `ae1ddaebd75e740c48eafc0b9ef2ad07cd99378b`

Tailscale endpoint: `http://100.94.63.83:8080/`

Immutable snapshot: `/tmp/geosolve-m73-uat.5EhWNL`

Ordered-manifest aggregate:
`371596d68a75ce4415970d3237f0511426958918b55b1376fc44700735ba2095`

The complete clean release gate passes. All seven files and `/` were fetched with proxy/cache
bypass and identity encoding, returned HTTP 200 and matched the frozen snapshot byte-for-byte.

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

- M73-U1: pending human review on the qualified candidate above.
- M73-U2: pending human review on the qualified candidate above.
- M73-U3: pending human review on the qualified candidate above.
- Final M73 approval: pending.
