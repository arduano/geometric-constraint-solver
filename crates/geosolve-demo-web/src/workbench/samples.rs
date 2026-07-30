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
    LockedElbow,
    BranchFourBar,
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
        Self::LockedElbow,
        Self::BranchFourBar,
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
            Self::LockedElbow => "locked-elbow-branches",
            Self::BranchFourBar => "locked-four-bar-branches",
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

const MECHANISMS: [SampleDefinition; 15] = [
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
        SampleId::LockedElbow,
        "Locked elbow · open/crossed branches",
        AlphaScenarioKind::BranchLockedElbow,
    ),
    sample(
        SampleId::BranchFourBar,
        "Locked four-bar · open/crossed branches",
        AlphaScenarioKind::BranchFourBar,
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

const CURVES: [SampleDefinition; 3] = [
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

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use geosolve_constraint_editor::{
        AlternateBranchSearchStatus, RetainedEditorCoordinator, SelectionItem,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{DocumentConstraintDefinition, RetainedSketchDocumentSession};

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
    fn branch_examples_offer_bounded_non_authoritative_alternates() {
        for (id, point_label) in [
            (SampleId::LockedElbow, "Locked elbow joint B"),
            (SampleId::BranchFourBar, "Four-bar output joint B"),
        ] {
            let mut catalog = SampleCatalogState::default();
            let mut coordinator = catalog.open_key(id.key()).expect("branch sample");
            let point = coordinator
                .session()
                .design_document()
                .points()
                .iter()
                .find(|point| point.label == point_label)
                .expect("branch point")
                .id;
            let design = coordinator.session().design_identity();
            let result = coordinator.propose_alternate_branch(point);
            assert_eq!(
                result.status,
                AlternateBranchSearchStatus::Proposed,
                "{}: {result:#?}",
                id.key()
            );
            assert_eq!(coordinator.session().design_identity(), design);
            assert!(coordinator.alternate_branch_preview_session().is_some());
        }
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
