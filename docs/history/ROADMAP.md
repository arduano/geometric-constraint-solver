<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Historical roadmap

This index preserves the milestone sequence and earlier section anchors. Detailed
requirements now live in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md),
[architecture reference](../reference/ARCHITECTURE_DETAILS.md), [scenarios](../SCENARIOS.md)
and individual milestone records. Repeated worklists and operational logs have been
consolidated. Consult the [current roadmap](../../PLAN.md) for active work and
[current acceptance status](../../ACCEPTANCE.md) for outstanding human review.

## Product deliverables

The two domain deliverables are constrained 2D sketches and planar/spatial rigid-body kinematics. The browser workbench and code/server hosts demonstrate their use.

### Deliverable 1: production-capable 2D CAD sketches

Historical subdivision; requirements and results are retained in the linked milestone and acceptance records.

### Deliverable 2: 2D and 3D rigid-body kinematics

Historical subdivision; requirements and results are retained in the linked milestone and acceptance records.

### User-approved next cut: 2D Sketch Playground Alpha

Historical subdivision; requirements and results are retained in the linked milestone and acceptance records.

### Production embedding north star

Historical subdivision; requirements and results are retained in the linked milestone and acceptance records.

## Architectural boundaries

Keep sketch/linkage domain models separate over a pure Rust core; use explicit branch state, independent validation and presentation-independent interaction. See the current [ownership map](../../ARCHITECTURE.md#ownership-map).

## Frozen baseline: M0-M7

Established representation, nonlinear solving, adversarial tests, diagnostics, sketch constraints, temporary/preference hierarchy and planar mechanisms. The permanent M1–M7 behavior is retained in the acceptance reference and scenario corpus.

## Common milestone gate

Applicable format, Clippy, native/WASM, finite-difference, golden, browser and performance checks remain required. See [Release qualification](../RELEASE_QUALIFICATION.md) for the current runner.

## M8: contract rebaseline and representative baselines

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M9: canonical component-local linearization and local AD

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M10: persistent solve sessions and first-class bounds

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M11: persistent SketchDocument, commands and history

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M12: editable Bezier curves and generic curve constraints

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M13: disposable browser playground

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M14: playground hardening and alpha gate

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M15: manifold geometry and spatial state

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M16: sparse structure, hierarchy and continuation

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M17: shared planar kinematic architecture

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M18: spatial kinematics vertical slice

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M19: ellipses and parametric conics

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M20: spatial mate and joint catalog

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M21: non-rational B-splines

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M22: NURBS and advanced CAD constraints

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M23: 2D/3D assembly completion

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M24: sketch extension and embedding foundation

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M25: associative linear constructions

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M26: visual line-profile detection

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M27: associative line fillet foundation

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M28: generic fillets and parent trimming

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M29: public API and release hardening

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M30: interactive construction and NURBS UAT

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M31: all-family visual profile analysis

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M32: post-expansion UAT and release hardening

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M33: CAD engine contract and baselines

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M34: retained design and accepted solved state

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M35: cancellation and operation control

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M36: semantic feature and scalar foundations

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M37: standard planar constraint catalog

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M38: dimensions and persistent measurements

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M39: CAD workbench foundation and core authoring

Detailed requirements and outcomes remain in the [acceptance reference](../reference/ACCEPTANCE_DETAILS.md) and [scenarios](../SCENARIOS.md).

## M40 pivot: mechanically qualified constraint editing

Records: [uat](../M40_UAT.md), [headless qualification](../M40_HEADLESS_QUALIFICATION.md).

### Post-M40 headless interaction ownership audit (2026-07-26)

The audit moved deterministic editing policy into the Rust headless adapter. See [ADR 0029](../adr/0029-headless-constraint-editor-state-machine.md).

#### Requirements

Historical subdivision; requirements and results are retained in the linked milestone and acceptance records.

#### Evidence and source pointers

Historical subdivision; requirements and results are retained in the linked milestone and acceptance records.

#### Decisions / inferred constraints

Historical subdivision; requirements and results are retained in the linked milestone and acceptance records.

#### Open questions

Historical subdivision; requirements and results are retained in the linked milestone and acceptance records.

#### Out of scope

Historical subdivision; requirements and results are retained in the linked milestone and acceptance records.

### M40.1: headless editor contract and transition inventory

Records: [uat](../M40_UAT.md), [headless qualification](../M40_HEADLESS_QUALIFICATION.md).

### M40.2: accepted scene, picking and selection foundation

Records: [uat](../M40_UAT.md), [headless qualification](../M40_HEADLESS_QUALIFICATION.md).

### M40.3: headless drafting, snapping and projection gestures

Records: [uat](../M40_UAT.md), [headless qualification](../M40_HEADLESS_QUALIFICATION.md).

### M40.4: headless edit lifecycle, history and diagnostics

Records: [uat](../M40_UAT.md), [headless qualification](../M40_HEADLESS_QUALIFICATION.md).

### M40.5: thin desktop web adapter

Records: [uat](../M40_UAT.md), [headless qualification](../M40_HEADLESS_QUALIFICATION.md).

### M40.6: automated core-interaction qualification

Records: [uat](../M40_UAT.md), [headless qualification](../M40_HEADLESS_QUALIFICATION.md).

### M40.7: human UAT 1 - core sketch interaction

Records: [uat](../M40_UAT.md), [headless qualification](../M40_HEADLESS_QUALIFICATION.md).

## M41: construction roles and activation

Records: [implementation](../M41_IMPLEMENTATION.md).

## M42: typed host parameters

Records: [implementation](../M42_IMPLEMENTATION.md).

## M43: immutable external 2D references

Records: [implementation](../M43_IMPLEMENTATION.md).

## M44: host-state workbench integration

Records: [implementation](../M44_IMPLEMENTATION.md).

## M45: cleanup investigation and UAT-point capture

Records: [cleanup plan](../M45_CLEANUP_PLAN.md), [test fixture cleanup investigation](../M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md), [ui cleanup investigation](../M45_UI_CLEANUP_INVESTIGATION.md).

## Post-cleanup numbering record

M46–M53 describe the staged migration away from legacy browser and host-state surfaces. The phase names below preserve the original sequence.

## Pre-cleanup phase

Replace direct ownership and regression coverage before deleting legacy implementations.

### M46: direct-test ownership freeze

Records: [direct test replacement](../M46_DIRECT_TEST_REPLACEMENT.md), [rebase inventory](../M46_REBASE_INVENTORY.md).

### M47: focused host-state replacement and M44 purge

Records: [implementation](../M47_IMPLEMENTATION.md).

### M48: direct workbench qualification and M40 purge

Records: [implementation](../M48_IMPLEMENTATION.md).

### M49: legacy semantic extraction

Records: [implementation](../M49_IMPLEMENTATION.md).

## Cleanup cut

Remove legacy surfaces only after their owning replacements pass.

### M50: old E2E and legacy application purge

Records: [implementation](../M50_IMPLEMENTATION.md).

## Post-cleanup phase

Consolidate the surviving workbench and qualify the host semantics.

### M51: single-workbench consolidation and hardening

Records: [implementation](../M51_IMPLEMENTATION.md).

### M52: post-cleanup host-semantics UAT candidate

Records: [implementation](../M52_IMPLEMENTATION.md).

### M53: human UAT 2 - CAD host semantics

Records: [uat](../M53_UAT.md).

## Post-M53 functional and release sequence

The following milestones extend diagnostics, interaction, topology and authored source, with intervening consolidation and acceptance checkpoints.

### M54: stable diagnostics and mobility evidence

Records: [implementation](../M54_IMPLEMENTATION.md).

### M55: alpha constraint, dimension and branch-action parity

Records: [implementation](../M55_IMPLEMENTATION.md), [contextual authoring](../M55_CONTEXTUAL_AUTHORING.md).

#### M55 contextual-authoring follow-up during M61 remediation

Records: [implementation](../M55_IMPLEMENTATION.md), [contextual authoring](../M55_CONTEXTUAL_AUTHORING.md).

### M56: prepared jobs and concurrency contract

Records: [implementation](../M56_IMPLEMENTATION.md).

### M57: incremental solving and production scale

Records: [implementation](../M57_IMPLEMENTATION.md).

### M58: sketch operations companion

Records: [implementation](../M58_IMPLEMENTATION.md).

### M59: production topology companion

Records: [implementation](../M59_IMPLEMENTATION.md).

### M60: advanced workbench completion

Records: [implementation](../M60_IMPLEMENTATION.md).

### M61: human UAT 3 - advanced geometry and topology

Records: [uat](../M61_UAT.md), [remediation](../M61_REMEDIATION.md).

### M62: CAD-style constraint and dimension authoring

Records: [implementation](../M62_IMPLEMENTATION.md), [uat](../M62_UAT.md).

### M63: canvas constraint visualization and interaction

Records: [implementation](../M63_IMPLEMENTATION.md), [uat](../M63_UAT.md).

### M64: editable sample library and scenario-harness cleanup

Records: [implementation](../M64_IMPLEMENTATION.md), [uat](../M64_UAT.md).

### M65: predictable bounded projected dragging

Records: [implementation](../M65_IMPLEMENTATION.md), [uat](../M65_UAT.md).

### M66: computed 2D Fillet features

Records: [implementation](../M66_IMPLEMENTATION.md), [uat](../M66_UAT.md).

### M67

Records: [implementation](../M67_IMPLEMENTATION.md), [uat](../M67_UAT.md), [m40 ownership](../M67_M40_OWNERSHIP.md).

### M68

Records: [implementation](../M68_IMPLEMENTATION.md), [uat](../M68_UAT.md).

### M69

Records: [implementation](../M69_IMPLEMENTATION.md), [uat](../M69_UAT.md).

### M70

Records: [implementation](../M70_IMPLEMENTATION.md), [uat](../M70_UAT.md).

### M70B

Records: [implementation](../M70B_IMPLEMENTATION.md), [uat](../M70B_UAT.md), [hardening](../M70B_HARDENING.md).

### M71

Records: [goals](../M71_GOALS.md), [implementation](../M71_IMPLEMENTATION.md), [uat](../M71_UAT.md), [handover](../M71_HANDOVER.md).

### M72: public workbench bulk fixes

Records: [goals](../M72_GOALS.md), [implementation](../M72_IMPLEMENTATION.md), [uat](../M72_UAT.md).

### M73: retained authoring semantic consolidation

Records: [goals](../M73_GOALS.md), [implementation](../M73_IMPLEMENTATION.md), [uat](../M73_UAT.md).

### M74: production-style sketch reference UX

Records: [goals](../M74_GOALS.md), [implementation](../M74_IMPLEMENTATION.md), [uat](../M74_UAT.md).

### M75: hover and primary pointer-owner parity

Records: [goals](../M75_GOALS.md), [implementation](../M75_IMPLEMENTATION.md), [uat](../M75_UAT.md).

### M76: production-quality constraint annotations

Records: [goals](../M76_GOALS.md), [implementation](../M76_IMPLEMENTATION.md), [uat](../M76_UAT.md).

### M77: CAD curve handles and implicit-parameter editing

Records: [goals](../M77_GOALS.md), [implementation](../M77_IMPLEMENTATION.md), [uat](../M77_UAT.md).

### M78: CAD geometry tool families and authoring variants

Records: [goals](../M78_GOALS.md), [implementation](../M78_IMPLEMENTATION.md), [uat](../M78_UAT.md).

### M79: stable inference candidate cycling and recovery

Records: [goals](../M79_GOALS.md), [implementation](../M79_IMPLEMENTATION.md), [uat](../M79_UAT.md).

### M80: native topology-preserving Profile Offset

Records: [goals](../M80_GOALS.md), [implementation](../M80_IMPLEMENTATION.md), [uat](../M80_UAT.md).

### M81: core architecture consolidation

Records: [goals](../M81_GOALS.md), [implementation](../M81_IMPLEMENTATION.md), [uat](../M81_UAT.md).

### M82: computed all-family Offset design exploration

Records: [deferred](../M82_DEFERRED.md).

Deferred design exploration; no computed all-family Offset feature is claimed.

### M83: projectional sketch design intent

Records: [goals](../M83_GOALS.md), [implementation](../M83_IMPLEMENTATION.md), [uat](../M83_UAT.md).

### M84: optional code/GUI sketch authoring

Records: [goals](../M84_GOALS.md), [implementation](../M84_IMPLEMENTATION.md), [uat](../M84_UAT.md).

### M85: responsive retained workbench presentation

Records: [goals](../M85_GOALS.md), [implementation](../M85_IMPLEMENTATION.md), [uat](../M85_UAT.md).

### M86: focused bug fixes and UAT follow-up

Records: [goals](../M86_GOALS.md), [implementation](../M86_IMPLEMENTATION.md), [uat](../M86_UAT.md), [handover](../M86_HANDOVER.md).

### M87: cohesive managed parameters and browser-free design loop

Records: [goals](../M87_GOALS.md), [implementation](../M87_IMPLEMENTATION.md), [uat](../M87_UAT.md), [audit](../M87_AUDIT.md).

### M88: workflow-led authoring workbench redesign

Records: [goals](../M88_GOALS.md), [implementation](../M88_IMPLEMENTATION.md), [uat](../M88_UAT.md), [audit](../M88_AUDIT.md).

### M89: executed, reversible managed sketches

Records: [goals](../M89_GOALS.md), [implementation](../M89_IMPLEMENTATION.md), [uat](../M89_UAT.md).

## M90 — typed executed sketch clean break

Records: [goals](../M90_GOALS.md), [implementation](../M90_IMPLEMENTATION.md), [uat](../M90_UAT.md).

## M91 — cohesive code-driven authoring

Records: [goals](../M91_GOALS.md), [implementation](../M91_IMPLEMENTATION.md), [uat](../M91_UAT.md).

## M92 — advanced sample showcase and scale corpus

Records: [goals](../M92_GOALS.md), [implementation](../M92_IMPLEMENTATION.md), [uat](../M92_UAT.md), [visual audit](../M92_VISUAL_AUDIT.md).

### Pre-pruning implementation and qualification evidence

M92 qualification retained complete sample/code/browser witnesses before pruning the catalog. See [implementation](../M92_IMPLEMENTATION.md).

### M92 visual-audit continuation

The [visual audit](../M92_VISUAL_AUDIT.md) records bounded captures and dispositions without converting automated checks into human acceptance.

## M93 — fast, proportional release qualification

Records: [goals](../M93_GOALS.md), [implementation](../M93_IMPLEMENTATION.md), [qualification](../M93_QUALIFICATION.md).

## M94 — accelerated canvas viewport

Records: [closure](../M94_CLOSURE.md), [goals](../M94_GOALS.md), [implementation](../M94_IMPLEMENTATION.md), [drag optimization](../M94_DRAG_OPTIMIZATION.md).

## M95 — connected code, Explorer and canvas selection

Records: [closure](../M95_CLOSURE.md), [goals](../M95_GOALS.md), [implementation](../M95_IMPLEMENTATION.md), [qualification](../M95_QUALIFICATION.md).

## M96 — finite-width manifold channels and silicone grooves

Records: [closure](../M96_CLOSURE.md), [goals](../M96_GOALS.md), [implementation](../M96_IMPLEMENTATION.md), [f002](../M96_F002.md).

## M97 — focused dimensions and stable annotation placement

Records: [closure](../M97_CLOSURE.md), [goals](../M97_GOALS.md), [implementation](../M97_IMPLEMENTATION.md), [qualification](../M97_QUALIFICATION.md).

## M98 — reliable project authoring and embeddable TypeScript engine

Records: [goals](../M98_GOALS.md), [qualification](../M98_QUALIFICATION.md), [uat](../M98_UAT.md), [implementation plan](../M98_IMPLEMENTATION_PLAN.md).

Mechanically qualified and delivered. Human U02 remains **Fail pending human recheck**; unperformed human rows remain **Not run**.

### Historical fast-track prototype

The initial M98 folder prototype preceded full engine and collaboration hardening. Its [handoff](../M98_HANDOFF.md) is historical; use [Getting started](../GETTING_STARTED.md) for current commands.

### M98-B — bounded baked-profile data follow-up

Added headless planar profile export through Rust topology and sampling. See the [bake contract](../M98_BAKE_CONTRACT.md); solid modeling remains a downstream concern.

## M99 — shared authoring and host infrastructure

Records: [closure](../M99_CLOSURE.md), [cleanup](../M99_CLEANUP.md), [qualification](../M99_QUALIFICATION.md).

Accepted and closed on 2026-09-13; 297/297 qualification obligations pass for the recorded product.

## M100 — final cleanup and pause readiness

Records: [final cleanup](../M100_FINAL_CLEANUP.md).

In progress. The repository-wide documentation pass is one part of final cleanup; product nomination and pause readiness remain open.

## Explicit non-goals

Solid modeling, dynamics, global root enumeration, arbitrary residual/curve plugins and mobile workbench support require separate product decisions. See the [current scope](../../PLAN.md#explicit-non-goals).
