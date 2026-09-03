// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::{
    ConstructionCommitPlan, ConstructionPoint, ConstructionProposal,
    ConstructionRelationDefinition, DraftContactDescriptor, DraftPointSlot, DraftSpanSlot,
    GeometryToolVariant, InferredRelation, NurbsConstructionOptions, ProjectionalEditorSession,
    projectional_construction_patch,
};
use geosolve_sketch::{
    ContactDomain, ContactNeighborhood, CurveSpan, DocumentArcSweep, DocumentBSplineForm,
    DocumentHyperbolaBranch, DocumentId, GeometryRole, PersistentId, TangentOrientation,
};
use geosolve_sketch_code::{
    CODE_AUTHORING_FAMILIES, CodeAuthoringDeclarationKind, CodeProject, CompiledManagedSource,
    EditorBootstrapDeclaration, KeyedReconcileState, ManagedPathSegment, ManagedValue, ProjectKey,
    SemanticSymbol, materialize_code_project_cold, prepare_editor_declaration_insertions,
    required_generated_members,
};
use geosolve_sketch_intent::{GeometryRecipeKind, IntentPlanDisposition, IntentSessionId};

const EMPTY_MANAGED: &str =
    include_str!("../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-empty.json");
const LINE_MANAGED: &str =
    include_str!("../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-segment.json");

fn accepted_empty_code_project() -> (
    CodeProject,
    geosolve_sketch_code::ExpandedCodeProject,
    ProjectionalEditorSession,
) {
    let compiled =
        CompiledManagedSource::from_json(EMPTY_MANAGED).expect("clean empty compiler envelope");
    let project = CodeProject::managed(ProjectKey("named-geometry-roundtrip".into()), compiled)
        .expect("empty managed project");
    let desired = required_generated_members(&project).expect("generated member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("empty reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_f003_0001),
        DocumentId(PersistentId::from_u128(0x89_f003_0001)),
        1.0,
    )
    .expect("accepted empty code project");
    (project, materialized.expansion, materialized.editor)
}

fn accepted_line_code_project() -> (
    CodeProject,
    geosolve_sketch_code::ExpandedCodeProject,
    ProjectionalEditorSession,
) {
    let compiled =
        CompiledManagedSource::from_json(LINE_MANAGED).expect("clean Segment compiler envelope");
    let project = CodeProject::managed(ProjectKey("named-tangent-roundtrip".into()), compiled)
        .expect("line managed project");
    let desired = required_generated_members(&project).expect("line member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("line reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_f003_0002),
        DocumentId(PersistentId::from_u128(0x89_f003_0002)),
        1.0,
    )
    .expect("accepted line code project");
    (project, materialized.expansion, materialized.editor)
}

const fn new(point: [f64; 2]) -> ConstructionPoint {
    ConstructionPoint::New(point)
}

fn profile() -> Vec<GeometryRole> {
    vec![GeometryRole::Profile]
}

const fn created_span(curve_index: usize) -> DraftSpanSlot {
    DraftSpanSlot::Created {
        curve_index,
        segment: 0,
    }
}

