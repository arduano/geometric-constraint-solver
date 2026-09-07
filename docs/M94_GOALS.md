<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M94 — accelerated canvas viewport

Status: **implementation qualified on 2026-09-07; awaiting supervising-user acceptance**. M93 remains accepted and closed.

## Outcome and ownership

Replace the live SVG sketch viewport with TypeScript, PixiJS 8 and WebGL2. Keep the accepted
appearance and complete editing behavior: geometry, grid, dimensions, labels, selection, handles,
previews and annotations draw on one persistent canvas. React popovers, menus, inspectors,
controls and toolbar icons remain native. Rust remains the sole geometry, scene, pick, constraint,
branch, history and accepted-state authority. No full-scene SVG parsing/rasterization or hidden
parallel DOM scene is an implementation path. Native SVG/PNG export APIs remain supported.

## Implementation sequence

1. Capture the immutable SVG baseline for all 16 samples and representative interaction states.
2. Add a finite typed draw-frame composer in geosolve-sketch-render, sharing existing presentation
   semantics and using EditorScene polylines, annotation geometry and exact control grips.
3. Replace the live frame SVG payload with the draw frame; update the transient bridge to v2.
   Preserve persisted projects, reproduction formats, and rejected-edit accepted-frame retention.
4. Install one retained GPU renderer with stable resources, on-demand RAF drawing, CSS/logical/DPR
   alignment, and native Rust input routing. Handle hide/resize/DPR/unmount/context loss explicitly.
5. Migrate browser geometry witnesses to typed presented-frame evidence plus actual canvas pixels;
   preserve complete source/workspace witnesses, numeric drag assertions and all test obligations.
6. Qualify the stable candidate using M93's proportional policy, review visuals, and deliver a
   separate candidate for supervising-user acceptance. Accepted M92 service remains untouched.

## Acceptance

- A real WebGL2 canvas draws every live viewport layer; native surrounding UI stays functional.
  No SVG geometry tree, silent SVG fallback, or duplicated domain/picking implementation remains.
- All 16 samples retain their intended appearance and complete two-edit/history workflows. Review
  side-by-side fitted captures plus hover, selection, drafting, dimension and zoom states. Small
  antialiasing/glow differences are permissible; missing information, clipping and pick drift are not.
- The camera uses the full canvas CSS width and height at any desktop aspect ratio, with uniform
  model scale and matching pointer coordinates. Resize preserves camera centre and zoom; Fit uses
  the current dimensions. Backing DPR never changes semantic coordinates. Hidden layouts, resize, DPR changes, lost capture, stale frames,
  context restoration and disposal receive targeted tests. GPU failure is a native viewport error;
  existing code/document controls remain usable.
- Accepted scene identity, finite geometry, independent residual validation, branches, history,
  all 271 mathematical golden rows and existing native exports remain intact.
- Browser receipts use a new canvas witness schema and reject old SVG evidence. A read-only
  observer exposes the last presented frame; required pixels are captured from the real canvas.
- Record hardware, browser, baseline/canvas paint and end-to-end interaction timings separately.
  Aim for smooth 60 Hz ordinary samples; disclose scale-lab results, GPU/software execution and
  unexplained regressions. Idle canvases must not redraw continuously.
- Preserve licence metadata, bundle ceilings and release inventories. Use focused owner checks
  during implementation; the stable candidate runs the integrated format/Clippy/native/WASM/
  golden/browser/artifact gate. Reuse only authenticated unaffected work after repairs.

## Boundaries and delivery

Desktop scope remains unchanged. WebGPU, renderer workers, new sketch features, sample expansion
and surrounding UI redesign are outside this milestone. Public draw-frame types are presentation
output, never accepted-scene or interaction input authority. Transient bridge v2 ships atomically
with its JS/WASM consumers; saved document and reproduction compatibility remains unchanged.
No publication, accepted-service replacement or milestone closure is inferred from implementation.
Exact implementation evidence and remaining work belong in [M94_IMPLEMENTATION.md](M94_IMPLEMENTATION.md).

## Authorized navigation optimization — 2026-09-07

The supervising user requested implementation of the measured dense-fixture optimizations,
prioritizing non-mutating pan, zoom and hover. M94-F002 follows the reproduced bridge and renderer
bottlenecks in [the diagnosis](M94_PERFORMANCE_DIAGNOSIS.md). Keep document/UI projections out of
camera updates, reproject authenticated retained scenes, retain GPU paths across exact
translations, and coalesce redundant hover/pan and ordered wheel samples before frame composition.
Rust still owns every camera operation, clamp, pick and annotation layout. Changes to native
publication/solver algorithms are a separate optimization concern.

Qualification compares direct WASM and actual GPU submission timings with the preserved baseline,
and exercises persistence/history preservation, picking after navigation, annotations/strokes,
resize/DPR and terminal/cancellation ordering. The stable candidate receives one integrated
qualification with authenticated unaffected reuse and exact replacement delivery at port 18096.
M94 remains open for supervising-user acceptance.
