<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M100 — final cleanup and pause readiness

**In progress.** Prepared on 2026-09-13 after M99 acceptance, then expanded to a
repository-wide public documentation pass. This document owns the remaining
maintenance work and acceptance contract. [M99 closure](M99_CLOSURE.md) and
[qualification](M99_QUALIFICATION.md) remain the accepted product baseline.
Documentation cleanup alone does not complete the code, tooling, retention or
restart work below.

## Outcome and boundaries

A returning maintainer should be able to identify the current product, build or
install it, run focused and integrated checks, open the supported editor modes,
and restore a saved project from maintained instructions without depending on
untracked helper code. Keep the codebase, retained artifacts and active documents
small enough to understand and maintain during a pause.

Retain M99's native catalogs, opaque authoring receipts, complete terminal proofs,
`EditableSession`, accepted browsing, detached interaction, worker mechanics and
`WorkbenchSession`. Separate construction/contextual-tool state machines and
standalone/folder/collaboration storage/history/publication policies are useful
infrastructure. No new solver/domain features, protocol redesign, generic plugin
framework or broad performance project belongs to this milestone.

Persisted formats, source text, branches, accepted authority, journals, invitations,
pending operations, personal histories and compatibility admissions remain exact.
Old folder-manifest admission and personal-view bytes in journal request identity
protect recovery; they must not be deleted as historical scaffolding. No mathematical
or golden expansion is planned. Use the defect skill whenever investigating a
suspected owning-layer failure or expanding golden coverage.

## Starting inventory

The preparation audit examined clean closeout `6408764` over qualified product
`eb4d2e0`. These are observed maintenance items, not new correctness findings:

| Item | Evidence and disposition |
| --- | --- |
| Documentation | Current guides, API references and historical records are being consolidated for public use. Preserve math/contracts, attribution and genuine acceptance outcomes. |
| Misplaced shared utility | `packages/geosolve-engine/src/point-gesture.ts` owns `decodePointValue`, imported by unrelated session/construction/tool modules. Move privately to a neutral owner. |
| Repeated compiler mechanics | CLI `src/workspace-runtime.ts`, browser `pending-managed-mutation.ts` and CLI `runtime/collaboration-domain-worker.mjs` repeat source compilation and receipt assembly. Compare exact contracts before consolidating. |
| Permanent tests named after M98 | `scripts/release_gate_m98.py` and `scripts/package-m98.test.mjs` are active host/package infrastructure. Make ownership discoverable; stable receipt/stage identities need no cosmetic rename. |
| Untracked operational helpers | M99 final receipt verification, evidence summary and authenticated archive install use scripts under ignored `target/m98`/`target/m99`. Move the necessary reusable operations into maintained tooling. |
| Evidence/build storage | Audit retained receipt dependencies, installed archives and project state before identifying disposable generated copies. Keep host-specific inventories in private local records. |
| Tight WASM margin | Qualified demo WASM is 20,940,556 bytes, 30,964 bytes below 20 MiB. Inspect remaining duplicate reachability; preserve numerical optimization and all existing limits. |
| Dense startup cost | Earlier focused manifold opening took 6.484 s. Preserve that limitation and the passing local-input budgets; profile cold initialization separately from navigation. |
| Multiple checkouts and retained references | Record branch/worktree reachability before cleanup. Never infer disposability from age, path or milestone number. |

## Ordered implementation

### 1. Freeze ownership, preservation and check routing

- [ ] Record current product/source, installed archives, live service data roots,
  branch/worktree reachability and the exact retained qualification dependencies.
  Keep private invitation/token/state locations in local records, not public docs.
- [ ] Classify each proposed edit as a shared owner, intentional host policy,
  durable compatibility path, or removable scaffolding, with an owning test family.
  Freeze this small worklist before implementation; avoid an open-ended repository rewrite.

### 2. Finish the small code-boundary cleanup

