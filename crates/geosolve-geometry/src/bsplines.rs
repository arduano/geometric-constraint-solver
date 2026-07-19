#![allow(clippy::float_cmp)] // Knot identity and one-sided limits use exact stored values.

use thiserror::Error;

use crate::{
    CurveEvaluationError, CurveJet2, CurveParameterDomain, CurveParameterError,
    CurveRegularityError, Point2, Vector2,
};

/// Defensive degree limit keeping basis storage and evaluation predictably bounded.
pub const MAX_BSPLINE_DEGREE: u32 = 64;

/// The serialized knot topology of a non-rational B-spline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BSplineForm {
    Clamped,
    Periodic,
}

/// The selected limit when evaluating exactly at a knot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BSplineKnotSide {
    Left,
    Right,
}

/// An immutable-basis-local span index.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BSplineSpanIndex(u32);

impl BSplineSpanIndex {
    /// Returns the positive-span ordinal in this immutable basis.
    #[must_use]
    pub const fn ordinal(self) -> u32 {
        self.0
    }
}

/// One positive knot interval and its local control support.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineSpan {
    index: BSplineSpanIndex,
    lower: f64,
    upper: f64,
    support: Vec<usize>,
    raw_span: isize,
}

impl BSplineSpan {
    #[must_use]
    pub const fn index(&self) -> BSplineSpanIndex {
        self.index
    }

    #[must_use]
    pub const fn lower(&self) -> f64 {
        self.lower
    }

    #[must_use]
    pub const fn upper(&self) -> f64 {
        self.upper
    }

    /// Returns exactly `degree + 1` active control indices in basis order.
    #[must_use]
    pub fn support(&self) -> &[usize] {
        &self.support
    }
}

/// One active basis function and its parameter derivatives through order three.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BSplineBasisTerm {
    pub control_index: usize,
    pub derivatives: [f64; 4],
}

/// Basis values and derivatives for exactly one selected span.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineBasisJet {
    pub span: BSplineSpanIndex,
    pub native_parameter: f64,
    pub terms: Vec<BSplineBasisTerm>,
}

/// Guaranteed parametric continuity derived from knot multiplicity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BSplineContinuity {
    Boundary,
    Guaranteed { multiplicity: u32, order: u32 },
}

/// Invalid immutable B-spline topology.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum BSplineDefinitionError {
    #[error("B-spline degree must be at least one, got {degree}")]
    InvalidDegree { degree: u32 },
    #[error("B-spline degree {actual} exceeds the supported limit {limit}")]
    DegreeLimit { actual: u32, limit: u32 },
    #[error("degree {degree} requires at least {minimum} controls, got {actual}")]
    InsufficientControls {
        degree: u32,
        minimum: usize,
        actual: usize,
    },
    #[error("B-spline basis expects {expected} controls, got {actual}")]
    ControlCount { expected: usize, actual: usize },
    #[error("B-spline knot count must be {expected}, got {actual}")]
    KnotCount { expected: usize, actual: usize },
    #[error("B-spline knot {index} must be finite, got {value}")]
    NonFiniteKnot { index: usize, value: f64 },
    #[error("B-spline knots decrease at indices {first} and {second}: {lower} > {upper}")]
    DecreasingKnots {
        first: usize,
        second: usize,
        lower: f64,
        upper: f64,
    },
    #[error("clamped B-spline endpoints must each have multiplicity degree + 1")]
    InvalidClamping,
    #[error("periodic B-spline one-period knots must begin at zero, got {origin}")]
    InvalidPeriodicOrigin { origin: f64 },
    #[error("periodic B-spline period must be positive and finite, got {period}")]
    InvalidPeriod { period: f64 },
    #[error("knot {parameter} has multiplicity {multiplicity}, above maximum {maximum}")]
    KnotMultiplicity {
        parameter: f64,
        multiplicity: usize,
        maximum: usize,
    },
    #[error("B-spline active parameter domain must have positive finite length")]
    EmptyDomain,
    #[error("B-spline control {index} must be finite")]
    NonFiniteControl { index: usize },
    #[error("B-spline count arithmetic exceeds the supported representation")]
    CountOverflow,
}

/// Typed B-spline evaluation failure.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum BSplineEvaluationError {
    #[error(transparent)]
    Curve(#[from] CurveEvaluationError),
    #[error("B-spline span index {ordinal} does not belong to this basis")]
    InvalidSpan { ordinal: u32 },
    #[error("span-local B-spline parameter must belong to [0, 1], got {parameter}")]
    InvalidLocalParameter { parameter: f64 },
    #[error("the {side:?} knot side is unavailable at parameter {parameter}")]
    UnavailableKnotSide {
        parameter: f64,
        side: BSplineKnotSide,
    },
    #[error("B-spline basis evaluation produced a non-finite value")]
    NonFiniteBasis,
    #[error("knot {parameter} guarantees C{available}, below requested C{required} continuity")]
    InsufficientContinuity {
        parameter: f64,
        required: u32,
        available: u32,
    },
    #[error("parameter {parameter} is a clamped B-spline boundary, not an interior knot")]
    BoundaryContinuity { parameter: f64 },
}

