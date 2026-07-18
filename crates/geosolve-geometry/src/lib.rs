//! Pure geometric value types shared by sketches and mechanisms.

mod conics;
mod curves;
mod frames;
mod pose;
mod validation;

pub use conics::{
    ConicDefinitionError, ConicEvaluationError, DirectedParameterTrim, Ellipse2,
    EllipseAxisObservability, EllipticalArc2, HyperbolaBranch, HyperbolaSegment2, ParabolaSegment2,
    ProperConicKind, RationalQuadraticConicSegment2, UnitDirection2, ellipse_jet,
    elliptical_arc_jet, hyperbola_segment_jet, parabola_segment_jet, rational_quadratic_conic_jet,
};
pub use curves::{
    CurveEvaluationError, CurveJet2, CurveParameterDomain, CurveParameterError,
    CurveRegularityError, circle_jet, circular_arc_jet, cubic_bezier_jet, line_jet,
    quadratic_bezier_jet,
};
pub use frames::{FRAME_ORTHONORMAL_TOLERANCE, Frame3, PlaneFrame, Workplane};
pub use nalgebra::{Matrix3, Point2, Point3, Rotation2, SMatrix, UnitQuaternion, Vector2, Vector3};
pub use pose::{Pose2, Pose3, QUATERNION_NORM_TOLERANCE, QUATERNION_SIGN_TOLERANCE};
pub use validation::{GeometryError, Vec3};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plane_frame_maps_local_coordinates() {
        let frame =
            PlaneFrame::try_new(Point3::new(10.0, 20.0, 30.0), Vector3::x(), Vector3::z()).unwrap();
        let mapped = frame.map_point(Point2::new(2.0, 3.0)).unwrap();

        assert!((mapped - Point3::new(12.0, 20.0, 33.0)).norm() < 1.0e-12);
    }
}
