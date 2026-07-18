use geosolve_geometry::{
    ConicDefinitionError, ConicEvaluationError, CurveJet2, DirectedParameterTrim, Ellipse2,
    EllipseAxisObservability, EllipticalArc2, HyperbolaBranch, HyperbolaSegment2, ParabolaSegment2,
    Point2, ProperConicKind, RationalQuadraticConicSegment2, UnitDirection2, Vector2, ellipse_jet,
    elliptical_arc_jet, hyperbola_segment_jet, parabola_segment_jet, rational_quadratic_conic_jet,
};

use crate::{ConicId, PointId, Sketch, SketchConstraintKind, SketchError};

/// Closed runtime conic topology over ordinary sketch points and explicit discrete state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConicKind {
    Ellipse {
        center: PointId,
        major_axis_point: PointId,
        minor_axis_ratio: f64,
    },
    EllipticalArc {
        center: PointId,
        major_axis_point: PointId,
        minor_axis_ratio: f64,
        start_angle: f64,
        signed_sweep: f64,
    },
    RationalQuadratic {
        start: PointId,
        weighted_middle: Vector2<f64>,
        middle_weight: f64,
        end: PointId,
    },
    ParabolaSegment {
        vertex: PointId,
        focus: PointId,
        trim: DirectedParameterTrim,
    },
    HyperbolaSegment {
        center: PointId,
        transverse_axis_point: PointId,
        semi_conjugate: f64,
        branch: HyperbolaBranch,
        trim: DirectedParameterTrim,
    },
}

impl ConicKind {
    /// Returns every ordinary sketch point referenced by this definition.
    #[must_use]
    pub fn points(self) -> Vec<PointId> {
        match self {
            Self::Ellipse {
                center,
                major_axis_point,
                ..
            }
            | Self::EllipticalArc {
                center,
                major_axis_point,
                ..
            } => vec![center, major_axis_point],
            Self::RationalQuadratic { start, end, .. } => vec![start, end],
            Self::ParabolaSegment { vertex, focus, .. } => vec![vertex, focus],
            Self::HyperbolaSegment {
                center,
                transverse_axis_point,
                ..
            } => vec![center, transverse_axis_point],
        }
    }

    pub(crate) fn references_point(self, point: PointId) -> bool {
        match self {
            Self::Ellipse {
                center,
                major_axis_point,
                ..
            }
            | Self::EllipticalArc {
                center,
                major_axis_point,
                ..
            } => center == point || major_axis_point == point,
            Self::RationalQuadratic { start, end, .. } => start == point || end == point,
            Self::ParabolaSegment { vertex, focus, .. } => vertex == point || focus == point,
            Self::HyperbolaSegment {
                center,
                transverse_axis_point,
                ..
            } => center == point || transverse_axis_point == point,
        }
    }

    /// Whether this curve uses an unwrapped periodic parameter.
    #[must_use]
    pub const fn is_periodic(self) -> bool {
        matches!(self, Self::Ellipse { .. })
    }
}

/// One runtime conic entity in the sketch's shared conic store.
#[derive(Clone, Debug, PartialEq)]
pub struct ConicCurve {
    pub(crate) kind: ConicKind,
    pub(crate) label: String,
}

impl ConicCurve {
    #[must_use]
    pub const fn kind(&self) -> ConicKind {
        self.kind
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn is_periodic(&self) -> bool {
        self.kind.is_periodic()
    }

    /// Returns all ordinary sketch points referenced by this conic.
    #[must_use]
    pub fn points(&self) -> Vec<PointId> {
        self.kind.points()
    }
}

/// Concrete immutable geometry reconstructed from one runtime or solved conic.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConicGeometry {
    Ellipse(Ellipse2),
    EllipticalArc(EllipticalArc2),
    RationalQuadratic(RationalQuadraticConicSegment2),
    ParabolaSegment(ParabolaSegment2),
    HyperbolaSegment(HyperbolaSegment2),
}

