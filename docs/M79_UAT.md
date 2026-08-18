<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M79 focused UAT — inference cycling and recovery

Status: **not started; frozen Tailscale candidate pending**.

Use only the exact source, immutable snapshot and Tailscale endpoint recorded here after clean
qualification. Findings must name the candidate and receive an M79 finding ID before replacement
work begins.

## U1 — exact reported reproduction

1. Draw a Center Rectangle with its centre snapped to Origin and a corner away from both axes.
2. Activate Midpoint Line and choose Origin as its centre.
3. Hover the midpoint of the rectangle's right edge.
4. Press Tab through every displayed candidate at least twice, including wraparound.

Every press must immediately update the guide/preview, must never leave a stale/dead state and must
continue to work without pointer movement. The candidate list and order must stay stable for that
stationary sample.

Return to the default `Midpoint + Horizontal` choice and place the line. Placement must succeed as
one Undo/Redo step. The retained scene must show the associative endpoint Midpoint relation and no
duplicate auto Horizontal relation; the line remains horizontal as a consequence of the accepted
geometry.

## U2 — movement and candidate refresh

At the same target, select a non-default candidate with Tab, move clearly away until snapping
disappears, then return. Normal candidates must immediately return and Tab must start from the
fresh ranked cohort. Repeat with two close points or midpoints so two same-position semantic
alternatives can be cycled A → B → A without either disappearing.

## U3 — modifiers and lifecycle

After choosing a candidate with Tab, exercise each transition before returning to the target:

- hold/release Ctrl or Cmd to suppress/restore inference;
- hold/release Shift on a recipe that supports regularization;
- Backspace, first/second Escape and a geometry-tool switch;
- Undo and Redo;
- pan, zoom and fit/reset camera;
- browser blur/focus and pointer leave/re-entry; and
- open/close an authoring overlay or otherwise transfer canvas ownership.

No old choice may leak into the new context. A stale click must never place another candidate
silently, and the next ordinary hover must recover without refresh/reload.

## U4 — queued movement and click truth

Move quickly between two different snap targets and press Tab before the pointer appears settled.
The selected guide must belong to the latest visible coordinate, never the previous target. Click
the unchanged stationary target and confirm the displayed choice is consumed exactly once. Move or
change tools before another click and confirm that choice is no longer active.

## U5 — adjacent candidate families

Spot-check candidate cycling for persistent points, line/polyline midpoints, point-on-curve,
Origin/axes, semantic centres, horizontal/vertical, parallel/perpendicular/collinear and a
two-reference Cartesian intersection. All compatible ranked alternatives remain cycleable; tied
alternatives require an explicit choice rather than guessing. Resource-limited, suppressed or
genuinely stale states remain noncommittable.

## Acceptance record

Pending supervising-human result. GitHub Pages publication and M79 closure occur only after this
exact frozen candidate is explicitly accepted.
