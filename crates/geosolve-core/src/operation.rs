// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::{Deref, DerefMut};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Supported per-axis bound for dense kernels in controlled M35 operations.
pub const CONTROLLED_DENSE_KERNEL_MAX_DIMENSION: usize = 256;

/// A deterministic safe boundary at which an operation may stop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OperationCheckpoint {
    DocumentValidation,
    DocumentDependency,
    DocumentLowering,
    ComponentBoundary,
    BeforeNonlinearIteration,
    AfterNonlinearIteration,
    BeforeTrialBoundary,
    AfterTrialBoundary,
    BeforeFactorization,
    AfterFactorization,
    BeforeRankKernel,
    AfterRankKernel,
    DiagnosticCandidate,
    DiagnosticTrial,
    ProfileCandidate,
    ProfileSubdivision,
    ProfileIntegration,
    ProfileContainment,
    ProfileFace,
    MeasurementIntegration,
    MeasurementDerivative,
    BeforeFinalValidation,
    AfterFinalValidation,
    BeforeCommit,
}

/// One deterministic class of algorithmic work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OperationWorkCounter {
    DocumentValidationItems,
    DocumentDependencyItems,
    DocumentLoweringItems,
    NonlinearIterations,
    RejectedTrials,
    ComponentLinearizations,
    DenseKernelRows,
    DenseKernelColumns,
    DenseKernelWorkUnits,
    Factorizations,
    RankKernels,
    DiagnosticCandidates,
    DiagnosticTrials,
    ProfileCandidatePairs,
    ProfileSubdivisions,
    ProfileRoots,
    ProfileFragments,
    ProfileIntegrations,
    ProfileContainmentTests,
    ProfileFaces,
    MeasurementIntegrations,
    MeasurementDerivativeEvaluations,
}

/// Typed deterministic limits for one synchronous operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationLimits {
    pub document_validation_items: usize,
    pub document_dependency_items: usize,
    pub document_lowering_items: usize,
    pub nonlinear_iterations: usize,
    pub rejected_trials: usize,
    pub component_linearizations: usize,
    /// Maximum dense-kernel row dimension. Values above 256 are clamped by the controller.
    pub dense_kernel_rows: usize,
    /// Maximum dense-kernel column dimension. Values above 256 are clamped by the controller.
    pub dense_kernel_columns: usize,
    /// Additive conservative dense-kernel work, charged as `max(rows, columns)^3` per kernel.
    pub dense_kernel_work_units: usize,
    pub factorizations: usize,
    pub rank_kernels: usize,
    pub diagnostic_candidates: usize,
    pub diagnostic_trials: usize,
    pub profile_candidate_pairs: usize,
    pub profile_subdivisions: usize,
    pub profile_roots: usize,
    pub profile_fragments: usize,
    pub profile_integrations: usize,
    pub profile_containment_tests: usize,
    pub profile_faces: usize,
    pub measurement_integrations: usize,
    pub measurement_derivative_evaluations: usize,
}

impl OperationLimits {
    #[must_use]
    pub const fn unlimited() -> Self {
        Self {
            document_validation_items: usize::MAX,
            document_dependency_items: usize::MAX,
            document_lowering_items: usize::MAX,
            nonlinear_iterations: usize::MAX,
            rejected_trials: usize::MAX,
            component_linearizations: usize::MAX,
            dense_kernel_rows: CONTROLLED_DENSE_KERNEL_MAX_DIMENSION,
            dense_kernel_columns: CONTROLLED_DENSE_KERNEL_MAX_DIMENSION,
            dense_kernel_work_units: usize::MAX,
            factorizations: usize::MAX,
            rank_kernels: usize::MAX,
            diagnostic_candidates: usize::MAX,
            diagnostic_trials: usize::MAX,
            profile_candidate_pairs: usize::MAX,
            profile_subdivisions: usize::MAX,
            profile_roots: usize::MAX,
            profile_fragments: usize::MAX,
            profile_integrations: usize::MAX,
            profile_containment_tests: usize::MAX,
            profile_faces: usize::MAX,
            measurement_integrations: usize::MAX,
            measurement_derivative_evaluations: usize::MAX,
        }
    }