- [ ] Move immutable native JSON decoding out of `point-gesture.ts` into one internal
  utility. Preserve result shapes, recursive freezing, public exports and native wire.
- [ ] Consolidate identical compiler-to-receipt mechanics in the existing compiler
  package after comparing custom artifacts, preparation identity, diagnostics and
  cancellation at every caller. Retain each host's failure, abort and publication
  policy. If a path has a distinct contract, document that difference and leave it
  explicit instead of adding flags to a universal host abstraction.
- [ ] Remove only proven redundant wrappers/imports from the migrated paths.
  Preserve public/domain boundaries and all exact forged/stale/cold-history checks.
  Large native files are not automatically defects; split only where it makes an
  existing responsibility independently navigable, without a new public API.
- [ ] Inspect demo-WASM reachability and cold browsing preparation once. Remove
  demonstrable duplicate work/code if found; otherwise record the measured costs
  as pause limitations. No solver changes, weakened size ceilings, artificial
  startup timeout changes or new frame-rate promise.

### 3. Make operation and qualification reproducible

- [ ] Provide maintained entry points for final receipt authentication, evidence
  reporting and exact offline archive installation, preferably within the existing
  release tooling. Remove dependence on ignored milestone helpers for current tasks.
  Preserve archive/installed-file hashing, clean/unchanged-source checks, full
  receipt provenance and offline dependency capture; reject incomplete/tampered input.
- [ ] Document the owning host/package test inventory and rename implementation
  modules only where it improves discovery. Update runner capture/import tests for
  any move; retain mandatory obligations, historical stage identities and
  authenticated reuse. Keep `--fresh` available.
- [ ] Prove the documented source workflow from a clean scratch checkout without
  copied `target` helpers, and the installed workflow from exact local archives
  without repository runtime imports. Document online build prerequisites separately
  from offline installed use and keep toolchain/version pins reproducible.

### 4. Simplify current docs and preserve restart state

- [x] Complete the repository-wide documentation pass: README product purpose and Pages
  demo link; current build, authoring and architecture guides; package/example and
  ADR navigation; consolidated milestone history. Remove private operational details
  and stale current claims. Preserve detailed mathematics, regression IDs, licences
  and acceptance truth; check links, commands and documentation consumers.
- [ ] Write one restart handoff covering source/product identity, build/install/check
  commands, artifact identity, editor modes, state storage, backup/restore, safe
  service restart and deferred work. Generic procedures must live in tracked source;
  machine-specific paths and credentials remain private.
- [x] Preserve an explicit human-acceptance ledger. M99 is closed. M98 U02 remains
  **Fail pending human recheck** and other unperformed human rows remain **Not run**;
  no automated replay can rewrite those outcomes. The ledger may carry these into
  the pause as unresolved, without blocking code cleanup or inventing signoff.
- [x] Inventory artifact retention with a dry run. Preserve accepted archives,
  current installation, required signed receipts/key and their evidence closure,
  live preview assets/data, source/journals/history and unique branch commits.
  Prune only enumerated disposable build/cache/scratch copies after checking those
  references; record measured reclaimed storage. Never delete all of `target`,
  infer disposal from a directory's milestone name, or remove an archive merely
  because it is not the active worktree.

### 5. Qualify and prepare the pause handoff

- [ ] Run focused owning-layer checks as changes land; preserve the existing golden,
  finite geometry, independent residual validation and complete failure retention.
- [ ] Nominate clean source through the integrated release gate once, resuming only
  through authenticated unchanged-input reuse after independent failures. Pass
  format, Clippy, native/headless, optimized WASM, browser, package and performance
  obligations with their existing assertions.
- [ ] Install the exact qualified archives offline and run the supported folder,
  shared and generator startup checks plus both downstream CAD pipelines against that CLI.
  Verify actual bytes/MIME/base paths/WASM readiness for any newly served endpoint.
