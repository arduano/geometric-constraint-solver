<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M97 qualification and review preview

Historical milestone record. For current setup and qualification, see the
[documentation index](README.md) and [release guide](RELEASE_QUALIFICATION.md).
Local artifact names below identify archived evidence; they are not current preview locations.

This records the original focused-dimension M97 candidate. The subsequent
[default-priority amendment](M97_PRIORITY_DIMENSIONS.md) and
[source-authored metadata amendment](M97_AUTHORING_IMPLEMENTATION.md) supersede it.
[M97 closure](M97_CLOSURE.md) identifies the accepted final product.

## Files, APIs and behavior

`geosolve-constraint-editor/src/dimension_presentation.rs` adds
`DimensionDisplayMode`, `DimensionPresentationState`,
`DimensionPresentationContext`, complete `SceneDimensionEntry` metadata and
`SceneDimensionEntry::target_metadata`. Annotation/coordinator integration retains
automatic positions. `DimensionTargetMetadata::storage_value_for_display` reuses
native conversion and preserves angle quadrant/full-turn semantics.

The workbench bridge's `dimensions.rs` adds accepted source ownership, exact
editing values, revision-authenticated commands, stable Inspector row identities,
mode/pin persistence and terminal camera metadata. Frontend dimension Inspector,
canvas controls, viewport and adapters connect those APIs. Numeric drawing, SVG
and native picking consume one resolved visibility policy. Focused owner tests,
three M97 browser workflows and corrected lifecycle harnesses qualify these seams.

The workbench defaults to **Focused**: without selection, only pins remain;
selection reveals at most six ordinary callouts. A stationary 250 ms hover previews
one measurement. The Inspector retains related measurements, with public patch
values such as the **12 mm channel width** before collapsed generated offsets.
Four pins persist outside design history. **All** and **Hidden** remain explicit
choices. Hidden callouts neither paint nor intercept clicks.

Automatic slots survive pan/zoom, selection-only rebuilding, Fit, centering and
resize. Affected geometry edits invalidate their cached bases. Automatic displacement
is bounded to 96 CSS pixels, with 6/12 px collision separation/restoration clearance.
Exact accepted values supply supported edits through existing transactions/history;
rounded canvas text never becomes an edit value. Standalone native scenes retain
their existing default policy.

No solver equations, residual tolerances, hard/soft priorities, rank/DOF rules,
domain geometry or explicit branch semantics changed. No golden row changed.

## Exact candidate and integrated evidence

| Identity | Value |
| --- | --- |
| Qualified source | `a39f35ade58c3d5a272899dfb4e616d6578a9d14` |
| Qualified tree | `e1918da331ebd7e54568ede8d0863bf0b65df5dd` |
| Signed gate run | `20260908T133354-cff36f90` |
| Artifact | 12 files, 28,392,001 bytes |
| Files SHA-256 | `53266b0d9ae8cb2288196cbc701fb4189ec975558ecbb0b8dffaf3d58ad64c24` |
| Manifest SHA-256 | `4837f58da4d3f746473d8999eed0f96e9e8b059348baaf7bf9f7f6072621b8fb` |

Executed nomination and evidence audit:

```bash
nix-shell shell.nix --run './scripts/release-gate.sh --resume 20260908T123427-82cdb8e6'
python3 target/m97/audit-qualification.py 20260908T133354-cff36f90
```

**243/243 obligations pass in 38m19s.** The runner conservatively executed all
243 stages afresh after the reviewed inventory change; none supplied a reused
success. The signed result is complete, clean-source and source-unchanged.
Format, warnings-denied workspace Clippy, native/headless suites, optimized WASM,
Rust documentation, builds, packaging/licences, browser, transport and isolated
performance all pass. Frontend preflight passes **216 tests**; the native
workbench suite passes **333**, with one existing ignored test.

