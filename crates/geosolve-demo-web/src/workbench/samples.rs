// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::fmt::Write as _;

use geosolve_constraint_editor::RetainedEditorCoordinator;
use geosolve_core::SolverConfig;
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, ContactNeighborhood, CurveDefinition, CurveId, CurveSpan,
    DesignPointId, DocumentConstraintDefinition, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentId, GeometryRole, PersistentId, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDocument, TangentOrientation, alpha_scenario,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SampleId {
    DraftingCompass,
    BezierContinuityBridge,
    TwinRollerCam,
    TangentOrbit,
    EllipticTrammel,
    ScotchYoke,
    RotatingConstraintSquare,
    ScissorJack,
    ScissorTower,
    PeaucellierInversor,
    FourBarCoupler,
    Pantograph,
    DrawingArm,
    ConstraintSampler,
    TangentRadialNormal,
    ContactBranch,
    AngleDimensions,
    ContextualAnnotations,
    DenseJunction,
    ConstructionReference,
    CurveGallery,
    PeriodicNurbs,
    FilletWorkshop,
    LineOffsetWorkshop,
    MirrorWorkshop,
}

impl SampleId {
    pub(crate) const ALL: [Self; 25] = [
        Self::DraftingCompass,
        Self::BezierContinuityBridge,
        Self::TwinRollerCam,
        Self::TangentOrbit,
        Self::EllipticTrammel,
        Self::ScotchYoke,
        Self::RotatingConstraintSquare,
        Self::ScissorJack,
        Self::ScissorTower,
        Self::PeaucellierInversor,
        Self::FourBarCoupler,
        Self::Pantograph,
        Self::DrawingArm,
        Self::ConstraintSampler,
        Self::TangentRadialNormal,
        Self::ContactBranch,
        Self::AngleDimensions,
        Self::ContextualAnnotations,
        Self::DenseJunction,
        Self::ConstructionReference,
        Self::CurveGallery,
        Self::PeriodicNurbs,
        Self::FilletWorkshop,
        Self::LineOffsetWorkshop,
        Self::MirrorWorkshop,
    ];

    pub(crate) const fn key(self) -> &'static str {
        match self {
            Self::DraftingCompass => "drafting-compass",
            Self::BezierContinuityBridge => "bezier-continuity-bridge",
            Self::TwinRollerCam => "twin-roller-cam",
            Self::TangentOrbit => "tangent-orbit",
            Self::EllipticTrammel => "elliptic-trammel",
            Self::ScotchYoke => "scotch-yoke",
            Self::RotatingConstraintSquare => "rotating-constraint-square",
            Self::ScissorJack => "scissor-jack",
            Self::ScissorTower => "five-stage-scissor-tower",
            Self::PeaucellierInversor => "peaucellier-inversor",
            Self::FourBarCoupler => "four-bar-coupler",
            Self::Pantograph => "pantograph-linkage",
            Self::DrawingArm => "three-link-drawing-arm",
            Self::ConstraintSampler => "constraint-dimension-sampler",
            Self::TangentRadialNormal => "tangent-radial-normal",
            Self::ContactBranch => "contact-branch-specimen",
            Self::AngleDimensions => "angle-dimension-annotations",
            Self::ContextualAnnotations => "contextual-constraint-annotations",
            Self::DenseJunction => "dense-constraint-junction",
            Self::ConstructionReference => "construction-reference-geometry",
            Self::CurveGallery => "curve-family-gallery",
            Self::PeriodicNurbs => "periodic-nurbs-specimen",
            Self::FilletWorkshop => "fillet-workshop",
            Self::LineOffsetWorkshop => "line-offset-workshop",
            Self::MirrorWorkshop => "mirror-workshop",
        }
    }

    pub(crate) fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|sample| sample.key() == key)
    }
}

#[derive(Clone, Copy)]
enum SampleSource {
    Alpha(AlphaScenarioKind),
    ConstructionReference,
    TangentRadialNormal,
    FilletWorkshop,
    LineOffsetWorkshop,
    MirrorWorkshop,
}

#[derive(Clone, Copy)]
pub(crate) struct SampleDefinition {
    pub(crate) id: SampleId,
    pub(crate) title: &'static str,
    source: SampleSource,
}

#[derive(Clone, Copy)]
pub(crate) struct SampleGroup {
    pub(crate) title: &'static str,
    pub(crate) samples: &'static [SampleDefinition],
}

