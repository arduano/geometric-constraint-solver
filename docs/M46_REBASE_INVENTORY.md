<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M46 cleanup rebase inventory

## Status

Rebase decisions are complete and applied to the authoritative roadmap: M45 completed
without human approval; M46-M53 completed the cleanup/UAT sequence; after M53 approval the
preserved functional sequence was renumbered, and a later supervising-user decision inserted the
new M55 alpha action-parity gate. The mapping below is a historical record and was superseded on
2026-07-29 when M61 closed with approval and the forecast M62-M64 hardening sequence was removed.
Those old numbers below do not define active milestones; M62 and M63 were subsequently scoped and
approved, and the current M64 is an unscoped placeholder.

## Requirements

- Rebase the old forward functional sequence M46--M55 away from the cleanup numbers, then preserve
  its dependency order when inserting the new early alpha action-parity milestone at M55. The
  current exact map is recorded below.
- Introduce cleanup milestones beginning at M46 without presenting displaced functional work as current M46--M55 work.
- Preserve genuinely historical milestone names (completed M1--M45 records and historical M13/M14 playground claims) while changing forward-looking ownership, gates, release order, and UAT placement.
- Inventory all repository text/metadata references to M45--M55 sequencing, acceptance, UAT numbering, release order, and legacy-playground removal; identify phase wording that conflicts with a pre-cleanup/post-cleanup split or relocated human UAT.

## Evidence and source pointers

### Initial authoritative findings

- `PLAN.md:3-5` says milestones execute in order and that the plan is active guidance. Its product summary (`:34-36`) assigns the production-embedding closeout to M33--M55, so forward scope/range wording must be rebased.
- `START_HERE.md:30` identifies `PLAN.md` as the authoritative execution order. `:36-57` names M45 active, describes the M45 cleanup investigation, and says legacy playground removal is planned M51 parity work; this is a direct pre-cleanup conflict.
- `ARCHITECTURE.md:26-35` labels M45 active and M46--M55 planned, while `:476-506` is the roadmap allocation. These are authoritative status/architecture consolidation points.
- `ACCEPTANCE.md:3` makes `PLAN.md` authoritative. It contains forward delivery/release claims at `:59-64`, post-alpha replacement wording at `:80-86`, and the old M45--M55 acceptance sequence (reported by search at `:792-859`).
- `docs/SCENARIOS.md:272-303` defines the M39 workbench qualification and explicitly retains `#/dev/lab` as a temporary advanced playground; this evidence needs a historical-versus-forward designation once cleanup sequencing is decided.

### Authoritative status and roadmap — forward-looking, must change

- `PLAN.md:34-36,84-89,112-115,135-137` uses M33--M55 as the future embedding/program range, assigns post-alpha workbench replacement through M51, promises Rust/WASM support through M55, and fixes the old UAT list. `:1901-2032` is completed M43--active M45 context, but `:2024-2027` assigns full playground removal to M51 and conflicts with cleanup-first wording.
- `PLAN.md:2034-2215` is the complete old functional M46--M55 specification and gate order: diagnostics; jobs; incremental scale; operations; topology; workbench removal/parity; advanced UAT; v5 freeze; integrated UAT; release. Rehome each section intact under its mapped later milestone, then create new M46+ cleanup sections ahead of it. `:2101,2219-2233` also use M55 as a future gate/non-goal boundary and must follow the final release milestone.
- `ARCHITECTURE.md:16,26-35,115-123,175-184,215-222,266-280,456-460,476-506` contains the status taxonomy, old future ownership, companion allocation, staged workbench/removal claim, old UAT list, v5 and prepared-job sequencing, and roadmap allocation. All references from M46 onward are forward-looking except the M45 current-state sentence; change their milestone identities and phase language together.
- `START_HERE.md:12-18,36-65,83-85` is the live handoff: it calls M45 active, says M51 will remove the old playground, gives the old M33--M55 north star/UAT list, and names M10--M55 as current numbering. Preserve the M45/M14 history but replace all post-cleanup ownership and the M51 removal assertion.
- `README.md:9,15,27,31-37` presents old M36--M55 scope, roadmap range, later workbench phases, and four UAT numbers to users; it must be synchronized with the authoritative plan.
- `CHANGELOG.md:16-18` is an Unreleased forward roadmap statement, not historical release evidence; update M33--M55 range and UAT wording.

