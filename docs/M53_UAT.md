<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M53 CAD host-semantics UAT scorecard

## Status and ownership

M45 is complete as the historical cleanup investigation and ten-point capture checkpoint retained
in the archived section below; it recorded no human approval. M46-M51 replaced or retired the old
browser assertions and removed the legacy application/E2E stack. M52 completed the minimal
post-cleanup candidate and objective direct qualification. This file is now consolidated and named
for its sole active purpose: **M53 is active and alone owns the supervising-human ratings and
approval here.**

`PLAN.md` now marks M52 complete. M53-S3 was superseded before ratings by the flyout-navigation
request M53-P012. The newly qualified M53-S4 candidate is ready for targeted human review; no
blank rating, automated result or historical M44 observation counts as M53 approval.

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
| M53-S2 | clean build-source commit `66fd90d62d06a810e465185e24fff40ebbea5ef2`; release distribution manifest SHA-256 `65f74cd18ed59e443848c65da1efd5e89150b7b95742eff629fa9bc9d8c8a751` | retired temporary endpoint; no longer valid | no human review started | superseded before ratings by the material M53-P011 scenario-selector presentation change; server stopped before S3 qualification |
| M53-S3 | clean build-source commit `17a4a25ee1e3d0ee4d0b27a1f08dd7d23e6437aa`; release distribution manifest SHA-256 `213b8f41b18af239738eb0336651216fa4693a8f1ecbefa0f2f7bf7d6b675518` | retired temporary endpoint; no longer valid | no human review started | superseded before ratings by M53-P012; server stopped before flyout-selector implementation and S4 qualification |
| M53-S4 | clean build-source commit `49ddcb8ea8d098ec7a3e38180465d9df43831457`; release distribution manifest SHA-256 `d2d91ff200a7e55d0e04bb90e863d9c771f10325cb286b5147790bdb8e192b33` | `http://100.94.63.83:8080/` from the completed distribution; non-watching temporary server | pending supervising-human entry | ready for targeted M53-P011/P012 retest and the remaining M53 scorecard; no human rating or approval has been recorded |

S1 used the SHA-256 output of `sha256sum dist/* | sha256sum`, but its watched distribution
changed after recording. Any candidate rebuild must receive a new session row and digest.
Temporary human delivery is permitted for M53 observation; the removed browser E2E/server scripts,
automated DOM qualification and old fixture routes remain prohibited.

S2 was produced by the clean release gate, then served from the completed `dist` directory without
a rebuild watcher. Direct HTTP retrieval of all seven files matched the local SHA-256 values,
including `index.html`, JavaScript, CSS and the WASM binary. The ledger-only handoff commit after
`66fd90d` does not change the recorded build input or distribution. M53-P011 subsequently
superseded S2 without erasing that record; the selector-led procedure must use a newly qualified
M53-S3 candidate.

S3 was produced by `nix-shell shell.nix --run './scripts/release-gate.sh'` from clean commit
`17a4a25`. The complete gate passed, including workspace tests, warnings-denied Clippy, docs,
benches, release performance checks, all-feature WASM, licences, package contents and the Trunk
release build. All seven files retrieved from the non-watching endpoint matched their local
SHA-256 values, and `sha256sum dist/* | sha256sum` remained
`213b8f41b18af239738eb0336651216fa4693a8f1ecbefa0f2f7bf7d6b675518` before and after delivery
verification. The later ledger/handoff commit changes no demo-web product input or frozen
distribution byte. M53-P012 subsequently superseded S3 without erasing that qualification record.

S4 was produced by the same complete clean release gate from commit `49ddcb8`. The gate passed,
including demo-web 29/29 and the release-only 256-moving-body sparse crossover test in 136.97s.
All seven files retrieved from the non-watching endpoint matched their local SHA-256 values;
`sha256sum dist/* | sha256sum` produced
`d2d91ff200a7e55d0e04bb90e863d9c771f10325cb286b5147790bdb8e192b33`. The handoff-only commit
that records this session changes no demo-web product input or frozen distribution byte.