const MECHANISMS: [SampleDefinition; 13] = [
    sample(
        SampleId::DraftingCompass,
        "Drafting compass · 1 DOF",
        AlphaScenarioKind::StressCompass,
    ),
    sample(
        SampleId::BezierContinuityBridge,
        "Bezier continuity bridge · 1 DOF",
        AlphaScenarioKind::StressBridge,
    ),
    sample(
        SampleId::TwinRollerCam,
        "Twin-roller cam · 2 DOF",
        AlphaScenarioKind::MotionCam,
    ),
    sample(
        SampleId::TangentOrbit,
        "Tangent orbit · 1 DOF",
        AlphaScenarioKind::MotionOrbit,
    ),
    sample(
        SampleId::EllipticTrammel,
        "Elliptic trammel · 1 DOF",
        AlphaScenarioKind::MotionTrammel,
    ),
    sample(
        SampleId::ScotchYoke,
        "Scotch yoke · 1 DOF",
        AlphaScenarioKind::MotionScotchYoke,
    ),
    sample(
        SampleId::RotatingConstraintSquare,
        "Rotating constraint square · 1 DOF",
        AlphaScenarioKind::MotionRotatingSquare,
    ),
    sample(
        SampleId::ScissorJack,
        "Scissor jack · 1 DOF",
        AlphaScenarioKind::MotionScissor,
    ),
    sample(
        SampleId::ScissorTower,
        "Five-stage scissor tower · 1 DOF",
        AlphaScenarioKind::MotionScissorTower,
    ),
    sample(
        SampleId::PeaucellierInversor,
        "Peaucellier inversor · 1 DOF",
        AlphaScenarioKind::MotionPeaucellier,
    ),
    sample(
        SampleId::FourBarCoupler,
        "Four-bar coupler · 1 DOF",
        AlphaScenarioKind::MotionFourBarCoupler,
    ),
    sample(
        SampleId::Pantograph,
        "Pantograph linkage · 2 DOF",
        AlphaScenarioKind::MotionPantograph,
    ),
    sample(
        SampleId::DrawingArm,
        "Three-link drawing arm · 3 DOF",
        AlphaScenarioKind::MotionDrawingArm,
    ),
];

const CONSTRAINTS: [SampleDefinition; 6] = [
    sample(
        SampleId::ConstraintSampler,
        "Constraint and dimension sampler",
        AlphaScenarioKind::Corpus,
    ),
    SampleDefinition {
        id: SampleId::TangentRadialNormal,
        title: "Tangent and radial-normal construction",
        source: SampleSource::TangentRadialNormal,
    },
    sample(
        SampleId::ContactBranch,
        "Contact branch specimen",
        AlphaScenarioKind::A3,
    ),
    sample(
        SampleId::AngleDimensions,
        "Angle and dimension annotations",
        AlphaScenarioKind::DirectedAngle,
    ),
    sample(
        SampleId::ContextualAnnotations,
        "Contextual constraint annotations",
        AlphaScenarioKind::Corpus,
    ),
    sample(
        SampleId::DenseJunction,
        "Dense constraint junction",
        AlphaScenarioKind::MotionRotatingSquare,
    ),
];

const CURVES: [SampleDefinition; 6] = [
    SampleDefinition {
        id: SampleId::ConstructionReference,
        title: "Construction and reference geometry",
        source: SampleSource::ConstructionReference,
    },
    sample(
        SampleId::CurveGallery,
        "Curve family gallery",
        AlphaScenarioKind::ProfileAllFamilies,
    ),
    sample(
        SampleId::PeriodicNurbs,
        "Periodic NURBS specimen",
        AlphaScenarioKind::NurbsPeriodic,
    ),
    SampleDefinition {
        id: SampleId::FilletWorkshop,
        title: "2D fillet workshop",
        source: SampleSource::FilletWorkshop,
    },
    SampleDefinition {
        id: SampleId::LineOffsetWorkshop,
        title: "Associative line offsets",
        source: SampleSource::LineOffsetWorkshop,
    },
    SampleDefinition {
        id: SampleId::MirrorWorkshop,
        title: "Mirror construction workshop",
        source: SampleSource::MirrorWorkshop,
    },
];

pub(crate) const GROUPS: [SampleGroup; 3] = [
    SampleGroup {
        title: "Mechanisms",
        samples: &MECHANISMS,
    },
    SampleGroup {
        title: "Constraints & dimensions",
        samples: &CONSTRAINTS,
    },
    SampleGroup {
        title: "Curves & constructions",
        samples: &CURVES,
    },
];

const fn sample(id: SampleId, title: &'static str, kind: AlphaScenarioKind) -> SampleDefinition {
    SampleDefinition {
        id,
        title,
        source: SampleSource::Alpha(kind),
    }
}

#[derive(Default)]
pub(crate) struct SampleCatalogState {
    selected: Option<SampleId>,
}

