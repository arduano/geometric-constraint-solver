<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Architecture decisions

These records explain why GeoSolve uses its current numerical, domain and authoring
boundaries. Read [Architecture](../../ARCHITECTURE.md) first for the current system,
[API compatibility](../API_COMPATIBILITY.md) for supported formats, and
[Authoring](../AUTHORING.md) for the code/UI workflow.

ADRs preserve the context and scope at the time of each decision. Old milestone
names, prototype APIs and historical verification sections are not current setup
instructions. Later accepted amendments supersede the affected details; the stable
principles include separate sketch/linkage domains, native residual validation,
explicit branches and presentation-independent interaction.

In particular, ADR 0010 records the original browser prototype; shared native
interaction now owns its picking and tool policies. ADR 0030 is archived in favor
of computed-feature authoring. ADR 0041 describes the first optional code layer;
ADR 0043 and its M90 amendment replace the original managed parser/execution model.
Current source compilation uses V4 receipts with bounded V3 compatibility. Missing
numbers correspond to withdrawn designs and remain available in repository history.

| Decision | Topic |
| --- | --- |
| [0001](0001-shared-kernel-separate-domains.md) | Shared numerical kernel, separate domain models |
| [0002](0002-planar-workplane-se2.md) | Local 2D workplanes and SE(2) mechanism bodies |
| [0003](0003-parametric-curve-contact.md) | Parametric curve contact uses latent parameters |
| [0004](0004-equation-audit-view.md) | Structured equation audit view |
| [0005](0005-component-local-linearization-and-ad.md) | Canonical component-local linearization and local AD |
| [0006](0006-pose-manifold-convention.md) | Pose2 and Pose3 manifold convention |
| [0007](0007-persistent-solve-session-and-bounds.md) | Persistent SolveSession, bounds and active sets |
| [0008](0008-sketch-design-graph-and-persistence.md) | Sketch document, commands, persistent IDs and closed curve definitions |
| [0009](0009-physical-grounding-and-numerical-gauge.md) | Physical grounding and numerical gauge are distinct |
| [0010](0010-disposable-sketch-playground.md) | Disposable browser sketch playground |
| [0011](0011-adaptive-pseudo-arclength-continuation.md) | Adaptive and pseudo-arclength continuation |
| [0012](0012-sparse-backend-and-rank-authority.md) | Sparse backend and rank authority |
| [0013](0013-spatial-features-mates-and-modes.md) | Spatial features, mates and assembly modes |
| [0014](0014-bspline-span-and-periodic-topology.md) | B-spline spans, periodic topology and refinement |
| [0015](0015-nurbs-gauge-and-differential-continuity.md) | NURBS weight gauge and differential continuity |
| [0016](0016-spatial-continuation-publication-and-gauge.md) | Spatial continuation publication and gauge |
| [0017](0017-spatial-velocity-fields-and-motion-bases.md) | Spatial velocity fields and motion bases |
| [0018](0018-spatial-document-persistence.md) | Spatial assembly document persistence |
| [0019](0019-sketch-extension-identity-and-attributes.md) | Sketch extension identity and typed attributes |
| [0020](0020-associative-linear-constructions.md) | Associative linear constructions and sketch JSON v2 |
| [0021](0021-visual-line-profile-analysis.md) | Visual-only line-profile analysis |
| [0022](0022-associative-line-fillet.md) | Associative line-line fillet foundation |
| [0023](0023-generic-fillet-trim-views.md) | Generic fillets and persistent trim views |
| [0024](0024-all-family-visual-profile-analysis.md) | All-family visual profile analysis |
| [0025](0025-retained-design-attempt-and-accepted-state.md) | Retained design, attempt and accepted-state identity |
| [0026](0026-immutable-host-inputs-and-external-snapshots.md) | Immutable host inputs and external snapshots |
| [0027](0027-cancellation-and-prepared-concurrency.md) | Cancellation, work exhaustion and prepared concurrency |
| [0028](0028-sketch-operations-and-production-topology-companions.md) | Sketch operations and production topology companions |
| [0029](0029-headless-constraint-editor-state-machine.md) | Headless constraint-editor state machine |
| [0030](0030-headless-sketch-operation-authoring.md) | Headless sketch-operation authoring |
| [0031](0031-computed-sketch-features.md) | Computed sketch features and revision-local output topology |
| [0032](0032-headless-computed-feature-direct-manipulation.md) | Headless computed-feature direct manipulation |
| [0033](0033-profile-and-construction-geometry-semantics.md) | Profile and construction geometry semantics |
| [0034](0034-headless-auto-constraint-drafting-intelligence.md) | Headless auto-constraint drafting intelligence |
| [0035](0035-retained-drafting-relation-lifecycle.md) | Retained drafting relation lifecycle and persistence |
| [0036](0036-headless-geometry-variants-and-atomic-recipes.md) | Headless geometry variants and atomic construction recipes |
| [0037](0037-native-topology-preserving-profile-offset.md) | Native topology-preserving Profile Offset |
| [0040](0040-projectional-design-intent-graph.md) | Projectional design-intent graph |
| [0041](0041-optional-code-gui-sketch-authoring.md) | Optional code/GUI sketch authoring |
| [0042](0042-managed-controls-and-headless-authoring.md) | Managed controls and headless authoring |
| [0043](0043-executed-reversible-managed-sketches.md) | executed, reversible managed sketches |
