<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M99 — shared authoring and host infrastructure

The supervising user approved this cleanup for implementation on 2026-09-13,
including coordinated clean breaks to experimental Rust/TypeScript APIs. M98's
human U02 recheck and acceptance remain open; this authorization does not record
those outcomes. Work continues in the existing `m98/file-workspace` checkout.

## Contract

Standalone, folder and collaborative editors share native authoring, source
projection/validation and browsing services. Hosts own their distinct storage,
history and publication policies. Preserve separate working source, accepted
authority, provisional geometry and personal presentation; independent residual
validation, explicit branches and separate navigation/authoring/compiler/text
queues remain mandatory. No new mathematics or golden expansion is planned.

Preserve project formats, sidecars, journals, pending operations, invitations,
personal histories and existing previews. Durable operation formats remain
unchanged. MiniCAD's baked-profile contract stays `geosolve-baked-profile-v1`;
its current consumer scripts migrate to the packaged CLI without rewriting
historical evidence. Replaced experimental API wrappers may be removed once all
consumers migrate.

## Ordered implementation

- [x] Generate tool projections from native catalogs; centralize name allocation,
  declaration metadata and source preparation in `geosolve-sketch-code`.
- [x] Centralize source/native terminal consistency checks, preserving the union
  of existing validations and host diagnostics.
- [x] Produce an opaque native completed-authoring receipt retaining defining
  inputs, operands, options/branches and created declaration correspondence.
- [x] Share engine preparation, compiler resolution, validation, candidate export
  and installation while keeping specialized operation state machines.
- [x] Extract detached interaction into the editor, source browsing into the code
  owner and read-only accepted-result inspection into the engine.
- [x] Introduce a capability-based workbench session; migrate collaboration,
  folder and standalone consumers onto shared authoring services.
- [x] Move production Node hosting into the CLI package; remove Node dependence
  on frontend/demo execution and milestone build paths; share worker mechanics.
- [x] Migrate MiniCAD runners to packaged CLI provenance and baked-profile output.
- [x] Remove superseded implementations and scaffolding after coverage mapping;
  shorten active docs while retaining linked historical evidence.
- [ ] Pass focused owner checks and integrated clean-source qualification;
  verify offline package installation, actual browser readiness and consumer use.

## Acceptance

All 25 construction variants, 13 constraints, five dimensions, Fillet, Offset,
roles and metadata retain authoring/source/identity behavior across hosts and
both prediction locations. Preserve three-point defining inputs, tangent arcs,
all fifteen F041 gestures, explicit branch rejection, cancellation, failure
retention, unrelated geometry, personal Undo/Redo and cold restoration.

Recovery covers concurrent and invalid source, Apply while typing, stale operands,
lost acknowledgements, restart, failed disk publication and late replies after
worker replacement. Keep 500 ms navigation/text budgets, zero drag reversals and
the delayed loading veil. Navigation remains local during a ten-second held
solve and text synchronizes before release. Preserve existing golden assertions.

The installed CLI/generator and MiniCAD pipeline must run without repository
implementation imports or `target/m98` runtime paths. Completion requires removing
duplicate owners, not merely moving files. Follow `RELEASE_QUALIFICATION.md`;
targeted checks are development evidence, not integrated qualification.

## Implementation evidence

Baseline: clean `1925143`, with qualified M98 product `b49e339` and unchanged live
previews. Implementation and exact check results are recorded below as work lands.


The first implementation slices now have single owning paths:

- Native catalogs generate Rust/TypeScript tool identities and browser maps.
- `geosolve-sketch-code::prepare_editor_source_insertion` owns native-order names,
  inserted declaration projection and default dimension intent. `editor_terminal`
  owns the combined terminal proof, including exact provenance/branch checks,
  bounded rectangle roundoff and full mismatch diagnostics.
- `CompletedConstructionReceipt` is private-construction, transient native evidence.
  It retains resolved defining samples, typed operands/options, the exact plan,
  created aliases and origin/terminal authority. Forks/restoration discard it;
  stale terminal identities cannot retrieve it. Three-point source publication
  now uses this evidence; the engine's separate click recorder is removed.