    const fn limit(self, counter: OperationWorkCounter) -> usize {
        match counter {
            OperationWorkCounter::DocumentValidationItems => self.document_validation_items,
            OperationWorkCounter::DocumentDependencyItems => self.document_dependency_items,
            OperationWorkCounter::DocumentLoweringItems => self.document_lowering_items,
            OperationWorkCounter::NonlinearIterations => self.nonlinear_iterations,
            OperationWorkCounter::RejectedTrials => self.rejected_trials,
            OperationWorkCounter::ComponentLinearizations => self.component_linearizations,
            OperationWorkCounter::DenseKernelRows => self.dense_kernel_rows,
            OperationWorkCounter::DenseKernelColumns => self.dense_kernel_columns,
            OperationWorkCounter::DenseKernelWorkUnits => self.dense_kernel_work_units,
            OperationWorkCounter::Factorizations => self.factorizations,
            OperationWorkCounter::RankKernels => self.rank_kernels,
            OperationWorkCounter::DiagnosticCandidates => self.diagnostic_candidates,
            OperationWorkCounter::DiagnosticTrials => self.diagnostic_trials,
            OperationWorkCounter::ProfileCandidatePairs => self.profile_candidate_pairs,
            OperationWorkCounter::ProfileSubdivisions => self.profile_subdivisions,
            OperationWorkCounter::ProfileRoots => self.profile_roots,
            OperationWorkCounter::ProfileFragments => self.profile_fragments,
            OperationWorkCounter::ProfileIntegrations => self.profile_integrations,
            OperationWorkCounter::ProfileContainmentTests => self.profile_containment_tests,
            OperationWorkCounter::ProfileFaces => self.profile_faces,
            OperationWorkCounter::MeasurementIntegrations => self.measurement_integrations,
            OperationWorkCounter::MeasurementDerivativeEvaluations => {
                self.measurement_derivative_evaluations
            }
        }
    }

    fn optional_ceiling(self, consumed: OperationWork) -> Self {
        fn half_remaining(limit: usize, current: usize) -> usize {
            current.saturating_add(limit.saturating_sub(current) / 2)
        }

        Self {
            document_validation_items: half_remaining(
                self.document_validation_items,
                consumed.document_validation_items,
            ),
            document_dependency_items: half_remaining(
                self.document_dependency_items,
                consumed.document_dependency_items,
            ),
            document_lowering_items: half_remaining(
                self.document_lowering_items,
                consumed.document_lowering_items,
            ),
            nonlinear_iterations: half_remaining(
                self.nonlinear_iterations,
                consumed.nonlinear_iterations,
            ),
            rejected_trials: half_remaining(self.rejected_trials, consumed.rejected_trials),
            component_linearizations: half_remaining(
                self.component_linearizations,
                consumed.component_linearizations,
            ),
            // Dense dimensions are maximum authorizations, not additive work.
            dense_kernel_rows: self.dense_kernel_rows,
            dense_kernel_columns: self.dense_kernel_columns,
            dense_kernel_work_units: half_remaining(
                self.dense_kernel_work_units,
                consumed.dense_kernel_work_units,
            ),
            factorizations: half_remaining(self.factorizations, consumed.factorizations),
            rank_kernels: half_remaining(self.rank_kernels, consumed.rank_kernels),
            diagnostic_candidates: half_remaining(
                self.diagnostic_candidates,
                consumed.diagnostic_candidates,
            ),
            diagnostic_trials: half_remaining(self.diagnostic_trials, consumed.diagnostic_trials),
            profile_candidate_pairs: half_remaining(
                self.profile_candidate_pairs,
                consumed.profile_candidate_pairs,
            ),
            profile_subdivisions: half_remaining(
                self.profile_subdivisions,
                consumed.profile_subdivisions,
            ),
            profile_roots: half_remaining(self.profile_roots, consumed.profile_roots),
            profile_fragments: half_remaining(self.profile_fragments, consumed.profile_fragments),
            profile_integrations: half_remaining(
                self.profile_integrations,
                consumed.profile_integrations,
            ),
            profile_containment_tests: half_remaining(
                self.profile_containment_tests,
                consumed.profile_containment_tests,
            ),
            profile_faces: half_remaining(self.profile_faces, consumed.profile_faces),
            measurement_integrations: half_remaining(
                self.measurement_integrations,
                consumed.measurement_integrations,
            ),
            measurement_derivative_evaluations: half_remaining(
                self.measurement_derivative_evaluations,
                consumed.measurement_derivative_evaluations,
            ),
        }
    }
}

impl Default for OperationLimits {
    fn default() -> Self {
        Self::unlimited()
    }
}

/// Algorithmic work consumed by one operation. Counters never wrap.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OperationWork {
    pub document_validation_items: usize,
    pub document_dependency_items: usize,
    pub document_lowering_items: usize,
    pub nonlinear_iterations: usize,
    pub rejected_trials: usize,
    pub component_linearizations: usize,
    /// Largest authorized dense-kernel row dimension.
    pub dense_kernel_rows: usize,
    /// Largest authorized dense-kernel column dimension.
    pub dense_kernel_columns: usize,
    /// Additive conservative dense-kernel work charged as `max(rows, columns)^3`.
    pub dense_kernel_work_units: usize,
    pub factorizations: usize,
    pub rank_kernels: usize,
    pub diagnostic_candidates: usize,
    pub diagnostic_trials: usize,
    pub profile_candidate_pairs: usize,
    pub profile_subdivisions: usize,
    pub profile_roots: usize,
    pub profile_fragments: usize,
    pub profile_integrations: usize,
    pub profile_containment_tests: usize,
    pub profile_faces: usize,
    pub measurement_integrations: usize,
    pub measurement_derivative_evaluations: usize,
}

