<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M100 — final cleanup and pause readiness

**Prepared on 2026-09-13; implementation has not started.** The supervising user
accepted and closed M99, then requested this final cleanup milestone before a
temporary project pause. This document is the bounded implementation plan and
acceptance contract. [M99 closure](M99_CLOSURE.md) and
[qualification](M99_QUALIFICATION.md) remain the accepted baseline.

## Outcome and boundaries

A returning maintainer should be able to identify the current product, build or
install it, run focused and integrated checks, open the supported editor modes,
and restore a saved project without reconstructing agent history or depending on
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
| Stale current docs | Main `README.md` still calls M93 next; engine package README says M99 is in implementation. Replace current guidance, preserve linked historical records. |
| Misplaced shared utility | `packages/geosolve-engine/src/point-gesture.ts` owns `decodePointValue`, imported by unrelated session/construction/tool modules. Move privately to a neutral owner. |
| Repeated compiler mechanics | CLI `src/workspace-runtime.ts`, browser `pending-managed-mutation.ts` and CLI `runtime/collaboration-domain-worker.mjs` repeat source compilation and receipt assembly. Compare exact contracts before consolidating. |
| Permanent tests named after M98 | `scripts/release_gate_m98.py` and `scripts/package-m98.test.mjs` are active host/package infrastructure. Make ownership discoverable; stable receipt/stage identities need no cosmetic rename. |
| Untracked operational helpers | M99 final receipt verification, evidence summary and authenticated archive install use scripts under ignored `target/m98`/`target/m99`. Move the necessary reusable operations into maintained tooling. |
| Evidence/build storage | Read-only `du` reports release store 279 GB, debug build 152 GB, release build 15 GB, M98 4.2 GB and M99 696 MB. These are inventory measurements, not promised reclaimable bytes. |
| Tight WASM margin | Qualified demo WASM is 20,940,556 bytes, 30,964 bytes below 20 MiB. Inspect remaining duplicate reachability; preserve numerical optimization and all existing limits. |
| Dense startup cost | Earlier focused manifold opening took 6.484 s. Preserve that limitation and the passing local-input budgets; profile cold initialization separately from navigation. |
| Continuation lives in a worktree | Use `m98/file-workspace` at its existing path. The primary checkout and `/tmp/geosolve-m91-oracle` are preserved references, not cleanup targets. |

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

- [ ] Give the main README a concise product overview, supported entry points,
  current limitations and links. Preserve milestone chronology in linked history.
  Add a concise current architecture/API map and update package/example instructions.
  Keep detailed mathematical contracts and acceptance history intact.
- [ ] Write one restart handoff covering canonical branch/path, build/install/check
  commands, artifact identity, editor modes, state storage, backup/restore, safe
  service restart and deferred work. Generic procedures must live in tracked source;
  machine-specific paths and credentials remain private.
- [ ] Preserve an explicit human-acceptance ledger. M99 is closed. M98 U02 remains
  **Fail pending human recheck** and other unperformed human rows remain **Not run**;
  no automated replay can rewrite those outcomes. The ledger may carry these into
  the pause as unresolved, without blocking code cleanup or inventing signoff.
- [ ] Inventory artifact retention with a dry run. Preserve accepted archives,
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
  shared and generator startup checks plus both MiniCAD pipelines against that CLI.
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
audit; and the complete clean-source gate plus installed MiniCAD use pass.

No closure claim depends on a new five-second startup target or unmeasured scaling
promise. No blanket type/tool reimplementation, mass test renumbering, format
migration, branch-history rewrite, public push or preview replacement is implied by
this preparation. M100 should end with a useful, honest resumption point for the
project pause, not an expanding feature backlog.
