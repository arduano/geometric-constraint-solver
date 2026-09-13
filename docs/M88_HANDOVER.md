<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M88 workflow-led workbench redesign: closure summary

M88 was accepted and closed on 2026-09-01. Milestone-level acceptance includes
M88-U1 through M88-U10 without claiming a separately logged row-by-row replay.
The React workbench and instance-scoped `WorkbenchHandle`/`WorkbenchBridge` became
the sole browser presentation path; the obsolete Rust-DOM host was removed.

Rust retains semantic authority. React owns browser presentation and platform work.
Detailed chronology and qualification remain in [implementation](M88_IMPLEMENTATION.md),
with [audit](M88_AUDIT.md), [goals](M88_GOALS.md) and [UAT](M88_UAT.md) as references.

## M88-F001 disposition

Initial UAT deterministically exposed `IntentBootstrapMetadata` twice because the bridge serialized
`IntentGraphNodeKind` using Rust `Debug` for Explorer and Inspector. The bridge now reuses semantic
`node_family_label`; Explorer omits the type/detail suffix entirely, Inspector keeps one compact
semantic kind, and Explorer row/group icons are non-shrinking. The exact regression initially
failed and now passes. Real release-WASM E2E rejects `IntentBootstrapMetadata` and `Bootstrap {`
and asserts the Explorer row/icon/Inspector contract. All UAT rows were still pending at this
historical F001 checkpoint.

## M88-F002 disposition

UAT next found that the migrated rail had lost recognizable CAD icons and semantic grouping, while
a scissors-like bucket combined Fillet/Offset with unrelated display actions. The current rail is
Select, Sketch, Constraint, Dimension and Modify with exact 25/13/5/2 inventories. Modify contains
only Fillet and Offset; Profile/Construction is contextual; Grid/Fit/Origin are canvas-local. A
bounded one-time Rust catalog owns command/icon identity and is absent from hot snapshots. View and
role commands preserve the active authoring tool. Browser qualification also covers long labels,
bounded scrolling, outside dismissal, destination-click routing, focus restoration and active
category state at both required desktop sizes. All human rows were still pending at this
historical F002 checkpoint.

## M88-F003 disposition

The frozen F002 release-WASM candidate independently reproduced ordinary point-and-click authoring
failure for Segment and Center–Radius Circle. Snapshot
`geosolve-m88-react-uat.kdSCUU`, WASM SHA-256
`7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0`, was built from the shared
tree based on `71a51ee534f034e2328a07e0d80f9a9ee5e0fc62`. Direct bridge click-click authoring already
worked. The React viewport unconditionally canceled `lostpointercapture`, although browsers
normally emit it after `pointerup`; the bridge cancellation path independently discarded the
editor's `ClearConstructionPreview` effect.

The viewport now remembers the exact pointer and retires ownership synchronously on `pointerup`
before awaiting the adapter. The expected later capture loss is stale and harmless; genuine
`pointercancel`/capture loss cancels exactly once. Bridge cancellation now dispatches every returned
effect. React normal-release/genuine-loss, direct bridge Segment/Circle parity, direct cancellation-
preview cleanup and real release-WASM regressions pass. Final F003 proportional totals were bridge
`19/19`, full demo-web `391/391`, frontend `19/19` and Playwright `8/8`; `npm run check`, focused
all-feature demo-web Clippy, formatting, locked WASM and distribution validation passed. Immutable
F003 snapshot `geosolve-m88-react-uat.QkVU1k`, aggregate
`e44bd8c22ccb67e542a8c73b58f62ed8ab236ab728b2d1bc2ae1aff1895ac167`, remains preserved after
F004 replaced its service. All human rows were still pending at this historical F003 checkpoint.

## Frozen UX target

- Compact File/project app bar with Undo/Redo, true title/status, Design/Split/Code and Export;
  repro/trace under Diagnostics.
- Narrow Select/Sketch/Constraint/Dimension/Modify rail with last-used state and click/keyboard
  submenus for every current variant.
