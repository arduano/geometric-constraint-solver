use std::fmt::Debug;

use thiserror::Error;

use crate::variable::validate_scales;
use crate::{CoreError, SourceConstraintId, VariableId, VariableValue};

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

/// One named value or feature reference used by a readable equation row.
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
        })
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
