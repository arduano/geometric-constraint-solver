use nalgebra::{Point2, Point3, Vector2, Vector3};

use crate::GeometryError;
use crate::validation::vector3_is_finite;

/// Tolerance used when validating imported orthonormal frame axes.
pub const FRAME_ORTHONORMAL_TOLERANCE: f64 = 1.0e-9;

/// A validated right-handed orthonormal 3D coordinate frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame3 {
    origin: Point3<f64>,
    x_axis: Vector3<f64>,
    y_axis: Vector3<f64>,
    z_axis: Vector3<f64>,
}

impl Frame3 {
    /// Constructs a right-handed orthonormal coordinate frame.
    ///
    /// Near-unit axes within [`FRAME_ORTHONORMAL_TOLERANCE`] are normalized
    /// after orthogonality and handedness validation.
    ///
    /// # Errors
    ///
    /// Returns a typed error for non-finite, degenerate, non-orthonormal, or
    /// left-handed input.
    pub fn try_new(
        origin: Point3<f64>,
        x_axis: Vector3<f64>,
        y_axis: Vector3<f64>,
        z_axis: Vector3<f64>,
    ) -> Result<Self, GeometryError> {
        validate_point(origin)?;
        let [x_axis, y_axis, z_axis] = validate_axes([x_axis, y_axis, z_axis])?;
        if (x_axis.cross(&y_axis) - z_axis).norm() > FRAME_ORTHONORMAL_TOLERANCE {
            return Err(GeometryError::LeftHandedFrame);
        }
        Ok(Self {
            origin,
            x_axis,
            y_axis,
            z_axis,
        })
    }

    #[must_use]
    pub fn origin(self) -> Point3<f64> {
        self.origin
    }

    #[must_use]
    pub fn x_axis(self) -> Vector3<f64> {
        self.x_axis
    }

    #[must_use]
    pub fn y_axis(self) -> Vector3<f64> {
        self.y_axis
    }

    #[must_use]
    pub fn z_axis(self) -> Vector3<f64> {
        self.z_axis
    }

    /// Maps a finite frame-local point into parent/world coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn transform_point(&self, point: Point3<f64>) -> Result<Point3<f64>, GeometryError> {
        validate_point(point)?;
        checked_point(
            self.origin + self.x_axis * point.x + self.y_axis * point.y + self.z_axis * point.z,
        )
    }

    /// Maps a finite frame-local vector into parent/world coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn transform_vector(&self, vector: Vector3<f64>) -> Result<Vector3<f64>, GeometryError> {
        validate_vector(vector)?;
        checked_vector(self.x_axis * vector.x + self.y_axis * vector.y + self.z_axis * vector.z)
    }

    /// Maps a finite parent/world point into frame-local coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn inverse_transform_point(
        &self,
        point: Point3<f64>,
    ) -> Result<Point3<f64>, GeometryError> {
        validate_point(point)?;
        let offset = point - self.origin;
        checked_point(Point3::new(
            self.x_axis.dot(&offset),
            self.y_axis.dot(&offset),
            self.z_axis.dot(&offset),
        ))
    }

    /// Maps a finite parent/world vector into frame-local coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn inverse_transform_vector(
        &self,
        vector: Vector3<f64>,
    ) -> Result<Vector3<f64>, GeometryError> {
        validate_vector(vector)?;
        checked_vector(Vector3::new(
            self.x_axis.dot(&vector),
            self.y_axis.dot(&vector),
            self.z_axis.dot(&vector),
        ))
    }
}

/// Embeds local 2D coordinates in parent/world 3D coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneFrame {
    origin: Point3<f64>,
    u: Vector3<f64>,
    v: Vector3<f64>,
}

/// Preferred name for a validated 2D coordinate plane embedded in 3D.
pub type Workplane = PlaneFrame;

impl PlaneFrame {
    /// Constructs a workplane from finite, near-unit orthogonal axes.
    ///
    /// # Errors
    ///
    /// Returns a typed error for non-finite, degenerate, or non-orthonormal input.
    pub fn try_new(
        origin: Point3<f64>,
        u: Vector3<f64>,
        v: Vector3<f64>,
    ) -> Result<Self, GeometryError> {
        validate_point(origin)?;
        let [u, v] = validate_axes([u, v])?;
        Ok(Self { origin, u, v })
    }

    #[must_use]
    pub fn origin(self) -> Point3<f64> {
        self.origin
    }

    #[must_use]
    pub fn u(self) -> Vector3<f64> {
        self.u
    }

    #[must_use]
    pub fn v(self) -> Vector3<f64> {
        self.v
    }

    /// Revalidates this frame's finite orthonormal representation.
    ///
    /// # Errors
    ///
    /// Returns a typed error for non-finite, degenerate, or non-orthonormal input.
    pub fn validate(&self) -> Result<(), GeometryError> {
        validate_point(self.origin)?;
        validate_axes([self.u, self.v]).map(|_| ())
    }

    #[must_use]
    pub fn normal(&self) -> Vector3<f64> {
        self.u.cross(&self.v)
    }

