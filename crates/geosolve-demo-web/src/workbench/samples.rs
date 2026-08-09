// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::fmt::Write as _;

use geosolve_constraint_editor::{
    ConstructionPoint, ConstructionProposal, NurbsConstructionOptions, RetainedEditorCoordinator,
};
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
    AutoConstraintDrafting,
    TangentRadialNormal,
    ContactBranch,
    AngleDimensions,
    ContextualAnnotations,
    DenseJunction,
    ConstructionReference,
    CurveGallery,
    PeriodicNurbs,
    FilletWorkshop,
}

impl SampleId {
    pub(crate) const ALL: [Self; 24] = [
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
        Self::AutoConstraintDrafting,
        Self::TangentRadialNormal,
        Self::ContactBranch,
        Self::AngleDimensions,
        Self::ContextualAnnotations,
        Self::DenseJunction,
        Self::ConstructionReference,
        Self::CurveGallery,
        Self::PeriodicNurbs,
        Self::FilletWorkshop,
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
            Self::AutoConstraintDrafting => "auto-constraint-drafting",
            Self::TangentRadialNormal => "tangent-radial-normal",
            Self::ContactBranch => "contact-branch-specimen",
            Self::AngleDimensions => "angle-dimension-annotations",
            Self::ContextualAnnotations => "contextual-constraint-annotations",
            Self::DenseJunction => "dense-constraint-junction",
            Self::ConstructionReference => "construction-reference-geometry",
            Self::CurveGallery => "curve-family-gallery",
            Self::PeriodicNurbs => "periodic-nurbs-specimen",
            Self::FilletWorkshop => "fillet-workshop",
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
    AutoConstraintDrafting,
    TangentRadialNormal,
    FilletWorkshop,
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

const CONSTRAINTS: [SampleDefinition; 7] = [
    sample(
        SampleId::ConstraintSampler,
        "Constraint and dimension sampler",
        AlphaScenarioKind::Corpus,
    ),
    SampleDefinition {
        id: SampleId::AutoConstraintDrafting,
        title: "Auto-constraint drafting playground",
        source: SampleSource::AutoConstraintDrafting,
    },
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

const CURVES: [SampleDefinition; 4] = [
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
        title: "2D Fillet playground",
        source: SampleSource::FilletWorkshop,
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
        SampleSource::AutoConstraintDrafting => auto_constraint_drafting_document()?,
        SampleSource::TangentRadialNormal => tangent_radial_normal_document()?,
        SampleSource::FilletWorkshop => fillet_workshop_document()?,
    };
    let session = RetainedSketchDocumentSession::new(document, request, SolverConfig::default())
        .map_err(|error| error.to_string())?;
    RetainedEditorCoordinator::new(session).map_err(|error| error.to_string())
}

#[allow(
    clippy::too_many_lines,
    reason = "one spatially separated ordinary-document drafting playground fixture"
)]
fn auto_constraint_drafting_document()
-> Result<(SketchDocument, geosolve_sketch::DocumentSolveRequest), String> {
    let mut document = workshop_document(0x7000_0000_0000_0000_0000_0000_0000_0001_u128)?;

    let midpoint_line = add_line(
        &mut document,
        "Midpoint and affine reference line",
        ("Midpoint reference start", [-11.0, 5.0]),
        ("Midpoint reference end", [-3.0, 5.0]),
    )?;
    fix_curve_points(&mut document, midpoint_line, "midpoint reference")?;

    let affine_polyline = add_polyline(
        &mut document,
        "Affine reference polyline",
        &[
            ("Affine polyline start", [-11.0, 1.0]),
            ("Affine polyline corner", [-7.0, 3.0]),
            ("Affine polyline end", [-3.0, 1.0]),
        ],
    )?;
    fix_curve_points(&mut document, affine_polyline, "affine reference polyline")?;

    add_point_specimen_marker(
        &mut document,
        "Profile point specimen marker",
        "Profile point specimen",
        [-11.0, -9.0],
        0.75,
        GeometryRole::Profile,
        false,
    )?;
    add_point_specimen_marker(
        &mut document,
        "Construction point specimen marker",
        "Construction point specimen",
        [-7.0, -9.0],
        0.75,
        GeometryRole::Construction,
        true,
    )?;

    // Drawing a line between these centers reuses both point identities.
    // A construction reference over the same point pair already owns the
    // Horizontal relation, so the inferred duplicate is deterministically
    // rejected while the line draft remains correction-ready.
    let mut rejection_points = Vec::new();
    for (suffix, position) in [("start", [2.0, -9.0]), ("end", [10.0, -9.0])] {
        rejection_points.push(add_point_specimen_marker(
            &mut document,
            &format!("Redundant inference rejection {suffix} marker"),
            &format!("Redundant inference rejection {suffix} point"),
            position,
            0.65,
            GeometryRole::Construction,
            false,
        )?);
    }
    let rejection_reference = document
        .add_curve(
            "Redundant inference rejection reference",
            CurveDefinition::Line {
                start: rejection_points[0],
                end: rejection_points[1],
                branch_direction: [1.0, 0.0],
            },
        )
        .map_err(|error| error.to_string())?;
    document
        .set_geometry_role(rejection_reference, GeometryRole::Construction)
        .map_err(|error| error.to_string())?;
    document
        .add_constraint(
            "Prepared rejection reference is horizontal",
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(rejection_reference),
            },
        )
        .map_err(|error| error.to_string())?;

    let circle_center = document
        .add_point("Curve target circle center", [2.0, 5.0])
        .map_err(|error| error.to_string())?;
    fix_point_at_current(&mut document, circle_center, "curve target circle center")?;
    let circle_radius = document
        .add_scalar(
            "Curve target circle radius",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    let circle = document
        .add_curve(
            "Curve target circle",
            CurveDefinition::Circle {
                center: circle_center,
                radius: circle_radius,
            },
        )
        .map_err(|error| error.to_string())?;
    let circle_radius_target = document
        .add_scalar(
            "Curve target circle radius target",
            2.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    document
        .add_dimension(
            "Curve target circle radius dimension",
            DocumentDimensionDefinition::Radius {
                curve: circle,
                target: circle_radius_target,
            },
            DocumentDimensionMode::Driving,
        )
        .map_err(|error| error.to_string())?;

    let bezier_controls = [
        ("Curve target Bezier start", [6.0, 3.0]),
        ("Curve target Bezier control 1", [8.0, 7.0]),
        ("Curve target Bezier control 2", [10.0, 2.0]),
        ("Curve target Bezier end", [12.0, 5.0]),
    ]
    .map(|(label, position)| document.add_point(label, position))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())?
    .try_into()
    .map_err(|_| "auto-constraint Bezier requires four controls".to_owned())?;
    let bezier = document
        .add_curve(
            "Curve target cubic Bezier",
            CurveDefinition::CubicBezier {
                controls: bezier_controls,
            },
        )
        .map_err(|error| error.to_string())?;
    fix_curve_points(&mut document, bezier, "curve target cubic Bezier")?;

    let nurbs = ConstructionProposal::Nurbs {
        controls: vec![
            ConstructionPoint::New([5.0, -1.0]),
            ConstructionPoint::New([7.0, -4.0]),
            ConstructionPoint::New([10.0, 0.0]),
            ConstructionPoint::New([12.0, -3.0]),
        ],
        options: NurbsConstructionOptions::default(),
    }
    .apply_with_role(&mut document, GeometryRole::Profile)
    .map_err(|error| error.to_string())?
    .curves
    .into_iter()
    .next()
    .ok_or_else(|| "auto-constraint NURBS construction created no curve".to_owned())?;
    fix_curve_points(&mut document, nurbs, "curve target NURBS")?;

    let overlap_profile = add_line(
        &mut document,
        "Profile overlap priority reference",
        ("Profile overlap start", [-11.0, -3.0]),
        ("Profile overlap end", [-3.0, -3.0]),
    )?;
    fix_curve_points(&mut document, overlap_profile, "profile overlap reference")?;
    let overlap_construction = add_line(
        &mut document,
        "Construction overlap priority reference",
        ("Construction overlap start", [-11.0, -3.0]),
        ("Construction overlap end", [-3.0, -3.0]),
    )?;
    fix_curve_points(
        &mut document,
        overlap_construction,
        "construction overlap reference",
    )?;
    document
        .set_geometry_role(overlap_construction, GeometryRole::Construction)
        .map_err(|error| error.to_string())?;

    for suffix in ["A", "B"] {
        let ambiguous = add_line(
            &mut document,
            &format!("Exact ambiguity reference {suffix}"),
            (&format!("Exact ambiguity {suffix} start"), [2.0, -6.0]),
            (&format!("Exact ambiguity {suffix} end"), [10.0, -6.0]),
        )?;
        fix_curve_points(
            &mut document,
            ambiguous,
            &format!("exact ambiguity reference {suffix}"),
        )?;
    }

    Ok((document, geosolve_sketch::DocumentSolveRequest::default()))
}

