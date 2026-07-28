<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M47 implementation record

## Status

Complete as of 2026-07-28. The five named direct fixture groups pass, and the authorized
M44 fixture/control/E2E deletion boundary has been applied. M48 subsequently completed its
direct workbench qualification/M40 purge; M49-M52 also subsequently completed.

## Fixture mapping and deletion boundary

### Requirements

- M47 must replace the non-canonical, non-persisted M44 composition with **five** small
  direct fixture groups, preserve every one of the six former M44 contracts and all ten
  M45 UAT points, then delete the fixture and its browser qualification stack
  (`PLAN.md:2056-2071`; `ACCEPTANCE.md:812-816`).
- Direct tests are Rust tests at the owning domain/editor/workbench/evidence boundary;
  they must not use Chromium, CDP, an HTTP server, DOM scraping, or source-substring
  scans (`docs/M46_DIRECT_TEST_REPLACEMENT.md:12-25,48-57,121-128`). Finding capture
  remains deterministic test/UAT infrastructure, not a stable product API
  (`docs/M46_DIRECT_TEST_REPLACEMENT.md:192-205`).
- Only the broad M44 composition and M44-only browser artifacts are in this slice.
  M48 owns M40 direct replacement/E2E deletion; M49/M50 own legacy-lab and remaining
  old-E2E cleanup (`PLAN.md:2073-2104`; `docs/M46_DIRECT_TEST_REPLACEMENT.md:161-170`).

### Direct replacement matrix

The module paths below are the implemented homes in the demo-web inline `#[cfg(test)]`
layout. Each test constructs only the minimum document/input objects it needs; none
reintroduces `HostState` as a reusable product fixture.

| Direct fixture group / concrete module and test | Former M44 group(s) replaced | M45 UAT point(s) retained | Direct assertions and minimal fixture state |
| --- | --- | --- | --- |
| **Role/profile/activity** — `crates/geosolve-demo-web/src/workbench/panels.rs`, `tests::m47_role_profile_activity_keeps_geometry_active_and_reports_reasons` | `m44.construction-profile`; `m44.suppression-dimension-mode` | **1** construction is solver-active but excluded from default profile; **2** suppression/reactivation differs from driving/reference; **3** host-inactive, missing-external, and dependency-unavailable are distinct | A one-curve role/document fixture plus one selected dimension and the minimum activation/external failure inputs. Assert role/profile participation and unchanged accepted geometry, then independently assert driving/reference and each `InactivityReason` presentation. The existing renderer already derives role, profile spans, activity, and external status from a session (`panels.rs:177-248,322-468`; `scene.rs:26-73`). |
| **Parameter/binding/proposal** — `crates/geosolve-demo-web/src/workbench/panels.rs`, `tests::m47_parameter_batch_proposal_stamps_are_atomic_and_recover` | `m44.parameters-bindings-proposals`; parameter half of `m44.identities-retention` | **4** shared parameter batch updates both bindings atomically with coherent input/proposal provenance; **5** invalid-kind and stale batches retain accepted evidence, then a valid batch recovers | A document with one length parameter bound to two driving dimensions and one accepted output proposal; typed valid, wrong-kind, stale, and recovery `ParameterBatch` inputs. Assert both binding targets, accepted stamp/proposal provenance advance only on complete valid batch, rejected/stale retention, and recovery. The former broad rectangle/bindings/output construction is at `host_state.rs:41-100,153-170`; presentation stamp/proposal fields are at `panels.rs:211-320`. |
| **External snapshot/rebind** — `crates/geosolve-demo-web/src/workbench/panels.rs`, `tests::m47_external_snapshot_rebind_retains_then_advances_evidence` | `m44.external-rebind-retention`; external/dependency portion of `m44.suppression-dimension-mode` | **6** missing/stale/topology-incompatible snapshots retain evidence/no implicit repair; **7** only explicit rebind plus fresh valid snapshot recovers | One external line binding with a topology digest, and typed missing, stale, topology-mismatch, rebind, and fresh-valid `ExternalSnapshotSet` inputs. Assert attempted/retained/accepted revisions/status and accepted scene retention before rebind+fresh valid advance. The old snapshot variants and rebind transaction are `host_state.rs:259-320,350-377`; renderer evidence is `panels.rs:396-468`. |
| **Lifecycle/retained evidence** — `crates/geosolve-demo-web/src/workbench/panels.rs` and `scene.rs`, `tests::m47_lifecycle_attempt_and_accepted_identity_never_leak_attempt_into_scene` | `m44.identities-retention`; lifecycle portion of `m44.host-boundary` | **8** design/latest-attempt/accepted identities are distinct across lifecycle/Problems/audit/revision cards/accepted-only scene; **10** state story has no stale display or unexpected geometry movement | A small accepted session followed by one failing typed input and a valid recovery. Assert lifecycle markup has the three identities and failure status, while `svg_markup` contains only accepted identity/input stamps and retains accepted geometry; assert recovery advances accepted identity. The host card and stamps are `panels.rs:177-248`; accepted-only scene provenance is `scene.rs:19-43` (and existing assertion `scene.rs:329-342`). |
| **Typed finding capture** — `crates/geosolve-demo-web/src/workbench/evidence.rs`, `tests::typed_host_capture_contains_inputs_attempt_and_accepted_evidence` | `m44.host-boundary` | **9** capture preserves exact typed parameter/external inputs and accepted/attempted evidence | A test/UAT-only minimal coordinator plus typed `ParameterBatch` and `ExternalSnapshotSet`; expose/refactor a deterministic serialization helper as necessary so the test checks envelope format/checksum, typed entries/snapshot JSON, lifecycle/transcript, accepted and attempted audits, and host-state evidence **without** `Document`, Blob, anchor, download, or fixture marker. This exact owner/test name is frozen in `docs/M46_DIRECT_TEST_REPLACEMENT.md:89-98`; current M45-only branch is `evidence.rs:70-80,145-242`, with checksum at `:258-265`. |

