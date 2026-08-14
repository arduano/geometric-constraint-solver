<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M73 proposal — Retained authoring semantic consolidation

Status: proposal moved from M72 after the approved M72 replacement on 2026-08-14. M73 is **not
activated**; this document authorizes no implementation, API change or constraint-catalog
expansion until the supervising caller explicitly accepts the scope.

## Recommendation

Take one foundation-first, behavior-preserving milestone before promoting more of the M37 catalog
or adding new inference families. M71 delivered the intended user behavior and exposed two narrow
places where equivalent semantic knowledge is still represented twice. Consolidating those seams
first makes a later retained-catalog expansion smaller and less likely to diverge between explicit
authoring, inferred authoring and host adapters.

## Motivation carried from M71

1. `construction_point_stage` and `draft_inference_subject` classify closely related construction
   stages through separate exhaustive `EditorTool` matches. Center-bearing capability,
   coordinate-only stages and prospective-curve participation should come from one internal
   semantic description rather than duplicated tool knowledge.
2. The compatibility `available_constraints`/`constraint_edit` surface and contextual
   `resolve_constraint` coordinator surface each contain relation applicability knowledge. M71-F002
   fixed one concrete divergence, but the lasting law should be one shared semantic predicate or
   an explicit, mechanically enforced parity boundary.
3. M71-F004/F005 showed that preview identity, confirmed-reference handoff and retained commit
   intent must describe the same semantic bundle. Consolidation must preserve that law rather than
   merely making similar code look alike.

## Proposed scope

- Introduce one internal construction-stage semantic descriptor consumed by explicit and inferred
  authoring. It should state subject capability, coordinate-bearing stage, centered-construction
  capability and prospective geometry slots without adding browser policy.
- Derive direct and contextual relation applicability from one owning predicate where their public
  contracts overlap. If a compatibility surface cannot share the predicate, document the
  deliberate difference and enforce a complete parity matrix at the boundary.
- Keep candidate identity, preview guides, confirmed semantic references and commit-plan relations
  traceable to one semantic source description through line and polyline handoff.
- Add a focused native parity matrix for existing retained relation families, selection orders,
  missing/foreign operands, typed disabled reasons and stale accepted-scene input. Add thin WASM
  coverage only for information crossing the adapter.
- Preserve the reviewed 234-row golden unless discovery proves a missing systemic dimension.
- Run formatting, warnings-denied workspace Clippy, locked all-feature tests, relevant native/WASM
  parity and the complete clean release gate before nomination, followed by focused human UAT.

The exact internal type names are deliberately not part of this proposal. Public API growth needs
separate justification; an internal consolidation is preferred when it can express the full law.

## Proposed acceptance

- Existing M70/M71 authoring and inference behavior remains unchanged, including explicit operand
  order, ambiguity, hysteresis, resource bounds, candidate identity, line/polyline handoff and
  transactional rejection.
- Equivalent direct and contextual selections produce equivalent applicability, definitions and
  disabled reasons; foreign, missing or stale selections cannot be advertised by one path and
  rejected as structurally absent by the other.
- Centered construction, ordinary point construction and coordinate-only stages are classified by
  one owner and retain the existing prospective-curve and point-identity precedence behavior.
- The browser continues to render and dispatch public DTOs without equations, applicability rules
  or inference ranking.
- No residual, Jacobian, solver priority, branch state, canonical sketch v1-v4 byte or unsupported
  draft-v5 meaning changes. Any discovered need for one of those changes stops this scope for a new
  decision.
- Direct qualification, unchanged-golden evidence, clean release qualification, immutable
  publication and an explicitly approved focused UAT all pass.

## Non-goals

This proposal does not yet promote the remaining M37 relations, broaden point-reference operands,
add generic intersections or quadrant anchors, infer nonlinear tangent/normal or equality/symmetry,
add host axes/grids/increments, persist wake state, freeze canonical sketch v5, chain computed
features, restore browser E2E, or add mobile behavior.

## Activation decision

Before implementation, the supervising caller should choose one of these directions:

1. accept this recommended behavior-preserving consolidation as M73;
2. replace it with a deliberately scoped retained M37 catalog expansion; or
3. prioritize a different deferred product boundary such as canonical-v5 planning.

If this proposal is accepted, add the final M73 acceptance section, scenario contract and any
required ADR before changing implementation code. Until then, M72 remains the active milestone
and no M73 work is active.