- Engine construction and contextual tools retain distinct state machines over
  shared compiler preparation, validation, candidate export and installation.
- `WorkbenchSession` now owns UI startup, snapshot installation, browser persistence,
  folder receipts and shared-text/recovery capabilities. React no longer imports
  concrete folder/shared adapters to make lifecycle decisions.
- Production Node modules, build and packaging belong to `packages/geosolve-cli`.
  Package exports replace repository runtime imports. Disposable loader/evaluation
  workers share lifetime/cancellation mechanics; queue policies remain explicit.
- MiniCAD's two runners consume installed `geosolve bake` and current receipts,
  preserving v1 profile output and recording package/input provenance.

Reusable browsing work is underway: exact presentation mapping, finite camera and
visibility ownership now reside in the native editor; accepted source navigation
resides in the source owner. Engine accepted inspection reads retained authority
without a solve or workbench reconstruction. Browser and server authoring selection
use the same engine/native selection mapping. Collaboration scene generation now
exports an engine seed directly. The folder semantic worker consumes engine and
compiler package exports; its browser adapter migration and final qualification
remain underway. The CLI no longer imports or packages a separate demo-WASM
execution runtime.

Focused development evidence (all through the pinned Nix shell):

- Shared terminal owner: 11 tests; native construction receipt lifecycle: three;
  engine construction/replay: 15 (two fixture generators intentionally ignored).
- Native camera owner: six tests; source navigation owner: three; demo navigation
  integration: eight, including channel-wall ownership and retained failed source.
- Engine development WASM build and package build; frontend type check and five
  actual-WASM authoring worker tests pass with engine-owned selection mapping.
- UI facade: 79 existing focused tests, 21 owner tests, TypeScript and UI build.
- CLI relocation: offline installed-package smoke 2/2; focused runtime/CLI/structural
  history/artifact checks 35/35; additional storage/loader/session/worker/artifact
  checks 87/87; host/CLI/export checks 17/17; capture checks 17/17; input checks 22/22.
- Shared disposable worker helper: 17 loader/generator/profile-export checks pass.
- MiniCAD: five receipt tests; both 85/90 mm pipelines and both case variants pass
  against the qualified installed M98 packages. Each case variant passed 111 STL
  inspections and 1707 geometry checks. M99 package rerun remains required.

Exact commands and logs are retained in `target/m99/` and the agent handoff files
`tool-catalog-status.txt`, `session-ui-handoff.md` and `minicad-status.txt`.
Intermediate compilation failures are retained in logs; fixes are being qualified
with focused owner checks. No integrated M99 nomination, preview replacement or
human acceptance is claimed. Existing previews and durable user data are unchanged.

### Detached interaction and worker ownership

`geosolve-constraint-editor::detached_interaction::DetachedCanvas` now owns the
finite camera, selection, captured pan, visibility and dimension command lifecycle.
Its host-facing state is read-only; replacement is prepared before the renderer
installs the complete candidate. `DetachedFrameContext` distinguishes navigation
from authored dimension interest. Demo composition consumes the prepared native
frame once, retaining provisional/presence paint without repeating annotation work.

Coverage mapping:

| Removed or narrowed implementation | Surviving owner and coverage |
| --- | --- |
| Demo camera implementation and six mathematical camera tests | Native `camera.rs`, six direct tests; renderer retains transform integration |
| Demo pointer rectangle normalization and capture tests | Native `input_coordinates.rs`, two direct tests |
| Demo local navigation and replacement logic | Native `detached_interaction.rs`; three direct tests plus existing cross-adapter replacement, visibility, paint and authority tests |
| Demo source-navigation index implementation | Source `navigation.rs`; three direct resolver tests and eight host integration tests |
| Engine/demo duplicate terminal consistency | Source `editor_terminal.rs`, combined strict checks and eleven direct tests |
| Private browser selection translation in server authoring | Engine/native detached selection mapping, native inspection and actual-WASM authoring worker tests |
| Repeated Node Worker construction and pending settlement | CLI `worker-lifetime.mjs`; five owner tests plus existing supervisor recovery tests |
| Repeated browser Worker correlation and generation mailboxes | Frontend `worker-channel.ts`; eight owner tests plus existing adapter tests |

