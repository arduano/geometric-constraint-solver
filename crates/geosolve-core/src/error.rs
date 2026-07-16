use thiserror::Error;

use crate::{
    BoundId, EvaluationErrorCategory, ResidualId, SourceConstraintId, VariableId, VariableKind,
};

/// Typed input or numerical failures from an accepted-state sensitivity solve.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum SensitivityError {
    #[error("normalized residual rate has dimension {actual}, expected {expected}")]
    DimensionMismatch { expected: usize, actual: usize },
    #[error("normalized residual rate {index} must be finite, got {value}")]
    NonFiniteRightHandSide { index: usize, value: f64 },
    #[error("accepted-state sensitivity solve failed: {context}")]
    NumericalFailure { context: &'static str },
}

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
    #[error("unknown bound ID {0:?}")]
    UnknownBound(BoundId),
    #[error("variable {0:?} is still referenced by a residual block")]
    VariableInUse(VariableId),
    #[error("source constraint {0:?} is still referenced by a residual block")]
    SourceInUse(SourceConstraintId),
    #[error("residual incidence contains variable {0:?} more than once")]
    DuplicateIncidentVariable(VariableId),
    #[error("variable {variable:?} tangent coordinate {coordinate} is already bounded")]
    DuplicateBoundCoordinate {
        variable: VariableId,
        coordinate: usize,
    },
    #[error(
        "bound coordinate {coordinate} is outside variable {variable:?}'s tangent dimension {dimension}"
    )]
    InvalidBoundCoordinate {
        variable: VariableId,
        coordinate: usize,
        dimension: usize,
    },
    #[error("bound {side} value must be finite, got {value}")]
    InvalidBoundValue { side: &'static str, value: f64 },
    #[error("bound lower value {lower} exceeds upper value {upper}")]
    InvalidBoundInterval { lower: f64, upper: f64 },
    #[error("a coordinate bound must provide a lower or upper value")]
    EmptyBound,
    #[error("bound label must not be empty")]
    EmptyBoundLabel,
    #[error(
        "variable {variable:?} coordinate {coordinate} value {value} is outside [{lower:?}, {upper:?}]"
    )]
    ValueOutsideBound {
        variable: VariableId,
        coordinate: usize,
        value: f64,
        lower: Option<f64>,
        upper: Option<f64>,
    },
    #[error("variable kind is {actual:?}, expected {expected:?}")]
    VariableKindMismatch {
        expected: VariableKind,
        actual: VariableKind,
    },
    #[error("invalid {kind:?} variable value: {message}")]
    InvalidVariableValue { kind: VariableKind, message: String },
    #[error("coordinate bounds do not support {kind:?} variable {variable:?}")]
    UnsupportedBoundVariableKind {
        variable: VariableId,
        kind: VariableKind,
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
    #[error("replacement for residual {residual:?} changes structural field {field}")]
    IncompatibleResidualReplacement {
        residual: ResidualId,
        field: &'static str,
    },
    #[error("accepted hard linearization validation failed: {context}")]
    InvalidAcceptedLinearization { context: &'static str },
    #[error("compatible session replacement changed reduced component ordering: {context}")]
    IncompatibleSessionPlan { context: &'static str },
}
