use std::collections::BTreeSet;

use geosolve_geometry::{
    BSplineBasis, BSplineCurve2, BSplineDefinitionError, BSplineEvaluationError, BSplineForm,
    BSplineSpanIndex, CurveJet2, Point2,
};

use crate::{BSplineId, PointId, Sketch, SketchConstraintKind, SketchError, model::nonempty_label};

/// A runtime non-rational B-spline over ordinary sketch point variables.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineCurve {
    basis: BSplineBasis,
    controls: Vec<PointId>,
    label: String,
}

impl BSplineCurve {
    #[must_use]
    pub const fn basis(&self) -> &BSplineBasis {
        &self.basis
    }

    #[must_use]
    pub fn controls(&self) -> &[PointId] {
        &self.controls
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Sketch {
    /// Adds a validated non-rational B-spline over existing point variables.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale or duplicate control IDs, or invalid knot topology.
    pub fn add_named_bspline(
        &mut self,
        label: impl Into<String>,
        form: BSplineForm,
        degree: u32,
        controls: Vec<PointId>,
        knots: Vec<f64>,
    ) -> Result<BSplineId, SketchError> {
        let label = nonempty_label(label, "B-spline")?;
        let mut unique = BTreeSet::new();
        for control in &controls {
            self.point_position(*control)?;
            if !unique.insert(*control) {
                return Err(SketchError::RepeatedBSplineControl(*control));
            }
        }
        let basis = match form {
            BSplineForm::Clamped => BSplineBasis::try_clamped(degree, controls.len(), knots),
            BSplineForm::Periodic => BSplineBasis::try_periodic(degree, controls.len(), knots),
        }
        .map_err(SketchError::InvalidBSpline)?;
        Ok(self.bsplines.insert(BSplineCurve {
            basis,
            controls,
            label,
        }))
    }

    #[must_use]
    pub fn bspline(&self, spline: BSplineId) -> Option<&BSplineCurve> {
        self.bsplines.get(spline)
    }

    pub fn bsplines(&self) -> impl Iterator<Item = (BSplineId, &BSplineCurve)> {
        self.bsplines.iter()
    }

    /// Removes a B-spline that is not referenced by a constraint.
    ///
    /// # Errors
    ///
    /// Returns a stale-ID or in-use error.
    pub fn remove_bspline(&mut self, spline: BSplineId) -> Result<BSplineCurve, SketchError> {
        if self
            .constraints
            .iter()
            .any(|(_, constraint)| constraint_references_bspline(constraint.kind(), spline))
        {
            return Err(SketchError::BSplineInUse(spline));
        }
        self.bsplines
            .remove(spline)
            .ok_or(SketchError::UnknownBSpline(spline))
    }

    /// Evaluates one selected runtime span in its local `[0, 1]` coordinate.
    ///
    /// # Errors
    ///
    /// Rejects a stale spline/control, invalid span/parameter, or irregular jet.
    pub fn evaluate_bspline(
        &self,
        spline: BSplineId,
        span: BSplineSpanIndex,
        parameter: f64,
    ) -> Result<CurveJet2, SketchError> {
        let curve = self
            .bspline(spline)
            .ok_or(SketchError::UnknownBSpline(spline))?;
        self.bspline_geometry(curve)?
            .jet_on_span(span, parameter)
            .map_err(SketchError::InvalidBSplineEvaluation)
    }

    pub(crate) fn bspline_geometry(
        &self,
        curve: &BSplineCurve,
    ) -> Result<BSplineCurve2, SketchError> {
        let controls = curve
            .controls
            .iter()
            .map(|control| self.point_position(*control))
            .collect::<Result<Vec<Point2<f64>>, _>>()?;
        BSplineCurve2::try_new(curve.basis.clone(), controls).map_err(SketchError::InvalidBSpline)
    }
}

fn constraint_references_bspline(kind: SketchConstraintKind, spline: BSplineId) -> bool {
    let references =
        |curve| matches!(curve, crate::SketchCurve::BSpline { spline: id, .. } if id == spline);
    match kind {
        SketchConstraintKind::PointOnCurve { contact, .. }
        | SketchConstraintKind::LineCurveTangency { contact, .. }
        | SketchConstraintKind::CurveDirection { contact, .. } => references(contact.curve),
        SketchConstraintKind::CurveCurveContact { first, second }
        | SketchConstraintKind::CurveCurveTangency { first, second, .. }
        | SketchConstraintKind::EqualCurvature { first, second, .. }
        | SketchConstraintKind::EndpointContinuity { first, second, .. } => {
            references(first.curve) || references(second.curve)
        }
        _ => false,
    }
}

impl From<BSplineDefinitionError> for SketchError {
    fn from(error: BSplineDefinitionError) -> Self {
        Self::InvalidBSpline(error)
    }
}

impl From<BSplineEvaluationError> for SketchError {
    fn from(error: BSplineEvaluationError) -> Self {
        Self::InvalidBSplineEvaluation(error)
    }
}
