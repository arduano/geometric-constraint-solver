use thiserror::Error;

use crate::{
    BSplineBasis, BSplineBasisJet, BSplineDefinitionError, BSplineEvaluationError,
    BSplineInsertionError, BSplineKnotSide, BSplineSpanIndex, CurveEvaluationError, CurveJet2,
    CurveParameterDomain, CurveRegularityError, Point2, Vector2,
};

const RATIONAL_DENOMINATOR_FACTOR: f64 = 64.0 * f64::EPSILON;

/// Invalid immutable NURBS geometry.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum NurbsDefinitionError {
    #[error(transparent)]
    Basis(#[from] BSplineDefinitionError),
    #[error("NURBS basis expects {expected} controls, got {actual}")]
    ControlCount { expected: usize, actual: usize },
    #[error("NURBS basis expects {expected} weights, got {actual}")]
    WeightCount { expected: usize, actual: usize },
    #[error("NURBS control {index} must be finite")]
    NonFiniteControl { index: usize },
    #[error("NURBS weight {index} must be positive and finite, got {weight}")]
    InvalidWeight { index: usize, weight: f64 },
    #[error(
        "active NURBS weights have an unrepresentable ratio on span {span:?}: {minimum} to {maximum}"
    )]
    MixedWeightScale {
        span: BSplineSpanIndex,
        minimum: f64,
        maximum: f64,
    },
    #[error(
        "NURBS control {control} on span {span:?} has an unrepresentable homogeneous coordinate"
    )]
    UnrepresentableHomogeneousControl {
        span: BSplineSpanIndex,
        control: usize,
    },
}

/// Typed immutable NURBS-evaluation failure.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum NurbsEvaluationError {
    #[error(transparent)]
    Basis(#[from] BSplineEvaluationError),
    #[error(transparent)]
    Curve(#[from] CurveEvaluationError),
    #[error(
        "active NURBS weights have an unrepresentable ratio at parameter {parameter}: {minimum} to {maximum}"
    )]
    MixedWeightScale {
        parameter: f64,
        minimum: f64,
        maximum: f64,
    },
    #[error(
        "rational denominator {denominator} is singular or ill-conditioned at parameter {parameter} (condition scale {condition_scale})"
    )]
    RationalDenominator {
        parameter: f64,
        denominator: f64,
        condition_scale: f64,
    },
}

/// Typed NURBS knot-insertion failure.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum NurbsInsertionError {
    #[error(transparent)]
    Basis(#[from] BSplineInsertionError),
    #[error(transparent)]
    Definition(#[from] NurbsDefinitionError),
    #[error("NURBS homogeneous refinement is not finite at output control {index}")]
    NonFiniteRefinement { index: usize },
    #[error("NURBS weights have an unrepresentable ratio: {minimum} to {maximum}")]
    MixedWeightScale { minimum: f64, maximum: f64 },
}

/// Homogeneous source provenance for one refined NURBS control/weight pair.
///
/// A copy preserves the source value exactly. A blend names only its homogeneous
/// contributors; it neither defines ordinary-control interpolation nor assigns a
/// persistent identity policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NurbsControlProvenance {
    Copy { control: usize },
    Blend { first: usize, second: usize },
}

/// A refined immutable NURBS plus rational control provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsRefinement2 {
    curve: NurbsCurve2,
    control_provenance: Vec<NurbsControlProvenance>,
    split_span: Option<BSplineSpanIndex>,
}

impl NurbsRefinement2 {
    #[must_use]
    pub const fn curve(&self) -> &NurbsCurve2 {
        &self.curve
    }

    #[must_use]
    pub fn control_provenance(&self) -> &[NurbsControlProvenance] {
        &self.control_provenance
    }

    #[must_use]
    pub const fn split_span(&self) -> Option<BSplineSpanIndex> {
        self.split_span
    }

    #[must_use]
    pub fn into_curve(self) -> NurbsCurve2 {
        self.curve
    }
}

/// A validated immutable planar non-uniform rational B-spline.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsCurve2 {
    basis: BSplineBasis,
    controls: Vec<Point2<f64>>,
    weights: Vec<f64>,
}