/// A control coefficient in a geometry-preserving knot refinement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BSplineControlStencil {
    pub first_control: usize,
    pub first_weight: f64,
    pub second_control: Option<usize>,
    pub second_weight: f64,
}

impl BSplineControlStencil {
    fn copy(control: usize) -> Self {
        Self {
            first_control: control,
            first_weight: 1.0,
            second_control: None,
            second_weight: 0.0,
        }
    }

    fn blend(first: usize, first_weight: f64, second: usize, second_weight: f64) -> Self {
        Self {
            first_control: first,
            first_weight,
            second_control: Some(second),
            second_weight,
        }
    }

    fn evaluate(self, controls: &[Point2<f64>]) -> Point2<f64> {
        let mut value = controls[self.first_control].coords * self.first_weight;
        if let Some(second) = self.second_control {
            value += controls[second].coords * self.second_weight;
        }
        Point2::from(value)
    }
}

/// A refined immutable basis plus coefficient provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineBasisRefinement {
    basis: BSplineBasis,
    control_stencils: Vec<BSplineControlStencil>,
    split_span: Option<BSplineSpanIndex>,
}

impl BSplineBasisRefinement {
    #[must_use]
    pub const fn basis(&self) -> &BSplineBasis {
        &self.basis
    }

    #[must_use]
    pub fn control_stencils(&self) -> &[BSplineControlStencil] {
        &self.control_stencils
    }

    #[must_use]
    pub const fn split_span(&self) -> Option<BSplineSpanIndex> {
        self.split_span
    }

    #[must_use]
    pub fn into_basis(self) -> BSplineBasis {
        self.basis
    }
}

/// A refined immutable curve plus coefficient provenance.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineRefinement2 {
    curve: BSplineCurve2,
    control_stencils: Vec<BSplineControlStencil>,
    split_span: Option<BSplineSpanIndex>,
}

impl BSplineRefinement2 {
    #[must_use]
    pub const fn curve(&self) -> &BSplineCurve2 {
        &self.curve
    }

    #[must_use]
    pub fn control_stencils(&self) -> &[BSplineControlStencil] {
        &self.control_stencils
    }

    #[must_use]
    pub const fn split_span(&self) -> Option<BSplineSpanIndex> {
        self.split_span
    }

    #[must_use]
    pub fn into_curve(self) -> BSplineCurve2 {
        self.curve
    }
}

/// Typed knot-insertion failure.
#[derive(Clone, Copy, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum BSplineInsertionError {
    #[error("knot insertion parameter must be finite, got {parameter}")]
    NonFiniteParameter { parameter: f64 },
    #[error("clamped knot insertion must be strictly inside the native domain, got {parameter}")]
    ClampedEndpoint { parameter: f64 },
    #[error(
        "knot {parameter} already has maximum connected multiplicity {multiplicity} for degree {degree}"
    )]
    MaximumMultiplicity {
        parameter: f64,
        multiplicity: usize,
        degree: u32,
    },
    #[error(transparent)]
    Evaluation(#[from] BSplineEvaluationError),
    #[error(transparent)]
    Definition(#[from] BSplineDefinitionError),
}

/// A validated non-rational B-spline basis.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineBasis {
    form: BSplineForm,
    degree_u32: u32,
    degree: usize,
    control_count: usize,
    knots: Vec<f64>,
    domain: CurveParameterDomain,
    spans: Vec<BSplineSpan>,
}

impl BSplineBasis {
    /// Constructs a clamped basis from a complete knot vector.
    ///
    /// # Errors
    ///
    /// Rejects an invalid degree/count, non-finite or decreasing knots, malformed
    /// endpoint multiplicity, excessive interior multiplicity, or empty domain.
    pub fn try_clamped(
        degree: u32,
        control_count: usize,
        knots: Vec<f64>,
    ) -> Result<Self, BSplineDefinitionError> {
        let degree_usize = validate_degree_and_controls(degree, control_count)?;
        let expected = control_count
            .checked_add(degree_usize)
            .and_then(|value| value.checked_add(1))
            .ok_or(BSplineDefinitionError::CountOverflow)?;
        if knots.len() != expected {
            return Err(BSplineDefinitionError::KnotCount {
                expected,
                actual: knots.len(),
            });
        }
        validate_knots(&knots)?;

        let lower = knots[degree_usize];
        let upper = knots[control_count];
        if !lower.is_finite()
            || !upper.is_finite()
            || lower >= upper
            || !(upper - lower).is_finite()
        {
            return Err(BSplineDefinitionError::EmptyDomain);
        }
        let lower_count = knots.iter().take_while(|value| **value == lower).count();
        let upper_count = knots
            .iter()
            .rev()
            .take_while(|value| **value == upper)
            .count();
        if lower_count != degree_usize + 1 || upper_count != degree_usize + 1 {
            return Err(BSplineDefinitionError::InvalidClamping);
        }
        validate_multiplicities(&knots[(degree_usize + 1)..control_count], degree_usize)?;

        let domain = CurveParameterDomain::Bounded { lower, upper };
        let spans = build_clamped_spans(degree_usize, control_count, &knots)?;
        if spans.is_empty() {
            return Err(BSplineDefinitionError::EmptyDomain);
        }
        Ok(Self {
            form: BSplineForm::Clamped,
            degree_u32: degree,
            degree: degree_usize,
            control_count,
            knots,
            domain,
            spans,
        })
    }