impl ConicGeometry {
    /// Evaluates a full conic jet in its public parameterization.
    ///
    /// # Errors
    ///
    /// Returns a typed domain, regularity, denominator, or overflow failure.
    pub fn evaluate(self, parameter: f64) -> Result<CurveJet2, ConicEvaluationError> {
        match self {
            Self::Ellipse(value) => ellipse_jet(&value, parameter),
            Self::EllipticalArc(value) => elliptical_arc_jet(&value, parameter),
            Self::RationalQuadratic(value) => rational_quadratic_conic_jet(&value, parameter),
            Self::ParabolaSegment(value) => parabola_segment_jet(&value, parameter),
            Self::HyperbolaSegment(value) => hyperbola_segment_jet(&value, parameter),
        }
    }

    /// Returns directed endpoints for bounded conics and `None` for a full ellipse.
    ///
    /// # Errors
    ///
    /// Returns a typed failure if either endpoint cannot be represented.
    pub fn endpoints(self) -> Result<Option<[Point2<f64>; 2]>, ConicEvaluationError> {
        match self {
            Self::Ellipse(_) => Ok(None),
            Self::EllipticalArc(value) => Ok(Some([value.start_point()?, value.end_point()?])),
            Self::RationalQuadratic(value) => Ok(Some([value.start_point(), value.end_point()])),
            Self::ParabolaSegment(value) => Ok(Some([value.start_point()?, value.end_point()?])),
            Self::HyperbolaSegment(value) => Ok(Some([value.start_point()?, value.end_point()?])),
        }
    }

    /// Returns the two foci for ellipses, elliptical arcs, and hyperbolas.
    #[must_use]
    pub fn foci(self) -> Option<[Point2<f64>; 2]> {
        match self {
            Self::Ellipse(value) => Some(value.foci()),
            Self::EllipticalArc(value) => Some(value.ellipse().foci()),
            Self::HyperbolaSegment(value) => Some(value.foci()),
            Self::RationalQuadratic(_) | Self::ParabolaSegment(_) => None,
        }
    }

    /// Returns the focus of a parabola segment.
    #[must_use]
    pub fn focus(self) -> Option<Point2<f64>> {
        match self {
            Self::ParabolaSegment(value) => Some(value.focus()),
            _ => None,
        }
    }

    /// Returns the major-axis endpoints for ellipses and elliptical arcs.
    #[must_use]
    pub fn major_axis_endpoints(self) -> Option<[Point2<f64>; 2]> {
        match self {
            Self::Ellipse(value) => Some(value.major_axis_endpoints()),
            Self::EllipticalArc(value) => Some(value.ellipse().major_axis_endpoints()),
            _ => None,
        }
    }

    /// Returns the minor-axis endpoints for ellipses and elliptical arcs.
    #[must_use]
    pub fn minor_axis_endpoints(self) -> Option<[Point2<f64>; 2]> {
        match self {
            Self::Ellipse(value) => Some(value.minor_axis_endpoints()),
            Self::EllipticalArc(value) => Some(value.ellipse().minor_axis_endpoints()),
            _ => None,
        }
    }

    #[must_use]
    pub fn axis_observability(self) -> Option<EllipseAxisObservability> {
        match self {
            Self::Ellipse(value) => Some(value.axis_observability()),
            Self::EllipticalArc(value) => Some(value.axis_observability()),
            _ => None,
        }
    }

    #[must_use]
    pub fn major_axis_length(self) -> Option<f64> {
        match self {
            Self::Ellipse(value) => Some(value.major_axis_length()),
            Self::EllipticalArc(value) => Some(value.ellipse().major_axis_length()),
            _ => None,
        }
    }

    #[must_use]
    pub fn minor_axis_length(self) -> Option<f64> {
        match self {
            Self::Ellipse(value) => Some(value.minor_axis_length()),
            Self::EllipticalArc(value) => Some(value.ellipse().minor_axis_length()),
            _ => None,
        }
    }

    #[must_use]
    pub fn linear_eccentricity(self) -> Option<f64> {
        match self {
            Self::Ellipse(value) => Some(value.linear_eccentricity()),
            Self::EllipticalArc(value) => Some(value.ellipse().linear_eccentricity()),
            _ => None,
        }
    }

    #[must_use]
    pub fn proper_conic_kind(self) -> Option<ProperConicKind> {
        match self {
            Self::RationalQuadratic(value) => Some(value.proper_conic_kind()),
            _ => None,
        }
    }

