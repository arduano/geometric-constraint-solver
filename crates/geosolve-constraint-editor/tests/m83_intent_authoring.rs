// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ColdIntentMaterializer, ConstructionCommitPlan, ConstructionPoint, ConstructionProposal,
    ConstructionRelationDefinition, DraftContactDescriptor, DraftPointSlot, DraftSpanSlot,
    GeometryToolVariant, InferredRelation, IntentNativeBinding, NurbsConstructionOptions,
    ProjectionalAuthoringError, ProjectionalIntentCoordinator, projectional_construction_patch,
};
use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveSpan, DocumentArcSweep, DocumentBSplineForm,
    DocumentConstraintDefinition, DocumentHyperbolaBranch, DocumentId, GeometryRole, PersistentId,
    TangentOrientation,
};
use geosolve_sketch_intent::{
    ConstraintKind, GeometryRecipeKind, InputRole, InputSlot, IntentKey, IntentLiteral,
    IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy,
    IntentPlanDisposition, IntentPortRole, IntentPortSelector, IntentSessionId, IntentUnit,
    LeafField, PatchPortRef,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).unwrap()
}

const fn selector(role: IntentPortRole, index: u16) -> IntentPortSelector {
    IntentPortSelector::Node { role, index }
}

const fn child_selector(ordinal: u16, role: IntentPortRole) -> IntentPortSelector {
    IntentPortSelector::InitialChild {
        ordinal,
        role,
        index: 0,
    }
}

fn coordinate(value: f64) -> IntentLiteral {
    IntentLiteral::Quantity {
        value,
        unit: IntentUnit::Length,
    }
}

fn coordinator(raw: u128) -> ProjectionalIntentCoordinator {
    ProjectionalIntentCoordinator::empty(
        IntentSessionId::from_raw(raw),
        ColdIntentMaterializer::with_default_policy(
            DocumentId(PersistentId::from_u128(raw << 32)),
            1.0,
        )
        .unwrap(),
    )
    .unwrap()
}

fn seed_point(
    coordinator: &mut ProjectionalIntentCoordinator,
    position: [f64; 2],
) -> geosolve_sketch::DesignPointId {
    let alias = key("seed-point");
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key("seed.point"),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::X,
        coordinate(position[0]),
    )
    .with_instance_leaf(
        selector(IntentPortRole::Primary, 0),
        LeafField::Y,
        coordinate(position[1]),
    );
    let outcome = coordinator
        .apply_patch(IntentPatch::new(
            coordinator.intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: alias.clone(),
                draft: Box::new(draft),
                cell: None,
            }],
        ))
        .unwrap();
    let port = outcome
        .aliases
        .port(&alias, selector(IntentPortRole::Primary, 0))
        .unwrap();
    let IntentNativeBinding::Point(point) = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(port)
        .unwrap()
    else {
        panic!("seed point output must own one native point");
    };
    point
}

fn new(position: [f64; 2]) -> ConstructionPoint {
    ConstructionPoint::New(position)
}

const fn created_span(curve_index: usize, segment: u32) -> DraftSpanSlot {
    DraftSpanSlot::Created {
        curve_index,
        segment,
    }
}

fn roles(count: usize) -> Vec<GeometryRole> {
    vec![GeometryRole::Profile; count]
}

fn rectangle_intrinsics(variant: GeometryToolVariant) -> Vec<ConstructionRelationDefinition> {
    use GeometryToolVariant as V;
    use InferredRelation as R;

    let mut relations = match variant {
        V::TwoPointAlignedRectangle | V::CenterRectangle => vec![
            R::Horizontal {
                line: created_span(0, 0),
            },
            R::Vertical {
                line: created_span(1, 0),
            },
            R::Horizontal {
                line: created_span(2, 0),
            },
            R::Vertical {
                line: created_span(3, 0),
            },
        ],
        V::ThreePointCornerRectangle | V::ThreePointCenterRectangle => vec![
            R::Perpendicular {
                first: created_span(0, 0),
                second: created_span(1, 0),
            },
            R::Parallel {
                first: created_span(0, 0),
                second: created_span(2, 0),
            },
            R::Parallel {
                first: created_span(1, 0),
                second: created_span(3, 0),
            },
        ],
        _ => panic!("not a rectangle recipe"),
    };
    if matches!(variant, V::CenterRectangle | V::ThreePointCenterRectangle) {
        relations.push(R::Midpoint {
            point: DraftPointSlot::Created { point_index: 0 },
            line: created_span(4, 0),
        });
    }
    relations
        .into_iter()
        .map(ConstructionRelationDefinition::recipe_intrinsic)
        .collect()
}