Latest focused evidence:

- Native detached interaction: three tests pass, including malformed late wheel
  samples, captured pan, unchanged accepted geometry, stale view rejection and exact
  native curve occurrences (`detached-owner-2.log`). The wheel/resize test moved
  from the demo; no golden assertions were removed or changed.
- The extraction passes the 26-test demo interaction suite in
  `detached-qualified.log` (226.66 s). Subsequent single-preparation render cleanup
  and typed frame context still require final adapter qualification.
- Node worker mechanics: 22 owner/workbench/mirror tests, 23 disposable loader /
  generator / profile tests and four preview-policy tests pass. Full domain/preview
  suites encountered an intermediate debug engine-WASM memory error; this remains
  a qualification issue, not a passing result. Exact commands and separation from
  worker transport are in `target/m99/worker-lifetime-handoff.md`.
- Browser worker mechanics: 56 tests across eight files and TypeScript pass;
  `target/m99/browser-worker-handoff.md` records exact commands and coverage.
- `START_HERE.md` now contains only current ownership, work and qualification rules,
  preserved delivery status and history links. The previous full handoff is retained
  at `docs/history/START_HERE_PRE_M99.md`; historical claims were not requalified.

### Saved source/native history migration

Native workspace and reproduction codecs now belong to `geosolve-constraint-editor`;
source checkpoint admission and the existing compressed v4/v5 source-workspace wire
belong to `geosolve-sketch-code`. The demo delegates to these owners. The exact
archived M70B/M90 payload bytes moved with their owning codec tests.

`EditableSession::open_persistable`, `restore_history` and `export_history` retain
complete native source history. `restore_source_workspace` / `export_source_workspace`
and their WASM/TypeScript adapters carry personal origin, selected file, unfinished
text and compiler diagnostics separately from accepted source/native authority.
The server's ordinary ephemeral session remains available. Restoring a duplicate
live session identity is rejected without replacing the installed authority.

The engine's accepted-result cache now reads `SketchCodeSession::retained_snapshots`
directly; it no longer serializes and parses the private history schema to discover
retained native handles. Cache identity includes accepted source identity as well
as native checkpoint identity, preserving source/document presentation when equal
geometry occurs in different source states.

Focused evidence:

- `workspace-codec-qualified-2.log`: 37 native workspace persistence tests and
  ten reproduction-filter tests pass (four overlap with the workspace inventory).
- `engine-history-tests.log`: four engine history tests and four native WASM
  adapter tests pass, including both history directions, corruption rejection,
  immutable failure retention and live identity collision rejection.
- `source-workspace-tests.log`: five engine history/workspace tests, four native
  WASM adapter tests and engine TypeScript build pass. The source-workspace test
  covers v4/v5, exact native geometry/source/design/history, unfinished Unicode
  text, duplicate-field rejection and source/history mismatch rejection.
- `source-workspace-clippy.log` remains a failed intermediate check on in-progress
  browsing projection lints. Release WASM rebuild and final integrated checks are
  still pending; subsequent migration edits require their own focused validation.

Exact commands use the pinned `nix-shell shell.nix -I nixpkgs=...` environment:
`cargo test --locked -p geosolve-sketch-engine --test editable_session`,
`cargo test --locked -p geosolve-sketch-engine-wasm --test adapter`,
`npm --prefix packages/geosolve-engine run build`.

### Standalone source/session migration

The standalone `CodeProjectWorkbench` now owns the shared engine `EditableSession`.
Its duplicate mutable source/history owner and materialization cache are removed;
native compiler resolution, source publication, point terminals and history use
engine services. The host retains its draft/presentation, native gestures, unique
namespace initialization and legacy non-code workspace support. Existing v4/v5
source/history envelopes remain unchanged. An opaque validated-history receipt
admits every retained native checkpoint once and carries the accepted editor into
engine restoration. Standalone retained failed source remains an explicit host
policy; atomic server edits still reject without replacing accepted state.

