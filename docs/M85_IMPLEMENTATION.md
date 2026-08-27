<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M85 implementation ledger — Responsive retained workbench presentation

Status: **mechanically qualified and nominated for hands-on UAT, but unaccepted**. Exact final
source `5c265e2` passes the clean release gate; its no-rebuild immutable candidate passes local and
Tailscale byte verification, the frozen-byte browser profile and native M85-U10 flat-adapter
evidence. Hands-on M85-U1-U9/U11-U12, explicit supervising-user approval and post-approval Pages
publication remain pending. `docs/M85_GOALS.md` owns the contract. Accepted M84 remains product and
public-byte authority.

## Exact nominated candidate

- Source: `5c265e211e20dabc8a27f6402d530f5d645ff15c`.
- Tree: `b55d012443f4dbf7551e30041da2912e666de9db`.
- Source state at qualification: clean.
- Product nomination: **mechanically nominated for hands-on UAT; not yet accepted**.
- Frozen distribution: `/tmp/geosolve-m85-uat.QX8fU3Q6`, directories/files `0555`/`0444`, seven
  regular files, zero symlinks, ordered-manifest aggregate
  `dc729ce5fa28929dba2aa086e49246aac7d5173a0583d3b0b64748ffbbb7fda5`.
- Nomination evidence: `/tmp/geosolve-m85-freeze-evidence.uj9HviX1`.
- Local endpoint: `http://127.0.0.1:18100/`, PID `2008536`, invocation
  `d9fdcccfa9ce46deafcf46f7b6148e6e`.
- Retained Tailscale endpoint: `http://100.94.63.83:8080/`, PID `2008538`, invocation
  `2c70402e2b2a45a5810ea29722f25f95`.
- Both services serve only the same immutable snapshot; their complete eight-path HTTP ledgers are
  byte-identical at SHA-256
  `305eccfc8fa60786aabfae59edd612e695ce3c15b7224abbf3be0d0852ae0d27`.
- GitHub Pages publication: **blocked until explicit supervising-user UAT approval**.

## Findings

### M85-F001 — Camera events rebuild the complete canvas

Disposition: **confirmed DEFECT; repaired, clean-qualified and frozen-profile qualified; hands-on
UAT remains pending**.

Reproduction authority is exact M84-F012 source `84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`,
tree `429ed56d2a5b3988d6604079d19e1002f9049d64` and immutable snapshot
`/tmp/geosolve-m84-f012-uat.nMOymIIM`. Open the PC Water Manifold with annotations visible, then
middle-button pan or issue a wheel burst. The original focused Chromium diagnosis recorded:

| Operation | Raw samples | Elapsed | Viewport child replacements | p95 frame gap |
|---|---:|---:|---:|---:|
| Pan, annotations visible | 30 | 14,933 ms | 33 | 583.3 ms |
| Wheel, annotations visible | 18 | 9,976 ms | 19 | 583.2 ms |
| Pan, annotations hidden diagnostic | 30 | 16,937 ms | 33 | 733.3 ms |

The first owner is `geosolve-demo-web` presentation. Projectional camera callbacks rebuilt
`ProjectionalEditorSession::scene`, screen-space curve tessellation, computed/annotation
presentation, full SVG text and `#wb-viewport.innerHTML` for each raw event. The flat compatibility
adapter performed the same class of reconstruction. This is not a solver equation or accepted-
scene defect and does not warrant golden-authoring expansion.

Repair slices are deliberately separate:

- `7de063e` adds authenticated retained-scene reprojection through public editor projections.
- `4b4b89b` adds independent newest-camera RAF queues, retained scene/grid/HUD transforms, retained
  hover presentation and the first actual browser work ledger for both adapters.
- `191a610` hardens exact pan terminals, current-camera hover admission and display-policy parity.
- `e9b4407` replaces event-inferred semantic counters with presentation-independent owner receipts
  and completes the optimized exact code-terminal path described under M85-F002.

Each camera RAF now admits exactly one lightweight retained-camera presentation and no exact
reprojection, scene composition, SVG serialization, viewport replacement, solver/preview, Intent
materialization, computed evaluation, native-history publication, code parse/expansion/publication,
persistence or durable-panel work. One separately authenticated exact reconciliation may run after
a completed burst; it is counted as exact reprojection rather than camera-RAF work and cannot run
per raw sample.

### M85-F002 — Unchanged-host code terminal overflows the ordinary stack

Disposition: **confirmed PANIC; repaired and covered by an ordinary default-stack exact
regression**.

The PC Water Manifold unchanged-host point-overlay path originally entered a normal nested Intent
transaction and then cleared its large Undo snapshot. Dropping that unnecessary snapshot on the
ordinary 2 MiB test stack terminated the process with `SIGABRT` after stack overflow. The owning
boundary is optional code/native composition plus the retained Intent coordinator; no solver
equation, branch or persistence format is implicated.