impl SampleCatalogState {
    pub(crate) const fn selected_key(&self) -> Option<&'static str> {
        match self.selected {
            Some(id) => Some(id.key()),
            None => None,
        }
    }

    pub(crate) fn selected_title(&self) -> Option<&'static str> {
        let selected = self.selected?;
        definition(selected).map(|definition| definition.title)
    }

    pub(crate) fn open_key(&mut self, key: &str) -> Result<RetainedEditorCoordinator, String> {
        let id = SampleId::from_key(key).ok_or_else(|| format!("unknown sample `{key}`"))?;
        let definition = definition(id).ok_or_else(|| format!("sample `{key}` is unavailable"))?;
        let coordinator = coordinator_from_source(definition.source)?;
        self.selected = Some(id);
        Ok(coordinator)
    }

    pub(crate) fn menu_markup(&self) -> String {
        let mut markup = String::new();
        for group in GROUPS {
            let _ = write!(
                markup,
                "<li class=\"wb-sample-branch\"><button type=\"button\" \
                 data-sample-group-trigger aria-haspopup=\"menu\" aria-expanded=\"false\">{}\
                 <span aria-hidden=\"true\">›</span></button><ul class=\"wb-sample-flyout\">",
                group.title
            );
            for definition in group.samples {
                let selected = if self.selected == Some(definition.id) {
                    " aria-current=\"true\""
                } else {
                    ""
                };
                let _ = write!(
                    markup,
                    "<li><button type=\"button\" data-sample-id=\"{}\"{selected}>{}</button></li>",
                    definition.id.key(),
                    definition.title
                );
            }
            markup.push_str("</ul></li>");
        }
        markup
    }
}

fn definition(id: SampleId) -> Option<SampleDefinition> {
    GROUPS
        .iter()
        .flat_map(|group| group.samples.iter())
        .copied()
        .find(|definition| definition.id == id)
}

fn coordinator_from_source(source: SampleSource) -> Result<RetainedEditorCoordinator, String> {
    let (document, request) = match source {
        SampleSource::Alpha(kind) => {
            let fixture = alpha_scenario(kind, 1.0).map_err(|error| error.to_string())?;
            (fixture.document, fixture.request)
        }
        SampleSource::ConstructionReference => construction_reference_document()?,
        SampleSource::TangentRadialNormal => tangent_radial_normal_document()?,
        SampleSource::FilletWorkshop => fillet_workshop_document()?,
        SampleSource::LineOffsetWorkshop => line_offset_workshop_document()?,
        SampleSource::MirrorWorkshop => mirror_workshop_document()?,
    };
    let session = RetainedSketchDocumentSession::new(document, request, SolverConfig::default())
        .map_err(|error| error.to_string())?;
    RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())
}

fn construction_reference_document()
-> Result<(SketchDocument, geosolve_sketch::DocumentSolveRequest), String> {
    let mut fixture =
        alpha_scenario(AlphaScenarioKind::A1, 1.0).map_err(|error| error.to_string())?;
    let AlphaScenarioIds::A1(ids) = fixture.ids else {
        return Err("A1 sample returned incompatible persistent roles".into());
    };
    fixture
        .document
        .set_geometry_role(ids.rectangle.curves[2], GeometryRole::Construction)
        .map_err(|error| error.to_string())?;
    Ok((fixture.document, fixture.request))
}

