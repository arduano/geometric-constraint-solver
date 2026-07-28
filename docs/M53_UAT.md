<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M53 CAD host-semantics UAT scorecard

## Status and ownership

M45 is complete as the historical cleanup investigation and ten-point capture checkpoint retained
in the archived section below; it recorded no human approval. M46-M51 replaced or retired the old
browser assertions and removed the legacy application/E2E stack. M52 completed the minimal
post-cleanup candidate and objective direct qualification. This file is now consolidated and named
for its sole active purpose: **M53 is active and alone owns the supervising-human ratings and
approval here.**

`PLAN.md` now marks M52 complete, so the scorecard may begin against its release candidate. No blank
rating, automated result or historical M44 observation counts as M53 approval.

## Durable M53 review and change control

This file is the authoritative working record for the M53 human session, findings, UI requests,
retests and approval. `PLAN.md` remains authoritative for milestone order and the displaced future
roadmap. A chat observation or requested edit must not replace either record.

Before changing the candidate in response to UAT, add a ledger row below with the exact observation
or request and classify it as:

- **objective defect** — behavior contradicts a domain/audit contract; add a direct regression
  before the fix and rerun the affected M52 qualification;
- **human clarity/layout** — presentation impairs comprehension or trust; record the visual context,
  implement only the scoped presentation change, rebuild and repeat the affected human observation;
- **future scope** — useful work outside M53; preserve it in the appropriate `PLAN.md` placeholder
  or explicit open question rather than silently broadening M53; or
- **review process** — candidate delivery, evidence capture or scorecard traceability that changes no
  product semantics.

Each row stays open until its disposition, automated evidence where applicable and supervising-human
retest are written down. A material candidate change creates a new candidate identity and invalidates
only the affected scorecard rows; it does not erase prior observations. No request may silently close,
replace or defer another finding, roadmap item or open question.

### Candidate sessions

| Session | Candidate identity | Human access | Browser/OS | State |
| --- | --- | --- | --- | --- |
| M53-S1 | `feat/m53-host-semantics-uat` at base `9002565`, qualified M38-M52 working tree; recorded release distribution manifest SHA-256 `ea878d3b4dceff61a46c68734dc00f627a089e77a9cd02720ce92017b9235af2` | retired temporary endpoint; no longer valid | no human review started | superseded before review: recovery audit found the live watcher had rebuilt `dist` to manifest `868d45822086714cd457bb2e9912d4b57f3aeb44dcc58663e363c365a7195d88`, so the served bytes no longer matched the recorded identity |

S1 used the SHA-256 output of `sha256sum dist/* | sha256sum`, but its watched distribution
changed after recording. Any candidate rebuild must receive a new session row and digest.
Temporary human delivery is permitted for M53 observation; the removed browser E2E/server scripts,
automated DOM qualification and old fixture routes remain prohibited.

### Findings and change-request ledger