The repair adds a history-free accepted-authority fork and delegated patch plan/publication path.
An unchanged-host terminal expands once, reuses authenticated native Fillet/host owners, plans
against the history-free fork, cold-materializes and independently validates the result, and keeps
the outer code session as the sole user-visible history. Removing native history changes the exact
session digest, so the patch is built only after the fork. Changed-host edits retain the complete
cold/warm oracle path.

The exact regression remains on the ordinary test thread and verifies one expansion, zero managed
parse or code publication at the composition layer, all six native host identities, finite valid
accepted authority and empty delegated Undo/Redo:

```bash
cargo test --locked -p geosolve-sketch-code \
  --test m84_native_composition \
  m85_large_unchanged_host_overlay_is_default_stack_and_history_neutral \
  -- --exact --nocapture
```

It exits `0` in `55.45 s` at the final implementation checkpoint. This command is intentionally
distinct from the complete release gate, whose broad workspace test process uses
`RUST_MIN_STACK=16777216` and therefore cannot by itself prove ordinary-stack safety.

### M85-F003 — Incremental retained results exhaust the ordinary stack

Disposition: **confirmed PANIC; repaired without public API changes and covered by two exact 2 MiB
regressions plus the complete 300-test demo-web suite**.

The final default-stack audit first reproduced `SIGABRT` while a managed rectangle and its dependent
line were deleted together. Avoiding an audited wrapper around the non-audited result repaired that
empty-project case, but the complete suite then reproduced the same failure while deleting only the
independent line and preserving the constrained rectangle. The second case reached
`geosolve_sketch::compiler::compile_constraint` with the thread stack at its guard page. No
recursion, invalid geometry, solver equation or history corruption was involved.

The owning defect was aggregate stack pressure across structural projection, audited work, warm
native materialization and the large inline `ProjectionalEditorSession` retained by
`MaterializedCodeProject`. Experimental checkpoint `d2b7d38` proved that heap ownership removed
the pressure, but changed several public incremental-code return signatures and is superseded.
Final repair `fd2c560` restores every pre-M85 public signature. The unaudited adapter calls the
common receipt-aware worker directly rather than wrapping and immediately unwrapping its large
result, and the projectional editor privately heap-owns six optional Fillet/Offset preview and
gesture states. `ProjectionalEditorSession` shrinks from `35,488` to `15,296` bytes and
`MaterializedCodeProject` from `36,320` to `16,128`. No stack-size setting, solver equation,
accepted authority, persistence format or code-authoring semantic changed.

The two pre-existing adapter regressions jointly freeze partial deletion with exact Undo and
transitive delete-all with authenticated empty acceptance:

```bash
RUST_MIN_STACK=2097152 cargo test --locked -p geosolve-demo-web --lib \
  managed_canvas_deletion_ -- --nocapture
```

It exits `0`: 2 passed, 298 filtered out, in approximately `2.04 s`. The full default-stack adapter
suite also exits `0`: 300/300 in `98.61 s`. The unchanged-host PC Water Manifold M85-F002 sentinel
remains separate and passes after this repair in `55.45 s`.

## Implemented architecture

### I1 — Actual owner receipts and browser ledger

- [x] Public `InteractionWorkReceipt` counts native preview/exact-release attempts, cold Intent
  materialization attempts, computed-evaluation attempts and durable native-history publications.
  `AuditedInteraction<T>` retains crossed work on success, rejection and stale calls.
- [x] Optional `CodeWorkReceipt` independently counts managed parses, deterministic expansions and
  accepted outer code publications. Merely attaching code authority performs no code work.
- [x] The browser ledger composes those receipts with camera presentation, exact reprojection,
  retained hover, scene composition, SVG serialization, viewport replacement, persistence and
  durable-panel counters. It no longer predicts semantic work from event labels.
- [x] Projectional and flat point/curve previews, terminals and Fillet/Offset authoring routes issue
  audited receipts. Cancel, clear, stale and no-motion paths retain exact zero-work evidence where
  their owner boundary was not crossed.

### I2 — Retained camera and hover presentation

- [x] Projectional/code and flat adapters own independent RAF camera queues with newest absolute pan
  samples, ordered anchored wheel folding, stale-generation rejection and exact terminal camera
  reconciliation.
- [x] Raw camera frames transform retained accepted paint plus adaptive grid/HUD state without full
  scene/SVG/viewport work. Fit, Origin, toolbar zoom, wheel and middle-button pan share the same
  camera authority.
- [x] Retained hover updates stable SVG class/visibility state without scene composition,
  serialization or viewport replacement. Scene/view/display stamps reject stale hover admission.