    /// Constructs a periodic basis from unique cyclic controls and one-period knots.
    ///
    /// # Errors
    ///
    /// Rejects invalid counts, a nonzero origin, invalid period, malformed knot
    /// ordering or multiplicity, or a period without a positive span.
    pub fn try_periodic(
        degree: u32,
        control_count: usize,
        one_period_knots: Vec<f64>,
    ) -> Result<Self, BSplineDefinitionError> {
        let degree_usize = validate_degree_and_controls(degree, control_count)?;
        let expected = control_count
            .checked_add(1)
            .ok_or(BSplineDefinitionError::CountOverflow)?;
        if one_period_knots.len() != expected {
            return Err(BSplineDefinitionError::KnotCount {
                expected,
                actual: one_period_knots.len(),
            });
        }
        validate_knots(&one_period_knots)?;
        let origin = one_period_knots[0];
        if origin != 0.0 {
            return Err(BSplineDefinitionError::InvalidPeriodicOrigin { origin });
        }
        let period = one_period_knots[control_count];
        if !period.is_finite() || period <= 0.0 || one_period_knots[control_count - 1] >= period {
            return Err(BSplineDefinitionError::InvalidPeriod { period });
        }
        validate_multiplicities(&one_period_knots[..control_count], degree_usize)?;

        let domain = CurveParameterDomain::Periodic { period };
        let spans = build_periodic_spans(degree_usize, control_count, &one_period_knots)?;
        if spans.is_empty() {
            return Err(BSplineDefinitionError::EmptyDomain);
        }
        Ok(Self {
            form: BSplineForm::Periodic,
            degree_u32: degree,
            degree: degree_usize,
            control_count,
            knots: one_period_knots,
            domain,
            spans,
        })
    }

    #[must_use]
    pub const fn form(&self) -> BSplineForm {
        self.form
    }

    #[must_use]
    pub const fn degree(&self) -> u32 {
        self.degree_u32
    }

    #[must_use]
    pub const fn control_count(&self) -> usize {
        self.control_count
    }

    #[must_use]
    pub const fn parameter_domain(&self) -> CurveParameterDomain {
        self.domain
    }

    /// Returns the complete clamped knot vector or periodic one-period knot vector.
    #[must_use]
    pub fn knots(&self) -> &[f64] {
        &self.knots
    }

    #[must_use]
    pub fn spans(&self) -> &[BSplineSpan] {
        &self.spans
    }

    #[must_use]
    pub fn span(&self, index: BSplineSpanIndex) -> Option<&BSplineSpan> {
        usize::try_from(index.0)
            .ok()
            .and_then(|ordinal| self.spans.get(ordinal))
    }

    /// Resolves a finite native parameter and explicit knot side to a positive span.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or out-of-domain parameters and unavailable clamped
    /// endpoint sides.
    pub fn locate_span(
        &self,
        parameter: f64,
        side: BSplineKnotSide,
    ) -> Result<BSplineSpanIndex, BSplineEvaluationError> {
        let parameter = self.normalized_parameter(parameter)?;
        if self.form == BSplineForm::Clamped {
            let CurveParameterDomain::Bounded { lower, upper } = self.domain else {
                unreachable!("clamped basis has a bounded domain")
            };
            if parameter == lower && side == BSplineKnotSide::Left
                || parameter == upper && side == BSplineKnotSide::Right
            {
                return Err(BSplineEvaluationError::UnavailableKnotSide { parameter, side });
            }
        }

        let seam_left =
            self.form == BSplineForm::Periodic && parameter == 0.0 && side == BSplineKnotSide::Left;
        let selected_parameter = if seam_left {
            self.period()
                .ok_or(BSplineEvaluationError::NonFiniteBasis)?
        } else {
            parameter
        };
        let span =
            match side {
                BSplineKnotSide::Left => self.spans.iter().rev().find(|span| {
                    span.lower < selected_parameter && selected_parameter <= span.upper
                }),
                BSplineKnotSide::Right => self.spans.iter().find(|span| {
                    span.lower <= selected_parameter && selected_parameter < span.upper
                }),
            };
        span.map(BSplineSpan::index).ok_or_else(|| {
            BSplineEvaluationError::Curve(
                CurveParameterError::OutOfDomain {
                    parameter: selected_parameter,
                    domain: self.domain,
                }
                .into(),
            )
        })
    }

    /// Evaluates active basis functions in native knot coordinates.
    ///
    /// # Errors
    ///
    /// Returns a typed parameter, side, span or finite-evaluation failure.
    pub fn basis_jet_at(
        &self,
        parameter: f64,
        side: BSplineKnotSide,
    ) -> Result<BSplineBasisJet, BSplineEvaluationError> {
        let normalized = self.normalized_parameter(parameter)?;
        let span_index = self.locate_span(parameter, side)?;
        let native = if self.form == BSplineForm::Periodic
            && normalized == 0.0
            && side == BSplineKnotSide::Left
        {
            self.period()
                .ok_or(BSplineEvaluationError::NonFiniteBasis)?
        } else {
            normalized
        };
        self.basis_jet(span_index, native, 1.0)
    }

