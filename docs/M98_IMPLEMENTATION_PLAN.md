<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 — approved real-world authoring and embedding implementation

Implementation authorized on 2026-09-09 after the prototype review. This supersedes
its fast-track limits; original handoffs remain historical evidence. M97 is accepted
and closed; M98 stays open until integrated qualification and supervising-user acceptance.

## Approved product

- Linux local folder/CLI workflow, complete local TypeScript/patch dependency snapshots,
  source edits in both directions, structured diagnostics and revision-checked agent edits.
- Existing workbench with one bridge owner per canonical folder and one active UI editor;
  explicit handoff, exact pending-edit authority, recovery and capabilities before interaction.
- A headless TypeScript engine usable in Node and browsers without React/demo dependencies,
  plus a dedicated pure Rust WASM adapter over shared validated domain APIs.
- Editable managed source retains its strict compiler/receipt contract. Generator mode uses
  ordinary TypeScript functions returning `sketch(...)`, including loops, conditions, helpers
  and imports; results never grant reverse source-edit/drag authority.
- Both modes use existing SDK builders, explicit identities, units, metadata, patches,
  Rust lowering, reconciliation, computed features and independent residual validation.
- Optional source-declared generator inputs provide inferred types, defaults, labels/help,
  numeric/integer/boolean/string/choice validation and unit metadata. Plain functions remain
  valid for custom hosts supplying complete inputs; `isKeyParameter` retains overview meaning.
- Inspectable design sidecar stores necessary semantic overrides/branches, not solved-geometry
  duplication. Generator input values are separate. Caches/history/view state are derived or
  personal; corruption cannot prevent valid source reconstruction.
- Complete accepted manifold profile export, including computed walls/arcs/caps/seal; named
  output/region selection, bounded model-space sampling and independent topology validation.
- Installable engine/authoring/CLI archives, a maintained agent CLI example, the manifold
  folder and a custom Gridfinity-style website using only headless engine and authoring SDK.

## Ordered work

- [x] Merge accepted M97 descendant into the existing M98 worktree without rewriting history.
- [x] Freeze and fix stale installed-vs-observed authority and broken-derived-cache startup.
- [x] Add SDK runtime recorder and separate validated generator-program admission.
- [x] Extract shared headless engine/native session and dedicated WASM/TS package.
- [x] Add complete project loading/watching and packaged CLI inspect/set/apply/status/recovery.
- [x] Add project lock, editing lease/session epochs, explicit field lifecycles and operation IDs.
- [x] Add journaled recoverable publication, design sidecar, bounded history and view state.
- [x] Complete computed-profile/named-output export through shared engine.
- [x] Polish first-party workbench modes/capabilities, maintain examples and measure navigation.
- [x] Register all new owning-layer/package/browser tests in integrated release qualification.
- [x] Qualify clean candidate, freeze production bytes and verify a reviewable preview.
- [x] Qualify the requested 500 ms canvas loading-feedback amendment and refresh previews.
- [x] Reproduce and repair M98-F016 asynchronous navigation backlog with owning regressions
  and exact-response negotiated compression.
- [x] Qualify and deliver the [navigation latency repair](M98_NAVIGATION_LATENCY.md).
- [ ] Obtain supervising-user acceptance and close M98.

Navigation repair candidate `b1243a6` passes all 261 obligations in
`20260909T221531-9189674e` (11 fresh, 250 authenticated reused). Both previews serve
verified frozen/installed artifacts; all seven original manifold files are preserved and
the new editing lease is unclaimed. Evidence: `target/m98/latency-preview-verification.json`
and `target/m98/latency-static-preview-verification.json`. Human acceptance and closure remain open.

Stages may develop independently behind their explicit APIs, with focused verification before
integration. No new solver primitive/equation, unsafe code, FFI solver, solid kernel, enclosure
design or reusable UI component library is authorized. No automatic package install, remote
code fetch, public registry publication or unrecorded consumer-repository change is required.

## Authority, loading and recovery contract

`createEngine`/`evaluate`/`dispose`, editable sessions, diagnostics, accepted model-space geometry,
named outputs and profile export form the public headless surface. Failed/cancelled/superseded
work cannot replace the last accepted immutable result. Generator records are bounded evaluated
programs, not fabricated managed lexical receipts. Module/input hashes bind provenance, not a
claim that arbitrary JavaScript is deterministic or authenticated by its own hash.

