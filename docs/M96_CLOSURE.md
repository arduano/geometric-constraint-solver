<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M96 acceptance and fresh-session handoff — 2026-09-08

**M96 is accepted and closed.** The supervising user stated “sounds good yeah, close the
milestone please” after reviewing the completed 12 mm manifold amendment. This accepts the
delivered layout and documented limits; it does not assert an unrecorded exhaustive human
replay. No further milestone is scoped by this closeout.

## Accepted checkpoint

| Item | Value |
| --- | --- |
| Branch | `m92/integration` (inherited name; accepted through M96) |
| Product source | `d77228559f9b860ce69cc03ceea6a5d34d8a3660` |
| Product tree | `bb221d35892a82d0bd4a89d63c190fb3c1ce381f` |
| Complete clean-source gate | `20260908T090848-ddc447b8` |
| Accepted endpoint | `http://100.94.63.83:18101/` |
| Frozen manifest | `target/m96/preview-20260908T010723-6a0a9164/production.json` |
| Frozen files | `target/m96/preview-20260908T010723-6a0a9164/geosolve-production` |
| Artifact | 12 files, 28,117,782 bytes |
| Files SHA-256 | `ac6725a875bee99d3536ec239f519165a82c35cab7312c1a3f26e6c163552dc1` |
| Manifest SHA-256 | `802e70ec15c4ad5aa2d0d8f08e99bf38282d682097dc0830f46082b09e0b90df` |

The closure commit contains prose only; `git log -1` identifies that documentation descendant.
Its checks preserve the qualified product and frozen bytes. The primary worktree remains the
continuation point. No branch rename, history rewrite, merge, push or Pages publication is part
of this closeout.

## Delivered files, API and geometry

The patch-private `p.computed.polylineChannel` recipe accepts a keyed polyline, width,
centreline bend radius and `caps: "both" | "end" | "none"`. Rust lowers it to two native
offset walls, computed corner Fillets and tangent semicircular end caps. Its six fixed outputs
are `left`, `right`, `startLeft`, `startRight`, `endLeft` and `endRight`. The TypeScript authoring
API, declaration catalog and generated frontend declarations share that output shape.

`crates/geosolve-sketch-code/src/expansion/channel.rs` owns the recipe, and
`composition/channel_validation.rs` checks final accepted line/arc boundaries during cold,
incremental and restored publication. The manifold folder contains reusable
`water-channel.patch.ts`, `point-to-point-channel.patch.ts` and `silicone-groove.patch.ts`
definitions with authenticated artifacts. Generated-wall navigation includes native straight
spans and computed corner arcs. [M96_IMPLEMENTATION.md](M96_IMPLEMENTATION.md) lists the
remaining owner files and focused commands; [M96_GOALS.md](M96_GOALS.md) defines the input contract.

The sample has three 12 mm channels joined to the reservoir, a separate 12 mm stair passage
with two bends and two port bores, and a closed 2.4 mm silicone groove. R8 water centreline
bends produce R2/R14 walls and R6 caps; the groove has R3.8/R6.2 walls. Independent tests check
areas of 10794.42469 mm² for the shared reservoir/circuit, 1230.69023 mm² for the stair, and
1515.39822 mm² for the groove, plus 2.8 mm water/groove and 8 mm stair/shared-circuit clearance.
The original reservoir/outlet dimensions remain editable through Undo, Redo and reload.

M96-F001–F006 regress stable keyed aliases, representable Fillet polishing, boundary
intersections, generated-wall selection, solved-geometry validation and large fully constrained
priority components. No solver equation, primitive family, unsafe code or FFI is added.
Branch state and independent final residual validation remain authoritative.

## Qualification and closure checks

Clean-source run `20260908T090848-ddc447b8` passes **243/243** stages with
3 fresh and 240 authenticated reused successes
(2m18.54s). Its signed manifest records `clean_source: true`,
`source_unchanged: true` and `qualified_release: true`. Coverage includes format,
warnings-denied Clippy, native/headless tests, optimized WASM, generated packages, Rustdocs,
licence checks, performance, all 271 unchanged golden cases and the complete 45-case browser
inventory. Fresh execution and original reused evidence remain distinguished in the manifest.

The three fresh stages are inventory, licences and artifact transport. The remaining
240 successes retain their authenticated original execution evidence.