### Findings and change-request ledger

| ID | Session/section and exact sequence | Observation or request | Classification | Status and disposition | Regression/requalification | Human retest/rating |
| --- | --- | --- | --- | --- | --- | --- |
| M53-P001 | M53-S1, before setup observation | Preserve every UAT UI request and all other development concerns/questions/plans durably so one request cannot shadow another. | review process | complete: added this ledger, candidate versioning, classification, continuity and retest rules before product review | `git diff --check`, format check and independent read-only documentation review passed 2026-07-28 | not a product rating |
| M53-P002 | M53-S1, before setup observation | The active scorecard name `M45_UAT` is misleading; consolidate it under M53, align the local branch name and verify Git hygiene without losing accumulated work. | review process | complete: renamed the scorecard to `docs/M53_UAT.md`, consolidated historical M45 context there, updated repository references and renamed the no-upstream local branch to `feat/m53-host-semantics-uat` | tracked/untracked whitespace checks, format check and independent read-only consolidation/Git review passed 2026-07-28 | not a product rating |
| M53-P003 | M53-S1, before setup observation | Plan coherent split commits for the accumulated qualified M38-M52 working tree before deciding whether to stage or commit it. | review process | superseded by M53-P006: the path coverage was complete, but the proposed dependency order omitted two documentation files compiled directly by tests/editor code; the interrupted checkpoint also left its first slice staged | independent recovery audit reproduced the first-slice failure at `geosolve-sketch/tests/m33.rs` | not a product rating |
| M53-P004 | Recovery request after M53-S1 was prepared, before human setup observation | Reconcile all surviving work into the main worktree, eliminate dangling untracked or misleading/outdated content, create clean commits, remove child-worktree dependence and finish with a trustworthy UAT-ready candidate. | review process | complete: commits `a5542da`, `ba711c3`, `5087c41` and `66fd90d` contain all recovered work; the sole registered worktree is the clean main worktree, with no stash or untracked files; duplicate/stale worktrees and generated recovery artifacts were removed only after proving they contained no unique source | `nix-shell shell.nix --run './scripts/release-gate.sh'` passed from clean build-source commit `66fd90d`; superseded M53-S2 recorded and previously served that frozen distribution | human review has not started |
| M53-P005 | M53-S1 delivery verification, before human setup observation | The recorded release manifest must identify the bytes actually served for UAT. | review process | complete: superseded M53-S1 without ratings, stopped its stale live-rebuild watcher, built clean M53-S2 and served only the completed distribution through a non-watching static server | all seven HTTP responses matched their local SHA-256 values; `sha256sum dist/* \| sha256sum` remained `65f74cd18ed59e443848c65da1efd5e89150b7b95742eff629fa9bc9d8c8a751` before and after delivery verification | human review has not started |
| M53-P006 | Interrupted commit consolidation, before human setup observation | Make each checkpoint commit independently buildable and keep compiled documentation inputs with their owning code/tests. | review process | complete: domain commit `a5542da` contains the M33 matrix; editor/workbench commit `ba711c3` contains the embedded M40 JSON; remaining durable records form the documentation checkpoint | exact staged domain tree passed core/sketch/linkage all-feature tests; exact staged editor tree passed editor 58/58, demo-web 24/24, all-feature WASM check and release Trunk build | not a product rating |
| M53-P007 | Recovery content audit, before human setup observation | Durable records and embedded qualification artifacts must distinguish historical M40-M52 evidence from current M53 procedure, remove references to the absent temporary handoff and stop advertising retired browser/E2E infrastructure or old milestones as active work. | review process | complete: retained every uniquely owned/compiled record, compacted the old M40 UAT into approved historical evidence, marked planning/browser material historical, removed ephemeral addresses and replaced every absent/brittle handoff reference with a durable section owner | repository reference scan and `git diff --check` pass; full release qualification remains a parent gate rather than a content-audit blocker | not a product rating |
| M53-P008 | Recovery release-gate audit, before human setup observation | The compatibility contract names five publishable lockstep crates, but the package-content loop checks only four and its prose also says four. | objective defect | complete in `ba711c3`: `geosolve-constraint-editor` is the fifth package-content target and the compatibility prose now says five; publication remains a maintainer action | the clean release gate ran `cargo package --locked --allow-dirty --list` and confirmed `LICENSE` and `README.md` for all five crates | not a product rating |
| M53-P009 | Recovery capability-contract audit, before human setup observation | The machine-read M33 capability matrix still labels completed M38 dimensions and path-length behavior as planned/current gaps. | objective defect | complete in `a5542da`: M38 catalog rows use `implemented_m38`; frozen legacy `EqualLength`/`CurveLength` rows point to the separate M38 path APIs instead of claiming M38 still waits | exact staged domain tree passed M33 2/2, M38 11/11 and the complete core/sketch/linkage all-feature suites | not a product rating |
| M53-P010 | Exact staged editor release build, before human setup observation | The supported Trunk 0.21 build must not reject the conventional inherited `NO_COLOR=1` environment value before compiling the candidate. | objective defect | complete in `66fd90d`: the release gate unsets `NO_COLOR` only for its Trunk subprocess; this changes no product bytes or solver/editor behavior | the complete clean release gate passed, including editor 58/58, demo-web 24/24, all-feature WASM check and Trunk 0.21.14 release build | not a product rating |
| M53-P011 | M53-S2, before any scored human section | Replace the easily lost bottom UAT launcher/action wall with a reusable scenario-definition system: a nested scenario selector near the top of the UI, grouped scenarios, scenario-specific descriptions/guidance in a sidebar, and clean removal of superseded one-off harnesses while preserving every current scenario for future reuse. | human clarity/layout | in progress awaiting human retest: implementation is complete in `17a4a25` with six typed scenario definitions under three nested groups, contextual objectives/questions/steps/expected results, recent transcript and typed evidence in the inspector; exact stale-input variants/revisions and last-submitted typed inputs remain observable; selection/switch/reset reconstruct deterministic ephemeral state, Exit restores the ordinary workspace, and the old launcher/overlay/action wall is deleted without restoring M44/playground/E2E infrastructure | catalog 6/6, preserved fixture semantics 4/4 and complete demo-web 29/29 pass; the clean full release gate passed from `17a4a25`, M53-S3 records manifest `213b8f41b18af239738eb0336651216fa4693a8f1ecbefa0f2f7bf7d6b675518`, and all seven served files match local SHA-256. Earlier local headless interaction was exploratory visual inspection only, is not retained browser qualification and does not replace human UAT | targeted human retest required for scenario discoverability, grouping, guidance and natural-use flow; no rating recorded yet |
| M53-P012 | M53-S3, before any scored human section | Replace the nested per-group collapsible disclosures with a right-expanding flyout menu: hovering or focusing a group item shows its submenu immediately on the right for quicker scenario navigation, while retaining the top **Scenarios** dropdown, stable scenario definitions and sidebar guidance. | human clarity/layout | in progress awaiting human retest: implementation is complete in `49ddcb8`; recursive plain-list branches open immediately to the right on hover or focus, group clicks cause no workspace save/render, `aria-expanded` follows visual state, and narrow layouts expose the same branches inline; fixture semantics, stable IDs, guidance and isolation are unchanged | focused catalog 6/6 and complete demo-web 29/29 pass with warnings-denied Clippy and locked all-feature WASM. The complete clean release gate passed from `49ddcb8`; M53-S4 records manifest `d2d91ff200a7e55d0e04bb90e863d9c771f10325cb286b5147790bdb8e192b33`, and all seven served files match local SHA-256. Exploratory headless interaction also verified hover bridge, focus/Tab selection, ARIA state, rightward placement at 1440/1024, inline fallback at 800 and no console errors; this is visual inspection only, not retained browser qualification | targeted human retest required for flyout speed, discoverability and navigation clarity; no rating recorded yet |

