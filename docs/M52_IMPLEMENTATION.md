<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M52 post-cleanup host-semantics UAT candidate

## Status

**Complete as of 2026-07-28.** The disposable, human-visible candidate, its direct objective
qualification, final parent gates and independent read-only verification pass. This document
records objective M52 evidence only; it records no human UAT rating or approval. Active M53
exclusively owns clarity, trust and sign-off.

> **Historical successor note (M53):** this record describes the UI and private symbols exactly as
> qualified at M52, including the then-current **Load disposable M52 UAT** button and overlay.
> Active finding M53-P011 later replaces only that one-off presentation with a private typed
> six-scenario catalog, top **Scenarios** selector and guide sidebar. The four fixture families,
> ten objective points, typed transitions/evidence and ordinary-workspace isolation remain; current
> selector qualification, candidate identity and human findings belong in `docs/M53_UAT.md`. This
> note does not reopen M52 acceptance or record M53 approval.

## Implemented boundary

M52 adds one explicitly labelled in-memory sidecar to the sole workbench. It composes four small
fixtures and the ten points under `docs/M53_UAT.md#preserved-verification-points`; it does not restore the deleted
playground, router, browser E2E, server or broad M44 fixture.

- `workbench/uat.rs::UatCandidate` owns fixed-identity role/activity,
  parameter/proposal, external/rebind and lifecycle/evidence fixtures.
- `UatAction` is the closed typed transition set used by both direct tests and the thin WASM
  event adapter. `UAT_POINTS` supplies deterministic numbered instructions and separates each
  objective check from its M53 human judgment.
- `workbench/mod.rs::Workbench::uat` is an optional sidecar. Rendering reads the active fixture
  while UAT is loaded, but the pre-existing ordinary coordinator remains untouched.
- `index.html` exposes **Load disposable M52 UAT** in the sole workbench. The panel provides
  reset, exit, typed actions and copyable fixed-provenance evidence.
- `evidence.rs::serialize_typed_host_evidence` remains the directly tested crate-private M47 path.
  The crate-private M52 path deliberately replaces canonical workspace documents with retained
  design/accepted identities and revisions while preserving typed input, lifecycle, transcript,
  audit and host-state evidence. Neither path adds a supported public export.

## Persistence and product isolation

- The candidate is never represented by `WorkspaceSnapshot` or a `localStorage` key, and M52
  finding evidence does not serialize its canonical design or accepted sketch documents.
- `save` returns immediately while UAT is active. Load, reset and typed UAT actions therefore
  cannot overwrite ordinary persistence.
- Every ordinary mutating action, including **New**, is rejected while UAT is active;
  ordinary tool/action buttons are disabled where presented. Problems may still be opened for
  inspection.
- **Exit UAT** discards the sidecar and reveals the unchanged ordinary coordinator. The following
  save writes that unchanged ordinary checkpoint. Reload likewise starts from ordinary persisted
  state only.
- The four fixtures use fixed, disjoint document namespaces, revisions, topology digests and
  capture provenance. Replaying the same typed sequence in a new `UatCandidate` produces identical
  evidence text.
- No UAT type is public, no sketch schema changed, and no host semantics moved into browser glue.

## Ten-point ownership matrix

| # | Preserved point | M52 candidate action/evidence | Direct objective owner |
| ---: | --- | --- | --- |
| 1 | Construction remains solver-active while default-profile participation changes. | **Construction**, then **Profile**; accepted scene and host profile card come from the active retained coordinator. | `m52_candidate_directly_qualifies_role_activity_and_mode_distinctions`; M41/M47 role regressions. |
| 2 | Suppression/reactivation is distinct from driving/reference mode. | **Suppress**, **Reactivate**, then **Reference** on one dimension. | Same M52 role/activity regression plus M41/M47 direct owners. |
| 3 | Host-inactive, unavailable-external and unavailable-dependency reasons remain distinct. | **Host inactive** marks the dimension directly; **Missing dependency** marks its curve unavailable-external and derives unavailable-dependency on the dimension. The external fixture separately demonstrates a missing required snapshot. | Same M52 role/activity regression plus M41/M43 direct owners. |
| 4 | One shared host parameter updates two bindings and proposal provenance atomically. | **Parameter valid** publishes revision 11 with two driving bindings and one accepted proposal. | `m52_candidate_transcript_retains_and_advances_only_at_typed_recovery_boundaries`; M42/M47 direct owners. |
| 5 | Invalid-kind/stale parameter input retains evidence and valid input recovers. | **Invalid kind**, **Parameter stale**, then **Parameter recovery**; accepted input remains at 11 until recovery publishes 13. | Same M52 transcript regression plus M42/M47 direct owners. |
| 6 | Missing, stale and topology-incompatible snapshots retain accepted evidence. | **External missing**, **External stale**, **Topology change**. | Same M52 transcript regression plus M43/M47 direct owners. |
| 7 | Recovery requires explicit rebind and then fresh compatible input. | **Explicit rebind** changes only the declaration; **Fresh recovery** alone advances accepted state. | Same M52 transcript regression plus M43/M47 direct owners. |
| 8 | Design, latest attempt and accepted identities remain separate and the canvas is accepted-only. | **Lifecycle rejected**, inspect lifecycle/Problems/host cards/canvas, then **Lifecycle recovery**. | Same M52 transcript regression; `scene.rs` and `panels.rs` M47 direct owners. |
| 9 | Finding evidence preserves typed inputs and accepted/attempted evidence. | **Capture typed evidence** emits fixed-provenance checksummed parameter, external and lifecycle sections for copying. | `m52_candidate_evidence_is_deterministic_and_contains_typed_inputs`; evidence serializer owner. |
| 10 | Natural recovery tells one coherent story without stale display or unexpected movement. | Repeat role/activity and parameter/external flows using the same typed actions. | Objective state/retention portions are covered above; coherence and trust remain M53 judgment. |

