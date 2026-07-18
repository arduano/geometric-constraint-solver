use crate::curves::{validate_bounded_parameter, validate_line_parameter};
use crate::{
    CurveContactNeighborhood, CurveTangentOrientation, PointId, SegmentEndpoint, SegmentId, Sketch,
    SketchConstraintId, SketchConstraintKind, SketchCurve, SketchCurveContact, SketchError,
};

impl Sketch {
    /// Adds a point-on-curve source through the geometry-generic curve adapter.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, degenerate, non-finite, or out-of-domain contact data.
    pub fn add_point_on_curve(
        &mut self,
        point: PointId,
        contact: SketchCurveContact,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point(point).ok_or(SketchError::UnknownPoint(point))?;
        self.validate_curve_contact(contact)?;
        Ok(self.insert_constraint(SketchConstraintKind::PointOnCurve { point, contact }))
    }

    /// Adds contact and tangency between one line endpoint and any alpha curve family.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, degenerate, non-finite, or out-of-domain contact data.
    pub fn add_line_curve_tangency(
        &mut self,
        line: SegmentId,
        endpoint: SegmentEndpoint,
        contact: SketchCurveContact,
        orientation: CurveTangentOrientation,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(line)?;
        self.validate_curve_contact(contact)?;
        Ok(
            self.insert_constraint(SketchConstraintKind::LineCurveTangency {
                line,
                endpoint,
                contact,
                orientation,
            }),
        )
    }

    /// Adds equality of two parameterized curve locations.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, degenerate, non-finite, or out-of-domain contact data.
    pub fn add_curve_curve_contact(
        &mut self,
        first: SketchCurveContact,
        second: SketchCurveContact,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_curve_contact(first)?;
        self.validate_curve_contact(second)?;
        Ok(self.insert_constraint(SketchConstraintKind::CurveCurveContact { first, second }))
    }

    /// Adds contact plus explicit aligned/opposed tangent orientation for two curves.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, degenerate, non-finite, or out-of-domain contact data.
    pub fn add_curve_curve_tangency(
        &mut self,
        first: SketchCurveContact,
        second: SketchCurveContact,
        orientation: CurveTangentOrientation,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_curve_contact(first)?;
        self.validate_curve_contact(second)?;
        Ok(
            self.insert_constraint(SketchConstraintKind::CurveCurveTangency {
                first,
                second,
                orientation,
            }),
        )
    }

    pub(crate) fn validate_curve_contact(
        &self,
        contact: SketchCurveContact,
    ) -> Result<(), SketchError> {
        match contact.curve {
            SketchCurve::Line { segment, domain } => {
                self.validate_segment_geometry(segment)?;
                validate_line_parameter(domain, contact.parameter)?;
                validate_neighborhood(
                    domain == crate::LineParameterDomain::BoundedSegment,
                    contact.parameter,
                    contact.neighborhood,
                )
            }
            SketchCurve::Circle(circle) => {
                self.circle_value(circle)?;
                crate::model::validate_finite(contact.parameter, "circle contact angle")?;
                validate_unbounded_neighborhood(contact.neighborhood)
            }
            SketchCurve::Arc(arc) => {
                self.arc_value(arc)?;
                validate_bounded_parameter(contact.parameter, "bounded-arc span [0, 1]")?;
                validate_neighborhood(true, contact.parameter, contact.neighborhood)
            }
            SketchCurve::Bezier(bezier) => {
                self.evaluate_bezier(bezier, contact.parameter)
                    .map_err(|_| SketchError::InvalidCurveContact("Bezier jet is not regular"))?;
                validate_neighborhood(true, contact.parameter, contact.neighborhood)
            }
            SketchCurve::Conic(conic) => {
                let value = self.conic_value(conic)?;
                self.evaluate_conic(conic, contact.parameter)
                    .map_err(|_| SketchError::InvalidCurveContact("conic jet is not regular"))?;
                validate_neighborhood(
                    !value.is_periodic(),
                    contact.parameter,
                    contact.neighborhood,
                )
            }
        }
    }
}

fn validate_unbounded_neighborhood(
    neighborhood: CurveContactNeighborhood,
) -> Result<(), SketchError> {
    if neighborhood == CurveContactNeighborhood::Interior {
        Ok(())
    } else {
        Err(SketchError::ParameterOutOfDomain {
            parameter: f64::NAN,
            domain: "an unbounded or periodic curve has only an interior neighborhood",
        })
    }
}

fn validate_neighborhood(
    bounded: bool,
    parameter: f64,
    neighborhood: CurveContactNeighborhood,
) -> Result<(), SketchError> {
    if !bounded {
        return validate_unbounded_neighborhood(neighborhood);
    }
    let valid = match neighborhood {
        CurveContactNeighborhood::Start => parameter.to_bits() == 0.0f64.to_bits(),
        CurveContactNeighborhood::End => parameter.to_bits() == 1.0f64.to_bits(),
        CurveContactNeighborhood::Interior => parameter > 0.0 && parameter < 1.0,
        CurveContactNeighborhood::Local { lower, upper } => {
            lower.is_finite()
                && upper.is_finite()
                && lower >= 0.0
                && lower < parameter
                && parameter < upper
                && upper <= 1.0
        }
    };
    if valid {
        Ok(())
    } else {
        Err(SketchError::ParameterOutOfDomain {
            parameter,
            domain: "the selected bounded contact neighborhood",
        })
    }
}
