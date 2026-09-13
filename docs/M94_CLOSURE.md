<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M94 acceptance and checkpoint audit — 2026-09-07

Historical milestone record. For current setup and qualification, see the
[documentation index](README.md) and [release guide](RELEASE_QUALIFICATION.md).
Local artifact names below identify archived evidence; they are not current preview locations.

**M94 is accepted and closed.** After reviewing the delivered canvas and performance corrections,
maintainer acceptance closed the milestone at its qualified scope. The final audit found no checkpoint
blocker. This records milestone-level acceptance of the delivered scope and its disclosed limits;
it does not invent an exhaustive human replay of every automated scenario.

## Delivered files, APIs and behavior

- `geosolve-sketch-render::drawing` adds finite `DrawFrame`, `DrawItem` and `compose_draw_frame`
  presentation output. `CanvasCamera` uses live CSS extents for drawing, Fit and mouse coordinates.
- The demo's transient bridge v2 feeds the retained TypeScript/PixiJS WebGL2 renderer in
  `canvas-scene.ts`, `canvas-renderer.ts`, `canvas-renderer-pixi.ts`,
  `canvas-renderer-geometry.ts` and `canvas-viewport.tsx`. Surrounding React UI remains native.
- Navigation reuses authenticated native scenes and GPU resources; ordered camera input and
  frame-only updates avoid unchanged document/UI publication. Ordinary point previews reuse
  that transport. Private immutable prepared edits and delegated checkpoint encoding remove
  redundant validation/serialization while retaining strict imported-state validation.
- Browser canvas witnesses, numeric/pixel lifecycle regressions, capture tools and licence
  notices support the replacement. F004 restores batched strokes and text after context loss
  during initial shader compilation.

No solver equation, residual, rank/DOF, tolerance, branch or hard/soft behavior changed. Rust
retains accepted geometry, scene, picking, annotations, constraints and history authority.
Semantic drag samples and exact terminal publication remain ordered. Persisted formats and
native SVG/PNG export contracts remain unchanged.

## Final audit

Reviewed the M94 change set from `37e3915` to `7727cbf`, focusing on the new drawing/bridge
boundaries and the subsequent aspect, navigation, drag and recovery corrections. Independent
native and frontend reviews found no correctness blocker:

- Native review checked scene-cache authentication/reprojection, ordered wheel clamps and
  invalid-tail atomicity, CSS/DPR alignment, resize cancellation and Fit containment. Point
  preview guards retain complete snapshots for errors, recovery and terminal events. Prepared
  capabilities preserve live compare-and-swap, history/allocator consistency, byte limits and
  canonical serialization; imported or mutable state still receives full validation.
- Frontend review checked immutable finite frame validation, stale publication rejection,
  input ordering, retained geometry/style comparison, disposal and context recovery. The
  version-bound Pixi repair is covered by actual line-interior and axis-text pixel assertions.
- Drawing and qualification review checked presentation-only provenance, whole-frame finite
  rejection, explicit paint layers, new canvas witness schemas and retained browser inventory.
  The final two released backplane screenshots were re-inspected: strokes, grid, axis labels,
  mount circles and routed geometry are present after the exact point gestures.
- The signed final gate and all 241 individual passing receipts authenticate. All 9,876
  referenced evidence files still match their hashes. The frozen manifest matches the signed
  browser preparation output; all 12 frozen/read-only and currently served files, plus `/`,
  byte-match. Changes after the qualified product are prose only.

No implementation change or additional test expansion was needed during this audit. Earlier
focused regressions and the integrated qualification remain the product evidence.

## Accepted product and qualification

| Identity | Accepted value |
| --- | --- |
| Product source | `7727cbfc35f64aad42023305601879b5d0c4f2b4` |
| Product tree | `019e2c4c1c5d0d1b9147d1d05ec3320174d807af` |
| Gate run | `20260907T161218-24b782dc` |
| Production size | 12 files, 27,708,655 bytes |
| Files SHA-256 | `b5f8554f8234e6674ac9b3939b8fba702d2484cb5321fd31fcdec97e8984d5c0` |