- One collapsible Explorer instead of simultaneous tree and Design hierarchy.
- Design canvas, resizable Split and first-class central Code modes.
- Right Inspector, Parameters and Problems tabs in Design/Split; the latter two rehost their one
  logical state into Code mode rather than creating duplicate views.
- Sticky Apply/Revert/status, at least 12 px source text, at least `720 x 500` CSS px of editor at
  `1024 x 720`, source tabs and secondary Parameters, Problems, Generated and Artifacts surfaces.
- Searchable New/Start from code/sample/import/recents surface covering all 37 samples.
- Exact managed-owner Open in code and canonical `project.json`/`sketch.ts`
  browser-to-headless-to-browser handoff.

## Historical performance baseline to retain

Gridfinity remains a valid 62x62 full-rank, zero-DoF system with no `FixedPoint` and one Y
`FixedCoordinate`. The actual WASM open/edit contract passes `4/4` together with the final
one-MiB native proxy regression. Five independent Chrome 151 processes measured cold opens at
`1770.1, 1795.7, 1561.9, 1455.7, 1551.1` ms (median/max `1561.9/1795.7`) and exact-CAS edits at
`1314.3, 928.8, 897.1, 871.0, 855.0` ms (median/max `897.1/1314.3`). Open/edit independently
validated maximum normalized Hard residuals were `2.8866e-15` and `4.8486e-12`; all accepted
geometry was finite and no browser error, trap or crash occurred. Native rank/DoF owner evidence,
not browser DOM, owns the full-rank/zero-DoF claim.

## Boundaries and cautions

- Presentation preferences never enter canonical project, repro, solver, accepted-scene or history
  authority. Resizing and mode changes do no semantic work.
- Invalid code retains prior accepted paint and one outer transaction boundary.
- Canonical project export returns a typed refusal while any unapplied draft exists; raw draft-
  source download remains separate and claims no accepted-project authority.
- Do not execute custom TypeScript in Rust/WASM or add an AI chat/stateful service.
- Do not build a general IDE, add new sketch mathematics or weaken independent residual checks.
- Do not restore LOD or hide accepted geometry for performance.

## M88-F004 disposition and current identity

F003 review found unconditional geometry-role controls, feedback that shifted the
layout and Finish enabled without a completable draft. Rust now publishes exact
Finish and Undo/Redo readiness. Geometry roles are contextual, feedback is an overlay,
and Enter shares the Finish gate. One-shot preselection returns to Select; camera
and first-Escape cancellation preserve the collector; Code-only mode omits canvas
tools; Ctrl-O/Ctrl-S perform file actions; dirty drafts prevent replacement; secondary
click remains browser input.

The accepted F004 artifact aggregate was
`700ebae4aec13ce20ab8786b63254e2c5b6239204c38bdc6e9d35f11c4159071`, with WASM SHA-256
`22944f00ddf8c327e943d055a8224a1948efce42ec2896c9326891a45bfdf2ff`.
Build, frozen-artifact and served manifests matched. Real-browser checks proved
right-click neutrality, exact Polyline Finish transitions and zero runtime errors.
Qualification passed bridge 23/23, demo 395/395, frontend 26/26 and Playwright 10/10.

After removal of the Rust-DOM compatibility host, qualification passed demo 344/344,
actual WASM 2/2, frontend 26/26, optimized Playwright 10/10, the unchanged 271-row
golden and the complete dirty-tree release gate. That rebuilt distribution is
qualification evidence; it did not replace the previously accepted F004 bytes.

## Superseded candidates

F001, F002 and F003 candidates are historical reproduction evidence. F002's WASM
SHA-256 `7a3376f1e0895dd7773ec2eabaa704414eac9263f7b05bc7b611b9b0ef6bd7c0` identifies
the normal-pointer-capture failure described above. No old preview address is a
current deployment contract. Use the [documentation index](README.md) for current setup.