impl NurbsCurve2 {
    /// Associates finite controls and positive finite weights with a validated basis.
    ///
    /// # Errors
    ///
    /// Rejects count mismatches, non-finite controls, and nonpositive or non-finite weights.
    pub fn try_new(
        basis: BSplineBasis,
        controls: Vec<Point2<f64>>,
        weights: Vec<f64>,
    ) -> Result<Self, NurbsDefinitionError> {
        if controls.len() != basis.control_count() {
            return Err(NurbsDefinitionError::ControlCount {
                expected: basis.control_count(),
                actual: controls.len(),
            });
        }
        if weights.len() != basis.control_count() {
            return Err(NurbsDefinitionError::WeightCount {
                expected: basis.control_count(),
                actual: weights.len(),
            });
        }
        for (index, point) in controls.iter().enumerate() {
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(NurbsDefinitionError::NonFiniteControl { index });
            }
        }
        for (index, weight) in weights.iter().copied().enumerate() {
            if !weight.is_finite() || weight <= 0.0 {
                return Err(NurbsDefinitionError::InvalidWeight { index, weight });
            }
        }
        validate_active_homogeneous_scales(&basis, &controls, &weights)?;
        Ok(Self {
            basis,
            controls,
            weights,
        })
    }

    /// Constructs a clamped NURBS from ordinary controls, positive weights, and a complete knot vector.
    ///
    /// # Errors
    ///
    /// Returns a typed definition failure.
    pub fn try_clamped(
        degree: u32,
        controls: Vec<Point2<f64>>,
        weights: Vec<f64>,
        knots: Vec<f64>,
    ) -> Result<Self, NurbsDefinitionError> {
        let basis = BSplineBasis::try_clamped(degree, controls.len(), knots)?;
        Self::try_new(basis, controls, weights)
    }

    /// Constructs a periodic NURBS from unique controls, positive weights, and one-period knots.
    ///
    /// # Errors
    ///
    /// Returns a typed definition failure.
    pub fn try_periodic(
        degree: u32,
        controls: Vec<Point2<f64>>,
        weights: Vec<f64>,
        one_period_knots: Vec<f64>,
    ) -> Result<Self, NurbsDefinitionError> {
        let basis = BSplineBasis::try_periodic(degree, controls.len(), one_period_knots)?;
        Self::try_new(basis, controls, weights)
    }

    #[must_use]
    pub const fn basis(&self) -> &BSplineBasis {
        &self.basis
    }

    #[must_use]
    pub fn controls(&self) -> &[Point2<f64>] {
        &self.controls
    }

    #[must_use]
    pub fn weights(&self) -> &[f64] {
        &self.weights
    }

    /// Evaluates a native-coordinate one-sided NURBS jet.
    ///
    /// # Errors
    ///
    /// Returns a typed parameter, knot-side, weight-scale, denominator, zero-speed,
    /// or finite-evaluation failure.
    pub fn jet_at(
        &self,
        parameter: f64,
        side: BSplineKnotSide,
    ) -> Result<CurveJet2, NurbsEvaluationError> {
        let basis = self.basis.basis_jet_at(parameter, side)?;
        rational_jet_from_basis(
            &basis,
            &self.controls,
            &self.weights,
            self.basis.parameter_domain(),
        )
    }

    /// Evaluates a NURBS jet in one selected span's local `[0, 1]` coordinate.
    ///
    /// # Errors
    ///
    /// Returns a typed span, parameter, weight-scale, denominator, zero-speed, or
    /// finite-evaluation failure.
    pub fn jet_on_span(
        &self,
        span: BSplineSpanIndex,
        local_parameter: f64,
    ) -> Result<CurveJet2, NurbsEvaluationError> {
        let basis = self.basis.basis_jet_on_span(span, local_parameter)?;
        rational_jet_from_basis(
            &basis,
            &self.controls,
            &self.weights,
            CurveParameterDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
    }

    /// Inserts one knot by refining homogeneous controls.
    ///
    /// # Errors
    ///
    /// Returns a typed basis, weight-scale, or homogeneous finite-evaluation failure.
    pub fn insert_knot(&self, parameter: f64) -> Result<NurbsRefinement2, NurbsInsertionError> {
        let refinement = self.basis.insert_knot(parameter)?;

        let mut controls = Vec::with_capacity(refinement.control_stencils().len());
        let mut weights = Vec::with_capacity(refinement.control_stencils().len());
        for (index, stencil) in refinement.control_stencils().iter().copied().enumerate() {
            let Some(second) = stencil.second_control else {
                controls.push(self.controls[stencil.first_control]);
                weights.push(self.weights[stencil.first_control]);
                continue;
            };
            let first_source_weight = self.weights[stencil.first_control];
            let second_source_weight = self.weights[second];
            let scale = first_source_weight.max(second_source_weight);
            let minimum = first_source_weight.min(second_source_weight);
            let first_normalized = first_source_weight / scale;
            let second_normalized = second_source_weight / scale;
            if first_normalized == 0.0 || second_normalized == 0.0 {
                return Err(NurbsInsertionError::MixedWeightScale {
                    minimum,
                    maximum: scale,
                });
            }
            let first_weight = first_normalized * stencil.first_weight;
            let second_weight = second_normalized * stencil.second_weight;
            if (stencil.first_weight != 0.0 && first_weight == 0.0)
                || (stencil.second_weight != 0.0 && second_weight == 0.0)
            {
                return Err(NurbsInsertionError::MixedWeightScale {
                    minimum,
                    maximum: scale,
                });
            }
            let mut homogeneous = self.controls[stencil.first_control].coords * first_weight;
            let mut weight = first_weight;
            homogeneous += self.controls[second].coords * second_weight;
            weight += second_weight;
            let control = homogeneous / weight;
            let restored_weight = weight * scale;
            if !weight.is_finite()
                || weight <= 0.0
                || !control.x.is_finite()
                || !control.y.is_finite()
                || !restored_weight.is_finite()
                || restored_weight <= 0.0
            {
                return Err(NurbsInsertionError::NonFiniteRefinement { index });
            }
            controls.push(Point2::from(control));
            weights.push(restored_weight);
        }

        let curve = Self::try_new(refinement.basis().clone(), controls, weights)?;
        Ok(NurbsRefinement2 {
            curve,
            control_provenance: refinement
                .control_stencils()
                .iter()
                .map(|stencil| match stencil.second_control {
                    Some(second) => NurbsControlProvenance::Blend {
                        first: stencil.first_control,
                        second,
                    },
                    None => NurbsControlProvenance::Copy {
                        control: stencil.first_control,
                    },
                })
                .collect(),
            split_span: refinement.split_span(),
        })
    }
}

