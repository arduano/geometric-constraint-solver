use crate::curves::{validate_bounded_parameter, validate_line_parameter};
use crate::{
    ArcId, CurveContactNeighborhood, CurveContinuity, CurveCurvatureRelation,
    CurveDirectionRelation, CurveMeasurementKind, CurveNormalSide, CurveTangentOrientation,
    FilletEndpointOrder, PointId, SegmentEndpoint, SegmentId, Sketch, SketchConstraintId,
    SketchConstraintKind, SketchCurve, SketchCurveContact, SketchError,
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

    /// Adds a generic tangent or explicitly sided normal direction at one curve location.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, degenerate, non-finite, or out-of-domain data.
    pub fn add_curve_direction(
        &mut self,
        line: SegmentId,
        contact: SketchCurveContact,
        relation: CurveDirectionRelation,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(line)?;
        self.validate_curve_contact(contact)?;
        Ok(
            self.insert_constraint(SketchConstraintKind::CurveDirection {
                line,
                contact,
                relation,
            }),
        )
    }

    /// Adds equality of signed curvature or one explicit magnitude-sign branch.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, degenerate, non-finite, or out-of-domain data.
    pub fn add_equal_curvature(
        &mut self,
        first: SketchCurveContact,
        second: SketchCurveContact,
        relation: CurveCurvatureRelation,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_curve_contact(first)?;
        self.validate_curve_contact(second)?;
        Ok(
            self.insert_constraint(SketchConstraintKind::EqualCurvature {
                first,
                second,
                relation,
            }),
        )
    }

    /// Adds ordered G0/G1/G2 or separately named parametric C2 endpoint continuity.
    ///
    /// # Errors
    ///
    /// Rejects non-endpoint contacts, invalid rates, stale geometry, and irregular jets.
    pub fn add_endpoint_continuity(
        &mut self,
        first: SketchCurveContact,
        second: SketchCurveContact,
        kind: CurveContinuity,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_curve_contact(first)?;
        self.validate_curve_contact(second)?;
        if !matches!(
            first.neighborhood,
            CurveContactNeighborhood::Start | CurveContactNeighborhood::End
        ) || !matches!(
            second.neighborhood,
            CurveContactNeighborhood::Start | CurveContactNeighborhood::End
        ) {
            return Err(SketchError::InvalidContinuityEndpoint);
        }
        if let CurveContinuity::ParametricC2 {
            first_rate,
            second_rate,
        } = kind
            && (!first_rate.is_finite()
                || first_rate <= 0.0
                || !second_rate.is_finite()
                || second_rate <= 0.0)
        {
            return Err(SketchError::InvalidContinuityRate);
        }
        Ok(
            self.insert_constraint(SketchConstraintKind::EndpointContinuity {
                first,
                second,
                kind,
            }),
        )
    }

    /// Associates one circular arc with two strict-interior bounded line contacts.
    ///
    /// Parent spans remain untrimmed; accepted arc endpoint angles are derived from the contacts.
    ///
    /// # Errors
    ///
    /// Rejects stale/non-line geometry, duplicate parents, non-interior contacts, or an invalid arc.
    pub fn add_line_line_fillet(
        &mut self,
        arc: ArcId,
        first: SketchCurveContact,
        first_side: CurveNormalSide,
        second: SketchCurveContact,
        second_side: CurveNormalSide,
        endpoint_order: FilletEndpointOrder,
    ) -> Result<SketchConstraintId, SketchError> {
        self.arc_value(arc)?;
        if self
            .constraints
            .iter()
            .any(|(_, constraint)| constraint_uses_arc(constraint.kind(), arc))
        {
            return Err(SketchError::InvalidCurveContact(
                "line fillet output arc is already used by an executable constraint",
            ));
        }
        self.validate_curve_contact(first)?;
        self.validate_curve_contact(second)?;
        let (
            SketchCurve::Line {
                segment: first_segment,
                domain: crate::LineParameterDomain::BoundedSegment,
            },
            SketchCurve::Line {
                segment: second_segment,
                domain: crate::LineParameterDomain::BoundedSegment,
            },
        ) = (first.curve, second.curve)
        else {
            return Err(SketchError::InvalidCurveContact(
                "line fillet contacts require bounded line spans",
            ));
        };
        if first_segment == second_segment
            || first.neighborhood != CurveContactNeighborhood::Interior
            || second.neighborhood != CurveContactNeighborhood::Interior
        {
            return Err(SketchError::InvalidCurveContact(
                "line fillet requires distinct strict-interior parent spans",
            ));
        }
        Ok(
            self.insert_constraint(SketchConstraintKind::LineLineFillet {
                arc,
                first,
                first_side,
                second,
                second_side,
                endpoint_order,
            }),
        )
    }

    /// Measures signed/unsigned curvature or finite osculating radius at a curve contact.
    ///
    /// # Errors
    ///
    /// Rejects stale or irregular geometry, invalid contact state, and an osculating
    /// radius at zero or unrepresentable curvature.
    pub fn measure_curve(
        &self,
        contact: SketchCurveContact,
        kind: CurveMeasurementKind,
    ) -> Result<f64, SketchError> {
        self.validate_curve_contact_inner(contact, false)?;
        let differential = self
            .evaluate_curve_contact(contact)?
            .differential()
            .map_err(SketchError::InvalidCurveDifferential)?;
        match kind {
            CurveMeasurementKind::SignedCurvature => Ok(differential.signed_curvature),
            CurveMeasurementKind::UnsignedCurvature => Ok(differential.unsigned_curvature()),
            CurveMeasurementKind::OsculatingRadius => differential
                .osculating_radius()
                .map_err(SketchError::InvalidCurveDifferential),
        }
    }

    pub(crate) fn validate_curve_contact(
        &self,
        contact: SketchCurveContact,
    ) -> Result<(), SketchError> {
        self.validate_curve_contact_inner(contact, true)
    }

    fn validate_curve_contact_inner(
        &self,
        contact: SketchCurveContact,
        executable: bool,
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
                if executable
                    && self.constraints.iter().any(|(_, constraint)| {
                        matches!(constraint.kind(), SketchConstraintKind::LineLineFillet { arc: output, .. } if output == arc)
                    })
                {
                    return Err(SketchError::InvalidCurveContact(
                        "associated line fillet arcs cannot own executable contacts before M28",
                    ));
                }
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
            SketchCurve::BSpline { spline, span } => {
                self.evaluate_bspline(spline, span, contact.parameter)
                    .map_err(|_| {
                        SketchError::InvalidCurveContact("B-spline span jet is not regular")
                    })?;
                validate_neighborhood(true, contact.parameter, contact.neighborhood)
            }
            SketchCurve::Nurbs { nurbs, span } => {
                self.evaluate_nurbs(nurbs, span, contact.parameter)?;
                validate_neighborhood(true, contact.parameter, contact.neighborhood)
            }
        }
    }

    fn evaluate_curve_contact(
        &self,
        contact: SketchCurveContact,
    ) -> Result<geosolve_geometry::CurveJet2, SketchError> {
        match contact.curve {
            SketchCurve::Line { segment, domain } => {
                let (start, end) = self.segment_endpoints(segment)?;
                geosolve_geometry::line_jet(
                    self.point_position(start)?,
                    self.point_position(end)?,
                    match domain {
                        crate::LineParameterDomain::SupportingLine => {
                            geosolve_geometry::CurveParameterDomain::SupportingLine
                        }
                        crate::LineParameterDomain::BoundedSegment => {
                            geosolve_geometry::CurveParameterDomain::Bounded {
                                lower: 0.0,
                                upper: 1.0,
                            }
                        }
                    },
                    contact.parameter,
                )
                .map_err(|_| SketchError::InvalidCurveContact("line jet is not regular"))
            }
            SketchCurve::Circle(circle) => {
                let circle = self.circle_value(circle)?;
                geosolve_geometry::circle_jet(
                    self.point_position(circle.center())?,
                    circle.radius(),
                    contact.parameter,
                )
                .map_err(|_| SketchError::InvalidCurveContact("circle jet is not regular"))
            }
            SketchCurve::Arc(arc) => {
                let arc = self.arc_value(arc)?;
                geosolve_geometry::circular_arc_jet(
                    self.point_position(arc.center())?,
                    arc.radius(),
                    arc.start_angle(),
                    arc.signed_sweep(),
                    contact.parameter,
                )
                .map_err(|_| SketchError::InvalidCurveContact("arc jet is not regular"))
            }
            SketchCurve::Bezier(bezier) => self
                .evaluate_bezier(bezier, contact.parameter)
                .map_err(|_| SketchError::InvalidCurveContact("Bezier jet is not regular")),
            SketchCurve::Conic(conic) => self
                .evaluate_conic(conic, contact.parameter)
                .map_err(|_| SketchError::InvalidCurveContact("conic jet is not regular")),
            SketchCurve::BSpline { spline, span } => {
                self.evaluate_bspline(spline, span, contact.parameter)
            }
            SketchCurve::Nurbs { nurbs, span } => {
                self.evaluate_nurbs(nurbs, span, contact.parameter)
            }
        }
    }
}

fn constraint_uses_arc(kind: SketchConstraintKind, arc: ArcId) -> bool {
    let contact_uses_arc = |contact: SketchCurveContact| matches!(contact.curve, SketchCurve::Arc(candidate) if candidate == arc);
    match kind {
        SketchConstraintKind::PointOnArc { arc: candidate, .. }
        | SketchConstraintKind::CircleArcTangency { arc: candidate, .. }
        | SketchConstraintKind::LineLineFillet { arc: candidate, .. } => candidate == arc,
        SketchConstraintKind::PointOnCurve { contact, .. }
        | SketchConstraintKind::LineCurveTangency { contact, .. }
        | SketchConstraintKind::CurveDirection { contact, .. } => contact_uses_arc(contact),
        SketchConstraintKind::CurveCurveContact { first, second }
        | SketchConstraintKind::CurveCurveTangency { first, second, .. }
        | SketchConstraintKind::EqualCurvature { first, second, .. }
        | SketchConstraintKind::EndpointContinuity { first, second, .. } => {
            contact_uses_arc(first) || contact_uses_arc(second)
        }
        _ => false,
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
