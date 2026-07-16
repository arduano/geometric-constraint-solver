use std::fmt::Debug;

use thiserror::Error;

use crate::variable::validate_scales;
use crate::{CoreError, SourceConstraintId, VariableId, VariableKind, VariableValue};

/// Priority class retained separately from nonlinear equation scaling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResidualCategory {
    Hard,
    Temporary,
    Preference,
}

/// Human-readable metadata for one high-level source constraint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceConstraint {
    label: String,
}

impl SourceConstraint {
    /// Creates source-level audit metadata.
    ///
    /// # Errors
    ///
    /// Returns an error if the label is empty.
    pub fn new(label: impl Into<String>) -> Result<Self, CoreError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(CoreError::EmptyAuditMetadata {
                field: "source label",
            });
        }
        Ok(Self { label })
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// One static named feature/reference binding used by a readable equation row.
/// Evaluated variable values are reported separately in audit snapshots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditBinding {
    pub name: String,
    pub value: String,
}

impl AuditBinding {
    #[must_use]
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

/// Static readable metadata paired with one executable scalar residual row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidualRowAudit {
    pub template: String,
    pub bindings: Vec<AuditBinding>,
    pub unit: String,
}

impl ResidualRowAudit {
    #[must_use]
    pub fn new(
        template: impl Into<String>,
        bindings: Vec<AuditBinding>,
        unit: impl Into<String>,
    ) -> Self {
        Self {
            template: template.into(),
            bindings,
            unit: unit.into(),
        }
    }
}

/// Machine-readable classification for geometry evaluation failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum EvaluationErrorCategory {
    /// A required geometric direction, radius, or local feature has collapsed.
    Degenerate,
    /// A finite state lies outside the evaluator's declared parameter domain.
    OutOfDomain,
    /// The residual value exists but its derivative is undefined at this state.
    Nondifferentiable,
    /// The evaluator cannot select one result without an explicit discrete choice.
    Ambiguous,
}

/// A domain evaluator can explicitly reject an invalid geometric state.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum EvaluationError {
    /// Unclassified invalid geometry retained for legacy evaluators and programmer contracts.
    #[error("{0}")]
    InvalidGeometry(String),
    /// A semantic geometry failure with a stable machine-readable category.
    #[error("{category:?}: {message}")]
    Categorized {
        /// Machine-readable failure class.
        category: EvaluationErrorCategory,
        /// Human-readable evaluator context.
        message: String,
    },
}

impl EvaluationError {
    #[must_use]
    pub fn invalid_geometry(message: impl Into<String>) -> Self {
        Self::InvalidGeometry(message.into())
    }

    #[must_use]
    pub fn degenerate(message: impl Into<String>) -> Self {
        Self::categorized(EvaluationErrorCategory::Degenerate, message)
    }

    #[must_use]
    pub fn out_of_domain(message: impl Into<String>) -> Self {
        Self::categorized(EvaluationErrorCategory::OutOfDomain, message)
    }

    #[must_use]
    pub fn nondifferentiable(message: impl Into<String>) -> Self {
        Self::categorized(EvaluationErrorCategory::Nondifferentiable, message)
    }

    #[must_use]
    pub fn ambiguous(message: impl Into<String>) -> Self {
        Self::categorized(EvaluationErrorCategory::Ambiguous, message)
    }

    #[must_use]
    pub const fn category(&self) -> Option<EvaluationErrorCategory> {
        match self {
            Self::Categorized { category, .. } => Some(*category),
            Self::InvalidGeometry(_) => None,
        }
    }

    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::InvalidGeometry(message) | Self::Categorized { message, .. } => message,
        }
    }

    fn categorized(category: EvaluationErrorCategory, message: impl Into<String>) -> Self {
        Self::Categorized {
            category,
            message: message.into(),
        }
    }
}

/// Row-major derivative block for one incident variable.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalJacobian {
    rows: usize,
    columns: usize,
    values: Vec<f64>,
}

impl LocalJacobian {
    #[must_use]
    pub fn new(rows: usize, columns: usize, values: Vec<f64>) -> Self {
        Self {
            rows,
            columns,
            values,
        }
    }

