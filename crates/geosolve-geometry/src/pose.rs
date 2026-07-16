use nalgebra::{
    Matrix3, Point2, Point3, Quaternion, Rotation2, SMatrix, UnitQuaternion, Vector2, Vector3,
};

use crate::GeometryError;
use crate::validation::vector3_is_finite;

const SMALL_ANGLE: f64 = 1.0e-4;

/// Maximum accepted absolute error in an imported quaternion norm.
pub const QUATERNION_NORM_TOLERANCE: f64 = 1.0e-6;

/// Components this close to zero use the quaternion vector part as the sign tie-breaker.
pub const QUATERNION_SIGN_TOLERANCE: f64 = 32.0 * f64::EPSILON;

/// A two-dimensional body-to-world rigid transform.
///
/// The ambient representation is `[t_x, t_y, angle]`. Tangents are body-local
/// `[v_x, v_y, omega]` and use right retraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose2 {
    pub translation: Vector2<f64>,
    pub angle: f64,
}

impl Pose2 {
    pub const AMBIENT_DIMENSION: usize = 3;
    pub const TANGENT_DIMENSION: usize = 3;

    #[must_use]
    pub fn identity() -> Self {
        Self {
            translation: Vector2::zeros(),
            angle: 0.0,
        }
    }

    /// Constructs a finite pose.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::NonFinitePose`] for non-finite coordinates.
    pub fn try_new(translation: Vector2<f64>, angle: f64) -> Result<Self, GeometryError> {
        let pose = Self { translation, angle };
        pose.validate()?;
        Ok(pose)
    }

    /// Constructs a finite pose from `[t_x, t_y, angle]`.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::NonFinitePose`] for non-finite coordinates.
    pub fn from_ambient(ambient: [f64; 3]) -> Result<Self, GeometryError> {
        Self::try_new(Vector2::new(ambient[0], ambient[1]), ambient[2])
    }

    #[must_use]
    pub fn ambient(self) -> [f64; 3] {
        [self.translation.x, self.translation.y, self.angle]
    }

    /// Validates all ambient coordinates.
    ///
    /// # Errors
    ///
    /// Returns [`GeometryError::NonFinitePose`] for non-finite coordinates.
    pub fn validate(&self) -> Result<(), GeometryError> {
        if self.translation.iter().all(|value| value.is_finite()) && self.angle.is_finite() {
            Ok(())
        } else {
            Err(GeometryError::NonFinitePose)
        }
    }

    /// Maps a point without finite validation.
    ///
    /// Use [`Pose2::try_transform_point`] at validated boundaries.
    #[must_use]
    pub fn transform_point(&self, point: Point2<f64>) -> Point2<f64> {
        Point2::from(self.translation + Rotation2::new(self.angle) * point.coords)
    }

    /// Maps a vector without finite validation.
    ///
    /// Use [`Pose2::try_transform_vector`] at validated boundaries.
    #[must_use]
    pub fn transform_vector(&self, vector: Vector2<f64>) -> Vector2<f64> {
        Rotation2::new(self.angle) * vector
    }

    /// Inverse-maps a point without finite validation.
    ///
    /// Use [`Pose2::try_inverse_transform_point`] at validated boundaries.
    #[must_use]
    pub fn inverse_transform_point(&self, point: Point2<f64>) -> Point2<f64> {
        Point2::from(Rotation2::new(-self.angle) * (point.coords - self.translation))
    }

    /// Inverse-maps a vector without finite validation.
    ///
    /// Use [`Pose2::try_inverse_transform_vector`] at validated boundaries.
    #[must_use]
    pub fn inverse_transform_vector(&self, vector: Vector2<f64>) -> Vector2<f64> {
        Rotation2::new(-self.angle) * vector
    }

    /// Validates this pose and maps a finite local point to world coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite pose, input, or result.
    pub fn try_transform_point(&self, point: Point2<f64>) -> Result<Point2<f64>, GeometryError> {
        self.validate()?;
        validate_point2(point)?;
        checked_point2(self.transform_point(point))
    }

