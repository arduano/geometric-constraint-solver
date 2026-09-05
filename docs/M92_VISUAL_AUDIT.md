<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M92 visual and geometric design audit

Status: **in progress; F015 bounded Jansen dragging is repaired at the core owner. The first
audited clean gate failed; no replacement candidate is nominated and no human UAT is accepted**.

F003–F014 repairs are integrated. The development sample browser audit passed 20/20 before F014;
exact legacy recovery and new scale restoration checks pass. F014's Jansen/field browser checks
pass 2/2 in 5.3 minutes, exit 0; its existing 1024 px layout check also passes. F015 exposes a
bounded drag path absent from the unlimited native sweeps and source-edit-only Jansen browser
audit. Final clean release qualification and immutable nomination remain pending. The atlas edit
log's 2/2 result retains its wrapper exit 143 caveat, detailed below.

The supervising user authorized implementation of the visual/geometric audit and repair plan on
2026-09-05. A sample that solves correctly but fails to demonstrate its named purpose must be
reworked within the existing 20-entry 2D scope. Correct residuals and backend parity alone do not
establish correct design intent.

## Evidence authority

Baseline source `b49dbd303d477e05c412ef8e7bf521be5ca6d20d`, tree
`07e6f4b857a49d05bdb1c3907beb41553cd5c6b3`, was clean when captured. Persistent evidence is under
`/home/arduano/m92-visual-audit-20260905/`; `baseline-b49dbd3/` preserves the sample sources,
headless executable, distribution, handover and qualification logs with `SHA256SUMS`.

All 20 baseline samples successfully rendered through the existing `geosolve-headless render`
command, with a separate 180-second bound per process and continued execution after failure.
Each successful output contains the canonical accepted project/source, report-v2 validation,
controls, SVG and PNG. Exact durations and dispositions are in `render-results.json`; rendering
success is not design acceptance. The previous gate journal retains invocation
`e43f17f8c62547b290f113fb8cb5b2da` and reaches distribution validation at 13:15:20 AEST;
its missing explicit exit receipt remains a qualification caveat.

Baseline overview sheets:

- [Mechanisms](/home/arduano/m92-visual-audit-20260905/baseline-b49dbd3/mechanisms.png)
- [Products 6–11](/home/arduano/m92-visual-audit-20260905/baseline-b49dbd3/products-a.png)
- [Products 12–16](/home/arduano/m92-visual-audit-20260905/baseline-b49dbd3/products-b.png)
- [Atlases and scale](/home/arduano/m92-visual-audit-20260905/baseline-b49dbd3/atlas-scale.png)

## Review contract

For each sample, define its intended geometry and meaningful changes before checking output.
Use native accepted geometry, public measurement APIs and independently reviewed reference
relationships for mathematical evidence. Use full-size renders, detail views and real browser
interaction for visual evidence. Compare edits with consistent cameras. Every sample receives its
declared edit plus a second meaningful parameter edit and exact applicable Undo/Redo/reload.

Require finite geometry, independent normalized Hard residual `<= 1e-9`, truthful independently
justified mobility, explicit assembly/contact state, and complete retained authority on rejection.
Mechanism sweeps use consecutive retained states and reverse motion; floating-point noise is not
meaningful travel. Record dimensional tolerances before evaluating a trajectory. Schematic
reference studies must still reproduce the functional relationships their names claim.

## Complete baseline inventory

The initial primary-agent overview inspection produced the historical targets below. Subsequent
measurement, repair and browser outcomes are recorded later in this ledger. Leads were numbered
only after independent reproduction.

