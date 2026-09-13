<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Milestone history

GeoSolve grew from a small Rust constraint solver into reusable sketch, interaction,
source-authoring and collaboration libraries. This index keeps that development
record separate from the [current documentation](../README.md).

The [historical roadmap](ROADMAP.md) retains the sequence and earlier section
anchors. Detailed [acceptance contracts](../reference/ACCEPTANCE_DETAILS.md),
[architecture contracts](../reference/ARCHITECTURE_DETAILS.md), [scenarios](../SCENARIOS.md)
and [ADRs](../adr/README.md) preserve the durable technical requirements.

## Development phases

| Milestones | Focus |
| --- | --- |
| M0–M7 | Numerical representation, solver, validation, diagnostics and first domains. |
| M8–M23 | Normalized linearization, persistence, curve families, bounds and spatial mechanisms. |
| M24–M39 | Sketch operations, topology, embedding and retained design/accepted-state separation. |
| M40–M53 | Headless interaction and migration to one desktop workbench. |
| M54–M81 | Diagnostics, authoring tools, annotations, fillets, offsets and ownership cleanup. |
| M82–M90 | Deferred computed Offset research, semantic intent and reversible TypeScript authoring. |
| M91–M97 | Authoring consolidation, samples, release qualification, rendering, channels and focused dimensions. |
| M98–M99 | Local projects, shared editing, responsive local interaction and reusable host infrastructure. |
| M100 | Final maintenance and documentation before a project pause; still in progress. |

## Reading historical evidence

A historical plan, mechanical qualification, human review and public deployment are
different records. Later acceptance does not retroactively pass an earlier failed
or unperformed human test. Current status is summarized in [ACCEPTANCE.md](../../ACCEPTANCE.md).
Older APIs and commands describe their original checkpoints; use the current
guides to build and run the project. Private service inventories, temporary paths
and repeated per-file artifact dumps have been retired from tracked prose. Source
and run identities remain where they substantiate a result.

## Milestone records

The early [M1–M4 bootstrap report](../../OVERNIGHT_REPORT.md) predates the numbered
document series. Subsequent records are grouped below. “Implementation” and
“qualification” links are evidence at that milestone, not claims about current
source.

