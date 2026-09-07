<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M95 connected navigation — qualified candidate

**Implemented and qualified; supervising-user acceptance is pending.** Review the frozen candidate
at **http://100.94.63.83:18100/**. Accepted M94 remains at port 18096, and M92 at port 18092.
No Pages publication or subsequent milestone is included.

## Delivered files, APIs and behavior

- `geosolve-constraint-editor/src/intent_editor.rs` adds accepted-output navigation selection,
  exact-binding resolution, reverse ownership and visible declaration ownership APIs. Existing
  singular declaration/mutation semantics remain separate. Owner tests live in the existing M83
  editor, Fillet and Offset suites.
- `workbench/code_projects/navigation.rs` indexes authenticated accepted statements and exact
  generated outputs. `workbench/bridge/navigation.rs` caches that ownership, projects selected and
  partial rows plus multiple source ranges, and publishes transient selection deltas. Instance,
  source and scene authority reject stale requests; exact UTF-8 boundaries are validated.
- Frontend `App.tsx`, `side-panels.tsx`, `code-editor.tsx`, `source-navigation.ts` and
  `wasm-adapter.ts` connect canvas, Explorer and CodeMirror. Source decorations reveal without
  moving cursor or focus. Ordinary selection preserves Design/Split/Code; explicit Show in canvas
  and Show in code open Split only when needed. Fit stays separate. Shift/Ctrl/Meta toggles,
  groups, exact generated members, hidden/suppressed rows and multi-selection retain honest owners.
- Dirty source clears outdated links while accepted row browsing remains available. Apply,
  Undo/Redo, deletion and replacement reconcile ownership. Active tools and captures guard
  navigation. Selection performs no compilation, solve, checkpoint encoding or automatic save.
- `bridge.rs` preserves selection through rejected semantic drag rollback and resolves dense
  hidden-row ownership from one accepted projection per batch. Browser workflow tests and the
  release inventory cover the new behavior and the existing surrounding interactions.

No solver equations, residuals, tolerances, hard/soft priorities, rank/DOF rules, branch choices,
primitive families, canonical persistence formats or golden rows change. Rust remains the accepted
geometry and interaction authority. [The implementation ledger](M95_IMPLEMENTATION.md) records
M95-F001–F005, focused reproductions, harness corrections and failed/interrupted attempts.

## Exact product and qualification

| Identity | Value |
| --- | --- |
| Product source | `f18ff9ebeff2a0dcf6697aae43ade12dc5a05f21` |
| Product tree | `7e6acedd231d42685ef8b916ced8dc9f44dd27cb` |
| Gate run | `20260907T193542-d9094458` |
| Frozen manifest | `/tmp/geosolve-m95-uat.zkdiegw4/production.json` |
| Frozen directory | `/tmp/geosolve-m95-uat.zkdiegw4/geosolve-production` |
| Artifact | 12 files, 27,944,519 bytes |
| Files SHA-256 | `43fce481d4966b1694192a1301061b93286d3c8f2b2e3eae152c8150fd772545` |
| Manifest SHA-256 | `3a3bc70d46fa2844d9fbc59e56345eba24f0c75f53944719ce1a941d1660ad7d` |

Executed nomination:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260907T184559-a60cf7cb'
python3 target/m95/audit-qualification.py 20260907T193542-d9094458
```

**241/241 stages pass in 25m54s**, with 24 fresh stages and 217 authenticated reused successes.
This includes formatting, warnings-denied Clippy, native/headless suites, optimized WASM lifecycle
and parity, golden compatibility, Rustdocs, builds, package/licence checks, browser coverage,
artifact transport and isolated performance. The fresh workbench suite passes 323 tests with one
existing ignored test. Frontend preflight passes 181 tests. All **271 golden rows** remain identical
at SHA-256 `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.

The complete **45-case browser inventory** has 17 fresh catalog/sample-open checks and 44 fresh
full workflows, with no failures, skips, retries or reused leaves. This includes all 16 sample
edit/history workflows, all three M95 workflows and existing canvas/pointer lifecycle checks.
The audit authenticates every stage receipt and hashes all **9,884** referenced evidence files;
it passes in 6.55 seconds. `target/m95/qualification-audit.json` records that independent audit.

The exact production files were copied without rebuilding, made read-only, served on localhost
and Tailscale, then verified with:

```bash
GEOSOLVE_CHROMIUM_PATH=/home/arduano/.nix-profile/bin/google-chrome \
  npm --prefix crates/geosolve-demo-web/frontend run verify:artifact -- \
  --manifest /tmp/geosolve-m95-uat.zkdiegw4/production.json \
  --directory /tmp/geosolve-m95-uat.zkdiegw4/geosolve-production \
  --url http://100.94.63.83:18100/ \
  --receipt /home/arduano/programming/geometric-constraint-solver/target/m95/tailscale-final.json
```

