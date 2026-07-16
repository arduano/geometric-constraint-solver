use thiserror::Error;

use crate::analysis::EliminationPlan;
use crate::{
    BoundId, BoundStatus, CoordinateBound, CoreError, HardValidity, Problem, ResidualBlock,
    ResidualId, ResidualRowAudit, SecondaryStatus, SolveReport, SolverConfig, SourceConstraint,
    SourceConstraintId, VariableId, VariableValue,
};

/// Independent revision counters for one accepted persistent session state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SessionRevisions {
    pub topology: u64,
    pub source: u64,
    pub state: u64,
    pub bound: u64,
}

/// Last accepted dependency revisions for one deterministic reduced component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ComponentDependencyStamp {
    pub component_index: usize,
    pub topology_revision: u64,
    pub source_revision: u64,
    pub state_revision: u64,
    pub bound_revision: u64,
}

/// A typed, revision-checked, non-structural session edit.
#[derive(Debug)]
pub struct SessionPatch {
    expected_revisions: SessionRevisions,
    variable_values: Vec<(VariableId, VariableValue)>,
    source_replacements: Vec<(SourceConstraintId, SourceConstraint)>,
    residual_replacements: Vec<(ResidualId, ResidualBlock)>,
    bound_replacements: Vec<(BoundId, CoordinateBound)>,
}

impl SessionPatch {
    #[must_use]
    pub const fn new(expected_revisions: SessionRevisions) -> Self {
        Self {
            expected_revisions,
            variable_values: Vec::new(),
            source_replacements: Vec::new(),
            residual_replacements: Vec::new(),
            bound_replacements: Vec::new(),
        }
    }

    #[must_use]
    pub const fn expected_revisions(&self) -> SessionRevisions {
        self.expected_revisions
    }

    /// Adds an ambient variable-value replacement. Dirty components are
    /// derived by the session, including fixed/alias fanout.
    pub fn set_variable_value(
        &mut self,
        variable_id: VariableId,
        value: VariableValue,
    ) -> &mut Self {
        self.variable_values.push((variable_id, value));
        self
    }

    /// Replaces source audit metadata without changing source identity/order.
    pub fn replace_source(
        &mut self,
        source_id: SourceConstraintId,
        source: SourceConstraint,
    ) -> &mut Self {
        self.source_replacements.push((source_id, source));
        self
    }

    /// Replaces evaluator parameters, scales, and audit payload while retaining
    /// source/category/incidence/output/elimination shape and residual identity.
    pub fn replace_residual(
        &mut self,
        residual_id: ResidualId,
        residual: ResidualBlock,
    ) -> &mut Self {
        self.residual_replacements.push((residual_id, residual));
        self
    }

    /// Replaces endpoints/label of an existing coordinate bound while retaining
    /// bound identity, variable, coordinate, and order.
    pub fn replace_bound(&mut self, bound_id: BoundId, bound: CoordinateBound) -> &mut Self {
        self.bound_replacements.push((bound_id, bound));
        self
    }

    fn is_empty(&self) -> bool {
        self.variable_values.is_empty()
            && self.source_replacements.is_empty()
            && self.residual_replacements.is_empty()
            && self.bound_replacements.is_empty()
    }
}

/// Revision-checked accepted-state metadata refresh.
///
/// This patch cannot replace an evaluator, scale, equation, variable, or bound.
/// Equation-affecting changes must use [`SessionPatch`] and a normal solve.
#[derive(Debug)]
pub struct AcceptedAuditPatch {
    expected_revisions: SessionRevisions,
    source_replacements: Vec<(SourceConstraintId, SourceConstraint)>,
    residual_audit_replacements: Vec<(ResidualId, Vec<ResidualRowAudit>)>,
}

impl AcceptedAuditPatch {
    #[must_use]
    pub const fn new(expected_revisions: SessionRevisions) -> Self {
        Self {
            expected_revisions,
            source_replacements: Vec::new(),
            residual_audit_replacements: Vec::new(),
        }
    }

    pub fn replace_source(
        &mut self,
        source_id: SourceConstraintId,
        source: SourceConstraint,
    ) -> &mut Self {
        self.source_replacements.push((source_id, source));
        self
    }