### Preserved development continuity

| Concern, question or plan | Durable owner | Current disposition |
| --- | --- | --- |
| M53 ratings, findings and explicit approval | this scorecard and `PLAN.md` M53 | active; no rating or approval recorded |
| Later diagnostics, concurrency, scale, operations/topology, advanced workbench/UAT and release work | `PLAN.md` M100X-M109X | preserved placeholders; M53 feedback must not silently consume or reorder them |
| Recovered M38-M52 implementation/evidence and the M53 scenario selector on `feat/m53-host-semantics-uat` | Git commits plus completed milestone records | product implementation is complete in dependency-safe commits through `49ddcb8`; all unique work is in the sole main worktree and no untracked files remain |
| Cargo duplicate `license`/`license-file` metadata warnings seen during release build | existing package metadata | known nonblocking concern; warnings-denied Clippy and M52 gates pass, and M53 does not broaden into metadata cleanup |

Recovery Git result (2026-07-28): the branch has no upstream or configured remote, no stash,
unmerged entry, hidden commit, child-worktree registration or untracked file, and the main worktree
contains all latest work. The interrupted agent's 23-path staged domain slice and exact detached
verification-worktree duplicate were reconciled into the dependency-safe commits above before the
duplicate/stale worktrees and generated recovery artifacts were removed.

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