fn rational_jet_from_basis(
    basis: &BSplineBasisJet,
    controls: &[Point2<f64>],
    weights: &[f64],
    domain: CurveParameterDomain,
) -> Result<CurveJet2, NurbsEvaluationError> {
    let active_weights = basis
        .terms
        .iter()
        .map(|term| weights[term.control_index])
        .collect::<Vec<_>>();
    let (minimum, maximum) = weight_range(&active_weights);
    let normalized_weights = active_weights
        .iter()
        .map(|weight| weight / maximum)
        .collect::<Vec<_>>();
    if normalized_weights.contains(&0.0) {
        return Err(NurbsEvaluationError::MixedWeightScale {
            parameter: basis.native_parameter,
            minimum,
            maximum,
        });
    }
    let reference = controls[basis.terms[0].control_index];
    let mut position_numerator = Vector2::zeros();
    let mut denominators = [0.0; 4];
    let mut condition_scale = 0.0;

    for (term, normalized_weight) in basis.terms.iter().zip(&normalized_weights) {
        let offset = controls[term.control_index] - reference;
        let weighted_offset = offset * *normalized_weight;
        if (offset.x != 0.0 && weighted_offset.x == 0.0)
            || (offset.y != 0.0 && weighted_offset.y == 0.0)
        {
            return Err(NurbsEvaluationError::MixedWeightScale {
                parameter: basis.native_parameter,
                minimum,
                maximum,
            });
        }
        for (order, derivative) in term.derivatives.into_iter().enumerate() {
            let coefficient = derivative * normalized_weight;
            denominators[order] += coefficient;
            if order == 0 {
                position_numerator += weighted_offset * derivative;
                condition_scale += coefficient.abs();
            }
        }
    }

    if !condition_scale.is_finite()
        || !denominators.into_iter().all(f64::is_finite)
        || !position_numerator.x.is_finite()
        || !position_numerator.y.is_finite()
    {
        return Err(NurbsEvaluationError::Curve(
            CurveRegularityError::NonFiniteJet.into(),
        ));
    }
    let denominator = denominators[0];
    if denominator <= RATIONAL_DENOMINATOR_FACTOR * condition_scale {
        return Err(NurbsEvaluationError::RationalDenominator {
            parameter: basis.native_parameter,
            denominator,
            condition_scale,
        });
    }

    let position_offset = position_numerator / denominator;
    let position = reference + position_offset;
    let centered_first =
        pairwise_rational_numerator(basis, controls, &normalized_weights, 1, minimum, maximum)?
            / denominator;
    let first = centered_first / denominator;
    let centered_second =
        pairwise_rational_numerator(basis, controls, &normalized_weights, 2, minimum, maximum)?
            / denominator;
    let second = (centered_second - first * (2.0 * denominators[1])) / denominator;
    let centered_third =
        pairwise_rational_numerator(basis, controls, &normalized_weights, 3, minimum, maximum)?
            / denominator;
    let third =
        (centered_third - first * (3.0 * denominators[2]) - second * (3.0 * denominators[1]))
            / denominator;
    checked_rational_jet(CurveJet2 {
        position: Point2::from(position),
        first_derivative: first,
        second_derivative: second,
        third_derivative: third,
        domain,
    })
}