fn rectangle_relations(variant: GeometryToolVariant) -> Vec<ConstructionRelationDefinition> {
    use GeometryToolVariant as V;

    let mut relations = match variant {
        V::TwoPointAlignedRectangle | V::CenterRectangle => vec![
            InferredRelation::Horizontal {
                line: created_span(0),
            },
            InferredRelation::Vertical {
                line: created_span(1),
            },
            InferredRelation::Horizontal {
                line: created_span(2),
            },
            InferredRelation::Vertical {
                line: created_span(3),
            },
        ],
        V::ThreePointCornerRectangle | V::ThreePointCenterRectangle => vec![
            InferredRelation::Perpendicular {
                first: created_span(0),
                second: created_span(1),
            },
            InferredRelation::Parallel {
                first: created_span(0),
                second: created_span(2),
            },
            InferredRelation::Parallel {
                first: created_span(1),
                second: created_span(3),
            },
        ],
        _ => panic!("{variant:?} is not a rectangle"),
    };
    if matches!(variant, V::CenterRectangle | V::ThreePointCenterRectangle) {
        relations.push(InferredRelation::Midpoint {
            point: DraftPointSlot::Created { point_index: 0 },
            line: created_span(4),
        });
    }
    relations
        .into_iter()
        .map(ConstructionRelationDefinition::recipe_intrinsic)
        .collect()
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive fixture keeps the closed geometry recipe catalog and its exact authoring plans reviewable together"
)]
fn recipe_plan(variant: GeometryToolVariant) -> ConstructionCommitPlan {
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
                end: new([2.0, 1.0]),
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::Polyline => ConstructionCommitPlan {
            proposal: ConstructionProposal::PolylinePath {
                points: vec![new([0.0, 0.0]), new([2.0, 0.0]), new([2.0, 2.0])],
                closed: false,
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::MidpointLine => ConstructionCommitPlan {
            proposal: ConstructionProposal::MidpointLine {
                center: new([0.0, 0.0]),
                endpoint: new([2.0, 0.0]),
                opposite: new([-2.0, 0.0]),
            },
            curve_roles: profile(),
            relations: vec![ConstructionRelationDefinition::recipe_intrinsic(
                InferredRelation::Midpoint {
                    point: DraftPointSlot::Created { point_index: 0 },
                    line: created_span(0),
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
            curve_roles: vec![GeometryRole::Profile; 4],
            relations: rectangle_relations(variant),
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
            curve_roles: vec![GeometryRole::Profile; 4],
            relations: rectangle_relations(variant),
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
            relations: rectangle_relations(variant),
        },
        V::CenterRadiusCircle | V::TwoPointDiameterCircle | V::ThreePointCircle => {
            ConstructionCommitPlan {
                proposal: ConstructionProposal::Circle {
                    center: new([8.0, 3.0]),
                    radius: 2.5,
                },
                curve_roles: profile(),
                relations: Vec::new(),
            }
        }
        V::CenterArc | V::ThreePointArc => ConstructionCommitPlan {
            proposal: ConstructionProposal::CircularArc {
                center: new([0.0, 0.0]),
                start: [3.0, 0.0],
                end: [0.0, 3.0],
                sweep: DocumentArcSweep::CounterClockwise,
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::TangentArc => panic!("Tangent Arc needs an accepted source span"),
        V::CenterAxesEllipse => ConstructionCommitPlan {
            proposal: ConstructionProposal::Ellipse {
                center: new([0.0, 0.0]),
                major_axis_point: new([3.0, 0.0]),
                minor_axis_ratio: 0.5,
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::AxisEndpointsEllipse => ConstructionCommitPlan {
            proposal: ConstructionProposal::AxisEndpointEllipse {
                major_axis_point: new([3.0, 0.0]),
                center: new([0.0, 0.0]),
                minor_axis_ratio: 0.5,
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::CenterAxesEllipticalArc => ConstructionCommitPlan {
            proposal: ConstructionProposal::EllipticalArc {
                center: new([0.0, 0.0]),
                major_axis_point: new([3.0, 0.0]),
                minor_axis_ratio: 0.5,
                start_angle: 0.0,
                end_angle: std::f64::consts::FRAC_PI_2,
                sweep: DocumentArcSweep::CounterClockwise,
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::AxisEndpointsEllipticalArc => ConstructionCommitPlan {
            proposal: ConstructionProposal::AxisEndpointEllipticalArc {
                major_axis_point: new([3.0, 0.0]),
                center: new([0.0, 0.0]),
                minor_axis_ratio: 0.5,
                start_angle: 0.0,
                end_angle: std::f64::consts::FRAC_PI_2,
                sweep: DocumentArcSweep::CounterClockwise,
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::QuadraticBezier => ConstructionCommitPlan {
            proposal: ConstructionProposal::QuadraticBezier {
                controls: [new([0.0, 0.0]), new([2.0, 4.0]), new([5.0, 0.0])],
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::Parabola => ConstructionCommitPlan {
            proposal: ConstructionProposal::Parabola {
                vertex: new([0.0, 0.0]),
                focus: new([0.0, 1.0]),
                trim_start: -1.0,
                trim_end: 1.0,
            },
            curve_roles: profile(),
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
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::CubicBezier => ConstructionCommitPlan {
            proposal: ConstructionProposal::CubicBezier {
                controls: [
                    new([0.0, 0.0]),
                    new([2.0, 4.0]),
                    new([5.0, 4.0]),
                    new([7.0, 0.0]),
                ],
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::RationalQuadraticConic => ConstructionCommitPlan {
            proposal: ConstructionProposal::RationalQuadraticConic {
                start: new([0.0, 0.0]),
                weighted_middle: [2.5, 3.0],
                middle_weight: 0.75,
                end: new([5.0, 0.0]),
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::OpenControlNurbs => ConstructionCommitPlan {
            proposal: ConstructionProposal::Nurbs {
                controls: vec![
                    new([0.0, 0.0]),
                    new([1.0, 2.0]),
                    new([3.0, -1.0]),
                    new([5.0, 2.0]),
                    new([7.0, 0.0]),
                ],
                options: NurbsConstructionOptions {
                    form: DocumentBSplineForm::Clamped,
                    degree: 3,
                    weights: Vec::new(),
                    gauge_index: 0,
                },
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        V::PeriodicControlNurbs => ConstructionCommitPlan {
            proposal: ConstructionProposal::Nurbs {
                controls: vec![
                    new([0.0, 0.0]),
                    new([2.0, 2.0]),
                    new([4.0, 0.0]),
                    new([2.0, -2.0]),
                ],
                options: NurbsConstructionOptions {
                    form: DocumentBSplineForm::Periodic,
                    degree: 2,
                    weights: Vec::new(),
                    gauge_index: 0,
                },
            },
            curve_roles: profile(),
            relations: Vec::new(),
        },
        _ => panic!("the closed geometry recipe catalog changed at {variant:?}"),
    }
}

const fn recipe_kind(variant: GeometryToolVariant) -> GeometryRecipeKind {
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
        _ => panic!("the closed geometry recipe catalog changed"),
    }
}

fn compact_source(value: &ManagedValue) -> String {
    match value {
        ManagedValue::Null => "null".into(),
        ManagedValue::Bool(value) => value.to_string(),
        ManagedValue::Number(value) => {
            assert!(value.is_finite());
            serde_json::to_string(value).expect("finite number")
        }
        ManagedValue::String(value) => serde_json::to_string(value).expect("string literal"),
        ManagedValue::Unit(value) => format!(
            "{}({})",
            value.unit,
            serde_json::to_string(&value.value).expect("finite unit value")
        ),
        ManagedValue::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(compact_source)
                .collect::<Vec<_>>()
                .join(",")
        ),
        ManagedValue::Object(fields) => format!(
            "{{{}}}",
            fields
                .iter()
                .map(|(name, value)| format!(
                    "{}:{}",
                    serde_json::to_string(name).expect("object key"),
                    compact_source(value)
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        ManagedValue::Reference { declaration, path } => {
            let mut source = declaration.0.clone();
            for segment in &path.0 {
                match segment {
                    ManagedPathSegment::Field(field) => {
                        assert!(
                            field
                                .chars()
                                .enumerate()
                                .all(|(index, character)| character == '_'
                                    || character == '$'
                                    || character.is_ascii_alphanumeric()
                                        && (index != 0 || !character.is_ascii_digit())),
                            "test source needs an identifier-safe field"
                        );
                        source.push('.');
                        source.push_str(field);
                    }
                    ManagedPathSegment::Index(index) => {
                        source.push('[');
                        source.push_str(&index.to_string());
                        source.push(']');
                    }
                    ManagedPathSegment::Member { member } => {
                        source.push('.');
                        source.push_str(member);
                    }
                }
            }
            source
        }
    }
}

fn assert_clean_named_value(value: &ManagedValue, label: GeometryToolVariant) {
    match value {
        ManagedValue::Array(values) => values
            .iter()
            .for_each(|value| assert_clean_named_value(value, label)),
        ManagedValue::Object(fields) => {
            for (name, value) in fields {
                assert!(
                    !matches!(
                        name.as_str(),
                        "fields"
                            | "values"
                            | "results"
                            | "operationOutputs"
                            | "outputs"
                            | "editLens"
                    ),
                    "{label:?} leaked transport property `{name}`",
                );
                assert_clean_named_value(value, label);
            }
        }
        ManagedValue::Null
        | ManagedValue::Bool(_)
        | ManagedValue::Number(_)
        | ManagedValue::String(_)
        | ManagedValue::Unit(_)
        | ManagedValue::Reference { .. } => {}
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one matrix regression applies the same named-source, finite-authority, and transport-exclusion contract to every ordinary geometry recipe"
)]
fn every_canvas_geometry_family_uses_lossless_reverse_source() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let cases = GeometryToolVariant::ALL
        .into_iter()
        .filter(|variant| {
            !matches!(
                variant,
                GeometryToolVariant::Segment
                    | GeometryToolVariant::Polyline
                    | GeometryToolVariant::TangentArc
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(cases.len(), 22, "the closed 25-recipe catalog changed");

    for (index, variant) in cases.into_iter().enumerate() {
        let expected_recipe = recipe_kind(variant);
        let mut candidate = accepted
            .fork_accepted_authority()
            .expect("candidate editor fork");
        let translated = projectional_construction_patch(
            candidate.coordinator().intent().identity(),
            candidate.coordinator().intent(),
            &candidate
                .coordinator()
                .accepted_materialization()
                .expect("accepted empty authority")
                .ownership,
            variant,
            &recipe_plan(variant),
        )
        .unwrap_or_else(|error| panic!("{variant:?} did not translate: {error}"));
        let alias = translated.geometry_alias.clone();
        let outcome = candidate
            .apply_patch(translated.patch)
            .unwrap_or_else(|error| panic!("{variant:?} did not materialize: {error}"));
        assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
        let node_id = outcome
            .aliases
            .node(&alias)
            .unwrap_or_else(|| panic!("{variant:?} has no created geometry node"));
        let symbol = SemanticSymbol(format!("geometry{}", index + 1));
        let insertion = prepare_editor_declaration_insertions(
            &project,
            &expansion,
            &accepted,
            &candidate,
            &[EditorBootstrapDeclaration::new(node_id, symbol.clone())],
        )
        .unwrap_or_else(|error| panic!("{variant:?} did not reverse-project: {error}"));
        let [declaration] = insertion.declarations.as_slice() else {
            panic!("{variant:?} must project as one declaration")
        };
        let family = CODE_AUTHORING_FAMILIES
            .iter()
            .find(|family| {
                family.declaration == CodeAuthoringDeclarationKind::Geometry(expected_recipe)
            })
            .unwrap_or_else(|| panic!("{variant:?} has no named authoring family"));
        assert_eq!(
            declaration.builder_path,
            [family.namespace, family.method],
            "{variant:?}"
        );
        let ManagedValue::Object(_) = &declaration.arguments else {
            panic!("{variant:?} arguments must be one named object")
        };
        assert_clean_named_value(&declaration.arguments, variant);

        let source = format!(
            "const {}=$.{}({},{});",
            declaration.variable,
            declaration.builder_path.join("."),
            serde_json::to_string(&declaration.symbol.0).expect("symbol literal"),
            compact_source(&declaration.arguments),
        );
        for forbidden in [
            ".recipe",
            "editLens",
            "operationOutputs",
            "\"fields\"",
            "\"values\"",
            "\"results\"",
            "\"outputs\"",
        ] {
            assert!(!source.contains(forbidden), "{variant:?}: {source}");
        }
        assert!(
            source.len() <= 1_024,
            "{variant:?} named declaration is unexpectedly large: {} bytes\n{source}",
            source.len(),
        );

        let accepted_geometry = candidate
            .coordinator()
            .accepted_materialization()
            .expect("accepted recipe materialization");
        assert!(
            accepted_geometry.validation.hard_residuals_validated,
            "{variant:?}"
        );
        assert!(
            accepted_geometry.validation.all_active_features_current,
            "{variant:?}"
        );
        assert!(
            accepted_geometry
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9),
            "{variant:?}",
        );
        let document = accepted_geometry.session.design_document();
        assert!(
            document
                .points()
                .iter()
                .flat_map(|point| point.position)
                .all(f64::is_finite),
            "{variant:?}",
        );
        assert!(
            document
                .scalars()
                .iter()
                .all(|scalar| scalar.value.is_finite()),
            "{variant:?}",
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the input-bound Tangent Arc keeps its typed source-contact projection in one focused regression"
)]
fn tangent_arc_keeps_its_named_source_contact() {
    assert_eq!(
        GeometryToolVariant::ALL
            .into_iter()
            .filter(|variant| !matches!(
                variant,
                GeometryToolVariant::Segment | GeometryToolVariant::Polyline
            ))
            .count(),
        23,
        "the closed compact-recipe catalog changed",
    );
    let (project, expansion, accepted) = accepted_line_code_project();
    let source = accepted
        .coordinator()
        .accepted_materialization()
        .expect("accepted line authority")
        .session
        .design_document()
        .curves()[0]
        .id;
    let source = CurveSpan {
        curve: source,
        segment: 0,
    };
    let plan = ConstructionCommitPlan {
        proposal: ConstructionProposal::CircularArc {
            center: new([2.0, 1.0]),
            start: [2.0, 0.0],
            end: [3.0, 1.0],
            sweep: DocumentArcSweep::CounterClockwise,
        },
        curve_roles: profile(),
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
                    span: created_span(0),
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
    };
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("Tangent Arc candidate fork");
    let translated = projectional_construction_patch(
        candidate.coordinator().intent().identity(),
        candidate.coordinator().intent(),
        &candidate
            .coordinator()
            .accepted_materialization()
            .expect("accepted line authority")
            .ownership,
        GeometryToolVariant::TangentArc,
        &plan,
    )
    .expect("Tangent Arc construction patch");
    let alias = translated.geometry_alias.clone();
    let outcome = candidate
        .apply_patch(translated.patch)
        .expect("accepted Tangent Arc");
    let tangent = outcome.aliases.node(&alias).expect("Tangent Arc node");
    let insertion = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            tangent,
            SemanticSymbol("tangent1".into()),
        )],
    )
    .expect("Tangent Arc reverse projection");
    let [declaration] = insertion.declarations.as_slice() else {
        panic!("one Tangent Arc declaration expected")
    };
    assert_eq!(declaration.builder_path, ["geometry", "tangentArc"]);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("Tangent Arc arguments must be named")
    };
    let ManagedValue::Object(source) = &arguments["source"] else {
        panic!("Tangent Arc must retain its typed source")
    };
    assert_eq!(
        source["span"],
        ManagedValue::Reference {
            declaration: SemanticSymbol("base".into()),
            path: geosolve_sketch_code::SemanticOutputPath(vec![ManagedPathSegment::Field(
                "span".into()
            )]),
        },
    );
    assert!(matches!(source["contact"], ManagedValue::Object(_)));
    assert_clean_named_value(&declaration.arguments, GeometryToolVariant::TangentArc);
    let compact = compact_source(&declaration.arguments);
    assert!(
        compact.len() <= 1_024,
        "Tangent Arc grew to {} bytes",
        compact.len()
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the Segment regression keeps named projection and exact authored geometry together"
)]
fn canvas_segment_preserves_its_named_construction_arguments() {
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("candidate editor fork");
    let plan = ConstructionCommitPlan {
        proposal: ConstructionProposal::Line {
            start: new([-2.0, 1.0]),
            end: new([4.0, 3.0]),
        },
        curve_roles: vec![GeometryRole::Construction],
        relations: Vec::new(),
    };
    let translated = projectional_construction_patch(
        candidate.coordinator().intent().identity(),
        candidate.coordinator().intent(),
        &candidate
            .coordinator()
            .accepted_materialization()
            .expect("accepted empty authority")
            .ownership,
        GeometryToolVariant::Segment,
        &plan,
    )
    .expect("construction Segment patch");
    let alias = translated.geometry_alias.clone();
    let outcome = candidate
        .apply_patch(translated.patch)
        .expect("accepted construction Segment");
    let segment = outcome.aliases.node(&alias).expect("Segment node");
    let insertion = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            segment,
            SemanticSymbol("construction1".into()),
        )],
    )
    .expect("construction Segment reverse projection");
    let [declaration] = insertion.declarations.as_slice() else {
        panic!("one construction Segment declaration expected")
    };
    assert_eq!(declaration.builder_path, ["geometry", "segment"]);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("line arguments must be an object")
    };
    assert_eq!(
        arguments["start"],
        ManagedValue::Array(vec![ManagedValue::Number(-2.0), ManagedValue::Number(1.0)])
    );
    assert_eq!(
        arguments["end"],
        ManagedValue::Array(vec![ManagedValue::Number(4.0), ManagedValue::Number(3.0)])
    );
    assert_eq!(
        arguments["role"],
        ManagedValue::String("construction".into())
    );
    assert!(matches!(
        arguments["branchDirection"],
        ManagedValue::Array(_)
    ));
    assert_clean_named_value(&declaration.arguments, GeometryToolVariant::Segment);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exact-native Polyline regression keeps named projection and keyed topology together"
)]
fn exact_native_canvas_polyline_reverse_projects_to_keyed_named_source() {
    assert_eq!(GeometryToolVariant::ALL.len(), 25);
    assert_eq!(GeometryRecipeKind::ALL.len(), 25);
    let (project, expansion, accepted) = accepted_empty_code_project();
    let mut candidate = accepted
        .fork_accepted_authority()
        .expect("Polyline candidate editor fork");
    let translated = projectional_construction_patch(
        candidate.coordinator().intent().identity(),
        candidate.coordinator().intent(),
        &candidate
            .coordinator()
            .accepted_materialization()
            .expect("accepted empty authority")
            .ownership,
        GeometryToolVariant::Polyline,
        &recipe_plan(GeometryToolVariant::Polyline),
    )
    .expect("exact-native Polyline patch");
    let alias = translated.geometry_alias.clone();
    let outcome = candidate
        .apply_patch(translated.patch)
        .expect("accepted exact-native Polyline");
    assert_eq!(outcome.disposition, IntentPlanDisposition::Accepted);
    let polyline = outcome
        .aliases
        .node(&alias)
        .expect("exact-native Polyline node");
    let insertion = prepare_editor_declaration_insertions(
        &project,
        &expansion,
        &accepted,
        &candidate,
        &[EditorBootstrapDeclaration::new(
            polyline,
            SemanticSymbol("polyline1".into()),
        )],
    )
    .expect("exact-native Polyline reverse projection");
    let [declaration] = insertion.declarations.as_slice() else {
        panic!("one exact-native Polyline declaration expected")
    };
    assert_eq!(declaration.builder_path, ["geometry", "polyline"]);
    let ManagedValue::Object(arguments) = &declaration.arguments else {
        panic!("Polyline arguments must be an object")
    };
    assert!(!arguments.contains_key("representation"));
    assert_eq!(arguments["role"], ManagedValue::String("profile".into()));
    assert_eq!(arguments["closed"], ManagedValue::Bool(false));
    let ManagedValue::Array(vertices) = &arguments["vertices"] else {
        panic!("Polyline vertices must be an array")
    };
    assert_eq!(vertices.len(), 3);
    assert_clean_named_value(&declaration.arguments, GeometryToolVariant::Polyline);
    let direct_source = format!(
        "const {}=$.geometry.polyline({},{});",
        declaration.variable,
        serde_json::to_string(&declaration.symbol.0).expect("Polyline symbol literal"),
        compact_source(&declaration.arguments),
    );
    assert!(
        direct_source.len() <= 1_024,
        "direct Polyline declaration unexpectedly grew to {} bytes",
        direct_source.len(),
    );
}