| # | Sample | Intended behavior / baseline audit target |
|---|---|---|
| 1 | Theo Jansen leg | Walking foot locus; source appears to rigidly attach foot to frame rocker. |
| 2 | Whitworth | Guided ram with unequal forward/return crank intervals. |
| 3 | Twin-roller cam | Two independent followers retain equal radius and tangent contact. |
| 4 | Peaucellier | Inversor output follows an independently derived straight line. |
| 5 | Five-stage scissor | Coupled stages and level platforms extend and retract together. |
| 6 | Water manifold | Channels, reservoir and seal paths retain coherent connections under edits. |
| 7 | Dogbone coupon | Fit differences and corner-relief intent remain legible and measurable. |
| 8 | Vacuum fixture | Gasket, vacuum-port grid and workholding pattern remain coherent. |
| 9 | Dust-shoe clamp | Bore edit changes actual bore; split and clamp lug have useful relationship. |
| 10 | Gridfinity | Height increases useful cavity depth while base/lip dimensions remain valid. |
| 11 | NEMA 17 | Pilot edit changes actual pilot while shaft and 31 mm mount pitch remain fixed. |
| 12 | HevORT | Referenced envelope, rail and carriage mounting datums agree with cited scope. |
| 13 | Voron panel | Referenced edge openings/notches are represented by connected profile topology. |
| 14 | Prusa MINI | Bearing/interface locations follow cited dimensions and explicit schematic limits. |
| 15 | Micron carriage | Rail-block and toolhead mounting relationships remain recognizable and useful. |
| 16 | Bondtech INDX | Three-point coupling seats respond to their intended pitch relationship. |
| 17 | Curves atlas | Each advertised family is visibly identifiable; contact and C2 survive edits. |
| 18 | Operations atlas | Each displayed operation has correct visible output and meaningful controls. |
| 19 | Perforated field | All 192 concentric pilot/counterbore pairs remain complete under radius edits. |
| 20 | Harness backplane | Eight routes, 80 clips and 64 computed bends preserve shared edit authority. |

## Repair and nomination boundary

Use focused owner regressions for confirmed findings; broaden the golden only for a systemic gap.
Record source identities, expected/observed measurements, exact commands and outcomes, before/after
images and remaining limitations. Regenerate compiler/patch assets through their owners, never by
hand-editing authenticated envelopes. Final integration must pass the clean release gate and
the separately counted 20-row release harness and two 19-row immutable production endpoint runs.
The final freeze must be a no-rebuild copy of those qualified production bytes.

M92-U1 through M92-U8 remain pending/unexecuted. Agent visual review does not accept a human row.
GitHub Pages remains the accepted M91 publication and is unchanged.

## Reproduced findings and integrated repairs

The original `b49dbd3` is preserved as historical evidence. The new native and real-browser review
has demonstrated that its prior successful solve/test statuses did not establish sample intent or
browser restoration. Findings below are independently reproduced; mathematical engine behavior is
unchanged unless a distinct owning boundary is named.

| Finding | Reproduction and repair | Owning regression/evidence |
|---|---|---|
| M92-F003 | Jansen's foot stayed at radius 9.4339811320566 mm from the frame, spread 1.95e-14 over 16 accepted crank steps. Replaced the rigid rocker topology with an articulated eight-bar walking leg. | `m92_mechanism_drag_witnesses`; `mechanisms/jansen-baseline-reproduction.log` |
| M92-F004 | Whitworth's horizontal ground arrangement made the return link unable to reach a complete crank cycle; tracking already missed at 210°. Reoriented ground pivots for the horizontal ram. | `m92_mechanism_drag_witnesses`; accepted forward/reverse trajectories |
| M92-F005 | Dust-shoe and NEMA representative edits changed radius seeds overridden by driving dimensions; actual rendered geometry was byte-identical. Witnesses now edit driving dimensions. | `m92_product_design_intent`; products-A original prepare/receipt/resolve exchanges |
| M92-F006 | Gridfinity 3U→4U edit raised cavity floor 7→14 mm. Floor now follows the base top; plan and cavity project section dimensions. | Gridfinity height/width tests in `m92_product_design_intent` |
| M92-F007 | Dust-shoe split x[32.5,46] lay opposite the clamp lug x[-64,-40]. Split now crosses lug and enters bore. | `dust_shoe_split_crosses_the_screw_lug_and_spindle_bore` |
| M92-F008 | Separate manifold seals crossed the shared reservoir. One common enclosing seal, actual outlet bores and outside fasteners now express the connected layout. | `manifold_seal_encloses_the_shared_reservoir_and_routes` |
| M92-F009 | Vacuum fixture through-fasteners were inside the gasket; width edit lost gasket centering. Fasteners are outside and symmetric gasket/port controls retain their datums. | Vacuum containment and two-edit tests |
| M92-F010 | Four reference interpretations disagree with pinned drawings: Voron edge-open T/notches, Prusa paired R7.5 seats at 34 mm, Micron 16×15/10×15 patterns, HevORT selected 20×20 mount. | `m92_product_reference_intent`; pinned STEP/DXF extracts and native baseline failures |
| M92-F011 | Harness service clip centre [54,-94], R2.4 overlapped lower-inner-east mount [55.3333,-96], R4; separation 2.4037008503 < 6.4 mm. Lower mounts moved to y=-104. | `harness_mounting_holes_are_separate_from_route_clips`; failing then passing native regression |
| M92-F012 | Bondtech pitch radius 14→15 changed only the construction circle while three seat centres retained norm 14. | `bondtech_pitch_edit_moves_the_three_seats`; repaired seat incidence/shape tests |