| Milestone | Records |
| --- | --- |
| M14 | [performance](../M14_PERFORMANCE.md) |
| M16 | [sparse crossover](../M16_SPARSE_CROSSOVER.md) |
| M29 | [scale performance](../M29_SCALE_PERFORMANCE.md) |
| M32 | [scale performance](../M32_SCALE_PERFORMANCE.md) |
| M33 | [cad capability matrix](../M33_CAD_CAPABILITY_MATRIX.md), [representative baselines](../M33_REPRESENTATIVE_BASELINES.md) |
| M35 | [cancellation latency](../M35_CANCELLATION_LATENCY.md) |
| M40 | [headless qualification](../M40_HEADLESS_QUALIFICATION.md), [uat](../M40_UAT.md) |
| M41 | [implementation](../M41_IMPLEMENTATION.md) |
| M42 | [implementation](../M42_IMPLEMENTATION.md) |
| M43 | [implementation](../M43_IMPLEMENTATION.md) |
| M44 | [implementation](../M44_IMPLEMENTATION.md) |
| M45 | [cleanup plan](../M45_CLEANUP_PLAN.md), [test fixture cleanup investigation](../M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md), [ui cleanup investigation](../M45_UI_CLEANUP_INVESTIGATION.md) |
| M46 | [direct test replacement](../M46_DIRECT_TEST_REPLACEMENT.md), [rebase inventory](../M46_REBASE_INVENTORY.md) |
| M47 | [implementation](../M47_IMPLEMENTATION.md) |
| M48 | [implementation](../M48_IMPLEMENTATION.md) |
| M49 | [implementation](../M49_IMPLEMENTATION.md) |
| M50 | [implementation](../M50_IMPLEMENTATION.md) |
| M51 | [implementation](../M51_IMPLEMENTATION.md) |
| M52 | [implementation](../M52_IMPLEMENTATION.md) |
| M53 | [uat](../M53_UAT.md) |
| M54 | [implementation](../M54_IMPLEMENTATION.md) |
| M55 | [contextual authoring](../M55_CONTEXTUAL_AUTHORING.md), [implementation](../M55_IMPLEMENTATION.md) |
| M56 | [implementation](../M56_IMPLEMENTATION.md) |
| M57 | [implementation](../M57_IMPLEMENTATION.md) |
| M58 | [implementation](../M58_IMPLEMENTATION.md) |
| M59 | [implementation](../M59_IMPLEMENTATION.md) |
| M60 | [implementation](../M60_IMPLEMENTATION.md) |
| M61 | [remediation](../M61_REMEDIATION.md), [uat](../M61_UAT.md) |
| M62 | [implementation](../M62_IMPLEMENTATION.md), [uat](../M62_UAT.md) |
| M63 | [implementation](../M63_IMPLEMENTATION.md), [uat](../M63_UAT.md) |
| M64 | [implementation](../M64_IMPLEMENTATION.md), [uat](../M64_UAT.md) |
| M65 | [implementation](../M65_IMPLEMENTATION.md), [uat](../M65_UAT.md) |
| M66 | [implementation](../M66_IMPLEMENTATION.md), [uat](../M66_UAT.md) |
| M67 | [implementation](../M67_IMPLEMENTATION.md), [m40 ownership](../M67_M40_OWNERSHIP.md), [uat](../M67_UAT.md) |
| M68 | [implementation](../M68_IMPLEMENTATION.md), [uat](../M68_UAT.md) |
| M69 | [implementation](../M69_IMPLEMENTATION.md), [uat](../M69_UAT.md) |
| M70 | [implementation](../M70_IMPLEMENTATION.md), [uat](../M70_UAT.md) |
| M70B | [hardening](../M70B_HARDENING.md), [implementation](../M70B_IMPLEMENTATION.md), [uat](../M70B_UAT.md) |
| M71 | [goals](../M71_GOALS.md), [handover](../M71_HANDOVER.md), [implementation](../M71_IMPLEMENTATION.md), [uat](../M71_UAT.md) |
| M72 | [goals](../M72_GOALS.md), [implementation](../M72_IMPLEMENTATION.md), [uat](../M72_UAT.md) |
| M73 | [goals](../M73_GOALS.md), [implementation](../M73_IMPLEMENTATION.md), [uat](../M73_UAT.md) |
| M74 | [goals](../M74_GOALS.md), [implementation](../M74_IMPLEMENTATION.md), [uat](../M74_UAT.md) |
| M75 | [goals](../M75_GOALS.md), [implementation](../M75_IMPLEMENTATION.md), [uat](../M75_UAT.md) |
| M76 | [goals](../M76_GOALS.md), [implementation](../M76_IMPLEMENTATION.md), [uat](../M76_UAT.md) |
| M77 | [goals](../M77_GOALS.md), [implementation](../M77_IMPLEMENTATION.md), [uat](../M77_UAT.md) |
| M78 | [goals](../M78_GOALS.md), [implementation](../M78_IMPLEMENTATION.md), [uat](../M78_UAT.md) |
| M79 | [goals](../M79_GOALS.md), [implementation](../M79_IMPLEMENTATION.md), [uat](../M79_UAT.md) |
| M80 | [goals](../M80_GOALS.md), [implementation](../M80_IMPLEMENTATION.md), [uat](../M80_UAT.md) |
| M81 | [goals](../M81_GOALS.md), [implementation](../M81_IMPLEMENTATION.md), [uat](../M81_UAT.md) |
| M82 | [deferred](../M82_DEFERRED.md) |
| M83 | [goals](../M83_GOALS.md), [implementation](../M83_IMPLEMENTATION.md), [uat](../M83_UAT.md) |
| M84 | [goals](../M84_GOALS.md), [implementation](../M84_IMPLEMENTATION.md), [uat](../M84_UAT.md) |
| M85 | [goals](../M85_GOALS.md), [implementation](../M85_IMPLEMENTATION.md), [uat](../M85_UAT.md) |
| M86 | [goals](../M86_GOALS.md), [handover](../M86_HANDOVER.md), [implementation](../M86_IMPLEMENTATION.md), [uat](../M86_UAT.md) |
| M87 | [audit](../M87_AUDIT.md), [goals](../M87_GOALS.md), [handover](../M87_HANDOVER.md), [headless](../M87_HEADLESS.md), [implementation](../M87_IMPLEMENTATION.md), [uat](../M87_UAT.md) |
| M88 | [audit](../M88_AUDIT.md), [goals](../M88_GOALS.md), [handover](../M88_HANDOVER.md), [implementation](../M88_IMPLEMENTATION.md), [uat](../M88_UAT.md) |
| M89 | [goals](../M89_GOALS.md), [implementation](../M89_IMPLEMENTATION.md), [uat](../M89_UAT.md) |
| M90 | [goals](../M90_GOALS.md), [implementation](../M90_IMPLEMENTATION.md), [uat](../M90_UAT.md) |
| M91 | [goals](../M91_GOALS.md), [implementation](../M91_IMPLEMENTATION.md), [uat](../M91_UAT.md) |
| M92 | [goals](../M92_GOALS.md), [implementation](../M92_IMPLEMENTATION.md), [uat](../M92_UAT.md), [visual audit](../M92_VISUAL_AUDIT.md) |
| M93 | [goals](../M93_GOALS.md), [implementation](../M93_IMPLEMENTATION.md), [qualification](../M93_QUALIFICATION.md) |
| M94 | [closure](../M94_CLOSURE.md), [drag optimization](../M94_DRAG_OPTIMIZATION.md), [goals](../M94_GOALS.md), [implementation](../M94_IMPLEMENTATION.md), [navigation optimization](../M94_NAVIGATION_OPTIMIZATION.md), [performance diagnosis](../M94_PERFORMANCE_DIAGNOSIS.md) |
| M95 | [closure](../M95_CLOSURE.md), [goals](../M95_GOALS.md), [implementation](../M95_IMPLEMENTATION.md), [qualification](../M95_QUALIFICATION.md) |
| M96 | [closure](../M96_CLOSURE.md), [f002](../M96_F002.md), [f005](../M96_F005.md), [f006](../M96_F006.md), [goals](../M96_GOALS.md), [implementation](../M96_IMPLEMENTATION.md) |
| M97 | [authoring implementation](../M97_AUTHORING_IMPLEMENTATION.md), [authoring metadata](../M97_AUTHORING_METADATA.md), [closure](../M97_CLOSURE.md), [goals](../M97_GOALS.md), [implementation](../M97_IMPLEMENTATION.md), [priority dimensions](../M97_PRIORITY_DIMENSIONS.md), [qualification](../M97_QUALIFICATION.md) |
| M98 | [authoring preview](../M98_AUTHORING_PREVIEW.md), [authoring quickstart](../M98_AUTHORING_QUICKSTART.md), [bake contract](../M98_BAKE_CONTRACT.md), [bake handoff](../M98_BAKE_HANDOFF.md), [collaboration](../M98_COLLABORATION.md), [engine implementation](../M98_ENGINE_IMPLEMENTATION.md), [field lifecycle](../M98_FIELD_LIFECYCLE.md), [goals](../M98_GOALS.md), [handoff](../M98_HANDOFF.md), [hardening](../M98_HARDENING.md), [implementation plan](../M98_IMPLEMENTATION_PLAN.md), [loading feedback](../M98_LOADING_FEEDBACK.md), [local canvas](../M98_LOCAL_CANVAS.md), [navigation](../M98_NAVIGATION.md), [navigation latency](../M98_NAVIGATION_LATENCY.md), [playground](../M98_PLAYGROUND.md), [qualification](../M98_QUALIFICATION.md), [takeover](../M98_TAKEOVER.md), [task brief](../M98_TASK_BRIEF.md), [tool parity](../M98_TOOL_PARITY.md), [uat](../M98_UAT.md), [workspace storage](../M98_WORKSPACE_STORAGE.md) |
| M99 | [cleanup](../M99_CLEANUP.md), [closure](../M99_CLOSURE.md), [qualification](../M99_QUALIFICATION.md) |
| M100 | [final cleanup](../M100_FINAL_CLEANUP.md) |