    #[must_use]
    pub const fn rows(&self) -> usize {
        self.rows
    }

    #[must_use]
    pub const fn columns(&self) -> usize {
        self.columns
    }

    #[must_use]
    pub fn values(&self) -> &[f64] {
        &self.values
    }
}

/// Caller-owned row-major storage for one incident variable's raw Jacobian block.
#[derive(Debug)]
pub struct LocalJacobianStorage<'a> {
    rows: usize,
    columns: usize,
    step_scales: &'a [f64],
    values: &'a mut [f64],
}

impl<'a> LocalJacobianStorage<'a> {
    pub(crate) fn new(
        rows: usize,
        columns: usize,
        step_scales: &'a [f64],
        values: &'a mut [f64],
    ) -> Self {
        Self {
            rows,
            columns,
            step_scales,
            values,
        }
    }

    #[must_use]
    /// Returns the scalar residual-row count.
    pub const fn rows(&self) -> usize {
        self.rows
    }

    #[must_use]
    /// Returns the incident variable's local tangent-column count.
    pub const fn columns(&self) -> usize {
        self.columns
    }

    #[must_use]
    /// Returns characteristic scales for converting normalized increments to raw tangents.
    pub const fn step_scales(&self) -> &[f64] {
        self.step_scales
    }

    #[must_use]
    /// Returns the current row-major raw-tangent derivative slots.
    pub fn values(&self) -> &[f64] {
        self.values
    }

    /// Returns mutable row-major raw-tangent derivative slots.
    pub fn values_mut(&mut self) -> &mut [f64] {
        self.values
    }
}

/// Caller-owned fused raw residual and local-Jacobian output storage.
///
/// Jacobian blocks are presented in declared residual incidence order.
#[derive(Debug)]
pub struct LinearizationStorage<'a, 'b> {
    residuals: &'a mut [f64],
    jacobian_blocks: &'b mut [LocalJacobianStorage<'a>],
    jacobian_coordinates: JacobianCoordinates,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum JacobianCoordinates {
    RawTangent,
    NormalizedTangent,
}

impl<'a, 'b> LinearizationStorage<'a, 'b> {
    pub(crate) fn new(
        residuals: &'a mut [f64],
        jacobian_blocks: &'b mut [LocalJacobianStorage<'a>],
    ) -> Self {
        Self {
            residuals,
            jacobian_blocks,
            jacobian_coordinates: JacobianCoordinates::RawTangent,
        }
    }

    #[must_use]
    /// Returns the current raw residual-value slots.
    pub fn residuals(&self) -> &[f64] {
        self.residuals
    }

    /// Returns mutable raw residual-value slots.
    pub fn residuals_mut(&mut self) -> &mut [f64] {
        self.residuals
    }

    #[must_use]
    /// Returns the number of declared incident Jacobian blocks.
    pub fn jacobian_block_count(&self) -> usize {
        self.jacobian_blocks.len()
    }

    #[must_use]
    /// Returns one incident raw-tangent Jacobian block in declaration order.
    pub fn jacobian_block(&self, index: usize) -> Option<&LocalJacobianStorage<'a>> {
        self.jacobian_blocks.get(index)
    }

    /// Returns one mutable incident raw-tangent Jacobian block in declaration order.
    pub fn jacobian_block_mut(&mut self, index: usize) -> Option<&mut LocalJacobianStorage<'a>> {
        self.jacobian_blocks.get_mut(index)
    }

    pub(crate) const fn jacobian_coordinates(&self) -> JacobianCoordinates {
        self.jacobian_coordinates
    }

    pub(crate) fn mark_normalized_tangent_jacobians(&mut self) {
        self.jacobian_coordinates = JacobianCoordinates::NormalizedTangent;
    }
}

/// Object-safe cloning support for [`ResidualEvaluator`].
///
/// This is public only because it is a supertrait of the pre-1.0 evaluator
/// seam. Implementors receive it automatically when they implement `Clone`.
#[doc(hidden)]
pub trait ResidualEvaluatorClone {
    fn clone_box(&self) -> Box<dyn ResidualEvaluator>;
}