Products-A repairs are integrated in `3b82f42`, `d90187e`, `8488864`, `a2c7b88`. Mechanism repairs
and regressions are integrated in `e60f2bb`, `70c63fd`, `d4f2417`, `3657445`, `77c3727`.
The independent mechanisms evidence directory is `/home/arduano/m92-visual-evidence/mechanisms/`;
its six trajectory files preserve 520 accepted native frames. Jansen stance is 155 crank degrees
within a 1 mm vertical band, with 56.1283 mm horizontal stance travel and 22.3079 mm lift. Whitworth
stroke is 12.2370791 mm, with 250°/110° turnaround intervals at 5° sampling. Cam passive follower
locality is within 1e-8 mm. Peaucellier output x=(L²−r²)/8 is checked within 1e-7 mm. Scissor stages
satisfy w²+h²=100 with a common rise, traversing 15→45 mm platform height and reversing.

Peaucellier's known coincidence singularity at 120° is outside its explicit 45–115° driver range.
Scissor's finite ground guide carries explicit 0.1–0.98 slider bounds. These are useful authored
ranges using existing domain support; neither is recorded as a solver defect.

## Atlas and scale measurement contracts

`m92_atlas_scale_intent` independently checks full visible family specimens, regular C2 position,
first and second derivatives, rail/circle tangent side and interior contact, exact parabola trim,
rectangle side lengths, fillet radius/contact/perpendicular normals, visible split/break/trim
intervals, extension endpoint, sampled mirror reflection, and 24 generated cross locations.
The source-family test now enumerates every advertised curves-atlas family instead of an empty
required-family list.

The perforated field must contain 192 shared native centres in a complete 24×8 lattice at 15 mm
pitch, each with one pilot and counterbore. Northern radius edits affect exactly 48 pairs while
central 96 and southern 48 remain unchanged. Harness checks retain 8 ten-vertex routes, 80 clips,
8 mounts and 64 computed bends; every bend contact is on a bounded route segment and has a
perpendicular radius. Shared radius edits cover all intended recipients.

Intentional scale mobility is independently explainable: field = 192 centres × 2 + 384 independent
radii + 4 rectangle freedoms = 772; harness = 80 route vertices × 2 + 80 clip radii + 8 mounting
circles × 3 + 4 rectangle freedoms = 268. Source parameters impose regeneration relationships;
these free native scale studies are not advertised as rigid mechanisms.

Atlas freedom is independently decomposed: curves = analytic specimens 36 (line 4, circle 3,
circular arc 3, ellipse 5, elliptical arc 5, rational conic 7, parabola 4, hyperbola 5) + spline
specimens 33 (quadratic 6, cubic 8, open spline 8, periodic NURBS 11) + rolling bench 2 + C2 pair 10 = 81.
Operations = 24 two-span crosses 192 + hexagon 12 + split/break/trim 12 + extension/limit 8 +
mirror axis/seed 10 + chamfer parents 6 + fillet parents 6 + offset source 4 + metrology 5 = 255.
The free analytic specimens' arc angles and conic trim bounds stay fixed during solving; the rational
conic's weighted middle has two variables and one NURBS weight is the fixed gauge. These atlases
intentionally expose free specimens;
source regeneration relationships do not imply native rigidity. The 192 pattern freedoms include
the two source lines and 46 copied lines; these copies and the six polygon vertices are free native
geometry initialized by generation. Mirror, chamfer, fillet and offset outputs are relationally
constrained and add no independent freedom beyond their listed parents.

## Real-browser restoration discovery

A separate 20-row `m92-sample-audit.spec.ts` checks ordinary open/point-selection, two Parameters
edits, exact accepted-source Undo/Redo/reload and captures full screens plus authoritative SVG.
The historical curves and operations atlas rows passed this workflow. Both scale rows failed at
reload after their first edit: `workbench bridge request exceeds 41943040 bytes`, followed by a
reset to Untitled. This is M92-F013, independently reproduced through the ordinary Rust bridge
constructor and code-workbench persistence owners; it is not a camera or test timing issue.