| ID | Session/section and exact sequence | Observation or request | Classification | Status and disposition | Regression/requalification | Human retest/rating |
| --- | --- | --- | --- | --- | --- | --- |
| M53-P001 | M53-S1, before setup observation | Preserve every UAT UI request and all other development concerns/questions/plans durably so one request cannot shadow another. | review process | complete: added this ledger, candidate versioning, classification, continuity and retest rules before product review | `git diff --check`, format check and independent read-only documentation review passed 2026-07-28 | not a product rating |
| M53-P002 | M53-S1, before setup observation | The active scorecard name `M45_UAT` is misleading; consolidate it under M53, align the local branch name and verify Git hygiene without losing accumulated work. | review process | complete: renamed the scorecard to `docs/M53_UAT.md`, consolidated historical M45 context there, updated repository references and renamed the no-upstream local branch to `feat/m53-host-semantics-uat` | tracked/untracked whitespace checks, format check and independent read-only consolidation/Git review passed 2026-07-28 | not a product rating |
| M53-P003 | M53-S1, before setup observation | Plan coherent split commits for the accumulated qualified M38-M52 working tree before deciding whether to stage or commit it. | review process | superseded by M53-P006: the path coverage was complete, but the proposed dependency order omitted two documentation files compiled directly by tests/editor code; the interrupted checkpoint also left its first slice staged | independent recovery audit reproduced the first-slice failure at `geosolve-sketch/tests/m33.rs` | not a product rating |
| M53-P004 | Recovery request after M53-S1 was prepared, before human setup observation | Reconcile all surviving work into the main worktree, eliminate dangling untracked or misleading/outdated content, create clean commits, remove child-worktree dependence and finish with a trustworthy UAT-ready candidate. | review process | in progress: commits `a5542da`, `ba711c3` and `5087c41` contain all recovered work; all duplicate/stale worktrees and recovery artifacts are removed and the main worktree is clean | pending full clean release qualification and M53-S2 publication | human review has not started |
| M53-P005 | M53-S1 delivery verification, before human setup observation | The recorded release manifest must identify the bytes actually served for UAT. | review process | in progress: M53-S1 is superseded without ratings because the live `trunk serve --release` watcher rebuilt `dist`; stop the stale watcher after recovery, create a clean release build and record a new candidate session/digest before UAT | local and served HTML/WASM bytes matched each other, but `sha256sum dist/* \| sha256sum` produced `868d45822086714cd457bb2e9912d4b57f3aeb44dcc58663e363c365a7195d88`, not the recorded S1 digest | human review has not started |
| M53-P006 | Interrupted commit consolidation, before human setup observation | Make each checkpoint commit independently buildable and keep compiled documentation inputs with their owning code/tests. | review process | complete: domain commit `a5542da` contains the M33 matrix; editor/workbench commit `ba711c3` contains the embedded M40 JSON; remaining durable records form the documentation checkpoint | exact staged domain tree passed core/sketch/linkage all-feature tests; exact staged editor tree passed editor 58/58, demo-web 24/24, all-feature WASM check and release Trunk build | not a product rating |
| M53-P007 | Recovery content audit, before human setup observation | Durable records and embedded qualification artifacts must distinguish historical M40-M52 evidence from current M53 procedure, remove references to the absent temporary handoff and stop advertising retired browser/E2E infrastructure or old milestones as active work. | review process | complete: retained every uniquely owned/compiled record, compacted the old M40 UAT into approved historical evidence, marked planning/browser material historical, removed ephemeral addresses and replaced every absent/brittle handoff reference with a durable section owner | repository reference scan and `git diff --check` pass; full release qualification remains a parent gate rather than a content-audit blocker | not a product rating |
| M53-P008 | Recovery release-gate audit, before human setup observation | The compatibility contract names five publishable lockstep crates, but the package-content loop checks only four and its prose also says four. | objective defect | complete in `ba711c3`: `geosolve-constraint-editor` is the fifth package-content target and the compatibility prose now says five; publication remains a maintainer action | `cargo package --locked --allow-dirty --list` confirmed `LICENSE` and `README.md` for all five crates; full clean release gate remains pending | not a product rating |
| M53-P009 | Recovery capability-contract audit, before human setup observation | The machine-read M33 capability matrix still labels completed M38 dimensions and path-length behavior as planned/current gaps. | objective defect | complete in `a5542da`: M38 catalog rows use `implemented_m38`; frozen legacy `EqualLength`/`CurveLength` rows point to the separate M38 path APIs instead of claiming M38 still waits | exact staged domain tree passed M33 2/2, M38 11/11 and the complete core/sketch/linkage all-feature suites | not a product rating |
| M53-P010 | Exact staged editor release build, before human setup observation | The supported Trunk 0.21 build must not reject the conventional inherited `NO_COLOR=1` environment value before compiling the candidate. | objective defect | in progress: constrain the release-gate Trunk subprocess to an unset `NO_COLOR`; this changes no product bytes or solver/editor behavior | isolated editor 58/58, demo-web 24/24 and WASM check passed; first Trunk call rejected only `NO_COLOR=1`, and the same exact tree built after unsetting it | not a product rating |

### Preserved development continuity

| Concern, question or plan | Durable owner | Current disposition |
| --- | --- | --- |
| M53 ratings, findings and explicit approval | this scorecard and `PLAN.md` M53 | active; no rating or approval recorded |
| Later diagnostics, concurrency, scale, operations/topology, advanced workbench/UAT and release work | `PLAN.md` M100X-M109X | preserved placeholders; M53 feedback must not silently consume or reorder them |
| Recovered M38-M52 implementation and evidence on `feat/m53-host-semantics-uat` | Git commits plus completed milestone records | supervising caller authorized dependency-safe commits; recovery must finish with all unique work in the main worktree and no untracked files |
| Cargo duplicate `license`/`license-file` metadata warnings seen during release build | existing package metadata | known nonblocking concern; warnings-denied Clippy and M52 gates pass, and M53 does not broaden into metadata cleanup |