### Acceptance, UAT, release and scenario documents — forward-looking, must change

- `ACCEPTANCE.md:59-64,80-86,90,148,154` makes M55 the delivery/endurance boundary, says M39--M51 replace the playground, and describes M10--M55 as current. The M8 text at `:154` is a preserved historical record and should retain its then-current allocation with an explicit historical label rather than be silently renumbered.
- `ACCEPTANCE.md:765-790` is completed M44 evidence plus deferred M45 preparation history; preserve it. `:792-859` is the authoritative old M45--M55 gate sequence. In particular `:795` forbids deletion before M51 parity, `:830-834` assigns removal to M51, `:836-852` assigns UAT 3/4 to M52/M54, and `:854-859` places release at M55. These are the highest-risk acceptance edits.
- `docs/SCENARIOS.md:1249,1339-1372` assigns production fixtures through M55, defines M45 UAT-C2 and M52/M54 UAT-C3/C4, and asserts that M40.7/M45/M52/M54 require explicit approval. Keep the completed/historical UAT-C1 and M45 evidence, but relocate forward scenario headings, gate references, and approval numbering to the decided post-cleanup phases.
- `docs/API_COMPATIBILITY.md:15-23,91,107-108` ties the implementation transition, v5 freeze, human gates, platform support and no-C-ABI boundary to M36--M55/M53/M54/M55. These are forward compatibility promises and must move with the mapped later milestones.
- `docs/M33_CAD_CAPABILITY_MATRIX.md:25-27,217-252` and `crates/geosolve-sketch/tests/m33.rs:118-119,142-143` encode M49 and M55 as matrix/coverage labels (`planned_m49`, `unsupported_through_m55`, `conditional`). They are code/test labels directly referenced by documentation; rebase label values, stable IDs, matrix prose, and test expectations atomically.

### Milestone-specific architecture/ADR/implementation documents — forward-looking, must change

- `docs/adr/0027-cancellation-and-prepared-concurrency.md:29,98,112,174,205`; `docs/adr/0025-retained-design-attempt-and-accepted-state.md:16,86,108,114`; and Rust doc comments at `crates/geosolve-sketch/src/document_session.rs:1099`, `:33,164` and `crates/geosolve-sketch/src/document.rs:7634,7662` assign prepared jobs, lifecycle extent, v5 freeze, and pre-freeze wire state to M47/M52/M53. They must move with M56/M61/M62 as appropriate.
- `docs/adr/0028-sketch-operations-and-production-topology-companions.md:27,98,109,168,192` assigns operation/topology ownership and its M55 contract to M49/M50/M55; rebase to M58/M59/M64.
- `docs/adr/0026-immutable-host-inputs-and-external-snapshots.md:123` and `docs/adr/0029-headless-constraint-editor-state-machine.md:70` name M53 as the freeze; rebase to M62.
- `docs/M35_CANCELLATION_LATENCY.md:47` and ADR 0027 `:98` name M48 as later latency improvement; rebase to M57.
- `docs/M41_IMPLEMENTATION.md:12,26,65-66,77,81,83,109`; `docs/M42_IMPLEMENTATION.md:68,221-222,253`; `docs/M43_IMPLEMENTATION.md:5,59,178,313,319,328,347,363,373,379,384-385`; and `docs/M44_IMPLEMENTATION.md:126` contain forward ownership exclusions/hand-offs to M46/M47/M50/M52/M53. These are not historical claims and must be rebased; completion evidence in the same files remains historically named.
- `docs/M46_DIRECT_TEST_REPLACEMENT.md:3,7-20,49-60` already calls itself M46 and directs deletion of M14/M40/M44 E2E only after direct replacement. It is a new-cleanup candidate, not old stable-diagnostics work. Its `:51-52,58` old M51/M53 successor references must be resolved against the new cleanup phase and later schedule.