    #[must_use]
    pub fn selected_branch_focus(self) -> Option<Point2<f64>> {
        match self {
            Self::HyperbolaSegment(value) => Some(value.selected_branch_focus()),
            _ => None,
        }
    }

    #[must_use]
    pub fn selected_branch_vertex(self) -> Option<Point2<f64>> {
        match self {
            Self::HyperbolaSegment(value) => Some(value.selected_branch_vertex()),
            _ => None,
        }
    }

    #[must_use]
    pub fn focal_distance(self) -> Option<f64> {
        match self {
            Self::HyperbolaSegment(value) => Some(value.focal_distance()),
            _ => None,
        }
    }

    #[must_use]
    pub fn transverse_axis_length(self) -> Option<f64> {
        match self {
            Self::HyperbolaSegment(value) => Some(value.transverse_axis_length()),
            _ => None,
        }
    }

    #[must_use]
    pub fn conjugate_axis_length(self) -> Option<f64> {
        match self {
            Self::HyperbolaSegment(value) => Some(value.conjugate_axis_length()),
            _ => None,
        }
    }
}

#[allow(clippy::missing_errors_doc)]
impl Sketch {
    /// Adds a full ellipse with a deterministic generated label.
    pub fn add_ellipse(
        &mut self,
        center: PointId,
        major_axis_point: PointId,
        minor_axis_ratio: f64,
    ) -> Result<ConicId, SketchError> {
        let label = format!("K{}", self.conics.next_ordinal());
        self.add_named_ellipse(label, center, major_axis_point, minor_axis_ratio)
    }