Recovery Git snapshot (2026-07-28): the branch has no upstream or configured remote, no stash,
unmerged entry, hidden commit or unique child-worktree file exists, and the main worktree contains
all latest work. The interrupted agent left a 23-path domain slice staged and an exact duplicate
in a detached verification worktree. The supervising caller has now authorized reviewed,
dependency-safe commits and removal of those duplicate/stale worktrees; discarding unique work
remains prohibited.

No other engineering blocker or unresolved M52 objective question is known at M53 entry. Add newly
discovered non-UAT work to this table and its roadmap owner before starting another requested change.

## Preserved verification points

1. Construction geometry remains solver-active while default-profile participation is
   independently controlled and clearly presented.
2. Suppression/reactivation remains distinct from driving/reference dimension mode, with truthful
   inactivity reasons and discoverable recovery.
3. Host-inactive, missing-external and unavailable-dependency states remain visibly and
   semantically distinct without replacing accepted geometry.
4. A shared host parameter updates all bound dimensions atomically, with coherent input revision
   and accepted output-proposal provenance.
5. Invalid-kind and stale-revision parameter batches retain accepted evidence; a later valid
   complete batch recovers and advances accepted state.
6. Missing, stale and topology-incompatible external snapshots retain accepted evidence and never
   trigger implicit topology repair.
7. External topology recovery requires explicit rebind followed by a fresh valid snapshot.
8. Design, latest attempt and accepted identities remain distinguishable across the tree,
   Problems, audit/revision cards and accepted-only canvas.
9. Finding evidence preserves exact typed parameter/external inputs and accepted/attempted
   evidence. A normal browser/OS screenshot is added only when a finding concerns visual layout.
10. Natural role/activation and parameter/external-recovery flows communicate one coherent,
    trustworthy host-state story without stale displays or unexpected geometry movement.

Objective ownership for each point is recorded in `docs/M52_IMPLEMENTATION.md`. The human review
must not attempt to prove revisions, digests, atomicity, solver validity or persistence isolation
by visual observation.

## Archived pre-cleanup record

The original M45 candidate used a broad M44 deterministic fixture, fixture-only controls, browser
E2E, served root URL and downloaded JSON/HTML/SVG evidence. Its focused web suite reported 103
tests and the M44 browser flow reported 6/6. The full historical M14 run remained incomplete by an
explicit supervising-user scope decision.

That record justified preserving the ten points, not retaining its infrastructure. M47 replaced
the broad fixture with direct owned groups; M48-M50 removed the old E2E, server, route and legacy
application; M51 consolidated the survivor. The old **Load deterministic fixture**, retained
URL/server machinery, downloads and browser-script instructions are intentionally not reproduced as
executable current procedure. They are not M52/M53 gates and must not be restored. A temporary
human-only candidate delivery endpoint may be recorded in the session table; it must not become
automated qualification or retained product state.

## Post-cleanup M53 procedure (30–45 minutes)

### Setup and reset

1. Open the release candidate supplied after objective M52 qualification. Record its identifying
   build/revision and the desktop browser/OS used for this human observation; those fields are
   finding context, not automated proof.
2. In the sole workbench choose **Load disposable M52 UAT**. Confirm the panel explicitly labels
   itself ephemeral and says ordinary save is disabled.
3. Use **Reset candidate** before each numbered section. Reset reconstructs the same four fixed
   in-memory fixtures. Do not use **New** as reset; ordinary mutation is intentionally locked while
   UAT is active.
4. Use **Exit UAT** only after completing or recording the current observation. It must reveal the
   pre-existing ordinary workspace. Reload is not a UAT transition; it exits ephemeral state and
   restores only ordinary persisted work.

The panel's numbered text identifies objective expectations and the M53 judgment boundary. If a
presentation is misleading, lossy, incorrect or difficult enough to prevent trustworthy host use,
capture a finding before reset and rate the area Concern or Blocker.

### 1. Role and profile participation (4–5 minutes)

- Reset; choose **Construction**, inspect the accepted canvas, tree, effective activity and
  accepted-profile card, then choose **Profile**.
- Expect construction styling and removal/recovery of profile participation while the curve stays
  active and accepted geometry does not unexpectedly move.
- Judge whether role versus solver activity versus profile participation is understandable.

### 2. Activation and dimension mode (5–6 minutes)