    /// Validates this pose and maps a finite local vector to world coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite pose, input, or result.
    pub fn try_transform_vector(
        &self,
        vector: Vector2<f64>,
    ) -> Result<Vector2<f64>, GeometryError> {
        self.validate()?;
        validate_vector2(vector)?;
        checked_vector2(self.transform_vector(vector))
    }

    /// Validates this pose and maps a finite world point to local coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite pose, input, or result.
    pub fn try_inverse_transform_point(
        &self,
        point: Point2<f64>,
    ) -> Result<Point2<f64>, GeometryError> {
        self.validate()?;
        validate_point2(point)?;
        checked_point2(self.inverse_transform_point(point))
    }

    /// Validates this pose and maps a finite world vector to local coordinates.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite pose, input, or result.
    pub fn try_inverse_transform_vector(
        &self,
        vector: Vector2<f64>,
    ) -> Result<Vector2<f64>, GeometryError> {
        self.validate()?;
        validate_vector2(vector)?;
        checked_vector2(self.inverse_transform_vector(vector))
    }

    /// Composes body-to-parent transforms as `self * child`.
    ///
    /// # Errors
    ///
    /// Returns an error if either input or the composed pose is non-finite.
    pub fn compose(&self, child: &Self) -> Result<Self, GeometryError> {
        self.validate()?;
        child.validate()?;
        Self::try_new(
            self.translation + self.transform_vector(child.translation),
            self.angle + child.angle,
        )
        .map_err(|_| GeometryError::NonFiniteResult)
    }

    /// Returns the inverse rigid transform.
    ///
    /// # Errors
    ///
    /// Returns an error if the input or inverse pose is non-finite.
    pub fn inverse(&self) -> Result<Self, GeometryError> {
        self.validate()?;
        let inverse_rotation = Rotation2::new(-self.angle);
        Self::try_new(-(inverse_rotation * self.translation), -self.angle)
            .map_err(|_| GeometryError::NonFiniteResult)
    }

    /// Evaluates the `SE(2)` exponential of body-local `[v_x, v_y, omega]`.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite tangent or result.
    pub fn exp(tangent: [f64; 3]) -> Result<Self, GeometryError> {
        if !tangent.iter().all(|value| value.is_finite()) {
            return Err(GeometryError::NonFiniteTangent);
        }
        let [velocity_x, velocity_y, angle] = tangent;
        let (sinc, cosc) = se2_exp_coefficients(angle);
        Self::try_new(
            Vector2::new(
                sinc * velocity_x - cosc * velocity_y,
                cosc * velocity_x + sinc * velocity_y,
            ),
            angle,
        )
        .map_err(|_| GeometryError::NonFiniteResult)
    }

    /// Evaluates the principal `SE(2)` logarithm in body-local tangent order.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite pose or result.
    pub fn log(&self) -> Result<[f64; 3], GeometryError> {
        self.validate()?;
        let (sine, cosine) = self.angle.sin_cos();
        let angle = sine.atan2(cosine);
        let half_angle = 0.5 * angle;
        let inverse_diagonal = half_angle_cotangent(half_angle);
        let tangent = [
            inverse_diagonal * self.translation.x + half_angle * self.translation.y,
            -half_angle * self.translation.x + inverse_diagonal * self.translation.y,
            angle,
        ];
        if tangent.iter().all(|value| value.is_finite()) {
            Ok(tangent)
        } else {
            Err(GeometryError::NonFiniteResult)
        }
    }

    /// Returns the adjoint mapping body-local tangents through this transform.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite pose.
    pub fn adjoint(&self) -> Result<Matrix3<f64>, GeometryError> {
        self.validate()?;
        let (sine, cosine) = self.angle.sin_cos();
        Ok(Matrix3::new(
            cosine,
            -sine,
            self.translation.y,
            sine,
            cosine,
            -self.translation.x,
            0.0,
            0.0,
            1.0,
        ))
    }

