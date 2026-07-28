<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M41 implementation record

Status: complete as of 2026-07-27. This document records the implemented decisions and
qualification evidence; M42-M52 subsequently completed and M53 now owns human UAT.

## Requirements

- M41 must make regular versus construction geometry a persistent domain role. Construction geometry continues to participate in solving, but is excluded from production profiles under the default declared scope (`PLAN.md` M41; `ACCEPTANCE.md` M41).
- Effective activation must be explicit, dependency-safe, and explain every inactive result with a closed reason set. Inactive branch/contact/topology state must survive reactivation without coordinate inference.
- All M1-M40 behavior remains mandatory: no false-success path, independently validated accepted state, transactional rejection, deterministic ordering, explicit discrete state, and frozen v1-v4 languages (`PLAN.md` common gate; `ACCEPTANCE.md` global/frozen gates).
- The implementation must preserve the M34 split among retained design, optional attempt, and independently accepted state. M41 is a domain-state extension, not a reason to label retained/attempted geometry accepted (`ARCHITECTURE.md` §5; `PLAN.md` M34).
- v1-v4 are frozen readers/writers with canonical v4 output. A draft-v5 may be developed before M107X but is unsupported and must not silently extend any existing schema (`docs/API_COMPATIBILITY.md` §§Persistence and API tiers).

## Current model

`SketchDocument` persists `DocumentConstraint` and `DocumentDimension` records. Each has a `source_id`, label, and `suppressed: bool`; dimensions additionally retain a driving/reference mode (`crates/geosolve-sketch/src/document.rs:988-1068`). `SketchDocument::source` exposes the same flag through `DocumentSourceRef`, in `source_order` semantic/audit order (`document.rs:2069-2105`).

Current suppression is source-only and mutable through `SketchDocument::set_source_suppressed`; it finds a constraint or dimension by source ID, changes only that Boolean, and errors for an unknown ID (`document.rs:5907-5934`). During document lowering, all points and curves are lowered first; sources remain in `DocumentRuntimeMap::sources` in source order, but a suppressed source receives `runtime: None` and is not lowered (`document_lowering.rs:321-379`). Thus M41 must replace this local Boolean test with one shared effective-activity result rather than add divergent lowering/profile checks.

Profiles already treat suppression as an eligibility input: coincident constraints used for welding exclude suppressed records, while profile join construction generally skips suppressed records with a deliberate curve-fillet exception (`profiles.rs:1178-1213`, `1536-1577`). M41 must route these policy decisions through declared roles and effective activity, preserving the exception only if it remains semantically required after dependency closure.

## Proposed public and persistence contract

`DocumentElementId` already names the whole persistent graph—document, point, scalar, curve, contact, constraint, dimension, and source—and intentionally does not use runtime IDs (`document.rs:1081-1144`). It is the correct identity seam for activation diagnostics/dependency edges. `DocumentSourceOwner` remains narrower (constraint or dimension), so source audit information cannot by itself represent entity/contact/association activity (`document.rs:1146-1160`).

Working constraint: retain v1-v4 byte-for-byte semantics. `SketchDocumentV4` serializes its exact v4 graph fields under `deny_unknown_fields` (`document.rs:1783-1837`); API policy forbids extending persisted types within a schema version and requires a new schema for fields/variants (`docs/API_COMPATIBILITY.md:52-55,70-97`). New role/activation fields therefore belong only in an explicitly draft-v5 envelope/DTO (or a separately versioned additive M41 state envelope), never in a v1-v4 request/document type. M107X is the freeze point, so any draft-v5 representation remains unsupported until then (`API_COMPATIBILITY.md:15-23`).

The persisted role must apply beyond sources because the plan explicitly generalizes activation over entities, constraints, dimensions, and associations (`PLAN.md:1854-1858`). A public activity report should be keyed by `DocumentElementId`, with source mappings reporting the derived activity of their owners, rather than attempting to overload `DocumentSourceRef::suppressed`.

## Dependency-closure algorithm

Target behavior: compute effective activity from explicit requested state plus graph dependency closure, emitting a deterministic, closed inactivity reason for each inactive element/source. At minimum the public diagnosis must distinguish the four plan-mandated causes: user suppression, host-configuration inactivity, unavailable dependency, and unavailable external reference (`PLAN.md:1856-1858`). The closure must suppress lowering/profile eligibility without deleting retained records or rewriting branch, span, winding, contact, trim, or ownership bytes. It must be calculated before lowering, since existing lowering creates all geometric runtime objects before skipping only suppressed equations (`document_lowering.rs:321-379`).