    /// Evaluates active basis functions in a selected span-local `[0, 1]` coordinate.
    ///
    /// Returned derivatives are with respect to the local coordinate.
    ///
    /// # Errors
    ///
    /// Rejects an invalid span, escaped local parameter, or non-finite basis jet.
    pub fn basis_jet_on_span(
        &self,
        span_index: BSplineSpanIndex,
        local_parameter: f64,
    ) -> Result<BSplineBasisJet, BSplineEvaluationError> {
        if !local_parameter.is_finite() || !(0.0..=1.0).contains(&local_parameter) {
            return Err(BSplineEvaluationError::InvalidLocalParameter {
                parameter: local_parameter,
            });
        }
        let span = self
            .span(span_index)
            .ok_or(BSplineEvaluationError::InvalidSpan {
                ordinal: span_index.0,
            })?;
        let width = span.upper - span.lower;
        let native = if local_parameter == 0.0 {
            span.lower
        } else if local_parameter == 1.0 {
            span.upper
        } else {
            width.mul_add(local_parameter, span.lower)
        };
        self.basis_jet(span_index, native, width)
    }

    /// Returns guaranteed continuity when the native parameter is exactly a knot.
    ///
    /// Non-knot parameters return `None`.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or out-of-domain parameters.
    pub fn continuity_at(
        &self,
        parameter: f64,
    ) -> Result<Option<BSplineContinuity>, BSplineEvaluationError> {
        let normalized = self.normalized_parameter(parameter)?;
        if self.form == BSplineForm::Clamped {
            let CurveParameterDomain::Bounded { lower, upper } = self.domain else {
                unreachable!("clamped basis has a bounded domain")
            };
            if normalized == lower || normalized == upper {
                return Ok(Some(BSplineContinuity::Boundary));
            }
        }
        let knots = if self.form == BSplineForm::Periodic {
            &self.knots[..self.control_count]
        } else {
            &self.knots
        };
        let multiplicity = knots.iter().filter(|knot| **knot == normalized).count();
        if multiplicity == 0 {
            return Ok(None);
        }
        let order = self.degree - multiplicity;
        Ok(Some(BSplineContinuity::Guaranteed {
            multiplicity: u32::try_from(multiplicity)
                .map_err(|_| BSplineEvaluationError::NonFiniteBasis)?,
            order: u32::try_from(order).map_err(|_| BSplineEvaluationError::NonFiniteBasis)?,
        }))
    }

    /// Requires topology to guarantee at least the requested continuity at a knot.
    ///
    /// # Errors
    ///
    /// Rejects boundaries and knots whose multiplicity guarantees a lower order.
    pub fn require_continuity(
        &self,
        parameter: f64,
        required: u32,
    ) -> Result<(), BSplineEvaluationError> {
        match self.continuity_at(parameter)? {
            None => Ok(()),
            Some(BSplineContinuity::Boundary) => {
                Err(BSplineEvaluationError::BoundaryContinuity { parameter })
            }
            Some(BSplineContinuity::Guaranteed { order, .. }) if order >= required => Ok(()),
            Some(BSplineContinuity::Guaranteed { order, .. }) => {
                Err(BSplineEvaluationError::InsufficientContinuity {
                    parameter,
                    required,
                    available: order,
                })
            }
        }
    }

    /// Inserts one connected knot and returns basis coefficient stencils.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or clamped-endpoint parameters and multiplicity above
    /// the connected-curve maximum.
    pub fn insert_knot(
        &self,
        parameter: f64,
    ) -> Result<BSplineBasisRefinement, BSplineInsertionError> {
        if !parameter.is_finite() {
            return Err(BSplineInsertionError::NonFiniteParameter { parameter });
        }
        match self.form {
            BSplineForm::Clamped => self.insert_clamped_knot(parameter),
            BSplineForm::Periodic => self.insert_periodic_knot(parameter),
        }
    }

