//! Pure geometric value types shared by sketches and mechanisms.

mod curves;

pub use curves::{
    CurveEvaluationError, CurveJet2, CurveParameterDomain, CurveParameterError,
    CurveRegularityError, circle_jet, circular_arc_jet, cubic_bezier_jet, line_jet,
    quadratic_bezier_jet,
};
pub use nalgebra::{Point2, Point3, Rotation2, Vector2, Vector3};

/// A two-dimensional rigid transform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose2 {
    pub translation: Vector2<f64>,
    pub angle: f64,
}

impl Pose2 {
    #[must_use]
    pub fn identity() -> Self {
        Self {
            translation: Vector2::zeros(),
            angle: 0.0,
        }
    }

    #[must_use]
    pub fn transform_point(&self, point: Point2<f64>) -> Point2<f64> {
        Point2::from(self.translation + Rotation2::new(self.angle) * point.coords)
    }
}

/// Embeds local 2D coordinates in world-space 3D.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneFrame {
    pub origin: Point3<f64>,
    pub u: Vector3<f64>,
    pub v: Vector3<f64>,
}

impl PlaneFrame {
    #[must_use]
    pub fn map_point(&self, point: Point2<f64>) -> Point3<f64> {
        self.origin + self.u * point.x + self.v * point.y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_frame_maps_local_coordinates() {
        let frame = PlaneFrame {
            origin: Point3::new(10.0, 20.0, 30.0),
            u: Vector3::x(),
            v: Vector3::z(),
        };
        let mapped = frame.map_point(Point2::new(2.0, 3.0));

        assert!((mapped - Point3::new(12.0, 20.0, 33.0)).norm() < 1.0e-12);
    }
}