    /// Maps a finite local point to world coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn map_point(&self, point: Point2<f64>) -> Result<Point3<f64>, GeometryError> {
        if !point.coords.iter().all(|value| value.is_finite()) {
            return Err(GeometryError::NonFinitePoint);
        }
        checked_point(self.origin + self.u * point.x + self.v * point.y)
    }

    /// Maps a finite local vector to world coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn map_vector(&self, vector: Vector2<f64>) -> Result<Vector3<f64>, GeometryError> {
        if !vector.iter().all(|value| value.is_finite()) {
            return Err(GeometryError::NonFiniteVector);
        }
        checked_vector(self.u * vector.x + self.v * vector.y)
    }

    /// Validates the frame and maps a finite point to world coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid frame, input, or result.
    pub fn try_map_point(&self, point: Point2<f64>) -> Result<Point3<f64>, GeometryError> {
        self.map_point(point)
    }

    /// Validates the frame and maps a finite vector to world coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid frame, input, or result.
    pub fn try_map_vector(&self, vector: Vector2<f64>) -> Result<Vector3<f64>, GeometryError> {
        self.map_vector(vector)
    }

    /// Maps a world point on this workplane back to local 2D coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid frame, non-finite input/result, or a
    /// point outside the workplane tolerance.
    pub fn inverse_map_point(&self, point: Point3<f64>) -> Result<Point2<f64>, GeometryError> {
        self.validate()?;
        validate_point(point)?;
        let offset = point - self.origin;
        ensure_intermediate_vector_is_finite(offset)?;
        ensure_on_workplane(self.normal().dot(&offset), max_component_scale(offset)?)?;
        let local = Point2::new(self.u.dot(&offset), self.v.dot(&offset));
        if local.coords.iter().all(|value| value.is_finite()) {
            Ok(local)
        } else {
            Err(GeometryError::NonFiniteResult)
        }
    }

    /// Maps a world vector parallel to this workplane back to local 2D coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid frame, non-finite input/result, or a
    /// vector outside the workplane tolerance.
    pub fn inverse_map_vector(&self, vector: Vector3<f64>) -> Result<Vector2<f64>, GeometryError> {
        self.validate()?;
        validate_vector(vector)?;
        ensure_on_workplane(self.normal().dot(&vector), max_component_scale(vector)?)?;
        let local = Vector2::new(self.u.dot(&vector), self.v.dot(&vector));
        if local.iter().all(|value| value.is_finite()) {
            Ok(local)
        } else {
            Err(GeometryError::NonFiniteResult)
        }
    }
}

fn validate_axes<const N: usize>(
    axes: [Vector3<f64>; N],
) -> Result<[Vector3<f64>; N], GeometryError> {
    if !axes.iter().all(vector3_is_finite) {
        return Err(GeometryError::NonFiniteVector);
    }
    let norms = axes.map(|axis| axis.norm());
    if norms.contains(&0.0) {
        return Err(GeometryError::DegenerateFrameAxis);
    }
    if norms
        .iter()
        .any(|norm| !norm.is_finite() || (norm - 1.0).abs() > FRAME_ORTHONORMAL_TOLERANCE)
    {
        return Err(GeometryError::NonOrthonormalFrame);
    }
    let normalized = std::array::from_fn(|index| axes[index] / norms[index]);
    for first in 0..N {
        for second in (first + 1)..N {
            if normalized[first].dot(&normalized[second]).abs() > FRAME_ORTHONORMAL_TOLERANCE {
                return Err(GeometryError::NonOrthonormalFrame);
            }
        }
    }
    Ok(normalized)
}

fn ensure_on_workplane(distance: f64, scale: f64) -> Result<(), GeometryError> {
    if !distance.is_finite() || !scale.is_finite() {
        return Err(GeometryError::NonFiniteResult);
    }
    let tolerance = FRAME_ORTHONORMAL_TOLERANCE * scale.max(1.0);
    if !tolerance.is_finite() {
        return Err(GeometryError::NonFiniteResult);
    }
    if distance.abs() <= tolerance {
        Ok(())
    } else {
        Err(GeometryError::OffWorkplane { distance })
    }
}

fn max_component_scale(vector: Vector3<f64>) -> Result<f64, GeometryError> {
    ensure_intermediate_vector_is_finite(vector)?;
    Ok(vector.x.abs().max(vector.y.abs()).max(vector.z.abs()))
}

fn ensure_intermediate_vector_is_finite(vector: Vector3<f64>) -> Result<(), GeometryError> {
    if vector3_is_finite(&vector) {
        Ok(())
    } else {
        Err(GeometryError::NonFiniteResult)
    }
}

fn validate_point(point: Point3<f64>) -> Result<(), GeometryError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(GeometryError::NonFinitePoint)
    }
}

fn validate_vector(vector: Vector3<f64>) -> Result<(), GeometryError> {
    if vector3_is_finite(&vector) {
        Ok(())
    } else {
        Err(GeometryError::NonFiniteVector)
    }
}

fn checked_point(point: Point3<f64>) -> Result<Point3<f64>, GeometryError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok(point)
    } else {
        Err(GeometryError::NonFiniteResult)
    }
}

fn checked_vector(vector: Vector3<f64>) -> Result<Vector3<f64>, GeometryError> {
    if vector3_is_finite(&vector) {
        Ok(vector)
    } else {
        Err(GeometryError::NonFiniteResult)
    }
}