    /// Applies a right/body-local retraction `self * Exp(delta)`.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn retract(&self, delta: [f64; 3]) -> Result<Self, GeometryError> {
        self.compose(&Self::exp(delta)?)
    }

    /// Returns `Log(inverse(self) * other)` in body-local tangent order.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn local_difference(&self, other: &Self) -> Result<[f64; 3], GeometryError> {
        self.inverse()?.compose(other)?.log()
    }
}

/// A quaternion-backed three-dimensional body-to-world rigid transform.
///
/// The canonical ambient representation is `[t_x, t_y, t_z, q_w, q_x, q_y,
/// q_z]`. Tangents are body-local `[v_x, v_y, v_z, omega_x, omega_y, omega_z]`
/// and use right retraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose3 {
    translation: Vector3<f64>,
    rotation: UnitQuaternion<f64>,
}

impl Pose3 {
    pub const AMBIENT_DIMENSION: usize = 7;
    pub const TANGENT_DIMENSION: usize = 6;

    #[must_use]
    pub fn identity() -> Self {
        Self {
            translation: Vector3::zeros(),
            rotation: UnitQuaternion::identity(),
        }
    }

    /// Constructs a pose from a translation and `[q_w, q_x, q_y, q_z]`.
    ///
    /// A finite quaternion within [`QUATERNION_NORM_TOLERANCE`] of unit norm is
    /// normalized and sign-canonicalized. Materially non-unit inputs reject.
    ///
    /// # Errors
    ///
    /// Returns an error for non-finite translation/quaternion data or a
    /// materially non-unit quaternion.
    pub fn try_new(translation: Vector3<f64>, quaternion: [f64; 4]) -> Result<Self, GeometryError> {
        if !vector3_is_finite(&translation) {
            return Err(GeometryError::NonFinitePose);
        }
        Ok(Self {
            translation,
            rotation: validated_rotation(quaternion)?,
        })
    }

    /// Constructs a pose from `[t_x, t_y, t_z, q_w, q_x, q_y, q_z]`.
    ///
    /// # Errors
    ///
    /// Returns the same validation failures as [`Pose3::try_new`].
    pub fn from_ambient(ambient: [f64; 7]) -> Result<Self, GeometryError> {
        Self::try_new(
            Vector3::new(ambient[0], ambient[1], ambient[2]),
            [ambient[3], ambient[4], ambient[5], ambient[6]],
        )
    }

    #[must_use]
    pub fn ambient(self) -> [f64; 7] {
        let quaternion = self.quaternion();
        [
            self.translation.x,
            self.translation.y,
            self.translation.z,
            quaternion[0],
            quaternion[1],
            quaternion[2],
            quaternion[3],
        ]
    }

    #[must_use]
    pub fn translation(self) -> Vector3<f64> {
        self.translation
    }

    #[must_use]
    pub fn quaternion(self) -> [f64; 4] {
        let quaternion = self.rotation.quaternion();
        [quaternion.w, quaternion.i, quaternion.j, quaternion.k].map(normalize_signed_zero)
    }

    #[must_use]
    pub fn rotation(self) -> UnitQuaternion<f64> {
        self.rotation
    }

    /// Maps a point without finite validation.
    ///
    /// Use [`Pose3::try_transform_point`] at validated boundaries.
    #[must_use]
    pub fn transform_point(&self, point: Point3<f64>) -> Point3<f64> {
        self.rotation.transform_point(&point) + self.translation
    }

    /// Maps a vector without finite validation.
    ///
    /// Use [`Pose3::try_transform_vector`] at validated boundaries.
    #[must_use]
    pub fn transform_vector(&self, vector: Vector3<f64>) -> Vector3<f64> {
        self.rotation.transform_vector(&vector)
    }

    /// Inverse-maps a point without finite validation.
    ///
    /// Use [`Pose3::try_inverse_transform_point`] at validated boundaries.
    #[must_use]
    pub fn inverse_transform_point(&self, point: Point3<f64>) -> Point3<f64> {
        self.rotation
            .inverse_transform_point(&(point - self.translation))
    }

    /// Inverse-maps a vector without finite validation.
    ///
    /// Use [`Pose3::try_inverse_transform_vector`] at validated boundaries.
    #[must_use]
    pub fn inverse_transform_vector(&self, vector: Vector3<f64>) -> Vector3<f64> {
        self.rotation.inverse_transform_vector(&vector)
    }