## Review corrections made during parent integration

Parent review found two objective defects in the first implementation and converted both to direct
regressions:

1. **New** was initially allowed while UAT was active. It could replace and then persist the
   ordinary coordinator, contradicting the sidecar isolation claim. The ordinary-action gate now
   permits only Problems during UAT, and the M52 composition test fixes that boundary.
2. **Missing dependency** initially applied `UnavailableExternalReference` directly to the
   dimension, so it did not demonstrate the distinct derived dependency reason. It now marks the
   dimension's supporting curve unavailable and directly verifies both the curve's
   `unavailable-external-reference` and the dimension's `unavailable-dependency` evidence.

3. M52 evidence initially reused the generic typed-host envelope and therefore included canonical
   workspace design/accepted JSON. Its dedicated crate-private path now emits identities and
   revisions instead, and the deterministic evidence regression rejects both canonical fields and
   every fixture checkpoint document.
4. Product isolation initially had no direct save/exit/reload owner, and the first correction
   tested separate gates rather than the production transition. `UatWorkbenchState` now owns the
   actual sidecar used by the WASM workbench for load, typed actions, ordinary-action admission,
   save decisions, rendering and exit. Its native regression proves active-sidecar save
   suppression, **New** and other ordinary-action rejection, an unchanged ordinary checkpoint,
   the real exit transition, exact post-exit persistence and snapshot codec reload equivalence.

Review also replaced clock/nonce-generated fixture document IDs with fixed disjoint namespaces and
now compares evidence from two independently constructed candidates, not merely two captures of
one instance.

## Direct tests

- `m52_candidate_composes_all_ten_preserved_points_without_persistence` executes every typed
  action, checks all ten numbered points and fixes the ordinary-action lock.
- `m52_candidate_directly_qualifies_role_activity_and_mode_distinctions` verifies profile
  participation, active construction geometry, unchanged accepted geometry, suppression,
  reference mode and all distinct activity reasons.
- `m52_candidate_transcript_retains_and_advances_only_at_typed_recovery_boundaries` verifies the
  atomic two-binding/proposal update, parameter retention/recovery, external retention,
  declaration-only rebind, fresh recovery and lifecycle retention/recovery.
- `m52_candidate_evidence_is_deterministic_and_contains_typed_inputs` compares separately created
  candidate replays; fixes exact typed parameter/external, accepted/attempted and M53-boundary
  evidence; and rejects canonical workspace fields and fixture checkpoint documents.
- `m52_sidecar_isolates_the_production_workspace_snapshot_flow` drives the same
  `UatWorkbenchState` used by the WASM workbench through load, typed mutation, ordinary action/save
  rejection, exit and persistence-codec reload around an unchanged ordinary coordinator.

## WASM adapter boundary

`geosolve-demo-web` remains a separate `cdylib`/`rlib` consumer. Its only supported WASM adapter
is still `lib.rs::wasm::start`, which obtains the browser `Document` and invokes crate-private
`workbench::wasm::install`. M52 adds no `wasm_bindgen` export and no public UAT/capture API. The
all-feature `wasm32-unknown-unknown` check and release Trunk build qualify that startup consumer;
they do not claim browser interaction, DOM observation or a new public adapter.

## Qualification commands

The commands below are the exact historical M52 qualification record. M53-P011 later removed the
private `workbench::uat` module and migrated its fixtures into the reusable scenario catalog, so
the focused `workbench::uat::` filter is no longer a current runnable M53 command. Use
`docs/M53_UAT.md` and the current release gate for M53 qualification.

Focused commands:

```bash
nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch --test m41 --test m42 --test m43'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features workbench::uat::'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features workbench::panels::tests::m47_'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features workbench::scene::tests::m47_'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features workbench::evidence::tests::typed_host_evidence_serializer_contains_inputs_attempt_and_accepted_evidence'
```

Final commands:

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check'
git diff --check
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo test --locked --workspace --all-features'
nix-shell shell.nix --run 'cargo clippy --locked --workspace --all-targets --all-features -- -D warnings'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
nix-shell shell.nix --run 'cd crates/geosolve-demo-web && trunk build --release'
```

These commands intentionally contain no browser launch, serving, Chromium/CDP, DOM scraping,
screenshot comparison, wall-clock timing/retry, download assertion or source-substring scan.

## Validation record

Parent review passed all focused commands above: M41/M42/M43 passed 9/16/10 tests; the five M52
tests passed; the three M47 panel tests, one M47 scene test and one generic evidence test passed.
The all-feature demo-web suite passed 24 tests. The complete locked all-feature workspace suite,
formatting/diff, warnings-denied workspace Clippy, all-feature WASM check and release Trunk build
also passed. The Trunk command was run from `crates/geosolve-demo-web` as
`nix-shell ../../shell.nix --run 'trunk build --release'`.

No browser launch, server, DOM observation, screenshot, timing/retry, download or source-substring
scan was used. Independent read-only verification passed after confirming the dedicated
identities-only evidence path and the production-used `UatWorkbenchState` load/action/save/exit
regression. M52 is complete and M53 is active.

## Out of scope

Human usability/trust ratings, finding disposition and explicit approval remain M53. Browser E2E,
restored serving infrastructure, persistent product fixtures, schema/storage changes, new solver
or editor semantics, and any supported public UAT/capture export remain out of scope.