    /// Adds a named full ellipse, including the exact ratio-one circle limit.
    pub fn add_named_ellipse(
        &mut self,
        label: impl Into<String>,
        center: PointId,
        major_axis_point: PointId,
        minor_axis_ratio: f64,
    ) -> Result<ConicId, SketchError> {
        self.add_named_conic(
            label,
            ConicKind::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            },
        )
    }

    /// Adds a directed elliptical arc with a deterministic generated label.
    pub fn add_elliptical_arc(
        &mut self,
        center: PointId,
        major_axis_point: PointId,
        minor_axis_ratio: f64,
        start_angle: f64,
        signed_sweep: f64,
    ) -> Result<ConicId, SketchError> {
        let label = format!("K{}", self.conics.next_ordinal());
        self.add_named_elliptical_arc(
            label,
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            signed_sweep,
        )
    }

    /// Adds a named directed elliptical arc over normalized parameter `[0, 1]`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_named_elliptical_arc(
        &mut self,
        label: impl Into<String>,
        center: PointId,
        major_axis_point: PointId,
        minor_axis_ratio: f64,
        start_angle: f64,
        signed_sweep: f64,
    ) -> Result<ConicId, SketchError> {
        self.add_named_conic(
            label,
            ConicKind::EllipticalArc {
                center,
                major_axis_point,
                minor_axis_ratio,
                start_angle,
                signed_sweep,
            },
        )
    }

    /// Adds a rational quadratic segment from its homogeneous middle control `(Q, w)`.
    pub fn add_rational_quadratic(
        &mut self,
        start: PointId,
        weighted_middle: Vector2<f64>,
        middle_weight: f64,
        end: PointId,
    ) -> Result<ConicId, SketchError> {
        let label = format!("K{}", self.conics.next_ordinal());
        self.add_named_rational_quadratic(label, start, weighted_middle, middle_weight, end)
    }

    /// Adds a named rational quadratic segment from its homogeneous middle control `(Q, w)`.
    pub fn add_named_rational_quadratic(
        &mut self,
        label: impl Into<String>,
        start: PointId,
        weighted_middle: Vector2<f64>,
        middle_weight: f64,
        end: PointId,
    ) -> Result<ConicId, SketchError> {
        self.add_named_conic(
            label,
            ConicKind::RationalQuadratic {
                start,
                weighted_middle,
                middle_weight,
                end,
            },
        )
    }

    /// Adds a directed trimmed parabola with a deterministic generated label.
    pub fn add_parabola_segment(
        &mut self,
        vertex: PointId,
        focus: PointId,
        trim: DirectedParameterTrim,
    ) -> Result<ConicId, SketchError> {
        let label = format!("K{}", self.conics.next_ordinal());
        self.add_named_parabola_segment(label, vertex, focus, trim)
    }

    /// Adds a named directed trimmed parabola.
    pub fn add_named_parabola_segment(
        &mut self,
        label: impl Into<String>,
        vertex: PointId,
        focus: PointId,
        trim: DirectedParameterTrim,
    ) -> Result<ConicId, SketchError> {
        self.add_named_conic(
            label,
            ConicKind::ParabolaSegment {
                vertex,
                focus,
                trim,
            },
        )
    }

    /// Adds one explicit directed hyperbola branch with a generated label.
    pub fn add_hyperbola_segment(
        &mut self,
        center: PointId,
        transverse_axis_point: PointId,
        semi_conjugate: f64,
        branch: HyperbolaBranch,
        trim: DirectedParameterTrim,
    ) -> Result<ConicId, SketchError> {
        let label = format!("K{}", self.conics.next_ordinal());
        self.add_named_hyperbola_segment(
            label,
            center,
            transverse_axis_point,
            semi_conjugate,
            branch,
            trim,
        )
    }

    /// Adds a named explicit directed hyperbola branch.
    #[allow(clippy::too_many_arguments)]
    pub fn add_named_hyperbola_segment(
        &mut self,
        label: impl Into<String>,
        center: PointId,
        transverse_axis_point: PointId,
        semi_conjugate: f64,
        branch: HyperbolaBranch,
        trim: DirectedParameterTrim,
    ) -> Result<ConicId, SketchError> {
        self.add_named_conic(
            label,
            ConicKind::HyperbolaSegment {
                center,
                transverse_axis_point,
                semi_conjugate,
                branch,
                trim,
            },
        )
    }

    #[must_use]
    pub fn conic(&self, conic: ConicId) -> Option<&ConicCurve> {
        self.conics.get(conic)
    }

    pub fn conics(&self) -> impl Iterator<Item = (ConicId, &ConicCurve)> {
        self.conics.iter()
    }

    /// Reconstructs the current immutable geometry definition.
    pub fn conic_geometry(&self, conic: ConicId) -> Result<ConicGeometry, SketchError> {
        let value = self.conic_value(conic)?;
        conic_geometry_from_kind(value.kind, |point| self.point_position(point))
    }

    /// Evaluates the current accepted conic through the geometry conic-jet API.
    pub fn evaluate_conic(&self, conic: ConicId, parameter: f64) -> Result<CurveJet2, SketchError> {
        self.conic_geometry(conic)?
            .evaluate(parameter)
            .map_err(SketchError::InvalidConicEvaluation)
    }

    /// Returns directed endpoints for bounded conics and `None` for a full ellipse.
    pub fn conic_endpoints(&self, conic: ConicId) -> Result<Option<[Point2<f64>; 2]>, SketchError> {
        self.conic_geometry(conic)?
            .endpoints()
            .map_err(SketchError::InvalidConicEvaluation)
    }

    pub fn conic_foci(&self, conic: ConicId) -> Result<Option<[Point2<f64>; 2]>, SketchError> {
        Ok(self.conic_geometry(conic)?.foci())
    }

    pub fn conic_focus(&self, conic: ConicId) -> Result<Option<Point2<f64>>, SketchError> {
        Ok(self.conic_geometry(conic)?.focus())
    }

    pub fn conic_major_axis_endpoints(
        &self,
        conic: ConicId,
    ) -> Result<Option<[Point2<f64>; 2]>, SketchError> {
        Ok(self.conic_geometry(conic)?.major_axis_endpoints())
    }

    pub fn conic_minor_axis_endpoints(
        &self,
        conic: ConicId,
    ) -> Result<Option<[Point2<f64>; 2]>, SketchError> {
        Ok(self.conic_geometry(conic)?.minor_axis_endpoints())
    }

    pub fn conic_axis_observability(
        &self,
        conic: ConicId,
    ) -> Result<Option<EllipseAxisObservability>, SketchError> {
        Ok(self.conic_geometry(conic)?.axis_observability())
    }

    pub fn conic_major_axis_length(&self, conic: ConicId) -> Result<Option<f64>, SketchError> {
        Ok(self.conic_geometry(conic)?.major_axis_length())
    }

    pub fn conic_minor_axis_length(&self, conic: ConicId) -> Result<Option<f64>, SketchError> {
        Ok(self.conic_geometry(conic)?.minor_axis_length())
    }

    pub fn conic_linear_eccentricity(&self, conic: ConicId) -> Result<Option<f64>, SketchError> {
        Ok(self.conic_geometry(conic)?.linear_eccentricity())
    }

    pub fn conic_proper_kind(
        &self,
        conic: ConicId,
    ) -> Result<Option<ProperConicKind>, SketchError> {
        Ok(self.conic_geometry(conic)?.proper_conic_kind())
    }

    pub fn conic_selected_branch_focus(
        &self,
        conic: ConicId,
    ) -> Result<Option<Point2<f64>>, SketchError> {
        Ok(self.conic_geometry(conic)?.selected_branch_focus())
    }

    pub fn conic_selected_branch_vertex(
        &self,
        conic: ConicId,
    ) -> Result<Option<Point2<f64>>, SketchError> {
        Ok(self.conic_geometry(conic)?.selected_branch_vertex())
    }

    pub fn conic_focal_distance(&self, conic: ConicId) -> Result<Option<f64>, SketchError> {
        Ok(self.conic_geometry(conic)?.focal_distance())
    }

    pub fn conic_transverse_axis_length(&self, conic: ConicId) -> Result<Option<f64>, SketchError> {
        Ok(self.conic_geometry(conic)?.transverse_axis_length())
    }

    pub fn conic_conjugate_axis_length(&self, conic: ConicId) -> Result<Option<f64>, SketchError> {
        Ok(self.conic_geometry(conic)?.conjugate_axis_length())
    }

    pub fn set_conic_minor_axis_ratio(
        &mut self,
        conic: ConicId,
        minor_axis_ratio: f64,
    ) -> Result<(), SketchError> {
        let mut kind = self.conic_value(conic)?.kind;
        match &mut kind {
            ConicKind::Ellipse {
                minor_axis_ratio: value,
                ..
            }
            | ConicKind::EllipticalArc {
                minor_axis_ratio: value,
                ..
            } => *value = minor_axis_ratio,
            _ => return Err(SketchError::InvalidConicScalarRole(conic)),
        }
        self.validate_conic_kind(kind)?;
        self.conics
            .get_mut(conic)
            .ok_or(SketchError::UnknownConic(conic))?
            .kind = kind;
        Ok(())
    }

    pub fn set_conic_middle_weight(
        &mut self,
        conic: ConicId,
        middle_weight: f64,
    ) -> Result<(), SketchError> {
        let ConicKind::RationalQuadratic {
            weighted_middle, ..
        } = self.conic_value(conic)?.kind
        else {
            return Err(SketchError::InvalidConicScalarRole(conic));
        };
        self.set_rational_quadratic_homogeneous(conic, weighted_middle, middle_weight)
    }

    /// Replaces the homogeneous weighted-coordinate vector `Q` transactionally.
    pub fn set_conic_weighted_middle(
        &mut self,
        conic: ConicId,
        weighted_middle: Vector2<f64>,
    ) -> Result<(), SketchError> {
        let ConicKind::RationalQuadratic { middle_weight, .. } = self.conic_value(conic)?.kind
        else {
            return Err(SketchError::InvalidConicScalarRole(conic));
        };
        self.set_rational_quadratic_homogeneous(conic, weighted_middle, middle_weight)
    }

    pub(crate) fn set_rational_quadratic_homogeneous(
        &mut self,
        conic: ConicId,
        weighted_middle: Vector2<f64>,
        middle_weight: f64,
    ) -> Result<(), SketchError> {
        let mut kind = self.conic_value(conic)?.kind;
        let ConicKind::RationalQuadratic {
            weighted_middle: candidate_middle,
            middle_weight: candidate_weight,
            ..
        } = &mut kind
        else {
            return Err(SketchError::InvalidConicScalarRole(conic));
        };
        *candidate_middle = weighted_middle;
        *candidate_weight = middle_weight;
        self.validate_conic_kind(kind)?;
        self.conics
            .get_mut(conic)
            .ok_or(SketchError::UnknownConic(conic))?
            .kind = kind;
        Ok(())
    }

    pub fn set_conic_semi_conjugate(
        &mut self,
        conic: ConicId,
        semi_conjugate: f64,
    ) -> Result<(), SketchError> {
        let mut kind = self.conic_value(conic)?.kind;
        let ConicKind::HyperbolaSegment {
            semi_conjugate: value,
            ..
        } = &mut kind
        else {
            return Err(SketchError::InvalidConicScalarRole(conic));
        };
        *value = semi_conjugate;
        self.validate_conic_kind(kind)?;
        self.conics
            .get_mut(conic)
            .ok_or(SketchError::UnknownConic(conic))?
            .kind = kind;
        Ok(())
    }

    /// Explicitly changes only the selected hyperbola branch.
    pub fn set_hyperbola_branch(
        &mut self,
        conic: ConicId,
        branch: HyperbolaBranch,
    ) -> Result<(), SketchError> {
        let mut kind = self.conic_value(conic)?.kind;
        let ConicKind::HyperbolaSegment {
            branch: current, ..
        } = &mut kind
        else {
            return Err(SketchError::InvalidConicBranchRole(conic));
        };
        *current = branch;
        self.validate_conic_kind(kind)?;
        self.conics
            .get_mut(conic)
            .ok_or(SketchError::UnknownConic(conic))?
            .kind = kind;
        Ok(())
    }

    /// Removes an unreferenced conic and leaves its runtime ID stale.
    pub fn remove_conic(&mut self, conic: ConicId) -> Result<ConicCurve, SketchError> {
        if self.constraint_references_conic(conic) {
            return Err(SketchError::ConicInUse(conic));
        }
        self.conics
            .remove(conic)
            .ok_or(SketchError::UnknownConic(conic))
    }

    pub(crate) fn conic_value(&self, conic: ConicId) -> Result<&ConicCurve, SketchError> {
        self.conics
            .get(conic)
            .ok_or(SketchError::UnknownConic(conic))
    }

    pub(crate) fn preflight_conics(&self) -> Result<(), SketchError> {
        for (_, conic) in self.conics.iter() {
            self.validate_conic_kind(conic.kind)?;
        }
        Ok(())
    }

    fn add_named_conic(
        &mut self,
        label: impl Into<String>,
        kind: ConicKind,
    ) -> Result<ConicId, SketchError> {
        let label = label.into();
        if label.trim().is_empty() {
            return Err(SketchError::EmptyLabel("conic"));
        }
        self.validate_conic_kind(kind)?;
        Ok(self.conics.insert(ConicCurve { kind, label }))
    }

    fn validate_conic_kind(&self, kind: ConicKind) -> Result<(), SketchError> {
        let geometry = conic_geometry_from_kind(kind, |point| self.point_position(point))?;
        validate_conic_geometry(geometry)
    }

    fn constraint_references_conic(&self, conic: ConicId) -> bool {
        self.constraints.iter().any(|(_, constraint)| {
            let references = |curve| matches!(curve, crate::SketchCurve::Conic(id) if id == conic);
            match constraint.kind() {
                SketchConstraintKind::PointOnCurve { contact, .. }
                | SketchConstraintKind::LineCurveTangency { contact, .. } => {
                    references(contact.curve)
                }
                SketchConstraintKind::CurveCurveContact { first, second }
                | SketchConstraintKind::CurveCurveTangency { first, second, .. } => {
                    references(first.curve) || references(second.curve)
                }
                _ => false,
            }
        })
    }
}