This passes exact local/HTTP byte, MIME, base-path and actual-WASM readiness checks.
`target/m95/nomination.json` binds the candidate to its original signed preparation and frozen copy.
The service is PID 1937583, launched by `target/m95/serve-frozen.mjs` with authenticated routes only.

## Final performance and visual evidence

The final production navigation probe uses Chromium 151, RTX 3090/ANGLE Vulkan/WebGL2,
1440×900 and DPR 1. Both dense samples preserve exact persistence, report no page errors and
render zero idle frames. The selected-segment screenshot shows actual amber strokes, both
Explorer rows and both source expressions; the generated harness screenshot shows the exact
`entry / curve` Inspector and `powerHarness` invocation. Cursor/focus/layout and exact saved bytes
are independent browser assertions.

Twelve alternating direct-WASM generated-member/invocation pairs measure **14.9/8.5 ms median**,
with 1.4 ms median decoding and roughly 457/481 thousand UTF-16 characters. M94's former Explorer
route measures 40.4/39.8 ms and about 1.069 million characters, while lacking the new canvas-output
selection. The richer behavior and differing payloads are intentional; these are bridge phases,
not end-to-end frame-rate claims. Complete persistence remains identical.

The final navigation sequence measures field wheel/pan/empty-hover bridge medians of
**18.4/19.4/6.7 ms**, and harness medians of **7.0/5.7/3.2 ms**. The earlier M94 baseline is
19.3/19.1/4.7 ms and 6.7/5.6/3.6 ms respectively. A second fresh M94 sequence is
19.1/18.6/5.4 ms and 6.1/11.3/5.4 ms, illustrating run-to-run variation. Field empty-hover is
1.3–2.0 ms slower in this capture; M95's idle hover remains on the unchanged frame-only path.
These samples support preserved pan/zoom performance, not a universal speedup or 60 Hz claim.
Final field wheel/pan rendering medians are 22.8/17.3 ms; harness medians are 8.5/6.7 ms.

A fresh paired point-drag probe retains the same six ordered moves and terminals:

| Harness point | M94 preview median | M95 preview median | M94 terminal | M95 terminal |
| --- | ---: | ---: | ---: | ---: |
| Mount centre | 32.0 ms | 30.0 ms | 1056.1 ms | 1054.2 ms |
| Power-bus source | 28.4 ms | 27.5 ms | 1021.1 ms | 977.6 ms |

Both retain unchanged managed source, accepted movement and no Problems. Terminal measurements
are direct synchronous WASM calls; separate browser saving is excluded. The full browser suite
and native regressions separately cover history, cancellation and publication behavior.

M95-F005's final bytes match the focused repaired artifact: direct Isolate falls from 139.9 seconds
to 0.76 seconds, and isolated Fit from 82.1 seconds to 0.52 seconds. Visible item counts and full
persistence match. The original 360-second field browser limit remains intact and the integrated
workflow passes. These single visibility captures demonstrate removal of repeated projection work;
source compilation and complete dense history workflows remain expensive.

Executed probes use `NAV_MANIFEST`, `NAV_URL`, `NAV_OUTPUT` with
`node target/m94/navigation/probe.mjs` and `node target/m95/selection-probe.mjs`; point drags use
`DRAG_MANIFEST`, `DRAG_URL`, `DRAG_OUTPUT` with `node target/m94/drag/direct-probe.mjs`.
Final M95 inputs are the frozen manifest above and `http://127.0.0.1:18100/`; M94 inputs are
`/tmp/geosolve-m94-f003-uat.qwhvb8yw/production.json` and `http://127.0.0.1:18096/`.
Reports and screenshots remain under `target/m95/performance-final`, `performance-final-drag`,
`performance-m94-drag` and `performance-m94-final`, with named logs under `target/m95/`.

## Acceptance disposition and limits

All mechanical M95 criteria pass: exact accepted ownership, source/Explorer/canvas agreement,
truthful multi-owner Inspector, modifier/group/generated/hidden behavior, stale/dirty/Unicode
handling, tool/capture guards, history/replacement reconciliation, saved-byte preservation and
qualified canvas performance. No implementation blocker remains. Supervising-user acceptance is
required to close the milestone; it has not been inferred from automated or visual agent review.

Source sites cover compiler-authenticated builder expressions, not arbitrary TypeScript references
or the preceding `const name =` prefix. Selecting a full declaration line intersects its expression;
a cursor only on the prefix is unmatched. Dense editing remains above a 16.7 ms frame budget,
and full source compilation/history restoration still costs substantially more than navigation.
Ordinary selection preserves the current layout and camera, so explicit Fit remains necessary
when selected geometry lies outside the current view.

Documentation-only checkpoint commands are `./scripts/release-gate.sh --docs-only --since f18ff9e`
and `git diff --check`. They pass for eight prose files and eight added links; the qualified
product bytes remain unchanged and no solver/browser gate is repeated for this documentation.