Focused checks preserve exact Undo/Redo labels, genuine compiler receipts, stale
and forged rejection, retained failed-source/native history, cold restoration,
source references and constrained companions. They pass 29 source-workbench tests,
seven managed bridge tests, seven terminal bridge tests, three new standalone
engine tests, six managed-authoring tests, fifteen point-gesture tests and thirteen
contextual tool tests. Five construction tests and ten recipe parity tests pass,
covering all 25 recipes with real compiler evidence. Explicit fixture-generator
tests remain ignored. A direct source regression rejects foreign completed-authoring
origins while admitting the exact validated history-free native fork; no partial
identity comparison is used. Exact commands and intermediate failures are retained
in `target/m99/standalone-migration-handoff.md`. Final integrated qualification and
actual optimized browser artifacts remain separate requirements.

### Accepted browsing and shared chrome

`AcceptedBrowsingSession` and `AcceptedBrowsingView` retain one exact accepted
source/native result and independent personal presentation. The demo's browsing
adapter no longer reconstructs a complete `WorkbenchBridge` or
`CodeProjectWorkbench`. Source declaration, control, property and Inspector
resolution belong to `geosolve-sketch-code`; ordinary editing and accepted browsing
share the demo's borrowed chrome, dimension and navigation projections. Browsing
updates reproject retained geometry without a solve, source publication or history.

Exact delegated materialization identity and scene seals remain enforced. Focused
regressions caught and repaired source Inspector identity after a presentation fork,
and navigation transfer between independently sealed renderer/editor scenes. Native
navigation installation preserves selected curve occurrences and explicit empty
Explorer rows. No solver mathematics, branches, priorities or golden assertions changed.

Focused evidence is in `target/m99/accepted-browsing-handoff.md`: the 25-test local
interaction run passed 23 initially, with each of its two failures subsequently
repaired and passing its exact focused rerun. Additional dimensions 7/7, metadata
2/2, source Inspector 2/2, native navigation 1/1 and engine browsing 2/2 pass. Library
Clippy passes. Optimized demo WASM builds, and all four browsing worker tests pass,
including two actual-WASM tests. Development WASM instead faults during its initial
workbench cold materialization, before browsing construction; that artifact-specific
failure remains recorded separately. Final all-target Clippy and integrated release
qualification still belong to the milestone gate.

### Native seed, browser initialization and folder authority

`AcceptedEvaluation::interaction_seed` and `EditableSession::interaction_seed`
export exact accepted scene/bindings, finite fitted viewport and source-owned
point/presence correspondence. Shared source `interaction.rs` retains the original
producer/reference disambiguation; ordinary editing delegates to the same owner.
The seed has no renderer, compiler, mutation capability or UI chrome. Browser
`BrowsingHandle.initialize` enriches dimensions and visibility through existing
read projections, preserving its original geometry, namespace and scene key.
Generated results use native accepted browsing without fabricated managed source.

Collaborative scene jobs now return `{seed}` directly from the retained engine.
The browser constructs its frame, Inspector and catalog locally. Accepted-model
initialization runs outside the navigation queue, preserving the previous canvas
while the independent browsing worker prepares a replacement. Same-revision
initializations coalesce; native stale selection and publication guards remain.

The folder worker is now an engine/compiler semantic host. Pointer, camera,
selection and tool-preview RPCs are removed. Complete native source history and
old outer `geosolve-workbench-presentation-v1` envelopes use shared strict Rust
codecs; v4/v5 source history, unfinished Unicode drafts, visibility and pins remain
separate state. `managedCompilerPatches`, accepted native workspace export and the
existing reproduction codec are shared engine APIs. The CLI packages only its
semantic runtime and browser distribution, with no `assets/demo-wasm` execution
copy. Browser migration and broad recovery checks remain active.

Old journal request identity still includes opted-in personal-view bytes exactly
as M98 did; retries authenticate those bytes without executing them. This retains
existing operation receipt/collision behavior through the experimental RPC break.
The bridge tests independently assert the exact old digest and full rejected
terminal/source/history retention.

Focused evidence (pinned Nix shell):

- `seed-envelope-owner-tests-r2.log`: two engine inspection and four native WASM
  adapter tests pass, covering finite seed dimensions, exact geometry, immutable
  authority, old outer/inner restoration and malformed/duplicate state rejection.
  A missing TypeScript return cast then failed the chained build; subsequently fixed.