Captured original field baseline persistence is 43,104,177 bytes; after one edit 81,671,710 bytes.
Escaping that edited workspace into the constructor produces 119,601,833 bytes. Its inner v4
code-workbench is 62,706,553 bytes and is within its owner's 64 MiB bound, but repeated JSON-string
wrapping exceeds the narrower bridge transport. Exact payloads and hashes are preserved under
`browser-restoration-reproduction-r2/perforated-fixture-field/`. The baseline scale WASM tests ended
after Redo and omitted constructing the persisted browser envelope. The added native regression
exercised that actual boundary and independently failed with the same byte counts and error in
412.20 seconds. At this historical discovery checkpoint, repair and a complete rerun were required
before qualification could proceed. The completed focused recovery results are recorded below;
final clean qualification and immutable nomination remain pending.

A first atlas/scale evidence run found a test-export `HARNESS_ERROR`: public `publish_render`
requires an existing parent directory. The shared optional exporter now creates that parent while
preserving the publisher's refusal to overwrite an existing output. No geometry assertion was
weakened. Focused atlas native baseline and warnings-denied Clippy passed. At that checkpoint, the
eight measured edit/history checks and a rerun of the interrupted first atlas edit were outstanding;
the follow-up results and wrapper caveat appear in the integrated qualification section below.

## Integrated reference repair and review follow-up

Products-B commits `d13cb27`, `04d6e74`, `80f867d`, `33082b1`, `a12d3b0`, `fb764b7`,
`3f2960c` integrate the four reference corrections and Bondtech propagation repair. Independent
reference dimensions and final native renders are recorded in `products-b/AUDIT.md` and
`products-b/integration-notes.md`; their early tentative finding numbers are superseded by this
ledger's F010/F012. HevORT retains distinct HD9 and MGN9 interfaces; Voron retains edge-open
passage/notches; Prusa uses paired bearing interfaces; Micron has one enclosing stepped body.

Independent review identified gaps in the initial audit tests. The shared code-history helper checks
canonical code-session transitions and deterministic cold geometry reconstruction; its placeholder
accepted checkpoint does **not** establish actual retained-editor history. The separate browser
workflow now compares all painted native/computed paths and points at a deterministic fitted camera
through baseline/edit/Undo/Redo/reload, checks visible bounds and group-isolation restoration, and
requires distinct control paths. Existing retained mechanism trajectory tests and scale native/WASM
adapter tests retain actual accepted editor authority. Screenshots remain reviewed evidence rather
than the mathematical oracle. The strengthened curves browser row passed against preserved baseline
production in 43.1 seconds. The later integrated development run passed 20/20 before F014, as
recorded below; the final clean-gate repeat remains pending.

Bondtech's original 14→15 pitch and 2.5→2.8 seat witness values extend beyond its fixed published
construction comparison envelope. That envelope is reference data and is not rescaled to disguise
clearance. The mandatory design demonstration now uses pitch 14→13 and seats 2.5→2.7, with an
independent envelope-containment assertion. The original 15 mm regression remains as explicit
propagation evidence, without an enclosure claim. This schematic has no general collision or
fit-limiting solver contract.

F013 owner repair `1a3aa61` emits code-workbench v5, compressing only complete canonical session
bytes using the existing bounded/checksummed codec (12 MiB compressed, 16 MiB text, 64 MiB decoded).
Canonical project/source remain readable, and decompression restores every exact history checkpoint.
Legacy v4 decoding feeds the same validating checkpoint decoder. Fourteen focused owner tests,
focused Clippy and WASM check pass. Root integration adds a strict dedicated raw 96 MiB legacy
presentation recovery path, exactly v4-only, with the existing 64 MiB owner bound and unchanged 40 MiB
ordinary request guard. Recovery retains presentation and must emit a compact ordinary request.
Frontend replacement constructs and validates a candidate before freeing the previous handle;
failed restoration retains saved bytes and pauses automatic saves until an explicit manual Save.
The focused frontend suite passed 41/41 and the small strict legacy bridge test passed. Exact
original-payload recovery and scale new-save restoration subsequently passed the checks below.
Final clean qualification remains pending.

## Integrated restoration and audit qualification