In the selector and guide, **P1-P10** mean the ten objective verification points above, not the
`M53-P001`-style finding/process identifiers in the ledger:

| Selector path | Scenario | Points |
| --- | --- | --- |
| **M53 Host semantics → Geometry intent** | **Role & profile participation** | P1 |
| **M53 Host semantics → Geometry intent** | **Activation & dimension mode** | P2-P3 |
| **M53 Host semantics → Host-owned inputs** | **Shared parameter & proposal** | P4 |
| **M53 Host semantics → Host-owned inputs** | **Invalid/stale parameter recovery** | P5 |
| **M53 Host semantics → Host-owned inputs** | **External loss & explicit recovery** | P6-P7 |
| **M53 Host semantics → Truth & evidence** | **Lifecycle, evidence & natural pass** | P8-P10 |

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

The private M53 selector is presentation over the direct-qualified M52 fixture state. It is not a
restoration of the deleted M44 fixture controls, playground, browser E2E or serving machinery.

## Post-cleanup M53 procedure (30–45 minutes)

### Setup and reset

1. Open the release candidate supplied after objective M52 qualification. Record its identifying
   build/revision and the desktop browser/OS used for this human observation; those fields are
   finding context, not automated proof.
2. Open **Scenarios** in the top command bar; its initial value is **Choose a review scenario**.
   Expand **M53 Host semantics**, its relevant group and one scenario leaf.
3. Selection constructs and activates a deterministic ephemeral candidate. Confirm the inspector
   guide shows the scenario description, P-numbered objective and human question, guided steps,
   expected results, recent transcript and typed-evidence area.
4. **Reset scenario** reconstructs the selected leaf; switching to another leaf constructs that
   leaf fresh. Do not use **New** as reset because ordinary mutation is intentionally locked while a
   scenario is active.
5. Use global **Capture typed evidence** before reset when recording a finding. Use **Exit scenario**
   only after completing or recording the current observation; it discards scenario state and
   reveals the unchanged pre-existing ordinary workspace. Reload is not a scenario transition.

The selected guide identifies objective expectations and the M53 judgment boundary. If a
presentation is misleading, lossy, incorrect or difficult enough to prevent trustworthy host use,
capture a finding before reset and rate the area Concern or Blocker.

### 1. Role and profile participation (4–5 minutes)

- Select **Scenarios → M53 Host semantics → Geometry intent → Role & profile participation**.
- Choose **Use construction role**, inspect the accepted canvas, tree, effective activity and
  accepted-profile card, then choose **Restore profile role**.