- [x] Authenticated public reprojection updates native and computed Fillet geometry, datum and
  annotation placement transactionally and fails closed for malformed or stale derived state.

### I3 — Exact transient and terminal work

- [x] Pointer previews retain newest-sample RAF coalescing. They may perform only the native solve,
  Intent materialization and computed evaluation evidenced by their owner receipt, plus transient
  scene/SVG presentation; they cannot publish history/code, persist or rebuild durable panels.
- [x] Native mutating terminals publish one accepted native-history transaction. Code-owned
  terminals reuse the already accepted live editor, perform one audited expansion and one outer
  publication, retain a history-neutral delegated editor/cache and expose one user-visible Undo
  step.
- [x] Mutating terminals retain synchronous exact validation and once-only save/durable rendering;
  no-motion, cancel and stale terminals publish none.
- [x] A provisional five-test profile on an unpinned pre-F003 implementation ancestor passed the
  camera, hover, drag and terminal budgets. It remains historical directional evidence only.
- [x] The same five-test profile passes against the exact no-rebuild frozen final bytes with all
  camera, hover, drag, terminal, durability and delayed-motion budgets satisfied.

### I4 — API-compatible default-stack repair

- [x] The unaudited incremental-code adapter calls its receipt-aware worker directly, avoiding a
  redundant large audited-result wrapper while retaining identical receipts at audited call sites.
- [x] All pre-M85 public incremental-code return signatures remain unchanged; the public boxed-
  result experiment at `d2b7d38` is superseded rather than adopted.
- [x] Six optional projectional Fillet/Offset preview and gesture states are privately heap-owned.
  This reduces `ProjectionalEditorSession` from `35,488` to `15,296` bytes and its enclosing
  `MaterializedCodeProject` from `36,320` to `16,128`.

## Verified focused evidence

The following exact commands ran through source `fd2c560`, exit `0`, and are implementation
evidence rather than complete milestone qualification:

```bash
cargo fmt --all -- --check
git diff --check

cargo test --locked -p geosolve-constraint-editor
cargo test --locked -p geosolve-sketch-code
cargo test --locked -p geosolve-demo-web --lib
RUST_MIN_STACK=2097152 cargo test --locked -p geosolve-demo-web --lib \
  managed_canvas_deletion_ -- --nocapture
```

Constraint-editor passes 756 tests with 3 ignored. The complete sketch-code crate, including every
integration target and doc test, passes. The final-source demo-web library passes 300/300 in
`98.61 s`; its explicit 2 MiB deletion filter passes 2/2 in approximately `2.04 s`. The focused
Compass Rose exact terminal and adaptive-polyline insertion regressions pass, and the ordinary-
stack M85-F002 sentinel passes in `55.45 s`. Warnings-denied all-target Clippy passes for
`geosolve-constraint-editor`, `geosolve-sketch-code` and `geosolve-demo-web`.

## Provisional ancestor browser evidence

The following focused browser command exited `0`, 5/5 in `2.2m`, but ran against an unpinned
pre-F003 implementation ancestor rather than source `fd2c560` or frozen final bytes:

```bash
cd /tmp/m85-pw
NO_COLOR=1 npx playwright test --config=/tmp/m85-pw/m85-final-profile.config.cjs
```

Harness SHA-256 is `0248cf43dfae7d49c2f834b2b900a5cd0faa8b85209e40f77cfc5b31cede0947`;
configuration SHA-256 is
`2093af3a2dad08968ebfe64c6265b931c5b51215627ba90b4d0f36f1191c0079`. Evidence directory is
`/tmp/geosolve-m85-profile.4X1M57ap`; the complete captured log is
`profile.typescript`, SHA-256
`8cab5533122f07667076ce0577115a0d00da88ac64cc8d85a7a4d90683559979`.

Across 30 bursts and 1,200 camera samples, all 1,200 admissions are camera-only and zero admit
forbidden work or navigation long tasks. Worst camera callback p95 is approximately `0.2 ms`,
camera RAF p95 `0.6 ms`, completed-presentation RAF p95 `0.8 ms`, frame-gap p95 `16.8 ms`, and
sustained performance `59.6–61.3 fps`. Ordinary drag preview worst p95 is `5.9 ms` with terminal
maximum `101.8 ms`; Compass Rose is `11.4 ms`/`158.2 ms`; Rounded Polyline is
`10.2 ms`/`150.6 ms`. Every release records one save, exact preview parity and no delayed movement.
These results are useful directional evidence only. They do not prove final-source or candidate
budgets; the final-source candidate evidence below, rather than this ancestor, owns nomination.

## Final clean qualification and immutable nomination