    pub fn replace_residual_rows(
        &mut self,
        residual_id: ResidualId,
        rows: Vec<ResidualRowAudit>,
    ) -> &mut Self {
        self.residual_audit_replacements.push((residual_id, rows));
        self
    }

    fn is_empty(&self) -> bool {
        self.source_replacements.is_empty() && self.residual_audit_replacements.is_empty()
    }
}

/// A core-level reason that a fully evaluated candidate was not committed.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SessionCoreRejection {
    HardValidity(HardValidity),
    HardResidual { maximum: f64, tolerance: f64 },
    RankInvalid,
    BoundViolation(BoundId),
    EvaluationFailure,
    NonFiniteReport,
}

/// Construction or pre-mutation patch validation failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SessionError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("stale solve-session patch: expected {expected:?}, accepted {actual:?}")]
    StalePatch {
        expected: SessionRevisions,
        actual: SessionRevisions,
    },
    #[error("session patch repeats {kind} ID {id}")]
    DuplicatePatchTarget { kind: &'static str, id: String },
    #[error("initial problem is not an accepted finite session state: {0:?}")]
    InitialRejected(SessionCoreRejection),
}

/// Attempt outcome. A rejected transaction owns its attempted report while the
/// session retains its prior accepted report bitwise.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionTransaction<R> {
    pub report: SolveReport,
    pub rejection: Option<SessionTransactionRejection<R>>,
    pub revisions: SessionRevisions,
}

impl<R> SessionTransaction<R> {
    #[must_use]
    pub const fn committed(&self) -> bool {
        self.rejection.is_none()
    }
}

/// Core or domain decision attached to a rejected transaction.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum SessionTransactionRejection<R> {
    Core(SessionCoreRejection),
    Domain(R),
}

/// Domain rejection plus its authoritative effect on attempted hard validity.
#[derive(Clone, Debug, PartialEq)]
pub struct SessionDomainRejection<R> {
    pub reason: R,
    pub hard_validity: HardValidity,
}

impl<R> SessionDomainRejection<R> {
    #[must_use]
    pub const fn new(reason: R, hard_validity: HardValidity) -> Self {
        Self {
            reason,
            hard_validity,
        }
    }

    #[must_use]
    pub const fn invalid(reason: R) -> Self {
        Self::new(reason, HardValidity::Invalid)
    }

    #[must_use]
    pub const fn not_evaluated(reason: R) -> Self {
        Self::new(reason, HardValidity::NotEvaluated)
    }

    #[must_use]
    pub const fn compatibility(reason: R) -> Self {
        Self::new(reason, HardValidity::Valid)
    }
}

/// Persistent compiled problem, accepted cache/report, and dependency stamps.
#[derive(Clone, Debug)]
pub struct SolveSession {
    problem: Problem,
    config: SolverConfig,
    report: SolveReport,
    revisions: SessionRevisions,
    component_stamps: Vec<ComponentDependencyStamp>,
    plan: EliminationPlan,
}

impl SolveSession {
    /// Solves and independently validates an initial accepted state.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid declarations/configuration or when the
    /// initial solve is not finite, hard-valid, rank-valid, and bound-feasible.
    pub fn new(mut problem: Problem, config: SolverConfig) -> Result<Self, SessionError> {
        let report = problem.solve(config)?;
        if let Some(rejection) = core_rejection(&report, config) {
            return Err(SessionError::InitialRejected(rejection));
        }
        let revisions = SessionRevisions::default();
        let plan = EliminationPlan::new(&problem)?;
        let component_stamps = initial_stamps(&plan, revisions);
        Ok(Self {
            problem,
            config,
            report,
            revisions,
            component_stamps,
            plan,
        })
    }

    #[must_use]
    pub const fn problem(&self) -> &Problem {
        &self.problem
    }

    #[must_use]
    pub const fn report(&self) -> &SolveReport {
        &self.report
    }

    #[must_use]
    pub const fn config(&self) -> SolverConfig {
        self.config
    }

    #[must_use]
    pub const fn revisions(&self) -> SessionRevisions {
        self.revisions
    }