    fn insert_clamped_knot(
        &self,
        parameter: f64,
    ) -> Result<BSplineBasisRefinement, BSplineInsertionError> {
        let CurveParameterDomain::Bounded { lower, upper } = self.domain else {
            unreachable!("clamped B-spline has bounded domain")
        };
        if parameter <= lower || parameter >= upper {
            return Err(BSplineInsertionError::ClampedEndpoint { parameter });
        }
        let multiplicity = self.knots.iter().filter(|knot| **knot == parameter).count();
        if multiplicity >= self.degree {
            return Err(BSplineInsertionError::MaximumMultiplicity {
                parameter,
                multiplicity,
                degree: self.degree(),
            });
        }
        let split_span = if multiplicity == 0 {
            Some(self.locate_span(parameter, BSplineKnotSide::Right)?)
        } else {
            None
        };
        let span = self.locate_span(parameter, BSplineKnotSide::Right)?;
        let raw_span = usize::try_from(
            self.span(span)
                .expect("located span belongs to basis")
                .raw_span,
        )
        .expect("clamped raw span is nonnegative");
        let new_count = self
            .control_count
            .checked_add(1)
            .ok_or(BSplineDefinitionError::CountOverflow)?;
        let mut stencils = vec![None; new_count];
        for (index, slot) in stencils
            .iter_mut()
            .enumerate()
            .take(raw_span - self.degree + 1)
        {
            *slot = Some(BSplineControlStencil::copy(index));
        }
        for (index, slot) in stencils
            .iter_mut()
            .enumerate()
            .take(raw_span - multiplicity + 1)
            .skip(raw_span - self.degree + 1)
        {
            let denominator = self.knots[index + self.degree] - self.knots[index];
            let alpha = (parameter - self.knots[index]) / denominator;
            *slot = Some(BSplineControlStencil::blend(
                index - 1,
                1.0 - alpha,
                index,
                alpha,
            ));
        }
        for old_index in (raw_span - multiplicity)..self.control_count {
            stencils[old_index + 1] = Some(BSplineControlStencil::copy(old_index));
        }
        let control_stencils = collect_stencils(stencils)?;
        let mut knots = self.knots.clone();
        knots.insert(raw_span + 1, parameter);
        let basis = Self::try_clamped(self.degree(), new_count, knots)?;
        Ok(BSplineBasisRefinement {
            basis,
            control_stencils,
            split_span,
        })
    }

    fn insert_periodic_knot(
        &self,
        parameter: f64,
    ) -> Result<BSplineBasisRefinement, BSplineInsertionError> {
        let period = self.period().expect("periodic basis has period");
        let normalized = parameter.rem_euclid(period);
        let normalized = if normalized == 0.0 { 0.0 } else { normalized };
        let logical_knots = &self.knots[..self.control_count];
        let multiplicity = logical_knots
            .iter()
            .filter(|knot| **knot == normalized)
            .count();
        if multiplicity >= self.degree {
            return Err(BSplineInsertionError::MaximumMultiplicity {
                parameter: normalized,
                multiplicity,
                degree: self.degree(),
            });
        }
        let split_span = if multiplicity == 0 {
            Some(self.locate_span(normalized, BSplineKnotSide::Right)?)
        } else {
            None
        };
        let span_index = self.locate_span(normalized, BSplineKnotSide::Right)?;
        let span = self
            .span(span_index)
            .expect("located periodic span belongs to basis");
        let raw_span = span.raw_span;
        let degree =
            isize::try_from(self.degree).map_err(|_| BSplineEvaluationError::NonFiniteBasis)?;
        let old_count = isize::try_from(self.control_count)
            .map_err(|_| BSplineEvaluationError::NonFiniteBasis)?;
        let new_count = old_count + 1;
        let first_output = raw_span - degree;
        let last_blend = raw_span
            - isize::try_from(multiplicity).map_err(|_| BSplineEvaluationError::NonFiniteBasis)?;
        let new_count_usize =
            usize::try_from(new_count).map_err(|_| BSplineDefinitionError::CountOverflow)?;
        let mut stencils = vec![None; new_count_usize];
        for output in first_output..=(first_output + old_count) {
            let target = usize::try_from(output.rem_euclid(new_count))
                .map_err(|_| BSplineEvaluationError::NonFiniteBasis)?;
            let stencil = if output == first_output {
                BSplineControlStencil::copy(periodic_control_index(output, old_count)?)
            } else if output <= last_blend {
                let denominator = self.knot(output + degree) - self.knot(output);
                let alpha = (normalized - self.knot(output)) / denominator;
                BSplineControlStencil::blend(
                    periodic_control_index(output - 1, old_count)?,
                    1.0 - alpha,
                    periodic_control_index(output, old_count)?,
                    alpha,
                )
            } else {
                BSplineControlStencil::copy(periodic_control_index(output - 1, old_count)?)
            };
            stencils[target] = Some(stencil);
        }
        let control_stencils = collect_stencils(stencils)?;
        let mut knots = self.knots.clone();
        let insertion_index = knots[..self.control_count]
            .iter()
            .take_while(|knot| **knot <= normalized)
            .count();
        knots.insert(insertion_index, normalized);
        let basis = Self::try_periodic(self.degree(), new_count_usize, knots)?;
        Ok(BSplineBasisRefinement {
            basis,
            control_stencils,
            split_span,
        })
    }

    fn period(&self) -> Option<f64> {
        match self.domain {
            CurveParameterDomain::Periodic { period } => Some(period),
            CurveParameterDomain::SupportingLine | CurveParameterDomain::Bounded { .. } => None,
        }
    }