impl OperationWork {
    const fn consumed(self, counter: OperationWorkCounter) -> usize {
        match counter {
            OperationWorkCounter::DocumentValidationItems => self.document_validation_items,
            OperationWorkCounter::DocumentDependencyItems => self.document_dependency_items,
            OperationWorkCounter::DocumentLoweringItems => self.document_lowering_items,
            OperationWorkCounter::NonlinearIterations => self.nonlinear_iterations,
            OperationWorkCounter::RejectedTrials => self.rejected_trials,
            OperationWorkCounter::ComponentLinearizations => self.component_linearizations,
            OperationWorkCounter::DenseKernelRows => self.dense_kernel_rows,
            OperationWorkCounter::DenseKernelColumns => self.dense_kernel_columns,
            OperationWorkCounter::DenseKernelWorkUnits => self.dense_kernel_work_units,
            OperationWorkCounter::Factorizations => self.factorizations,
            OperationWorkCounter::RankKernels => self.rank_kernels,
            OperationWorkCounter::DiagnosticCandidates => self.diagnostic_candidates,
            OperationWorkCounter::DiagnosticTrials => self.diagnostic_trials,
            OperationWorkCounter::ProfileCandidatePairs => self.profile_candidate_pairs,
            OperationWorkCounter::ProfileSubdivisions => self.profile_subdivisions,
            OperationWorkCounter::ProfileRoots => self.profile_roots,
            OperationWorkCounter::ProfileFragments => self.profile_fragments,
            OperationWorkCounter::ProfileIntegrations => self.profile_integrations,
            OperationWorkCounter::ProfileContainmentTests => self.profile_containment_tests,
            OperationWorkCounter::ProfileFaces => self.profile_faces,
            OperationWorkCounter::MeasurementIntegrations => self.measurement_integrations,
            OperationWorkCounter::MeasurementDerivativeEvaluations => {
                self.measurement_derivative_evaluations
            }
        }
    }

    fn set(&mut self, counter: OperationWorkCounter, value: usize) {
        match counter {
            OperationWorkCounter::DocumentValidationItems => self.document_validation_items = value,
            OperationWorkCounter::DocumentDependencyItems => self.document_dependency_items = value,
            OperationWorkCounter::DocumentLoweringItems => self.document_lowering_items = value,
            OperationWorkCounter::NonlinearIterations => self.nonlinear_iterations = value,
            OperationWorkCounter::RejectedTrials => self.rejected_trials = value,
            OperationWorkCounter::ComponentLinearizations => self.component_linearizations = value,
            OperationWorkCounter::DenseKernelRows => self.dense_kernel_rows = value,
            OperationWorkCounter::DenseKernelColumns => self.dense_kernel_columns = value,
            OperationWorkCounter::DenseKernelWorkUnits => {
                self.dense_kernel_work_units = value;
            }
            OperationWorkCounter::Factorizations => self.factorizations = value,
            OperationWorkCounter::RankKernels => self.rank_kernels = value,
            OperationWorkCounter::DiagnosticCandidates => self.diagnostic_candidates = value,
            OperationWorkCounter::DiagnosticTrials => self.diagnostic_trials = value,
            OperationWorkCounter::ProfileCandidatePairs => self.profile_candidate_pairs = value,
            OperationWorkCounter::ProfileSubdivisions => self.profile_subdivisions = value,
            OperationWorkCounter::ProfileRoots => self.profile_roots = value,
            OperationWorkCounter::ProfileFragments => self.profile_fragments = value,
            OperationWorkCounter::ProfileIntegrations => self.profile_integrations = value,
            OperationWorkCounter::ProfileContainmentTests => self.profile_containment_tests = value,
            OperationWorkCounter::ProfileFaces => self.profile_faces = value,
            OperationWorkCounter::MeasurementIntegrations => self.measurement_integrations = value,
            OperationWorkCounter::MeasurementDerivativeEvaluations => {
                self.measurement_derivative_evaluations = value;
            }
        }
    }
}

/// Exact operation-control reason that stopped work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OperationStopReason {
    Cancelled {
        checkpoint: OperationCheckpoint,
    },
    WorkExhausted {
        counter: OperationWorkCounter,
        checkpoint: OperationCheckpoint,
    },
}

/// Configured and consumed deterministic work for one operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationReport {
    pub configured: OperationLimits,
    pub consumed: OperationWork,
    pub stopping_reason: Option<OperationStopReason>,
}

/// Completion or interruption of one synchronous operation.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum OperationOutcome<T> {
    Completed { value: T, report: OperationReport },
    Cancelled { report: OperationReport },
    WorkExhausted { report: OperationReport },
}

impl<T> OperationOutcome<T> {
    #[must_use]
    pub const fn report(&self) -> &OperationReport {
        match self {
            Self::Completed { report, .. }
            | Self::Cancelled { report }
            | Self::WorkExhausted { report } => report,
        }
    }