### Legacy playground-removal and cleanup handoffs — phase-conflicted, must change together

- `docs/M42_IMPLEMENTATION.md:4`; `docs/M43_IMPLEMENTATION.md:68`; and `docs/M44_IMPLEMENTATION.md:4,15-17,159-182` are historical/current M42--M45 dependency and qualification records. Preserve the references to the active M45 checkpoint and incomplete M14 evidence; only their separate future M46+ hand-offs listed above change.
- `docs/M45_CLEANUP_PLAN.md:7-10,24-38,42-67,71-90,94-113` is the detailed current boundary: workbench default; lab transitional; M45 fixture replacement first; old-lab removal at M51 after parity. Preserve its factual investigation/history, but replace every future M50/M51/M53 owner and the claim that cleanup cannot pull removal forward if the new M46+ cleanup plan deliberately does so.
- `docs/M45_UI_CLEANUP_INVESTIGATION.md:7-9,15-20,26-33,39,45-59,63-65,69-71` is evidence/history that the lab remains live and cannot be deleted without replacement. Its ordered future deletion proposal is at `:53-59`; it must be rephased but its evidence must remain historically named.
- `docs/M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md:7-14,42-61,95-115,119-152,165-172,271-295` distinguishes durable/transitional/obsolete tests and currently allocates class-E decisions to M50/M51/M53. Preserve classifications/counts (92 = A40/B29/C3/D4/E16) as historical evidence; reassign only future successor/removal milestone references.
- The temporary handoff was never tracked. Its durable M44/M45 status, incomplete-M14
  evidence and cleanup decisions are consolidated in `docs/M44_IMPLEMENTATION.md`,
  `docs/M45_CLEANUP_PLAN.md` and the archived section of `docs/M53_UAT.md`; no absent
  handoff remains an authority.
- The record formerly named `docs/M45_UAT.md` captured the deferred M45 review, preserved points,
  incomplete M14 status and blockers. M53 consolidated that historical material with its then-active
  human scorecard in `docs/M53_UAT.md`; preserve the M45 facts in its archived section and the M53
  naming for the completed session, findings and approval.
- `scripts/serve-m45.sh:17` was the M45-specific UAT banner at this inventory snapshot. M50
  removed that obsolete server after direct replacement, and M53 uses only a temporary human
  delivery endpoint; do not restore or mechanically rename the old script.

### Secondary forward ranges and release-order wording — must change

- `PLAN.md:35,112,2101,2219`; `ARCHITECTURE.md:16,35,487`; `ACCEPTANCE.md:59,90,148,154`; `START_HERE.md:59,85`; `README.md:9,15`; and `CHANGELOG.md:16` use M55 or M10--M55/M33--M55 as an endpoint/current range. Replace each with the final release endpoint or a phase-neutral phrase.
- `docs/SCENARIOS.md:1249`; `docs/M33_CAD_CAPABILITY_MATRIX.md:26-27,217-252`; `docs/API_COMPATIBILITY.md:17,107-108`; and ADR 0028 `:109` use “through M55” as a support/contract horizon. These must track M64, not cleanup M55.

### Scripts/handoffs and code/test labels

- At this inventory checkpoint, `scripts/serve-m45.sh` remained temporary
  archived-fixture infrastructure pending M50 and `scripts/release-gate.sh` still owned
  the old E2E invocation. M50 subsequently removed both paths; this paragraph is
  provenance, not a current instruction.