    fn normalized_parameter(&self, parameter: f64) -> Result<f64, BSplineEvaluationError> {
        if !parameter.is_finite() {
            return Err(BSplineEvaluationError::Curve(
                CurveParameterError::NonFinite { parameter }.into(),
            ));
        }
        match self.domain {
            CurveParameterDomain::Bounded { lower, upper } => {
                if (lower..=upper).contains(&parameter) {
                    Ok(parameter)
                } else {
                    Err(BSplineEvaluationError::Curve(
                        CurveParameterError::OutOfDomain {
                            parameter,
                            domain: self.domain,
                        }
                        .into(),
                    ))
                }
            }
            CurveParameterDomain::Periodic { period } => {
                let wrapped = parameter.rem_euclid(period);
                if wrapped == 0.0 { Ok(0.0) } else { Ok(wrapped) }
            }
            CurveParameterDomain::SupportingLine => unreachable!("B-spline domain is not a line"),
        }
    }

    fn knot(&self, index: isize) -> f64 {
        match self.form {
            BSplineForm::Clamped => {
                self.knots[usize::try_from(index).expect("validated clamped knot index")]
            }
            BSplineForm::Periodic => {
                let control_count =
                    isize::try_from(self.control_count).expect("validated periodic control count");
                let degree = isize::try_from(self.degree).expect("validated periodic degree");
                let shifted = index - degree;
                let period_offset = shifted.div_euclid(control_count);
                let knot_index = usize::try_from(shifted.rem_euclid(control_count))
                    .expect("nonnegative periodic knot index");
                let period = self.period().expect("periodic basis has a period");
                let offset = match period_offset {
                    -1 => -period,
                    0 => 0.0,
                    1 => period,
                    _ => unreachable!("basis evaluation accesses only adjacent periods"),
                };
                self.knots[knot_index] + offset
            }
        }
    }

    fn basis_jet(
        &self,
        span_index: BSplineSpanIndex,
        native_parameter: f64,
        derivative_scale: f64,
    ) -> Result<BSplineBasisJet, BSplineEvaluationError> {
        let span = self
            .span(span_index)
            .ok_or(BSplineEvaluationError::InvalidSpan {
                ordinal: span_index.0,
            })?;
        if !native_parameter.is_finite()
            || native_parameter < span.lower
            || native_parameter > span.upper
            || !derivative_scale.is_finite()
            || derivative_scale <= 0.0
        {
            return Err(BSplineEvaluationError::NonFiniteBasis);
        }

        let width = self
            .degree
            .checked_add(2)
            .ok_or(BSplineEvaluationError::NonFiniteBasis)?;
        let mut levels = vec![vec![[0.0; 4]; width]; self.degree + 1];
        levels[0][self.degree][0] = 1.0;
        let base = span.raw_span
            - isize::try_from(self.degree).map_err(|_| BSplineEvaluationError::NonFiniteBasis)?;

        for degree in 1..=self.degree {
            for slot in (self.degree - degree)..=self.degree {
                let index = base
                    + isize::try_from(slot).map_err(|_| BSplineEvaluationError::NonFiniteBasis)?;
                let degree_index =
                    isize::try_from(degree).map_err(|_| BSplineEvaluationError::NonFiniteBasis)?;
                let first_denominator = self.knot(index + degree_index) - self.knot(index);
                let second_denominator = self.knot(index + degree_index + 1) - self.knot(index + 1);
                let first = if first_denominator == 0.0 {
                    0.0
                } else {
                    (native_parameter - self.knot(index)) / first_denominator
                        * levels[degree - 1][slot][0]
                };
                let second = if second_denominator == 0.0 {
                    0.0
                } else {
                    (self.knot(index + degree_index + 1) - native_parameter) / second_denominator
                        * levels[degree - 1][slot + 1][0]
                };
                levels[degree][slot][0] = first + second;

                for derivative in 1..=3.min(degree) {
                    let multiplier = f64::from(
                        u32::try_from(degree)
                            .map_err(|_| BSplineEvaluationError::NonFiniteBasis)?,
                    );
                    let first = if first_denominator == 0.0 {
                        0.0
                    } else {
                        multiplier / first_denominator * levels[degree - 1][slot][derivative - 1]
                    };
                    let second = if second_denominator == 0.0 {
                        0.0
                    } else {
                        multiplier / second_denominator
                            * levels[degree - 1][slot + 1][derivative - 1]
                    };
                    levels[degree][slot][derivative] = first - second;
                }
            }
        }

        let mut terms = Vec::with_capacity(self.degree + 1);
        for (slot, control_index) in span.support.iter().copied().enumerate() {
            let mut derivatives = levels[self.degree][slot];
            for (order, derivative) in derivatives.iter_mut().enumerate().skip(1) {
                for _ in 0..order {
                    if *derivative == 0.0 {
                        break;
                    }
                    *derivative *= derivative_scale;
                }
            }
            if !derivatives.into_iter().all(f64::is_finite) {
                return Err(BSplineEvaluationError::NonFiniteBasis);
            }
            terms.push(BSplineBasisTerm {
                control_index,
                derivatives,
            });
        }
        Ok(BSplineBasisJet {
            span: span_index,
            native_parameter,
            terms,
        })
    }
}

/// A validated immutable planar non-rational B-spline.
#[derive(Clone, Debug, PartialEq)]
pub struct BSplineCurve2 {
    basis: BSplineBasis,
    controls: Vec<Point2<f64>>,
}