fn fillet_workshop_document()
-> Result<(SketchDocument, geosolve_sketch::DocumentSolveRequest), String> {
    let mut document = workshop_document(0x6600_0000_0000_0000_0000_0000_0000_0001_u128)?;
    let line_line_horizontal = add_line(
        &mut document,
        "Line-line horizontal support",
        ("Line-line horizontal start", [-9.0, 5.0]),
        ("Line-line horizontal end", [-1.0, 5.0]),
    )?;
    let line_line_vertical = add_line(
        &mut document,
        "Line-line vertical support",
        ("Line-line vertical start", [-3.0, 1.0]),
        ("Line-line vertical end", [-3.0, 9.0]),
    )?;
    fix_curve_points(&mut document, line_line_horizontal, "Line-line horizontal")?;
    fix_curve_points(&mut document, line_line_vertical, "Line-line vertical")?;
    let circle_center = document
        .add_point("Line-circle center", [6.0, 4.0])
        .map_err(|error| error.to_string())?;
    let circle_radius = document
        .add_scalar(
            "Line-circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    let circle = document
        .add_curve(
            "Line-circle circular support",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .map_err(|error| error.to_string())?;
    fix_point_at_current(&mut document, circle_center, "Line-circle center")?;
    let circle_radius_target = document
        .add_scalar(
            "Line-circle source radius target",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    document
        .add_dimension(
            "Line-circle source radius",
            DocumentDimensionDefinition::Radius {
                curve: circle,
                target: circle_radius_target,
            },
            DocumentDimensionMode::Driving,
        )
        .map_err(|error| error.to_string())?;
    let line_circle_line = add_line(
        &mut document,
        "Line-circle linear support",
        ("Line-circle line start", [2.0, 1.0]),
        ("Line-circle line end", [10.0, 1.0]),
    )?;
    fix_curve_points(&mut document, line_circle_line, "Line-circle line")?;
    let bezier_controls = [
        ("Line-Bezier start", [1.0, -3.0]),
        ("Line-Bezier control", [4.0, -7.0]),
        ("Line-Bezier end", [8.0, -3.0]),
    ]
    .map(|(label, position)| document.add_point(label, position))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())?
    .try_into()
    .map_err(|_| "line-Bezier workshop requires three controls".to_owned())?;
    let bezier = document
        .add_curve(
            "Line-Bezier curved support",
            CurveDefinition::QuadraticBezier {
                controls: bezier_controls,
            },
        )
        .map_err(|error| error.to_string())?;
    fix_curve_points(&mut document, bezier, "Line-Bezier curve")?;
    let line_bezier_line = add_line(
        &mut document,
        "Line-Bezier linear support",
        ("Line-Bezier line start", [6.0, -8.0]),
        ("Line-Bezier line end", [6.0, 0.0]),
    )?;
    fix_curve_points(&mut document, line_bezier_line, "Line-Bezier line")?;
    Ok((document, geosolve_sketch::DocumentSolveRequest::default()))
}

fn line_offset_workshop_document()
-> Result<(SketchDocument, geosolve_sketch::DocumentSolveRequest), String> {
    let mut document = workshop_document(0x6600_0000_0000_0000_0000_0000_0000_0002_u128)?;
    add_line(
        &mut document,
        "Horizontal offset source",
        ("Horizontal source start", [-8.0, -4.0]),
        ("Horizontal source end", [-1.0, -4.0]),
    )?;
    add_line(
        &mut document,
        "Angled offset source",
        ("Angled source start", [1.0, -4.0]),
        ("Angled source end", [7.0, -1.0]),
    )?;
    add_polyline(
        &mut document,
        "Polyline offset source",
        &[
            ("Polyline source start", [-8.0, 2.0]),
            ("Polyline source corner", [-4.0, 6.0]),
            ("Polyline source end", [1.0, 2.0]),
        ],
    )?;
    Ok((document, geosolve_sketch::DocumentSolveRequest::default()))
}

fn mirror_workshop_document()
-> Result<(SketchDocument, geosolve_sketch::DocumentSolveRequest), String> {
    let mut document = workshop_document(0x6600_0000_0000_0000_0000_0000_0000_0003_u128)?;
    let axis = add_line(
        &mut document,
        "Mirror axis",
        ("Mirror axis start", [0.0, -7.0]),
        ("Mirror axis end", [0.0, 7.0]),
    )?;
    document
        .set_geometry_role(axis, GeometryRole::Construction)
        .map_err(|error| error.to_string())?;
    let (axis_start, axis_end) = match document.curve(axis).map(|curve| &curve.definition) {
        Some(CurveDefinition::Line { start, end, .. }) => (*start, *end),
        _ => return Err("mirror workshop axis must remain a line".into()),
    };
    for (label, point, target) in [
        ("Fix mirror axis start", axis_start, [0.0, -7.0]),
        ("Fix mirror axis end", axis_end, [0.0, 7.0]),
    ] {
        document
            .add_constraint(
                label,
                DocumentConstraintDefinition::FixedPoint { point, target },
            )
            .map_err(|error| error.to_string())?;
    }
    add_line(
        &mut document,
        "Line mirror source",
        ("Line source start", [-8.0, -5.0]),
        ("Line source end", [-3.0, -3.0]),
    )?;
    let controls = [
        ("Bezier mirror start", [-8.0, -1.0]),
        ("Bezier mirror first control", [-7.0, 2.0]),
        ("Bezier mirror second control", [-4.0, -2.0]),
        ("Bezier mirror end", [-2.0, 1.0]),
    ]
    .map(|(label, position)| document.add_point(label, position))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())?
    .try_into()
    .map_err(|_| "cubic mirror source requires four controls".to_owned())?;
    document
        .add_curve(
            "Bezier mirror source",
            CurveDefinition::CubicBezier { controls },
        )
        .map_err(|error| error.to_string())?;
    let spline_controls = [
        ("B-spline mirror start", [-8.0, 4.0]),
        ("B-spline mirror first control", [-7.0, 7.0]),
        ("B-spline mirror second control", [-4.0, 7.0]),
        ("B-spline mirror end", [-2.0, 4.0]),
    ]
    .map(|(label, position)| document.add_point(label, position))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())?;
    document
        .add_curve(
            "B-spline mirror source",
            CurveDefinition::BSpline {
                form: geosolve_sketch::DocumentBSplineForm::Clamped,
                degree: 2,
                controls: spline_controls,
                knots: vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
                span_ids: vec![0, 1],
                next_span_id: 2,
            },
        )
        .map_err(|error| error.to_string())?;
    Ok((document, geosolve_sketch::DocumentSolveRequest::default()))
}

