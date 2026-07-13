use crate::CoreError;

/// The storage and local-increment shape of a variable block.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VariableKind {
    Scalar,
    Vec2,
    /// Stored as `[translation_x, translation_y, unwrapped_angle]`.
    Pose2,
}

impl VariableKind {
    #[must_use]
    pub const fn ambient_dimension(self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Vec2 => 2,
            Self::Pose2 => 3,
        }
    }

    #[must_use]
    pub const fn tangent_dimension(self) -> usize {
        match self {
            Self::Scalar => 1,
            Self::Vec2 => 2,
            Self::Pose2 => 3,
        }
    }
}

/// Ambient values passed to residual evaluators in declared incidence order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VariableValue {
    Scalar(f64),
    Vec2([f64; 2]),
    /// `[translation_x, translation_y, unwrapped_angle]`.
    Pose2([f64; 3]),
}

impl VariableValue {
    #[must_use]
    pub const fn kind(self) -> VariableKind {
        match self {
            Self::Scalar(_) => VariableKind::Scalar,
            Self::Vec2(_) => VariableKind::Vec2,
            Self::Pose2(_) => VariableKind::Pose2,
        }
    }

    #[must_use]
    pub fn ambient_values(&self) -> &[f64] {
        match self {
            Self::Scalar(value) => std::slice::from_ref(value),
            Self::Vec2(values) => values,
            Self::Pose2(values) => values,
        }
    }

    pub(crate) fn validate_finite(&self) -> Result<(), CoreError> {
        validate_finite(self.ambient_values(), "variable ambient")
    }

    pub(crate) fn plus(&mut self, delta: &[f64]) -> Result<(), CoreError> {
        let expected = self.kind().tangent_dimension();
        if delta.len() != expected {
            return Err(CoreError::DimensionMismatch {
                context: "local increment",
                expected,
                actual: delta.len(),
            });
        }
        validate_finite(delta, "local increment")?;

        let mut updated = *self;
        match &mut updated {
            Self::Scalar(value) => *value += delta[0],
            Self::Vec2(values) => {
                values[0] += delta[0];
                values[1] += delta[1];
            }
            Self::Pose2(values) => {
                values[0] += delta[0];
                values[1] += delta[1];
                values[2] += delta[2];
            }
        }
        updated.validate_finite()?;
        *self = updated;
        Ok(())
    }
}

/// A variable value and the characteristic size of each tangent coordinate.
#[derive(Clone, Debug, PartialEq)]
pub struct VariableBlock {
    value: VariableValue,
    step_scales: Vec<f64>,
}

impl VariableBlock {
    /// Creates a variable block with one scale per tangent coordinate.
    ///
    /// # Errors
    ///
    /// Returns an error if a value is non-finite, a scale is not positive and
    /// finite, or the number of scales does not match the variable kind.
    pub fn new(value: VariableValue, step_scales: Vec<f64>) -> Result<Self, CoreError> {
        value.validate_finite()?;
        let expected = value.kind().tangent_dimension();
        if step_scales.len() != expected {
            return Err(CoreError::DimensionMismatch {
                context: "variable step scales",
                expected,
                actual: step_scales.len(),
            });
        }
        validate_scales(&step_scales, "variable step")?;
        Ok(Self { value, step_scales })
    }

    /// Creates a scalar block.
    ///
    /// # Errors
    ///
    /// Returns an error if the value or scale is invalid.
    pub fn scalar(value: f64, step_scale: f64) -> Result<Self, CoreError> {
        Self::new(VariableValue::Scalar(value), vec![step_scale])
    }

    /// Creates a two-coordinate vector block.
    ///
    /// # Errors
    ///
    /// Returns an error if a value or scale is invalid.
    pub fn vec2(value: [f64; 2], step_scales: [f64; 2]) -> Result<Self, CoreError> {
        Self::new(VariableValue::Vec2(value), step_scales.to_vec())
    }

    /// Creates a planar pose block stored as `[x, y, unwrapped_angle]`.
    ///
    /// # Errors
    ///
    /// Returns an error if a value or scale is invalid.
    pub fn pose2(value: [f64; 3], step_scales: [f64; 3]) -> Result<Self, CoreError> {
        Self::new(VariableValue::Pose2(value), step_scales.to_vec())
    }

    #[must_use]
    pub const fn kind(&self) -> VariableKind {
        self.value.kind()
    }

    #[must_use]
    pub const fn value(&self) -> VariableValue {
        self.value
    }

    #[must_use]
    pub fn step_scales(&self) -> &[f64] {
        &self.step_scales
    }

    /// Replaces the ambient value without changing the block kind.
    ///
    /// # Errors
    ///
    /// Returns an error for a different variable kind or a non-finite value.
    pub fn set_value(&mut self, value: VariableValue) -> Result<(), CoreError> {
        let expected = self.kind();
        let actual = value.kind();
        if actual != expected {
            return Err(CoreError::VariableKindMismatch { expected, actual });
        }
        value.validate_finite()?;
        self.value = value;
        Ok(())
    }

    /// Applies a tangent increment without wrapping the `Pose2` angle.
    ///
    /// # Errors
    ///
    /// Returns an error if the increment dimension is wrong or the increment
    /// or resulting value is non-finite.
    pub fn apply_local_increment(&mut self, delta: &[f64]) -> Result<(), CoreError> {
        self.value.plus(delta)
    }

    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        self.value.validate_finite()?;
        validate_scales(&self.step_scales, "variable step")
    }
}

pub(crate) fn validate_scales(scales: &[f64], context: &'static str) -> Result<(), CoreError> {
    for (index, &value) in scales.iter().enumerate() {
        if !value.is_finite() || value <= 0.0 {
            return Err(CoreError::InvalidScale {
                context,
                index,
                value,
            });
        }
    }
    Ok(())
}

fn validate_finite(values: &[f64], context: &'static str) -> Result<(), CoreError> {
    for (index, &value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(CoreError::NonFiniteValue {
                context,
                index,
                value,
            });
        }
    }
    Ok(())
}
