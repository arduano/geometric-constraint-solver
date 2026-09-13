<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M98 — approved real-world authoring and embedding implementation

Implementation authorized on 2026-09-09 after the prototype review. This supersedes
its fast-track limits; original handoffs remain historical evidence. M97 is accepted
and closed; M98 maintainer acceptance and milestone closure remain open.

The [approved collaboration amendment](M98_COLLABORATION.md) extends this baseline with
concurrent editors and shared TypeScript authoring. The current [full toolbar candidate](M98_TOOL_PARITY.md)
and F041–F043 repairs are qualified on `b49e339` with 293/293 obligations and delivered
with exact served-byte, browser and preservation verification. [Final signoff UAT](M98_UAT.md)
covers the complete amended scope. The cold free-corner source-reference transport repair
passes all fifteen original gestures and both installed client/server prediction workflows,
including peer visibility, personal Undo/Redo and reload. U02 remains Fail pending human
recheck; acceptance and closure remain open.
The original single-editor implementation remains available alongside the opt-in shared host.

## Approved product

- Linux local folder/CLI workflow, complete local TypeScript/patch dependency snapshots,
  source edits in both directions, structured diagnostics and revision-checked agent edits.
- Original single-editor workbench with one bridge owner per canonical folder, one active
  UI editor and explicit handoff; opt-in collaboration extends it with concurrent invited
  editors, shared raw TypeScript and server-ordered validated model edits.
- Local Rust/WASM canvas navigation, picking, selection and dimension presentation over
  detached accepted scenes, with server-owned sketch edits and exact state reconciliation.
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
- Installable engine/authoring/collaboration/CLI archives, a maintained agent CLI example,
  the manifold folder and a custom Gridfinity-style website using only the headless engine
  and authoring SDK.

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
- [x] Implement and qualify the [local canvas boundary](M98_LOCAL_CANVAS.md), including
  stalled-server responsiveness, shared native semantics and preserved user-folder delivery.
- [x] Implement and qualify the [multi-editor amendment](M98_COLLABORATION.md), including
  shared draft/accepted separation, checked personal history, durable recovery and browser/load proof.
- [x] Verify exact qualified static and collaborative replacement previews.
- [x] Prepare isolated [final UAT](M98_UAT.md) with served-artifact/browser readiness,
  folder/CLI and generator workflows, complete exports and a human result ledger.
- [x] Repair and qualify F041 cold corners, F042 interactive rendering and F043 dense typing;
  replace shared, folder and generator previews while preserving their source and histories.
- [ ] Obtain human U02 recheck; retain its reported Fail until then.
- [ ] Obtain maintainer acceptance and close M98.

The historical collaboration candidate `513463f` passed all 288 obligations in
`20260910T204328-4a05c31a` (32 fresh, 256 authenticated reused; 44m4.890s).
[Its qualification](M98_QUALIFICATION.md#qualified-multi-editor-collaboration) records
signed evidence. Exact production and four archives are frozen and installed offline;
static and collaborative previews passed exact served-byte and browser verification. The [current nomination](M98_QUALIFICATION.md#qualified-uat-repairs)
supersedes that shared candidate: `b49e339`, 293/293 obligations in
`20260913T020232-eaefaaaf` (44 fresh, 249 authenticated reused; 42m41.084s).
All 49 ordinary and 17 collaboration browser workflows pass. Delivery preserved
source, drafts, invitations and personal histories.
Human acceptance and milestone closure remain open.

The preceding local-canvas candidate `d5f9e40` passed all 261 obligations in
`20260910T022428-a1d7c652`. Historical delivery evidence remains in `target/m98/local-canvas-preview-verification.json`
and `target/m98/local-canvas-static-preview-verification.json`.

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

[Focused hardening](M98_HARDENING.md) records reproductions, owning-layer repairs and
nomination harness corrections. [Final qualification](M98_QUALIFICATION.md#qualified-uat-repairs)
records clean `b49e339` with all 293 obligations passing and preserved shared, folder,
generator and launcher delivery. Source-derived branch transport, interactive raster work
and native text ownership improvements retain strict authority and all original budgets.
Maintainer acceptance remains open. The early checkpoints below preserve historical
baseline evidence.


Merge `1584a5a` integrates M97 `3152f33` with the earlier M98 prototype. It resolves additive
bridge modules, preserves folder draft tracking and M97 metadata controls, and imports accepted
M97 docs. This is integration source, not new qualification. M97 acceptance remains recorded separately in [its closure](M97_CLOSURE.md).

The checked implementation items have automated qualification, not maintainer acceptance.
[Engine implementation](M98_ENGINE_IMPLEMENTATION.md) records native/session/profile ownership.
The [authoring quickstart](M98_AUTHORING_QUICKSTART.md) provides agent handoff/edit/retry,
shared folder startup and a small generator host. The current package smoke installs all
four matching archives offline and runs a browser application using only the installed
SDK/engine. [Baseline navigation](M98_NAVIGATION.md) and
[collaboration evidence](M98_COLLABORATION.md) preserve their separate measurements.
The [nomination](M98_QUALIFICATION.md) records current qualification and verified
preview delivery, with maintainer acceptance and closure unchecked.