Custom hosts bundle trusted generators. First-party CLI/preview and the generator example use
terminable workers/processes with bounded work, preserving last accepted output. Inline host
functions execute in the caller's JS realm; worker isolation is not a security sandbox.

Every design mutation, including Undo and metadata/extraction, binds installed source/session
revision and immutable draft basis. Background observation is not installed authority. Navigation
must not take full history rollback snapshots, write disk or compile source. New input cancels or
supersedes older work while preserving terminal gesture order and independently accepted state.

File publication stages candidate bytes with expected input identities, retains displaced bytes
and permissions, journals publication/acknowledgment, and reconciles interrupted transactions on
startup without overwriting newer disk data. Operation IDs resolve lost acknowledgments. Provide
explicit inspect/resolve recovery operations. Guarantee stale-edit refusal and recoverable competing
bytes; do not promise atomic CAS against uncooperative external writers holding old file descriptors.

## Acceptance and qualification

- Loading feedback: no flash below 500 ms; grey canvas and accessible status during slow
  folder and standalone solves; retained accepted geometry on success/rejection; continuous
  queued/prepared work, external disk evaluation and safe captured-pointer termination.
  [Implementation and amendment evidence](M98_LOADING_FEEDBACK.md) records current progress.

- Deterministic filesystem fault points: staging/displacement/publication/acknowledgment; external
  rename and in-place writes; missing files, failed writes, stale lock, corrupt cache and restart.
- Adapter/session: installed vs observed identity, field draft text, metadata/textarea/extraction,
  stale Undo, lease handoff, session epoch, reconnect and exactly-once acknowledged operations.
- Generator: loops/conditions/helpers/count changes, explicit IDs, nested outputs, optional inputs,
  forged/wrong-kind/duplicate references, nonfinite/invalid geometry, cancellation and no reverse edits.
- Equivalent editable/generator and Node/browser results use independent accepted-geometry evidence.
- Multi-file manifold: external patch edits, shared parameter/metadata GUI writeback, drag sidecar,
  Undo/Redo/restart, rejected source preservation and complete independently checked baked regions.
- Preserve original Pi/circle bake semantics and v1 fields; add complete-project/input provenance,
  named profile selection, supported computed geometry and explicit unsupported/partial failure.
- Real Chromium workflows plus a host-owned generator website; small/Gridfinity/manifold/dense
  performance measurements, bounded queues, zero navigation disk writes/recompiles.
- Focused checks during development; integrated clean-source format/Clippy/native/WASM/package/
  browser/performance gate at nomination, authenticating unchanged evidence only. Golden expansion
  requires row-by-row review and owning-layer defects use the hardening skill.

## Evidence ledger

[M98-F001/F002 focused hardening](M98_HARDENING.md) records the reproduced
failures and current transport/cache/session repairs with focused owner checks. Final
[qualification and preview](M98_QUALIFICATION.md) include the loading amendment on clean `a68fffa`
with all 261 obligations passing; the early checkpoints below are historical and supervising-user
acceptance remains open.


Merge `1584a5a` integrates M97 `3152f33` with the earlier M98 prototype. It resolves additive
bridge modules, preserves folder draft tracking and M97 metadata controls, and imports accepted
M97 docs. This is integration source, not new qualification. The accepted M97 artifact stays at
`http://100.94.63.83:18105/` and is never rebuilt by this milestone.

The checked implementation items have focused owner evidence, not milestone acceptance.
Native/session/profile implementation is recorded in [M98_ENGINE_IMPLEMENTATION.md](M98_ENGINE_IMPLEMENTATION.md).
The [authoring quickstart](M98_AUTHORING_QUICKSTART.md) provides a complete agent handoff/edit/retry
workflow and a small generator host. The current transaction/worker/cache findings and exact regressions are recorded in
[M98_HARDENING.md](M98_HARDENING.md). Package smoke installs all three archives offline
and runs a browser application using only the installed SDK/engine. Navigation measurements are recorded in [M98_NAVIGATION.md](M98_NAVIGATION.md). Browser/HTTP
migration is complete, including source-draft downloads, actual failed-write recovery, invalid
source retention and route isolation. The final clean gate passes 261/261 obligations, and both frozen-workbench and installed-folder
previews have fresh byte/browser evidence. [The nomination](M98_QUALIFICATION.md) preserves
performance limits and leaves supervising-user acceptance and closure unchecked.
