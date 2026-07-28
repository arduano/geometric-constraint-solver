// SPDX-License-Identifier: GPL-3.0-or-later

//! Stable sketch-owned diagnostic data transfer objects.
//!
//! These types deliberately translate the numerical kernel's evolving report into
//! persistent sketch identities. Hosts should consume this module rather than
//! interpreting `geosolve-core` reports or runtime IDs.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_core::{
    AuditEvaluationStatus, BoundStatus, DiagnosticCompleteness, DiagnosticIncompleteReason,
    DiagnosticStatus, HardValidity, OneSidedMobility, ResidualCategory, SolveReport,
    SolveTermination, SolverConfig, StructuralClassification,
};

use crate::{
    ActivationDigest, DocumentElementId, DocumentExternalBindingId, DocumentParameterId,
    DocumentParameterKind, DocumentParameterTarget, DocumentRuntimeMap, DocumentSourceId,
    DocumentSourceOwner, EffectiveActivity, ExternalFeatureKindV1, ExternalSnapshotInputError,
    ExternalSnapshotSetDigest, InactivityReason, ParameterDigest, SketchAcceptedStateIdentity,
    SketchAttemptIdentity, SketchAttemptInput, SketchBound, SketchDesignIdentity, SketchDocument,
    SketchSolveResult, SketchSource,
};

/// Provenance of one immutable diagnostic snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchDiagnosticProvenance {
    Accepted {
        accepted: SketchAcceptedStateIdentity,
        originating_attempt: SketchAttemptIdentity,
        design: SketchDesignIdentity,
    },
    Attempt {
        attempt: SketchAttemptIdentity,
        design: SketchDesignIdentity,
        parent_accepted: Option<SketchAcceptedStateIdentity>,
    },
}

/// Deterministic identity of the solver policy used by one attempt.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct SketchSolverPolicyDigest([u8; 32]);

impl SketchSolverPolicyDigest {
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Stable implemented subset of the complete attempt input stamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SketchDiagnosticInputIdentity {
    pub design: SketchDesignIdentity,
    pub activation_revision: u64,
    pub activation_digest: ActivationDigest,
    pub parameter_revision: u64,
    pub parameter_digest: ParameterDigest,
    pub external_snapshot_revision: u64,
    pub external_snapshot_digest: ExternalSnapshotSetDigest,
    pub solver_policy_digest: SketchSolverPolicyDigest,
}

/// Sketch-owned hard-validity classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchHardValidity {
    Valid,
    Invalid,
    NotEvaluated,
    Unknown,
}

/// Sketch-owned nonlinear termination classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchSolveTermination {
    Converged,
    Stalled,
    IterationLimit,
    InvalidGeometry,
    NumericalFailure,
    Unknown,
}

/// Stable solve facts, distinct from rank and mobility.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchSolveDiagnostic {
    pub accepted: bool,
    pub hard_validity: SketchHardValidity,
    pub termination: SketchSolveTermination,
    pub hard_residuals_validated: bool,
    pub maximum_normalized_hard_residual: Option<f64>,
    pub normalized_hard_residual_l2: Option<f64>,
    pub iterations: usize,
}

/// Sketch-owned structural classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchStructuralClassification {
    Under,
    Well,
    Over,
    Mixed,
    Unknown,
}

/// Stable rank evidence. Structural and numerical facts are never collapsed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchRankDiagnostic {
    pub numerical_valid: bool,
    pub numerical_rank: Option<usize>,
    pub numerical_left_nullity: Option<usize>,
    pub numerical_right_nullity: Option<usize>,
    pub singular: Option<bool>,
    pub near_singular: Option<bool>,
    pub structural_rank: usize,
    pub structural_left_nullity: usize,
    pub structural_right_nullity: usize,
    pub structural_classification: SketchStructuralClassification,
}

/// Sketch-owned one-sided feasible-motion result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchOneSidedMobility {
    Exists,
    None,
    NotEvaluated,
    Unknown,
}

/// Equality, bounded lineality, and one-sided motion remain separate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SketchMobilityDiagnostic {
    pub equality_degrees_of_freedom: Option<usize>,
    pub bidirectional_bounded_degrees_of_freedom: Option<usize>,
    pub one_sided: SketchOneSidedMobility,
}

/// Stable status of one active coordinate bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchBoundStatus {
    Inactive,
    ActiveLower,
    ActiveUpper,
    Fixed,
    Unknown,
}

/// Persistent target and finite accepted-state evidence for one bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SketchBoundDiagnostic {
    pub target: DocumentElementId,
    pub status: SketchBoundStatus,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub value: f64,
}

/// Stable identity of one reduced component through complete persistent membership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchDiagnosticComponentIdentity {
    pub document: crate::DocumentId,
    pub elements: Vec<DocumentElementId>,
    pub sources: Vec<DocumentSourceId>,
}

/// Stable component-local rank and mobility evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchComponentDiagnostic {
    pub identity: SketchDiagnosticComponentIdentity,
    pub rank: SketchRankDiagnostic,
    pub mobility: SketchMobilityDiagnostic,
}

/// Stable source-row evaluation summary.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)]
pub struct SketchSourceDiagnostic {
    pub source: DocumentSourceId,
    pub owner: DocumentSourceOwner,
    pub label: String,
    pub inactivity: Option<InactivityReason>,
    pub active_row_count: usize,
    pub evaluated_row_count: usize,
    pub failed_row_count: usize,
    pub maximum_normalized_residual: Option<f64>,
    pub conflict_candidate: bool,
    pub fully_redundant: bool,
    pub contains_redundant_rows: bool,
    pub singular: bool,
}