- No TOML, shell (other than `serve-m45.sh`), JS/MJS comment/string, or JSON milestone label outside the listed direct references was found by the allowed-text scan. Existing E2E file names (`m14.mjs`, `m40.mjs`, `m44.mjs`) are historical/test artifact names, not M46--M55 sequence labels.

## Decisions / inferred constraints

- **Exact final functional map:**

  | Old functional milestone | Final milestone after M53 | Existing functional scope preserved |
  | --- | --- | --- |
  | M46 | M54 | stable diagnostics and mobility evidence |
  | New insertion | M55 | alpha constraint, dimension and explicit branch-action parity |
  | M47 | M56 | prepared jobs and concurrency |
  | M48 | M57 | incremental solving and production scale |
  | M49 | M58 | sketch operations companion |
  | M50 | M59 | production topology companion |
  | M51 | M60 | advanced workbench completion and direct automated qualification; legacy-playground removal moved to cleanup M50 |
  | M52 | M61 | advanced geometry/topology human UAT |
  | M53 | M62 | API/schema release-candidate freeze |
  | M54 | M63 | integrated release-candidate human UAT |
  | M55 | M64 | production embedding release |

- **Historical names that remain:** completed M1--M44 claims; M45’s deferred-preparation history and final investigation completion without approval; M13/M14 alpha and incomplete-full-M14 evidence; M39/M40/M44 qualification results; dated cleanup-investigation facts; and existing test/script file names. A historical sentence may retain an old number only where it reports an event that occurred under that number and does not schedule a future owner.
- **Forward names that change:** every old M46--M55 heading, gate, target, capability-matrix value, test expectation, ADR future-owner statement, range endpoint, UAT-C3/C4 heading, release dependency, and future removal/successor assignment. “M51 parity/removal” is forward even inside an M45 historical investigation document.
- Treat explicit completed-record language as historical unless it assigns work after M45; do not renumber accepted historical M13/M14, M39--M45 completion/preparation evidence merely because it mentions the playground.
- Treat any statement that says M46--M55 “will,” “targets,” “remains,” “requires,” “through M55,” or assigns a future UAT/release gate as forward-looking and therefore rebase-sensitive.
- M51-specific claims are high risk: they currently combine a future parity/removal requirement with an assertion that the legacy lab is still transitional. Cleanup milestones beginning at M46 must either move that removal work into the cleanup phase or explicitly defer it under its later owner; both statements cannot remain unchanged.
- **Phase conflicts to eliminate:** (1) “cleanup must not pull deletion forward” versus a cleanup roadmap that removes/rehomes the lab; (2) “M39--M51 replace the playground” versus post-cleanup replacement phases; (3) old UAT 3/4 labels at M52/M54 versus relocated human UAT; (4) M53 v5 freeze before old M54/M55 release language versus any inserted cleanup gates; and (5) “through M55” support/non-goal/release claims versus M55 becoming a cleanup milestone rather than the functional release endpoint.

## Resolved decisions in the superseded rebase

- The temporary ten-entry sequence retained scope and dependency order until M53 approval. The
  later M55 action-parity insertion expanded the then-current final sequence to M54-M64 without
  merging unrelated acceptance gates.
- M46-M49 are pre-cleanup replacement/extraction, M50 is the purge cut, and M51-M53 are
  post-cleanup consolidation/candidate/human-UAT work.
- Legacy-playground removal occurs at M50, not M60. M60 owns
  only later advanced-workbench completion over the already-clean application.
- Human UAT 2 relocated to M53; the superseded map assigned advanced/integrated UAT to M61/M63.
- Functional support/release horizons formerly phrased “through M55” used M64 or phase-neutral
  wording.
- The 29-file initial scan remains the provenance inventory; current consistency must be
  proven by a fresh repository search and diff review rather than its original line numbers.

## Out of scope

- No source, test, configuration, or generated-file change.
- No renumbering of historical completion claims absent a demonstrated forward-looking ownership conflict.