All **271 golden cases** pass and match the reviewed checklist at SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797`.
All three optimized-WASM lifecycle cases pass, including complete frame equality
for every bundled sample against independently composed native scenes. The final
isolated performance stage passes in 186.9 s; its 256-moving-body linkage check
passes in 133.8 s.

The complete **48-case browser inventory** has 17 fresh catalog/sample-open checks
and 47 fresh full workflows. There are no failures, skips, retries or reused
browser leaves. The three M97 cases pass in 50.3 s (focus/pins), 27.2 s (stable
positions) and 77.2 s (editing/history). Cubic authoring, reproduction import and
Circle authoring pass in 47.8, 37.9 and 73.6 s respectively. Browser qualification
takes 21m13s including preparation of its initial-state witnesses.

The independent audit authenticates every stage receipt and all **9,881 referenced
evidence files**, passing in 19.3 s. Its report is
`target/m97/qualification-audit.json`. The gate log is
`target/m97/release-gate-r5.log`; signed evidence is under the run directory.

## Frozen preview and served-byte verification

```bash
python3 target/m97/freeze-preview.py 20260908T133354-cff36f90
nix-shell shell.nix --run 'GEOSOLVE_CHROMIUM_PATH=$(command -v google-chrome) node crates/geosolve-demo-web/frontend/scripts/verify-artifact.mjs --manifest target/m97/preview-20260908T133354-cff36f90/production.json --directory target/m97/preview-20260908T133354-cff36f90/geosolve-production --url ${PREVIEW_URL} --receipt target/m97/preview-artifact-verification.json'
python3 target/m97/audit-preview-artifact.py 20260908T133354-cff36f90
```

All 13 HTTP routes (12 files plus `/`) pass exact bytes, MIME/base-path and bounded
actual-WASM readiness checks. `target/m97/preview-artifact-binding.json` binds the
served artifact and frozen copy to the signed preparation and qualified source.
The accepted M96 service and artifact are preserved.

The documentation-only handoff passes `git diff --check` and
`./scripts/release-gate.sh --docs-only --since a39f35a` for seven prose files and
14 added links. It preserves the qualified product and frozen bytes; no product
rebuild or repeated solver/browser gate is used for that prose.

## Acceptance criteria and final navigation measurements

Mechanical criteria pass for all eight dimension families, reference notation,
complete metadata, bounded visibility/placement, manual layout, hidden picking,
hover/label transit, pins/reload, groups, precise edits, stale/failed requests,
input focus, native/managed history and standalone rendering compatibility.
M97-F001 originally moved 45 of 82 manifold labels by up to 628 px after four
-90 wheel samples and an empty click. The corrected browser regression retains
all 82 within **0.01 CSS pixels** across that zoom and cold scene rebuild.

The final frozen artifact uses the same comparative probe and Chromium 151,
1440×900, DPR 1, RTX 3090/ANGLE Vulkan/WebGL2 as the retained M96 baseline:

```bash
NAV_OUTPUT=target/m97/performance-final NAV_MANIFEST=target/m97/preview-20260908T133354-cff36f90/production.json node target/m97/run-navigation-probe.mjs
```

Median bridge work in milliseconds:

| Sample | Wheel, M96 → M97 | Pan, M96 → M97 | Idle hover, M96 → M97 |
| --- | ---: | ---: | ---: |
| Dogbone coupon | 3.9 → 3.0 | 3.6 → 3.8 | 1.5 → 2.3 |
| Manifold | 11.6 → 7.5 | 14.1 → 9.4 | 6.1 → 9.1 |
| Dense robotic harness | 6.5 → 7.9 | 7.7 → 5.3 | 2.9 → 3.8 |

Manifold rendering medians fall from 15.2 to 10.1 ms for wheel navigation and
11.4 to 6.6 ms for pan. Dense pan rendering falls from 8.1 to 5.5 ms. Hover costs
increase, dense wheel work increases and coupon pan is slightly slower; this is
not a universal speedup or a universal 60 Hz guarantee. All three samples retain
exact direct-probe saved bytes, report zero browser errors and produce zero idle
frames. The final report and simple/manifold/dense screenshots are in
`target/m97/performance-final/`. Contextual overview, selection and edit screenshots
also remain in the signed browser run's M97 result directories.

## Failed attempts and remaining limits

[Implementation evidence](M97_IMPLEMENTATION.md) preserves exact focused commands,
regressions and the failed/interrupted attempts. The initial Clippy float assertion
failure was corrected with exact bit comparisons. WASM expected-frame composition
now explicitly applies the workbench's native Focused policy; context-recovery
pixel witnesses now clear the toolbar. Complete frame/pixel assertions remain.

Later overlapping manifold authoring/import workflows exposed pending-publication
timeouts. A separate import assertion started before asynchronous restoration
completed. The harness now requires persisted original source → no managed source
after New sketch → exact restored source before its unchanged UI/geometry checks.
Cubic, Circle, reproduction import and M97 edits use the existing one-worker
project; all cases and timeout constants remain intact. An interrupted parent run
remains incomplete. None of these attempts is presented as a qualified release.

Large manifold source compilation, authoring and restoration can still take tens
of seconds and block the UI. Scheduling/completion corrections qualify functional
results; they do not fix that product cost. Crowded callouts can remain available
only in the Inspector. Six ordinary callouts and four pins are deliberate bounds.

**maintainer acceptance remains pending.** The implementation, mechanical
qualification and verified preview are ready for review; M97 is not closed.