impl<T> ResidualEvaluatorClone for T
where
    T: ResidualEvaluator + Clone + 'static,
{
    fn clone_box(&self) -> Box<dyn ResidualEvaluator> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn ResidualEvaluator> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Executable residual and local analytic derivatives.
///
/// This trait is an unstable pre-1.0 extension seam. Implementations must be
/// behavior-pure: calls with the same variables must not depend on or mutate
/// shared behavior-affecting state. Interior mutability may be used only for
/// telemetry that cannot change later evaluation results. `Clone` creates a
/// distinct trait object, but Rust cannot infer whether fields such as
/// `Arc<Mutex<_>>` are deep snapshots, so they must not control equations.
pub trait ResidualEvaluator: ResidualEvaluatorClone + Debug + Send + Sync {
    /// Evaluates raw residual values in row order.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationError`] when the supplied state is invalid geometry.
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError>;

    /// Returns one row-major block per variable, in incidence order.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationError`] when a derivative is undefined for the
    /// supplied geometry.
    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError>;

    /// Writes raw residuals and one raw row-major Jacobian block per incident
    /// variable into pre-sized caller storage.
    ///
    /// The default returns `None`, which asks core dispatch to use
    /// [`Self::evaluate`] and [`Self::jacobian`]. Implementations return
    /// `Some(Ok(()))` after overwriting every slot, or `Some(Err(_))` when the
    /// supplied state cannot be linearized.
    ///
    /// # Errors
    ///
    /// Returns [`EvaluationError`] when the supplied state cannot be
    /// linearized. Every output slot must be overwritten on success.
    fn linearize(
        &self,
        _variables: &[VariableValue],
        _storage: &mut LinearizationStorage<'_, '_>,
    ) -> Option<Result<(), EvaluationError>> {
        None
    }
}

/// One residual block, including executable equations and audit metadata.
#[derive(Clone, Debug)]
pub struct ResidualBlock {
    source: SourceConstraintId,
    category: ResidualCategory,
    incident_variables: Vec<VariableId>,
    output_dimension: usize,
    scales: Vec<f64>,
    audit_rows: Vec<ResidualRowAudit>,
    evaluator: Box<dyn ResidualEvaluator>,
    exact_elimination: Option<ExactElimination>,
}

/// Trusted elimination meaning attached only by core-owned residual constructors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum ExactElimination {
    Fixed {
        variable_id: VariableId,
        value: VariableValue,
    },
    Alias {
        alias: VariableId,
        representative: VariableId,
        kind: VariableKind,
    },
}

impl ResidualBlock {
    /// Creates an executable residual block and its row audit descriptors.
    ///
    /// # Errors
    ///
    /// Returns an error for zero or inconsistent dimensions, invalid scales,
    /// or incomplete audit metadata. IDs are validated when the block is added
    /// to a [`crate::Problem`].
    #[allow(clippy::too_many_arguments)]
    pub fn new<E>(
        source: SourceConstraintId,
        category: ResidualCategory,
        incident_variables: Vec<VariableId>,
        output_dimension: usize,
        scales: Vec<f64>,
        audit_rows: Vec<ResidualRowAudit>,
        evaluator: E,
    ) -> Result<Self, CoreError>
    where
        E: ResidualEvaluator + 'static,
    {
        if output_dimension == 0 {
            return Err(CoreError::EmptyDimension {
                context: "residual output",
            });
        }
        if scales.len() != output_dimension {
            return Err(CoreError::DimensionMismatch {
                context: "residual scales",
                expected: output_dimension,
                actual: scales.len(),
            });
        }
        validate_scales(&scales, "residual")?;
        if audit_rows.len() != output_dimension {
            return Err(CoreError::DimensionMismatch {
                context: "residual audit rows",
                expected: output_dimension,
                actual: audit_rows.len(),
            });
        }
        for row in &audit_rows {
            validate_audit_row(row)?;
        }

        Ok(Self {
            source,
            category,
            incident_variables,
            output_dimension,
            scales,
            audit_rows,
            evaluator: Box::new(evaluator),
            exact_elimination: None,
        })
    }

