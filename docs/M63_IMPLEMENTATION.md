<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M63 implementation — canvas constraint visualization

Status: implementation and mechanical qualification complete; human UAT pending.

## 1. Files and APIs

- `geosolve-constraint-editor` adds public typed scene annotations for constraint glyphs,
  linear/radial/angular dimensions, visibility, direct operands and hit geometry.
- `ConstraintEditor` retains typed exact-occurrence hover and separate geometry reveal context,
  emits `HoverChanged`, clears both on surface leave/cancel and supports annotation-first
  Select-mode pointer input with diagnostic context.
- The sole workbench consumes those DTOs as accessible SVG symbols, leaders, dimension geometry,
  values and related-operand highlighting.
- One workbench-owned text-free vector catalog supplies all nineteen accepted constraint glyphs,
  all eleven authoring intents and all five dimension actions; shared concepts reuse the same
  fragment rather than maintaining palette and canvas dialects.
- Three stable M63 scenario definitions and this milestone/UAT record are added. No persistence
  schema or solver API changes.

## 2. Mathematical behavior

No residual, equation, tolerance, rank, convergence or branch behavior changes. Annotation
positions are derived only from independently accepted finite geometry, persistent contacts and
existing dimension presentation values. The browser does not evaluate constraint equations.

## 3. Verification

Required final commands:

```text
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && env -u NO_COLOR trunk build --release'
```

All five commands passed on 2026-07-30. The first Trunk invocation was accidentally run in
parallel with the standalone WASM check and lost a transient optimized-artifact copy race;
rerunning the exact release build alone passed. No source or generated distribution input changed
between those invocations. Existing Cargo manifest `license`/`license-file` advisories remain
unchanged and non-failing.

## 4. Acceptance

Direct coverage owns complete active constraint/dimension projection across representative public
ordinary and advanced scenarios, contextual visibility, always-visible reference angles,
deterministic fan-out, hover transitions, annotation selection and presentation metadata.
Human acceptance remains pending in `docs/M63_UAT.md`.

Follow-up `M63-F001` removes tessellation-dependent radial placement. Full-circle radius and
diameter annotations use canonical parameter zero, while circular arcs use their bounded semantic
midpoint. Both are evaluated through the public accepted curve API. The focused headless
regression and editor/web qualification slice pass; human retest remains pending.

Follow-up `M63-F002` replaces nominal ring-slot fan-out with deterministic collision-checked
placement. Every final glyph center maintains at least 22 px separation from every earlier glyph,
including glyphs whose semantic origins are merely nearby. The rotating-square headless regression
checks all final pairs and confirms displaced glyphs retain leaders. Human retest remains pending.

Follow-up `M63-F003` made visible fan-out leaders contextual hover corridors, but human retest
showed that was insufficient when the pointer departed from another location on the geometry.
`M63-F004` supersedes it: the headless editor retains the last direct geometry-hover position,
builds bounded corridors from there to directly related annotations, chooses the nearest
overlapping corridor and clears immediately outside all corridors. The regression is explicitly
outside both geometry and leader hit tolerances. Human retest subsequently found this state model
insufficient.

Follow-up `M63-F005` separates three concepts that `M63-F004` conflated: geometry-owned reveal
context, bounded transit, and exact annotation proximity. `EditorHoverState` now publishes an
optional geometry context owner independently of `EditorHoverTarget`; glyph targets carry a
deterministic marker index. Leader and inter-icon corridors retain context but never claim icon
hover, directly related occurrences preserve the original context owner, and clicks still resolve
to the persistent constraint. SVG applies hover only to the matching glyph child rather than the
whole persistent annotation group. Direct regressions cover sibling visibility, transit without
hover, first-to-second icon traversal, blank exit, occurrence-specific multi-marker hover,
persistent selection and rendered child-level hover. Human retest remains pending.

Follow-up `M63-F006` audits and replaces the relevant icon surface. A dedicated `icons` module
owns distinct text-free SVG fragments for every `SceneConstraintGlyph`, maps the coarser
authoring intents onto matching shared concepts, and adds representative distance, length,
radius, diameter and angle symbols. The palette installs those vectors into accessible,
non-semantic icon hosts while retaining visible button labels. Specialized accepted-state
symbols distinguish generic contact, tangent direction, curve normal, equal curvature,
continuity and fillet. Selection/error states remain outline-based so circular icons do not turn
into filled blobs. Direct tests require complete unique catalogs, text-free markup and exact
palette/canvas reuse for shared concepts. Human visual review remains pending.

Follow-up `M63-F007` gives every line-relation occurrence an interior anchor. The presentation
previously selected the middle polyline array index; a line has two display samples, so integer
indexing selected its first endpoint. Horizontal, vertical, parallel, perpendicular, collinear and
equal-length relations now use the geometric midpoint of each line's accepted endpoints.
Curve/contact/radial anchors are unchanged. The parallel-relation regression requires one
undisplaced marker at each related line midpoint. Human visual review remains pending.

Follow-up `M63-F008` specializes line-line perpendicular presentation. Public
`SceneAnnotationGeometry::RightAngle` carries the finite supporting-line vertex and three square
corner points needed for rendering, exact hit testing, contextual corridors and targeted problem
placement. The 12 px square chooses rays into a line span when the intersection is at one of its
endpoints; otherwise the persistent directed spans deterministically choose the quadrant. Its
corner is reserved before ordinary glyph fan-out so dense junction symbols do not cover it. If
the supporting-line intersection lies outside the viewport margin, presentation falls back to the
compact perpendicular midpoint glyphs instead of drawing a geometrically false corner. A
curve-contact Normal remains a separate contact-local glyph. Direct headless and workbench tests
cover exact geometry, selection, SVG output and the rotating-square density fixture. Human visual
review remains pending.

## 5. Known limitations

- Annotation layout is intentionally compact and deterministic, not a general-purpose CAD
  drafting-layout optimizer.
- M63 does not add manual dimension-label dragging or persisted annotation placement.
- Desktop-only workbench and existing solver/branch scope remain unchanged.