    /// Maps only a completed value while preserving cancellation/work-exhaustion
    /// evidence exactly.
    #[must_use]
    pub fn map<U, F>(self, map: F) -> OperationOutcome<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Completed { value, report } => OperationOutcome::Completed {
                value: map(value),
                report,
            },
            Self::Cancelled { report } => OperationOutcome::Cancelled { report },
            Self::WorkExhausted { report } => OperationOutcome::WorkExhausted { report },
        }
    }
}

/// Read-only monotonic cancellation state passed to an operation.
#[derive(Clone, Debug, Default)]
pub struct CancellationToken {
    requested: Arc<AtomicBool>,
}

impl CancellationToken {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}

/// Host-owned monotonic cancellation requester.
#[derive(Clone, Debug)]
pub struct CancellationHandle {
    requested: Arc<AtomicBool>,
}

impl CancellationHandle {
    pub fn cancel(&self) {
        self.requested.store(true, Ordering::Release);
    }
}

/// Creates one operation-local cancellation handle/token pair.
#[must_use]
pub fn cancellation_pair() -> (CancellationHandle, CancellationToken) {
    let requested = Arc::new(AtomicBool::new(false));
    (
        CancellationHandle {
            requested: Arc::clone(&requested),
        },
        CancellationToken { requested },
    )
}

/// Cancellation and deterministic limits supplied to one operation.
#[derive(Clone, Debug)]
pub struct OperationControl {
    pub token: CancellationToken,
    pub limits: OperationLimits,
}

impl OperationControl {
    #[must_use]
    pub const fn new(token: CancellationToken, limits: OperationLimits) -> Self {
        Self { token, limits }
    }

    #[must_use]
    pub fn unlimited() -> Self {
        Self {
            token: CancellationToken::default(),
            limits: OperationLimits::unlimited(),
        }
    }
}

impl Default for OperationControl {
    fn default() -> Self {
        Self::unlimited()
    }
}

/// Mutable per-call tracker used by domain and numerical operation boundaries.
#[derive(Debug)]
pub struct OperationController {
    control: OperationControl,
    consumed: OperationWork,
    stopping_reason: Option<OperationStopReason>,
}

/// Result of work that may improve an already-valid candidate but is not
/// required to publish it.
pub(crate) enum OptionalWorkOutcome<T> {
    Completed(T),
    WorkExhausted(T),
    Interrupted,
}

/// Scoped operation boundary that always checks its after checkpoint on exit.
pub(crate) struct OperationBoundary<'a> {
    controller: &'a mut OperationController,
    after: OperationCheckpoint,
}

impl Deref for OperationBoundary<'_> {
    type Target = OperationController;

    fn deref(&self) -> &Self::Target {
        self.controller
    }
}

impl DerefMut for OperationBoundary<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.controller
    }
}

impl Drop for OperationBoundary<'_> {
    fn drop(&mut self) {
        let _ = self.controller.checkpoint(self.after);
    }
}

impl OperationController {
    #[must_use]
    pub fn new(mut control: OperationControl) -> Self {
        control.limits.dense_kernel_rows = control
            .limits
            .dense_kernel_rows
            .min(CONTROLLED_DENSE_KERNEL_MAX_DIMENSION);
        control.limits.dense_kernel_columns = control
            .limits
            .dense_kernel_columns
            .min(CONTROLLED_DENSE_KERNEL_MAX_DIMENSION);
        Self {
            control,
            consumed: OperationWork {
                document_validation_items: 0,
                document_dependency_items: 0,
                document_lowering_items: 0,
                nonlinear_iterations: 0,
                rejected_trials: 0,
                component_linearizations: 0,
                dense_kernel_rows: 0,
                dense_kernel_columns: 0,
                dense_kernel_work_units: 0,
                factorizations: 0,
                rank_kernels: 0,
                diagnostic_candidates: 0,
                diagnostic_trials: 0,
                profile_candidate_pairs: 0,
                profile_subdivisions: 0,
                profile_roots: 0,
                profile_fragments: 0,
                profile_integrations: 0,
                profile_containment_tests: 0,
                profile_faces: 0,
                measurement_integrations: 0,
                measurement_derivative_evaluations: 0,
            },
            stopping_reason: None,
        }
    }

    /// Checks monotonic cancellation at one deterministic safe boundary.
    ///
    /// # Errors
    ///
    /// Returns the first stop reason retained by this controller.
    pub fn checkpoint(
        &mut self,
        checkpoint: OperationCheckpoint,
    ) -> Result<(), OperationStopReason> {
        if let Some(reason) = self.stopping_reason {
            return Err(reason);
        }
        if self.control.token.is_cancelled() {
            let reason = OperationStopReason::Cancelled { checkpoint };
            self.stopping_reason = Some(reason);
            Err(reason)
        } else {
            Ok(())
        }
    }