Coverage accounting: the first four rows collectively replace the five semantic browser
groups; the fifth replaces the capture half of `m44.host-boundary`. The old source scan
half of that group is retired, not ported: executable typed API construction supersedes
the scan (`docs/M46_DIRECT_TEST_REPLACEMENT.md:121-128`). Thus all six frozen IDs from
`e2e/m44.mjs:18-32` and UAT points 1--10 retained under
`docs/M53_UAT.md#preserved-verification-points` have exactly one
of the five direct fixture-group owners above (some points deliberately share a group).

### Deletion inventory

#### Deleted in M47 after the five groups passed

| Category | Exact M44-only item(s) and evidence | Disposition |
| --- | --- | --- |
| Broad constructor/state | Delete `workbench/host_state.rs` in its entirety: `HostState` fields, `evidence_marker`, `fixture`, `perform`, `parameter_batch`, `snapshot_set`, and its four fixture-coupled tests (`host_state.rs:20-377,379-534`). This removes the fixed rectangle, three parameters, two external bindings, hard-coded revisions/digests/topologies, and string-action dispatcher. | Replace with test-local minimal fixtures in the five rows; do not retain a production or persisted aggregate fixture. |
| Workbench fixture wiring | Remove `mod host_state` and `Workbench.host_state`; fixture reset/load, action prefix dispatch, M44 suppress/reactivate/dimension actions, fixture-only notice, and fixture autosave bypass (`mod.rs:8,50-55,93-99,430-506,730-744`). | Ordinary `new`, editor actions, normal persistence, and workbench rendering remain. Direct tests must call typed public transactions, not action strings. |
| Fixture-only controls/hooks | Remove `m44-load`, `m44-role-profile`, `m44-role-construction`, `m44-parameter-valid`, `m44-parameter-invalid`, `m44-parameter-inactive`, `m44-parameter-stale`, `m44-external-missing`, `m44-external-topology`, `m44-external-stale`, `m44-external-rebind`, `m44-external-valid`, `m44-suppress`, `m44-reactivate`, `m44-dimension-driving`, and `m44-dimension-reference`; remove their corresponding DOM buttons/hooks, including `#m44-load` and every `data-wb-action` carrying these names. Source dispatch evidence: `mod.rs:437-505`; browser selector/action evidence: `e2e/m44.mjs:106-120,171-293`. | These exist solely to drive the temporary composition. Generic workbench actions (`new`, undo/redo, cancel, finish, delete, constraint, dimension, problems, capture-finding) remain (`mod.rs:430-467,500-507`). |
| Fixture-labelled markup/hooks | Delete the M44 identity/profile/activity/external coverage labels and fixture-loaded flag: `data-coverage-id="m44-three-identities"`, `m44-input-stamps`, `m44-parameters`, `m44-activity`, `m44-external`, `m44-accepted-profile`, `data-fixture-loaded`, and `data-profile-span-list="accepted"` only where introduced for M44 qualification (`panels.rs:177-248,251-361,364-468`). Delete the M45 capture marker, `M45*` payload types, `capture_m45`, and M45-only three-file package (`evidence.rs:31-68,145-242`). | Keep generic lifecycle/host-state presentation data as ordinary, directly tested output, but remove test-ID/fixture marker contracts. M47 capture is typed deterministic content, not HTML/DOM/download evidence. |
| Fixture-coupled inline tests | Rewrite/remove tests importing `HostState`: `panels.rs:553-598` and `scene.rs:292-342`; replacement tests belong to the matrix. | Preserve non-fixture scene and panel behavior under direct inputs. |
| M44 browser artifact | Delete `crates/geosolve-demo-web/e2e/m44.mjs` entirely, including its static forbidden/required-string scan, Node HTTP server, Chromium/CDP connection, `/tmp/geosolve-m44-browser-profile`, `M44_PORT`, `M44_DEBUG_PORT`, `CHROMIUM`, DOM polling/scraping, mutation timing, and download interception (`e2e/m44.mjs:3-17,34-166,263-302`). | M47 deletion is limited to this script and its exclusively referenced machinery; retain no M44 browser profile/server helper. |

