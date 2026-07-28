# GeoSolve active handoff

## Objective

Build two production library deliverables over the validated M1-M7 baseline:

1. a comprehensive embeddable 2D CAD sketch engine, including ordinary planar
   relations/dimensions, advanced curves, host integration and separate operations and
   production-topology companions;
2. 2D and 3D rigid-body kinematics for linkages and CAD assemblies.

M10-M14 delivered a disposable 2D Sketch Playground Alpha over reusable Rust APIs.
M39-M44 established a desktop-only CAD-like workbench. Cleanup M46-M50 replaced every
retained assertion with direct owning-layer tests and removed the separately routed legacy
application and browser E2E stack. M51 hardened the one surviving workbench; M52 completed the
direct-qualified post-cleanup host-semantics candidate, M53 received supervising-human approval,
M54 completed stable sketch-owned diagnostics, M55 completed the preserved alpha
constraint/dimension/branch-action surface, M56 completed prepared jobs and exact-input
compare-and-swap publication, M57 completed dependency-local retained solving and bounded
production-scale evidence, and M58 is now active. The workbench is a
non-authoritative demo consumer, not a production UI or third solver. Mobile support and
physics remain outside future acceptance.

## Read first

1. `AGENTS.md`
2. `PLAN.md`
3. `ARCHITECTURE.md`
4. `ACCEPTANCE.md`
5. `docs/SCENARIOS.md`
6. `REFERENCES.md`
7. `docs/adr/0001-*.md` through `docs/adr/0029-*.md`

`PLAN.md` is the authoritative execution order. `OVERNIGHT_REPORT.md` is a historical
M1-M4 record, not current status.

## Current state

M0-M44 and the advanced free-radius circle/arc tangency follow-up are complete. M0-M32
establish the frozen numerical/domain baseline, persistent sketch and linkage products,
advanced curves/constructions, visual-profile analysis and the supported `0.2.0` preview.
M33 freezes production-embedding decisions and workloads; M34-M38 add retained unsolved
intent, operation control, closed semantic operands, the ordinary CAD relation/dimension
catalog and persistent measurements; M39-M40 establish the workbench and headless editor;
M41-M43 add construction/activation, typed parameters and immutable external snapshots;
M44 integrates those host semantics in the workbench.

M45 is complete as a cleanup investigation and UAT-point-capture checkpoint. It records
no human approval. M46 freezes a direct owner or reviewed retirement for every old
M14/M40/M44 E2E, static-scan and legacy-inline assertion. M47 is also complete: five small
direct fixture groups cover all six former M44 contracts and ten preserved UAT points, and
the broad host fixture, fixture-only controls and M44 E2E infrastructure are removed. The
M48 direct tests now own the retained editor/workbench contracts, and its M40 E2E, serving
script, source scans and browser-only qualification controls are removed. The old M14 full
browser run remains incomplete historical evidence and is not a cleanup gate.

M49 is complete: every retained class-A/M14 semantic claim has a direct sketch, linkage,
editor, persistence or focused presentation owner, direct duplicates are confirmed, legacy
delivery/out-of-scope claims are explicitly retired, and the ledger has zero unowned
assertions. M50 is also complete: it deleted the final M14 E2E/CDP/server machinery,
`#/dev/lab`, the playground application and its legacy-only platform glue in one reviewed
cut. M51 is complete: it removed the remaining design-only storage migration, duplicate M40
report/evidence fixtures and stale distribution copy while preserving directly owned
presentation, persistence and typed evidence transformations. M52 is complete: its minimal
in-memory host-semantics candidate passed direct native/WASM qualification and independent
verification. M53 is complete: the supervising human rated every M53-S5 scorecard area Pass,
reported no concern or blocker and explicitly approved the host-semantics gate on 2026-07-28. Its
objective state claims remain backed by M52 tests and the final direct-qualified candidate. M54
then completed persistent-ID diagnostic DTOs, separated rank/mobility evidence and isolated raw
core reports behind explicitly unstable seams. M55 then completed the closed 13-relation and
five-dimension alpha action matrix, persistent contact/angle branch editing, typed disabled
reasons, direct native/WASM presentation qualification and two reusable scenario leaves. M56 then
added complete-input snapshots, typed scratch jobs, non-mutating patches and safe host-owned
native/WASM scheduling. M57 retained compatible runtime/core state, indexed persistent mappings,
dependency-local updates, accepted-revision profile caches and honest bounded rank/scale evidence.
The active milestone is **M58: sketch operations companion**. Old Chromium/CDP, HTTP
serving, DOM-scraping, screenshot, wall-clock browser-timing and source-substring-scan gates remain
retired.