- Reset; choose **Suppress**, **Reactivate**, then **Reference**.
- Expect user suppression/reactivation to be shown independently of driving/reference mode.
- Choose **Host inactive**, then **Missing dependency**. Inspect the directly unavailable curve,
  the dependent dimension reason, lifecycle and accepted canvas.
- Judge whether host inactivity, unavailable external input and derived dependency loss are
  distinguishable and whether recovery is discoverable.

### 3. Shared parameter and proposal ownership (4–5 minutes)

- Reset; choose **Parameter valid**.
- Inspect the two driving-dimension bindings, accepted parameter revision and output proposal.
- Expect one coherent accepted transition with no visible intermediate partial update.
- Judge whether shared host ownership and accepted proposal provenance are clear.

### 4. Invalid/stale parameter recovery (5–6 minutes)

- Continue or reset and first choose **Parameter valid**. Then choose **Invalid kind** and
  **Parameter stale**; inspect lifecycle, Problems, design/latest-attempt/accepted cards and the
  accepted-only canvas after each.
- Choose **Parameter recovery**.
- Expect both rejected inputs to retain accepted geometry/evidence and only complete recovery to
  advance accepted state.
- Judge whether retained intent, latest failure and accepted truth are visually distinct.

### 5. External loss and explicit recovery (6–7 minutes)

- Reset; choose **External missing**, **External stale** and **Topology change**, inspecting
  attempted, retained and accepted external revision/digest/status evidence after each.
- Choose **Explicit rebind**. Expect declaration evidence to change without accepted publication.
- Choose **Fresh recovery**. Expect only this compatible fresh input to advance accepted state.
- Judge stale/missing/topology ownership, no-implicit-repair behavior and recovery clarity.

### 6. Lifecycle, evidence and natural-use pass (5–7 minutes)

- Reset; choose **Lifecycle rejected**. Compare lifecycle badge, tree, Problems, diagnostics/host
  cards and accepted canvas; then choose **Lifecycle recovery**.
- Choose **Capture typed evidence** and inspect/copy the fixed-provenance text.
- Reset and naturally repeat the role/activity flow and one parameter/external recovery flow
  without relying on this document step-by-step.
- Judge whether labels, accepted-only canvas and evidence communicate one coherent trustworthy
  state story without stale display or unexpected movement.

## Finding capture

For every Concern or Blocker, stop before reset and:

1. choose **Capture typed evidence**;
2. copy the complete text from **Copyable fixed-provenance evidence** into the finding record;
3. record the exact typed action sequence, expected result and observed result;
4. attach a normal browser/OS screenshot only when the finding concerns layout, clipping, color,
   ordering or another visual fact; and
5. assign an `M53-*` identifier, add the complete request/finding to the ledger before any edit and
   reference that identifier in the scorecard table.

The copied evidence contains checksummed typed parameter/external input and accepted/attempted
state from public domain/audit APIs. Its location/agent labels are fixed candidate provenance, not
live browser telemetry. Human evidence supports a clarity/trust finding; it does not replace M52
direct assertions.

## M53 scorecard

| Area | Rating: Pass / Concern / Blocker / Not tested | Finding identifiers or notes |
| --- | --- | --- |
| Construction role and profile participation |  |  |
| Suppression/reactivation versus dimension mode |  |  |
| Host-inactive/external/dependency reason clarity |  |  |
| Shared parameter ownership and atomic update clarity |  |  |
| Output proposal provenance clarity |  |  |
| Invalid/stale parameter recovery and retained intent |  |  |
| External missing/stale/topology/rebind recovery |  |  |
| Design/latest-attempt/accepted distinction |  |  |
| Finding evidence usefulness |  |  |
| Overall host-semantics trust |  |  |

| Review context | Value |
| --- | --- |
| Candidate build/revision | pending reconciled M53-S2 clean build and distribution manifest |
| Desktop browser/OS | pending supervising-human entry |
| Elapsed time | pending supervising-human entry |
| Supervising human/date | pending supervising-human entry |
| Nonblocking concerns and disposition | pending supervising-human entry |
| Decision | **not recorded** |

## Approval gate

M53 passes only when the supervising human explicitly records approval above, every area is rated,
and no ownership, stale-data, recovery or state-trust Blocker remains. Any objective defect found
during review must first become a direct regression and pass targeted requalification; only the
affected human observations need repeating unless the candidate changes materially. Every ledger
row must also have an explicit disposition; deferred future scope must have a durable `PLAN.md` or
open-question owner rather than disappearing from M53.