/// Stable completeness status for bounded explanatory searches.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchDiagnosticSearchStatus {
    Complete,
    Truncated,
    Skipped,
    Unknown,
}

/// Stable reason why a bounded explanatory search is incomplete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchDiagnosticIncompleteReason {
    Disabled,
    HardConstraintsValid,
    HardInvalid,
    InvalidEvaluation,
    InvalidRank,
    ComponentTangentBudget,
    ComponentRowBudget,
    CandidateSourceBudget,
    TrialBudget,
    Unknown,
}

/// Stable configured budget for one bounded explanatory search.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SketchDiagnosticBudget {
    pub enabled: bool,
    pub maximum_component_tangent_dimension: usize,
    pub maximum_component_scalar_rows: usize,
    pub maximum_candidate_sources: usize,
    pub maximum_trials: usize,
}

/// Stable consumed-work evidence for one bounded explanatory search.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SketchDiagnosticWork {
    pub components: usize,
    pub tangent_dimensions: usize,
    pub scalar_rows: usize,
    pub candidate_sources: usize,
    pub trials: usize,
}

/// Bounded source-level explanatory candidates with explicit completeness.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchDiagnosticSearch {
    pub status: SketchDiagnosticSearchStatus,
    pub reason: Option<SketchDiagnosticIncompleteReason>,
    pub budget: SketchDiagnosticBudget,
    pub consumed: SketchDiagnosticWork,
    pub candidates: Vec<DocumentSourceId>,
}

/// Stable per-element activation and dependency evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchDependencyDiagnostic {
    pub element: DocumentElementId,
    pub inactivity: Option<InactivityReason>,
    pub dependencies: Vec<DocumentElementId>,
}

/// Stable parameter-input state for one declared persistent parameter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchParameterState {
    Applied,
    Inactive,
    OutputOnly,
    Missing,
    WrongKind,
    Unexpected,
    Invalid,
    Unknown,
}

/// Structured parameter failure retained by an attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchParameterInputIssue {
    Missing(DocumentParameterId),
    WrongKind {
        parameter: DocumentParameterId,
        expected: DocumentParameterKind,
        actual: DocumentParameterKind,
    },
    Unexpected(DocumentParameterId),
    Unknown(DocumentParameterId),
    InvalidValue(DocumentParameterId),
    InvalidDocument,
}

impl SketchParameterInputIssue {
    #[must_use]
    pub const fn parameter(self) -> Option<DocumentParameterId> {
        match self {
            Self::Missing(parameter)
            | Self::Unexpected(parameter)
            | Self::Unknown(parameter)
            | Self::InvalidValue(parameter)
            | Self::WrongKind { parameter, .. } => Some(parameter),
            Self::InvalidDocument => None,
        }
    }
}

/// Stable diagnostics for one persistent host-owned parameter declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchParameterDiagnostic {
    pub parameter: DocumentParameterId,
    pub kind: DocumentParameterKind,
    pub state: SketchParameterState,
    pub targets: Vec<DocumentParameterTarget>,
}

/// Stable external-reference availability classification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchExternalReferenceState {
    Available,
    Inactive,
    Missing,
    WrongKind,
    TopologyMismatch,
    Invalid,
    Unknown,
}

/// Stable diagnostics for one persistent local external binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SketchExternalReferenceDiagnostic {
    pub binding: DocumentExternalBindingId,
    pub expected_kind: ExternalFeatureKindV1,
    pub state: SketchExternalReferenceState,
}

/// Machine-readable, non-mutating repair hint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SketchRepairSuggestion {
    ReviewOrSuppressSource(DocumentSourceId),
    SupplyParameter(DocumentParameterId),
    CorrectParameterKind(DocumentParameterId),
    RemoveUnexpectedParameter(DocumentParameterId),
    CorrectParameterValue(DocumentParameterId),
    SupplyExternalSnapshot(DocumentExternalBindingId),
    CorrectExternalSnapshotKind(DocumentExternalBindingId),
    RebindExternalTopology(DocumentExternalBindingId),
    ReviewGlobalInput,
}

/// Complete stable sketch-owned diagnostics for one accepted state or attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchDiagnosticSnapshot {
    pub provenance: SketchDiagnosticProvenance,
    pub input: SketchDiagnosticInputIdentity,
    pub solve: Option<SketchSolveDiagnostic>,
    pub rank: Option<SketchRankDiagnostic>,
    pub mobility: Option<SketchMobilityDiagnostic>,
    pub bounds: Vec<SketchBoundDiagnostic>,
    pub components: Vec<SketchComponentDiagnostic>,
    pub sources: Vec<SketchSourceDiagnostic>,
    pub conflicts: SketchDiagnosticSearch,
    pub redundancy: SketchDiagnosticSearch,
    pub activation_revision: u64,
    pub activation_digest: ActivationDigest,
    pub dependencies: Vec<SketchDependencyDiagnostic>,
    pub parameters: Vec<SketchParameterDiagnostic>,
    pub external_references: Vec<SketchExternalReferenceDiagnostic>,
    pub repair_suggestions: Vec<SketchRepairSuggestion>,
}

#[derive(Clone, Copy)]
pub(crate) struct SketchDiagnosticBuildInput<'a> {
    pub provenance: SketchDiagnosticProvenance,
    pub input: SketchAttemptInput,
    pub document: &'a SketchDocument,
    pub solve: Option<&'a SketchSolveResult>,
    pub mappings: Option<&'a DocumentRuntimeMap>,
    pub activity: &'a EffectiveActivity,
    pub parameter_issue: Option<SketchParameterInputIssue>,
    pub external_issue: Option<&'a ExternalSnapshotInputError>,
    pub variable_elements: &'a BTreeMap<geosolve_core::VariableId, DocumentElementId>,
}