Recovery commit `fa3ab7c` passes the exact captured legacy native test in 475.38 seconds, preserving
exact canonical project/session and presentation, restoring ordinary persistence, and replaying Undo/Redo.
The original 81,671,710-byte save becomes 9,800,854 bytes and an 11,812,081-byte constructor request.
The two actual new-save scale samples pass the native bridge regression in 759.31 seconds: field
persistence 9,800,857 bytes / request 11,812,084; harness 4,465,154 / 5,285,061. The extended actual release-WASM
scale lifecycle passes 1/1 in 64.86 seconds, including construction and history after reload.

The exact original save also passes Chromium recovery, Undo/Redo, v5 persistence and ordinary reload
through `browser-legacy-recovery.mjs`; original SHA-256 is checked before browser loading. The first
probe attempted to send the 81.7 MB string through Playwright's debugging channel and the page closed
before injection: HARNESS_ERROR. The corrected probe fetches the unchanged bytes from a local
read-only fixture endpoint into IndexedDB; no product or payload change was needed. Its result is
`browser-legacy-recovered/result.json`, with no browser errors and compact SHA-256
`1a307cd45a852287c0b629c37bc1813fd95573e156fe941216550ede847638f0`.

Atlas follow-up commit `ff506cb` includes all reviewer-requested measurements: full hexagon,
obround closure/tangency, chamfer setbacks, signed normal offset, every generated pattern arm,
metrology, finite interior fillet contacts with side/sweep, witness radius and periodic NURBS
closure. Focused baselines 3/3 and four atlas edit loops 2/2 pass; warnings-denied Clippy and formatting
pass. The atlas edit log records 2/2 passing in 415.68 seconds, but its outer tool wrapper returned
143; this wrapper anomaly is preserved in `atlas-followup.E37YD2/RESULTS.md`, not represented as a zero exit.
The complete clean gate will provide the authoritative repeated qualification receipt.

Bondtech's contained demonstration edits and original pitch-propagation regression pass 3/3 in
65.41 seconds.
The mandatory release gate now includes the separate 20-row workbench harness and 20-row sample audit
in addition to its language-service row, with the four headless audit suites after sidecar build.

The integrated development browser sample audit passed 20/20 in 15.7 minutes before the F014
presentation correction, checking 40 source edits, actual geometry through Undo/Redo/reload,
ownership and group restoration. Its complete evidence
is `browser-integrated-r1/`. Full-size/detail inspection across all four families agrees with the
native measurement contracts. Jansen has a recognizable articulated leg and closed foot loop;
Whitworth has a reachable full cycle; the cam/Peaucellier/scissor visibly retain their intended
structure. Product clearances and reference interpretations are described with their schematic
limits above. Atlas specimens occupy separate rows; the drilling crosses, dense scale points and
Gridfinity dimensions require isolation/detail zoom for close inspection. Scale output remains
complete through both edits and reload. This run discovered F014; its Explorer screenshots preserve
the pre-repair presentation. `browser-explorer-r2/` records the corrected panel evidence.

### M92-F014 — Explorer labels scrolled out of the panel

Browser image review found the 125 px Explorer declaration viewport had 258 px scroll width.
Clicking its first Isolate control moved `scrollLeft` from 0 to 133, hiding the group/row label
starts. `explorer-layout-baseline.log` independently records those DOM measurements. This is a
pure presentation defect. Rows and child grids now shrink within the panel, long labels retain
full hover titles, and the default Explorer share increases from 9% to 16% (minimum from 6% to
12%) so source names and action controls remain useful. A browser assertion rejects horizontal
scrolling after group isolation/restoration. Existing canvas minimum and resizable layout remain
supported.

F014's corrected independent DOM probe reports no overflowing scroll container before or after
Isolate. Full-size Jansen evidence shows group names, declarations and controls visible; the
Jansen browser check passes. The existing 1024 px layout-floor workbench test passes 1/1 in 6.4
seconds. Frontend 41/41, TypeScript and formatting checks pass at the F014 checkpoint. The final
focused Jansen/field browser run passes 2/2 in 5.3 minutes, exit 0 (`browser-explorer-r2.log`).
Repair commit `a034585` includes the presentation and test transport changes. The final 20-row
sample audit will repeat inside the clean gate using the corrected presentation and a test
transport optimization: parse persisted authority in the browser and return only source, avoiding
repeated multi-megabyte history copies through Playwright. No geometry comparison is weakened.

### F013 follow-up — protect the separately saved source draft