fn construction_reference_document()
-> Result<(SketchDocument, geosolve_sketch::DocumentSolveRequest), String> {
    let mut fixture =
        alpha_scenario(AlphaScenarioKind::A1, 1.0).map_err(|error| error.to_string())?;
    let AlphaScenarioIds::A1(ids) = fixture.ids else {
        return Err("A1 sample returned incompatible persistent roles".into());
    };
    let shared_diagonal = fixture
        .document
        .add_curve(
            "Shared-corner construction diagonal",
            CurveDefinition::Line {
                start: ids.rectangle.points[0],
                end: ids.rectangle.points[2],
                branch_direction: [0.8, 0.6],
            },
        )
        .map_err(|error| error.to_string())?;
    fixture
        .document
        .set_geometry_role(shared_diagonal, GeometryRole::Construction)
        .map_err(|error| error.to_string())?;
    let overlapping_guide = add_line(
        &mut fixture.document,
        "Construction guide overlapping the profile base",
        ("Overlapping guide start", [0.0, 0.0]),
        ("Overlapping guide end", [4.0, 0.0]),
    )?;
    fix_curve_points(
        &mut fixture.document,
        overlapping_guide,
        "Overlapping construction guide",
    )?;
    fixture
        .document
        .set_geometry_role(overlapping_guide, GeometryRole::Construction)
        .map_err(|error| error.to_string())?;
    Ok((fixture.document, fixture.request))
}

#[allow(
    clippy::too_many_lines,
    reason = "one spatially organized ordinary-document playground fixture"
)]
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
    add_fixed_high_valence_junction(&mut document)?;
    add_fixed_line_circle_island(
        &mut document,
        "Friendly line-circle",
        [6.0, 4.0],
        1.0,
        [1.0, 2.5],
        [11.0, 2.5],
    )?;
    add_fixed_line_circle_island(
        &mut document,
        "Near-fold stress line-circle",
        [22.0, 7.0],
        2.0,
        [18.0, 4.0],
        [26.0, 4.0],
    )?;
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
    add_polyline(
        &mut document,
        "Editable batch and sequential polyline",
        &[
            ("Batch polyline start", [-10.0, -2.0]),
            ("Batch polyline first corner", [-6.0, -2.0]),
            ("Batch polyline second corner", [-6.0, -7.0]),
            ("Batch polyline end", [-2.0, -7.0]),
        ],
    )?;
    add_polyline(
        &mut document,
        "Editable short-middle conflict polyline",
        &[
            ("Conflict polyline start", [11.0, -3.0]),
            ("Conflict polyline first corner", [15.0, -3.0]),
            ("Conflict polyline second corner", [15.0, -4.75]),
            ("Conflict polyline end", [19.0, -4.75]),
        ],
    )?;
    Ok((document, geosolve_sketch::DocumentSolveRequest::default()))
}