fn checked_rational_jet(jet: CurveJet2) -> Result<CurveJet2, NurbsEvaluationError> {
    let finite = jet.position.x.is_finite()
        && jet.position.y.is_finite()
        && jet.first_derivative.x.is_finite()
        && jet.first_derivative.y.is_finite()
        && jet.second_derivative.x.is_finite()
        && jet.second_derivative.y.is_finite()
        && jet.third_derivative.x.is_finite()
        && jet.third_derivative.y.is_finite();
    if !finite {
        Err(NurbsEvaluationError::Curve(
            CurveRegularityError::NonFiniteJet.into(),
        ))
    } else if jet.first_derivative.x.hypot(jet.first_derivative.y) == 0.0 {
        Err(NurbsEvaluationError::Curve(
            CurveRegularityError::ZeroSpeed.into(),
        ))
    } else {
        Ok(jet)
    }
}

fn pairwise_rational_numerator(
    basis: &BSplineBasisJet,
    controls: &[Point2<f64>],
    normalized_weights: &[f64],
    order: usize,
    minimum: f64,
    maximum: f64,
) -> Result<Vector2<f64>, NurbsEvaluationError> {
    let mut numerator = Vector2::zeros();
    for first in 0..basis.terms.len() {
        for second in first + 1..basis.terms.len() {
            let first_term = basis.terms[first];
            let second_term = basis.terms[second];
            let weight_product = normalized_weights[first] * normalized_weights[second];
            if weight_product == 0.0 {
                return Err(NurbsEvaluationError::MixedWeightScale {
                    parameter: basis.native_parameter,
                    minimum,
                    maximum,
                });
            }
            let basis_cross = first_term.derivatives[order].mul_add(
                second_term.derivatives[0],
                -second_term.derivatives[order] * first_term.derivatives[0],
            );
            let difference =
                controls[first_term.control_index] - controls[second_term.control_index];
            let weighted_difference = difference * weight_product;
            if (difference.x != 0.0 && weighted_difference.x == 0.0)
                || (difference.y != 0.0 && weighted_difference.y == 0.0)
            {
                return Err(NurbsEvaluationError::MixedWeightScale {
                    parameter: basis.native_parameter,
                    minimum,
                    maximum,
                });
            }
            numerator += weighted_difference * basis_cross;
        }
    }
    if numerator.x.is_finite() && numerator.y.is_finite() {
        Ok(numerator)
    } else {
        Err(NurbsEvaluationError::Curve(
            CurveRegularityError::NonFiniteJet.into(),
        ))
    }
}

fn weight_range(weights: &[f64]) -> (f64, f64) {
    let mut minimum = f64::INFINITY;
    let mut maximum: f64 = 0.0;
    for weight in weights {
        minimum = minimum.min(*weight);
        maximum = maximum.max(*weight);
    }
    (minimum, maximum)
}

fn validate_active_homogeneous_scales(
    basis: &BSplineBasis,
    controls: &[Point2<f64>],
    weights: &[f64],
) -> Result<(), NurbsDefinitionError> {
    for span in basis.spans() {
        let active_weights = span
            .support()
            .iter()
            .map(|index| weights[*index])
            .collect::<Vec<_>>();
        let (minimum, maximum) = weight_range(&active_weights);
        if minimum / maximum == 0.0 {
            return Err(NurbsDefinitionError::MixedWeightScale {
                span: span.index(),
                minimum,
                maximum,
            });
        }
        let normalized = active_weights
            .iter()
            .map(|weight| weight / maximum)
            .collect::<Vec<_>>();
        for first in 0..normalized.len() {
            for second in first + 1..normalized.len() {
                if normalized[first] * normalized[second] == 0.0 {
                    return Err(NurbsDefinitionError::MixedWeightScale {
                        span: span.index(),
                        minimum,
                        maximum,
                    });
                }
            }
        }
        for control in span.support().iter().copied() {
            let normalized_weight = weights[control] / maximum;
            let weighted = controls[control].coords * normalized_weight;
            if (controls[control].x != 0.0 && weighted.x == 0.0)
                || (controls[control].y != 0.0 && weighted.y == 0.0)
            {
                return Err(NurbsDefinitionError::UnrepresentableHomogeneousControl {
                    span: span.index(),
                    control,
                });
            }
        }
    }
    Ok(())
}