    /// Maps a finite local point to world coordinates and validates the result.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn try_transform_point(&self, point: Point3<f64>) -> Result<Point3<f64>, GeometryError> {
        validate_point3(point)?;
        checked_point3(self.transform_point(point))
    }

    /// Maps a finite local vector to world coordinates and validates the result.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn try_transform_vector(
        &self,
        vector: Vector3<f64>,
    ) -> Result<Vector3<f64>, GeometryError> {
        validate_vector3(vector)?;
        checked_vector3(self.transform_vector(vector))
    }

    /// Maps a finite world point to local coordinates and validates the result.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn try_inverse_transform_point(
        &self,
        point: Point3<f64>,
    ) -> Result<Point3<f64>, GeometryError> {
        validate_point3(point)?;
        checked_point3(self.inverse_transform_point(point))
    }

    /// Maps a finite world vector to local coordinates and validates the result.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn try_inverse_transform_vector(
        &self,
        vector: Vector3<f64>,
    ) -> Result<Vector3<f64>, GeometryError> {
        validate_vector3(vector)?;
        checked_vector3(self.inverse_transform_vector(vector))
    }

    /// Composes body-to-parent transforms as `self * child`.
    ///
    /// # Errors
    ///
    /// Returns an error if composition produces non-finite coordinates.
    pub fn compose(&self, child: &Self) -> Result<Self, GeometryError> {
        Self::try_new(
            self.translation + self.transform_vector(child.translation),
            quaternion_components(self.rotation * child.rotation),
        )
        .map_err(|_| GeometryError::NonFiniteResult)
    }

    /// Returns the inverse rigid transform.
    ///
    /// # Errors
    ///
    /// Returns an error if inversion produces non-finite coordinates.
    pub fn inverse(&self) -> Result<Self, GeometryError> {
        let inverse_rotation = self.rotation.inverse();
        Self::try_new(
            inverse_rotation.transform_vector(&(-self.translation)),
            quaternion_components(inverse_rotation),
        )
        .map_err(|_| GeometryError::NonFiniteResult)
    }

    /// Evaluates the `SE(3)` exponential of a body-local tangent.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite tangent or result.
    pub fn exp(tangent: [f64; 6]) -> Result<Self, GeometryError> {
        if !tangent.iter().all(|value| value.is_finite()) {
            return Err(GeometryError::NonFiniteTangent);
        }
        let velocity = Vector3::new(tangent[0], tangent[1], tangent[2]);
        let angular = Vector3::new(tangent[3], tangent[4], tangent[5]);
        let angle = robust_norm(angular);
        if !angle.is_finite() {
            return Err(GeometryError::NonFiniteResult);
        }
        let (first_coefficient, second_coefficient) = se3_exp_coefficients(angle);
        let first_cross = angular.cross(&velocity);
        let translation = velocity
            + first_cross * first_coefficient
            + angular.cross(&first_cross) * second_coefficient;
        let quaternion = if angle.to_bits() == std::f64::consts::PI.to_bits() {
            [0.0, angular.x / angle, angular.y / angle, angular.z / angle]
        } else {
            let half_angle = 0.5 * angle;
            let vector_scale = if angle < SMALL_ANGLE {
                let angle_squared = angle * angle;
                0.5 - angle_squared / 48.0 + angle_squared * angle_squared / 3_840.0
            } else {
                half_angle.sin() / angle
            };
            [
                half_angle.cos(),
                vector_scale * angular.x,
                vector_scale * angular.y,
                vector_scale * angular.z,
            ]
        };
        Self::try_new(translation, quaternion).map_err(|_| GeometryError::NonFiniteResult)
    }

    /// Evaluates the principal `SE(3)` logarithm in body-local tangent order.
    ///
    /// # Errors
    ///
    /// Returns an error if the logarithm produces non-finite coordinates.
    pub fn log(&self) -> Result<[f64; 6], GeometryError> {
        let angular = rotation_log(self.rotation);
        let angle = robust_norm(angular);
        let inverse_coefficient = se3_log_coefficient(angle);
        let first_cross = angular.cross(&self.translation);
        let velocity = self.translation - first_cross * 0.5
            + angular.cross(&first_cross) * inverse_coefficient;
        let tangent = [
            velocity.x, velocity.y, velocity.z, angular.x, angular.y, angular.z,
        ];
        if tangent.iter().all(|value| value.is_finite()) {
            Ok(tangent)
        } else {
            Err(GeometryError::NonFiniteResult)
        }
    }

    /// Returns the `6 x 6` adjoint for `[v_x, v_y, v_z, omega_x, omega_y, omega_z]`.
    #[must_use]
    pub fn adjoint(&self) -> SMatrix<f64, 6, 6> {
        let rotation = self.rotation.to_rotation_matrix();
        let rotation_matrix = rotation.matrix();
        let translation_cross = skew(self.translation);
        let mut adjoint = SMatrix::<f64, 6, 6>::zeros();
        adjoint
            .fixed_view_mut::<3, 3>(0, 0)
            .copy_from(rotation_matrix);
        adjoint
            .fixed_view_mut::<3, 3>(0, 3)
            .copy_from(&(translation_cross * rotation_matrix));
        adjoint
            .fixed_view_mut::<3, 3>(3, 3)
            .copy_from(rotation_matrix);
        adjoint
    }

    /// Applies a right/body-local retraction `self * Exp(delta)`.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite input or result.
    pub fn retract(&self, delta: [f64; 6]) -> Result<Self, GeometryError> {
        self.compose(&Self::exp(delta)?)
    }

    /// Returns `Log(inverse(self) * other)` in body-local tangent order.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-finite result.
    pub fn local_difference(&self, other: &Self) -> Result<[f64; 6], GeometryError> {
        self.inverse()?.compose(other)?.log()
    }
}

