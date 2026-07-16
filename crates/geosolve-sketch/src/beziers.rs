use geosolve_geometry::{
    CurveEvaluationError, CurveJet2, Point2, cubic_bezier_jet, quadratic_bezier_jet,
};

use crate::{
    BezierId, CurveTangentOrientation, PointId, SegmentEndpoint, SegmentId, Sketch,
    SketchConstraintId, SketchConstraintKind, SketchError,
};

/// Closed editable Bezier topology over ordinary sketch points.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BezierKind {
    Quadratic { controls: [PointId; 3] },
    Cubic { controls: [PointId; 4] },
}

/// One quadratic or cubic Bezier entity.
#[derive(Clone, Debug, PartialEq)]
pub struct BezierCurve {
    kind: BezierKind,
    label: String,
}

impl BezierCurve {
    #[must_use]
    pub const fn kind(&self) -> BezierKind {
        self.kind
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn controls(&self) -> &[PointId] {
        match &self.kind {
            BezierKind::Quadratic { controls } => controls,
            BezierKind::Cubic { controls } => controls,
        }
    }
}

impl Sketch {
    /// Adds a quadratic Bezier whose editable controls are ordinary sketch points.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty label or stale control ID.
    pub fn add_quadratic_bezier(
        &mut self,
        label: impl Into<String>,
        controls: [PointId; 3],
    ) -> Result<BezierId, SketchError> {
        self.add_bezier(label, BezierKind::Quadratic { controls })
    }

    /// Adds a cubic Bezier whose editable controls are ordinary sketch points.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty label or stale control ID.
    pub fn add_cubic_bezier(
        &mut self,
        label: impl Into<String>,
        controls: [PointId; 4],
    ) -> Result<BezierId, SketchError> {
        self.add_bezier(label, BezierKind::Cubic { controls })
    }

    #[must_use]
    pub fn bezier(&self, id: BezierId) -> Option<&BezierCurve> {
        self.beziers.get(id)
    }

    pub fn beziers(&self) -> impl Iterator<Item = (BezierId, &BezierCurve)> {
        self.beziers.iter()
    }

    /// Evaluates the current accepted control geometry.
    ///
    /// # Errors
    ///
    /// Returns a stale-ID error or a typed domain/regularity failure.
    pub fn evaluate_bezier(
        &self,
        id: BezierId,
        parameter: f64,
    ) -> Result<CurveJet2, BezierEvaluationError> {
        let curve =
            self.bezier(id)
                .ok_or(BezierEvaluationError::Sketch(SketchError::UnknownBezier(
                    id,
                )))?;
        match curve.kind {
            BezierKind::Quadratic {
                controls: [first, second, third],
            } => Ok(quadratic_bezier_jet(
                [
                    self.control_position(first)?,
                    self.control_position(second)?,
                    self.control_position(third)?,
                ],
                parameter,
            )?),
            BezierKind::Cubic {
                controls: [first, second, third, fourth],
            } => Ok(cubic_bezier_jet(
                [
                    self.control_position(first)?,
                    self.control_position(second)?,
                    self.control_position(third)?,
                    self.control_position(fourth)?,
                ],
                parameter,
            )?),
        }
    }

    /// Removes an unreferenced Bezier and leaves its runtime ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale or referenced Bezier.
    pub fn remove_bezier(&mut self, id: BezierId) -> Result<BezierCurve, SketchError> {
        if self.constraint_references_bezier(id) {
            return Err(SketchError::BezierInUse(id));
        }
        self.beziers
            .remove(id)
            .ok_or(SketchError::UnknownBezier(id))
    }

    /// Adds a point-on-Bezier source with one bounded latent parameter.
    ///
    /// # Errors
    ///
    /// Returns an error for stale IDs or a parameter outside `[0, 1]`.
    pub fn add_point_on_bezier(
        &mut self,
        point: PointId,
        bezier: BezierId,
        parameter: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point(point).ok_or(SketchError::UnknownPoint(point))?;
        self.bezier(bezier)
            .ok_or(SketchError::UnknownBezier(bezier))?;
        validate_parameter(parameter)?;
        Ok(self.insert_constraint(SketchConstraintKind::PointOnBezier {
            point,
            bezier,
            parameter,
        }))
    }

    /// Adds contact and tangent alignment between one line endpoint and a Bezier location.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/degenerate geometry or a parameter outside `[0, 1]`.
    pub fn add_line_bezier_tangency(
        &mut self,
        line: SegmentId,
        endpoint: SegmentEndpoint,
        bezier: BezierId,
        bezier_parameter: f64,
        orientation: CurveTangentOrientation,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(line)?;
        self.bezier(bezier)
            .ok_or(SketchError::UnknownBezier(bezier))?;
        validate_parameter(bezier_parameter)?;
        Ok(
            self.insert_constraint(SketchConstraintKind::LineBezierTangency {
                line,
                endpoint,
                bezier,
                bezier_parameter,
                orientation,
            }),
        )
    }

    fn add_bezier(
        &mut self,
        label: impl Into<String>,
        kind: BezierKind,
    ) -> Result<BezierId, SketchError> {
        for control in kind.controls() {
            self.point(*control)
                .ok_or(SketchError::UnknownPoint(*control))?;
        }
        let label = label.into();
        if label.trim().is_empty() {
            return Err(SketchError::EmptyLabel("Bezier"));
        }
        Ok(self.beziers.insert(BezierCurve { kind, label }))
    }

    fn control_position(&self, point: PointId) -> Result<Point2<f64>, BezierEvaluationError> {
        self.point(point)
            .map(crate::SketchPoint::position)
            .ok_or(BezierEvaluationError::Sketch(SketchError::UnknownPoint(
                point,
            )))
    }

    pub(crate) fn constraint_references_bezier(&self, id: BezierId) -> bool {
        self.constraints().any(|(_, constraint)| {
            matches!(
                constraint.kind(),
                SketchConstraintKind::PointOnBezier { bezier, .. }
                    | SketchConstraintKind::LineBezierTangency { bezier, .. }
                    if bezier == id
            ) || match constraint.kind() {
                SketchConstraintKind::PointOnCurve { contact, .. }
                | SketchConstraintKind::LineCurveTangency { contact, .. } => {
                    matches!(contact.curve, crate::SketchCurve::Bezier(bezier) if bezier == id)
                }
                SketchConstraintKind::CurveCurveContact { first, second }
                | SketchConstraintKind::CurveCurveTangency { first, second, .. } => [first, second]
                    .into_iter()
                    .any(|contact| matches!(contact.curve, crate::SketchCurve::Bezier(bezier) if bezier == id)),
                _ => false,
            }
        })
    }
}

fn validate_parameter(parameter: f64) -> Result<(), SketchError> {
    if parameter.is_finite() && (0.0..=1.0).contains(&parameter) {
        Ok(())
    } else {
        Err(SketchError::ParameterOutOfDomain {
            parameter,
            domain: "bounded Bezier span [0, 1]",
        })
    }
}

impl BezierKind {
    fn controls(&self) -> &[PointId] {
        match self {
            Self::Quadratic { controls } => controls,
            Self::Cubic { controls } => controls,
        }
    }
}

/// Public Bezier evaluation failure preserving geometry's typed error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum BezierEvaluationError {
    #[error(transparent)]
    Sketch(#[from] SketchError),
    #[error(transparent)]
    Curve(#[from] CurveEvaluationError),
}