impl BSplineCurve2 {
    /// Associates finite controls with a validated basis.
    ///
    /// # Errors
    ///
    /// Rejects a control-count mismatch or non-finite control coordinate.
    pub fn try_new(
        basis: BSplineBasis,
        controls: Vec<Point2<f64>>,
    ) -> Result<Self, BSplineDefinitionError> {
        if controls.len() != basis.control_count {
            return Err(BSplineDefinitionError::ControlCount {
                expected: basis.control_count,
                actual: controls.len(),
            });
        }
        for (index, point) in controls.iter().enumerate() {
            if !point.x.is_finite() || !point.y.is_finite() {
                return Err(BSplineDefinitionError::NonFiniteControl { index });
            }
        }
        Ok(Self { basis, controls })
    }

    /// Constructs a clamped curve from controls and a complete knot vector.
    ///
    /// # Errors
    ///
    /// Returns a typed definition failure.
    pub fn try_clamped(
        degree: u32,
        controls: Vec<Point2<f64>>,
        knots: Vec<f64>,
    ) -> Result<Self, BSplineDefinitionError> {
        let basis = BSplineBasis::try_clamped(degree, controls.len(), knots)?;
        Self::try_new(basis, controls)
    }

    /// Constructs a periodic curve from unique controls and one-period knots.
    ///
    /// # Errors
    ///
    /// Returns a typed definition failure.
    pub fn try_periodic(
        degree: u32,
        controls: Vec<Point2<f64>>,
        one_period_knots: Vec<f64>,
    ) -> Result<Self, BSplineDefinitionError> {
        let basis = BSplineBasis::try_periodic(degree, controls.len(), one_period_knots)?;
        Self::try_new(basis, controls)
    }

    #[must_use]
    pub const fn basis(&self) -> &BSplineBasis {
        &self.basis
    }

    #[must_use]
    pub fn controls(&self) -> &[Point2<f64>] {
        &self.controls
    }

    /// Evaluates a native-coordinate one-sided curve jet.
    ///
    /// # Errors
    ///
    /// Returns a typed parameter, knot-side, zero-speed or finite-evaluation failure.
    pub fn jet_at(
        &self,
        parameter: f64,
        side: BSplineKnotSide,
    ) -> Result<CurveJet2, BSplineEvaluationError> {
        let basis = self.basis.basis_jet_at(parameter, side)?;
        curve_jet_from_basis(&basis, &self.controls, self.basis.parameter_domain())
    }