    #[must_use]
    pub fn component_dependency_stamps(&self) -> &[ComponentDependencyStamp] {
        &self.component_stamps
    }

    /// Applies a core-only transaction.
    ///
    /// # Errors
    ///
    /// Returns only stale/invalid patch or solver-start failures. Numerical
    /// candidate rejection is represented in the returned transaction.
    pub fn apply(
        &mut self,
        patch: SessionPatch,
    ) -> Result<SessionTransaction<std::convert::Infallible>, SessionError> {
        self.apply_with(patch, |_, _| Ok(()))
    }

    /// Applies a transaction with one domain decision before atomic commit.
    /// The callback observes only the candidate and cannot mutate the session.
    ///
    /// # Errors
    ///
    /// Returns only stale/invalid patch or solver-start failures. Core/domain
    /// rejection is represented in the returned transaction.
    #[allow(clippy::too_many_lines)]
    pub fn apply_with<R, F>(
        &mut self,
        patch: SessionPatch,
        decide: F,
    ) -> Result<SessionTransaction<R>, SessionError>
    where
        F: FnOnce(&Problem, &SolveReport) -> Result<(), R>,
    {
        self.apply_with_output(patch, |problem, report| {
            decide(problem, report).map_err(SessionDomainRejection::invalid)
        })
        .map(|(transaction, _)| transaction)
    }

    /// Applies a transaction whose validation closure constructs the complete
    /// domain candidate before core state is committed.
    ///
    /// # Errors
    ///
    /// Returns stale/invalid patch or solver-start failures. A rejected domain
    /// candidate is returned in the transaction with attempted hard validity
    /// updated from [`SessionDomainRejection`].
    #[allow(clippy::too_many_lines)]
    pub fn apply_with_output<R, T, F>(
        &mut self,
        patch: SessionPatch,
        decide: F,
    ) -> Result<(SessionTransaction<R>, Option<T>), SessionError>
    where
        F: FnOnce(&Problem, &SolveReport) -> Result<T, SessionDomainRejection<R>>,
    {
        if patch.expected_revisions != self.revisions {
            return Err(SessionError::StalePatch {
                expected: patch.expected_revisions,
                actual: self.revisions,
            });
        }
        validate_unique_targets(&patch)?;
        let patch_is_empty = patch.is_empty();
        let plan = &self.plan;
        if plan.component_layouts.len() != plan.components.len() {
            return Err(CoreError::DimensionMismatch {
                context: "cached session component layouts",
                expected: plan.components.len(),
                actual: plan.component_layouts.len(),
            }
            .into());
        }
        let mut candidate = self.problem.clone();
        let mut dirty_components = Vec::new();
        let source_changed =
            !patch.source_replacements.is_empty() || !patch.residual_replacements.is_empty();
        let bound_changed = !patch.bound_replacements.is_empty();

        for (variable_id, value) in patch.variable_values {
            add_variable_dependencies(plan, variable_id, &mut dirty_components)?;
            candidate.set_variable_value(variable_id, value)?;
        }
        for (source_id, source) in patch.source_replacements {
            for (residual_id, residual) in self.problem.residuals.iter() {
                if residual.source() == source_id {
                    add_residual_dependencies(
                        plan,
                        residual_id,
                        residual.incident_variables(),
                        &mut dirty_components,
                    )?;
                }
            }
            candidate.replace_source(source_id, source)?;
        }
        for (residual_id, residual) in patch.residual_replacements {
            let incidence = self
                .problem
                .residual(residual_id)
                .ok_or(CoreError::UnknownResidual(residual_id))?
                .incident_variables()
                .to_vec();
            add_residual_dependencies(plan, residual_id, &incidence, &mut dirty_components)?;
            candidate.replace_residual_compatible(residual_id, residual)?;
        }
        for (bound_id, bound) in patch.bound_replacements {
            let variable_id = self
                .problem
                .bound(bound_id)
                .ok_or(CoreError::UnknownBound(bound_id))?
                .variable_id();
            add_variable_dependencies(plan, variable_id, &mut dirty_components)?;
            candidate.replace_bound_compatible(bound_id, bound)?;
        }
        dirty_components.sort_unstable();
        dirty_components.dedup();

        let mut report =
            candidate.solve_session_components(self.config, &dirty_components, &self.plan)?;
        let mut output = None;
        let rejection = if let Some(rejection) = core_rejection_before_domain(&report, self.config)
        {
            Some(SessionTransactionRejection::Core(rejection))
        } else {
            match decide(&candidate, &report) {
                Ok(candidate_output) => {
                    if let Some(rejection) = secondary_rejection(&report) {
                        Some(SessionTransactionRejection::Core(rejection))
                    } else {
                        output = Some(candidate_output);
                        None
                    }
                }
                Err(rejection) => {
                    report.hard_validity = rejection.hard_validity;
                    Some(SessionTransactionRejection::Domain(rejection.reason))
                }
            }
        };
        if rejection.is_some() {
            return Ok((
                SessionTransaction {
                    report,
                    rejection,
                    revisions: self.revisions,
                },
                None,
            ));
        }

        let mut revisions = self.revisions;
        if !patch_is_empty {
            revisions.state = revisions.state.saturating_add(1);
        }
        if source_changed {
            revisions.source = revisions.source.saturating_add(1);
        }
        if bound_changed {
            revisions.bound = revisions.bound.saturating_add(1);
        }
        let mut stamps = self.component_stamps.clone();
        let stamp_count = stamps.len();
        for &component_index in &dirty_components {
            let stamp = stamps
                .get_mut(component_index)
                .ok_or(CoreError::DimensionMismatch {
                    context: "session component dependency stamp",
                    expected: stamp_count,
                    actual: component_index,
                })?;
            stamp.state_revision = revisions.state;
            if source_changed {
                stamp.source_revision = revisions.source;
            }
            if bound_changed {
                stamp.bound_revision = revisions.bound;
            }
        }
        self.problem = candidate;
        self.report = report.clone();
        self.revisions = revisions;
        self.component_stamps = stamps;
        Ok((
            SessionTransaction {
                report,
                rejection: None,
                revisions,
            },
            output,
        ))
    }