fn se2_exp_coefficients(angle: f64) -> (f64, f64) {
    if angle.abs() < SMALL_ANGLE {
        let squared = angle * angle;
        let fourth = squared * squared;
        (
            1.0 - squared / 6.0 + fourth / 120.0 - fourth * squared / 5_040.0,
            angle * (0.5 - squared / 24.0 + fourth / 720.0 - fourth * squared / 40_320.0),
        )
    } else {
        (angle.sin() / angle, (1.0 - angle.cos()) / angle)
    }
}

fn half_angle_cotangent(half_angle: f64) -> f64 {
    if half_angle.abs() < 0.5 * SMALL_ANGLE {
        let squared = half_angle * half_angle;
        1.0 - squared / 3.0 - squared * squared / 45.0 - 2.0 * squared.powi(3) / 945.0
    } else {
        half_angle / half_angle.tan()
    }
}

fn se3_exp_coefficients(angle: f64) -> (f64, f64) {
    if angle < SMALL_ANGLE {
        let squared = angle * angle;
        let fourth = squared * squared;
        (
            0.5 - squared / 24.0 + fourth / 720.0 - fourth * squared / 40_320.0,
            1.0 / 6.0 - squared / 120.0 + fourth / 5_040.0 - fourth * squared / 362_880.0,
        )
    } else {
        (
            (1.0 - angle.cos()) / angle / angle,
            (angle - angle.sin()) / angle / angle / angle,
        )
    }
}

fn se3_log_coefficient(angle: f64) -> f64 {
    if angle < SMALL_ANGLE {
        let squared = angle * angle;
        1.0 / 12.0 + squared / 720.0 + squared * squared / 30_240.0
    } else {
        let half_angle = 0.5 * angle;
        (1.0 - half_angle / half_angle.tan()) / (angle * angle)
    }
}

fn rotation_log(rotation: UnitQuaternion<f64>) -> Vector3<f64> {
    let quaternion = rotation.quaternion();
    let mut vector = Vector3::new(quaternion.i, quaternion.j, quaternion.k);
    let scalar = if quaternion.w < 0.0 {
        vector = -vector;
        -quaternion.w
    } else {
        quaternion.w
    };
    let sine_half = robust_norm(vector);
    if sine_half == 0.0 {
        return Vector3::zeros();
    }
    let scale = if sine_half < SMALL_ANGLE {
        let squared = sine_half * sine_half;
        2.0 + squared / 3.0 + 3.0 * squared * squared / 20.0
    } else {
        2.0 * sine_half.atan2(scalar) / sine_half
    };
    vector * scale
}