    /// Evaluates a curve jet in one selected span's local `[0, 1]` coordinate.
    ///
    /// # Errors
    ///
    /// Returns a typed span, parameter, zero-speed or finite-evaluation failure.
    pub fn jet_on_span(
        &self,
        span: BSplineSpanIndex,
        local_parameter: f64,
    ) -> Result<CurveJet2, BSplineEvaluationError> {
        let basis = self.basis.basis_jet_on_span(span, local_parameter)?;
        curve_jet_from_basis(
            &basis,
            &self.controls,
            CurveParameterDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
    }

    /// Inserts one connected knot and returns geometry-preserving control stencils.
    ///
    /// # Errors
    ///
    /// Rejects non-finite or clamped-endpoint parameters and multiplicity above
    /// the connected-curve maximum.
    pub fn insert_knot(&self, parameter: f64) -> Result<BSplineRefinement2, BSplineInsertionError> {
        let BSplineBasisRefinement {
            basis,
            control_stencils,
            split_span,
        } = self.basis.insert_knot(parameter)?;
        let controls = control_stencils
            .iter()
            .map(|stencil| stencil.evaluate(&self.controls))
            .collect();
        let curve = Self::try_new(basis, controls)?;
        Ok(BSplineRefinement2 {
            curve,
            control_stencils,
            split_span,
        })
    }
}

fn validate_degree_and_controls(
    degree: u32,
    control_count: usize,
) -> Result<usize, BSplineDefinitionError> {
    if degree == 0 {
        return Err(BSplineDefinitionError::InvalidDegree { degree });
    }
    if degree > MAX_BSPLINE_DEGREE {
        return Err(BSplineDefinitionError::DegreeLimit {
            actual: degree,
            limit: MAX_BSPLINE_DEGREE,
        });
    }
    let degree_usize =
        usize::try_from(degree).map_err(|_| BSplineDefinitionError::CountOverflow)?;
    let minimum = degree_usize
        .checked_add(1)
        .ok_or(BSplineDefinitionError::CountOverflow)?;
    if control_count < minimum {
        return Err(BSplineDefinitionError::InsufficientControls {
            degree,
            minimum,
            actual: control_count,
        });
    }
    Ok(degree_usize)
}

fn validate_knots(knots: &[f64]) -> Result<(), BSplineDefinitionError> {
    for (index, value) in knots.iter().copied().enumerate() {
        if !value.is_finite() {
            return Err(BSplineDefinitionError::NonFiniteKnot { index, value });
        }
    }
    for (index, pair) in knots.windows(2).enumerate() {
        if pair[0] > pair[1] {
            return Err(BSplineDefinitionError::DecreasingKnots {
                first: index,
                second: index + 1,
                lower: pair[0],
                upper: pair[1],
            });
        }
    }
    Ok(())
}

fn validate_multiplicities(knots: &[f64], maximum: usize) -> Result<(), BSplineDefinitionError> {
    let mut start = 0;
    while start < knots.len() {
        let parameter = knots[start];
        let mut end = start + 1;
        while end < knots.len() && knots[end] == parameter {
            end += 1;
        }
        let multiplicity = end - start;
        if multiplicity > maximum {
            return Err(BSplineDefinitionError::KnotMultiplicity {
                parameter,
                multiplicity,
                maximum,
            });
        }
        start = end;
    }
    Ok(())
}

fn build_clamped_spans(
    degree: usize,
    control_count: usize,
    knots: &[f64],
) -> Result<Vec<BSplineSpan>, BSplineDefinitionError> {
    let mut spans = Vec::new();
    for raw_span in degree..control_count {
        if knots[raw_span] < knots[raw_span + 1] {
            let support = ((raw_span - degree)..=raw_span).collect();
            push_span(
                &mut spans,
                knots[raw_span],
                knots[raw_span + 1],
                support,
                isize::try_from(raw_span).map_err(|_| BSplineDefinitionError::CountOverflow)?,
            )?;
        }
    }
    Ok(spans)
}

fn build_periodic_spans(
    degree: usize,
    control_count: usize,
    knots: &[f64],
) -> Result<Vec<BSplineSpan>, BSplineDefinitionError> {
    let mut spans = Vec::new();
    for knot_span in 0..control_count {
        if knots[knot_span] < knots[knot_span + 1] {
            let support = (knot_span..=knot_span + degree)
                .map(|index| index % control_count)
                .collect();
            let raw_span = degree
                .checked_add(knot_span)
                .ok_or(BSplineDefinitionError::CountOverflow)?;
            push_span(
                &mut spans,
                knots[knot_span],
                knots[knot_span + 1],
                support,
                isize::try_from(raw_span).map_err(|_| BSplineDefinitionError::CountOverflow)?,
            )?;
        }
    }
    Ok(spans)
}

fn push_span(
    spans: &mut Vec<BSplineSpan>,
    lower: f64,
    upper: f64,
    support: Vec<usize>,
    raw_span: isize,
) -> Result<(), BSplineDefinitionError> {
    let ordinal = u32::try_from(spans.len()).map_err(|_| BSplineDefinitionError::CountOverflow)?;
    spans.push(BSplineSpan {
        index: BSplineSpanIndex(ordinal),
        lower,
        upper,
        support,
        raw_span,
    });
    Ok(())
}

fn curve_jet_from_basis(
    basis: &BSplineBasisJet,
    controls: &[Point2<f64>],
    domain: CurveParameterDomain,
) -> Result<CurveJet2, BSplineEvaluationError> {
    let mut values = [Vector2::zeros(); 4];
    for term in &basis.terms {
        for (derivative, coefficient) in term.derivatives.into_iter().enumerate() {
            values[derivative] += controls[term.control_index].coords * coefficient;
        }
    }
    let jet = CurveJet2 {
        position: Point2::from(values[0]),
        first_derivative: values[1],
        second_derivative: values[2],
        third_derivative: values[3],
        domain,
    };
    let finite = jet.position.x.is_finite()
        && jet.position.y.is_finite()
        && jet.first_derivative.x.is_finite()
        && jet.first_derivative.y.is_finite()
        && jet.second_derivative.x.is_finite()
        && jet.second_derivative.y.is_finite()
        && jet.third_derivative.x.is_finite()
        && jet.third_derivative.y.is_finite();
    if !finite {
        Err(BSplineEvaluationError::Curve(
            CurveRegularityError::NonFiniteJet.into(),
        ))
    } else if jet.first_derivative.x.hypot(jet.first_derivative.y) == 0.0 {
        Err(BSplineEvaluationError::Curve(
            CurveRegularityError::ZeroSpeed.into(),
        ))
    } else {
        Ok(jet)
    }
}

fn collect_stencils(
    stencils: Vec<Option<BSplineControlStencil>>,
) -> Result<Vec<BSplineControlStencil>, BSplineEvaluationError> {
    stencils
        .into_iter()
        .map(|stencil| {
            let stencil = stencil.ok_or(BSplineEvaluationError::NonFiniteBasis)?;
            let finite = stencil.first_weight.is_finite()
                && stencil.second_weight.is_finite()
                && stencil.first_weight >= 0.0
                && stencil.second_weight >= 0.0
                && (stencil.first_weight + stencil.second_weight - 1.0).abs()
                    <= 16.0 * f64::EPSILON;
            if finite {
                Ok(stencil)
            } else {
                Err(BSplineEvaluationError::NonFiniteBasis)
            }
        })
        .collect()
}

fn periodic_control_index(
    index: isize,
    control_count: isize,
) -> Result<usize, BSplineEvaluationError> {
    usize::try_from(index.rem_euclid(control_count))
        .map_err(|_| BSplineEvaluationError::NonFiniteBasis)
}