    /// Refreshes accepted source labels and residual row descriptors without
    /// exposing an equation-changing post-acceptance path.
    ///
    /// # Errors
    ///
    /// Rejects stale, duplicate, or invalid metadata and leaves this session
    /// unchanged. A non-empty refresh advances the source revision.
    pub fn refresh_accepted_audit(
        &mut self,
        patch: AcceptedAuditPatch,
    ) -> Result<(), SessionError> {
        if patch.expected_revisions != self.revisions {
            return Err(SessionError::StalePatch {
                expected: patch.expected_revisions,
                actual: self.revisions,
            });
        }
        validate_unique_audit_targets(&patch)?;
        if patch.is_empty() {
            return Ok(());
        }
        let refreshed_residuals = patch
            .residual_audit_replacements
            .iter()
            .map(|(residual_id, _)| *residual_id)
            .collect::<Vec<_>>();
        let refreshed_sources = patch
            .source_replacements
            .iter()
            .map(|(source_id, _)| *source_id)
            .collect::<Vec<_>>();
        let mut candidate = self.problem.clone();
        let mut affected_components = Vec::new();
        for (source_id, source) in patch.source_replacements {
            for (residual_id, residual) in self.problem.residuals.iter() {
                if residual.source() == source_id {
                    add_residual_dependencies(
                        &self.plan,
                        residual_id,
                        residual.incident_variables(),
                        &mut affected_components,
                    )?;
                }
            }
            candidate.replace_source(source_id, source)?;
        }
        for (residual_id, rows) in patch.residual_audit_replacements {
            let residual = self
                .problem
                .residual(residual_id)
                .ok_or(CoreError::UnknownResidual(residual_id))?;
            add_residual_dependencies(
                &self.plan,
                residual_id,
                residual.incident_variables(),
                &mut affected_components,
            )?;
            candidate.replace_residual_audit_rows(residual_id, rows)?;
        }

        let fresh_audit = candidate.audit_snapshot_partial_for_residuals(&refreshed_residuals);
        let mut report = self.report.clone();
        merge_refreshed_audit(&mut report.audit, fresh_audit);
        for source_id in refreshed_sources {
            let label = candidate
                .source(source_id)
                .ok_or(CoreError::UnknownSource(source_id))?
                .label();
            if let Some(source) = report
                .audit
                .sources
                .iter_mut()
                .find(|source| source.source_id == source_id)
            {
                label.clone_into(&mut source.source_label);
            }
        }
        if let Some(cache) = candidate.decomposition_cache.as_mut() {
            cache.report = Some(Box::new(report.clone()));
        }
        let source_revision = self.revisions.source.saturating_add(1);
        let mut stamps = self.component_stamps.clone();
        affected_components.sort_unstable();
        affected_components.dedup();
        for component in affected_components {
            let stamp_count = stamps.len();
            let stamp = stamps
                .get_mut(component)
                .ok_or(CoreError::DimensionMismatch {
                    context: "session component dependency stamp",
                    expected: stamp_count,
                    actual: component,
                })?;
            stamp.source_revision = source_revision;
        }
        self.problem = candidate;
        self.report = report;
        self.revisions.source = source_revision;
        self.component_stamps = stamps;
        Ok(())
    }