fn workshop_document(id: u128) -> Result<SketchDocument, String> {
    SketchDocument::with_id(8.0, DocumentId(PersistentId::from_u128(id)))
        .map_err(|error| error.to_string())
}

fn tangent_radial_normal_document()
-> Result<(SketchDocument, geosolve_sketch::DocumentSolveRequest), String> {
    let namespace = 0x6400_0000_0000_0000_0000_0000_0000_0002_u128;
    let mut document = SketchDocument::with_id(8.0, DocumentId(PersistentId::from_u128(namespace)))
        .map_err(|error| error.to_string())?;
    let center = document
        .add_point("Circle center", [0.0, 0.0])
        .map_err(|error| error.to_string())?;
    document
        .add_constraint(
            "Fixed circle center",
            DocumentConstraintDefinition::FixedPoint {
                point: center,
                target: [0.0, 0.0],
            },
        )
        .map_err(|error| error.to_string())?;
    let radius = document
        .add_scalar(
            "Circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    let circle = document
        .add_curve(
            "Reference circle",
            CurveDefinition::Circle { center, radius },
        )
        .map_err(|error| error.to_string())?;
    let radius_target = document
        .add_scalar(
            "Radius dimension target",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    document
        .add_dimension(
            "Circle radius 2",
            DocumentDimensionDefinition::Radius {
                curve: circle,
                target: radius_target,
            },
            DocumentDimensionMode::Driving,
        )
        .map_err(|error| error.to_string())?;
    add_tangent_line(&mut document, circle)?;
    add_radial_normal(&mut document, center)?;
    Ok((document, geosolve_sketch::DocumentSolveRequest::default()))
}

fn add_tangent_line(document: &mut SketchDocument, circle: CurveId) -> Result<(), String> {
    let tangent = add_line(
        document,
        "True tangent line",
        ("Tangent start", [-4.0, 2.0]),
        ("Tangent end", [4.0, 2.0]),
    )?;
    let line_contact = document
        .add_curve_contact(
            "Tangent line contact",
            CurveSpan::line(tangent),
            0.5,
            0,
            ContactNeighborhood::Interior,
            Some(TangentOrientation::Opposed),
        )
        .map_err(|error| error.to_string())?;
    let circle_contact = document
        .add_curve_contact(
            "Tangent circle contact",
            CurveSpan::line(circle),
            std::f64::consts::FRAC_PI_2,
            0,
            ContactNeighborhood::Interior,
            Some(TangentOrientation::Opposed),
        )
        .map_err(|error| error.to_string())?;
    document
        .add_constraint(
            "Line tangent to circle",
            DocumentConstraintDefinition::CurveCurveTangency {
                first_contact: line_contact,
                second_contact: circle_contact,
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn add_radial_normal(document: &mut SketchDocument, center: DesignPointId) -> Result<(), String> {
    let normal = add_line(
        document,
        "Radial normal line",
        ("Normal start", [-4.0, 0.0]),
        ("Normal end", [4.0, 0.0]),
    )?;
    let contact = document
        .add_curve_contact(
            "Radial normal contact",
            CurveSpan::line(normal),
            0.5,
            0,
            ContactNeighborhood::Interior,
            None,
        )
        .map_err(|error| error.to_string())?;
    document
        .add_constraint(
            "Circle center on normal line",
            DocumentConstraintDefinition::PointOnCurve {
                point: center,
                contact,
            },
        )
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn add_line(
    document: &mut SketchDocument,
    label: &str,
    start: (&str, [f64; 2]),
    end: (&str, [f64; 2]),
) -> Result<CurveId, String> {
    let delta = [end.1[0] - start.1[0], end.1[1] - start.1[1]];
    let norm = delta[0].hypot(delta[1]);
    let branch_direction = [delta[0] / norm, delta[1] / norm];
    let start = document
        .add_point(start.0, start.1)
        .map_err(|error| error.to_string())?;
    let end = document
        .add_point(end.0, end.1)
        .map_err(|error| error.to_string())?;
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction,
            },
        )
        .map_err(|error| error.to_string())
}

fn fix_curve_points(
    document: &mut SketchDocument,
    curve: CurveId,
    label: &str,
) -> Result<(), String> {
    let points = match document.curve(curve).map(|curve| &curve.definition) {
        Some(CurveDefinition::Line { start, end, .. }) => vec![*start, *end],
        Some(CurveDefinition::QuadraticBezier { controls }) => controls.to_vec(),
        _ => return Err(format!("{label} does not expose fixable workshop controls")),
    };
    for (index, point) in points.into_iter().enumerate() {
        fix_point_at_current(document, point, &format!("{label} control {}", index + 1))?;
    }
    Ok(())
}

fn fix_point_at_current(
    document: &mut SketchDocument,
    point: DesignPointId,
    label: &str,
) -> Result<(), String> {
    let target = document
        .point(point)
        .ok_or_else(|| format!("{label} point is missing"))?
        .position;
    document
        .add_constraint(
            format!("Fix {label}"),
            DocumentConstraintDefinition::FixedPoint { point, target },
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn add_polyline(
    document: &mut SketchDocument,
    label: &str,
    points: &[(&str, [f64; 2])],
) -> Result<CurveId, String> {
    let point_ids = points
        .iter()
        .map(|(point_label, position)| document.add_point(*point_label, *position))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let branch_directions = points
        .windows(2)
        .map(|pair| {
            let delta = [pair[1].1[0] - pair[0].1[0], pair[1].1[1] - pair[0].1[1]];
            let norm = delta[0].hypot(delta[1]);
            [delta[0] / norm, delta[1] / norm]
        })
        .collect();
    document
        .add_curve(
            label,
            CurveDefinition::Polyline {
                points: point_ids,
                closed: false,
                branch_directions,
            },
        )
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use geosolve_constraint_editor::{
        OperationAuthoringOptions, OperationAuthoringOutcome, OperationAuthoringPreviewOutcome,
        OperationAuthoringState, OperationAuthoringTool, RetainedEditorCoordinator, SelectionItem,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        CurveDefinition, CurveSpan, DocumentConstraintDefinition, DocumentDimensionDefinition,
        DocumentEdit, GeometryRole, RetainedSketchDocumentSession,
    };

    use super::super::persistence::WorkspaceSnapshot;
    use super::{GROUPS, SampleCatalogState, SampleId};

    #[test]
    fn catalog_is_flat_by_purpose_with_unique_stable_keys() {
        assert_eq!(GROUPS.len(), 3);
        assert_eq!(
            GROUPS.map(|group| group.title),
            [
                "Mechanisms",
                "Constraints & dimensions",
                "Curves & constructions"
            ]
        );
        let leaves = GROUPS
            .iter()
            .flat_map(|group| group.samples.iter())
            .collect::<Vec<_>>();
        assert_eq!(leaves.len(), SampleId::ALL.len());
        assert_eq!(
            leaves
                .iter()
                .map(|definition| definition.id.key())
                .collect::<HashSet<_>>()
                .len(),
            leaves.len()
        );
        assert!(leaves.iter().all(|definition| {
            !definition.title.contains('M') || !definition.title.contains("M6")
        }));
    }

    #[test]
    fn every_sample_builds_an_accepted_ordinary_coordinator() {
        let mut catalog = SampleCatalogState::default();
        for id in SampleId::ALL {
            let coordinator = catalog.open_key(id.key()).unwrap_or_else(|error| {
                panic!("sample {} failed to build: {error}", id.key());
            });
            assert!(
                coordinator.session().accepted_state().is_some(),
                "{} has no accepted state",
                id.key()
            );
            assert_eq!(coordinator.history_len(), 1, "{}", id.key());
            assert!(!coordinator.can_undo(), "{}", id.key());

            let snapshot = WorkspaceSnapshot::from_checkpoint(coordinator.checkpoint());
            let decoded =
                WorkspaceSnapshot::decode(&snapshot.encode().expect("encode")).expect("decode");
            let design = decoded.design_document().expect("design document");
            let accepted = decoded
                .accepted_document()
                .expect("accepted payload")
                .expect("accepted document");
            let restored = RetainedSketchDocumentSession::restore_design_with_accepted(
                design,
                accepted,
                decoded.revisions(),
                geosolve_sketch::DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .unwrap_or_else(|error| {
                panic!("sample {} failed to restore: {error}", id.key());
            });
            let restored = RetainedEditorCoordinator::new(restored).expect("restored coordinator");
            assert_eq!(restored.history_len(), 1, "{}", id.key());
            assert!(
                restored.session().accepted_state().is_some(),
                "{}",
                id.key()
            );
        }
    }

    #[test]
    fn m66_workshops_are_plain_editable_documents_with_expected_sources() {
        let mut catalog = SampleCatalogState::default();
        let fillet = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("fillet workshop");
        assert_eq!(fillet.session().design_document().curves().len(), 6);
        assert!(
            fillet
                .session()
                .design_document()
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::Circle { .. }))
        );
        assert!(
            fillet
                .session()
                .design_document()
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::QuadraticBezier { .. }))
        );

        let offsets = catalog
            .open_key(SampleId::LineOffsetWorkshop.key())
            .expect("line-offset workshop");
        assert_eq!(offsets.session().design_document().curves().len(), 3);
        assert!(
            offsets
                .session()
                .design_document()
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::Polyline { .. }))
        );

        let mirror = catalog
            .open_key(SampleId::MirrorWorkshop.key())
            .expect("mirror workshop");
        assert_eq!(mirror.session().design_document().curves().len(), 4);
        let axis = mirror
            .session()
            .design_document()
            .curves()
            .iter()
            .find(|curve| curve.label == "Mirror axis")
            .expect("mirror axis");
        assert_eq!(
            mirror.session().design_document().geometry_role(axis.id),
            Some(GeometryRole::Construction)
        );
        assert!(
            mirror
                .session()
                .design_document()
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::CubicBezier { .. }))
        );
        assert!(
            mirror
                .session()
                .design_document()
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::BSpline { .. }))
        );

        let fillet_document = fillet.session().design_document();
        assert_eq!(fillet_document.constraints().len(), 12);
        assert!(
            fillet_document
                .constraints()
                .iter()
                .all(|constraint| matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::FixedPoint { .. }
                ))
        );
        assert_eq!(fillet_document.dimensions().len(), 1);
        assert!(matches!(
            fillet_document.dimensions()[0].definition,
            DocumentDimensionDefinition::Radius { .. }
        ));
        assert!(fillet.session().accepted_state().is_some());

        assert!(offsets.session().design_document().constraints().is_empty());
        assert!(offsets.session().design_document().dimensions().is_empty());
        assert!(offsets.session().accepted_state().is_some());
        assert_eq!(mirror.session().design_document().constraints().len(), 2);
        assert!(mirror.session().design_document().constraints().iter().all(
            |constraint| matches!(
                constraint.definition,
                DocumentConstraintDefinition::FixedPoint { .. }
            )
        ));
        assert!(mirror.session().design_document().dimensions().is_empty());
        assert!(mirror.session().accepted_state().is_some());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "the actual fillet sample is qualified through one complete persisted lifecycle"
    )]
    fn fillet_workshop_commit_round_trip_radius_and_parent_edits_remain_accepted() {
        let mut catalog = SampleCatalogState::default();
        let mut coordinator = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("fillet workshop");
        let (line, circle) = {
            let document = coordinator.session().design_document();
            let by_label = |label: &str| {
                document
                    .curves()
                    .iter()
                    .find(|curve| curve.label == label)
                    .expect("workshop curve")
                    .id
            };
            (
                by_label("Line-circle linear support"),
                by_label("Line-circle circular support"),
            )
        };
        let operation_document = coordinator
            .operation_authoring_document()
            .expect("accepted operation document")
            .clone();
        let picks = [
            coordinator
                .operation_pick_for_item(SelectionItem::Curve(CurveSpan::line(line)), Some(0.28))
                .expect("line pick"),
            coordinator
                .operation_pick_for_item(SelectionItem::Curve(CurveSpan::line(circle)), Some(4.05))
                .expect("circle pick"),
        ];
        let mut authoring = OperationAuthoringState::default();
        let _ = authoring.set_options(
            &operation_document,
            OperationAuthoringOptions {
                fillet_radius: Some(0.8),
                ..OperationAuthoringOptions::default()
            },
        );
        let outcome =
            authoring.activate(&operation_document, OperationAuthoringTool::Fillet, &picks);
        let OperationAuthoringOutcome::PreviewRequested { candidate, .. } = outcome else {
            panic!("workshop fillet candidate expected: {outcome:?}");
        };
        let preview = coordinator
            .prepare_operation_preview(&candidate)
            .expect("workshop fillet preview");
        let OperationAuthoringPreviewOutcome::Ready(metadata) = preview else {
            panic!("accepted workshop fillet preview expected: {preview:?}");
        };
        let fillet = metadata.primary_created_curve;
        coordinator
            .apply_operation_preview(metadata.token, &candidate)
            .expect("workshop fillet commit");

        let snapshot = WorkspaceSnapshot::from_checkpoint(coordinator.checkpoint());
        let decoded = WorkspaceSnapshot::decode(&snapshot.encode().expect("encode operation"))
            .expect("decode operation");
        let restored = RetainedSketchDocumentSession::restore_design_with_accepted(
            decoded.design_document().expect("design document"),
            decoded
                .accepted_document()
                .expect("accepted payload")
                .expect("accepted document"),
            decoded.revisions(),
            geosolve_sketch::DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("restore committed fillet");
        let mut restored = RetainedEditorCoordinator::new(restored).expect("restored coordinator");
        let (fillet_center, fillet_radius, fillet_target, source_radius_target) = {
            let document = restored.session().design_document();
            let (center, radius) = match &document.curve(fillet).expect("fillet curve").definition {
                CurveDefinition::CircularArc { center, radius, .. } => (*center, *radius),
                other => panic!("fillet output must be a circular arc: {other:?}"),
            };
            let fillet_target = document
                .dimensions()
                .iter()
                .find_map(|dimension| match &dimension.definition {
                    DocumentDimensionDefinition::Radius { curve, target } if *curve == fillet => {
                        Some(*target)
                    }
                    _ => None,
                })
                .expect("fillet radius target");
            let source_radius_target = document
                .dimensions()
                .iter()
                .find_map(|dimension| match &dimension.definition {
                    DocumentDimensionDefinition::Radius { target, .. }
                        if dimension.label == "Line-circle source radius" =>
                    {
                        Some(*target)
                    }
                    _ => None,
                })
                .expect("source circle radius target");
            (center, radius, fillet_target, source_radius_target)
        };
        let radius_edit = restored
            .apply_edit(
                restored.session().design_identity(),
                DocumentEdit::SetScalarValue {
                    scalar: fillet_target,
                    value: 0.96,
                },
            )
            .expect("edit restored fillet radius");
        assert!(radius_edit.published_accepted.is_some());
        let accepted = restored
            .session()
            .accepted_state()
            .expect("accepted fillet radius edit")
            .document();
        assert!(
            (accepted.scalar(fillet_radius).expect("fillet radius").value - 0.96).abs() <= 1.0e-7
        );
        let center_before = accepted
            .point(fillet_center)
            .expect("fillet center")
            .position;

        let parent_edit = restored
            .apply_edit(
                restored.session().design_identity(),
                DocumentEdit::SetScalarValue {
                    scalar: source_radius_target,
                    value: 2.1,
                },
            )
            .expect("edit restored source radius");
        assert!(parent_edit.published_accepted.is_some());
        let center_after = restored
            .session()
            .accepted_state()
            .expect("accepted source-radius edit")
            .document()
            .point(fillet_center)
            .expect("associated fillet center")
            .position;
        assert!(
            (center_after[0] - center_before[0]).hypot(center_after[1] - center_before[1]) > 1.0e-6
        );
    }

    #[test]
    fn opened_sample_is_editable_and_delete_undo_restores_a_fixed_constraint() {
        let mut catalog = SampleCatalogState::default();
        let mut coordinator = catalog
            .open_key(SampleId::DraftingCompass.key())
            .expect("drafting compass");
        let fixed = coordinator
            .session()
            .design_document()
            .constraints()
            .iter()
            .find(|constraint| {
                matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::FixedPoint { .. }
                )
            })
            .expect("fixed constraint")
            .id;
        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Constraint(fixed)]);
        coordinator
            .delete_selected(coordinator.session().design_identity())
            .expect("delete fixed constraint");
        assert!(
            coordinator
                .session()
                .design_document()
                .constraint(fixed)
                .is_none()
        );
        assert!(coordinator.can_undo());
        coordinator.undo().expect("undo delete");
        assert!(
            coordinator
                .session()
                .design_document()
                .constraint(fixed)
                .is_some()
        );
    }

    #[test]
    fn menu_contains_only_sample_navigation_without_guided_controls() {
        let markup = SampleCatalogState::default().menu_markup();
        assert_eq!(
            markup.matches("data-sample-id=").count(),
            SampleId::ALL.len()
        );
        for retired in [
            "scenario",
            "guide",
            "verification",
            "transcript",
            "evidence",
            "data-sample-action",
            "data-sample-control",
        ] {
            assert!(
                !markup.contains(retired),
                "{retired} leaked into sample menu"
            );
        }
    }

    #[test]
    fn browser_shell_contains_no_retired_guided_harness_surface() {
        let html = include_str!("../../index.html");
        let css = include_str!("../../styles.css");
        for retired in [
            "wb-scenario",
            "data-scenario",
            "scenario-guide",
            "scenario-transcript",
            "scenario-evidence",
            "review scenario",
        ] {
            assert!(!html.contains(retired), "{retired} remains in index.html");
            assert!(!css.contains(retired), "{retired} remains in styles.css");
        }
    }
}