Independent restoration review found the fallback's unguarded draft-storage effect could remove or
overwrite `geosolve.source-draft.v1` while correctly preserving IndexedDB project bytes. The valid
and malformed saved-draft regressions both reproduce deletion on `a034585`:
`restore-draft-reproduction.log`, 2 failed, exit 1. This extends F013's recovery preservation
contract.

Commit `41744ca` makes draft restoration read-only and guards draft writes/deletion until project
storage is resolved or the user successfully saves an explicit replacement. The regression checks
fallback editing, presentation changes, failed manual saving and successful replacement of both
stores. The first repair run preserved data but encountered a jsdom/CodeMirror synthetic-keyboard
harness error in its new draft-content assertion; the test now submits an exact editor
transaction. `restore-draft-fixed-r2.log` records 43/43 frontend checks and TypeScript success,
exit 0. Command:

```sh
nix-shell shell.nix --run 'cd crates/geosolve-demo-web/frontend && npx vitest run src/App.test.tsx src/lib/wasm-adapter.test.ts && npx tsc -b'
```

Independent follow-up review found no remaining blocker in raw legacy bounds, strict owner/history
validation, candidate replacement or either storage-preservation path. No solver equations
changed.

### M92-F015 — bounded crank previews retain the old frame without movement

The first audited clean gate found that
`workbench::bridge::tests::theo_jansen_drag_uses_canonical_managed_authority_without_compilation`
still selected the removed `[-8, 3]` point. That initial fixture failure is `HARNESS_ERROR`.
Correcting selection to the repaired crank at `[15, 0]` and sending 12 moves toward
`[14.265847744427303, 4.635254915624211]` independently reproduced an actual `DEFECT` at the
bounded retained-editor preview boundary.

`jansen-bounded-outcome.log` records `WorkExhausted` at `BeforeFactorization`, with 256/256
factorizations, 254 nonlinear iterations and a largest dense kernel of 23×12. The initial
previews reject 144–148 trials; later attempts in the same log reach 158. The editor retains
finite previously validated geometry, but the requested point does not move, release produces no
Undo entry and `last_error` remains empty. The focused corrected bridge regression fails its
movement assertion (0 passed / 1 failed, 5.84 seconds). A retained accepted frame after rejection
does not establish that the drag accepted movement.

The new actual-crank browser check is inside Jansen's existing sample workflow, so the sample
audit inventory remains 20. Independent Chromium reproduction fails at disabled Undo, exit 1,
in `jansen-bounded-browser-baseline.log` (one row, 9.3 seconds). The log identifies the browser
error context and trace under
`test-results/m92-sample-audit-M92-visual-workflow-1-theo-jansen-leg-chromium/` as
`error-context.md` and `trace.zip`. The prior 520-frame native mechanism trajectories used
unlimited work budgets; the earlier 20/20 browser sample audit checked source edits/history
without dragging Jansen. Both remain valid evidence for their original scope, but neither
qualifies this bounded interaction.

At reproduction, core diagnosis and repair were pending. The correction and focused
qualification are recorded below.
The required regression must prove meaningful crank and foot movement within the ordinary
configured budget, finite independently validated hard residuals, retained explicit branches and
fixed ground, unchanged managed source, one history action on release and exact geometry
Undo/Redo/reload without compilation. The browser check requires input target error at most
0.05 mm and output travel greater than 0.1 mm. Valid prior-state retention remains mandatory on
rejection. Focused owner and browser passes must precede a complete replacement clean gate.

### First audited clean gate — failed, no nomination

This is the first clean gate after the visual-audit integration. It is distinct from the historical
pre-F002 `7941614` server-start timeout and the original baseline journal with no exit receipt.
The audited invocation has a complete failure receipt:

| Field | Recorded value |
|---|---|
| Source | `c4c02abd8a92e2e929ccfb23b59c211a32a7400a` |
| Tree | `74ff8155a389ef97249c3984e4692f5059269b49` |
| Worktree | `/home/arduano/programming/geometric-constraint-solver.worktrees/m92-final-audit` |
| Service | `geosolve-m92-audited-release-gate.service` |
| Invocation | `d10ff7c22efa47b4937c0d7c07f73c35` |
| Start UTC | `2026-09-05T05:41:16.299236+00:00` |
| End UTC | `2026-09-05T05:59:54.846172+00:00` |
| Exit | `101` |
| Source/tree status | Clean and unchanged at start and end |
| Log bytes / lines | `150960` / `1815` |
| Log SHA-256 | `fd748cf22ba1979ecddcf92bc179776b7c31d06c73d35f27fbc481ee97a63037` |