    /// Explicitly rebuilds structurally changed topology as one validated swap.
    ///
    /// # Errors
    ///
    /// Rejects stale callers before compiling/solving and retains this session
    /// if construction of the replacement accepted state fails.
    pub fn rebuild(
        &mut self,
        expected_revisions: SessionRevisions,
        problem: Problem,
    ) -> Result<&SolveReport, SessionError> {
        if expected_revisions != self.revisions {
            return Err(SessionError::StalePatch {
                expected: expected_revisions,
                actual: self.revisions,
            });
        }
        let mut rebuilt = Self::new(problem, self.config)?;
        rebuilt.revisions = SessionRevisions {
            topology: self.revisions.topology.saturating_add(1),
            source: self.revisions.source.saturating_add(1),
            state: self.revisions.state.saturating_add(1),
            bound: self.revisions.bound.saturating_add(1),
        };
        rebuilt.component_stamps = initial_stamps(&rebuilt.plan, rebuilt.revisions);
        *self = rebuilt;
        Ok(&self.report)
    }
}

fn merge_refreshed_audit(retained: &mut crate::AuditSnapshot, fresh: crate::AuditSnapshot) {
    for fresh_source in fresh.sources {
        let Some(retained_source) = retained
            .sources
            .iter_mut()
            .find(|source| source.source_id == fresh_source.source_id)
        else {
            continue;
        };
        retained_source.source_label = fresh_source.source_label;
        for mut fresh_row in fresh_source.rows {
            if let Some(retained_row) = retained_source.rows.iter_mut().find(|row| {
                row.residual_id == fresh_row.residual_id
                    && row.row_in_block == fresh_row.row_in_block
            }) {
                fresh_row.annotations = retained_row.annotations;
                fresh_row
                    .active_bounds
                    .clone_from(&retained_row.active_bounds);
                *retained_row = fresh_row;
            }
        }
    }
}

fn validate_unique_targets(patch: &SessionPatch) -> Result<(), SessionError> {
    unique_ids(
        "variable",
        patch
            .variable_values
            .iter()
            .map(|(id, _)| format!("{id:?}")),
    )?;
    unique_ids(
        "source",
        patch
            .source_replacements
            .iter()
            .map(|(id, _)| format!("{id:?}")),
    )?;
    unique_ids(
        "residual",
        patch
            .residual_replacements
            .iter()
            .map(|(id, _)| format!("{id:?}")),
    )?;
    unique_ids(
        "bound",
        patch
            .bound_replacements
            .iter()
            .map(|(id, _)| format!("{id:?}")),
    )
}

fn validate_unique_audit_targets(patch: &AcceptedAuditPatch) -> Result<(), SessionError> {
    unique_ids(
        "source",
        patch
            .source_replacements
            .iter()
            .map(|(id, _)| format!("{id:?}")),
    )?;
    unique_ids(
        "residual audit",
        patch
            .residual_audit_replacements
            .iter()
            .map(|(id, _)| format!("{id:?}")),
    )
}

