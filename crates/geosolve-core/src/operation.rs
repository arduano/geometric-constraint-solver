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

    /// Authorizes one dense kernel against the effective per-axis M35 bound.
    ///
    /// The consumed values retain the largest authorized dimensions rather than
    /// summing dimensions across kernel calls.
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
}