Existing semantic decisions must also consume this closure. For example, `curve_branch_is_enforced` currently derives enforcement from unsuppressed axis constraints, unsuppressed driving length dimensions, and unsuppressed sided fillets (`document.rs:6791-6830`); trim projection similarly decides fillet ownership from unsuppressed constraints (`document.rs:2440-2469`). These must query effective activity, never infer branch state from retained geometry.

## Implementation slices

1. Inventory/document state and persistence seams; characterize v1-v4 bytes and M34 suppression behavior.
2. Add draft-only role/requested-activation data and deterministic effective-activity closure.
3. Route lowering, accepted projection/audit, and default profile eligibility through effective activity; retain inactive discrete bytes.
4. Add public mutations, retained-session/history/persistence behavior, and focused regressions; keep M44 UI work out of M41.

The current session mutation seam is `DocumentEdit::SetSourceSuppressed`, dispatched to `set_source_suppressed` and reported as `DocumentCommandEffect::UpdatedSource` (`document_session.rs:619-759,3650-3664`). M41 should add role/activation edits and effects at this domain seam, performing closure validation transactionally before a design revision is published.

## Qualification matrix

Required vectors include construction curves that remain constrainable but are excluded from default profiles; direct suppression/unsuppression; each of the four named inactivity causes; transitive dependency inactivity/reactivation; exact preservation of branch/span/winding/contact/ownership bytes; v1-v4 golden/canonical compatibility; retained-unsolved/attempt/accepted separation; deterministic closure ordering; rollback; and required scale/validation regressions where a lowered residual remains active.

The existing lifecycle regression is the immediate baseline: after a failed `SetSourceSuppressed { suppressed: false }`, design contains the unsuppressed intent but accepted state retains the prior suppressed source; re-suppression can subsequently publish a new accepted state (`tests/m34_lifecycle.rs:356-403`). Role and activation edits need the same retained-design/attempt/accepted vector. Existing persistence tests also establish that persistent-element/source views cover the full graph and source-order mapping (`tests/m24.rs:17-90`), while M24 rejects a v5 payload against the frozen v4 reader (`tests/m24.rs:269-288`).

## Evidence and source pointers

- `PLAN.md:1848-1862` is the authoritative M41 scope and atomicity gate; `ACCEPTANCE.md:732-736` supplies the three acceptance outcomes.
- `crates/geosolve-sketch/src/document.rs:988-1068,1081-1160,1783-1837,5907-5934,6791-6830` identifies current persistent source suppression, graph identity, v4 DTO, mutation, and branch-enforcement seams.
- `crates/geosolve-sketch/src/document_lowering.rs:321-379` identifies the deterministic source-lowering seam; `profiles.rs:1178-1213,1536-1577` identifies current suppression-dependent visual-profile logic.
- `docs/adr/0025-retained-design-attempt-and-accepted-state.md:55-100` requires immutable activation stamp evidence and forbids inference for inactive elements; `docs/adr/0026-immutable-host-inputs-and-external-snapshots.md:104-124` allocates activation inputs/reasons to M41.
- `docs/adr/0024-all-family-visual-profile-analysis.md:19-30,71-84` defines current visual profiles as accepted, read-only, nonpersistent analysis with active validated joins. `docs/adr/0028-sketch-operations-and-production-topology-companions.md:146-170` keeps later production topology distinct from visual profiles.

## Decisions / inferred constraints