- `engine-seed-release-qualified.log`: optimized engine build and 15 actual-WASM
  session, retained-domain and preview tests pass after server scene migration.
- `scene-adapter-focused-r2.log`: 19 frontend adapter tests pass, including held
  browser initialization with local wheel painting, same-revision installation,
  stale source navigation, text, toolbar and retained drag lifecycle.
- `browsing-initialize-wasm-tests.log`: 11 actual-WASM/bootstrap/worker checks pass
  for managed and generated initialization, precise dimensions/navigation/source
  edits and independent personal view reuse. Optional saved-view initialization
  receives its own follow-up qualification.
- `host-release-qualified-r2.log`: latest optimized engine/CLI builds and 11 actual
  session/domain-scene/folder-bridge tests pass, including old journal request
  identity and source/native/disk authority across malformed or stale edits.

The integrated runner now requires `browsing-initialize.test.ts` in the prepared
native frontend inventory; it is excluded from preflight until authentic current
WASM exists. No golden assertions or mathematical behavior changed. M99 still
requires the completed folder browser migration, integrated clean-source gate,
offline installed-product/browser checks and MiniCAD rerun. No human acceptance,
preview replacement or service/data migration is claimed by these focused results.

Engine-only hosts now return accepted native interaction seeds; the browser's
`BrowsingHandle.initialize()` creates `{snapshot, seed, toolCatalog}` through shared
read-only chrome and detached rendering. Managed project/design and generated
artifact constructors both use native acceptance. Generator browsing has no managed
source context. Dimension/visibility enrichment maps back into the original seed
namespace while preserving scene, bindings and semantic targets. The optional
existing presentation envelope restores saved visibility and dimension preferences.

Focused qualification passes the native bootstrap and two engine browsing tests,
12 actual-WASM/worker tests, a final three-test bootstrap rerun, and demo/engine
all-target Clippy. The optimized WASM includes this initializer. Exact commands,
intermediate artifact mismatches and final evidence are retained in
`target/m99/browsing-initialize-handoff.md`. Folder and collaboration lifecycle
integration and the final milestone gate remain separate requirements.

### M99-F001 — authoring across retained and reconstructed history

Independently reproduced on the M99 changes based on `1925143`: apply circle
source, drag its center to `[3,-2]`, open the resulting project/design in a fresh
engine and finish a polyline. The warm folder server rejected that exact-basis
command because native-generated labels used private history revisions (`0003`
versus `0002`). The native regression also reproduces the same mismatch for a
segment. Adjacent Fillet replay exposed the same counter in computed-feature
labels. This is an authoring authority defect; no residual or equation changed.

Exact replay remains the first path. If declarations differ, the engine rebuilds
the trusted source/design basis and authenticates the complete original command.
Shared source projection then retains only independently verified default labels
on unrenamed new native nodes; a second complete declaration comparison still
checks operands, branches, source names and every other metadata field. Fillet and
Offset replay with the authenticated original native feature label, preserving
exact computed-feature comparison. Historical replay shares that same path.
No client-supplied label override API is exposed. The additional cold evaluation
occurs only on a replay mismatch, off the canvas/navigation path.

The focused owning regressions cover segment, polyline, circle, rectangle,
dimensions, a relation, Fillet, Offset and role changes. They use real compiler
receipts, reject forged labels/comments without altering complete saved history,
retain finite independently accepted geometry and verify Undo/Redo. Initial
failures and harness fixture corrections remain in `warm-cold-construction-*`
and `warm-cold-tools-*` logs under `target/m99/`. The two exact regressions pass;
collateral owner checks and optimized-WASM integration are running separately.

Browser personal preferences now export through
`DetachedCanvas::export_presentation_json` / `InteractionHandle.exportPresentation`.
The existing native presentation codec owns visibility and exact dimension pin
identities; browser storage owns saving the opaque result. Source, semantic
history, selection and camera do not enter that saved preference envelope.

### M99-F002 — retain invalid GUI source without publication

