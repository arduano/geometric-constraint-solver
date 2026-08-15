<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M72 focused UAT — Public workbench bulk fixes

Status: **complete and explicitly approved on 2026-08-15**. Implementation, clean qualification,
final public GitHub Pages deployment, exact hosted-artifact byte verification and local/public
Chromium qualification pass. The supervising caller approved the recorded focused UAT scope
against accepted follow-up commit `b700313` and requested milestone closure.

Direct Rust/WASM tests are authoritative for accepted geometry, residuals, rank/DOF, branches,
history and problem ownership. Human UAT should focus on feel, discoverability and browser
presentation.

Historical initial mechanical release authority was clean source
`dc09b019704fe4a5cd48aff1ae838dfa52f36813`, tree
`38d79f5e05cb5274cc7eeb6bc6c0c2fac7d6f624`. Its complete gate log and SHA-256 are recorded in
`docs/M72_IMPLEMENTATION.md`; the unchanged golden passes **234/234**. The full-history Gitleaks
report is empty. Initial deployment source `6eb2c63f6349851e70200570c9c2db07631acd3a`
passed corrected run `31802816639` attempt 2, including the unchanged 180-second sparse ceiling in
`176.27s`. Artifact `9221899077`, all initial public HTTP responses and bytes, the WASM media type,
both desktop Chromium sizes and browser-local reload persistence passed mechanically.

The accepted follow-up passed the same two-size Chromium automation from its immutable Tailscale
release snapshot before human approval. Final product source
`b7003137960afb1b9d29c990d595df44bcd7c2d4` then passed the complete local release gate. Its
documentation-only approval descendant `2d1513912787445ff825836705158c2b563dc7ff` passed Pages run
`31862218764`, which deployed artifact `9241248173`. All seven final public files return 200,
byte-match that artifact and use the expected JavaScript, WASM and CSS media types. The full public
Chromium contract passes at `1440x900` and `1024x720`, including reload persistence.

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

- every option-bearing palette entry is one centered button with no separate chevron, and pressing
  that main button implicitly opens its options;
- exactly one panel appears at the bottom-left of the canvas, never clipped by the tool palette;
- only fields relevant to that exact family/subtype appear;
- valid settings remain available when returning to the tool during the same page session;
- pressing the same option-bearing button again leaves its panel open;
- blur, an outside or canvas click, zoom and ordinary non-tool controls leave the panel open while
  still performing their normal action;
- switching to another option-bearing tool replaces the panel, while switching to a tool without
  options closes it;
- Escape and `×` close the panel, activate Select and move keyboard focus to Select;
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
4. Repeat one option-panel persistence/close sequence and one rectangle resize on the public
   build.

Pass when the public site behaves like the qualified local build and the edited scene survives
reload.

## Approval record

- M72-U1: **Accepted** under the 2026-08-15 scoped close decision.
- M72-U2: **Accepted** under the 2026-08-15 scoped close decision.
- M72-U3: **Accepted** under the 2026-08-15 scoped close decision.
- M72-U4: **Accepted and mechanically complete** — final Pages artifact `9241248173` passes exact
  public byte/media verification and the two-size Chromium contract.
- Final M72 approval: **Pass** — explicitly approved by the supervising caller on 2026-08-15.

The supervising caller confirmed that the presented fixes resolve the reported behavior and
explicitly requested M72 closure. This accepts M72-U1 through M72-U4 for the recorded scope without
claiming a separate exhaustive replay of every scripted permutation; direct automated
qualification remains authoritative.