The exact clean Nix release gate for source
`5c265e211e20dabc8a27f6402d530f5d645ff15c`, tree
`b55d012443f4dbf7551e30041da2912e666de9db`, ran from
`2026-08-28T06:40:49.038108300+10:00` through
`2026-08-28T07:13:16.391445220+10:00` and exited `0`. Its 6,993-line, 456,580-byte log is
`/tmp/geosolve-m85-gate.8vK5wDu4/release-gate.log`, SHA-256
`0b09720dfd4491575ab10bd3baba2f8f6e7fae9e8e90954ff0026a64de4458eb`. The gate passes workspace
warnings-denied Clippy/tests/doc tests, unchanged 271-row golden checks, WASM parity and adapter
checks, TypeScript Intent/sketch-code/package verification, warnings-denied Rustdoc, benchmark
compilation, release performance, 256-body linkage crossover, cargo-deny licences and the final
Trunk release build.

Without rebuilding, the gate-produced `dist` is frozen at `/tmp/geosolve-m85-uat.QX8fU3Q6`.
It contains exactly seven regular files and zero symlinks at directory/file modes `0555`/`0444`;
its ordered-manifest aggregate is
`dc729ce5fa28929dba2aa086e49246aac7d5173a0583d3b0b64748ffbbb7fda5`. Complete freeze evidence is
`/tmp/geosolve-m85-freeze-evidence.uj9HviX1`. Local service PID `2008536`, invocation
`d9fdcccfa9ce46deafcf46f7b6148e6e`, serves that snapshot at `http://127.0.0.1:18100/`; retained
Tailscale service PID `2008538`, invocation `2c70402e2b2a45a5810ea29722f25f95`, serves the same
snapshot at `http://100.94.63.83:8080/`. Both complete eight-path HTTP ledgers have SHA-256
`305eccfc8fa60786aabfae59edd612e695ce3c15b7224abbf3be0d0852ae0d27`.

The final frozen-byte Playwright profile passes 5/5 in `2.2m`. Its captured log SHA-256 is
`d1174515c320e1f3a0e006bbaad47cc47ba0aeda52fe9bf05c956d91750951c7`, and summary SHA-256 is
`db508a28f46774fbc74f9bdebfc20c513c46934bfd29a7b0775d96962caa63e6`. Across 30 navigation
bursts and 1,200 camera samples, all 1,200 admissions are camera-only, with zero forbidden
admissions and zero navigation long tasks. Worst callback, camera-RAF and completed-presentation
p95 are `0.2`, `0.6` and `0.8 ms`; worst frame-gap p95 is `16.8 ms`; minimum sustained rate is
`59.8249 fps`; and worst retained-hover p95 is `3.5 ms`. Ordinary drag preview p95/terminal maximum
is `5.4`/`87.23 ms`, Compass Rose is `8.7`/`175.50 ms`, and Rounded Polyline is
`6.6`/`184.93 ms`. All nine releases retain preview/terminal parity, no delayed movement and one
save each.

Final-source native flat-adapter evidence passes 19/19 exact tests. It covers camera coalescing and
one-paint work admission, projectional/flat toolbar and terminal-pan parity, authenticated retained
hover and pointer leave, transient/terminal history and work neutrality, shared retained-
coordinator receipts, flat v6 compatibility bootstrap and strict v1-v6 history-free normalization.
Evidence is `/tmp/geosolve-m85-u10-final.D1auvz5d`; command/result/manifest SHA-256 values are
`5da8bf46936228d22034f4195e9e571f6f0227510db257f943ef27686dee6545`,
`5efaa7d874d1c07189ab8ec7398863945abd1d15bb50933e55f47bac888f7f79` and
`ec2b205710bf6d79c09e696fb6023b01ebcac1f922bcae04c2e8486c80702189`.

## Remaining qualification and release

- [x] Pass workspace tests, WASM parity/checks, unchanged 271-row clean golden, release performance,
  warnings-denied workspace Clippy/Rustdoc and the complete clean release gate from final committed
  source.
- [x] Freeze the gate-produced distribution without rebuild, exact-verify local/Tailscale service
  bytes and pass the five-test browser profile against those exact bytes.
- [x] Record final-source native flat-adapter M85-U10 evidence and nominate the immutable candidate.
- [ ] Complete hands-on M85-U1-U9/U11-U12 and obtain explicit supervising-user approval.
- [ ] Only after approval, publish the accepted descendant to GitHub Pages, download and exact-
  verify the separately rebuilt artifact and hosted paths, retire the retained service and close
  M85.

## Semantic-preservation ledger

Camera-only input leaves accepted/current identity, retained document, Intent graph, code session,
history, selection, annotation layout and canonical persistence bytes unchanged. M85 changes no
native equation, residual, Jacobian, priority, branch, tolerance, persistence wire or managed-code
meaning. A fast path that bypasses current-camera hit testing, independent terminal validation,
transactional rejection or exact outer-history authority fails even if it meets timing budgets.