Exact executed qualification command:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260907T143802-fee2e639'
```

Passed **241/241 stages**, 69 fresh and 172 authenticated reused, in 30m33s. This covers
formatting, warnings-denied Clippy, native/headless tests, optimized WASM build/lifecycle,
Rustdocs, package/licence/size checks, mathematical oracle, browser and exclusive performance
obligations. The unaffected exclusive performance stage retained its authenticated original run.
All **271 golden rows** remain byte-identical at SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.
The complete **42-case browser inventory** is covered by 17 fresh catalog/sample prefixes and
41 fresh full workflows, including all 16 two-edit/history workflows and four canvas lifecycle
cases; no reused leaves, skipped cases, failures or retries.

Exact delivered-artifact verification command, already passed:

```bash
nix-shell shell.nix --run 'GEOSOLVE_CHROMIUM_PATH=$(command -v google-chrome) npm --prefix crates/geosolve-demo-web/frontend run verify:artifact -- --manifest geosolve-m94-f003-uat.qwhvb8yw/production.json --directory geosolve-m94-f003-uat.qwhvb8yw/geosolve-production --url ${PREVIEW_URL} --receipt target/m94/drag/tailscale-final.json'
```

This establishes exact local/HTTP bytes, MIME types and real-WASM readiness. Final production
drag measurements additionally use Chrome 151, RTX 3090/ANGLE Vulkan/WebGL2, 1440×900, DPR 1
and 2× supersampling. Original SVG comparisons cover 16 samples/35 states; aspect corrections
add wide/tall/fractional extents and DPR 2; final recovery tests verify actual strokes and text.

Closure commands:

```bash
python3 target/m94/closure/audit-evidence.py
./scripts/release-gate.sh --docs-only --since 7727cbf
git diff --check
```

The evidence audit passes in 11.53 seconds; its script and receipt remain under
`target/m94/closure/`. The docs-only check passes for 10 prose files and 20 added links in 1.27s
(`target/release-gate/docs/2657ed4d0d34421ea140abe87b262e75.json`); `git diff --check` also passes.
The docs-only check preserves the accepted product without rebuilding it or rerunning the full
gate. The nomination and frozen qualification receipts retain their
historical awaiting-acceptance status; this document supplies the subsequent user acceptance.

## Accepted limits

- Dense point-preview WASM medians improve from 55–56 ms to 30–37 ms; release including saving
  improves from 1.83 s to about 1.32 s. Dense editing still exceeds a 16.7 ms frame budget.
  Computed Fillet rail previews remain separately expensive at 180–288 ms. Further optimization
  of publication/materialization and scene work is future scope.
- Navigation improves substantially (field pan/zoom bridge medians about 220/208 ms to 18/17 ms),
  but the largest scenes still exceed 60 Hz end to end. Hiding point rows may conservatively
  force a fresh scene because cache authentication rejects the retained candidate.
- F004 repairs recovery from the observed external initial context-loss trigger. Its private
  Pixi cache seam deliberately supports 8.20.1 and fails visibly for incompatible internals.
  A future asynchronous adapter also needs command-wide serialization beyond the input queue.
- Desktop scope, minor raster/glow differences and the recorded conservative release reuse
  policy remain. Accepted M92 at port 18092 is unchanged. No public Pages deployment is part of
  this closeout, and no subsequent milestone starts automatically.

Detailed implementation, regressions and measurements remain in
[M94_IMPLEMENTATION.md](M94_IMPLEMENTATION.md),
[M94_NAVIGATION_OPTIMIZATION.md](M94_NAVIGATION_OPTIMIZATION.md) and
[M94_DRAG_OPTIMIZATION.md](M94_DRAG_OPTIMIZATION.md).