Completed M53 finding M53-P011 replaced only the M52 candidate's one-off launcher and overlay with
six reusable typed host-semantics scenario definitions, a nested **Scenarios** selector and a
contextual guide sidebar. The four fixed fixture families, ten objective points and ordinary-workspace isolation
retain their direct M52 qualification. Clean integrated requalification passes and
`docs/M53_UAT.md` records the frozen M53-S3 candidate and final M53-S5 approval.

M53-P012 subsequently supersedes S3 before human ratings: the top dropdown remains, but nested
group disclosures are replaced by right-expanding hover/focus flyouts for faster navigation. The
stable scenarios, guidance and fixture semantics are unchanged. The complete clean release gate
passes from build-source commit `49ddcb8`; `docs/M53_UAT.md` records the frozen M53-S4 manifest and
temporary human endpoint. The final M53-S5 review rated the flyout navigation Pass.

M53-P013 subsequently supersedes S4 before ratings. It requests structured current-error metadata
at the headless-editor/UI seam, persistent owner-and-operand highlights on the accepted canvas,
accessible error markers, a global fallback when attribution is not defensible, and two reusable
demonstration scenarios under a fourth selector group. The implementation and direct regressions
are complete, growing the catalog to eight scenarios. The complete clean release gate passes from
build-source commit `f72116b`; `docs/M53_UAT.md` records the frozen M53-S5 manifest and temporary
human endpoint. The targeted attribution review passed as part of final M53-S5 approval.

The finalized M54-M64 functional/release roadmap covers completed stable diagnostics and early
alpha constraint/dimension/branch-action parity, then prepared jobs, incremental scale, operations,
production topology, advanced workbench, advanced UAT, API/schema freeze, integrated UAT and the
production embedding release.

Durable cleanup records:

- `docs/M45_CLEANUP_PLAN.md`
- `docs/M45_UI_CLEANUP_INVESTIGATION.md`
- `docs/M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md`
- `docs/M53_UAT.md` (approved M53 scorecard with the archived M45 capture retained in one place)
- `docs/M46_DIRECT_TEST_REPLACEMENT.md`
- `docs/M46_REBASE_INVENTORY.md`
- `docs/M47_IMPLEMENTATION.md`
- `docs/M48_IMPLEMENTATION.md`
- `docs/M49_IMPLEMENTATION.md`
- `docs/M50_IMPLEMENTATION.md`
- `docs/M51_IMPLEMENTATION.md`
- `docs/M52_IMPLEMENTATION.md`
- `docs/M54_IMPLEMENTATION.md`
- `docs/M55_IMPLEMENTATION.md`
- `docs/M56_IMPLEMENTATION.md`
- `docs/M57_IMPLEMENTATION.md`

The workspace-wide warnings-denied Clippy blocker formerly reported at
`crates/geosolve-linkage/src/spatial.rs:2804` was cleared during M46 and the complete
workspace Clippy command now passes. M47 removed only its authorized M44 fixture and E2E
slice. M48 subsequently removed its authorized M40 E2E/serving slice after direct
qualification; M49 completed semantic extraction, M50 removed the final M14 E2E, playground and
obsolete browser/serving infrastructure, M51 consolidated the directly tested survivor, and M52
direct-qualified its disposable sidecar candidate after independent verification.

## Work rules

1. Complete milestones in `PLAN.md` order.
2. Keep `geosolve-sketch` and `geosolve-linkage` as separate domains over `geosolve-core`.
3. Preserve explicit branch/span/winding/assembly state.
4. Never report success without independent residual and domain validation.
5. Never weaken a tolerance or remove a regression merely to pass a gate.
6. Keep APIs private or crate-private until a milestone requires public exposure.
7. Add a finite-difference Jacobian test and structured audit descriptor for every residual.
8. Make commits only when the supervising caller permits them.

## Standard verification

Run through the project shell:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown'
```

Run the relevant Trunk build only when shared public APIs or the WASM consumer change.
During cleanup, automated qualification is direct Rust/WASM testing; do not launch an old
browser E2E suite or cite it as a current gate.
