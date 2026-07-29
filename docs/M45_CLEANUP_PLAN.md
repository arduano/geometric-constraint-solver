<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M45 cleanup investigation record

## Status

M45 is complete as an investigation and UAT-point-capture checkpoint; it records no human
approval. At the M45 snapshot implementation cleanup had not started and the temporary M44
fixture remained in source. M47 completed on 2026-07-28: its five direct groups now preserve
the ten points retained under `docs/M53_UAT.md#preserved-verification-points`, and the broad fixture/M44 E2E slice is
removed. M48 also
completed on 2026-07-28 after direct qualification and removed the M40 browser-E2E/serving
slice. Human UAT remains relocated to post-cleanup M53.
M49 subsequently completed the zero-unowned-claim extraction, and M50 removed the final
legacy application/E2E/serving slice after direct qualification and independent verification.

Independent read-only review found the investigation sufficient to guide implementation:
the locked demo-web suite passed 103/103 in `nix-shell`, `git diff --check` passed, and no
major/minor documentation inconsistency was found. No browser suite was run during this
investigation. Workspace-wide Clippy reproduced the separately recorded linkage lint at
that checkpoint; M46 subsequently cleared it.

Source inventories:

- `docs/M45_UI_CLEANUP_INVESTIGATION.md`
- `docs/M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md`

## M45 snapshot: UI state

The new workbench has replaced the playground only as the default CAD-like application.
Root and ordinary routes load `src/workbench/**`; `#/dev/lab` still deliberately loads
`src/playground.rs`. Both applications, both DOM roots and both persistence models are
still compiled and shipped.

At the M45 checkpoint, the workbench owned the M40-M45 product path: the headless editor adapter, ordinary
CAD interaction, retained design/attempt/accepted presentation, host-state panels and
focused finding evidence. The legacy lab still uniquely owns broad conic/NURBS/fillet
authoring, profile/performance demonstrations, import/export/capsules, spatial diagnostic
examples and the full M14 browser workflow.

Therefore the old UI is **transitional, not dead at the M45 snapshot**. M49 must extract
every retained semantic assertion or record explicit retirement before M50 deletes the
legacy runtime. The cleanup does not require UI feature parity: durable advanced
mathematics remains in native domain tests, while legacy-only browser delivery retires.

## M45 snapshot: test state

The focused demo-web run reports 103 tests, but 92 belong to the legacy consumers:

| Current tests | Count | Decision |
| --- | ---: | --- |
| Legacy assertions whose non-presentation contract must move to sketch/editor owners | 40 | Migrate before deleting their legacy home. |
| Legacy browser-adapter assertions | 29 | Keep only while the lab ships; retain a much smaller workbench smoke set later. |
| Exact duplicates of cited native M13/M14 coverage | 3 | Retirement candidates when their legacy code slice is removed. |
| Legacy selector/page-presentation inventory | 4 | UI bloat; retire with the corresponding lab DOM/route. |
| Assertions awaiting product/owner disposition | 16 | Allocate through M46/M49 to direct replacement or explicit retirement; do not delete by accident. |
| Current workbench-focused inline tests | 11 | Keep and expand around smaller host fixtures. |

The 40 migration candidates are useful tests in the wrong layer, not a reason to retain
the old UI forever. The 29 adapter tests are transitional rather than solver authority.
The 3 duplicate and 4 presentation-only tests are the clearest bloat, but removing seven
tests alone provides little value while their legacy application still exists.

The M14 browser script has one semantic migration group, two browser-persistence adapter
groups, one legacy-layout group and nine groups coupled to unresolved advanced/profile/
performance/file/mobile scope. It is no longer an M45 gate and should not be rerun as one.
Its required assertions must be extracted into focused native, editor or direct presentation
suites before the script and lab route are retired.

Durable tests to retain regardless of browser cleanup are native sketch M13, M14 and
M41-M43, plus the constraint-editor M40 transition corpus/golden oracle. M40 browser
qualification was transitional cross-channel evidence; M44's six focused groups were the
host-semantics consumer evidence at that checkpoint.

## Parent cleanup decisions

1. **Keep `src/workbench/**` as the sole product/default UI.** Browser code remains a thin
   public-API consumer; interaction policy stays in `geosolve-constraint-editor`.
2. **Quarantine `#/dev/lab` as a temporary diagnostic application.** Add no new product
   behavior there. M49 extracts retained semantics or records retirement; M50 removes it.
3. **Do not migrate spatial demo UI into the planar workbench by default.** Preserve
   `geosolve-linkage` domain coverage; retire or split the read-only browser diagnostic at
   M50 unless a separate linkage consumer is explicitly approved.
4. **Treat the current M44 host fixture as disposable composition, not product state.** It
   must not enter workspace persistence or canonical sketch state.
5. **Replace the broad host fixture with small owned fixtures:** role/profile/activity;
   parameter/binding/proposal; external snapshot/rebind; lifecycle/retained evidence; and
   finding capture. A later human-UAT scene may compose these without becoming a persisted
   product fixture.
6. **Keep exact host-state capture capability, but decouple it from the broad fixture.** A
   smaller deterministic evidence harness must continue to record typed parameter and
   external inputs plus accepted/attempted evidence.
7. **Keep scene-capsule semantics only as transitional diagnostic input.** M49 either moves
   retained codec/evidence behavior to direct tests or retires it; M50 removes obsolete
   glue. A future explicitly scoped milestone owns any later supported release-surface decision.

## Ordered cleanup boundary

1. M45 preserves the UAT checklist and exhaustive test inventory (complete).
2. M46 froze direct owners and explicit retirements for every old assertion (complete).
3. M47 replaced the broad host fixture with five small regression groups and purged M44 E2E (complete).
4. M48 replaced retained M40 contracts and purged M40 browser infrastructure (complete).
5. M49 extracted retained legacy semantics and completed a zero-unowned-assertion ledger (complete).
6. M50 removed the old E2E stack, `#/dev/lab`, playground code and obsolete glue (complete).
7. M51 hardened one workbench, M52 direct-qualified the minimal UAT candidate and M53 completed
   supervising-human UAT (complete).

This ordering removes the legacy application without losing regression authority or
pretending browser-only delivery is a durable product capability.

## Known closure blockers

- Human UAT and approval were pending at this investigation checkpoint and later passed at M53.
- The workspace-wide warnings-denied Clippy failure found during M45 was cleared by the
  behavior-preserving M46 match-guard rewrite in `crates/geosolve-linkage/src/spatial.rs`.
- No old playground, fixture or test deletion was authorized merely by this
  investigation; implementation subsequently followed the completed M46-M50 gates
  in `PLAN.md`.