#[allow(clippy::too_many_lines)]
pub(crate) fn build_diagnostic_snapshot(
    input: &SketchDiagnosticBuildInput<'_>,
) -> SketchDiagnosticSnapshot {
    let report = input.solve.map(SketchSolveResult::unstable_core_report);
    let persistent_core_source = |core_source| {
        let solve = input.solve?;
        let mappings = input.mappings?;
        let runtime = solve.source_mappings.iter().find_map(|mapping| {
            (mapping.core_source_id == Some(core_source)).then_some(mapping.source)
        })?;
        persistent_runtime_source(mappings, runtime)
    };
    let conflicts = diagnostic_search(
        report.map(|report| report.conflict_diagnostics),
        report.map_or(&[][..], |report| report.conflicting_sources.as_slice()),
        &persistent_core_source,
    );
    let redundancy = diagnostic_search(
        report.map(|report| report.redundancy_diagnostics),
        report.map_or(&[][..], |report| report.redundant_sources.as_slice()),
        &persistent_core_source,
    );
    let rank = report.map(rank_diagnostic);
    let mobility = report.map(mobility_diagnostic);
    let sources = source_diagnostics(
        input.document,
        input.solve,
        input.mappings,
        input.activity,
        &conflicts.candidates,
        &redundancy.candidates,
    );
    let components = report.map_or_else(Vec::new, |report| {
        component_diagnostics(
            input.document,
            input.solve.expect("report belongs to solve"),
            input.mappings,
            report,
            input.variable_elements,
        )
    });
    let bounds = input.solve.map_or_else(Vec::new, |solve| {
        bound_diagnostics(input.document, solve, input.mappings)
    });
    let dependencies = input
        .activity
        .elements()
        .iter()
        .map(|entry| SketchDependencyDiagnostic {
            element: entry.element,
            inactivity: entry.reason,
            dependencies: input.document.dependency_closure(entry.element),
        })
        .collect();
    let parameters = parameter_diagnostics(
        input.document,
        input.activity,
        input.parameter_issue,
        input.solve.is_some_and(SketchSolveResult::accepted),
    );
    let external_references = external_reference_diagnostics(
        input.document,
        input.activity,
        input.external_issue,
        input.solve.is_some(),
    );
    let mut repair_suggestions = conflicts
        .candidates
        .iter()
        .copied()
        .map(SketchRepairSuggestion::ReviewOrSuppressSource)
        .collect::<Vec<_>>();
    if let Some(issue) = input.parameter_issue {
        repair_suggestions.push(parameter_repair(issue));
    }
    if let Some(issue) = input.external_issue {
        repair_suggestions.push(external_repair(issue));
    }
    repair_suggestions.sort_by_key(repair_sort_key);
    repair_suggestions.dedup();
    let diagnostic_input = SketchDiagnosticInputIdentity {
        design: input.input.design_identity(),
        activation_revision: input.input.effective_activation_revision(),
        activation_digest: input.input.activation_digest(),
        parameter_revision: input.input.parameter_revision(),
        parameter_digest: input.input.parameter_digest(),
        external_snapshot_revision: input.input.external_snapshot_set_revision(),
        external_snapshot_digest: input.input.external_snapshot_set_digest(),
        solver_policy_digest: solver_policy_digest(input.input.solver_config()),
    };
    SketchDiagnosticSnapshot {
        provenance: input.provenance,
        input: diagnostic_input,
        solve: report
            .map(|report| solve_diagnostic(input.solve.expect("report has solve"), report)),
        rank,
        mobility,
        bounds,
        components,
        sources,
        conflicts,
        redundancy,
        activation_revision: input.activity.activation_revision(),
        activation_digest: input.activity.activation_digest(),
        dependencies,
        parameters,
        external_references,
        repair_suggestions,
    }
}

pub(crate) fn diagnostic_variable_elements(
    solve: &SketchSolveResult,
    mappings: &DocumentRuntimeMap,
) -> BTreeMap<geosolve_core::VariableId, DocumentElementId> {
    solve
        .diagnostic_variable_owners
        .iter()
        .filter_map(|(variable, owner)| {
            let element = match *owner {
                crate::compiler::DiagnosticVariableOwner::Point(point) => mappings
                    .point_mappings()
                    .iter()
                    .find_map(|mapping| {
                        (mapping.runtime == point)
                            .then_some(DocumentElementId::Point(mapping.persistent))
                    }),
                crate::compiler::DiagnosticVariableOwner::Circle(circle) => mappings
                    .curve_mappings()
                    .iter()
                    .find_map(|mapping| {
                        matches!(mapping.runtime, crate::RuntimeCurve::Circle(id) if id == circle)
                            .then_some(DocumentElementId::Curve(mapping.persistent))
                    }),
                crate::compiler::DiagnosticVariableOwner::Arc(arc) => mappings
                    .curve_mappings()
                    .iter()
                    .find_map(|mapping| {
                        matches!(mapping.runtime, crate::RuntimeCurve::CircularArc(id) if id == arc)
                            .then_some(DocumentElementId::Curve(mapping.persistent))
                    }),
                crate::compiler::DiagnosticVariableOwner::Conic(conic) => mappings
                    .curve_mappings()
                    .iter()
                    .find_map(|mapping| {
                        matches!(mapping.runtime, crate::RuntimeCurve::Conic(id) if id == conic)
                            .then_some(DocumentElementId::Curve(mapping.persistent))
                    }),
                crate::compiler::DiagnosticVariableOwner::Nurbs(nurbs) => mappings
                    .curve_mappings()
                    .iter()
                    .find_map(|mapping| {
                        matches!(mapping.runtime, crate::RuntimeCurve::Nurbs { nurbs: id, .. } if id == nurbs)
                            .then_some(DocumentElementId::Curve(mapping.persistent))
                    }),
                crate::compiler::DiagnosticVariableOwner::Contact {
                    constraint_id,
                    role,
                } => mappings.contact_mappings().iter().find_map(|mapping| {
                    (mapping.constraint == constraint_id
                        && contact_role_matches(mapping.role, role))
                    .then_some(DocumentElementId::Contact(mapping.persistent))
                }),
            }?;
            Some((*variable, element))
        })
        .collect()
}