    /// Creates a trusted exact residual fixing an entire variable block.
    ///
    /// # Errors
    ///
    /// Returns an error for inconsistent dimensions, invalid scales, non-finite
    /// fixed data, or incomplete audit metadata.
    pub fn fixed_variable(
        source: SourceConstraintId,
        variable_id: VariableId,
        value: VariableValue,
        scales: Vec<f64>,
        audit_rows: Vec<ResidualRowAudit>,
    ) -> Result<Self, CoreError> {
        value.validate_finite()?;
        let dimension = value.kind().tangent_dimension();
        let mut residual = Self::new(
            source,
            ResidualCategory::Hard,
            vec![variable_id],
            dimension,
            scales,
            audit_rows,
            FixedVariableEvaluator { value },
        )?;
        residual.exact_elimination = Some(ExactElimination::Fixed { variable_id, value });
        Ok(residual)
    }

    /// Creates a trusted exact residual enforcing `alias == representative`.
    ///
    /// # Errors
    ///
    /// Returns an error for inconsistent dimensions, invalid scales, or
    /// incomplete audit metadata.
    pub fn exact_alias(
        source: SourceConstraintId,
        alias: VariableId,
        representative: VariableId,
        kind: VariableKind,
        scales: Vec<f64>,
        audit_rows: Vec<ResidualRowAudit>,
    ) -> Result<Self, CoreError> {
        let dimension = kind.tangent_dimension();
        let mut residual = Self::new(
            source,
            ResidualCategory::Hard,
            vec![alias, representative],
            dimension,
            scales,
            audit_rows,
            ExactAliasEvaluator { kind },
        )?;
        residual.exact_elimination = Some(ExactElimination::Alias {
            alias,
            representative,
            kind,
        });
        Ok(residual)
    }

    #[must_use]
    pub const fn source(&self) -> SourceConstraintId {
        self.source
    }

    #[must_use]
    pub const fn category(&self) -> ResidualCategory {
        self.category
    }

    #[must_use]
    pub fn incident_variables(&self) -> &[VariableId] {
        &self.incident_variables
    }

    #[must_use]
    pub const fn output_dimension(&self) -> usize {
        self.output_dimension
    }

    #[must_use]
    pub fn scales(&self) -> &[f64] {
        &self.scales
    }

    #[must_use]
    pub fn audit_rows(&self) -> &[ResidualRowAudit] {
        &self.audit_rows
    }

    pub(crate) fn replace_audit_rows(
        &mut self,
        audit_rows: Vec<ResidualRowAudit>,
    ) -> Result<(), CoreError> {
        if audit_rows.len() != self.output_dimension {
            return Err(CoreError::DimensionMismatch {
                context: "residual audit rows",
                expected: self.output_dimension,
                actual: audit_rows.len(),
            });
        }
        for row in &audit_rows {
            validate_audit_row(row)?;
        }
        self.audit_rows = audit_rows;
        Ok(())
    }

    pub(crate) fn evaluator(&self) -> &dyn ResidualEvaluator {
        self.evaluator.as_ref()
    }

    /// Remaps IDs on a freshly constructed block for a stable-identity
    /// compatible replacement. Domain compilers use this to rebuild one source
    /// without rebuilding the containing problem topology.
    #[doc(hidden)]
    pub fn remap_for_compatible_replacement(
        mut self,
        source: SourceConstraintId,
        variables: &[(VariableId, VariableId)],
    ) -> Result<Self, CoreError> {
        self.source = source;
        for variable in &mut self.incident_variables {
            if let Some((_, replacement)) =
                variables.iter().find(|(current, _)| current == variable)
            {
                *variable = *replacement;
            }
        }
        for (index, variable) in self.incident_variables.iter().enumerate() {
            if self.incident_variables[..index].contains(variable) {
                return Err(CoreError::DuplicateIncidentVariable(*variable));
            }
        }
        Ok(self)
    }

    pub(crate) const fn exact_elimination(&self) -> Option<ExactElimination> {
        self.exact_elimination
    }

    pub(crate) fn structurally_compatible_with(&self, replacement: &Self) -> Option<&'static str> {
        if self.source != replacement.source {
            return Some("source");
        }
        if self.category != replacement.category {
            return Some("category");
        }
        if self.incident_variables != replacement.incident_variables {
            return Some("incidence");
        }
        if self.output_dimension != replacement.output_dimension {
            return Some("output dimension");
        }
        if !same_elimination_shape(self.exact_elimination, replacement.exact_elimination) {
            return Some("elimination shape");
        }
        None
    }
}