fn validated_rotation(quaternion: [f64; 4]) -> Result<UnitQuaternion<f64>, GeometryError> {
    if !quaternion.iter().all(|value| value.is_finite()) {
        return Err(GeometryError::NonFiniteQuaternion);
    }
    let norm = quaternion[0]
        .hypot(quaternion[1])
        .hypot(quaternion[2])
        .hypot(quaternion[3]);
    if norm == 0.0 || !norm.is_finite() || (norm - 1.0).abs() > QUATERNION_NORM_TOLERANCE {
        return Err(GeometryError::InvalidQuaternionNorm { norm });
    }
    let normalized = quaternion.map(|component| component / norm);
    let canonical = canonical_quaternion(normalized);
    Ok(UnitQuaternion::new_normalize(Quaternion::new(
        canonical[0],
        canonical[1],
        canonical[2],
        canonical[3],
    )))
}

fn canonical_quaternion(mut quaternion: [f64; 4]) -> [f64; 4] {
    let flip = if quaternion[0] < -QUATERNION_SIGN_TOLERANCE {
        true
    } else if quaternion[0] > QUATERNION_SIGN_TOLERANCE {
        false
    } else {
        quaternion[1..]
            .iter()
            .find(|component| component.abs() > QUATERNION_SIGN_TOLERANCE)
            .is_some_and(|component| component.is_sign_negative())
    };
    if flip {
        for component in &mut quaternion {
            *component = -*component;
        }
    }
    for component in &mut quaternion {
        *component = normalize_signed_zero(*component);
    }
    quaternion
}

fn quaternion_components(rotation: UnitQuaternion<f64>) -> [f64; 4] {
    let quaternion = rotation.quaternion();
    [quaternion.w, quaternion.i, quaternion.j, quaternion.k]
}

fn robust_norm(vector: Vector3<f64>) -> f64 {
    vector.x.hypot(vector.y).hypot(vector.z)
}

fn normalize_signed_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

fn validate_point2(point: Point2<f64>) -> Result<(), GeometryError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(GeometryError::NonFinitePoint)
    }
}

fn validate_vector2(vector: Vector2<f64>) -> Result<(), GeometryError> {
    if vector.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(GeometryError::NonFiniteVector)
    }
}

fn checked_point2(point: Point2<f64>) -> Result<Point2<f64>, GeometryError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok(point)
    } else {
        Err(GeometryError::NonFiniteResult)
    }
}

fn checked_vector2(vector: Vector2<f64>) -> Result<Vector2<f64>, GeometryError> {
    if vector.iter().all(|value| value.is_finite()) {
        Ok(vector)
    } else {
        Err(GeometryError::NonFiniteResult)
    }
}

fn validate_point3(point: Point3<f64>) -> Result<(), GeometryError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(GeometryError::NonFinitePoint)
    }
}

fn validate_vector3(vector: Vector3<f64>) -> Result<(), GeometryError> {
    if vector3_is_finite(&vector) {
        Ok(())
    } else {
        Err(GeometryError::NonFiniteVector)
    }
}

fn checked_point3(point: Point3<f64>) -> Result<Point3<f64>, GeometryError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok(point)
    } else {
        Err(GeometryError::NonFiniteResult)
    }
}

fn checked_vector3(vector: Vector3<f64>) -> Result<Vector3<f64>, GeometryError> {
    if vector3_is_finite(&vector) {
        Ok(vector)
    } else {
        Err(GeometryError::NonFiniteResult)
    }
}

fn skew(vector: Vector3<f64>) -> Matrix3<f64> {
    Matrix3::new(
        0.0, -vector.z, vector.y, vector.z, 0.0, -vector.x, -vector.y, vector.x, 0.0,
    )
}