pub(crate) fn conic_geometry_from_kind(
    kind: ConicKind,
    mut point: impl FnMut(PointId) -> Result<Point2<f64>, SketchError>,
) -> Result<ConicGeometry, SketchError> {
    match kind {
        ConicKind::Ellipse {
            center,
            major_axis_point,
            minor_axis_ratio,
        } => {
            validate_minor_axis_ratio(minor_axis_ratio)?;
            let center = point(center)?;
            let axis_point = point(major_axis_point)?;
            let axis = axis_point - center;
            let semi_major = axis.norm();
            let direction = UnitDirection2::try_new(axis).map_err(SketchError::InvalidConic)?;
            let ellipse =
                Ellipse2::try_new(center, direction, semi_major, semi_major * minor_axis_ratio)
                    .map_err(SketchError::InvalidConic)?;
            Ok(ConicGeometry::Ellipse(ellipse))
        }
        ConicKind::EllipticalArc {
            center,
            major_axis_point,
            minor_axis_ratio,
            start_angle,
            signed_sweep,
        } => {
            validate_minor_axis_ratio(minor_axis_ratio)?;
            let center = point(center)?;
            let axis_point = point(major_axis_point)?;
            let axis = axis_point - center;
            let semi_major = axis.norm();
            let direction = UnitDirection2::try_new(axis).map_err(SketchError::InvalidConic)?;
            let ellipse =
                Ellipse2::try_new(center, direction, semi_major, semi_major * minor_axis_ratio)
                    .map_err(SketchError::InvalidConic)?;
            Ok(ConicGeometry::EllipticalArc(
                EllipticalArc2::try_new(ellipse, start_angle, signed_sweep)
                    .map_err(SketchError::InvalidConic)?,
            ))
        }
        ConicKind::RationalQuadratic {
            start,
            weighted_middle,
            middle_weight,
            end,
        } => Ok(ConicGeometry::RationalQuadratic(
            RationalQuadraticConicSegment2::try_new(
                point(start)?,
                weighted_middle,
                middle_weight,
                point(end)?,
            )
            .map_err(SketchError::InvalidConic)?,
        )),
        ConicKind::ParabolaSegment {
            vertex,
            focus,
            trim,
        } => {
            let vertex = point(vertex)?;
            let focus = point(focus)?;
            let axis = focus - vertex;
            let focal_length = axis.norm();
            let direction = UnitDirection2::try_new(axis).map_err(SketchError::InvalidConic)?;
            Ok(ConicGeometry::ParabolaSegment(
                ParabolaSegment2::try_new(vertex, direction, focal_length, trim)
                    .map_err(SketchError::InvalidConic)?,
            ))
        }
        ConicKind::HyperbolaSegment {
            center,
            transverse_axis_point,
            semi_conjugate,
            branch,
            trim,
        } => {
            validate_positive_conic_scalar(semi_conjugate)?;
            let center = point(center)?;
            let axis_point = point(transverse_axis_point)?;
            let axis = axis_point - center;
            let semi_transverse = axis.norm();
            let direction = UnitDirection2::try_new(axis).map_err(SketchError::InvalidConic)?;
            Ok(ConicGeometry::HyperbolaSegment(
                HyperbolaSegment2::try_new(
                    center,
                    direction,
                    semi_transverse,
                    semi_conjugate,
                    branch,
                    trim,
                )
                .map_err(SketchError::InvalidConic)?,
            ))
        }
    }
}

pub(crate) fn validate_conic_geometry(geometry: ConicGeometry) -> Result<(), SketchError> {
    let samples: &[f64] = match geometry {
        ConicGeometry::Ellipse(_) => &[0.0, std::f64::consts::FRAC_PI_2, std::f64::consts::PI],
        _ => &[0.0, 0.5, 1.0],
    };
    for &parameter in samples {
        geometry
            .evaluate(parameter)
            .map_err(SketchError::InvalidConicEvaluation)?;
    }
    geometry
        .endpoints()
        .map_err(SketchError::InvalidConicEvaluation)?;
    Ok(())
}

pub(crate) fn validate_minor_axis_ratio(value: f64) -> Result<(), SketchError> {
    if value.is_finite() && value > 0.0 && value <= 1.0 {
        Ok(())
    } else {
        Err(SketchError::InvalidConic(
            ConicDefinitionError::InvalidEllipseSemiaxes {
                semi_major: 1.0,
                semi_minor: value,
            },
        ))
    }
}

pub(crate) fn validate_positive_conic_scalar(value: f64) -> Result<(), SketchError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(SketchError::InvalidConic(
            ConicDefinitionError::InvalidHyperbolaSemiaxis { length: value },
        ))
    }
}
