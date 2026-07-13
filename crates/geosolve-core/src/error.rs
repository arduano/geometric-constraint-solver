use thiserror::Error;

use crate::{ResidualId, SourceConstraintId, VariableId, VariableKind};

/// Construction and evaluation failures detected before numerical solving.
#[derive(Clone, Debug, Error, PartialEq)]
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
    #[error("finite-difference step must be positive and finite, got {0}")]
    InvalidFiniteDifferenceStep(f64),
}
