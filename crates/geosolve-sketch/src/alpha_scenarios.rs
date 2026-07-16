use std::f64::consts::PI;

use crate::{
    ContactId, ContactNeighborhood, CurveDefinition, CurveId, CurveSpan, DesignPointId,
    DesignScalarId, DocumentArcSweep, DocumentArcTangencySide, DocumentConstraintDefinition,
    DocumentConstraintId, DocumentDimensionDefinition, DocumentDimensionId, DocumentDimensionMode,
    DocumentError, DocumentId, DocumentLineSide, DocumentSolveRequest, FeatureEndpoint,
    PersistentId, RectangleIds, ScalarDomain, ScalarUnit, SketchDocument, TangentOrientation,
};

/// Canonical playground-alpha scenarios shared by native tests and browser examples.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaScenarioKind {
    A1,
    A2,
    A3,
    A4,
    A5,
    A8,
    Corpus,
    StressCompass,
    StressBridge,
    MotionCam,
    MotionOrbit,
}

/// Deterministic workload sizes used by the M14 interaction budgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaPerformanceSize {
    Small,
    Medium,
}

impl AlphaScenarioKind {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::A1 => "a1",
            Self::A2 => "a2",
            Self::A3 => "a3",
            Self::A4 => "a4",
            Self::A5 => "a5",
            Self::A8 => "a8",
            Self::Corpus => "corpus",
            Self::StressCompass => "stress-compass",
            Self::StressBridge => "stress-bridge",
            Self::MotionCam => "motion-cam",
            Self::MotionOrbit => "motion-orbit",
        }
    }
}

/// Persistent roles in A1.
#[derive(Clone, Debug, PartialEq)]
pub struct A1ScenarioIds {
    pub rectangle: RectangleIds,
    pub diagonal_target: DesignScalarId,
    pub diagonal: DocumentDimensionId,
}

/// Persistent roles in A2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct A2ScenarioIds {
    pub a: DesignPointId,
    pub b: DesignPointId,
    pub c: DesignPointId,
    pub ab: CurveId,
    pub distance_ac: DocumentDimensionId,
}

/// Persistent roles in A3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct A3ScenarioIds {
    pub line: CurveId,
    pub guide: DesignPointId,
    pub center: DesignPointId,
    pub circle: CurveId,
    pub line_contact: ContactId,
    pub circle_contact: ContactId,
    pub tangency: DocumentConstraintId,
}

/// Persistent roles in A4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct A4ScenarioIds {
    pub arc_center: DesignPointId,
    pub circle_center: DesignPointId,
    pub arc: CurveId,
    pub circle: CurveId,
    pub circle_radius: DesignScalarId,
    pub circle_contact: ContactId,
    pub arc_contact: ContactId,
    pub tangency: DocumentConstraintId,
}

/// Persistent roles in A5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct A5ScenarioIds {
    pub controls: [DesignPointId; 4],
    pub a: DesignPointId,
    pub b: DesignPointId,
    pub line: CurveId,
    pub bezier: CurveId,
    pub bezier_contact: ContactId,
    pub tangency: DocumentConstraintId,
}

/// Persistent roles in the combined A8 round-trip document.
#[derive(Clone, Debug, PartialEq)]
pub struct A8ScenarioIds {
    pub a1: A1ScenarioIds,
    pub a3: A3ScenarioIds,
    pub a4: A4ScenarioIds,
    pub a5: A5ScenarioIds,
}

/// Persistent roles in the symmetric drafting-compass stress example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StressCompassIds {
    pub origin: DesignPointId,
    pub first_tip: DesignPointId,
    pub second_tip: DesignPointId,
    pub first_arm: CurveId,
    pub second_arm: CurveId,
    pub angle: DocumentDimensionId,
}

/// Persistent roles in the cubic Bezier C1-bridge stress example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StressBridgeIds {
    pub left_seam: DesignPointId,
    pub right_seam: DesignPointId,
    pub left_curve: CurveId,
    pub right_curve: CurveId,
    pub tangency: DocumentConstraintId,
    pub equal_handles: DocumentConstraintId,
}

/// Persistent roles in the twin-roller Bezier-cam motion example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionCamIds {
    pub controls: [DesignPointId; 3],
    pub left_center: DesignPointId,
    pub right_center: DesignPointId,
    pub cam: CurveId,
    pub left_circle: CurveId,
    pub right_circle: CurveId,
}

/// Persistent roles in the branch-preserving tangent-orbit motion example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionOrbitIds {
    pub fixed_center: DesignPointId,
    pub moving_center: DesignPointId,
    pub fixed_circle: CurveId,
    pub moving_circle: CurveId,
    pub tangency: DocumentConstraintId,
}

/// Scenario-specific persistent roles.
#[derive(Clone, Debug, PartialEq)]
pub enum AlphaScenarioIds {
    A1(A1ScenarioIds),
    A2(A2ScenarioIds),
    A3(A3ScenarioIds),
    A4(A4ScenarioIds),
    A5(A5ScenarioIds),
    A8(Box<A8ScenarioIds>),
    Corpus,
    StressCompass(StressCompassIds),
    StressBridge(StressBridgeIds),
    MotionCam(MotionCamIds),
    MotionOrbit(MotionOrbitIds),
}