fn solve_diagnostic(solve: &SketchSolveResult, report: &SolveReport) -> SketchSolveDiagnostic {
    SketchSolveDiagnostic {
        accepted: solve.accepted(),
        hard_validity: hard_validity(report.hard_validity),
        termination: solve_termination(report.termination),
        hard_residuals_validated: report.hard_residuals_validated,
        maximum_normalized_hard_residual: solve
            .acceptance_hard_residual_max
            .filter(|value| value.is_finite())
            .or_else(|| {
                (report.hard_residuals_validated && report.hard_residual_max.is_finite())
                    .then_some(report.hard_residual_max)
            }),
        normalized_hard_residual_l2: report
            .hard_residual_l2
            .is_finite()
            .then_some(report.hard_residual_l2),
        iterations: report.iterations,
    }
}

fn rank_diagnostic(report: &SolveReport) -> SketchRankDiagnostic {
    SketchRankDiagnostic {
        numerical_valid: report.rank_is_valid,
        numerical_rank: report.rank_is_valid.then_some(report.rank),
        numerical_left_nullity: report.rank_is_valid.then_some(report.left_nullity),
        numerical_right_nullity: report.rank_is_valid.then_some(report.right_nullity),
        singular: report.rank_is_valid.then_some(report.is_singular),
        near_singular: report.rank_is_valid.then_some(report.near_singular),
        structural_rank: report.structural.structural_rank,
        structural_left_nullity: report.structural.structural_left_nullity,
        structural_right_nullity: report.structural.structural_right_nullity,
        structural_classification: structural_classification(
            report.structural.structural_classification,
        ),
    }
}

fn component_rank_diagnostic(
    structural: &geosolve_core::ComponentStructuralSummary,
    numerical: Option<&geosolve_core::ComponentSolveReport>,
) -> SketchRankDiagnostic {
    let valid = numerical.is_some_and(|report| report.rank_is_valid);
    SketchRankDiagnostic {
        numerical_valid: valid,
        numerical_rank: numerical
            .filter(|report| report.rank_is_valid)
            .map(|report| report.rank),
        numerical_left_nullity: numerical
            .filter(|report| report.rank_is_valid)
            .map(|report| report.left_nullity),
        numerical_right_nullity: numerical
            .filter(|report| report.rank_is_valid)
            .map(|report| report.right_nullity),
        singular: numerical
            .filter(|report| report.rank_is_valid)
            .map(|report| report.is_singular),
        near_singular: numerical
            .filter(|report| report.rank_is_valid)
            .map(|report| report.near_singular),
        structural_rank: structural.structural_rank,
        structural_left_nullity: structural.structural_left_nullity,
        structural_right_nullity: structural.structural_right_nullity,
        structural_classification: structural_classification(structural.structural_classification),
    }
}

fn mobility_diagnostic(report: &SolveReport) -> SketchMobilityDiagnostic {
    SketchMobilityDiagnostic {
        equality_degrees_of_freedom: report.rank_is_valid.then_some(report.right_nullity),
        bidirectional_bounded_degrees_of_freedom: report
            .rank_is_valid
            .then_some(report.bidirectional_degrees_of_freedom),
        one_sided: one_sided_mobility(report.one_sided_mobility),
    }
}

fn component_diagnostics(
    document: &SketchDocument,
    solve: &SketchSolveResult,
    mappings: Option<&DocumentRuntimeMap>,
    report: &SolveReport,
    variable_elements: &BTreeMap<geosolve_core::VariableId, DocumentElementId>,
) -> Vec<SketchComponentDiagnostic> {
    report
        .structural
        .component_summaries
        .iter()
        .map(|structural| {
            let numerical = report
                .component_solves
                .iter()
                .find(|candidate| candidate.component_index == structural.component_index);
            let mut sources = BTreeSet::new();
            for residual in &structural.residual_ids {
                for source_mapping in &solve.source_mappings {
                    if source_mapping.residual_ids.contains(residual)
                        && let Some(mappings) = mappings
                        && let Some(source) =
                            persistent_runtime_source(mappings, source_mapping.source)
                    {
                        sources.insert(source);
                    }
                }
            }
            let mut elements = structural
                .variable_ids
                .iter()
                .filter_map(|variable| variable_elements.get(variable).copied())
                .collect::<BTreeSet<_>>();
            for source in &sources {
                elements.insert(DocumentElementId::Source(*source));
                if let Some(source_ref) = document.source(*source) {
                    let owner = match source_ref.owner {
                        DocumentSourceOwner::Constraint(id) => DocumentElementId::Constraint(id),
                        DocumentSourceOwner::Dimension(id) => DocumentElementId::Dimension(id),
                    };
                    elements.insert(owner);
                    elements.extend(document.dependency_closure(owner));
                }
            }
            let mobility = numerical.map_or(
                SketchMobilityDiagnostic {
                    equality_degrees_of_freedom: None,
                    bidirectional_bounded_degrees_of_freedom: None,
                    one_sided: SketchOneSidedMobility::NotEvaluated,
                },
                |numerical| SketchMobilityDiagnostic {
                    equality_degrees_of_freedom: numerical
                        .rank_is_valid
                        .then_some(numerical.right_nullity),
                    bidirectional_bounded_degrees_of_freedom: numerical
                        .rank_is_valid
                        .then_some(numerical.bidirectional_degrees_of_freedom),
                    one_sided: one_sided_mobility(numerical.one_sided_mobility),
                },
            );
            SketchComponentDiagnostic {
                identity: SketchDiagnosticComponentIdentity {
                    document: document.id(),
                    elements: elements.into_iter().collect(),
                    sources: sources.into_iter().collect(),
                },
                rank: component_rank_diagnostic(structural, numerical),
                mobility,
            }
        })
        .collect()
}

