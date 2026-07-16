use nalgebra::Vector3;
use thiserror::Error;

/// A finite three-dimensional vector.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec3(Vector3<f64>);

impl Vec3 {
    /// Constructs a finite vector.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::NonFiniteVector`] if any component is not finite.
    pub fn try_new(x: f64, y: f64, z: f64) -> Result<Self, GeometryError> {
        Self::try_from_vector(Vector3::new(x, y, z))
    }

    /// Validates an existing vector.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::NonFiniteVector`] if any component is not finite.
    pub fn try_from_vector(vector: Vector3<f64>) -> Result<Self, GeometryError> {
        if vector.iter().all(|component| component.is_finite()) {
            Ok(Self(vector))
        } else {
            Err(GeometryError::NonFiniteVector)
        }
    }

    #[must_use]
    pub fn x(self) -> f64 {
        self.0.x
    }

    #[must_use]
    pub fn y(self) -> f64 {
        self.0.y
    }

    #[must_use]
    pub fn z(self) -> f64 {
        self.0.z
    }

    #[must_use]
    pub fn as_vector(&self) -> &Vector3<f64> {
        &self.0
    }

    #[must_use]
    pub fn into_vector(self) -> Vector3<f64> {
        self.0
    }

    #[must_use]
    pub fn as_array(self) -> [f64; 3] {
        self.0.into()
    }
}

impl TryFrom<Vector3<f64>> for Vec3 {
    type Error = GeometryError;

    fn try_from(vector: Vector3<f64>) -> Result<Self, Self::Error> {
        Self::try_from_vector(vector)
    }
}

impl TryFrom<[f64; 3]> for Vec3 {
    type Error = GeometryError;

    fn try_from(vector: [f64; 3]) -> Result<Self, Self::Error> {
        Self::try_new(vector[0], vector[1], vector[2])
    }
}

impl From<Vec3> for Vector3<f64> {
    fn from(vector: Vec3) -> Self {
        vector.into_vector()
    }
}

impl From<Vec3> for [f64; 3] {
    fn from(vector: Vec3) -> Self {
        vector.as_array()
    }
}

/// Validation failures for rigid transforms and coordinate frames.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum GeometryError {
    #[error("vector components must be finite")]
    NonFiniteVector,
    #[error("point coordinates must be finite")]
    NonFinitePoint,
    #[error("pose coordinates must be finite")]
    NonFinitePose,
    #[error("tangent coordinates must be finite")]
    NonFiniteTangent,
    #[error("quaternion components must be finite")]
    NonFiniteQuaternion,
    #[error("quaternion norm {norm} is not within the accepted near-unit band")]
    InvalidQuaternionNorm { norm: f64 },
    #[error("frame axis is degenerate")]
    DegenerateFrameAxis,
    #[error("frame axes must be orthonormal")]
    NonOrthonormalFrame,
    #[error("frame axes must form a right-handed basis")]
    LeftHandedFrame,
    #[error("point or vector is off the workplane by {distance}")]
    OffWorkplane { distance: f64 },
    #[error("geometry operation produced a non-finite result")]
    NonFiniteResult,
}

pub(crate) fn vector3_is_finite(vector: &Vector3<f64>) -> bool {
    vector.iter().all(|component| component.is_finite())
}