/// One deterministic document and initial solve request for an alpha scenario.
#[derive(Clone, Debug, PartialEq)]
pub struct AlphaScenarioFixture {
    pub document: SketchDocument,
    pub request: DocumentSolveRequest,
    pub ids: AlphaScenarioIds,
}

/// Builds one canonical alpha scenario at a uniform model scale.
///
/// Persistent identities are invariant across scales for the same scenario.
///
/// # Errors
///
/// Returns a document validation error for a nonpositive/non-finite scale or an
/// unexpected construction failure.
pub fn alpha_scenario(
    kind: AlphaScenarioKind,
    scale: f64,
) -> Result<AlphaScenarioFixture, DocumentError> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(DocumentError::InvalidField {
            field: "alpha scenario scale",
            message: "must be positive and finite".into(),
        });
    }
    let namespace = match kind {
        AlphaScenarioKind::A1 => 0xa1_0000,
        AlphaScenarioKind::A2 => 0xa2_0000,
        AlphaScenarioKind::A3 => 0xa3_0000,
        AlphaScenarioKind::A4 => 0xa4_0000,
        AlphaScenarioKind::A5 => 0xa5_0000,
        AlphaScenarioKind::A8 => 0xa8_0000,
        AlphaScenarioKind::Corpus => 0xac_0000,
        AlphaScenarioKind::StressCompass => 0xc1_0000,
        AlphaScenarioKind::StressBridge => 0xc2_0000,
        AlphaScenarioKind::MotionCam => 0xc3_0000,
        AlphaScenarioKind::MotionOrbit => 0xc4_0000,
    };
    let mut document =
        SketchDocument::with_id(10.0 * scale, DocumentId(PersistentId::from_u128(namespace)))?;
    let (ids, request) = match kind {
        AlphaScenarioKind::A1 => (
            AlphaScenarioIds::A1(add_a1(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::A2 => (
            AlphaScenarioIds::A2(add_a2(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::A3 => (
            AlphaScenarioIds::A3(add_a3(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::A4 => (
            AlphaScenarioIds::A4(add_a4(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::A5 => (
            AlphaScenarioIds::A5(add_a5(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::A8 => {
            let a1 = add_a1(&mut document, scale, [0.0, 0.0])?;
            let a3 = add_a3(&mut document, scale, [20.0 * scale, 0.0])?;
            let a4 = add_a4(&mut document, scale, [40.0 * scale, 0.0])?;
            let a5 = add_a5(&mut document, scale, [60.0 * scale, 0.0])?;
            (
                AlphaScenarioIds::A8(Box::new(A8ScenarioIds { a1, a3, a4, a5 })),
                DocumentSolveRequest::default(),
            )
        }
        AlphaScenarioKind::Corpus => {
            add_alpha_corpus(&mut document, scale)?;
            (AlphaScenarioIds::Corpus, DocumentSolveRequest::default())
        }
        AlphaScenarioKind::StressCompass => (
            AlphaScenarioIds::StressCompass(add_stress_compass(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::StressBridge => (
            AlphaScenarioIds::StressBridge(add_stress_bridge(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::MotionCam => (
            AlphaScenarioIds::MotionCam(add_motion_cam(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::MotionOrbit => (
            AlphaScenarioIds::MotionOrbit(add_motion_orbit(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
    };
    Ok(AlphaScenarioFixture {
        document,
        request,
        ids,
    })
}

/// Builds a deterministic disconnected mixed-alpha performance document.
///
/// Small contains one A1/A3/A4/A5 tile. Medium contains eight translated tiles.
///
/// # Errors
///
/// Returns an unexpected document construction or validation error.
pub fn alpha_performance_document(
    size: AlphaPerformanceSize,
) -> Result<SketchDocument, DocumentError> {
    let count = match size {
        AlphaPerformanceSize::Small => 1,
        AlphaPerformanceSize::Medium => 8,
    };
    let namespace = match size {
        AlphaPerformanceSize::Small => 0xb1_0000,
        AlphaPerformanceSize::Medium => 0xb8_0000,
    };
    let mut document =
        SketchDocument::with_id(10.0, DocumentId(PersistentId::from_u128(namespace)))?;
    for index in 0..count {
        let origin = [f64::from(index) * 80.0, 0.0];
        add_a1(&mut document, 1.0, origin)?;
        add_a3(&mut document, 1.0, [origin[0] + 20.0, 0.0])?;
        add_a4(&mut document, 1.0, [origin[0] + 40.0, 0.0])?;
        add_a5(&mut document, 1.0, [origin[0] + 60.0, 0.0])?;
    }
    Ok(document)
}

fn translated(origin: [f64; 2], point: [f64; 2], scale: f64) -> [f64; 2] {
    [origin[0] + point[0] * scale, origin[1] + point[1] * scale]
}

fn add_line(
    document: &mut SketchDocument,
    label: &str,
    start: DesignPointId,
    end: DesignPointId,
) -> Result<CurveId, DocumentError> {
    let first = document
        .point(start)
        .ok_or(DocumentError::UnknownId {
            kind: "point",
            id: start.0,
        })?
        .position;
    let second = document
        .point(end)
        .ok_or(DocumentError::UnknownId {
            kind: "point",
            id: end.0,
        })?
        .position;
    let delta = [second[0] - first[0], second[1] - first[1]];
    let norm = delta[0].hypot(delta[1]);
    document.add_curve(
        label,
        CurveDefinition::Line {
            start,
            end,
            branch_direction: [delta[0] / norm, delta[1] / norm],
        },
    )
}

fn fix_point(
    document: &mut SketchDocument,
    label: &str,
    point: DesignPointId,
) -> Result<DocumentConstraintId, DocumentError> {
    let target = document
        .point(point)
        .ok_or(DocumentError::UnknownId {
            kind: "point",
            id: point.0,
        })?
        .position;
    document.add_constraint(
        label,
        DocumentConstraintDefinition::FixedPoint { point, target },
    )
}

fn add_length_dimension(
    document: &mut SketchDocument,
    label: &str,
    curve: CurveSpan,
    value: f64,
    mode: DocumentDimensionMode,
) -> Result<DocumentDimensionId, DocumentError> {
    let target = document.add_scalar(
        format!("{label} target"),
        value,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        label,
        DocumentDimensionDefinition::CurveLength { curve, target },
        mode,
    )
}

fn add_radius_dimension(
    document: &mut SketchDocument,
    label: &str,
    curve: CurveId,
    value: f64,
) -> Result<DocumentDimensionId, DocumentError> {
    let target = document.add_scalar(
        format!("{label} target"),
        value,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        label,
        DocumentDimensionDefinition::Radius { curve, target },
        DocumentDimensionMode::Driving,
    )
}

fn add_a1(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A1ScenarioIds, DocumentError> {
    let rectangle = document.add_rectangle("A1 rectangle", origin, 4.0 * scale, 3.0 * scale)?;
    let width_source = document
        .dimension(rectangle.dimensions[0])
        .expect("new rectangle width dimension")
        .source_id;
    document.set_source_label(width_source, "width-4")?;
    let diagonal_target = document.add_scalar(
        "A1 diagonal target",
        5.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let diagonal = document.add_dimension(
        "A1 diagonal reference",
        DocumentDimensionDefinition::PointDistance {
            first: rectangle.points[0],
            second: rectangle.points[2],
            target: diagonal_target,
        },
        DocumentDimensionMode::Reference,
    )?;
    Ok(A1ScenarioIds {
        rectangle,
        diagonal_target,
        diagonal,
    })
}

fn add_a2(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A2ScenarioIds, DocumentError> {
    let a = document.add_point("A2 A", translated(origin, [0.0, 0.0], scale))?;
    let b = document.add_point("A2 B", translated(origin, [4.0, 0.0], scale))?;
    let c = document.add_point("A2 C", translated(origin, [2.2, 2.0], scale))?;
    let ab = add_line(document, "A2 AB", a, b)?;
    fix_point(document, "A2 A fixed", a)?;
    document.add_constraint(
        "A2 AB horizontal",
        DocumentConstraintDefinition::Horizontal {
            line: CurveSpan::line(ab),
        },
    )?;
    add_length_dimension(
        document,
        "A2 AB length",
        CurveSpan::line(ab),
        4.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    let distance_target = document.add_scalar(
        "A2 AC target",
        3.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let distance_ac = document.add_dimension(
        "A2 AC distance",
        DocumentDimensionDefinition::PointDistance {
            first: a,
            second: c,
            target: distance_target,
        },
        DocumentDimensionMode::Driving,
    )?;
    Ok(A2ScenarioIds {
        a,
        b,
        c,
        ab,
        distance_ac,
    })
}

fn add_a3(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A3ScenarioIds, DocumentError> {
    let start = document.add_point("A3 line start", translated(origin, [-5.0, 0.0], scale))?;
    let end = document.add_point("A3 line end", translated(origin, [5.0, 0.0], scale))?;
    let guide = document.add_point("A3 guide G", translated(origin, [1.0, 0.0], scale))?;
    let center = document.add_point("A3 circle center O", translated(origin, [1.0, 3.0], scale))?;
    let line = add_line(document, "A3 fixed line", start, end)?;
    let guide_line = add_line(document, "A3 vertical guide", guide, center)?;
    let radius = document.add_scalar(
        "A3 circle radius",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let circle = document.add_curve("A3 circle", CurveDefinition::Circle { center, radius })?;
    fix_point(document, "A3 line start fixed", start)?;
    fix_point(document, "A3 line end fixed", end)?;
    fix_point(document, "A3 guide fixed", guide)?;
    document.add_constraint(
        "A3 center guide vertical",
        DocumentConstraintDefinition::Vertical {
            line: CurveSpan::line(guide_line),
        },
    )?;
    add_radius_dimension(document, "A3 radius 2", circle, 2.0 * scale)?;
    let line_contact = document.add_curve_contact(
        "A3 line contact",
        CurveSpan::line(line),
        0.6,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    let circle_contact = document.add_curve_contact(
        "A3 circle contact",
        CurveSpan::line(circle),
        1.5 * PI,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    let tangency = document.add_constraint(
        "A3 line-circle tangency",
        DocumentConstraintDefinition::LineCircleTangency {
            line_contact,
            circle_contact,
            side: DocumentLineSide::Left,
        },
    )?;
    Ok(A3ScenarioIds {
        line,
        guide,
        center,
        circle,
        line_contact,
        circle_contact,
        tangency,
    })
}

fn add_a4(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A4ScenarioIds, DocumentError> {
    let arc_center = document.add_point("A4 arc center", origin)?;
    let circle_center =
        document.add_point("A4 circle center", translated(origin, [8.0, 0.0], scale))?;
    let arc_radius = document.add_scalar(
        "A4 arc radius",
        5.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let start_angle = document.add_scalar(
        "A4 arc start",
        -5.0 * PI / 6.0,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    let end_angle = document.add_scalar(
        "A4 arc end",
        5.0 * PI / 6.0,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    let arc = document.add_curve(
        "A4 300 degree arc",
        CurveDefinition::CircularArc {
            center: arc_center,
            radius: arc_radius,
            start_angle,
            end_angle,
            sweep: DocumentArcSweep::CounterClockwise,
        },
    )?;
    let circle_radius = document.add_scalar(
        "A4 free circle radius",
        3.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let circle = document.add_curve(
        "A4 free-radius circle",
        CurveDefinition::Circle {
            center: circle_center,
            radius: circle_radius,
        },
    )?;
    fix_point(document, "A4 arc center fixed", arc_center)?;
    add_radius_dimension(document, "A4 arc radius 5", arc, 5.0 * scale)?;
    let circle_contact = document.add_curve_contact(
        "A4 circle contact",
        CurveSpan::line(circle),
        PI,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let arc_contact = document.add_curve_contact(
        "A4 arc contact",
        CurveSpan::line(arc),
        0.5,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let tangency = document.add_constraint(
        "A4 circle-arc tangency",
        DocumentConstraintDefinition::CircleArcTangency {
            circle_contact,
            arc_contact,
            side: DocumentArcTangencySide::OutsideArc,
        },
    )?;
    Ok(A4ScenarioIds {
        arc_center,
        circle_center,
        arc,
        circle,
        circle_radius,
        circle_contact,
        arc_contact,
        tangency,
    })
}

fn add_a5(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A5ScenarioIds, DocumentError> {
    let controls = [
        document.add_point("A5 P0", translated(origin, [0.0, 0.0], scale))?,
        document.add_point("A5 P1", translated(origin, [1.0, 0.0], scale))?,
        document.add_point("A5 P2", translated(origin, [2.0, 1.0], scale))?,
        document.add_point("A5 P3", translated(origin, [3.0, 1.0], scale))?,
    ];
    let a = document.add_point("A5 line A", translated(origin, [0.0, 0.0], scale))?;
    let b = document.add_point("A5 line B", translated(origin, [2.0, 0.0], scale))?;
    let line = add_line(document, "A5 tangent line", a, b)?;
    let bezier =
        document.add_curve("A5 cubic Bezier", CurveDefinition::CubicBezier { controls })?;
    fix_point(document, "A5 A fixed", a)?;
    add_length_dimension(
        document,
        "A5 line length 2",
        CurveSpan::line(line),
        2.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    let bezier_contact = document.add_curve_contact(
        "A5 Bezier start contact",
        CurveSpan::line(bezier),
        0.0,
        0,
        ContactNeighborhood::Start,
        Some(TangentOrientation::Aligned),
    )?;
    let tangency = document.add_constraint(
        "A5 line-Bezier tangency",
        DocumentConstraintDefinition::LineCurveTangency {
            line: CurveSpan::line(line),
            endpoint: FeatureEndpoint::Start,
            curve_contact: bezier_contact,
        },
    )?;
    Ok(A5ScenarioIds {
        controls,
        a,
        b,
        line,
        bezier,
        bezier_contact,
        tangency,
    })
}

fn add_stress_compass(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<StressCompassIds, DocumentError> {
    let sqrt_three = 3.0_f64.sqrt();
    let origin = document.add_point("Compass pivot O", [0.0, 0.0])?;
    let axis_tip = document.add_point(
        "Compass fixed bisector K",
        [2.5 * sqrt_three * scale, 2.5 * scale],
    )?;
    let first_tip = document.add_point("Compass tip A", [4.0 * scale, 0.0])?;
    let second_tip =
        document.add_point("Compass tip B", [2.0 * scale, 2.0 * sqrt_three * scale])?;
    let axis = add_line(document, "Compass symmetry axis", origin, axis_tip)?;
    let first_arm = add_line(document, "Compass arm OA", origin, first_tip)?;
    let second_arm = add_line(document, "Compass arm OB", origin, second_tip)?;
    let chord = add_line(document, "Compass chord AB", first_tip, second_tip)?;
    fix_point(document, "Compass pivot fixed", origin)?;
    fix_point(document, "Compass bisector fixed", axis_tip)?;
    document.add_constraint(
        "Compass symmetric tips",
        DocumentConstraintDefinition::SymmetricAboutLine {
            first: first_tip,
            second: second_tip,
            line: CurveSpan::line(axis),
        },
    )?;
    document.add_constraint(
        "Compass equal arms",
        DocumentConstraintDefinition::EqualLength {
            first: CurveSpan::line(first_arm),
            second: CurveSpan::line(second_arm),
        },
    )?;
    add_length_dimension(
        document,
        "Compass arm length 4",
        CurveSpan::line(first_arm),
        4.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    add_length_dimension(
        document,
        "Compass second arm reference",
        CurveSpan::line(second_arm),
        4.0 * scale,
        DocumentDimensionMode::Reference,
    )?;
    add_length_dimension(
        document,
        "Compass chord reference",
        CurveSpan::line(chord),
        4.0 * scale,
        DocumentDimensionMode::Reference,
    )?;
    let angle_target = document.add_scalar(
        "Compass opening angle target",
        PI / 3.0,
        ScalarUnit::Angle,
        ScalarDomain::Positive,
    )?;
    let angle = document.add_dimension(
        "Compass opening angle 60 deg",
        DocumentDimensionDefinition::OrientedAngle {
            first: CurveSpan::line(first_arm),
            second: CurveSpan::line(second_arm),
            target: angle_target,
            orientation: crate::DocumentAngleOrientation::CounterClockwise,
        },
        DocumentDimensionMode::Reference,
    )?;
    Ok(StressCompassIds {
        origin,
        first_tip,
        second_tip,
        first_arm,
        second_arm,
        angle,
    })
}

fn add_stress_bridge(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<StressBridgeIds, DocumentError> {
    let left = [
        document.add_point("Bridge left P0", [-4.0 * scale, 0.0])?,
        document.add_point("Bridge left P1", [-3.0 * scale, 2.0 * scale])?,
        document.add_point("Bridge left P2", [-scale, 2.0 * scale])?,
        document.add_point("Bridge left seam", [0.0, 0.0])?,
    ];
    let right = [
        document.add_point("Bridge right seam", [0.0, 0.0])?,
        document.add_point("Bridge right P1", [1.0 * scale, -2.0 * scale])?,
        document.add_point("Bridge right P2", [3.0 * scale, -2.0 * scale])?,
        document.add_point("Bridge right P3", [4.0 * scale, 0.0])?,
    ];
    let left_curve = document.add_curve(
        "Bridge left cubic Bezier",
        CurveDefinition::CubicBezier { controls: left },
    )?;
    let right_curve = document.add_curve(
        "Bridge right cubic Bezier",
        CurveDefinition::CubicBezier { controls: right },
    )?;
    let handle_direction = [1.0 / 5.0_f64.sqrt(), -2.0 / 5.0_f64.sqrt()];
    let left_handle = document.add_curve(
        "Bridge left seam handle",
        CurveDefinition::Line {
            start: left[2],
            end: left[3],
            branch_direction: handle_direction,
        },
    )?;
    let right_handle = document.add_curve(
        "Bridge right seam handle",
        CurveDefinition::Line {
            start: right[0],
            end: right[1],
            branch_direction: handle_direction,
        },
    )?;
    for (index, point) in [left[0], left[1], left[2], right[1], right[2], right[3]]
        .into_iter()
        .enumerate()
    {
        fix_point(
            document,
            &format!("Bridge outer control {} fixed", index + 1),
            point,
        )?;
    }
    let left_contact = document.add_curve_contact(
        "Bridge left endpoint contact",
        CurveSpan::line(left_curve),
        1.0,
        0,
        ContactNeighborhood::End,
        Some(TangentOrientation::Aligned),
    )?;
    let right_contact = document.add_curve_contact(
        "Bridge right endpoint contact",
        CurveSpan::line(right_curve),
        0.0,
        0,
        ContactNeighborhood::Start,
        Some(TangentOrientation::Aligned),
    )?;
    let tangency = document.add_constraint(
        "Bridge C1 endpoint tangency",
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: left_contact,
            second_contact: right_contact,
        },
    )?;
    let equal_handles = document.add_constraint(
        "Bridge equal seam handles",
        DocumentConstraintDefinition::EqualLength {
            first: CurveSpan::line(left_handle),
            second: CurveSpan::line(right_handle),
        },
    )?;
    suppress_constraint(document, equal_handles)?;
    add_length_dimension(
        document,
        "Bridge left handle reference",
        CurveSpan::line(left_handle),
        5.0_f64.sqrt() * scale,
        DocumentDimensionMode::Reference,
    )?;
    add_length_dimension(
        document,
        "Bridge right handle reference",
        CurveSpan::line(right_handle),
        5.0_f64.sqrt() * scale,
        DocumentDimensionMode::Reference,
    )?;
    Ok(StressBridgeIds {
        left_seam: left[3],
        right_seam: right[0],
        left_curve,
        right_curve,
        tangency,
        equal_handles,
    })
}

fn add_motion_cam(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionCamIds, DocumentError> {
    let controls = [
        document.add_point("Cam Q0", [-4.0 * scale, 0.0])?,
        document.add_point("Cam Q1", [0.0, 4.0 * scale])?,
        document.add_point("Cam Q2", [4.0 * scale, 0.0])?,
    ];
    let cam = document.add_curve(
        "Quadratic Bezier cam",
        CurveDefinition::QuadraticBezier { controls },
    )?;
    for (index, point) in controls.into_iter().enumerate() {
        fix_point(document, &format!("Cam Q{index} fixed"), point)?;
    }
    let inverse_sqrt_five = 1.0 / 5.0_f64.sqrt();
    let left_center = document.add_point(
        "Left roller center",
        [
            (-2.0 - inverse_sqrt_five) * scale,
            (1.5 + 2.0 * inverse_sqrt_five) * scale,
        ],
    )?;
    let right_center = document.add_point(
        "Right roller center",
        [
            (2.0 + inverse_sqrt_five) * scale,
            (1.5 + 2.0 * inverse_sqrt_five) * scale,
        ],
    )?;
    let left_radius = document.add_scalar(
        "Left roller radius",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let right_radius = document.add_scalar(
        "Right roller radius",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let left_circle = document.add_curve(
        "Left cam roller",
        CurveDefinition::Circle {
            center: left_center,
            radius: left_radius,
        },
    )?;
    let right_circle = document.add_curve(
        "Right cam roller",
        CurveDefinition::Circle {
            center: right_center,
            radius: right_radius,
        },
    )?;
    add_radius_dimension(document, "Cam roller radius 1", left_circle, scale)?;
    document.add_constraint(
        "Cam rollers equal radius",
        DocumentConstraintDefinition::EqualRadius {
            first: left_circle,
            second: right_circle,
        },
    )?;
    add_motion_cam_tangency(
        document,
        "Left roller",
        cam,
        0.25,
        left_circle,
        2.0 * PI - 2.0_f64.atan(),
    )?;
    add_motion_cam_tangency(
        document,
        "Right roller",
        cam,
        0.75,
        right_circle,
        PI + 2.0_f64.atan(),
    )?;
    let diameter_target = document.add_scalar(
        "Right roller diameter target",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "Right roller diameter reference",
        DocumentDimensionDefinition::Diameter {
            curve: right_circle,
            target: diameter_target,
        },
        DocumentDimensionMode::Reference,
    )?;
    Ok(MotionCamIds {
        controls,
        left_center,
        right_center,
        cam,
        left_circle,
        right_circle,
    })
}

fn add_motion_orbit(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionOrbitIds, DocumentError> {
    let fixed_center = document.add_point("Orbit fixed center", [0.0, 0.0])?;
    let moving_center = document.add_point("Orbit satellite center", [4.0 * scale, 0.0])?;
    let fixed_radius = document.add_scalar(
        "Orbit fixed radius",
        3.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let moving_radius = document.add_scalar(
        "Orbit satellite radius",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let fixed_circle = document.add_curve(
        "Orbit fixed circle",
        CurveDefinition::Circle {
            center: fixed_center,
            radius: fixed_radius,
        },
    )?;
    let moving_circle = document.add_curve(
        "Orbit tangent satellite",
        CurveDefinition::Circle {
            center: moving_center,
            radius: moving_radius,
        },
    )?;
    fix_point(document, "Orbit center fixed", fixed_center)?;
    add_radius_dimension(document, "Orbit fixed radius 3", fixed_circle, 3.0 * scale)?;
    add_radius_dimension(document, "Orbit satellite radius 1", moving_circle, scale)?;
    let fixed_contact = document.add_curve_contact(
        "Orbit fixed-circle contact",
        CurveSpan::line(fixed_circle),
        0.0,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let moving_contact = document.add_curve_contact(
        "Orbit satellite contact",
        CurveSpan::line(moving_circle),
        PI,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let tangency = document.add_constraint(
        "Orbit external tangency",
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: fixed_contact,
            second_contact: moving_contact,
        },
    )?;
    let distance_target = document.add_scalar(
        "Orbit center distance target",
        4.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "Orbit center distance reference",
        DocumentDimensionDefinition::PointDistance {
            first: fixed_center,
            second: moving_center,
            target: distance_target,
        },
        DocumentDimensionMode::Reference,
    )?;
    Ok(MotionOrbitIds {
        fixed_center,
        moving_center,
        fixed_circle,
        moving_circle,
        tangency,
    })
}

#[allow(clippy::too_many_lines)]
fn add_alpha_corpus(document: &mut SketchDocument, scale: f64) -> Result<(), DocumentError> {
    add_a1(document, scale, [0.0, 0.0])?;
    add_a3(document, scale, [20.0 * scale, 0.0])?;
    add_a4(document, scale, [40.0 * scale, 0.0])?;
    add_corpus_bezier_tangency(document, scale)?;

    document.add_point("corpus point", [80.0 * scale, 0.0])?;
    let polyline_points = [
        document.add_point("corpus polyline A", [82.0 * scale, 0.0])?,
        document.add_point("corpus polyline B", [83.0 * scale, scale])?,
        document.add_point("corpus polyline C", [84.0 * scale, 0.0])?,
    ];
    document.add_curve(
        "corpus polyline",
        CurveDefinition::Polyline {
            points: polyline_points.to_vec(),
            closed: false,
            branch_directions: [
                [
                    std::f64::consts::FRAC_1_SQRT_2,
                    std::f64::consts::FRAC_1_SQRT_2,
                ],
                [
                    std::f64::consts::FRAC_1_SQRT_2,
                    -std::f64::consts::FRAC_1_SQRT_2,
                ],
            ]
            .to_vec(),
        },
    )?;
    let quadratic_controls = [
        document.add_point("corpus quadratic A", [86.0 * scale, 0.0])?,
        document.add_point("corpus quadratic B", [87.0 * scale, scale])?,
        document.add_point("corpus quadratic C", [88.0 * scale, 0.0])?,
    ];
    document.add_curve(
        "corpus quadratic",
        CurveDefinition::QuadraticBezier {
            controls: quadratic_controls,
        },
    )?;

    let coincident_a = document.add_point("corpus coincident A", [100.0 * scale, 0.0])?;
    let coincident_b = document.add_point("corpus coincident B", [100.0 * scale, 0.0])?;
    document.add_constraint(
        "corpus coincident",
        DocumentConstraintDefinition::Coincident {
            first: coincident_a,
            second: coincident_b,
        },
    )?;

    let on_line_a = document.add_point("corpus on-line A", [109.0 * scale, 0.0])?;
    let on_line_b = document.add_point("corpus on-line B", [111.0 * scale, 0.0])?;
    let on_line_point = document.add_point("corpus on-line point", [110.0 * scale, 0.0])?;
    let on_line = add_line(document, "corpus point line", on_line_a, on_line_b)?;
    let point_contact = document.add_curve_contact(
        "corpus point contact",
        CurveSpan::line(on_line),
        0.5,
        0,
        ContactNeighborhood::Interior,
        None,
    )?;
    document.add_constraint(
        "corpus point on curve",
        DocumentConstraintDefinition::PointOnCurve {
            point: on_line_point,
            contact: point_contact,
        },
    )?;

    let parallel_first = corpus_line(
        document,
        "corpus parallel first",
        scale,
        120.0,
        0.0,
        2.0,
        0.0,
    )?;
    let parallel_second = corpus_line(
        document,
        "corpus parallel second",
        scale,
        120.0,
        1.0,
        2.0,
        0.0,
    )?;
    document.add_constraint(
        "corpus parallel",
        DocumentConstraintDefinition::Parallel {
            first: CurveSpan::line(parallel_first),
            second: CurveSpan::line(parallel_second),
        },
    )?;

    let perpendicular_first = corpus_line(
        document,
        "corpus perpendicular first",
        scale,
        130.0,
        0.0,
        2.0,
        0.0,
    )?;
    let perpendicular_second = corpus_line(
        document,
        "corpus perpendicular second",
        scale,
        131.0,
        -1.0,
        0.0,
        2.0,
    )?;
    document.add_constraint(
        "corpus perpendicular",
        DocumentConstraintDefinition::Perpendicular {
            first: CurveSpan::line(perpendicular_first),
            second: CurveSpan::line(perpendicular_second),
        },
    )?;

    let equal_length_first = corpus_line(
        document,
        "corpus equal length first",
        scale,
        140.0,
        0.0,
        2.0,
        0.0,
    )?;
    let equal_length_second = corpus_line(
        document,
        "corpus equal length second",
        scale,
        140.0,
        1.0,
        0.0,
        2.0,
    )?;
    document.add_constraint(
        "corpus equal length",
        DocumentConstraintDefinition::EqualLength {
            first: CurveSpan::line(equal_length_first),
            second: CurveSpan::line(equal_length_second),
        },
    )?;

    let equal_center_a = document.add_point("corpus radius center A", [150.0 * scale, 0.0])?;
    let equal_center_b = document.add_point("corpus radius center B", [153.0 * scale, 0.0])?;
    let equal_radius_a = document.add_scalar(
        "corpus radius A",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let equal_radius_b = document.add_scalar(
        "corpus radius B",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let equal_circle_a = document.add_curve(
        "corpus radius circle A",
        CurveDefinition::Circle {
            center: equal_center_a,
            radius: equal_radius_a,
        },
    )?;
    let equal_circle_b = document.add_curve(
        "corpus radius circle B",
        CurveDefinition::Circle {
            center: equal_center_b,
            radius: equal_radius_b,
        },
    )?;
    document.add_constraint(
        "corpus equal radius",
        DocumentConstraintDefinition::EqualRadius {
            first: equal_circle_a,
            second: equal_circle_b,
        },
    )?;

    let midpoint_line = corpus_line(
        document,
        "corpus midpoint line",
        scale,
        159.0,
        0.0,
        2.0,
        0.0,
    )?;
    let midpoint = document.add_point("corpus midpoint", [160.0 * scale, 0.0])?;
    document.add_constraint(
        "corpus midpoint",
        DocumentConstraintDefinition::Midpoint {
            point: midpoint,
            line: CurveSpan::line(midpoint_line),
        },
    )?;

    let symmetry_axis = corpus_line(
        document,
        "corpus symmetry axis",
        scale,
        168.0,
        0.0,
        4.0,
        0.0,
    )?;
    let symmetry_a = document.add_point("corpus symmetry A", [170.0 * scale, scale])?;
    let symmetry_b = document.add_point("corpus symmetry B", [170.0 * scale, -scale])?;
    document.add_constraint(
        "corpus symmetry",
        DocumentConstraintDefinition::SymmetricAboutLine {
            first: symmetry_a,
            second: symmetry_b,
            line: CurveSpan::line(symmetry_axis),
        },
    )?;

    let contact_first = corpus_line(
        document,
        "corpus contact first",
        scale,
        179.0,
        0.0,
        2.0,
        0.0,
    )?;
    let contact_second = corpus_line(
        document,
        "corpus contact second",
        scale,
        180.0,
        -1.0,
        0.0,
        2.0,
    )?;
    let first_contact = document.add_curve_contact(
        "corpus first curve contact",
        CurveSpan::line(contact_first),
        0.5,
        0,
        ContactNeighborhood::Interior,
        None,
    )?;
    let second_contact = document.add_curve_contact(
        "corpus second curve contact",
        CurveSpan::line(contact_second),
        0.5,
        0,
        ContactNeighborhood::Interior,
        None,
    )?;
    document.add_constraint(
        "corpus curve contact",
        DocumentConstraintDefinition::CurveCurveContact {
            first_contact,
            second_contact,
        },
    )?;

    let tangent_first = corpus_line(
        document,
        "corpus tangent first",
        scale,
        189.0,
        0.0,
        2.0,
        0.0,
    )?;
    let tangent_second = corpus_line(
        document,
        "corpus tangent second",
        scale,
        189.0,
        0.0,
        2.0,
        0.0,
    )?;
    let first_tangent = document.add_curve_contact(
        "corpus first tangent contact",
        CurveSpan::line(tangent_first),
        0.5,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    let second_tangent = document.add_curve_contact(
        "corpus second tangent contact",
        CurveSpan::line(tangent_second),
        0.5,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    document.add_constraint(
        "corpus curve tangency",
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: first_tangent,
            second_contact: second_tangent,
        },
    )?;

    let distance_a = document.add_point("corpus distance A", [200.0 * scale, 0.0])?;
    let distance_b = document.add_point("corpus distance B", [202.0 * scale, 0.0])?;
    let distance_target = document.add_scalar(
        "corpus distance target",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "corpus distance",
        DocumentDimensionDefinition::PointDistance {
            first: distance_a,
            second: distance_b,
            target: distance_target,
        },
        DocumentDimensionMode::Driving,
    )?;

    let diameter_center = document.add_point("corpus diameter center", [210.0 * scale, 0.0])?;
    let diameter_radius = document.add_scalar(
        "corpus diameter radius",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let diameter_circle = document.add_curve(
        "corpus diameter circle",
        CurveDefinition::Circle {
            center: diameter_center,
            radius: diameter_radius,
        },
    )?;
    let diameter_target = document.add_scalar(
        "corpus diameter target",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "corpus diameter",
        DocumentDimensionDefinition::Diameter {
            curve: diameter_circle,
            target: diameter_target,
        },
        DocumentDimensionMode::Driving,
    )?;

    let angle_first = corpus_line(document, "corpus angle first", scale, 220.0, 0.0, 2.0, 0.0)?;
    let angle_second = corpus_line(document, "corpus angle second", scale, 220.0, 0.0, 0.0, 2.0)?;
    let angle_target = document.add_scalar(
        "corpus angle target",
        std::f64::consts::FRAC_PI_2,
        ScalarUnit::Angle,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "corpus angle",
        DocumentDimensionDefinition::OrientedAngle {
            first: CurveSpan::line(angle_first),
            second: CurveSpan::line(angle_second),
            target: angle_target,
            orientation: crate::DocumentAngleOrientation::CounterClockwise,
        },
        DocumentDimensionMode::Driving,
    )?;
    Ok(())
}

fn corpus_line(
    document: &mut SketchDocument,
    label: &str,
    scale: f64,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
) -> Result<CurveId, DocumentError> {
    let start = document.add_point(format!("{label} A"), [x * scale, y * scale])?;
    let end = document.add_point(format!("{label} B"), [(x + dx) * scale, (y + dy) * scale])?;
    add_line(document, label, start, end)
}

fn suppress_constraint(
    document: &mut SketchDocument,
    constraint: DocumentConstraintId,
) -> Result<(), DocumentError> {
    let source = document
        .constraint(constraint)
        .expect("new scenario constraint")
        .source_id;
    document.set_source_suppressed(source, true)
}

fn add_motion_cam_tangency(
    document: &mut SketchDocument,
    roller_label: &str,
    cam: CurveId,
    cam_parameter: f64,
    circle: CurveId,
    circle_parameter: f64,
) -> Result<DocumentConstraintId, DocumentError> {
    let cam_contact = document.add_curve_contact(
        format!("{roller_label} cam contact"),
        CurveSpan::line(cam),
        cam_parameter,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    let circle_contact = document.add_curve_contact(
        format!("{roller_label} circle contact"),
        CurveSpan::line(circle),
        circle_parameter,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    document.add_constraint(
        format!("{roller_label} tangent to cam"),
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: cam_contact,
            second_contact: circle_contact,
        },
    )
}

fn add_corpus_bezier_tangency(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<(), DocumentError> {
    let controls = [
        document.add_point("corpus cubic A", [60.0 * scale, 0.0])?,
        document.add_point("corpus cubic B", [61.0 * scale, 0.0])?,
        document.add_point("corpus cubic C", [62.0 * scale, 0.0])?,
        document.add_point("corpus cubic D", [63.0 * scale, 0.0])?,
    ];
    let bezier = document.add_curve(
        "corpus cubic Bezier",
        CurveDefinition::CubicBezier { controls },
    )?;
    let line_start = document.add_point("corpus Bezier line A", [61.5 * scale, 0.0])?;
    let line_end = document.add_point("corpus Bezier line B", [63.5 * scale, 0.0])?;
    let line = add_line(document, "corpus Bezier tangent line", line_start, line_end)?;
    let contact = document.add_curve_contact(
        "corpus Bezier tangent contact",
        CurveSpan::line(bezier),
        0.5,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    document.add_constraint(
        "corpus line-Bezier tangency",
        DocumentConstraintDefinition::LineCurveTangency {
            line: CurveSpan::line(line),
            endpoint: FeatureEndpoint::Start,
            curve_contact: contact,
        },
    )?;
    Ok(())
}