The exact launch in `final-gate/run.sh` was:

```sh
nix-shell shell.nix --run 'TMPDIR=/home/arduano/t ./scripts/release-gate.sh'
```

Formatting and warnings-denied Clippy passed. The demo library finished 292 passed / 1 failed /
1 ignored in 750.90 seconds, failing
`workbench::bridge::tests::theo_jansen_drag_uses_canonical_managed_authority_without_compilation`
with `theo-jansen-leg draggable point at [-8.0, 3.0]`. It therefore stopped before completing
the gate. The receipt is
`/home/arduano/m92-visual-audit-20260905/final-gate/receipt.json`; the corresponding log is
`/home/arduano/m92-visual-audit-20260905/final-gate/release-gate.log`.

Corrected selection subsequently exposed the independently reproduced F015 defect above. No
nomination follows from this failed gate or prior partial runs. After repair, run the full gate
again from clean integrated source and preserve a new complete receipt. All M92-U1–U8 human rows
remain pending and unexecuted; GitHub Pages is unchanged.


### F015 core repair and bounded qualification

Core commit `a9d7532` extends existing first-improvement backtracking to all Temporary objectives.
A fully driven one-DOF mechanism has no passive Preference anchors; the previous conditional made
its line search evaluate all 20 smaller steps after finding valid descent. Each Jansen trial
spent 12–15 iterations hard-reprojecting near floating-point resolution. No work cap, tolerance,
branch rule, Hard validation or priority certification changes.

The minimal core crank regression fails at 256 factorizations before correction and completes
with 51 in the first repaired unit-scale case. Final coverage includes both directions at
1e-6, 1 and 1e6 scales, independently checked radius, finite target placement and rank 1/DOF 1.
The 18-test priority suite, all core tests (213 passed / 1 existing ignored), 26 lifecycle tests,
five locality tests and three managed bridge drags pass. Core Clippy and formatting pass;
`f015-owner-update.txt` preserves exact commands and outcomes.

`every_mechanism_witness_accepts_default_bounded_pointer_frames` uses the real editor constructor's
default control and actual scene picking/pointer frames. All ten witnesses across five mechanisms
pass, requiring explicit accepted preview effects, finite independently validated Hard residuals,
expected mobility, history-free previews and exact terminal/Undo/Redo/reload. The focused run
`f015-all-bounded-witnesses.log` passes in 62.11 seconds, exit 0. The complete nine-test mechanism
suite, including all original trajectories and retained witnesses, passes in 115.63 seconds
(`bounded-mechanism-full-suite.log`). Its strict witness reader also accepts the registry's
existing secondary-edit field. Initial focused Clippy found only two manual-midpoint style
violations in the new test; corrected Clippy passes. No production equation changed.

Browser preflight passed 17 of 18 other ordinary workbench rows. The Jansen Polyline test's third
container-relative click landed in the SVG's letterboxed area, outside the viewBox, and correctly
created no vertex. Mapping all three clicks through the actual SVG screen transform preserves
its original Horizontal/Vertical authoring assertions and passes against unchanged pre-fix
production (1/1, `jansen-authoring-camera-fixed.log`); classify as HARNESS_ERROR. The post-fix Jansen browser workflow passes in 18.8 seconds, including actual crank/foot
movement and exact geometry through Undo/Redo/reload. Scissor completes all five coupled-stage
drags; its old assertion incorrectly required the compressed v5 save to exceed 5 MiB. The actual
4,547,985-byte save contains 17,903,027 decoded session bytes. Checking bounded zlib decoding and
that real history size preserves the large-history contract. The corrected scissor row passes
1/1 in 16.2 seconds (`f015-scissor-browser-fixed.log`); TypeScript passes. The first two browser
launches lacked isolated package dependencies and the direct-Node launcher lacked npm bin PATH;
those harness failures ran no browser assertions. Qualification used the built release WASM
through a local development server, never a nominated endpoint.

The pre-F015 golden survey/check both completed with exit 0 and unchanged 271 PASS rows. Final
clean release qualification must repeat the oracle against the repaired source. No human UAT
row or immutable nomination is established by these focused results.