fn source_diagnostics(
    document: &SketchDocument,
    solve: Option<&SketchSolveResult>,
    mappings: Option<&DocumentRuntimeMap>,
    activity: &EffectiveActivity,
    conflicts: &[DocumentSourceId],
    redundant: &[DocumentSourceId],
) -> Vec<SketchSourceDiagnostic> {
    let report = solve.map(SketchSolveResult::unstable_core_report);
    let containing = report.map_or(&[][..], |report| {
        report.sources_containing_redundant_rows.as_slice()
    });
    document
        .sources()
        .map(|source| {
            let core_source = mappings
                .and_then(|mappings| mappings.runtime_source(source.id))
                .and_then(|runtime| {
                    solve?.source_mappings.iter().find_map(|mapping| {
                        (mapping.source == runtime_sketch_source(runtime))
                            .then_some(mapping.core_source_id)
                            .flatten()
                    })
                });
            let audit = core_source.and_then(|core_source| {
                report?
                    .audit
                    .sources
                    .iter()
                    .find(|audit| audit.source_id == core_source)
            });
            let mut maximum = 0.0_f64;
            let mut has_finite = false;
            let mut evaluated = 0;
            let mut failed = 0;
            if let Some(audit) = audit {
                for row in &audit.rows {
                    match row.evaluation_status {
                        AuditEvaluationStatus::Evaluated => {
                            evaluated += 1;
                            if row.normalized_residual.is_finite() {
                                maximum = maximum.max(row.normalized_residual.abs());
                                has_finite = true;
                            }
                        }
                        AuditEvaluationStatus::Failed => failed += 1,
                        _ => {}
                    }
                }
            }
            let singular = core_source.is_some_and(|core_source| {
                report.is_some_and(|report| {
                    report
                        .singular_rows
                        .iter()
                        .any(|row| row.source_id == core_source)
                })
            });
            SketchSourceDiagnostic {
                source: source.id,
                owner: source.owner,
                label: source.label.to_owned(),
                inactivity: activity.reason(source.id),
                active_row_count: audit.map_or(0, |audit| {
                    audit
                        .rows
                        .iter()
                        .filter(|row| row.category == ResidualCategory::Hard)
                        .count()
                }),
                evaluated_row_count: evaluated,
                failed_row_count: failed,
                maximum_normalized_residual: has_finite.then_some(maximum),
                conflict_candidate: conflicts.contains(&source.id),
                fully_redundant: redundant.contains(&source.id),
                contains_redundant_rows: mappings.is_some_and(|mappings| {
                    containing.iter().any(|core_source| {
                        solve
                            .and_then(|solve| {
                                solve.source_mappings.iter().find_map(|mapping| {
                                    (mapping.core_source_id == Some(*core_source))
                                        .then_some(mapping.source)
                                })
                            })
                            .and_then(|runtime| persistent_runtime_source(mappings, runtime))
                            == Some(source.id)
                    })
                }),
                singular,
            }
        })
        .collect()
}

fn bound_diagnostics(
    document: &SketchDocument,
    solve: &SketchSolveResult,
    mappings: Option<&DocumentRuntimeMap>,
) -> Vec<SketchBoundDiagnostic> {
    let Some(mappings) = mappings else {
        return Vec::new();
    };
    solve
        .bound_mappings
        .iter()
        .filter_map(|mapping| {
            let report = solve
                .unstable_core_report()
                .bounds
                .iter()
                .find(|report| report.bound_id == mapping.bound_id)?;
            let target = persistent_bound_target(document, mappings, mapping.bound)?;
            Some(SketchBoundDiagnostic {
                target,
                status: bound_status(report.status),
                lower: report.lower,
                upper: report.upper,
                value: report.value,
            })
        })
        .collect()
}

fn parameter_diagnostics(
    document: &SketchDocument,
    activity: &EffectiveActivity,
    issue: Option<SketchParameterInputIssue>,
    accepted: bool,
) -> Vec<SketchParameterDiagnostic> {
    document
        .parameters()
        .iter()
        .map(|parameter| {
            let targets = document
                .parameter_bindings()
                .iter()
                .filter_map(|binding| (binding.parameter == parameter.id).then_some(binding.target))
                .collect::<Vec<_>>();
            let issue_state = issue.and_then(|issue| {
                (issue.parameter() == Some(parameter.id)).then_some(match issue {
                    SketchParameterInputIssue::Missing(_) => SketchParameterState::Missing,
                    SketchParameterInputIssue::WrongKind { .. } => SketchParameterState::WrongKind,
                    SketchParameterInputIssue::Unexpected(_) => SketchParameterState::Unexpected,
                    SketchParameterInputIssue::Unknown(_)
                    | SketchParameterInputIssue::InvalidValue(_) => SketchParameterState::Invalid,
                    SketchParameterInputIssue::InvalidDocument => SketchParameterState::Unknown,
                })
            });
            let active_target = targets.iter().any(|target| match target {
                DocumentParameterTarget::DrivingDimension(id) => activity.is_active(*id),
                DocumentParameterTarget::DimensionlessFixedScalar(property) => {
                    activity.is_active(property.scalar)
                }
                DocumentParameterTarget::Activation(_) => true,
            });
            let output_only = !active_target
                && document
                    .parameter_outputs()
                    .iter()
                    .any(|output| output.parameter == parameter.id);
            SketchParameterDiagnostic {
                parameter: parameter.id,
                kind: parameter.kind,
                state: issue_state.unwrap_or({
                    if output_only {
                        SketchParameterState::OutputOnly
                    } else if active_target && accepted {
                        SketchParameterState::Applied
                    } else if active_target {
                        SketchParameterState::Unknown
                    } else {
                        SketchParameterState::Inactive
                    }
                }),
                targets,
            }
        })
        .collect()
}

