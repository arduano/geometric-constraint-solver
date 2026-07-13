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

/// A domain evaluator can explicitly reject a degenerate geometric state.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum EvaluationError {
    #[error("{0}")]
    InvalidGeometry(String),
}

impl EvaluationError {
    #[must_use]
    pub fn invalid_geometry(message: impl Into<String>) -> Self {
        Self::InvalidGeometry(message.into())
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

/// Executable residual and local analytic derivatives.
pub trait ResidualEvaluator: Debug + Send + Sync {
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
}

/// One residual block, including executable equations and audit metadata.
#[derive(Debug)]
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

    pub(crate) fn evaluator(&self) -> &dyn ResidualEvaluator {
        self.evaluator.as_ref()
    }

    pub(crate) const fn exact_elimination(&self) -> Option<ExactElimination> {
        self.exact_elimination
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
