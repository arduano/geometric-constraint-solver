use std::collections::BTreeSet;

use geosolve_geometry::{
    BSplineBasis, BSplineForm, BSplineSpanIndex, CurveJet2, NurbsCurve2, NurbsDefinitionError,
    NurbsEvaluationError, Point2,
};

use crate::{NurbsId, PointId, Sketch, SketchConstraintKind, SketchError, model::nonempty_label};

/// A runtime NURBS over ordinary sketch point variables and positive scalar weights.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve {
    basis: BSplineBasis,
    controls: Vec<PointId>,
    weights: Vec<f64>,
    gauge_index: usize,
    label: String,
}

impl NurbsCurve {
    #[must_use]
    pub const fn basis(&self) -> &BSplineBasis {
        &self.basis
    }

    #[must_use]
    pub fn controls(&self) -> &[PointId] {
        &self.controls
    }

    #[must_use]
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    #[must_use]
    pub const fn gauge_index(&self) -> usize {
        self.gauge_index
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

impl Sketch {
    /// Adds a validated NURBS over existing point variables.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale or duplicate controls, invalid topology,
    /// nonpositive weights, or a gauge index whose weight is not exactly one.
    #[allow(clippy::too_many_arguments)]
    pub fn add_named_nurbs(
        &mut self,
        label: impl Into<String>,
        form: BSplineForm,
        degree: u32,
        controls: Vec<PointId>,
        weights: Vec<f64>,
        gauge_index: usize,
        knots: Vec<f64>,
    ) -> Result<NurbsId, SketchError> {
        let label = nonempty_label(label, "NURBS")?;
        let mut unique = BTreeSet::new();
        let positions = controls
            .iter()
            .map(|control| {
                let position = self.point_position(*control)?;
                if !unique.insert(*control) {
                    return Err(SketchError::RepeatedNurbsControl(*control));
                }
                Ok(position)
            })
            .collect::<Result<Vec<Point2<f64>>, SketchError>>()?;
        let basis = match form {
            BSplineForm::Clamped => BSplineBasis::try_clamped(degree, controls.len(), knots),
            BSplineForm::Periodic => BSplineBasis::try_periodic(degree, controls.len(), knots),
        }
        .map_err(SketchError::InvalidBSpline)?;
        NurbsCurve2::try_new(basis.clone(), positions, weights.clone())
            .map_err(SketchError::InvalidNurbs)?;
        let Some(gauge_weight) = weights.get(gauge_index) else {
            return Err(SketchError::InvalidNurbsGauge { gauge_index });
        };
        if gauge_weight.to_bits() != 1.0f64.to_bits() {
            return Err(SketchError::InvalidNurbsGauge { gauge_index });
        }
        Ok(self.nurbs.insert(NurbsCurve {
            basis,
            controls,
            weights,
            gauge_index,
            label,
        }))
    }

    #[must_use]
    pub fn nurbs(&self, nurbs: NurbsId) -> Option<&NurbsCurve> {
        self.nurbs.get(nurbs)
    }

    pub fn nurbs_curves(&self) -> impl Iterator<Item = (NurbsId, &NurbsCurve)> {
        self.nurbs.iter()
    }

    /// Replaces one non-gauge NURBS weight.
    ///
    /// # Errors
    ///
    /// Rejects stale IDs, the gauge coordinate, an invalid index, or a nonpositive
    /// or non-finite value.
    pub fn set_nurbs_weight(
        &mut self,
        nurbs: NurbsId,
        index: usize,
        weight: f64,
    ) -> Result<(), SketchError> {
        let curve = self
            .nurbs
            .get(nurbs)
            .ok_or(SketchError::UnknownNurbs(nurbs))?;
        if index >= curve.weights.len() {
            return Err(SketchError::InvalidNurbsWeightIndex { nurbs, index });
        }
        if index == curve.gauge_index {
            return Err(SketchError::NurbsGaugeWeightEdit(nurbs));
        }
        if !weight.is_finite() || weight <= 0.0 {
            return Err(SketchError::InvalidNurbsWeight { index, weight });
        }
        let mut weights = curve.weights.clone();
        weights[index] = weight;
        self.replace_nurbs_weights(nurbs, weights)
    }

    pub(crate) fn replace_nurbs_weights(
        &mut self,
        nurbs: NurbsId,
        weights: Vec<f64>,
    ) -> Result<(), SketchError> {
        let curve = self
            .nurbs
            .get(nurbs)
            .ok_or(SketchError::UnknownNurbs(nurbs))?;
        if weights.len() != curve.weights.len()
            || weights
                .get(curve.gauge_index)
                .is_none_or(|weight| weight.to_bits() != 1.0f64.to_bits())
        {
            return Err(SketchError::InvalidNurbsGauge {
                gauge_index: curve.gauge_index,
            });
        }
        let controls = curve
            .controls
            .iter()
            .map(|control| self.point_position(*control))
            .collect::<Result<Vec<_>, _>>()?;
        NurbsCurve2::try_new(curve.basis.clone(), controls, weights.clone())
            .map_err(SketchError::InvalidNurbs)?;
        self.nurbs
            .get_mut(nurbs)
            .ok_or(SketchError::UnknownNurbs(nurbs))?
            .weights = weights;
        Ok(())
    }

    /// Removes a NURBS that is not referenced by a constraint.
    ///
    /// # Errors
    ///
    /// Returns a stale-ID or in-use error.
    pub fn remove_nurbs(&mut self, nurbs: NurbsId) -> Result<NurbsCurve, SketchError> {
        if self
            .constraints
            .iter()
            .any(|(_, constraint)| constraint_references_nurbs(constraint.kind(), nurbs))
        {
            return Err(SketchError::NurbsInUse(nurbs));
        }
        self.nurbs
            .remove(nurbs)
            .ok_or(SketchError::UnknownNurbs(nurbs))
    }

    /// Evaluates one selected runtime span in its local `[0, 1]` coordinate.
    ///
    /// # Errors
    ///
    /// Rejects stale geometry, invalid weights/span/parameter, or an irregular jet.
    pub fn evaluate_nurbs(
        &self,
        nurbs: NurbsId,
        span: BSplineSpanIndex,
        parameter: f64,
    ) -> Result<CurveJet2, SketchError> {
        let curve = self.nurbs(nurbs).ok_or(SketchError::UnknownNurbs(nurbs))?;
        self.nurbs_geometry(curve)?
            .jet_on_span(span, parameter)
            .map_err(SketchError::InvalidNurbsEvaluation)
    }

    pub(crate) fn nurbs_geometry(&self, curve: &NurbsCurve) -> Result<NurbsCurve2, SketchError> {
        let controls = curve
            .controls
            .iter()
            .map(|control| self.point_position(*control))
            .collect::<Result<Vec<Point2<f64>>, _>>()?;
        NurbsCurve2::try_new(curve.basis.clone(), controls, curve.weights.clone())
            .map_err(SketchError::InvalidNurbs)
    }
}

fn constraint_references_nurbs(kind: SketchConstraintKind, nurbs: NurbsId) -> bool {
    let references =
        |curve| matches!(curve, crate::SketchCurve::Nurbs { nurbs: id, .. } if id == nurbs);
    match kind {
        SketchConstraintKind::PointOnCurve { contact, .. }
        | SketchConstraintKind::LineCurveTangency { contact, .. }
        | SketchConstraintKind::CurveDirection { contact, .. } => references(contact.curve),
        SketchConstraintKind::CurveCurveContact { first, second }
        | SketchConstraintKind::CurveCurveTangency { first, second, .. }
        | SketchConstraintKind::EqualCurvature { first, second, .. }
        | SketchConstraintKind::EndpointContinuity { first, second, .. }
        | SketchConstraintKind::CurveCurveFillet { first, second, .. } => {
            references(first.curve) || references(second.curve)
        }
        _ => false,
    }
}

impl From<NurbsDefinitionError> for SketchError {
    fn from(error: NurbsDefinitionError) -> Self {
        Self::InvalidNurbs(error)
    }
}

impl From<NurbsEvaluationError> for SketchError {
    fn from(error: NurbsEvaluationError) -> Self {
        Self::InvalidNurbsEvaluation(error)
    }
}