The earlier complete run `20260908T012505-e9d6c999` remains provisional dirty-source evidence.
The clean run independently authenticates eligible successes and the frozen artifact's
preparation; no provisional receipt is relabelled as a clean run. The closure audit report
`target/m96/clean-qualification-audit.json` authenticates the final stage receipts,
9,892 evidence files and 224 output roots. Fresh verification passes all 13 HTTP routes
(12 files plus `/`), exact bytes/MIME and actual-WASM manifold readiness with 182 geometry
items and no browser errors. `target/m96/closure-artifact-verification.json` records that
check; `target/m96/closure-artifact-binding.json` binds every frozen file to the clean
run's authenticated browser preparation.

Executed qualification and closeout commands:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260908T012505-e9d6c999'
python3 target/m96/audit-clean-qualification.py 20260908T090848-ddc447b8
nix-shell shell.nix --run 'GEOSOLVE_CHROMIUM_PATH=$(command -v google-chrome) node crates/geosolve-demo-web/frontend/scripts/verify-artifact.mjs --manifest /home/arduano/programming/geometric-constraint-solver/target/m96/preview-20260908T010723-6a0a9164/production.json --directory /home/arduano/programming/geometric-constraint-solver/target/m96/preview-20260908T010723-6a0a9164/geosolve-production --url http://100.94.63.83:18101/ --receipt /home/arduano/programming/geometric-constraint-solver/target/m96/closure-artifact-verification.json'
python3 target/m96/audit-closure-artifact.py 20260908T090848-ddc447b8
./scripts/release-gate.sh --docs-only --since d77228559f9b860ce69cc03ceea6a5d34d8a3660
git diff --check
git status --porcelain=v1 --untracked-files=all
```

The prose input/link check passes for eight files and 23 added links; `git diff --check`
passes. The post-commit status check establishes a clean worktree. Under
[RELEASE_QUALIFICATION.md](RELEASE_QUALIFICATION.md), the prose closeout preserves the product
evidence and needs no rebuild or repeat solver/browser qualification.

## Accepted limits and continuation

- This is a planar channel layout. Depth, flow, pressure, seal compression and solid Boolean
  union are outside its scope; no manufacturing or hydraulic qualification is claimed.
- The helper accepts straight-span keyed polylines. It rejects nonfinite lengths, nonpositive
  width, bend radius at most half the width, zero-length/collinear/reversing or unresolved
  corners, Fillets that do not fit and invalid composed boundaries. Curved input spans and
  automatic path cleanup are unsupported. The validator permits at most 512 composed edges,
  with existing managed-source and expansion limits also enforced.
- Inter-channel, seal, port, plate and screw clearance is an independently checked sample
  property. The recipe does not provide generic multi-channel Boolean or clearance operations.
- M96-F006 adds a rank factorization for large square/tall hard components before preferences.
  It preserves existing rank policy and actual free motion; a general speedup for rank-deficient
  models is not claimed. The existing active-bound positive-cost control remains fail-closed.

No M96 implementation blocker or acceptance action remains. Read this handoff, [START_HERE.md](../START_HERE.md),
[PLAN.md](../PLAN.md), [ARCHITECTURE.md](../ARCHITECTURE.md), [ACCEPTANCE.md](../ACCEPTANCE.md)
and [SCENARIOS.md](SCENARIOS.md) before further implementation, following `AGENTS.md`.
The previous M95 checkpoint remains documented in [M95_CLOSURE.md](M95_CLOSURE.md).

Local receipts, screenshots and the frozen product live under ignored `target/m96/`;
authenticated gate evidence lives under `target/release-gate/`. Preserve both for continuity.
If an older saved workspace is open, load a fresh **PC liquid-cooling manifold** sample.
Frozen local files and serving processes are not guaranteed to survive cleanup or reboot.
Moving or restarting identical artifact bytes requires the bounded transport/readiness check
from [RELEASE_QUALIFICATION.md](RELEASE_QUALIFICATION.md); do not rebuild and describe new bytes
as the same accepted artifact. Restart the preserved files, if needed, with
`node target/m96/serve-preview.mjs` after checking listener ownership. The launcher reads
`target/m96/preview-location.json`. No build, test or release-gate process is left running
by this closeout; the accepted preview service remains available.
