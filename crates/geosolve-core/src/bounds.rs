use crate::{BoundId, CoreError, Problem, VariableId, VariableKind, VariableValue};

/// A finite optional lower/upper box bound on one additive scalar/vector coordinate.
#[derive(Clone, Debug, PartialEq)]
pub struct CoordinateBound {
    variable_id: VariableId,
    coordinate: usize,
    lower: Option<f64>,
    upper: Option<f64>,
    label: String,
}

impl CoordinateBound {
    /// Creates a coordinate bound. Coordinate compatibility is validated when
    /// the bound is added to a [`Problem`]. A finite outside initial guess is
    /// projected to the nearest endpoint when solving starts.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty interval/label, non-finite endpoint, or
    /// lower endpoint greater than the upper endpoint.
    pub fn new(
        variable_id: VariableId,
        coordinate: usize,
        lower: Option<f64>,
        upper: Option<f64>,
        label: impl Into<String>,
    ) -> Result<Self, CoreError> {
        if lower.is_none() && upper.is_none() {
            return Err(CoreError::EmptyBound);
        }
        if let Some(value) = lower
            && !value.is_finite()
        {
            return Err(CoreError::InvalidBoundValue {
                side: "lower",
                value,
            });
        }
        if let Some(value) = upper
            && !value.is_finite()
        {
            return Err(CoreError::InvalidBoundValue {
                side: "upper",
                value,
            });
        }
        if let (Some(lower), Some(upper)) = (lower, upper)
            && lower > upper
        {
            return Err(CoreError::InvalidBoundInterval { lower, upper });
        }
        let label = label.into();
        if label.trim().is_empty() {
            return Err(CoreError::EmptyBoundLabel);
        }
        Ok(Self {
            variable_id,
            coordinate,
            lower,
            upper,
            label,
        })
    }

    #[must_use]
    pub const fn variable_id(&self) -> VariableId {
        self.variable_id
    }

    #[must_use]
    pub const fn coordinate(&self) -> usize {
        self.coordinate
    }

    #[must_use]
    pub const fn lower(&self) -> Option<f64> {
        self.lower
    }

    #[must_use]
    pub const fn upper(&self) -> Option<f64> {
        self.upper
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    pub(crate) fn validate_for_problem(&self, problem: &Problem) -> Result<(), CoreError> {
        let variable = problem
            .variable(self.variable_id)
            .ok_or(CoreError::UnknownVariable(self.variable_id))?;
        if matches!(variable.kind(), VariableKind::Pose2 | VariableKind::Pose3) {
            return Err(CoreError::UnsupportedBoundVariableKind {
                variable: self.variable_id,
                kind: variable.kind(),
            });
        }
        let dimension = variable.kind().tangent_dimension();
        if self.coordinate >= dimension {
            return Err(CoreError::InvalidBoundCoordinate {
                variable: self.variable_id,
                coordinate: self.coordinate,
                dimension,
            });
        }
        Ok(())
    }

    pub(crate) fn contains(&self, value: f64) -> bool {
        value.is_finite()
            && self.lower.is_none_or(|lower| value >= lower)
            && self.upper.is_none_or(|upper| value <= upper)
    }
}

/// Accepted-state location of one bound.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BoundStatus {
    Inactive,
    ActiveLower,
    ActiveUpper,
    Fixed,
}

/// Whether a nonzero direction exists in the active feasible tangent cone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum OneSidedMobility {
    Exists,
    None,
    NotEvaluated,
}

/// Deterministic accepted-state bound audit and active-set entry.
#[derive(Clone, Debug, PartialEq)]
pub struct BoundReport {
    pub bound_id: BoundId,
    pub variable_id: VariableId,
    pub coordinate: usize,
    pub label: String,
    pub lower: Option<f64>,
    pub upper: Option<f64>,
    pub value: f64,
    pub status: BoundStatus,
}

pub(crate) fn coordinate_value(value: VariableValue, coordinate: usize) -> f64 {
    value.ambient_values()[coordinate]
}

pub(crate) fn set_coordinate_value(
    value: &mut VariableValue,
    coordinate: usize,
    target: f64,
) -> Result<(), CoreError> {
    let mut delta = vec![0.0; value.kind().tangent_dimension()];
    delta[coordinate] = target - coordinate_value(*value, coordinate);
    value.plus(&delta)
}
