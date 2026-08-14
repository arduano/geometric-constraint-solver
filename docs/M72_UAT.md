<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M72 focused UAT — Public workbench bulk fixes

Status: implementation, clean qualification, public GitHub Pages deployment, exact hosted-artifact
byte verification and local/public Chromium preflight pass. The workbench is live at
`https://arduano.github.io/geometric-constraint-solver/`. Explicit supervising-human approval is
the only remaining milestone gate.

Direct Rust/WASM tests are authoritative for accepted geometry, residuals, rank/DOF, branches,
history and problem ownership. Human UAT should focus on feel, discoverability and browser
presentation.

Mechanical release authority is clean source `dc09b019704fe4a5cd48aff1ae838dfa52f36813`, tree
`38d79f5e05cb5274cc7eeb6bc6c0c2fac7d6f624`. Its complete gate log and SHA-256 are recorded in
`docs/M72_IMPLEMENTATION.md`; the unchanged golden passes **234/234**. The full-history Gitleaks
report is empty. Deployment source `6eb2c63f6349851e70200570c9c2db07631acd3a` passed corrected
run `31802816639` attempt 2, including the unchanged 180-second sparse ceiling in `176.27s`.
Artifact `9221899077`, all public HTTP responses and bytes, the WASM media type, both desktop
Chromium sizes and browser-local reload persistence pass mechanically. Human review should now
score presentation and interaction feel through M72-U1 through M72-U4 below.

## M72-U1 — Problems recover and stay current

1. Open an editable sample and create an invalid/rejected operation that shows Problems.
2. Undo to the last accepted state.
3. Confirm the old global message disappears without refreshing the page.
4. Redo the rejected operation and confirm its current problem returns; Undo again and confirm it
   clears.
5. Close Problems with its `×`. Confirm the canvas/tree evidence is unchanged. Cause a different
   current problem and confirm the card opens automatically.

Pass when no message from an older attempt survives recovery, while a genuine current failure is
never hidden merely because a prior set was dismissed.

## M72-U2 — Interactive rectangle is free-size

1. Start a new sketch, choose Rectangle and draw an ordinary rectangle.
2. Confirm it is axis-aligned but has no automatic lock, width dimension or height dimension.
3. Drag a corner and confirm both size and position can change consistently with the four H/V
   relations.
4. Undo/Redo the resize, then Undo/Redo the construction.

Pass when the rectangle remains rectangular and editable with predictable history. This test does
not change the separate constrained rectangle macro API.

## M72-U3 — One consistent canvas options surface

Select each option-bearing tool at least once: Equal, Tangent, Continuity, Distance, Length,
Radius, Diameter, Angle, Fillet, Ellipse, Elliptical arc, Rational conic, Parabola, Hyperbola,
NURBS and the Construction display control.

Check that:

- exactly one panel appears at the bottom-left of the canvas, never clipped by the tool palette;
- only fields relevant to that exact family/subtype appear;
- valid settings remain available when returning to the tool during the same page session;
- Escape and `×` close the panel and return keyboard focus to the opener;
- clicking another ordinary control closes the panel while still performing that action;
- an invalid C2/conic/NURBS value does not block selecting or using an unrelated tool.

At a compact desktop window near `1024x720`, also check that a tall panel scrolls internally and
does not escape the canvas. Mobile/tablet layout is outside M72.

## M72-U4 — Public release and persistence

1. Open `https://arduano.github.io/geometric-constraint-solver/` in Chromium with a normal desktop
   profile.
2. Confirm the workbench loads without a blank screen or missing styles and that Source and License
   open the expected public repository pages.
3. Create or edit ordinary geometry, reload the page, and confirm the browser-local workspace
   returns.
4. Repeat one option-panel open/close and one rectangle resize on the public build.

Pass when the public site behaves like the qualified local build and the edited scene survives
reload.

## Approval record

- M72-U1: pending supervising-human review.
- M72-U2: pending supervising-human review.
- M72-U3: pending supervising-human review.
- M72-U4: mechanical public deployment/browser preflight passed; pending supervising-human review.
- Final M72 approval: pending.