fn unique_ids(kind: &'static str, ids: impl Iterator<Item = String>) -> Result<(), SessionError> {
    let mut seen = Vec::new();
    for id in ids {
        if seen.contains(&id) {
            return Err(SessionError::DuplicatePatchTarget { kind, id });
        }
        seen.push(id);
    }
    Ok(())
}

fn add_variable_dependencies(
    plan: &EliminationPlan,
    variable_id: VariableId,
    dirty: &mut Vec<usize>,
) -> Result<(), CoreError> {
    if plan.root(variable_id).is_none() {
        return Err(CoreError::UnknownVariable(variable_id));
    }
    for component in &plan.components {
        if component.variable_ids.contains(&variable_id)
            || component.referenced_variables.contains(&variable_id)
            || component.active_group_indices.iter().any(|&group| {
                plan.active_groups[group].root == variable_id
                    || plan.active_groups[group].members.contains(&variable_id)
            })
        {
            dirty.push(component.index);
        }
    }
    Ok(())
}

fn add_residual_dependencies(
    plan: &EliminationPlan,
    residual_id: ResidualId,
    incidence: &[VariableId],
    dirty: &mut Vec<usize>,
) -> Result<(), CoreError> {
    let mut found = false;
    for component in &plan.components {
        if component.residual_ids.contains(&residual_id) {
            dirty.push(component.index);
            found = true;
        }
    }
    for &variable_id in incidence {
        add_variable_dependencies(plan, variable_id, dirty)?;
    }
    // Valid fixed/componentless secondary rows have no reduced hard component.
    // Their priority pass and returned audit are evaluated globally.
    let _ = found;
    Ok(())
}

fn initial_stamps(
    plan: &EliminationPlan,
    revisions: SessionRevisions,
) -> Vec<ComponentDependencyStamp> {
    plan.components
        .iter()
        .map(|component| ComponentDependencyStamp {
            component_index: component.index,
            topology_revision: revisions.topology,
            source_revision: revisions.source,
            state_revision: revisions.state,
            bound_revision: revisions.bound,
        })
        .collect()
}

fn core_rejection(report: &SolveReport, config: SolverConfig) -> Option<SessionCoreRejection> {
    core_rejection_before_domain(report, config).or_else(|| secondary_rejection(report))
}

fn core_rejection_before_domain(
    report: &SolveReport,
    config: SolverConfig,
) -> Option<SessionCoreRejection> {
    if report.hard_validity != HardValidity::Valid {
        return Some(SessionCoreRejection::HardValidity(report.hard_validity));
    }
    if !report.hard_residuals_validated
        || report.hard_residual_max > config.normalized_residual_tolerance
    {
        return Some(SessionCoreRejection::HardResidual {
            maximum: report.hard_residual_max,
            tolerance: config.normalized_residual_tolerance,
        });
    }
    if !report.rank_is_valid {
        return Some(SessionCoreRejection::RankInvalid);
    }
    if let Some(bound) = report.bounds.iter().find(|bound| {
        !bound.value.is_finite()
            || bound.lower.is_some_and(|lower| bound.value < lower)
            || bound.upper.is_some_and(|upper| bound.value > upper)
            || matches!(bound.status, BoundStatus::Fixed) && bound.lower != bound.upper
    }) {
        return Some(SessionCoreRejection::BoundViolation(bound.bound_id));
    }
    if report
        .accepted_state
        .ambient()
        .iter()
        .any(|value| !value.is_finite())
        || report.audit.sources.iter().any(|source| {
            source
                .rows
                .iter()
                .any(|row| !row.raw_residual.is_finite() || !row.normalized_residual.is_finite())
        })
    {
        return Some(SessionCoreRejection::NonFiniteReport);
    }
    None
}

fn secondary_rejection(report: &SolveReport) -> Option<SessionCoreRejection> {
    (matches!(report.temporary_status, SecondaryStatus::EvaluationFailure)
        || matches!(report.preference_status, SecondaryStatus::EvaluationFailure))
    .then_some(SessionCoreRejection::EvaluationFailure)
}