- Expect construction styling and removal/recovery of profile participation while the curve stays
  active and accepted geometry does not unexpectedly move.
- Judge whether role versus solver activity versus profile participation is understandable.

### 2. Activation and dimension mode (5–6 minutes)

- Select **Scenarios → M53 Host semantics → Geometry intent → Activation & dimension mode**.
- Choose **Suppress dimension**, **Reactivate dimension**, then **Make dimension reference**.
- Expect user suppression/reactivation to be shown independently of driving/reference mode.
- Choose **Set host inactive**, then **Remove dependency**. Inspect the directly unavailable curve,
  dependent dimension reason, lifecycle and accepted canvas.
- Judge whether host inactivity, unavailable external input and derived dependency loss are
  distinguishable and whether recovery is discoverable.

### 3. Shared parameter and proposal ownership (4–5 minutes)

- Select **Scenarios → M53 Host semantics → Host-owned inputs → Shared parameter & proposal**.
- Choose **Submit valid parameter**.
- Inspect the two driving-dimension bindings, accepted parameter revision and output proposal.
- Expect one coherent accepted transition with no visible intermediate partial update.
- Judge whether shared host ownership and accepted proposal provenance are clear.

### 4. Invalid/stale parameter recovery (5–6 minutes)

- Select **Scenarios → M53 Host semantics → Host-owned inputs → Invalid/stale parameter recovery**.
- Choose **Submit valid parameter**, **Submit invalid kind** and **Submit stale parameter**;
  inspect lifecycle, Problems, design/latest-attempt/accepted cards and the accepted-only canvas
  after each.
- Choose **Submit recovery parameter**.
- Expect both rejected inputs to retain accepted geometry/evidence and only complete recovery to
  advance accepted state.
- Judge whether retained intent, latest failure and accepted truth are visually distinct.

### 5. External loss and explicit recovery (6–7 minutes)

- Select **Scenarios → M53 Host semantics → Host-owned inputs → External loss & explicit
  recovery**.
- Choose **Remove external snapshot**, **Submit stale snapshot** and **Change topology**, inspecting
  attempted, retained and accepted external revision/digest/status evidence after each.
- Choose **Declare explicit rebind**. Expect declaration evidence to change without accepted
  publication.
- Choose **Submit fresh snapshot**. Expect only this compatible fresh input to advance accepted
  state.
- Judge stale/missing/topology ownership, no-implicit-repair behavior and recovery clarity.

### 6. Lifecycle, evidence and natural-use pass (5–7 minutes)

- Select **Scenarios → M53 Host semantics → Truth & evidence → Lifecycle, evidence & natural
  pass**.
- Choose **Submit rejected attempt**. Compare lifecycle badge, tree, Problems, diagnostics/host
  cards and accepted canvas; then choose **Submit valid recovery**.
- Choose **Capture typed evidence** and inspect/copy the fixed-provenance text.
- Use the selector to revisit one **Geometry intent** leaf and one **Host-owned inputs** recovery
  leaf, then repeat those flows naturally without relying on this document step-by-step.
- Judge whether labels, accepted-only canvas and evidence communicate one coherent trustworthy
  state story without stale display or unexpected movement.

## Finding capture

For every Concern or Blocker, stop before reset and:

1. choose global **Capture typed evidence**;
2. copy the complete text from **Typed evidence** in the inspector into the finding record;
3. record the exact selector path, stable scenario ID/title, recent transcript, expected result and
   observed result;
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
| Scenario selector discoverability, grouping and guide clarity |  | M53-P011 targeted retest required |
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
| Candidate build/revision | M53-S4: `49ddcb8ea8d098ec7a3e38180465d9df43831457`; distribution manifest `d2d91ff200a7e55d0e04bb90e863d9c771f10325cb286b5147790bdb8e192b33` |
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
open-question owner rather than disappearing from M53. M53-P011 additionally requires the targeted
selector discoverability, grouping, guidance and natural-use retest recorded in its ledger row.