#### Must remain (not M47 deletion)

| Item | Why / owner |
| --- | --- |
| Generic `capture-finding` action and the non-fixture `capture` path, generic evidence envelope/checksum, ordinary scene SVG capture, and platform download plumbing (`mod.rs:504-506`; `evidence.rs:11-29,70-143,245-279`) | M48 still owns direct M40 evidence-package replacement and M40 browser-only download/blob retirement; M47 may refactor this into a testable serialization helper but must not delete the generic capability prematurely (`PLAN.md:2080-2085`; `docs/M46_DIRECT_TEST_REPLACEMENT.md:76-78,167-169`). |
| `workbench/panels.rs`, `scene.rs`, normal host-state presentation, accepted-scene rendering, and generic workbench routing/effect/persistence/platform modules | Current workbench remains the product/default UI; M47 replaces only M44 fixture qualification. M48 directly qualifies broader presentation/persistence; M50 handles legacy application purge (`docs/M45_CLEANUP_PLAN.md:72-90,92-103`). |
| `e2e/m40.mjs`, `e2e/m14.mjs`, their shared CDP/server/profile/download pieces if any, and `scripts/serve-m40.sh` | M48/M50 deletion gates explicitly own these; do not remove shared M40/M14 infrastructure merely because `m44.mjs` is gone (`docs/M46_DIRECT_TEST_REPLACEMENT.md:148-159,166-170`). |
| `scripts/serve-m45.sh` | It is archived M45 manual-UAT serving, not proven M44-exclusive code. The frozen ledger assigns its deletion/replacement to M50 when fixture UAT instructions are removed (`scripts/serve-m45.sh:6-22`; `docs/M46_DIRECT_TEST_REPLACEMENT.md:152-155,169`). |

### Dependencies, order, and focused acceptance commands

1. Start from the M46 frozen ownership ledger; add the five direct groups before deleting
   anything. Retain M41--M43 domain assertions as semantic foundations and test only
   workbench presentation/capture deltas in M47 (`docs/M46_DIRECT_TEST_REPLACEMENT.md:85-98,130-136`).
2. Make evidence serialization deterministic and native-testable before removing the
   fixture-specific M45 capture branch; do not make a DOM/download test its replacement.
3. Delete the aggregate fixture, all action/DOM hooks and M44-only E2E stack only after
   all five groups pass. Then confirm no M44 marker/action/script/profile reference remains.
4. Leave M40 and M14 artifacts and `serve-m45.sh` intact for their assigned M48/M50 slices.

Focused acceptance commands (workspace root; no browser server/CDP/Node E2E):

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'git diff --check'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch --test m41 --test m42 --test m43'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
```

These are the M46 direct-validation commands specialized to the M47 owners
(`docs/M46_DIRECT_TEST_REPLACEMENT.md:172-185`); do not run `node e2e/m44.mjs` as
replacement evidence.

### Decisions / inferred constraints

- “Five groups” is a test-fixture partition, not five product-state objects. The old
  `HostState` fused unrelated role, parameter, external, lifecycle, and capture data;
  each direct fixture should contain only the state needed for its assertion.
- The existing panel/scene markup is a formatting boundary suitable for deterministic
  Rust-string/view-model assertions, but M44-specific `data-*` IDs are not retained API
  contracts. Preserve semantic content, not selector inventory.
- M44’s static forbidden-symbol scan (`e2e/m44.mjs:263-272`) is explicitly retired:
  compiler-visible use of typed public `ParameterBatch`, `ExternalSnapshotSet`, role,
  and rebind transactions is the durable boundary test.
- `host_state.rs` was deliberately sidecar/non-persistent (`host_state.rs:20-23`), and
  the focused M47 fixtures retain that property (`ACCEPTANCE.md:812-816`).

### Implementation resolutions

- `typed_host_capture` returns deterministic checksummed JSON and is exercised natively
  without `Document`, Blob, anchor, download or browser APIs.
- The M44 DOM buttons/actions, coverage markers and fixture-loaded hooks were removed from
  their workbench/HTML declarations. Scoped source search finds no M44 or `HostState`
  fixture marker in demo-web Rust, HTML or E2E sources.

### Completion evidence

- Focused M41-M43 integration suites pass.
- The all-feature `geosolve-demo-web` suite passes 101 tests.
- The complete locked all-feature workspace test suite, warnings-denied workspace Clippy,
  formatting, diff and the all-feature `wasm32-unknown-unknown` check pass.
- No browser E2E was run or used as M47 evidence.
- M40/M14 browser artifacts and the legacy playground remain for M48-M50 only.

### Out of scope

- M48 M40 qualification and generic evidence/download retirement.
- M49 retained legacy semantic extraction and M50 legacy route/application, all-old-E2E,
  and `serve-m45.sh` deletion.
- Any domain behavior change, canonical persistence change, browser UAT, or new browser
  automation.