fn same_elimination_shape(
    first: Option<ExactElimination>,
    second: Option<ExactElimination>,
) -> bool {
    match (first, second) {
        (None, None) => true,
        (
            Some(ExactElimination::Fixed {
                variable_id: first, ..
            }),
            Some(ExactElimination::Fixed {
                variable_id: second,
                ..
            }),
        ) => first == second,
        (
            Some(ExactElimination::Alias {
                alias: first_alias,
                representative: first_representative,
                kind: first_kind,
            }),
            Some(ExactElimination::Alias {
                alias: second_alias,
                representative: second_representative,
                kind: second_kind,
            }),
        ) => {
            first_alias == second_alias
                && first_representative == second_representative
                && first_kind == second_kind
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug)]
struct FixedVariableEvaluator {
    value: VariableValue,
}

impl ResidualEvaluator for FixedVariableEvaluator {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [value] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "fixed residual expected one variable",
            ));
        };
        if value.kind() != self.value.kind() {
            return Err(EvaluationError::invalid_geometry(
                "fixed residual variable kind changed",
            ));
        }
        Ok(value
            .ambient_values()
            .iter()
            .zip(self.value.ambient_values())
            .map(|(actual, expected)| actual - expected)
            .collect())
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [value] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "fixed residual expected one variable",
            ));
        };
        if value.kind() != self.value.kind() {
            return Err(EvaluationError::invalid_geometry(
                "fixed residual variable kind changed",
            ));
        }
        Ok(vec![identity_jacobian(value.kind(), 1.0)])
    }
}

#[derive(Clone, Copy, Debug)]
struct ExactAliasEvaluator {
    kind: VariableKind,
}

impl ResidualEvaluator for ExactAliasEvaluator {
    fn evaluate(&self, variables: &[VariableValue]) -> Result<Vec<f64>, EvaluationError> {
        let [alias, representative] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "alias residual expected two variables",
            ));
        };
        if alias.kind() != self.kind || representative.kind() != self.kind {
            return Err(EvaluationError::invalid_geometry(
                "alias residual variable kind changed",
            ));
        }
        Ok(alias
            .ambient_values()
            .iter()
            .zip(representative.ambient_values())
            .map(|(alias, representative)| alias - representative)
            .collect())
    }

    fn jacobian(&self, variables: &[VariableValue]) -> Result<Vec<LocalJacobian>, EvaluationError> {
        let [alias, representative] = variables else {
            return Err(EvaluationError::invalid_geometry(
                "alias residual expected two variables",
            ));
        };
        if alias.kind() != self.kind || representative.kind() != self.kind {
            return Err(EvaluationError::invalid_geometry(
                "alias residual variable kind changed",
            ));
        }
        Ok(vec![
            identity_jacobian(self.kind, 1.0),
            identity_jacobian(self.kind, -1.0),
        ])
    }
}

fn identity_jacobian(kind: VariableKind, sign: f64) -> LocalJacobian {
    let dimension = kind.tangent_dimension();
    let mut values = vec![0.0; dimension * dimension];
    for coordinate in 0..dimension {
        values[coordinate * dimension + coordinate] = sign;
    }
    LocalJacobian::new(dimension, dimension, values)
}

fn validate_audit_row(row: &ResidualRowAudit) -> Result<(), CoreError> {
    if row.template.trim().is_empty() {
        return Err(CoreError::EmptyAuditMetadata {
            field: "row template",
        });
    }
    if row.unit.trim().is_empty() {
        return Err(CoreError::EmptyAuditMetadata { field: "row unit" });
    }
    if row.bindings.is_empty() {
        return Err(CoreError::EmptyAuditMetadata {
            field: "row bindings",
        });
    }
    for binding in &row.bindings {
        if binding.name.trim().is_empty() {
            return Err(CoreError::EmptyAuditMetadata {
                field: "binding name",
            });
        }
        if binding.value.trim().is_empty() {
            return Err(CoreError::EmptyAuditMetadata {
                field: "binding value",
            });
        }
    }
    Ok(())
}