- [ ] Perform a bounded saved-project restart/recovery exercise using copies of
  real supported fixtures. Recheck source, accepted geometry, history and personal
  presentation before/after restore, including failure/retry data.
- [ ] Publish the local final evidence index and pause handoff, with explicit product,
  tests, measured limits, preserved data and outstanding human dispositions. Use a
  documentation-only closeout after product qualification. Milestone closure and
  any deployment/service-retirement decisions remain separately recorded actions.

## Completion criteria

M100 is ready for signoff when the frozen cleanup list is either implemented and
qualified or explicitly justified as an intentional retained boundary; the documented
fresh-source and installed workflows run without ignored helper scripts; exact
state survives the restart exercise; removable artifact storage has a preservation
audit; and the complete clean-source gate plus installed downstream consumer use pass.

No closure claim depends on a new five-second startup target or unmeasured scaling
promise. No blanket type/tool reimplementation, mass test renumbering, format
migration, branch-history rewrite, public push or preview replacement is implied by
this preparation. M100 should end with a useful, honest resumption point for the
project pause, not an expanding feature backlog.

## Documentation pass — completed

The public README now leads with the embeddable Rust/WASM solver, headless UI
adapter, bidirectional TypeScript authoring, browser demo and local/shared server.
The documentation index, Getting started, Authoring and Development guides provide
current workflows. Root architecture/acceptance/roadmap/handoff guidance has been
reduced from 17,643 lines to about 300, with detailed numerical contracts retained
in references and a linked milestone index. Package/example guides and ADR navigation
are updated. Historical machine details, private endpoints, process inventories,
repeated artifact dumps and stale current-status claims have been removed or
consolidated. Historical finding IDs and human-review dispositions remain intact.

This slice changes Markdown only: no mathematical behavior, API implementation,
solver tolerance, protocol, fixture oracle, live service or qualified artifact changed.
Licences, attribution and the identified Rust-embedded/fixture Markdown remain
byte-identical to the M99 closeout. Package READMEs and compatibility documentation
will be new package inputs at the next product nomination.

Validation:

- Repository-wide audit of 274 Markdown documents: local links and heading fragments
  resolve; private host/path/token and code-fence checks pass. Historical scenario
  and finding identities remain discoverable in the retained documentation.
- `git diff --check` passes.
- `node packages/geosolve-cli/bin/geosolve.mjs check examples/file-workspace` passes
  with independently validated hard residual zero in the prepared development shell.
- The exact README TypeScript block was written to a disposable initialized folder.
  `node packages/geosolve-cli/bin/geosolve.mjs inspect target/m100/readme-example`
  and `node packages/geosolve-cli/bin/geosolve.mjs check target/m100/readme-example`
  both pass, including independent hard validation and the expected single curve.
- `curl -I -L --max-time 20 --silent --show-error https://arduano.github.io/geometric-constraint-solver/`
  returns HTTP 200 for the linked demo. This verifies reachability, not a new deployment.
- `./scripts/release-gate.sh --docs-only --since 5cc48b1` rejects this broader diff
  as outside its reviewed prose set: package/example READMEs and CHANGELOG are not
  admitted by that mode. Its rules were not changed. The independent documentation
  checks above pass; no integrated product qualification is claimed for this slice.

Build/startup commands were cross-checked against package scripts and CLI option
handling, including the full-workspace Rust 1.90 requirement, SDK dependency setup,
separate WASM builds and ordinary-folder `GEOSOLVE_DIST` versus shared `--artifact`.
A full fresh-checkout build, newly packaged installed-product checks, final integrated
qualification and the copied-state restart exercise remain in the worklist above.
M99 stays accepted; M98 U02 remains Fail pending human recheck. M100 remains open.

## Acceptance storage — implementation and cleanup