- Treat current `suppressed` as the M40-compatible user-suppression input, not as the complete effective-activation model. Preserve it unchanged for v1-v4 documents and migrate it deterministically into the draft M41 model.
- Compute one immutable, ordered effective-activity snapshot before lowering, profile analysis, branch-enforcement checks, or ownership-dependent edits. Every consumer must use that snapshot so an inactive dependency cannot be evaluated through a side path.
- Construction is a geometry role, not deactivation: construction geometry remains eligible for solver constraints. Default profile scope excludes it, but a future explicitly declared scope may include it.
- Do not alter `visible_interval`, trim, contact, branch, sweep, winding, or association ownership merely because an element is inactive. Reactivation restores the retained explicit state; it does not project or choose from coordinates.
- Keep visual profile analysis/read-only behavior separate from M104X production topology. M41 supplies eligibility/role information to the existing profile policy; it does not create B-rep topology or persistent profile entities.
- **Persistence decision:** evolve the in-memory document model and add an explicitly unsupported draft-v5 DTO/codec for M41 state. Frozen v1-v4 readers and canonical v4 bytes remain unchanged for documents whose roles and requested activation are representable by v4; supported v4 encoding must reject rather than silently discard non-default M41 state. M107X alone may freeze the draft language.
- **Role decision:** use a closed `Profile`/`Construction` geometry-role enum keyed by persistent curve identity. `Profile` is the deterministic migration/default for every v1-v4 curve. Role does not imply activity and never removes a curve from lowering.
- **Activation decision:** keep user-requested state separate from immutable host configuration input and from derived effective activity. Existing `suppressed: bool` maps exactly to user suppression for v1-v4 compatibility. The M41 activation payload carries a monotone revision plus canonical digest and finite, bounded persistent-element overrides; an absent payload has canonical empty identity as required by ADR 0025.
- **Reason decision:** the closed effective reason vocabulary is `UserSuppressed`, `HostConfigurationInactive`, `UnavailableDependency`, and `UnavailableExternalReference`. Reports include the affected persistent `DocumentElementId` and, where applicable, the direct unavailable dependency identity. Deterministic closure order follows canonical persistent element order, never hash-map iteration.
- **Dependency decision:** derive dependency edges from the typed document graph; do not persist a second arbitrary graph. An element becomes unavailable only when one of its required typed operands/owners is inactive or an M43-reserved external dependency is explicitly unavailable. Inactivity propagates to dependents, not to independent operands. Every lowering/profile/branch/ownership consumer receives the same immutable closure snapshot.

## Out of scope

- M42 typed parameter identities/batches and bindings, except for the M41 activation-input boundary and immutable-stamp requirements.
- M43 external-reference snapshot implementation, except for reporting its required `unavailable external reference` inactivity cause.
- M44 desktop/workbench controls and presentation; the editor consumes typed M41 domain APIs rather than defining closure semantics.
- M104X production topology, B-rep conversion, and geometry-operation behavior.

## Risks and open questions

- Draft-v5 remains deliberately unsupported and may evolve through M106X; tests must distinguish its private codec from supported v1-v4 import/export.
- M42 activation-parameter bindings and M43 external binding records must extend the M41 payload and reserved external-unavailability seam without changing the four public reason categories.
- Complete input-stamp propagation reaches later M42/M43/M101X surfaces. M41 must establish activation revision/digest identity now, but must not fabricate parameter or external-snapshot revisions before their milestones.

## Completion report

1. **Files/API added:** `document.rs` exposes closed `GeometryRole`, immutable
   `HostConfigurationActivation`, canonical `ActivationDigest`, typed
   `InactivityReason` and `EffectiveActivity`; lowering and profiles consume the same
   closure; retained sessions expose activation-stamped `SketchAttemptInput`; typed
   edits/effects and exhaustive characterizations cover role, user suppression and host
   activation. `tests/m41.rs` contains the focused acceptance corpus.
2. **Mathematical behavior:** construction curves remain ordinary solver geometry but
   default profile analysis excludes them. Effective activity is the deterministic
   transitive closure of explicit requested state over typed persistent dependencies.
   Inactivity removes unavailable equations/geometry before evaluation without deleting
   or reconstructing branch, span, sweep, winding, contact, trim or ownership state.
3. **Commands run:** `cargo fmt --all -- --check`; focused and full sketch tests;
   `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`;
   `cargo test --locked --workspace --all-features`;
   `cargo check --locked -p geosolve-demo-web --target wasm32-unknown-unknown`; and
   `nix-shell ../../shell.nix --run 'trunk build --release'` from the web crate all
   passed. Cargo emitted only the pre-existing duplicate license metadata warnings.
4. **Acceptance passed:** constrained construction behavior, all four typed inactivity
   reasons, dependency-safe atomic activation, retained attempted/accepted separation,
   activation revision/digest publication checks, exact discrete-state reactivation,
   deterministic characterization and frozen supported persistence all passed.
5. **Known limitations/next blocker:** draft-v5 remains deliberately unsupported until
   M107X. M42 must add parameter revision/digest input stamps and bindings without
   changing M41's four reason categories; M43 later owns external snapshot records.