fn external_reference_diagnostics(
    document: &SketchDocument,
    activity: &EffectiveActivity,
    issue: Option<&ExternalSnapshotInputError>,
    reached_solve: bool,
) -> Vec<SketchExternalReferenceDiagnostic> {
    document
        .external_bindings()
        .iter()
        .map(|binding| {
            let state = external_issue_state(issue, binding.id).unwrap_or_else(|| {
                if activity.reason(binding.id).is_some() {
                    SketchExternalReferenceState::Inactive
                } else if reached_solve {
                    SketchExternalReferenceState::Available
                } else {
                    SketchExternalReferenceState::Unknown
                }
            });
            SketchExternalReferenceDiagnostic {
                binding: binding.id,
                expected_kind: binding.expected_kind,
                state,
            }
        })
        .collect()
}

fn diagnostic_search(
    completeness: Option<DiagnosticCompleteness>,
    candidates: &[geosolve_core::SourceConstraintId],
    persistent_source: &impl Fn(geosolve_core::SourceConstraintId) -> Option<DocumentSourceId>,
) -> SketchDiagnosticSearch {
    let Some(completeness) = completeness else {
        return SketchDiagnosticSearch {
            status: SketchDiagnosticSearchStatus::Skipped,
            reason: Some(SketchDiagnosticIncompleteReason::InvalidEvaluation),
            budget: SketchDiagnosticBudget {
                enabled: false,
                maximum_component_tangent_dimension: 0,
                maximum_component_scalar_rows: 0,
                maximum_candidate_sources: 0,
                maximum_trials: 0,
            },
            consumed: SketchDiagnosticWork::default(),
            candidates: Vec::new(),
        };
    };
    let mut persistent = candidates
        .iter()
        .filter_map(|source| persistent_source(*source))
        .collect::<Vec<_>>();
    persistent.sort_unstable();
    persistent.dedup();
    SketchDiagnosticSearch {
        status: diagnostic_status(completeness.status),
        reason: completeness.reason.map(diagnostic_incomplete_reason),
        budget: SketchDiagnosticBudget {
            enabled: completeness.budget.enabled,
            maximum_component_tangent_dimension: completeness
                .budget
                .max_component_tangent_dimension,
            maximum_component_scalar_rows: completeness.budget.max_component_scalar_rows,
            maximum_candidate_sources: completeness.budget.max_candidate_sources,
            maximum_trials: completeness.budget.max_trials,
        },
        consumed: SketchDiagnosticWork {
            components: completeness.consumed.components,
            tangent_dimensions: completeness.consumed.tangent_dimensions,
            scalar_rows: completeness.consumed.scalar_rows,
            candidate_sources: completeness.consumed.candidate_sources,
            trials: completeness.consumed.trials,
        },
        candidates: persistent,
    }
}

fn persistent_runtime_source(
    mappings: &DocumentRuntimeMap,
    source: SketchSource,
) -> Option<DocumentSourceId> {
    let runtime = match source {
        SketchSource::Constraint(id) => crate::RuntimeSource::Constraint(id),
        SketchSource::Dimension(id) => crate::RuntimeSource::Dimension(id),
        SketchSource::DragTarget(_) | SketchSource::PreviousState(_) => return None,
    };
    mappings
        .source_mappings()
        .iter()
        .find_map(|mapping| (mapping.runtime == Some(runtime)).then_some(mapping.source_id))
}

fn runtime_sketch_source(source: crate::RuntimeSource) -> SketchSource {
    match source {
        crate::RuntimeSource::Constraint(id) => SketchSource::Constraint(id),
        crate::RuntimeSource::Dimension(id) => SketchSource::Dimension(id),
    }
}

fn persistent_bound_target(
    document: &SketchDocument,
    mappings: &DocumentRuntimeMap,
    bound: SketchBound,
) -> Option<DocumentElementId> {
    match bound {
        SketchBound::CircleRadius(runtime) => mappings.curve_mappings().iter().find_map(|mapping| {
            matches!(mapping.runtime, crate::RuntimeCurve::Circle(id) if id == runtime)
                .then_some(DocumentElementId::Curve(mapping.persistent))
        }),
        SketchBound::ArcRadius(runtime) => mappings.curve_mappings().iter().find_map(|mapping| {
            matches!(mapping.runtime, crate::RuntimeCurve::CircularArc(id) if id == runtime)
                .then_some(DocumentElementId::Curve(mapping.persistent))
        }),
        SketchBound::ConicScalar { conic_id, .. } => {
            mappings.curve_mappings().iter().find_map(|mapping| {
                matches!(mapping.runtime, crate::RuntimeCurve::Conic(id) if id == conic_id)
                    .then_some(DocumentElementId::Curve(mapping.persistent))
            })
        }
        SketchBound::NurbsWeight { nurbs_id, .. } => {
            mappings.curve_mappings().iter().find_map(|mapping| {
                matches!(mapping.runtime, crate::RuntimeCurve::Nurbs { nurbs, .. } if nurbs == nurbs_id)
                    .then_some(DocumentElementId::Curve(mapping.persistent))
            })
        }
        SketchBound::Contact {
            constraint_id,
            role,
        } => mappings.contact_mappings().iter().find_map(|mapping| {
            (mapping.constraint == constraint_id && contact_role_matches(mapping.role, role))
                .then_some(DocumentElementId::Contact(mapping.persistent))
        }),
    }
    .filter(|element| document.contains_element(*element))
}