fn add_fixed_line_circle_island(
    document: &mut SketchDocument,
    label: &str,
    center_position: [f64; 2],
    radius: f64,
    line_start: [f64; 2],
    line_end: [f64; 2],
) -> Result<(), String> {
    let center = document
        .add_point(format!("{label} center"), center_position)
        .map_err(|error| error.to_string())?;
    let radius_scalar = document
        .add_scalar(
            format!("{label} radius"),
            radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    let circle = document
        .add_curve(
            format!("{label} circular support"),
            CurveDefinition::Circle {
                center,
                radius: radius_scalar,
            },
        )
        .map_err(|error| error.to_string())?;
    fix_point_at_current(document, center, &format!("{label} center"))?;
    let radius_target = document
        .add_scalar(
            format!("{label} source radius target"),
            radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    document
        .add_dimension(
            format!("{label} source radius"),
            DocumentDimensionDefinition::Radius {
                curve: circle,
                target: radius_target,
            },
            DocumentDimensionMode::Driving,
        )
        .map_err(|error| error.to_string())?;
    let line = add_line(
        document,
        &format!("{label} linear support"),
        (&format!("{label} line start"), line_start),
        (&format!("{label} line end"), line_end),
    )?;
    fix_curve_points(document, line, &format!("{label} line"))?;
    Ok(())
}

fn add_fixed_high_valence_junction(document: &mut SketchDocument) -> Result<(), String> {
    let center = document
        .add_point("High-valence shared junction", [14.0, 6.0])
        .map_err(|error| error.to_string())?;
    let endpoints = [
        ("High-valence upper endpoint", [14.0, 10.0]),
        ("High-valence lower-left endpoint", [10.5, 3.5]),
        ("High-valence lower-right endpoint", [17.5, 3.5]),
    ]
    .map(|(label, position)| document.add_point(label, position))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(|error| error.to_string())?;
    for (index, endpoint) in endpoints.iter().copied().enumerate() {
        let start = document
            .point(center)
            .ok_or_else(|| "high-valence center is missing".to_owned())?
            .position;
        let end = document
            .point(endpoint)
            .ok_or_else(|| "high-valence endpoint is missing".to_owned())?
            .position;
        let delta = [end[0] - start[0], end[1] - start[1]];
        let norm = delta[0].hypot(delta[1]);
        document
            .add_curve(
                format!("High-valence branch {}", index + 1),
                CurveDefinition::Line {
                    start: center,
                    end: endpoint,
                    branch_direction: [delta[0] / norm, delta[1] / norm],
                },
            )
            .map_err(|error| error.to_string())?;
    }
    fix_point_at_current(document, center, "high-valence shared junction")?;
    for (index, endpoint) in endpoints.into_iter().enumerate() {
        fix_point_at_current(
            document,
            endpoint,
            &format!("high-valence endpoint {}", index + 1),
        )?;
    }
    Ok(())
}

fn workshop_document(id: u128) -> Result<SketchDocument, String> {
    SketchDocument::with_id(5.0, DocumentId(PersistentId::from_u128(id)))
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

fn add_point_specimen_marker(
    document: &mut SketchDocument,
    curve_label: &str,
    point_label: &str,
    position: [f64; 2],
    radius: f64,
    role: GeometryRole,
    fixed: bool,
) -> Result<DesignPointId, String> {
    let center = document
        .add_point(point_label, position)
        .map_err(|error| error.to_string())?;
    if fixed {
        fix_point_at_current(document, center, point_label)?;
    }
    let radius_scalar = document
        .add_scalar(
            format!("{curve_label} radius"),
            radius,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .map_err(|error| error.to_string())?;
    let marker = document
        .add_curve(
            curve_label,
            CurveDefinition::Circle {
                center,
                radius: radius_scalar,
            },
        )
        .map_err(|error| error.to_string())?;
    document
        .set_geometry_role(marker, role)
        .map_err(|error| error.to_string())?;
    Ok(center)
}

fn fix_curve_points(
    document: &mut SketchDocument,
    curve: CurveId,
    label: &str,
) -> Result<(), String> {
    let points = match document.curve(curve).map(|curve| &curve.definition) {
        Some(CurveDefinition::Line { start, end, .. }) => vec![*start, *end],
        Some(CurveDefinition::Polyline { points, .. }) => points.clone(),
        Some(CurveDefinition::QuadraticBezier { controls }) => controls.to_vec(),
        Some(CurveDefinition::CubicBezier { controls }) => controls.to_vec(),
        Some(
            CurveDefinition::BSpline { controls, .. } | CurveDefinition::Nurbs { controls, .. },
        ) => controls.clone(),
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
        ComputedFilletContinuationLimitKind, CoordinatorError, EditorEffect, EditorHoverState,
        EditorScene, EditorTool, FeatureAuthoringOptions, FeatureAuthoringOutcome,
        FeatureAuthoringState, FeatureAuthoringTool, FeatureAuthoringWarningKind,
        GeometryInteractionPolicy, GeometryPickScope, InferredRelation, Modifiers, PickTolerance,
        PointerInput, RetainedEditorCoordinator, ScreenPoint, SelectionItem, Viewport,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        CurveDefinition, CurveSpan, DocumentConstraintDefinition, DocumentDimensionDefinition,
        GeometryRole, RetainedSketchDocumentSession,
    };
    use geosolve_sketch_features::ComputedFeatureFailure;

    use super::super::persistence::WorkspaceSnapshot;
    use super::{GROUPS, SampleCatalogState, SampleId};

    fn fillet_playground_scene(coordinator: &RetainedEditorCoordinator) -> EditorScene {
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("current accepted playground state");
        EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            Viewport::new([1000.0, 700.0], [4.0, 1.0], 25.0).expect("playground viewport"),
            0.25,
        )
        .expect("playground editor scene")
    }

    fn activate_fillet_playground(
        coordinator: &RetainedEditorCoordinator,
    ) -> FeatureAuthoringState {
        let snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current playground authoring snapshot");
        let document = snapshot.sketch_document();
        let mut authoring = FeatureAuthoringState::default();
        assert!(matches!(
            authoring.activate(&snapshot, document, FeatureAuthoringTool::Fillet, &[]),
            FeatureAuthoringOutcome::ModeEntered(_)
        ));
        authoring
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the helper covers one complete browser-independent authoring and affordance boundary"
    )]
    fn line_circle_affordance_scene(
        coordinator: &mut RetainedEditorCoordinator,
        prefix: &str,
    ) -> EditorScene {
        let source_scene = fillet_playground_scene(coordinator);
        let mut authoring = activate_fillet_playground(coordinator);
        let (line, circle, line_position, circle_position) = {
            let snapshot = coordinator
                .feature_authoring_snapshot()
                .expect("current line-circle source snapshot");
            let document = snapshot.sketch_document();
            let span = |suffix: &str| CurveSpan {
                curve: document
                    .curves()
                    .iter()
                    .find(|curve| curve.label == format!("{prefix} {suffix}"))
                    .unwrap_or_else(|| panic!("missing {prefix} {suffix}"))
                    .id,
                segment: 0,
            };
            let line = span("linear support");
            let circle = span("circular support");
            (
                line,
                circle,
                document
                    .evaluate_curve_jet(line, 0.4)
                    .expect("line pick position")
                    .position,
                document
                    .evaluate_curve_jet(circle, 4.5)
                    .expect("circle pick position")
                    .position,
            )
        };
        let first = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &source_scene,
                source_scene.viewport.model_to_screen(line_position.into()),
                PickTolerance::default(),
                format!("{prefix} Fillet"),
            )
            .expect("line Fillet pick");
        assert!(matches!(
            first.outcome,
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        let second = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &source_scene,
                source_scene
                    .viewport
                    .model_to_screen(circle_position.into()),
                PickTolerance::default(),
                format!("{prefix} Fillet"),
            )
            .expect("circle Fillet pick");
        assert!(matches!(
            second.outcome,
            FeatureAuthoringOutcome::PreviewRequested { .. }
        ));
        let (mut scene, feature) = {
            let preview = coordinator
                .feature_authoring_preview()
                .expect("held line-circle preview");
            let accepted = coordinator
                .session()
                .accepted_state_for_current_input()
                .expect("accepted line-circle source");
            let expected = preview.snapshot().input();
            let scene = EditorScene::from_accepted_with_computed(
                accepted.identity().revision().get(),
                coordinator.session().design_identity(),
                accepted.document(),
                coordinator.session().design_document(),
                &coordinator
                    .session()
                    .accepted_prepared_input()
                    .expect("accepted line-circle input"),
                &expected,
                preview.snapshot(),
                source_scene.viewport,
                0.25,
            )
            .expect("line-circle computed scene");
            (scene, preview.metadata().feature)
        };
        coordinator
            .populate_computed_fillet_affordances(
                &mut scene,
                &[SelectionItem::Feature(feature)],
                0.25,
            )
            .expect("line-circle affordance population");
        assert_eq!(
            scene
                .computed_curves
                .iter()
                .filter(|curve| curve.contacts.iter().any(|contact| {
                    contact.source.span == line || contact.source.span == circle
                }))
                .count(),
            1,
        );
        scene
    }

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
    #[allow(
        clippy::too_many_lines,
        reason = "one inventory test keeps every spatially separated M70 UAT specimen auditable"
    )]
    fn auto_constraint_playground_has_role_scope_and_rejection_specimens() {
        let mut catalog = SampleCatalogState::default();
        let coordinator = catalog
            .open_key(SampleId::AutoConstraintDrafting.key())
            .expect("auto-constraint drafting playground");
        let document = coordinator.session().design_document();

        for expected in [
            "Midpoint and affine reference line",
            "Affine reference polyline",
            "Profile point specimen marker",
            "Construction point specimen marker",
            "Redundant inference rejection start marker",
            "Redundant inference rejection end marker",
            "Redundant inference rejection reference",
            "Curve target circle",
            "Curve target cubic Bezier",
            "NURBS",
            "Profile overlap priority reference",
            "Construction overlap priority reference",
            "Exact ambiguity reference A",
            "Exact ambiguity reference B",
        ] {
            assert!(
                document
                    .curves()
                    .iter()
                    .any(|curve| curve.label == expected),
                "missing drafting playground curve `{expected}`"
            );
        }
        assert!(
            document
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::CubicBezier { .. }))
        );
        assert!(
            document
                .curves()
                .iter()
                .any(|curve| matches!(curve.definition, CurveDefinition::Nurbs { .. }))
        );

        let marker = |label: &str| {
            let curve = document
                .curves()
                .iter()
                .find(|curve| curve.label == label)
                .unwrap_or_else(|| panic!("missing point marker `{label}`"));
            let CurveDefinition::Circle { center, .. } = &curve.definition else {
                panic!("point marker `{label}` must remain circular")
            };
            (curve.id, *center)
        };
        let (profile_marker, profile_point) = marker("Profile point specimen marker");
        let (construction_marker, construction_point) =
            marker("Construction point specimen marker");
        assert_eq!(
            document.geometry_role(profile_marker),
            Some(GeometryRole::Profile)
        );
        assert_eq!(
            document.geometry_role(construction_marker),
            Some(GeometryRole::Construction)
        );

        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted playground scene");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            document,
            Viewport::new([1000.0, 700.0], [0.0, -2.0], 25.0).expect("playground viewport"),
            0.25,
        )
        .expect("playground scene");
        let scene_point = |point| {
            scene
                .points
                .iter()
                .find(|value| value.id == point)
                .copied()
                .expect("point specimen in accepted scene")
        };
        let profile_point = scene_point(profile_point);
        let construction_point = scene_point(construction_point);
        assert!(profile_point.role_incidence.profile);
        assert!(!profile_point.role_incidence.construction);
        assert!(!construction_point.role_incidence.profile);
        assert!(construction_point.role_incidence.construction);
        let profile_scope = GeometryInteractionPolicy {
            scope: GeometryPickScope::Profile,
            ..GeometryInteractionPolicy::default()
        };
        let construction_scope = GeometryInteractionPolicy {
            scope: GeometryPickScope::Construction,
            ..GeometryInteractionPolicy::default()
        };
        assert!(profile_point.is_interactive(profile_scope));
        assert!(!profile_point.is_interactive(construction_scope));
        assert!(!construction_point.is_interactive(profile_scope));
        assert!(construction_point.is_interactive(construction_scope));

        let rejection_points = [
            "Redundant inference rejection start marker",
            "Redundant inference rejection end marker",
        ]
        .map(|label| marker(label).1);
        let rejection_positions = rejection_points.map(|point| {
            document
                .point(point)
                .expect("rejection specimen point")
                .position
        });
        assert_eq!(
            rejection_positions[0][1].to_bits(),
            rejection_positions[1][1].to_bits(),
            "the prepared point-to-point horizontal inference must be algebraically redundant"
        );
        assert_ne!(
            rejection_positions[0][0].to_bits(),
            rejection_positions[1][0].to_bits()
        );
        assert!(rejection_points.into_iter().all(|point| {
            document.constraints().iter().all(|constraint| {
                !matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::FixedPoint { point: fixed, .. } if fixed == point
                )
            })
        }));
        let rejection_reference = document
            .curves()
            .iter()
            .find(|curve| curve.label == "Redundant inference rejection reference")
            .expect("rejection reference")
            .id;
        assert!(document.constraints().iter().any(|constraint| matches!(
            constraint.definition,
            DocumentConstraintDefinition::Horizontal { line }
                if line == CurveSpan::line(rejection_reference)
        )));

        let ambiguity =
            ["Exact ambiguity reference A", "Exact ambiguity reference B"].map(|label| {
                let curve = document
                    .curves()
                    .iter()
                    .find(|curve| curve.label == label)
                    .expect("exact ambiguity reference");
                let CurveDefinition::Line { start, end, .. } = curve.definition else {
                    panic!("exact ambiguity reference must remain a line")
                };
                [
                    document.point(start).expect("ambiguity start").position,
                    document.point(end).expect("ambiguity end").position,
                ]
            });
        assert_eq!(ambiguity[0], ambiguity[1]);

        assert!(coordinator.editor().selection().is_empty());
        assert!(coordinator.feature_document().features().is_empty());
        assert_eq!(coordinator.history_len(), 1);
        assert!(!coordinator.can_undo());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one rejection fixture proves displayed plan, atomic refusal, recoverable draft and retry"
    )]
    fn auto_constraint_rejection_specimen_preserves_the_recoverable_draft() {
        let mut catalog = SampleCatalogState::default();
        let mut coordinator = catalog
            .open_key(SampleId::AutoConstraintDrafting.key())
            .expect("auto-constraint drafting playground");
        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted playground");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            Viewport::new([1000.0, 700.0], [0.0, -2.0], 25.0).expect("playground viewport"),
            0.25,
        )
        .expect("playground scene")
        .with_retained_session(coordinator.session())
        .expect("bound playground scene");
        let positions = [
            "Redundant inference rejection start point",
            "Redundant inference rejection end point",
        ]
        .map(|label| {
            coordinator
                .session()
                .design_document()
                .points()
                .iter()
                .find(|point| point.label == label)
                .unwrap_or_else(|| panic!("missing rejection point `{label}`"))
                .position
        });

        coordinator.editor_mut().activate_tool(EditorTool::Line);
        let history = (coordinator.history_len(), coordinator.history_cursor());
        let first = coordinator.pointer_down(
            &scene,
            PointerInput {
                pointer_id: 170,
                position: scene.viewport.model_to_screen(positions[0]),
                modifiers: Modifiers::default(),
            },
        );
        assert!(first.iter().all(|effect| !matches!(
            effect,
            EditorEffect::CommitConstruction { .. } | EditorEffect::CommitConstructionPlan { .. }
        )));
        let second = coordinator.pointer_down(
            &scene,
            PointerInput {
                pointer_id: 170,
                position: scene.viewport.model_to_screen(positions[1]),
                modifiers: Modifiers::default(),
            },
        );
        let (token, commit) = second
            .iter()
            .find_map(|effect| match effect {
                EditorEffect::CommitConstructionPlan { token, plan, .. }
                    if matches!(
                        plan.relations.as_slice(),
                        [InferredRelation::Horizontal { .. }]
                    ) =>
                {
                    Some((*token, effect))
                }
                _ => None,
            })
            .expect("redundant horizontal plan");
        let rejection = coordinator.apply_editor_effect(commit);
        assert!(
            matches!(
                rejection,
                Err(CoordinatorError::RedundantInferredConstruction { .. })
            ),
            "expected redundant inferred construction, got {rejection:?}"
        );
        assert_eq!(
            (coordinator.history_len(), coordinator.history_cursor()),
            history
        );
        assert!(
            coordinator
                .editor()
                .pending_construction_commit_token()
                .is_some()
        );
        assert!(
            coordinator
                .acknowledge_construction_commit(token, false)
                .is_empty()
        );
        let corrected = [positions[1][0], positions[1][1] + 2.0];
        let preview = coordinator.editor_mut().pointer_move(
            &scene,
            PointerInput {
                pointer_id: 170,
                position: scene.viewport.model_to_screen(corrected),
                modifiers: Modifiers::default(),
            },
        );
        assert!(
            preview
                .iter()
                .any(|effect| matches!(effect, EditorEffect::PreviewConstruction(_)))
        );
        let retry = coordinator.pointer_down(
            &scene,
            PointerInput {
                pointer_id: 170,
                position: scene.viewport.model_to_screen(corrected),
                modifiers: Modifiers::default(),
            },
        );
        assert!(retry.iter().any(|effect| matches!(
            effect,
            EditorEffect::CommitConstructionPlan { plan, .. } if plan.relations.is_empty()
        )));
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one direct sample lifecycle covers role, retained point, deletion, history, and workspace reload"
    )]
    fn auto_constraint_playground_remains_ordinary_editable_across_reload() {
        let mut catalog = SampleCatalogState::default();
        let mut coordinator = catalog
            .open_key(SampleId::AutoConstraintDrafting.key())
            .expect("auto-constraint drafting playground");
        let (profile_marker, profile_point, construction_marker) = {
            let document = coordinator.session().design_document();
            let marker = |label: &str| {
                let curve = document
                    .curves()
                    .iter()
                    .find(|curve| curve.label == label)
                    .unwrap_or_else(|| panic!("missing point marker `{label}`"));
                let CurveDefinition::Circle { center, .. } = &curve.definition else {
                    panic!("point marker `{label}` must remain circular")
                };
                (curve.id, *center)
            };
            let (profile_marker, profile_point) = marker("Profile point specimen marker");
            let (construction_marker, _) = marker("Construction point specimen marker");
            (profile_marker, profile_point, construction_marker)
        };

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(CurveSpan::line(profile_marker))]);
        coordinator
            .toggle_selected_geometry_role(coordinator.session().design_identity())
            .expect("ordinary sample role edit");
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .geometry_role(profile_marker),
            Some(GeometryRole::Construction)
        );
        coordinator.undo().expect("undo role edit");
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .geometry_role(profile_marker),
            Some(GeometryRole::Profile)
        );
        coordinator.redo().expect("redo role edit");

        let moved_position = [-10.5, -7.75];
        coordinator
            .apply_edit(
                coordinator.session().design_identity(),
                geosolve_sketch::DocumentEdit::SetPointPosition {
                    point: profile_point,
                    position: moved_position,
                },
            )
            .expect("ordinary retained point mutation");
        let accepted_position = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted moved sample")
            .document()
            .point(profile_point)
            .expect("moved profile point")
            .position;
        assert_eq!(
            accepted_position.map(f64::to_bits),
            moved_position.map(f64::to_bits)
        );

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Curve(CurveSpan::line(construction_marker))]);
        coordinator
            .delete_selected(coordinator.session().design_identity())
            .expect("delete ordinary sample curve");
        assert!(
            coordinator
                .session()
                .design_document()
                .curve(construction_marker)
                .is_none()
        );
        coordinator.undo().expect("undo sample deletion");
        assert!(
            coordinator
                .session()
                .design_document()
                .curve(construction_marker)
                .is_some()
        );
        coordinator.redo().expect("redo sample deletion");
        assert!(
            coordinator
                .session()
                .design_document()
                .curve(construction_marker)
                .is_none()
        );
        coordinator
            .undo()
            .expect("restore sample curve before save");

        let design_json = coordinator
            .session()
            .design_document()
            .to_draft_v5_json()
            .expect("edited sample JSON");
        let accepted_json = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted edited sample")
            .document()
            .to_draft_v5_json()
            .expect("accepted edited sample JSON");
        let snapshot =
            WorkspaceSnapshot::from_coordinator(&coordinator).expect("capture edited sample");
        let decoded = WorkspaceSnapshot::decode(
            &snapshot
                .encode()
                .expect("encode edited auto-constraint playground"),
        )
        .expect("decode edited auto-constraint playground");
        let session = decoded
            .restore_session(
                geosolve_sketch::DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .expect("restore edited sample session");
        let mut restored = RetainedEditorCoordinator::with_features_and_high_water(
            session,
            decoded
                .feature_document()
                .expect("restore empty feature sidecar"),
            decoded.feature_lifecycle_high_water(),
            decoded.computed_evaluation_high_water(),
        )
        .expect("restore edited sample coordinator");
        assert_eq!(
            restored
                .session()
                .design_document()
                .to_draft_v5_json()
                .expect("restored sample JSON"),
            design_json
        );
        assert_eq!(
            restored
                .session()
                .accepted_state_for_current_input()
                .expect("restored accepted sample")
                .document()
                .to_draft_v5_json()
                .expect("restored accepted sample JSON"),
            accepted_json
        );
        assert!(restored.feature_document().features().is_empty());
        assert_eq!(restored.history_len(), 1);

        let checkpoint = restored
            .persistence_checkpoint()
            .expect("restored sample persistence checkpoint");
        restored
            .editor_mut()
            .set_selection([SelectionItem::Curve(CurveSpan::line(construction_marker))]);
        restored
            .delete_selected(restored.session().design_identity())
            .expect("mutate restored sample before reload");
        restored
            .reload(&checkpoint)
            .expect("reload exact edited sample checkpoint");
        assert!(
            restored
                .session()
                .design_document()
                .curve(construction_marker)
                .is_some()
        );
        assert_eq!(restored.history_len(), 1);
        assert!(restored.editor().selection().is_empty());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one focused M69 sample test owns roles, overlap priority and thin rendering metadata"
    )]
    fn construction_reference_sample_owns_shared_incidence_and_exact_overlap_priority() {
        let mut catalog = SampleCatalogState::default();
        let coordinator = catalog
            .open_key(SampleId::ConstructionReference.key())
            .expect("Construction/reference sample");
        let document = coordinator.session().design_document();
        let diagonal = document
            .curves()
            .iter()
            .find(|curve| curve.label == "Shared-corner construction diagonal")
            .expect("shared diagonal");
        let guide = document
            .curves()
            .iter()
            .find(|curve| curve.label == "Construction guide overlapping the profile base")
            .expect("overlapping guide");
        assert_eq!(
            document.geometry_role(diagonal.id),
            Some(GeometryRole::Construction)
        );
        assert_eq!(
            document.geometry_role(guide.id),
            Some(GeometryRole::Construction)
        );

        let accepted = coordinator
            .session()
            .accepted_state_for_current_input()
            .expect("accepted sample");
        let viewport = Viewport::new([800.0, 600.0], [2.0, 1.5], 100.0).expect("viewport");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            document,
            viewport,
            0.5,
        )
        .expect("scene");
        let overlap = viewport.model_to_screen([2.0, 0.0]);
        let all_hit = scene
            .hit_test_with_policy(
                overlap,
                PickTolerance::default(),
                GeometryInteractionPolicy::default(),
            )
            .expect("All overlap hit");
        let SelectionItem::Curve(all_span) = all_hit.item else {
            panic!("overlap midpoint should resolve a curve")
        };
        assert_eq!(
            document.geometry_role(all_span.curve),
            Some(GeometryRole::Profile),
            "Profile must win the near-identical All-scope overlap"
        );
        let construction_hit = scene
            .hit_test_with_policy(
                ScreenPoint {
                    x: overlap.x,
                    y: overlap.y,
                },
                PickTolerance::default(),
                GeometryInteractionPolicy {
                    scope: GeometryPickScope::Construction,
                    ..GeometryInteractionPolicy::default()
                },
            )
            .expect("Construction-only overlap hit");
        assert_eq!(
            construction_hit.item,
            SelectionItem::Curve(CurveSpan::line(guide.id))
        );

        let profile_policy = GeometryInteractionPolicy {
            scope: GeometryPickScope::Profile,
            ..GeometryInteractionPolicy::default()
        };
        let markup = super::super::scene::svg_markup_with_computed_context_and_action_stamp(
            Some(&scene),
            Some(accepted),
            &[],
            &[],
            &[],
            EditorHoverState::default(),
            None,
            None,
            None,
            None,
            None,
            profile_policy,
            viewport,
        );
        let guide_path = markup
            .split("<path")
            .find(|fragment| {
                fragment.contains(&format!("data-persistent-id=\"{}\"", guide.id))
                    && fragment.contains("data-editor-segment=\"0\"")
            })
            .and_then(|fragment| fragment.split("/>").next())
            .expect("visible Construction guide path");
        assert!(guide_path.contains("data-interactive=\"false\""));
        assert!(!guide_path.contains("data-editor-item="));
        let guide_start = match &guide.definition {
            CurveDefinition::Line { start, .. } => *start,
            _ => panic!("Construction guide must remain a line"),
        };
        let guide_point = markup
            .split("<circle")
            .find(|fragment| fragment.contains(&format!("data-persistent-id=\"{guide_start}\"")))
            .and_then(|fragment| fragment.split("/>").next())
            .expect("visible Construction-only guide point");
        assert!(guide_point.contains("data-interactive=\"false\""));
        assert!(!guide_point.contains("data-editor-item="));
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

            let snapshot = WorkspaceSnapshot::from_coordinator(&coordinator)
                .expect("capture sample workspace");
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
    #[allow(
        clippy::too_many_lines,
        reason = "one inventory test keeps the ordinary Fillet playground fixture auditable"
    )]
    fn fillet_workshop_is_a_plain_editable_document_with_expected_sources() {
        let mut catalog = SampleCatalogState::default();
        let fillet = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("fillet workshop");
        assert_eq!(fillet.session().design_document().curves().len(), 13);
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
        let fillet_document = fillet.session().design_document();
        let editable_polylines = fillet_document
            .curves()
            .iter()
            .filter_map(|curve| match &curve.definition {
                CurveDefinition::Polyline {
                    points,
                    closed: false,
                    ..
                } if points.len() == 4 => Some(points),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(editable_polylines.len(), 2);
        assert_eq!(fillet_document.constraints().len(), 19);
        assert!(
            fillet_document
                .constraints()
                .iter()
                .all(|constraint| matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::FixedPoint { .. }
                ))
        );
        let fixed_points = fillet_document
            .constraints()
            .iter()
            .filter_map(|constraint| match constraint.definition {
                DocumentConstraintDefinition::FixedPoint { point, .. } => Some(point),
                _ => None,
            })
            .collect::<HashSet<_>>();
        assert!(
            editable_polylines
                .iter()
                .flat_map(|points| points.iter())
                .all(|point| !fixed_points.contains(point)),
            "playground polylines must be directly draggable without deleting setup constraints"
        );
        let shared_junction = fillet_document
            .points()
            .iter()
            .find(|point| point.label == "High-valence shared junction")
            .expect("high-valence playground point")
            .id;
        assert_eq!(
            fillet_document
                .curves()
                .iter()
                .filter(|curve| matches!(
                    curve.definition,
                    CurveDefinition::Line { start, end, .. }
                        if start == shared_junction || end == shared_junction
                ))
                .count(),
            3
        );
        assert_eq!(fillet_document.dimensions().len(), 2);
        assert!(
            fillet_document
                .dimensions()
                .iter()
                .all(|dimension| matches!(
                    dimension.definition,
                    DocumentDimensionDefinition::Radius { .. }
                ))
        );
        for label in ["Friendly line-circle", "Near-fold stress line-circle"] {
            assert!(
                fillet_document
                    .curves()
                    .iter()
                    .any(|curve| curve.label == format!("{label} circular support"))
            );
            assert!(
                fillet_document
                    .curves()
                    .iter()
                    .any(|curve| curve.label == format!("{label} linear support"))
            );
        }
        assert!(fillet.session().accepted_state().is_some());
    }

    #[test]
    fn friendly_line_circle_has_a_finite_rail_while_stress_is_an_explicit_fold() {
        let mut catalog = SampleCatalogState::default();
        let mut friendly = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("friendly Fillet playground");
        let friendly_scene = line_circle_affordance_scene(&mut friendly, "Friendly line-circle");
        assert_eq!(friendly_scene.fillet_affordances.len(), 1);
        let rail = friendly_scene.fillet_affordances[0].radius_rail;
        assert!(rail.model_derivative.into_iter().all(f64::is_finite));
        assert!(rail.model_derivative[0].hypot(rail.model_derivative[1]) > 0.0);
        assert!(
            friendly_scene
                .computed_fillet_continuation_statuses
                .is_empty()
        );

        let mut stress = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("stress Fillet playground");
        let stress_scene =
            line_circle_affordance_scene(&mut stress, "Near-fold stress line-circle");
        assert!(
            stress_scene.fillet_affordances.is_empty(),
            "a branch fold must not advertise a finite radius rail"
        );
        assert_eq!(stress_scene.computed_fillet_continuation_statuses.len(), 1);
        assert_eq!(
            stress_scene.computed_fillet_continuation_statuses[0]
                .limit
                .kind,
            ComputedFilletContinuationLimitKind::BranchFold,
        );
        let status_markup = super::super::scene::fillet_action_panel_markup_with_stamp(
            &stress_scene,
            None,
            GeometryInteractionPolicy::default(),
        );
        assert!(status_markup.contains("data-fillet-limit=\"branch-fold\""));
        assert!(status_markup.contains("<strong>Branch fold:</strong>"));
    }

    #[test]
    fn fillet_playground_screen_picks_prepare_two_current_corner_arcs() {
        let mut catalog = SampleCatalogState::default();
        let mut coordinator = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("fillet playground");
        let scene = fillet_playground_scene(&coordinator);
        let mut authoring = activate_fillet_playground(&coordinator);
        let first = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([-6.0, -2.0]),
                PickTolerance::default(),
                "Playground batch Fillet",
            )
            .expect("first playground corner transaction");
        assert!(matches!(
            first.outcome,
            FeatureAuthoringOutcome::PreviewRequested { ref guidance, .. }
                if guidance.completed_corners == 1
        ));
        assert!(first.preview.is_some());
        let second = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([-6.0, -7.0]),
                PickTolerance::default(),
                "Playground batch Fillet",
            )
            .expect("second playground corner transaction");
        assert!(matches!(
            second.outcome,
            FeatureAuthoringOutcome::PreviewRequested { ref guidance, .. }
                if guidance.completed_corners == 2
        ));
        assert!(second.preview.is_some());
        assert_eq!(
            coordinator
                .feature_authoring_preview()
                .expect("held playground preview")
                .snapshot()
                .edges()
                .iter()
                .filter(|edge| matches!(
                    edge.geometry,
                    geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(_)
                ))
                .count(),
            2
        );
    }

    #[test]
    fn fillet_playground_crossing_and_high_valence_use_bounded_screen_pick_policy() {
        let mut catalog = SampleCatalogState::default();
        let mut coordinator = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("fillet playground");
        let scene = fillet_playground_scene(&coordinator);
        let mut authoring = activate_fillet_playground(&coordinator);
        let first = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([-6.0, 5.0]),
                PickTolerance::default(),
                "Playground crossing Fillet",
            )
            .expect("first overlapping-line transaction");
        assert!(matches!(
            first.outcome,
            FeatureAuthoringOutcome::Collecting { ref pending, .. } if pending.len() == 1
        ));
        let second = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([-3.0, 8.0]),
                PickTolerance::default(),
                "Playground crossing Fillet",
            )
            .expect("second overlapping-line transaction");
        assert!(
            matches!(
            &second.outcome,
            FeatureAuthoringOutcome::PreviewRequested { guidance, .. }
                    if guidance.completed_corners == 1
            ),
            "second crossing click did not complete the corner: {:?}",
            second.outcome
        );
        assert!(second.preview.is_some());

        let mut coordinator = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("fresh fillet playground");
        let scene = fillet_playground_scene(&coordinator);
        let mut authoring = activate_fillet_playground(&coordinator);
        let before = authoring.clone();
        let ambiguous = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([14.0, 6.0]),
                PickTolerance::default(),
                "Playground high-valence Fillet",
            )
            .expect("high-valence transaction");
        assert!(matches!(
            ambiguous.outcome,
            FeatureAuthoringOutcome::Warning(ref warning)
                if warning.kind == FeatureAuthoringWarningKind::AmbiguousTrimSide
        ));
        assert!(ambiguous.preview.is_none());
        assert_eq!(authoring, before);

        let first_branch = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([14.0, 8.0]),
                PickTolerance::default(),
                "Playground high-valence recovery",
            )
            .expect("first unambiguous branch transaction");
        assert!(matches!(
            first_branch.outcome,
            FeatureAuthoringOutcome::Collecting { ref pending, .. } if pending.len() == 1
        ));
        let second_branch = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([12.25, 4.75]),
                PickTolerance::default(),
                "Playground high-valence recovery",
            )
            .expect("second unambiguous branch transaction");
        assert!(matches!(
            second_branch.outcome,
            FeatureAuthoringOutcome::PreviewRequested { ref guidance, .. }
                if guidance.completed_corners == 1
        ));
        assert!(second_branch.preview.is_some());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exact rejected-screen-pick and recovery transaction"
    )]
    fn fillet_playground_short_middle_span_rejects_and_recovers_transactionally() {
        let mut catalog = SampleCatalogState::default();
        let mut coordinator = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("fillet playground");
        let scene = fillet_playground_scene(&coordinator);
        let mut authoring = activate_fillet_playground(&coordinator);
        let initial_options = FeatureAuthoringOptions {
            fillet_radius: Some(1.0),
            ..authoring.options()
        };
        coordinator
            .transact_feature_authoring_options(
                &mut authoring,
                initial_options,
                None,
                "Oversized playground Fillet",
            )
            .expect("initialize oversized playground radius");
        let first = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([15.0, -3.0]),
                PickTolerance::default(),
                "Playground claim-conflict Fillet",
            )
            .expect("first short-middle corner transaction");
        assert!(matches!(
            first.outcome,
            FeatureAuthoringOutcome::PreviewRequested { ref guidance, .. }
                if guidance.completed_corners == 1
        ));
        let one_corner_state = authoring.clone();
        let valid_preview = coordinator
            .feature_authoring_preview()
            .expect("valid one-corner short-middle preview")
            .metadata()
            .clone();

        let error = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([15.0, -4.75]),
                PickTolerance::default(),
                "Playground claim-conflict Fillet",
            )
            .expect_err("oversized second corner must reject");
        assert!(matches!(
            error,
            CoordinatorError::FeatureAuthoringPreviewRejected(
                ComputedFeatureFailure::ConsumedSourceInterval { .. }
                    | ComputedFeatureFailure::EndpointClaimConflict { .. }
            )
        ));
        assert_eq!(authoring, one_corner_state);
        assert_eq!(
            coordinator
                .feature_authoring_preview()
                .expect("valid preview retained after rejection")
                .metadata(),
            &valid_preview
        );

        let recovered_options = FeatureAuthoringOptions {
            fillet_radius: Some(0.5),
            ..authoring.options()
        };
        let resized = coordinator
            .transact_feature_authoring_options(
                &mut authoring,
                recovered_options,
                None,
                "Recovered playground Fillet",
            )
            .expect("smaller shared-radius retry");
        assert!(matches!(
            resized.outcome,
            FeatureAuthoringOutcome::PreviewRequested { ref guidance, .. }
                if guidance.completed_corners == 1
        ));
        assert!(resized.preview.is_some());
        let recovered = coordinator
            .transact_feature_authoring_pick_at(
                &mut authoring,
                &scene,
                scene.viewport.model_to_screen([15.0, -4.75]),
                PickTolerance::default(),
                "Recovered playground Fillet",
            )
            .expect("retry second short-middle corner");
        assert!(matches!(
            recovered.outcome,
            FeatureAuthoringOutcome::PreviewRequested { ref guidance, .. }
                if guidance.completed_corners == 2
        ));
        assert!(recovered.preview.is_some());
        assert_eq!(
            authoring.options().fillet_radius.map(f64::to_bits),
            Some(0.5_f64.to_bits())
        );
        assert_eq!(
            coordinator
                .feature_authoring_preview()
                .expect("recovered two-corner preview")
                .snapshot()
                .edges()
                .iter()
                .filter(|edge| matches!(
                    edge.geometry,
                    geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(_)
                ))
                .count(),
            2
        );
    }

    #[test]
    fn fillet_playground_curve_specimens_author_through_screen_transactions() {
        let cases = [
            (
                "Friendly line-circle linear support",
                0.4,
                "Friendly line-circle circular support",
                4.5,
            ),
            (
                "Line-Bezier linear support",
                0.44,
                "Line-Bezier curved support",
                0.74,
            ),
        ];
        for (first_label, first_parameter, second_label, second_parameter) in cases {
            let mut catalog = SampleCatalogState::default();
            let mut coordinator = catalog
                .open_key(SampleId::FilletWorkshop.key())
                .expect("fillet playground");
            let scene = fillet_playground_scene(&coordinator);
            let mut authoring = activate_fillet_playground(&coordinator);
            assert_eq!(
                authoring.options().fillet_radius.map(f64::to_bits),
                Some(0.5_f64.to_bits()),
                "the playground should open with a useful curved-pair radius"
            );
            let (first_span, second_span, first_position, second_position) = {
                let snapshot = coordinator
                    .feature_authoring_snapshot()
                    .expect("accepted playground snapshot");
                let document = snapshot.sketch_document();
                let span = |label| CurveSpan {
                    curve: document
                        .curves()
                        .iter()
                        .find(|curve| curve.label == label)
                        .unwrap_or_else(|| panic!("missing playground curve `{label}`"))
                        .id,
                    segment: 0,
                };
                let first_span = span(first_label);
                let second_span = span(second_label);
                (
                    first_span,
                    second_span,
                    document
                        .evaluate_curve_jet(first_span, first_parameter)
                        .expect("first playground pick position")
                        .position,
                    document
                        .evaluate_curve_jet(second_span, second_parameter)
                        .expect("second playground pick position")
                        .position,
                )
            };
            let first = coordinator
                .transact_feature_authoring_pick_at(
                    &mut authoring,
                    &scene,
                    scene.viewport.model_to_screen(first_position.into()),
                    PickTolerance::default(),
                    format!("{first_label} Fillet"),
                )
                .expect("first curve-family transaction");
            assert!(matches!(
                first.outcome,
                FeatureAuthoringOutcome::Collecting { ref pending, .. }
                    if pending.len() == 1
            ));
            let second = coordinator
                .transact_feature_authoring_pick_at(
                    &mut authoring,
                    &scene,
                    scene.viewport.model_to_screen(second_position.into()),
                    PickTolerance::default(),
                    format!("{first_label} / {second_label} Fillet"),
                )
                .expect("second curve-family transaction");
            let FeatureAuthoringOutcome::PreviewRequested {
                ref candidate,
                ref guidance,
            } = second.outcome
            else {
                panic!(
                    "{first_label} / {second_label} did not produce a preview: {:?}",
                    second.outcome
                );
            };
            assert_eq!(guidance.completed_corners, 1);
            let corner = &candidate.corners()[0].corner;
            assert!([corner.first.source.span, corner.second.source.span].contains(&first_span));
            assert!([corner.first.source.span, corner.second.source.span].contains(&second_span));
            assert!(second.preview.is_some());
        }
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one exact computed-feature persisted lifecycle"
    )]
    fn fillet_workshop_computed_set_round_trips_without_mutating_the_sketch_graph() {
        let mut catalog = SampleCatalogState::default();
        let mut coordinator = catalog
            .open_key(SampleId::FilletWorkshop.key())
            .expect("fillet workshop");
        let corner = {
            let document = coordinator.session().design_document();
            let polyline = document
                .curves()
                .iter()
                .find(|curve| curve.label == "Editable batch and sequential polyline")
                .expect("workshop polyline corner");
            let CurveDefinition::Polyline { points, .. } = &polyline.definition else {
                panic!("workshop corner support must remain a polyline");
            };
            points[1]
        };
        let ordinary_before = coordinator
            .session()
            .design_document()
            .to_draft_v5_json()
            .expect("ordinary sketch JSON");
        let ordinary_identity = coordinator.session().design_identity();
        let ordinary_counts = {
            let document = coordinator.session().design_document();
            (
                document.points().len(),
                document.curves().len(),
                document.constraints().len(),
                document.dimensions().len(),
                document.contacts().len(),
                document.trim_views().len(),
            )
        };
        let authoring_snapshot = coordinator
            .feature_authoring_snapshot()
            .expect("current accepted feature-authoring snapshot");
        let accepted_document = authoring_snapshot.sketch_document().clone();
        let picks = coordinator
            .feature_authoring_picks_for_item(SelectionItem::Point(corner), None)
            .expect("unambiguous polyline corner picks");
        let mut authoring = FeatureAuthoringState::default();
        let _ = authoring.activate(
            &authoring_snapshot,
            &accepted_document,
            FeatureAuthoringTool::Fillet,
            &[],
        );
        assert!(matches!(
            authoring.set_options(
                &authoring_snapshot,
                FeatureAuthoringOptions {
                    fillet_radius: Some(0.8),
                    ..FeatureAuthoringOptions::default()
                },
            ),
            FeatureAuthoringOutcome::Collecting { .. }
        ));
        let outcome = authoring.pick_many(&authoring_snapshot, picks);
        let FeatureAuthoringOutcome::PreviewRequested { candidate, .. } = outcome else {
            panic!("workshop computed Fillet candidate expected: {outcome:?}");
        };
        let metadata = coordinator
            .prepare_feature_authoring_preview(
                coordinator.feature_document().identity(),
                &candidate,
                "Workshop computed Fillet",
            )
            .expect("whole-set computed preview");
        assert_eq!(
            coordinator
                .feature_authoring_preview()
                .expect("held preview")
                .metadata()
                .token,
            metadata.token
        );
        let fillet = coordinator
            .apply_feature_authoring_preview(metadata.token, &candidate)
            .expect("exact held-preview apply")
            .value;

        assert_eq!(coordinator.session().design_identity(), ordinary_identity);
        assert_eq!(
            coordinator
                .session()
                .design_document()
                .to_draft_v5_json()
                .expect("ordinary sketch JSON after feature"),
            ordinary_before
        );
        let ordinary_after_counts = {
            let document = coordinator.session().design_document();
            (
                document.points().len(),
                document.curves().len(),
                document.constraints().len(),
                document.dimensions().len(),
                document.contacts().len(),
                document.trim_views().len(),
            )
        };
        assert_eq!(ordinary_after_counts, ordinary_counts);
        assert_eq!(coordinator.feature_document().features().len(), 1);
        assert!(
            coordinator
                .computed_snapshot()
                .expect("computed geometry")
                .edges()
                .iter()
                .any(|edge| matches!(
                    edge.geometry,
                    geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(_)
                ))
        );

        let snapshot =
            WorkspaceSnapshot::from_coordinator(&coordinator).expect("capture workspace");
        let decoded = WorkspaceSnapshot::decode(&snapshot.encode().expect("encode v4 workspace"))
            .expect("decode v4 workspace");
        let session = decoded
            .restore_session(
                geosolve_sketch::DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .expect("restore sketch lifecycle");
        let mut restored = RetainedEditorCoordinator::with_features_and_high_water(
            session,
            decoded.feature_document().expect("restore feature sidecar"),
            decoded.feature_lifecycle_high_water(),
            decoded.computed_evaluation_high_water(),
        )
        .expect("restore computed workspace");
        assert_eq!(restored.feature_document().features().len(), 1);
        assert_eq!(
            restored
                .session()
                .design_document()
                .to_draft_v5_json()
                .expect("restored ordinary sketch JSON"),
            ordinary_before
        );
        restored
            .set_computed_fillet_radius(restored.feature_document().identity(), fillet, 0.96)
            .expect("edit restored shared computed radius");
        let feature = restored
            .feature_document()
            .feature(fillet)
            .expect("restored Fillet set");
        let geosolve_sketch_features::ComputedFeatureDefinition::FilletSet(fillet_set) =
            &feature.definition;
        assert!((fillet_set.radius - 0.96).abs() <= 1.0e-12);
        assert_eq!(
            restored
                .session()
                .design_document()
                .to_draft_v5_json()
                .expect("ordinary sketch after radius edit"),
            ordinary_before,
            "computed radius editing must not create an M28 curve, constraint, dimension, contact, or trim"
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
    fn menu_contains_every_sample_once() {
        let markup = SampleCatalogState::default().menu_markup();
        assert_eq!(
            markup.matches("data-sample-id=").count(),
            SampleId::ALL.len()
        );
        for sample in SampleId::ALL {
            assert_eq!(
                markup
                    .matches(&format!("data-sample-id=\"{}\"", sample.key()))
                    .count(),
                1,
                "{} must have exactly one menu leaf",
                sample.key()
            );
        }
    }
}