A public v2 folder HTTP regression independently reproduced a failed `source.prepare`
returning HTTP 400 and rolling away its intended diagnostic draft. The new engine
correctly refused canonical export of invalid text, but old host publication code
called that export before checking whether the snapshot was accepted. A second
assertion proved Revert could unnecessarily rewrite unchanged disk source using
canonical formatting. The host now exports only accepted clean publication
candidates and treats source Revert as a draft-only operation.

`invalid v2 GUI source retains its diagnostic draft without publishing or rolling
it back` checks retained Problems/text, exact accepted seed/design/history,
unchanged disk bytes/hash/write count, refused canonical export and clean Revert
without a write. Initial failures remain in `folder-invalid-gui-before.log` and
`folder-invalid-gui-revert-before.log`. The source/compiler rules and storage
rollback assertions are unchanged; collateral publication checks are recorded
in `folder-publication-final.log`.

### Final host integration development evidence

All commands use the pinned Nix shell above:

- `cargo test --locked -p geosolve-sketch-engine --test construction --test tool_operations`:
  20 passed, two explicit compiler-fixture generators ignored;
  `authoring-owners-final.log`. Its subsequent Clippy found a 105-line test;
  a shared two-edit loop removed the duplication. The exact all-target
  demo/engine Clippy rerun passes in `authoring-clippy-r2.log`.
- Optimized engine and demo WASM, CLI bundle, `node --test scripts/workspace-engine-runtime.test.mjs`
  (4/4), actual `browsing-initialize.test.ts` (3/3, including exact saved pin
  export/reopen), and full frontend TypeScript pass in `authoring-and-personal-wasm.log`.
- Folder/browser worker unit suite 51/51 plus TypeScript passes;
  `folder-browser-handoff.md`. App generator and collaboration adapter 21/21
  pass in `app-and-scene-r3.log`.
- `node --test scripts/workspace-browser.test.mjs` against the freshly bundled
  optimized assets passes 11/11 in 96.01 s (`browser-folder-final.log`). This
  includes held-server local input, exact point samples, three-vertex native
  construction across zoom, source/disk recovery, generator feedback and a real
  manifold width edit exporting all 18 regions.

The dense sample now performs one browser cold reconstruction before first canvas
presentation. Measured final opening was 6.484 s; earlier diagnosis measured 7.30 s,
including 5.25 s in browsing initialization. This is a known startup cost, not an
input-latency claim. The manifold harness now explicitly allows 30 s for accepted
canvas setup within its existing 180 s whole-case deadline, matching the asynchronous
readiness distinction already recorded in `M98_LOADING_FEEDBACK.md`. Its implicit
five-second locator failure remains recorded. Navigation, busy-veil, authoring,
geometry, history and export assertions retain their existing budgets and values.
No second raw-scene snapshot owner or server UI execution was introduced.

All 19 current folder/CLI test owners are required by the integrated runner. Exact
relocated source/helper capture, real private package-export resolution and
preparation invalidation tests pass (runner 20/20, inputs 22/22); see
`folder-inventory-audit.md`. These focused checks still do not constitute clean
integrated qualification. Installed-product/browser and MiniCAD M99 verification
and the integrated gate remain outstanding.

`folder-publication-final.log` passes all 23 project/publication checks, including
M99-F002, failed staging/export/acknowledgment rollback and dense manifold edits.
`package-installed-final-r2.log` passes both offline package tests (33.15 s),
including actual installed folder/shared/generator browser startup and native
local drafts that leave disk source unchanged. The first package attempt used
`sketch.ts` for the generator fixture; its corrected test uses the declared
`generator.ts` entry. Product bytes were unchanged by that fixture correction.

Initial preflight run `20260913T134809-4dc8ff73` failed one of 186 runner tests:
a historical assertion still counted three native frontend runtime patterns.
The mandatory browsing bootstrap made four. The runner regression now requires
those exact four patterns explicitly; its focused suite passes 16/16. No test
family was omitted and the failed attempt remains failed.

Preflight `20260913T135018-a662db92` passes all five stages: inventory (186 runner
and 13 oracle-harness tests), metadata/format, managed, frontend and catalog/build
contracts. The implementation is locally checkpointed for clean integrated
nomination; the complete gate and final MiniCAD consumer run remain open.