#[allow(clippy::too_many_lines)]
fn recipe_plan(
    variant: GeometryToolVariant,
    tangent_source: Option<CurveSpan>,
) -> ConstructionCommitPlan {
    use GeometryToolVariant as V;

    match variant {
        V::SketchPoint => ConstructionCommitPlan {
            proposal: ConstructionProposal::Point {
                point: new([1.0, 2.0]),
            },
            curve_roles: Vec::new(),
            relations: Vec::new(),
        },
        V::Segment => ConstructionCommitPlan {
            proposal: ConstructionProposal::Line {
                start: new([0.0, 0.0]),
                end: new([2.0, 0.0]),
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::Polyline => ConstructionCommitPlan {
            proposal: ConstructionProposal::PolylinePath {
                points: vec![new([0.0, 0.0]), new([2.0, 0.0]), new([2.0, 2.0])],
                closed: false,
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::MidpointLine => ConstructionCommitPlan {
            proposal: ConstructionProposal::MidpointLine {
                center: new([0.0, 0.0]),
                endpoint: new([2.0, 0.0]),
                opposite: new([-2.0, 0.0]),
            },
            curve_roles: roles(1),
            relations: vec![ConstructionRelationDefinition::recipe_intrinsic(
                InferredRelation::Midpoint {
                    point: DraftPointSlot::Created { point_index: 0 },
                    line: created_span(0, 0),
                },
            )],
        },
        V::TwoPointAlignedRectangle => ConstructionCommitPlan {
            proposal: ConstructionProposal::RectangleLoop {
                points: vec![
                    new([0.0, 0.0]),
                    new([4.0, 2.0]),
                    new([4.0, 0.0]),
                    new([0.0, 2.0]),
                ],
                corners: [0, 2, 1, 3],
                center: None,
            },
            curve_roles: roles(4),
            relations: rectangle_intrinsics(variant),
        },
        V::ThreePointCornerRectangle => ConstructionCommitPlan {
            proposal: ConstructionProposal::RectangleLoop {
                points: vec![
                    new([0.0, 0.0]),
                    new([4.0, 0.0]),
                    new([4.0, 2.0]),
                    new([0.0, 2.0]),
                ],
                corners: [0, 1, 2, 3],
                center: None,
            },
            curve_roles: roles(4),
            relations: rectangle_intrinsics(variant),
        },
        V::CenterRectangle | V::ThreePointCenterRectangle => ConstructionCommitPlan {
            proposal: ConstructionProposal::RectangleLoop {
                points: vec![
                    new([0.0, 0.0]),
                    new([2.0, 1.0]),
                    new([-2.0, 1.0]),
                    new([-2.0, -1.0]),
                    new([2.0, -1.0]),
                ],
                corners: [1, 2, 3, 4],
                center: Some(0),
            },
            curve_roles: vec![
                GeometryRole::Profile,
                GeometryRole::Profile,
                GeometryRole::Profile,
                GeometryRole::Profile,
                GeometryRole::Construction,
            ],
            relations: rectangle_intrinsics(variant),
        },
        V::CenterRadiusCircle | V::TwoPointDiameterCircle | V::ThreePointCircle => {
            ConstructionCommitPlan {
                proposal: ConstructionProposal::Circle {
                    center: new([0.0, 0.0]),
                    radius: 2.0,
                },
                curve_roles: roles(1),
                relations: Vec::new(),
            }
        }
        V::CenterArc | V::ThreePointArc => ConstructionCommitPlan {
            proposal: ConstructionProposal::CircularArc {
                center: new([0.0, 0.0]),
                start: [2.0, 0.0],
                end: [0.0, 2.0],
                sweep: DocumentArcSweep::CounterClockwise,
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::TangentArc => {
            let source = tangent_source.expect("Tangent Arc requires an accepted source span");
            ConstructionCommitPlan {
                proposal: ConstructionProposal::CircularArc {
                    center: new([2.0, 1.0]),
                    start: [2.0, 0.0],
                    end: [3.0, 1.0],
                    sweep: DocumentArcSweep::CounterClockwise,
                },
                curve_roles: roles(1),
                relations: vec![ConstructionRelationDefinition::recipe_intrinsic(
                    InferredRelation::CurveCurveTangency {
                        first: DraftContactDescriptor {
                            span: DraftSpanSlot::Existing(source),
                            domain: ContactDomain::Bounded {
                                lower: 0.0,
                                upper: 1.0,
                            },
                            parameter: 1.0,
                            winding: 0,
                            neighborhood: ContactNeighborhood::End,
                        },
                        second: DraftContactDescriptor {
                            span: created_span(0, 0),
                            domain: ContactDomain::Bounded {
                                lower: 0.0,
                                upper: 1.0,
                            },
                            parameter: 0.0,
                            winding: 0,
                            neighborhood: ContactNeighborhood::Start,
                        },
                        orientation: TangentOrientation::Aligned,
                    },
                )],
            }
        }
        V::CenterAxesEllipse => ConstructionCommitPlan {
            proposal: ConstructionProposal::Ellipse {
                center: new([0.0, 0.0]),
                major_axis_point: new([2.0, 0.0]),
                minor_axis_ratio: 0.5,
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::AxisEndpointsEllipse => ConstructionCommitPlan {
            proposal: ConstructionProposal::AxisEndpointEllipse {
                major_axis_point: new([2.0, 0.0]),
                center: new([0.0, 0.0]),
                minor_axis_ratio: 0.5,
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::CenterAxesEllipticalArc => ConstructionCommitPlan {
            proposal: ConstructionProposal::EllipticalArc {
                center: new([0.0, 0.0]),
                major_axis_point: new([2.0, 0.0]),
                minor_axis_ratio: 0.5,
                start_angle: 0.0,
                end_angle: std::f64::consts::FRAC_PI_2,
                sweep: DocumentArcSweep::CounterClockwise,
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::AxisEndpointsEllipticalArc => ConstructionCommitPlan {
            proposal: ConstructionProposal::AxisEndpointEllipticalArc {
                major_axis_point: new([2.0, 0.0]),
                center: new([0.0, 0.0]),
                minor_axis_ratio: 0.5,
                start_angle: 0.0,
                end_angle: std::f64::consts::FRAC_PI_2,
                sweep: DocumentArcSweep::CounterClockwise,
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::QuadraticBezier => ConstructionCommitPlan {
            proposal: ConstructionProposal::QuadraticBezier {
                controls: [new([0.0, 0.0]), new([1.0, 1.0]), new([2.0, 0.0])],
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::CubicBezier => ConstructionCommitPlan {
            proposal: ConstructionProposal::CubicBezier {
                controls: [
                    new([0.0, 0.0]),
                    new([1.0, 1.0]),
                    new([2.0, 1.0]),
                    new([3.0, 0.0]),
                ],
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::RationalQuadraticConic => ConstructionCommitPlan {
            proposal: ConstructionProposal::RationalQuadraticConic {
                start: new([0.0, 0.0]),
                weighted_middle: [1.0, 1.0],
                middle_weight: 1.0,
                end: new([2.0, 0.0]),
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::Parabola => ConstructionCommitPlan {
            proposal: ConstructionProposal::Parabola {
                vertex: new([0.0, 0.0]),
                focus: new([0.0, 1.0]),
                trim_start: -1.0,
                trim_end: 1.0,
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::Hyperbola => ConstructionCommitPlan {
            proposal: ConstructionProposal::Hyperbola {
                center: new([0.0, 0.0]),
                transverse_axis_point: new([2.0, 0.0]),
                semi_conjugate: 1.0,
                branch: DocumentHyperbolaBranch::Positive,
                trim_start: -1.0,
                trim_end: 1.0,
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::OpenControlNurbs => ConstructionCommitPlan {
            proposal: ConstructionProposal::Nurbs {
                controls: vec![
                    new([0.0, 0.0]),
                    new([1.0, 1.0]),
                    new([2.0, -1.0]),
                    new([3.0, 1.0]),
                    new([4.0, 0.0]),
                ],
                options: NurbsConstructionOptions {
                    form: DocumentBSplineForm::Clamped,
                    degree: 3,
                    weights: Vec::new(),
                    gauge_index: 0,
                },
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        V::PeriodicControlNurbs => ConstructionCommitPlan {
            proposal: ConstructionProposal::Nurbs {
                controls: vec![
                    new([0.0, 0.0]),
                    new([1.0, 1.0]),
                    new([2.0, 0.0]),
                    new([1.0, -1.0]),
                ],
                options: NurbsConstructionOptions {
                    form: DocumentBSplineForm::Periodic,
                    degree: 2,
                    weights: Vec::new(),
                    gauge_index: 0,
                },
            },
            curve_roles: roles(1),
            relations: Vec::new(),
        },
        _ => panic!("unrecognized geometry recipe"),
    }
}

fn expected_recipe(variant: GeometryToolVariant) -> GeometryRecipeKind {
    use GeometryRecipeKind as R;
    use GeometryToolVariant as V;
    match variant {
        V::SketchPoint => R::SketchPoint,
        V::Segment => R::Segment,
        V::Polyline => R::Polyline,
        V::MidpointLine => R::MidpointLine,
        V::TwoPointAlignedRectangle => R::TwoPointAlignedRectangle,
        V::ThreePointCornerRectangle => R::ThreePointCornerRectangle,
        V::CenterRectangle => R::CenterRectangle,
        V::ThreePointCenterRectangle => R::ThreePointCenterRectangle,
        V::CenterRadiusCircle => R::CenterRadiusCircle,
        V::TwoPointDiameterCircle => R::TwoPointDiameterCircle,
        V::ThreePointCircle => R::ThreePointCircle,
        V::CenterArc => R::CenterArc,
        V::ThreePointArc => R::ThreePointArc,
        V::TangentArc => R::TangentArc,
        V::CenterAxesEllipse => R::CenterAxesEllipse,
        V::AxisEndpointsEllipse => R::AxisEndpointsEllipse,
        V::CenterAxesEllipticalArc => R::CenterAxesEllipticalArc,
        V::AxisEndpointsEllipticalArc => R::AxisEndpointsEllipticalArc,
        V::QuadraticBezier => R::QuadraticBezier,
        V::CubicBezier => R::CubicBezier,
        V::RationalQuadraticConic => R::RationalQuadraticConic,
        V::Parabola => R::Parabola,
        V::Hyperbola => R::Hyperbola,
        V::OpenControlNurbs => R::OpenControlNurbs,
        V::PeriodicControlNurbs => R::PeriodicControlNurbs,
        _ => panic!("unrecognized geometry recipe"),
    }
}

fn proposal_new_point_count(proposal: &ConstructionProposal) -> usize {
    let count = |points: &[ConstructionPoint]| {
        points
            .iter()
            .filter(|point| matches!(point, ConstructionPoint::New(_)))
            .count()
    };
    match proposal {
        ConstructionProposal::Point { point } => count(std::slice::from_ref(point)),
        ConstructionProposal::Line { start, end }
        | ConstructionProposal::RationalQuadraticConic { start, end, .. } => count(&[*start, *end]),
        ConstructionProposal::Polyline { points }
        | ConstructionProposal::PolylinePath { points, .. }
        | ConstructionProposal::RectangleLoop { points, .. }
        | ConstructionProposal::Nurbs {
            controls: points, ..
        } => count(points),
        ConstructionProposal::MidpointLine {
            center,
            endpoint,
            opposite,
        } => count(&[*center, *endpoint, *opposite]),
        ConstructionProposal::Circle { center, .. }
        | ConstructionProposal::CounterClockwiseArc { center, .. }
        | ConstructionProposal::CircularArc { center, .. } => count(std::slice::from_ref(center)),
        ConstructionProposal::QuadraticBezier { controls } => count(controls),
        ConstructionProposal::CubicBezier { controls } => count(controls),
        ConstructionProposal::Ellipse {
            center,
            major_axis_point,
            ..
        }
        | ConstructionProposal::EllipticalArc {
            center,
            major_axis_point,
            ..
        } => count(&[*center, *major_axis_point]),
        ConstructionProposal::AxisEndpointEllipse {
            major_axis_point,
            center,
            ..
        }
        | ConstructionProposal::AxisEndpointEllipticalArc {
            major_axis_point,
            center,
            ..
        } => count(&[*major_axis_point, *center]),
        ConstructionProposal::Parabola { vertex, focus, .. } => count(&[*vertex, *focus]),
        ConstructionProposal::Hyperbola {
            center,
            transverse_axis_point,
            ..
        } => count(&[*center, *transverse_axis_point]),
        ConstructionProposal::Rectangle { .. } => 4,
    }
}

const fn embedded_constraint_count(variant: GeometryToolVariant) -> usize {
    use GeometryToolVariant as V;
    match variant {
        V::MidpointLine | V::TangentArc => 1,
        V::TwoPointAlignedRectangle | V::ThreePointCenterRectangle => 4,
        V::ThreePointCornerRectangle => 3,
        V::CenterRectangle => 5,
        _ => 0,
    }
}

fn accepted_document(
    coordinator: &ProjectionalIntentCoordinator,
) -> &geosolve_sketch::SketchDocument {
    coordinator
        .accepted_materialization()
        .unwrap()
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document()
}

fn assert_validation(coordinator: &ProjectionalIntentCoordinator) {
    let accepted = coordinator.accepted_materialization().unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    assert!(
        accepted_document(coordinator)
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite)
    );
    assert!(
        accepted_document(coordinator)
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
}

#[test]
fn all_25_geometry_recipes_lower_as_one_validated_patch_each() {
    let mut coordinator = coordinator(0x8300_7101);
    let _seed = seed_point(&mut coordinator, [-20.0, -20.0]);
    let mut tangent_source = None;

    for variant in GeometryToolVariant::ALL {
        let plan = recipe_plan(variant, tangent_source);
        let before_points = accepted_document(&coordinator).points().len();
        let before_curves = accepted_document(&coordinator).curves().len();
        let before_constraints = accepted_document(&coordinator).constraints().len();
        let before_history = coordinator.intent().history_projection().applied.len();
        let translated = projectional_construction_patch(
            coordinator.intent().identity(),
            coordinator.intent(),
            &coordinator.accepted_materialization().unwrap().ownership,
            variant,
            &plan,
        )
        .unwrap_or_else(|error| panic!("{variant:?} did not translate: {error}"));

        assert_eq!(translated.patch.policy, IntentPatchPolicy::RequireAccepted);
        assert_eq!(
            translated.patch.operations().len(),
            1,
            "recipe-owned relations must remain inside {variant:?}"
        );
        let alias = translated.geometry_alias.clone();
        let outcome = coordinator
            .apply_patch(translated.patch)
            .unwrap_or_else(|error| panic!("{variant:?} did not materialize: {error}"));
        assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
        assert_eq!(
            coordinator.intent().history_projection().applied.len(),
            before_history + 1,
            "{variant:?} must publish one history transaction"
        );

        let node = outcome.aliases.node(&alias).unwrap();
        assert_eq!(
            coordinator.intent().graph().node(node).unwrap().kind,
            IntentNodeKind::Geometry {
                recipe: expected_recipe(variant),
            }
        );
        assert_eq!(
            accepted_document(&coordinator).points().len(),
            before_points + proposal_new_point_count(&plan.proposal),
            "wrong stored-point inventory for {variant:?}"
        );
        assert_eq!(
            accepted_document(&coordinator).curves().len(),
            before_curves + plan.curve_roles.len(),
            "wrong curve inventory for {variant:?}"
        );
        assert_eq!(
            accepted_document(&coordinator).constraints().len(),
            before_constraints + embedded_constraint_count(variant),
            "wrong intrinsic relation inventory for {variant:?}"
        );
        assert_validation(&coordinator);

        if variant == GeometryToolVariant::Segment {
            let port = outcome
                .aliases
                .port(&alias, selector(IntentPortRole::Span, 0))
                .unwrap();
            let IntentNativeBinding::CurveSpan(span) = coordinator
                .accepted_materialization()
                .unwrap()
                .ownership
                .port(port)
                .unwrap()
            else {
                panic!("Segment span output must bind one native span");
            };
            tangent_source = Some(span);
        }
    }
}

#[test]
fn new_start_existing_end_uses_one_sparse_semantic_alias() {
    let mut coordinator = coordinator(0x8300_7102);
    let existing_end = seed_point(&mut coordinator, [3.0, 4.0]);
    let plan = ConstructionCommitPlan {
        proposal: ConstructionProposal::Line {
            start: new([0.0, 4.0]),
            end: ConstructionPoint::Existing {
                id: existing_end,
                position: [3.0, 4.0],
            },
        },
        curve_roles: roles(1),
        relations: Vec::new(),
    };
    let before_points = accepted_document(&coordinator).points().len();
    let translated = projectional_construction_patch(
        coordinator.intent().identity(),
        coordinator.intent(),
        &coordinator.accepted_materialization().unwrap().ownership,
        GeometryToolVariant::Segment,
        &plan,
    )
    .unwrap();
    let geometry = translated
        .patch
        .operations()
        .iter()
        .find_map(|operation| match operation {
            IntentPatchOperation::CreateNode { alias, draft, .. }
                if alias == &translated.geometry_alias =>
            {
                Some(draft.as_ref())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(geometry.inputs.len(), 1);
    assert!(
        geometry
            .inputs
            .contains_key(&InputSlot::new(InputRole::Point, 1))
    );
    assert!(
        geometry
            .initial_instance
            .contains_key(&selector(IntentPortRole::Start, 0))
    );
    assert!(
        !geometry
            .initial_instance
            .contains_key(&selector(IntentPortRole::End, 0))
    );

    let alias = translated.geometry_alias.clone();
    let outcome = coordinator.apply_patch(translated.patch).unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        accepted_document(&coordinator).points().len(),
        before_points + 1
    );
    let start_port = outcome
        .aliases
        .port(&alias, selector(IntentPortRole::Start, 0))
        .unwrap();
    let end_port = outcome
        .aliases
        .port(&alias, selector(IntentPortRole::End, 0))
        .unwrap();
    let ownership = &coordinator.accepted_materialization().unwrap().ownership;
    let IntentNativeBinding::Point(start) = ownership.port(start_port).unwrap() else {
        panic!("start must bind a native point");
    };
    assert_eq!(
        ownership.port(end_port),
        Some(IntentNativeBinding::Point(existing_end))
    );
    assert_ne!(start, existing_end);
    assert_validation(&coordinator);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one focused span-routing test keeps both variable-cardinality recipes auditable"
)]
fn polyline_and_nurbs_relations_preserve_exact_span_identity() {
    let mut coordinator = coordinator(0x8300_7103);
    let existing_end = seed_point(&mut coordinator, [2.0, 2.0]);

    let polyline = ConstructionCommitPlan {
        proposal: ConstructionProposal::PolylinePath {
            points: vec![
                new([0.0, 0.0]),
                new([2.0, 0.0]),
                ConstructionPoint::Existing {
                    id: existing_end,
                    position: [2.0, 2.0],
                },
            ],
            closed: false,
        },
        curve_roles: roles(1),
        relations: vec![ConstructionRelationDefinition::auto_inference(
            InferredRelation::Vertical {
                line: created_span(0, 1),
            },
        )],
    };
    let translated = projectional_construction_patch(
        coordinator.intent().identity(),
        coordinator.intent(),
        &coordinator.accepted_materialization().unwrap().ownership,
        GeometryToolVariant::Polyline,
        &polyline,
    )
    .unwrap();
    assert_eq!(translated.patch.operations().len(), 2);
    let expected_polyline_span = PatchPortRef::Alias {
        node: translated.geometry_alias.clone(),
        selector: child_selector(1, IntentPortRole::Span),
    };
    let vertical = translated
        .patch
        .operations()
        .iter()
        .find_map(|operation| match operation {
            IntentPatchOperation::CreateNode { draft, .. }
                if draft.kind
                    == (IntentNodeKind::Constraint {
                        constraint: ConstraintKind::Vertical,
                    }) =>
            {
                Some(draft.as_ref())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        vertical.inputs.get(&InputSlot::new(InputRole::Span, 0)),
        Some(&expected_polyline_span)
    );
    let polyline_alias = translated.geometry_alias.clone();
    let outcome = coordinator.apply_patch(translated.patch).unwrap();
    let polyline_span_port = outcome
        .aliases
        .port(&polyline_alias, child_selector(1, IntentPortRole::Span))
        .unwrap();
    let IntentNativeBinding::CurveSpan(polyline_span) = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(polyline_span_port)
        .unwrap()
    else {
        panic!("polyline child span must bind a native span");
    };
    assert_eq!(polyline_span.segment, 1);
    assert!(
        accepted_document(&coordinator)
            .constraints()
            .iter()
            .any(|constraint| {
                matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::Vertical { line } if line == polyline_span
                )
            })
    );

    let nurbs = ConstructionCommitPlan {
        proposal: ConstructionProposal::Nurbs {
            controls: vec![
                new([-2.0, 0.0]),
                new([-1.0, 1.0]),
                new([0.0, -1.0]),
                new([1.0, 1.0]),
                ConstructionPoint::Existing {
                    id: existing_end,
                    position: [2.0, 2.0],
                },
            ],
            options: NurbsConstructionOptions {
                form: DocumentBSplineForm::Clamped,
                degree: 3,
                weights: Vec::new(),
                gauge_index: 0,
            },
        },
        curve_roles: roles(1),
        relations: vec![ConstructionRelationDefinition::auto_inference(
            InferredRelation::PointOnCurve {
                point: DraftPointSlot::Existing(existing_end),
                contact: DraftContactDescriptor {
                    span: created_span(0, 1),
                    domain: ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    },
                    parameter: 1.0,
                    winding: 0,
                    neighborhood: ContactNeighborhood::End,
                },
            },
        )],
    };
    let translated = projectional_construction_patch(
        coordinator.intent().identity(),
        coordinator.intent(),
        &coordinator.accepted_materialization().unwrap().ownership,
        GeometryToolVariant::OpenControlNurbs,
        &nurbs,
    )
    .unwrap();
    let expected_nurbs_span = PatchPortRef::Alias {
        node: translated.geometry_alias.clone(),
        selector: selector(IntentPortRole::Span, 1),
    };
    let point_on_curve = translated
        .patch
        .operations()
        .iter()
        .find_map(|operation| match operation {
            IntentPatchOperation::CreateNode { draft, .. }
                if draft.kind
                    == (IntentNodeKind::Constraint {
                        constraint: ConstraintKind::PointOnCurve,
                    }) =>
            {
                Some(draft.as_ref())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(
        point_on_curve
            .inputs
            .get(&InputSlot::new(InputRole::Span, 0)),
        Some(&expected_nurbs_span)
    );
    let nurbs_alias = translated.geometry_alias.clone();
    let outcome = coordinator.apply_patch(translated.patch).unwrap();
    let nurbs_span_port = outcome
        .aliases
        .port(&nurbs_alias, selector(IntentPortRole::Span, 1))
        .unwrap();
    let IntentNativeBinding::CurveSpan(nurbs_span) = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(nurbs_span_port)
        .unwrap()
    else {
        panic!("NURBS logical span must bind a native semantic span");
    };
    assert_ne!(nurbs_span, polyline_span);
    assert!(
        accepted_document(&coordinator)
            .constraints()
            .iter()
            .any(|constraint| {
                let DocumentConstraintDefinition::PointOnCurve { point, contact } =
                    constraint.definition
                else {
                    return false;
                };
                point == existing_end
                    && accepted_document(&coordinator)
                        .contact(contact)
                        .is_some_and(|contact| contact.curve == nurbs_span)
            })
    );
    assert_validation(&coordinator);
}

#[test]
fn tangent_arc_keeps_contact_branch_intrinsic_and_atomic() {
    let mut coordinator = coordinator(0x8300_7104);
    let _seed = seed_point(&mut coordinator, [-20.0, -20.0]);
    let source_plan = recipe_plan(GeometryToolVariant::Segment, None);
    let translated = projectional_construction_patch(
        coordinator.intent().identity(),
        coordinator.intent(),
        &coordinator.accepted_materialization().unwrap().ownership,
        GeometryToolVariant::Segment,
        &source_plan,
    )
    .unwrap();
    let source_alias = translated.geometry_alias.clone();
    let source_outcome = coordinator.apply_patch(translated.patch).unwrap();
    let source_port = source_outcome
        .aliases
        .port(&source_alias, selector(IntentPortRole::Span, 0))
        .unwrap();
    let IntentNativeBinding::CurveSpan(source) = coordinator
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(source_port)
        .unwrap()
    else {
        panic!("source Segment must bind a span");
    };

    let plan = recipe_plan(GeometryToolVariant::TangentArc, Some(source));
    let before_history = coordinator.intent().history_projection().applied.len();
    let before_constraints = accepted_document(&coordinator).constraints().len();
    let before_contacts = accepted_document(&coordinator).contacts().len();
    let translated = projectional_construction_patch(
        coordinator.intent().identity(),
        coordinator.intent(),
        &coordinator.accepted_materialization().unwrap().ownership,
        GeometryToolVariant::TangentArc,
        &plan,
    )
    .unwrap();
    assert_eq!(translated.patch.operations().len(), 1);
    let geometry = translated
        .patch
        .operations()
        .iter()
        .find_map(|operation| match operation {
            IntentPatchOperation::CreateNode { draft, .. } => Some(draft.as_ref()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        geometry.inputs.get(&InputSlot::new(InputRole::Span, 0)),
        Some(&PatchPortRef::Stable { port: source_port })
    );
    assert!(geometry.fields.iter().any(|(field, value)| {
        field.0.as_str() == "orientation"
            && matches!(value, IntentLiteral::Enum(value) if value.as_str() == "aligned")
    }));

    let outcome = coordinator.apply_patch(translated.patch).unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        coordinator.intent().history_projection().applied.len(),
        before_history + 1
    );
    assert_eq!(
        accepted_document(&coordinator).constraints().len(),
        before_constraints + 1
    );
    assert_eq!(
        accepted_document(&coordinator).contacts().len(),
        before_contacts + 2
    );
    assert!(
        accepted_document(&coordinator)
            .constraints()
            .iter()
            .any(|constraint| {
                matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::CurveCurveTangency { .. }
                )
            })
    );
    assert_validation(&coordinator);
}

#[test]
fn intrinsic_and_regularization_inventory_mismatches_fail_before_patch_creation() {
    let mut coordinator = coordinator(0x8300_7105);
    let _seed = seed_point(&mut coordinator, [-20.0, -20.0]);
    let ownership = &coordinator.accepted_materialization().unwrap().ownership;

    let mut midpoint = recipe_plan(GeometryToolVariant::MidpointLine, None);
    midpoint.relations.clear();
    assert_eq!(
        projectional_construction_patch(
            coordinator.intent().identity(),
            coordinator.intent(),
            ownership,
            GeometryToolVariant::MidpointLine,
            &midpoint,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::IntrinsicRelationMismatch
    );

    let mut rectangle = recipe_plan(GeometryToolVariant::TwoPointAlignedRectangle, None);
    rectangle
        .relations
        .push(ConstructionRelationDefinition::recipe_regularization(
            InferredRelation::EqualLength {
                first: created_span(0, 0),
                second: created_span(2, 0),
            },
        ));
    assert_eq!(
        projectional_construction_patch(
            coordinator.intent().identity(),
            coordinator.intent(),
            ownership,
            GeometryToolVariant::TwoPointAlignedRectangle,
            &rectangle,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::IntrinsicRelationMismatch
    );

    let mut segment = recipe_plan(GeometryToolVariant::Segment, None);
    segment
        .relations
        .push(ConstructionRelationDefinition::recipe_regularization(
            InferredRelation::EqualLength {
                first: created_span(0, 0),
                second: created_span(0, 0),
            },
        ));
    assert_eq!(
        projectional_construction_patch(
            coordinator.intent().identity(),
            coordinator.intent(),
            ownership,
            GeometryToolVariant::Segment,
            &segment,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::IntrinsicRelationMismatch
    );
}

#[test]
fn recipe_direction_shadows_only_its_ambient_subject_in_one_patch() {
    let mut coordinator = coordinator(0x8300_7106);
    let _seed = seed_point(&mut coordinator, [-20.0, -20.0]);
    let mut plan = recipe_plan(GeometryToolVariant::TwoPointAlignedRectangle, None);
    plan.relations.extend([
        ConstructionRelationDefinition::auto_inference(InferredRelation::Vertical {
            line: created_span(0, 0),
        }),
        ConstructionRelationDefinition::auto_inference(InferredRelation::CoincidentWithOrigin {
            point: DraftPointSlot::Created { point_index: 0 },
        }),
    ]);
    let before_history = coordinator.intent().history_projection().applied.len();
    let before_constraints = accepted_document(&coordinator).constraints().len();
    let translated = projectional_construction_patch(
        coordinator.intent().identity(),
        coordinator.intent(),
        &coordinator.accepted_materialization().unwrap().ownership,
        GeometryToolVariant::TwoPointAlignedRectangle,
        &plan,
    )
    .unwrap();
    assert_eq!(
        translated.patch.operations().len(),
        2,
        "one geometry declaration plus the compatible positional relation must survive"
    );
    let peer_kinds = translated
        .patch
        .operations()
        .iter()
        .filter_map(|operation| match operation {
            IntentPatchOperation::CreateNode { draft, .. } => match draft.kind {
                IntentNodeKind::Constraint { constraint } => Some(constraint),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(peer_kinds, vec![ConstraintKind::CoincidentWithOrigin]);

    let outcome = coordinator.apply_patch(translated.patch).unwrap();
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    assert_eq!(
        coordinator.intent().history_projection().applied.len(),
        before_history + 1
    );
    assert_eq!(
        accepted_document(&coordinator).constraints().len(),
        before_constraints + 5,
        "four recipe constraints plus one compatible ambient constraint"
    );
    assert_validation(&coordinator);
}

#[test]
fn malformed_recipe_role_and_created_slot_inputs_are_rejected_without_state_change() {
    let mut coordinator = coordinator(0x8300_7107);
    let _seed = seed_point(&mut coordinator, [-20.0, -20.0]);
    let identity = coordinator.intent().identity();
    let history = coordinator.intent().history_projection();
    let ownership = &coordinator.accepted_materialization().unwrap().ownership;

    let mismatched = recipe_plan(GeometryToolVariant::CenterRadiusCircle, None);
    assert_eq!(
        projectional_construction_patch(
            identity,
            coordinator.intent(),
            ownership,
            GeometryToolVariant::Segment,
            &mismatched,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::RecipeProposalMismatch
    );

    let mut wrong_roles = recipe_plan(GeometryToolVariant::Segment, None);
    wrong_roles.curve_roles.clear();
    assert_eq!(
        projectional_construction_patch(
            identity,
            coordinator.intent(),
            ownership,
            GeometryToolVariant::Segment,
            &wrong_roles,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::CurveRoleMismatch
    );

    let mut invalid_slot = recipe_plan(GeometryToolVariant::Segment, None);
    invalid_slot
        .relations
        .push(ConstructionRelationDefinition::auto_inference(
            InferredRelation::Horizontal {
                line: created_span(0, 7),
            },
        ));
    assert_eq!(
        projectional_construction_patch(
            identity,
            coordinator.intent(),
            ownership,
            GeometryToolVariant::Segment,
            &invalid_slot,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::InvalidCreatedSlot
    );

    assert_eq!(coordinator.intent().identity(), identity);
    assert_eq!(coordinator.intent().history_projection(), history);
    assert_validation(&coordinator);
}

#[test]
fn stale_session_and_foreign_ownership_are_rejected_before_translation() {
    let mut first = coordinator(0x8300_7108);
    let _first_point = seed_point(&mut first, [1.0, 2.0]);
    let mut foreign = coordinator(0x8300_7109);
    let _foreign_point = seed_point(&mut foreign, [3.0, 4.0]);
    let plan = recipe_plan(GeometryToolVariant::Segment, None);

    let stale_identity = foreign.intent().identity();
    assert_eq!(
        projectional_construction_patch(
            stale_identity,
            first.intent(),
            &first.accepted_materialization().unwrap().ownership,
            GeometryToolVariant::Segment,
            &plan,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::StaleIntentIdentity
    );
    assert_eq!(
        projectional_construction_patch(
            first.intent().identity(),
            first.intent(),
            &foreign.accepted_materialization().unwrap().ownership,
            GeometryToolVariant::Segment,
            &plan,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::StaleOwnership
    );

    let mut excessive = plan;
    excessive.relations = (0..=geosolve_constraint_editor::MAX_CONSTRUCTION_PLAN_RELATIONS)
        .map(|_| {
            ConstructionRelationDefinition::auto_inference(InferredRelation::Horizontal {
                line: created_span(0, 0),
            })
        })
        .collect();
    assert_eq!(
        projectional_construction_patch(
            first.intent().identity(),
            first.intent(),
            &first.accepted_materialization().unwrap().ownership,
            GeometryToolVariant::Segment,
            &excessive,
        )
        .unwrap_err(),
        ProjectionalAuthoringError::RelationLimitExceeded
    );
}