    /// Charges deterministic work after checking cancellation.
    ///
    /// # Errors
    ///
    /// Returns a typed reason when cancellation was requested, arithmetic
    /// would overflow, or the configured counter limit would be exceeded.
    pub fn charge(
        &mut self,
        counter: OperationWorkCounter,
        amount: usize,
        checkpoint: OperationCheckpoint,
    ) -> Result<(), OperationStopReason> {
        self.checkpoint(checkpoint)?;
        let current = self.consumed.consumed(counter);
        let next = current.checked_add(amount).ok_or_else(|| {
            let reason = OperationStopReason::WorkExhausted {
                counter,
                checkpoint,
            };
            self.stopping_reason = Some(reason);
            reason
        })?;
        if next > self.control.limits.limit(counter) {
            let reason = OperationStopReason::WorkExhausted {
                counter,
                checkpoint,
            };
            self.stopping_reason = Some(reason);
            return Err(reason);
        }
        self.consumed.set(counter, next);
        Ok(())
    }

    pub(crate) fn charged_boundary(
        &mut self,
        counter: OperationWorkCounter,
        amount: usize,
        before: OperationCheckpoint,
        after: OperationCheckpoint,
    ) -> Result<OperationBoundary<'_>, OperationStopReason> {
        self.charge(counter, amount, before)?;
        Ok(OperationBoundary {
            controller: self,
            after,
        })
    }

    pub(crate) fn boundary(
        &mut self,
        before: OperationCheckpoint,
        after: OperationCheckpoint,
    ) -> Result<OperationBoundary<'_>, OperationStopReason> {
        self.checkpoint(before)?;
        Ok(OperationBoundary {
            controller: self,
            after,
        })
    }

    /// Runs optional work against an isolated copy of the remaining operation
    /// budget.
    ///
    /// Each additive counter, including dense-kernel work, receives at most
    /// half its remaining allowance, so mandatory validation after this stage
    /// retains the other half. Dense dimensions remain maximum authorizations.
    /// Consumed work is retained in the parent report. Exhausting the isolated
    /// allowance does not poison an already-valid parent result, while
    /// cancellation remains monotonic and is propagated to the parent.
    pub(crate) fn run_optional<T>(
        &mut self,
        work: impl FnOnce(&mut Self) -> T,
    ) -> OptionalWorkOutcome<T> {
        if self.stopping_reason.is_some() {
            return OptionalWorkOutcome::Interrupted;
        }
        let mut optional_control = self.control.clone();
        optional_control.limits = self.control.limits.optional_ceiling(self.consumed);
        let mut optional = Self {
            control: optional_control,
            consumed: self.consumed,
            stopping_reason: None,
        };
        let value = work(&mut optional);
        let _ = optional.checkpoint(OperationCheckpoint::ComponentBoundary);
        self.consumed = optional.consumed;
        match optional.stopping_reason {
            None => OptionalWorkOutcome::Completed(value),
            Some(reason @ OperationStopReason::Cancelled { .. }) => {
                self.stopping_reason = Some(reason);
                OptionalWorkOutcome::Interrupted
            }
            Some(OperationStopReason::WorkExhausted { .. }) => {
                OptionalWorkOutcome::WorkExhausted(value)
            }
        }
    }

    /// Authorizes one dense kernel against the effective per-axis M35 bound.
    ///
    /// The consumed dimensions retain the largest authorized dimensions rather
    /// than summing dimensions across calls. Every authorization additionally
    /// charges the conservative additive work `max(rows, columns)^3` exactly
    /// once before the caller enters the kernel.
    pub(crate) fn authorize_dense_kernel(
        &mut self,
        rows: usize,
        columns: usize,
        checkpoint: OperationCheckpoint,
    ) -> Result<(), OperationStopReason> {
        self.checkpoint(checkpoint)?;
        self.authorize_maximum(OperationWorkCounter::DenseKernelRows, rows, checkpoint)?;
        self.authorize_maximum(
            OperationWorkCounter::DenseKernelColumns,
            columns,
            checkpoint,
        )?;
        let dimension = rows.max(columns);
        let work_units = dimension
            .checked_mul(dimension)
            .and_then(|square| square.checked_mul(dimension))
            .ok_or_else(|| {
                let reason = OperationStopReason::WorkExhausted {
                    counter: OperationWorkCounter::DenseKernelWorkUnits,
                    checkpoint,
                };
                self.stopping_reason = Some(reason);
                reason
            })?;
        self.charge(
            OperationWorkCounter::DenseKernelWorkUnits,
            work_units,
            checkpoint,
        )
    }

    fn authorize_maximum(
        &mut self,
        counter: OperationWorkCounter,
        amount: usize,
        checkpoint: OperationCheckpoint,
    ) -> Result<(), OperationStopReason> {
        if amount > self.control.limits.limit(counter) {
            let reason = OperationStopReason::WorkExhausted {
                counter,
                checkpoint,
            };
            self.stopping_reason = Some(reason);
            return Err(reason);
        }
        let maximum = self.consumed.consumed(counter).max(amount);
        self.consumed.set(counter, maximum);
        Ok(())
    }

    #[must_use]
    pub const fn report(&self) -> OperationReport {
        OperationReport {
            configured: self.control.limits,
            consumed: self.consumed,
            stopping_reason: self.stopping_reason,
        }
    }

    pub fn outcome<T>(&self, value: T) -> OperationOutcome<T> {
        let report = self.report();
        match self.stopping_reason {
            None => OperationOutcome::Completed { value, report },
            Some(OperationStopReason::Cancelled { .. }) => OperationOutcome::Cancelled { report },
            Some(OperationStopReason::WorkExhausted { .. }) => {
                OperationOutcome::WorkExhausted { report }
            }
        }
    }

    /// Builds an interrupted outcome when no value exists.
    ///
    /// # Panics
    ///
    /// Panics if the controller has not recorded an interruption.
    #[must_use]
    pub fn outcome_unchecked<T>(&self) -> OperationOutcome<T> {
        let report = self.report();
        match self.stopping_reason {
            Some(OperationStopReason::Cancelled { .. }) => OperationOutcome::Cancelled { report },
            Some(OperationStopReason::WorkExhausted { .. }) => {
                OperationOutcome::WorkExhausted { report }
            }
            None => panic!("an interrupted operation outcome requires a stopping reason"),
        }
    }

    #[must_use]
    pub const fn is_stopped(&self) -> bool {
        self.stopping_reason.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_is_monotonic_and_typed() {
        let (handle, token) = cancellation_pair();
        handle.cancel();
        let mut controller =
            OperationController::new(OperationControl::new(token, OperationLimits::unlimited()));
        assert_eq!(
            controller.checkpoint(OperationCheckpoint::DocumentValidation),
            Err(OperationStopReason::Cancelled {
                checkpoint: OperationCheckpoint::DocumentValidation,
            })
        );
        assert!(matches!(
            controller.outcome(()),
            OperationOutcome::Cancelled { .. }
        ));
    }

    #[test]
    fn outcome_mapping_changes_only_completed_values() {
        let completed = OperationController::new(OperationControl::unlimited())
            .outcome(2_u32)
            .map(|value| value + 3);
        assert!(matches!(
            completed,
            OperationOutcome::Completed { value: 5, .. }
        ));

        let (handle, token) = cancellation_pair();
        handle.cancel();
        let mut controller =
            OperationController::new(OperationControl::new(token, OperationLimits::unlimited()));
        controller
            .checkpoint(OperationCheckpoint::DocumentValidation)
            .unwrap_err();
        let cancelled = controller
            .outcome_unchecked::<u32>()
            .map(|_| panic!("cancelled outcome must not invoke the completed-value mapper"));
        assert!(matches!(cancelled, OperationOutcome::Cancelled { .. }));
    }

    #[test]
    fn work_exhaustion_does_not_wrap_or_overconsume() {
        let mut limits = OperationLimits::unlimited();
        limits.nonlinear_iterations = 1;
        let mut controller =
            OperationController::new(OperationControl::new(CancellationToken::default(), limits));
        controller
            .charge(
                OperationWorkCounter::NonlinearIterations,
                1,
                OperationCheckpoint::BeforeNonlinearIteration,
            )
            .unwrap();
        assert!(
            controller
                .charge(
                    OperationWorkCounter::NonlinearIterations,
                    usize::MAX,
                    OperationCheckpoint::BeforeNonlinearIteration,
                )
                .is_err()
        );
        assert_eq!(controller.report().consumed.nonlinear_iterations, 1);
    }

    #[test]
    fn dense_kernel_limits_are_clamped_and_lower_limits_are_honored() {
        let mut limits = OperationLimits::unlimited();
        limits.dense_kernel_rows = usize::MAX;
        limits.dense_kernel_columns = usize::MAX;
        let mut controller =
            OperationController::new(OperationControl::new(CancellationToken::default(), limits));
        assert_eq!(
            controller.report().configured.dense_kernel_rows,
            CONTROLLED_DENSE_KERNEL_MAX_DIMENSION
        );
        assert_eq!(
            controller.report().configured.dense_kernel_columns,
            CONTROLLED_DENSE_KERNEL_MAX_DIMENSION
        );
        controller
            .authorize_dense_kernel(
                CONTROLLED_DENSE_KERNEL_MAX_DIMENSION,
                CONTROLLED_DENSE_KERNEL_MAX_DIMENSION,
                OperationCheckpoint::BeforeFactorization,
            )
            .unwrap();

        limits.dense_kernel_rows = 32;
        limits.dense_kernel_columns = 48;
        let mut controller =
            OperationController::new(OperationControl::new(CancellationToken::default(), limits));
        assert_eq!(
            controller.authorize_dense_kernel(33, 48, OperationCheckpoint::BeforeRankKernel),
            Err(OperationStopReason::WorkExhausted {
                counter: OperationWorkCounter::DenseKernelRows,
                checkpoint: OperationCheckpoint::BeforeRankKernel,
            })
        );
    }

    #[test]
    fn dense_kernel_work_is_additive_and_uses_the_largest_rectangular_dimension() {
        let mut limits = OperationLimits::unlimited();
        limits.dense_kernel_work_units = 5 * 5 * 5 + 4 * 4 * 4;
        let mut controller =
            OperationController::new(OperationControl::new(CancellationToken::default(), limits));

        controller
            .authorize_dense_kernel(2, 5, OperationCheckpoint::BeforeFactorization)
            .unwrap();
        controller
            .authorize_dense_kernel(4, 3, OperationCheckpoint::BeforeRankKernel)
            .unwrap();

        let report = controller.report();
        assert_eq!(report.consumed.dense_kernel_rows, 4);
        assert_eq!(report.consumed.dense_kernel_columns, 5);
        assert_eq!(report.consumed.dense_kernel_work_units, 125 + 64);
        assert_eq!(report.consumed.factorizations, 0);
        assert_eq!(report.consumed.rank_kernels, 0);

        controller
            .charge(
                OperationWorkCounter::Factorizations,
                1,
                OperationCheckpoint::BeforeFactorization,
            )
            .unwrap();
        controller
            .charge(
                OperationWorkCounter::RankKernels,
                1,
                OperationCheckpoint::BeforeRankKernel,
            )
            .unwrap();
        let report = controller.report();
        assert_eq!(report.consumed.dense_kernel_work_units, 125 + 64);
        assert_eq!(report.consumed.factorizations, 1);
        assert_eq!(report.consumed.rank_kernels, 1);
    }

    #[test]
    fn dense_kernel_work_honors_exact_full_dimension_operation_boundaries() {
        const FULL_DIMENSION_WORK: usize = 16_777_216;
        assert_eq!(
            FULL_DIMENSION_WORK,
            CONTROLLED_DENSE_KERNEL_MAX_DIMENSION
                * CONTROLLED_DENSE_KERNEL_MAX_DIMENSION
                * CONTROLLED_DENSE_KERNEL_MAX_DIMENSION
        );

        for operation_limit in 1..=3 {
            let mut limits = OperationLimits::unlimited();
            limits.dense_kernel_work_units = FULL_DIMENSION_WORK * operation_limit;
            let mut controller = OperationController::new(OperationControl::new(
                CancellationToken::default(),
                limits,
            ));

            for completed in 1..=operation_limit {
                controller
                    .authorize_dense_kernel(
                        CONTROLLED_DENSE_KERNEL_MAX_DIMENSION,
                        CONTROLLED_DENSE_KERNEL_MAX_DIMENSION,
                        OperationCheckpoint::BeforeFactorization,
                    )
                    .unwrap();
                assert_eq!(
                    controller.report().consumed.dense_kernel_work_units,
                    FULL_DIMENSION_WORK * completed
                );
            }

            assert_eq!(
                controller.authorize_dense_kernel(
                    CONTROLLED_DENSE_KERNEL_MAX_DIMENSION,
                    CONTROLLED_DENSE_KERNEL_MAX_DIMENSION,
                    OperationCheckpoint::BeforeFactorization,
                ),
                Err(OperationStopReason::WorkExhausted {
                    counter: OperationWorkCounter::DenseKernelWorkUnits,
                    checkpoint: OperationCheckpoint::BeforeFactorization,
                })
            );
            let report = controller.report();
            assert_eq!(
                report.consumed.dense_kernel_work_units,
                FULL_DIMENSION_WORK * operation_limit
            );
            assert_eq!(report.consumed.factorizations, 0);
            assert_eq!(report.consumed.rank_kernels, 0);
        }
    }

    #[test]
    fn dense_kernel_work_overflow_stops_before_kernel_counters_advance() {
        let mut controller = OperationController::new(OperationControl::unlimited());
        // Bypass the public controlled-axis clamp to exercise the checked cube
        // defensively on dimensions no valid controller can ordinarily admit.
        controller.control.limits.dense_kernel_rows = usize::MAX;
        controller.control.limits.dense_kernel_columns = usize::MAX;

        assert_eq!(
            controller.authorize_dense_kernel(
                usize::MAX,
                1,
                OperationCheckpoint::BeforeFactorization,
            ),
            Err(OperationStopReason::WorkExhausted {
                counter: OperationWorkCounter::DenseKernelWorkUnits,
                checkpoint: OperationCheckpoint::BeforeFactorization,
            })
        );
        let report = controller.report();
        assert_eq!(report.consumed.dense_kernel_rows, usize::MAX);
        assert_eq!(report.consumed.dense_kernel_columns, 1);
        assert_eq!(report.consumed.dense_kernel_work_units, 0);
        assert_eq!(report.consumed.factorizations, 0);
        assert_eq!(report.consumed.rank_kernels, 0);

        let mut controller = OperationController::new(OperationControl::unlimited());
        controller
            .charge(
                OperationWorkCounter::DenseKernelWorkUnits,
                usize::MAX - 7,
                OperationCheckpoint::BeforeFactorization,
            )
            .unwrap();
        assert_eq!(
            controller.authorize_dense_kernel(2, 2, OperationCheckpoint::BeforeFactorization),
            Err(OperationStopReason::WorkExhausted {
                counter: OperationWorkCounter::DenseKernelWorkUnits,
                checkpoint: OperationCheckpoint::BeforeFactorization,
            })
        );
        let report = controller.report();
        assert_eq!(report.consumed.dense_kernel_work_units, usize::MAX - 7);
        assert_eq!(report.consumed.factorizations, 0);
        assert_eq!(report.consumed.rank_kernels, 0);
    }

    #[test]
    fn optional_dense_kernel_work_uses_half_the_remaining_additive_allowance() {
        let mut limits = OperationLimits::unlimited();
        limits.dense_kernel_work_units = 24;
        let mut controller =
            OperationController::new(OperationControl::new(CancellationToken::default(), limits));
        controller
            .authorize_dense_kernel(2, 2, OperationCheckpoint::BeforeFactorization)
            .unwrap();

        let outcome = controller.run_optional(|optional| {
            assert_eq!(optional.report().configured.dense_kernel_work_units, 16);
            optional
                .authorize_dense_kernel(2, 2, OperationCheckpoint::BeforeFactorization)
                .unwrap();
            optional
                .authorize_dense_kernel(1, 1, OperationCheckpoint::BeforeFactorization)
                .unwrap_err();
        });

        assert!(matches!(outcome, OptionalWorkOutcome::WorkExhausted(())));
        assert!(!controller.is_stopped());
        assert_eq!(controller.report().consumed.dense_kernel_work_units, 16);
        controller
            .authorize_dense_kernel(2, 2, OperationCheckpoint::BeforeFactorization)
            .unwrap();
        assert_eq!(controller.report().consumed.dense_kernel_work_units, 24);
    }

    #[test]
    fn scoped_boundaries_observe_cancellation_at_after_checkpoints() {
        for (before, after) in [
            (
                OperationCheckpoint::BeforeNonlinearIteration,
                OperationCheckpoint::AfterNonlinearIteration,
            ),
            (
                OperationCheckpoint::BeforeTrialBoundary,
                OperationCheckpoint::AfterTrialBoundary,
            ),
        ] {
            let (handle, token) = cancellation_pair();
            let mut controller = OperationController::new(OperationControl::new(
                token,
                OperationLimits::unlimited(),
            ));
            {
                let _boundary = controller.boundary(before, after).unwrap();
                handle.cancel();
            }
            assert_eq!(
                controller.report().stopping_reason,
                Some(OperationStopReason::Cancelled { checkpoint: after })
            );
        }
    }

    #[test]
    fn optional_work_exhaustion_retains_accounting_without_stopping_the_parent() {
        let mut limits = OperationLimits::unlimited();
        limits.factorizations = 3;
        let mut controller =
            OperationController::new(OperationControl::new(CancellationToken::default(), limits));
        controller
            .charge(
                OperationWorkCounter::Factorizations,
                1,
                OperationCheckpoint::BeforeFactorization,
            )
            .unwrap();

        let outcome = controller.run_optional(|optional| {
            optional
                .charge(
                    OperationWorkCounter::Factorizations,
                    1,
                    OperationCheckpoint::BeforeFactorization,
                )
                .unwrap();
            optional
                .charge(
                    OperationWorkCounter::Factorizations,
                    1,
                    OperationCheckpoint::BeforeFactorization,
                )
                .unwrap_err();
            7
        });

        assert!(matches!(outcome, OptionalWorkOutcome::WorkExhausted(7)));
        assert!(!controller.is_stopped());
        assert_eq!(controller.report().consumed.factorizations, 2);
        assert_eq!(controller.report().stopping_reason, None);
        controller
            .charge(
                OperationWorkCounter::Factorizations,
                1,
                OperationCheckpoint::BeforeFactorization,
            )
            .unwrap();
        assert_eq!(controller.report().consumed.factorizations, 3);
    }

    #[test]
    fn optional_work_propagates_cancellation_to_the_parent() {
        let (handle, token) = cancellation_pair();
        let mut controller =
            OperationController::new(OperationControl::new(token, OperationLimits::unlimited()));

        let outcome = controller.run_optional(|_| {
            handle.cancel();
        });

        assert!(matches!(outcome, OptionalWorkOutcome::Interrupted));
        assert_eq!(
            controller.report().stopping_reason,
            Some(OperationStopReason::Cancelled {
                checkpoint: OperationCheckpoint::ComponentBoundary,
            })
        );
    }
}