`scripts/release_storage.py` now owns dry-run audits, explicit pruning, signed
whole-run and delivery-stage pins, and retained-evidence verification. Its
checkout-local policy is in `scripts/release_storage_policy.json`; the
[storage guide](RELEASE_STORAGE.md) documents operation and retirement. The
release gate shares its lock with cleanup and internal preparation, prunes
before/after qualification, enforces admission/post-run budgets and monitors
the filesystem reserve during stage execution. The storage program, policy and
tests are qualification inputs; reviewed equivalence pins were not weakened.

New golden observations dispose of private compiler caches. Package/host
execution disposes of copied repositories and npm caches even on failure,
timeout or handled interruption, retaining diagnostics and proof first. The
parent cleans stopped private runtimes before sealing evidence. These are
test-infrastructure changes: no solver equations, public domain APIs,
tolerances, protocol, golden rows or acceptance assertions changed.

The applied audit retained complete M98 UAT and accepted M99 qualification
dependencies, including cross-run donors, browser provenance and captured
symlink targets. Delivery-stage pins retain package/browser evidence for nine
offline deliveries. Source, project state, installed products and live previews
were excluded from deletion candidates. The cleanup removed 71 unreferenced
preparation payloads, 2,478 old stage payloads and six known Cargo cache trees.

| Measurement | Before | After |
| --- | --- | --- |
| Checkout `target` allocation | 472.496 GiB | 115.493 GiB |
| Acceptance store allocation | 278.763 GiB | 98.260 GiB |
| Filesystem free space | 360.040 GiB | 556.356 GiB |

The observed filesystem gain is 196.316 GiB. Shared extents/hard links mean
directory allocation, projected reclamation and actual free-space gain differ.
The retained store contains 605 stage payloads and 38 preparation payloads;
its old signed scratch remains part of immutable evidence. Automatic limits
are 128 GiB for the store, 160 GiB for target, and a 32 GiB filesystem reserve.
Size limits apply before/after a run, not as hard runtime quotas. Pins are
never silently retired to meet a budget.

Validation for this slice:

- `PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s scripts/tests -p 'test_*.py'`
  passes 210 tests in 89.113 s, including 19 destructive-boundary/retention tests,
  host-stage failure/timeout cleanup and the existing release harness.
- `PYTHONDONTWRITEBYTECODE=1 python3 scripts/golden_oracle_test.py` passes 13
  harness tests; the reviewed 271-row golden is unchanged.
- `python3 scripts/release_gate.py --check-inventory` passes.
- `nix-shell shell.nix --run './scripts/release-gate.sh --plan --preflight'`
  passes in the recorded development environment, retaining all five preflight
  obligations. This is a read-only plan, not product qualification.
- `python3 scripts/release_storage.py audit --build-cache --json` produced the
  reviewed candidate list; `python3 scripts/release_storage.py verify` passed
  before deletion: 607 authenticated receipts, 20,557 evidence files and 20
  captured output trees. `python3 scripts/release_storage.py prune --build-cache --apply --json`
  applied a newly computed plan under the exclusive lock and recorded the
  measurements above. The same verification command passed after deletion with
  all 607 receipts, 20,557 evidence files and 20 output trees still valid.
  A final audit has no remaining deletion candidates and passes all three
  store/target/free-space budgets.
- Independent before/after hashes match for all 17 frozen delivery trees.
  All eight inventoried Node preview/server processes retain the same PID and
  exact command arguments. Local inventory paths and process details remain
  in ignored private records.
- `python3 -m py_compile scripts/release_storage.py scripts/release_gate.py scripts/release_gate_m98.py scripts/golden_oracle.py`
  and `git diff --check` pass. The documentation audit covers 275 Markdown
  files and more than 1,200 local links with no broken paths/fragments or private host details.

This focused infrastructure slice does not nominate new product bytes. Final
clean-source format/Clippy/native/WASM/browser/package/performance qualification,
the maintained installer and copied-state restart exercise remain M100 work.
M99 remains accepted; M98 U02 remains Fail pending human recheck.