fn contact_role_matches(
    persistent: crate::DocumentContactRole,
    runtime: crate::LatentVariableRole,
) -> bool {
    use crate::DocumentContactRole as P;
    use crate::LatentVariableRole as R;
    matches!(
        (persistent, runtime),
        (P::LineParameter, R::LineParameter)
            | (P::CircleAngle, R::CircleAngle)
            | (P::ArcSpanParameter, R::ArcSpanParameter)
            | (P::BezierParameter, R::BezierParameter)
            | (
                P::ConicParameter | P::BSplineParameter | P::NurbsParameter | P::CurveParameter,
                R::CurveParameter
            )
            | (P::FirstCurveParameter, R::FirstCurveParameter)
            | (P::SecondCurveParameter, R::SecondCurveParameter)
    )
}

fn external_issue_state(
    issue: Option<&ExternalSnapshotInputError>,
    binding: DocumentExternalBindingId,
) -> Option<SketchExternalReferenceState> {
    match issue? {
        ExternalSnapshotInputError::MissingBinding { binding: target } if *target == binding => {
            Some(SketchExternalReferenceState::Missing)
        }
        ExternalSnapshotInputError::WrongKind {
            binding: target, ..
        } if *target == binding => Some(SketchExternalReferenceState::WrongKind),
        ExternalSnapshotInputError::TopologyMismatch { binding: target } if *target == binding => {
            Some(SketchExternalReferenceState::TopologyMismatch)
        }
        ExternalSnapshotInputError::UnknownBinding { binding: target }
        | ExternalSnapshotInputError::InvalidSourceRevision { binding: target }
        | ExternalSnapshotInputError::InvalidFeature {
            binding: target, ..
        }
        | ExternalSnapshotInputError::DuplicateBinding { binding: target }
            if *target == binding =>
        {
            Some(SketchExternalReferenceState::Invalid)
        }
        _ => None,
    }
}

fn parameter_repair(issue: SketchParameterInputIssue) -> SketchRepairSuggestion {
    match issue {
        SketchParameterInputIssue::Missing(parameter) => {
            SketchRepairSuggestion::SupplyParameter(parameter)
        }
        SketchParameterInputIssue::WrongKind { parameter, .. } => {
            SketchRepairSuggestion::CorrectParameterKind(parameter)
        }
        SketchParameterInputIssue::Unexpected(parameter) => {
            SketchRepairSuggestion::RemoveUnexpectedParameter(parameter)
        }
        SketchParameterInputIssue::Unknown(parameter)
        | SketchParameterInputIssue::InvalidValue(parameter) => {
            SketchRepairSuggestion::CorrectParameterValue(parameter)
        }
        SketchParameterInputIssue::InvalidDocument => SketchRepairSuggestion::ReviewGlobalInput,
    }
}

fn external_repair(issue: &ExternalSnapshotInputError) -> SketchRepairSuggestion {
    match issue {
        ExternalSnapshotInputError::MissingBinding { binding } => {
            SketchRepairSuggestion::SupplyExternalSnapshot(*binding)
        }
        ExternalSnapshotInputError::WrongKind { binding, .. } => {
            SketchRepairSuggestion::CorrectExternalSnapshotKind(*binding)
        }
        ExternalSnapshotInputError::TopologyMismatch { binding } => {
            SketchRepairSuggestion::RebindExternalTopology(*binding)
        }
        _ => SketchRepairSuggestion::ReviewGlobalInput,
    }
}

fn repair_sort_key(suggestion: &SketchRepairSuggestion) -> (u8, u128) {
    match *suggestion {
        SketchRepairSuggestion::ReviewOrSuppressSource(id) => (0, id.0.as_u128()),
        SketchRepairSuggestion::SupplyParameter(id) => (1, id.0.as_u128()),
        SketchRepairSuggestion::CorrectParameterKind(id) => (2, id.0.as_u128()),
        SketchRepairSuggestion::RemoveUnexpectedParameter(id) => (3, id.0.as_u128()),
        SketchRepairSuggestion::CorrectParameterValue(id) => (4, id.0.as_u128()),
        SketchRepairSuggestion::SupplyExternalSnapshot(id) => (5, id.0.as_u128()),
        SketchRepairSuggestion::CorrectExternalSnapshotKind(id) => (6, id.0.as_u128()),
        SketchRepairSuggestion::RebindExternalTopology(id) => (7, id.0.as_u128()),
        SketchRepairSuggestion::ReviewGlobalInput => (8, 0),
    }
}

fn hard_validity(value: HardValidity) -> SketchHardValidity {
    match value {
        HardValidity::Valid => SketchHardValidity::Valid,
        HardValidity::Invalid => SketchHardValidity::Invalid,
        HardValidity::NotEvaluated => SketchHardValidity::NotEvaluated,
        _ => SketchHardValidity::Unknown,
    }
}

