use thiserror::Error;

use crate::{EvaluationErrorCategory, ResidualId, SourceConstraintId, VariableId, VariableKind};

/// Construction and evaluation failures detected before numerical solving.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CoreError {
    #[error("{context} dimension must be positive")]
    EmptyDimension { context: &'static str },
    #[error("{context} has dimension {actual}, expected {expected}")]
    DimensionMismatch {
        context: &'static str,
        expected: usize,
        actual: usize,
    },
    #[error("{context} dimensions overflow addressable storage")]
    DimensionOverflow { context: &'static str },
    #[error("{context} scale {index} must be positive and finite, got {value}")]
    InvalidScale {
        context: &'static str,
        index: usize,
        value: f64,
    },
    #[error("{context} value {index} must be finite, got {value}")]
    NonFiniteValue {
        context: &'static str,
        index: usize,
        value: f64,
    },
    #[error("unknown variable ID {0:?}")]
    UnknownVariable(VariableId),
    #[error("unknown residual ID {0:?}")]
    UnknownResidual(ResidualId),
    #[error("unknown source constraint ID {0:?}")]
    UnknownSource(SourceConstraintId),
    #[error("variable {0:?} is still referenced by a residual block")]
    VariableInUse(VariableId),
    #[error("source constraint {0:?} is still referenced by a residual block")]
    SourceInUse(SourceConstraintId),
    #[error("residual incidence contains variable {0:?} more than once")]
    DuplicateIncidentVariable(VariableId),
    #[error("variable kind is {actual:?}, expected {expected:?}")]
    VariableKindMismatch {
        expected: VariableKind,
        actual: VariableKind,
    },
    #[error("audit metadata field {field} must not be empty")]
    EmptyAuditMetadata { field: &'static str },
    #[error("residual {residual:?} reported invalid geometry: {message}")]
    InvalidGeometry {
        residual: ResidualId,
        message: String,
    },
    #[error("residual {residual:?} reported {category:?}: {message}")]
    /// A residual reported a machine-readable semantic geometry failure.
    CategorizedEvaluation {
        /// Residual block that rejected evaluation.
        residual: ResidualId,
        /// Stable semantic failure category.
        category: EvaluationErrorCategory,
        /// Human-readable evaluator context.
        message: String,
    },
    #[error("finite-difference step must be positive and finite, got {0}")]
    InvalidFiniteDifferenceStep(f64),
    #[error("invalid solver configuration field {field}: {message}")]
    InvalidSolverConfig {
        field: &'static str,
        message: &'static str,
    },
    #[error("residual {residual:?} is not a valid {declaration} elimination row: {message}")]
    InvalidEliminationResidual {
        residual: ResidualId,
        declaration: &'static str,
        message: &'static str,
    },
    #[error("variable {variable:?} has conflicting elimination declarations: {message}")]
    ConflictingElimination {
        variable: VariableId,
        message: &'static str,
    },
    #[error("exact aliases {alias:?} and {representative:?} have different step scales")]
    AliasScaleMismatch {
        alias: VariableId,
        representative: VariableId,
    },
    #[error("exact alias declaration for {variable:?} creates a cycle")]
    AliasCycle { variable: VariableId },
}