fn solve_termination(value: SolveTermination) -> SketchSolveTermination {
    match value {
        SolveTermination::Converged => SketchSolveTermination::Converged,
        SolveTermination::Stalled => SketchSolveTermination::Stalled,
        SolveTermination::IterationLimit => SketchSolveTermination::IterationLimit,
        SolveTermination::InvalidGeometry => SketchSolveTermination::InvalidGeometry,
        SolveTermination::NumericalFailure => SketchSolveTermination::NumericalFailure,
        _ => SketchSolveTermination::Unknown,
    }
}

fn structural_classification(value: StructuralClassification) -> SketchStructuralClassification {
    match value {
        StructuralClassification::Under => SketchStructuralClassification::Under,
        StructuralClassification::Well => SketchStructuralClassification::Well,
        StructuralClassification::Over => SketchStructuralClassification::Over,
        StructuralClassification::Mixed => SketchStructuralClassification::Mixed,
        _ => SketchStructuralClassification::Unknown,
    }
}

fn one_sided_mobility(value: OneSidedMobility) -> SketchOneSidedMobility {
    match value {
        OneSidedMobility::Exists => SketchOneSidedMobility::Exists,
        OneSidedMobility::None => SketchOneSidedMobility::None,
        OneSidedMobility::NotEvaluated => SketchOneSidedMobility::NotEvaluated,
        _ => SketchOneSidedMobility::Unknown,
    }
}

fn bound_status(value: BoundStatus) -> SketchBoundStatus {
    match value {
        BoundStatus::Inactive => SketchBoundStatus::Inactive,
        BoundStatus::ActiveLower => SketchBoundStatus::ActiveLower,
        BoundStatus::ActiveUpper => SketchBoundStatus::ActiveUpper,
        BoundStatus::Fixed => SketchBoundStatus::Fixed,
        _ => SketchBoundStatus::Unknown,
    }
}

fn diagnostic_status(value: DiagnosticStatus) -> SketchDiagnosticSearchStatus {
    match value {
        DiagnosticStatus::Complete => SketchDiagnosticSearchStatus::Complete,
        DiagnosticStatus::Truncated => SketchDiagnosticSearchStatus::Truncated,
        DiagnosticStatus::Skipped => SketchDiagnosticSearchStatus::Skipped,
        _ => SketchDiagnosticSearchStatus::Unknown,
    }
}

fn diagnostic_incomplete_reason(
    value: DiagnosticIncompleteReason,
) -> SketchDiagnosticIncompleteReason {
    match value {
        DiagnosticIncompleteReason::Disabled => SketchDiagnosticIncompleteReason::Disabled,
        DiagnosticIncompleteReason::HardConstraintsValid => {
            SketchDiagnosticIncompleteReason::HardConstraintsValid
        }
        DiagnosticIncompleteReason::HardInvalid => SketchDiagnosticIncompleteReason::HardInvalid,
        DiagnosticIncompleteReason::InvalidEvaluation => {
            SketchDiagnosticIncompleteReason::InvalidEvaluation
        }
        DiagnosticIncompleteReason::InvalidRank => SketchDiagnosticIncompleteReason::InvalidRank,
        DiagnosticIncompleteReason::ComponentTangentBudget => {
            SketchDiagnosticIncompleteReason::ComponentTangentBudget
        }
        DiagnosticIncompleteReason::ComponentRowBudget => {
            SketchDiagnosticIncompleteReason::ComponentRowBudget
        }
        DiagnosticIncompleteReason::CandidateSourceBudget => {
            SketchDiagnosticIncompleteReason::CandidateSourceBudget
        }
        DiagnosticIncompleteReason::TrialBudget => SketchDiagnosticIncompleteReason::TrialBudget,
        _ => SketchDiagnosticIncompleteReason::Unknown,
    }
}

fn solver_policy_digest(config: SolverConfig) -> SketchSolverPolicyDigest {
    let mut bytes = Vec::new();
    for value in [
        config.normalized_residual_tolerance,
        config.normalized_step_tolerance,
        config.rank_relative_tolerance,
        config.initial_damping,
        config.minimum_damping,
        config.maximum_damping,
        config.damping_increase_factor,
        config.damping_decrease_factor,
        config.step_acceptance_ratio,
        config.max_block_normalized_step,
    ] {
        bytes.extend_from_slice(&value.to_bits().to_be_bytes());
    }
    bytes.extend_from_slice(&config.max_iterations.to_be_bytes());
    bytes.push(match config.linear_solve_backend {
        geosolve_core::LinearSolveBackendPolicy::Auto => 0,
        geosolve_core::LinearSolveBackendPolicy::DenseOnly => 1,
        geosolve_core::LinearSolveBackendPolicy::SparsePreferred => 2,
        _ => u8::MAX,
    });
    for budget in [
        config.redundancy_diagnostic_budget,
        config.conflict_diagnostic_budget,
    ] {
        bytes.push(u8::from(budget.enabled));
        bytes.extend_from_slice(&budget.max_component_tangent_dimension.to_be_bytes());
        bytes.extend_from_slice(&budget.max_component_scalar_rows.to_be_bytes());
        bytes.extend_from_slice(&budget.max_candidate_sources.to_be_bytes());
        bytes.extend_from_slice(&budget.max_trials.to_be_bytes());
    }
    let mut lanes = [
        0xcbf2_9ce4_8422_2325_u64,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0x517c_c1b7_2722_0a95,
    ];
    for byte in bytes {
        for (index, lane) in lanes.iter_mut().enumerate() {
            *lane ^= u64::from(byte).wrapping_add(index as u64);
            *lane = lane.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    let mut digest = [0_u8; 32];
    for (index, lane) in lanes.into_iter().enumerate() {
        digest[index * 8..(index + 1) * 8].copy_from_slice(&lane.to_be_bytes());
    }
    SketchSolverPolicyDigest(digest)
}
