// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    BOOTSTRAP_COMPUTED_FEATURE_CODEC_V1, BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1,
    BOOTSTRAP_POINT_CODEC_V1, BOOTSTRAP_SOURCE_ORDER_ENTRY_CODEC_V1, FeatureAuthoringOutcome,
    FeatureAuthoringState, FeatureAuthoringTool, IntentBootstrapError, IntentNativeBinding,
    IntentNativeWritableLeaf, PickTolerance, ProjectionalEditorSession, Viewport,
    decode_flat_intent_bootstrap, flat_intent_bootstrap_materialization_map,
    normalize_flat_sketch_intent,
};
use geosolve_sketch::{
    ContactDefinition, ContactDomain, ContactNeighborhood, CurveDefinition, CurveSpan,
    DocumentCurveNormalSide, DocumentCurveTrimView, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentElementId, DocumentFilletEndpointOrder,
    DocumentFilletTrimEndpoint, DocumentParameterKind, DocumentParameterTarget,
    DocumentSolveRequest, DocumentTrimBoundary, DocumentTrimParameter, ExternalFeatureKindV1,
    GeometryRole, HostActivationOverride, HostConfigurationActivation,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
    SketchMaterializationBatch, SketchMaterializationReservationAllocator,
    SketchMaterializationSemanticCatalog, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedFeatureAllocatorHighWater, ComputedFeatureAuthoringSnapshot, ComputedFeatureCornerId,
    ComputedFeatureDefinition, ComputedFeatureDocument, ComputedFeatureId,
    ComputedFeatureLifecycleHighWater, ComputedFeatureRevision, ComputedFilletParent,
    NativeCurveSpanSource, NewComputedFilletCorner,
};
use geosolve_sketch_intent::{
    BootstrapNativeKind, GeometryRecipeKind, InputRole, InputSlot, IntentBootstrapObject,
    IntentEvaluation, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind, IntentPatch,
    IntentPatchOperation, IntentPatchPolicy, IntentPortRole, IntentPortSelector, IntentSession,
    IntentSessionId, IntentUnit, LeafField, MaterializationEvidence, PatchPortRef,
};

fn key(value: &str) -> IntentKey {
    IntentKey::new(value).expect("test key")
}

#[allow(
    clippy::too_many_lines,
    reason = "one complete native inventory fixture keeps bootstrap coverage auditable"
)]
fn fixture() -> (
    SketchDocument,
    ComputedFeatureDocument,
    ComputedFeatureLifecycleHighWater,
) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let rectangle = document
        .add_rectangle("bootstrap rectangle", [0.0, 0.0], 4.0, 3.0)
        .expect("rectangle");
    document
        .set_geometry_role(rectangle.curves[3], GeometryRole::Construction)
        .expect("construction role");
    document
        .replace_trim_views(
            CurveSpan::line(rectangle.curves[2]),
            vec![DocumentCurveTrimView {
                support: CurveSpan::line(rectangle.curves[2]),
                start: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.1,
                    winding: 0,
                }),
                end: DocumentTrimBoundary::Fixed(DocumentTrimParameter {
                    parameter: 0.9,
                    winding: 0,
                }),
            }],
        )
        .expect("trim view");

    let contact_parameter = document
        .add_scalar(
            "contact parameter",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: 0.0,
                upper: 1.0,
            },
        )
        .expect("contact parameter");
    let contact = document
        .add_contact(
            "bootstrap contact",
            ContactDefinition {
                curve: CurveSpan::line(rectangle.curves[0]),
                parameter: contact_parameter,
                domain: ContactDomain::Bounded {
                    lower: 0.0,
                    upper: 1.0,
                },
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                tangent_orientation: None,
            },
        )
        .expect("contact");
    document
        .set_element_user_suppressed(DocumentElementId::Contact(contact), true)
        .expect("user inactivity");

    let input = document
        .add_parameter("width input", DocumentParameterKind::Length)
        .expect("input parameter");
    document
        .add_parameter_binding(
            input,
            DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
        )
        .expect("parameter binding");
    let reference_target = document
        .add_scalar(
            "reference target",
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Finite,
        )
        .expect("reference target");
    let reference = document
        .add_dimension(
            "reference width",
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(rectangle.curves[0]),
                target: reference_target,
            },
            DocumentDimensionMode::Reference,
        )
        .expect("reference dimension");
    let output = document
        .add_parameter("width output", DocumentParameterKind::Length)
        .expect("output parameter");
    document
        .add_parameter_output(output, reference)
        .expect("parameter output");

    let external = document
        .add_external_binding("external point", ExternalFeatureKindV1::Point, None)
        .expect("external binding");
    document
        .set_host_configuration_activation(
            HostConfigurationActivation::new(
                7,
                vec![HostActivationOverride::UnavailableExternalReference(
                    DocumentElementId::ExternalBinding(external),
                )],
            )
            .expect("activation"),
        )
        .expect("host activation");

    let mut allocator =
        SketchMaterializationReservationAllocator::new(document.persistent_identity_high_water())
            .expect("semantic allocator");
    let catalog = allocator
        .reserve_semantic_catalog()
        .expect("semantic catalog");
    let semantic_source = allocator
        .reserve_semantic_source(catalog)
        .expect("semantic source");
    let mut batch =
        SketchMaterializationBatch::new(allocator.finish().expect("semantic reservations"));
    batch.push_semantic_catalog(SketchMaterializationSemanticCatalog {
        catalog,
        sources: vec![semantic_source],
    });
    document
        .apply_materialization_batch(&batch)
        .expect("semantic materialization");
    document.validate().expect("complete flat document");

    let mut features = ComputedFeatureDocument::new(document.id());
    let parent = |curve, retained_endpoint| ComputedFilletParent {
        source: NativeCurveSpanSource {
            span: CurveSpan::line(curve),
        },
        picked_parameter: 0.5,
        winding: 0,
        neighborhood: ContactNeighborhood::Interior,
        normal_side: DocumentCurveNormalSide::Left,
        retained_endpoint,
        periodic_anchor: None,
    };
    features
        .create_fillet_set(
            "bootstrap fillet",
            0.25,
            vec![NewComputedFilletCorner {
                first: parent(rectangle.curves[0], DocumentFilletTrimEndpoint::End),
                second: parent(rectangle.curves[1], DocumentFilletTrimEndpoint::Start),
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
            }],
        )
        .expect("computed feature");
    let current = features.allocator_high_water();
    let lifecycle = ComputedFeatureLifecycleHighWater {
        revision: ComputedFeatureRevision::from_raw(features.revision().raw() + 5),
        allocator: ComputedFeatureAllocatorHighWater {
            next_feature_id: ComputedFeatureId::from_raw(current.next_feature_id.raw() + 7),
            next_corner_id: ComputedFeatureCornerId::from_raw(current.next_corner_id.raw() + 11),
        },
    };
    (document, features, lifecycle)
}

fn accepted_computed_fillet_fixture() -> (
    SketchDocument,
    ComputedFeatureDocument,
    ComputedFeatureLifecycleHighWater,
) {
    let mut document = SketchDocument::new(10.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let corner = document.add_point("corner", [4.0, 0.0]).expect("corner");
    let end = document.add_point("end", [4.0, 4.0]).expect("end");
    let first = document
        .add_curve(
            "first line",
            CurveDefinition::Line {
                start,
                end: corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("first line");
    let second = document
        .add_curve(
            "second line",
            CurveDefinition::Line {
                start: corner,
                end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("second line");
    let second_start = document
        .add_point("second start", [8.0, 0.0])
        .expect("second start");
    let second_corner = document
        .add_point("second corner", [12.0, 0.0])
        .expect("second corner");
    let second_end = document
        .add_point("second end", [12.0, 4.0])
        .expect("second end");
    document
        .add_curve(
            "third line",
            CurveDefinition::Line {
                start: second_start,
                end: second_corner,
                branch_direction: [1.0, 0.0],
            },
        )
        .expect("third line");
    document
        .add_curve(
            "fourth line",
            CurveDefinition::Line {
                start: second_corner,
                end: second_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("fourth line");

    let parent = |curve, retained_endpoint| ComputedFilletParent {
        source: NativeCurveSpanSource {
            span: CurveSpan::line(curve),
        },
        picked_parameter: 0.5,
        winding: 0,
        neighborhood: ContactNeighborhood::Interior,
        normal_side: DocumentCurveNormalSide::Left,
        retained_endpoint,
        periodic_anchor: None,
    };
    let mut features = ComputedFeatureDocument::new(document.id());
    features
        .create_fillet_set(
            "bootstrap fillet",
            0.25,
            vec![NewComputedFilletCorner {
                first: parent(first, DocumentFilletTrimEndpoint::End),
                second: parent(second, DocumentFilletTrimEndpoint::Start),
                endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
                sweep: geosolve_sketch::DocumentArcSweep::CounterClockwise,
            }],
        )
        .expect("computed feature");
    let current = features.allocator_high_water();
    let lifecycle = ComputedFeatureLifecycleHighWater {
        revision: ComputedFeatureRevision::from_raw(features.revision().raw() + 5),
        allocator: ComputedFeatureAllocatorHighWater {
            next_feature_id: ComputedFeatureId::from_raw(current.next_feature_id.raw() + 7),
            next_corner_id: ComputedFeatureCornerId::from_raw(current.next_corner_id.raw() + 11),
        },
    };
    (document, features, lifecycle)
}

#[test]
fn bootstrap_materialization_map_binds_exact_native_ports_and_reservations() {
    let (mut document, features, lifecycle) = fixture();
    let spline_points = [[6.0, 0.0], [7.0, 1.0], [8.0, 0.0]]
        .into_iter()
        .map(|position| document.add_point("spline control", position).unwrap())
        .collect::<Vec<_>>();
    let spline = document
        .add_curve(
            "bootstrap spline",
            CurveDefinition::BSpline {
                form: geosolve_sketch::DocumentBSplineForm::Clamped,
                degree: 2,
                controls: spline_points,
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![41],
                next_span_id: 42,
            },
        )
        .unwrap();
    assert_eq!(document.curve_spans(spline).unwrap()[0].segment, 41);
    let session = normalize_flat_sketch_intent(
        IntentSessionId::from_raw(0x83_b103),
        &document,
        &features,
        lifecycle,
    )
    .unwrap();
    let ownership = flat_intent_bootstrap_materialization_map(&session).unwrap();

    assert_eq!(ownership.semantic, session.semantic_identity());
    assert_eq!(ownership.nodes.len(), session.graph().nodes().len());
    assert_eq!(
        ownership.ports.len(),
        session
            .graph()
            .nodes()
            .values()
            .map(|node| node.ports.len())
            .sum::<usize>()
    );
    assert_eq!(
        ownership.reservations.len(),
        session
            .graph()
            .nodes()
            .values()
            .map(|node| node.reservations.len())
            .sum::<usize>()
    );
    for point in document.points() {
        assert!(
            ownership
                .ports
                .iter()
                .any(|(_, binding)| { *binding == IntentNativeBinding::Point(point.id) })
        );
    }
    for curve in document.curves() {
        assert!(
            ownership
                .ports
                .iter()
                .any(|(_, binding)| { *binding == IntentNativeBinding::Curve(curve.id) })
        );
        let first = document.curve_spans(curve.id).unwrap()[0];
        assert!(
            ownership
                .ports
                .iter()
                .any(|(_, binding)| { *binding == IntentNativeBinding::CurveSpan(first) })
        );
    }
    for constraint in document.constraints() {
        assert!(
            ownership
                .ports
                .iter()
                .any(|(_, binding)| { *binding == IntentNativeBinding::Constraint(constraint.id) })
        );
        assert!(
            ownership.ports.iter().any(|(_, binding)| {
                *binding == IntentNativeBinding::Source(constraint.source_id)
            })
        );
    }
    for dimension in document.dimensions() {
        assert!(
            ownership
                .ports
                .iter()
                .any(|(_, binding)| { *binding == IntentNativeBinding::Dimension(dimension.id) })
        );
        assert!(
            ownership.ports.iter().any(|(_, binding)| {
                *binding == IntentNativeBinding::Source(dimension.source_id)
            })
        );
    }
    for point in document.points() {
        assert!(ownership.writable_leaves.iter().any(|(native, _)| {
            *native
                == geosolve_constraint_editor::IntentNativeWritableLeaf::PointX { point: point.id }
        }));
        assert!(ownership.writable_leaves.iter().any(|(native, _)| {
            *native
                == geosolve_constraint_editor::IntentNativeWritableLeaf::PointY { point: point.id }
        }));
    }
    for scalar in document.scalars() {
        assert!(ownership.writable_leaves.iter().any(|(native, _)| {
            *native
                == geosolve_constraint_editor::IntentNativeWritableLeaf::ScalarValue {
                    scalar: scalar.id,
                }
        }));
    }
}

#[test]
fn mixed_bootstrap_cold_rebuilds_new_declarations_and_edits_historical_free_leaves() {
    let mut document = SketchDocument::new(1.0).expect("document");
    let historical = document
        .add_point("historical point", [0.0, 0.0])
        .expect("historical point");
    let original_high_water = document.persistent_identity_high_water();
    let features = ComputedFeatureDocument::new(document.id());
    let intent = normalize_flat_sketch_intent(
        IntentSessionId::from_raw(0x83_b120),
        &document,
        &features,
        features.lifecycle_high_water(),
    )
    .expect("normalized bootstrap");
    let native = RetainedSketchDocumentSession::new(
        document.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted native bootstrap");
    let mut editor = ProjectionalEditorSession::restore_native_bootstrap(intent, native)
        .expect("projectional bootstrap");

    let historical_port = editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted bootstrap")
        .ownership
        .ports
        .iter()
        .find_map(|(port, binding)| {
            (*binding == IntentNativeBinding::Point(historical)).then_some(*port)
        })
        .expect("historical logical point");
    let end = IntentPortSelector::Node {
        role: IntentPortRole::End,
        index: 0,
    };
    let segment = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::Segment,
        },
        key("post_migration_edge"),
    )
    .with_input(
        InputSlot::new(InputRole::Point, 0),
        PatchPortRef::Stable {
            port: historical_port,
        },
    )
    .with_instance_leaf(
        end,
        LeafField::X,
        IntentLiteral::Quantity {
            value: 4.0,
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        end,
        LeafField::Y,
        IntentLiteral::Quantity {
            value: 1.0,
            unit: IntentUnit::Length,
        },
    );
    editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("post_migration_edge"),
                draft: Box::new(segment),
                cell: None,
            }],
        ))
        .expect("post-migration declaration");

    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .expect("mixed accepted authority");
    assert_eq!(
        accepted.session.design_document().point(historical),
        document.point(historical)
    );
    assert_eq!(accepted.session.design_document().curves().len(), 1);
    assert_ne!(
        accepted.session.persistent_identity_high_water(),
        &original_high_water,
        "new reservations must advance above the historical high-water"
    );

    let canonical = editor
        .coordinator()
        .intent()
        .to_canonical_json()
        .expect("canonical mixed intent");
    let restored_intent = IntentSession::from_json(&canonical).expect("restore mixed intent");
    let restored = ProjectionalEditorSession::restore_with_bootstrap_prefix(restored_intent)
        .expect("cold mixed-bootstrap reconstruction");
    assert_eq!(
        restored
            .coordinator()
            .accepted_materialization()
            .expect("restored accepted authority")
            .session
            .design_document(),
        accepted.session.design_document()
    );

    let x_leaf = editor
        .coordinator()
        .accepted_materialization()
        .expect("mixed ownership")
        .ownership
        .writable_leaf(IntentNativeWritableLeaf::PointX { point: historical })
        .expect("historical point x leaf");
    editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::SetInstanceLeaf {
                leaf: x_leaf,
                value: IntentLiteral::Quantity {
                    value: 2.0,
                    unit: IntentUnit::Length,
                },
            }],
        ))
        .expect("edit historical free leaf");
    assert_eq!(
        editor
            .coordinator()
            .accepted_materialization()
            .expect("edited authority")
            .session
            .design_document()
            .point(historical)
            .expect("historical point")
            .position[0],
        2.0
    );
    assert!(editor.undo().expect("undo historical edit").is_some());
    assert_eq!(
        editor
            .coordinator()
            .accepted_materialization()
            .expect("undo authority")
            .session
            .design_document()
            .point(historical)
            .expect("historical point")
            .position[0],
        0.0
    );
    assert!(editor.redo().expect("redo historical edit").is_some());
}

#[test]
fn mixed_bootstrap_preserves_authenticated_computed_sidecars() {
    let (document, features, lifecycle) = accepted_computed_fillet_fixture();
    let intent = normalize_flat_sketch_intent(
        IntentSessionId::from_raw(0x83_b121),
        &document,
        &features,
        lifecycle,
    )
    .expect("normalized computed bootstrap");
    let native = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("native bootstrap");
    let mut editor = ProjectionalEditorSession::restore_native_bootstrap(intent, native)
        .expect("computed bootstrap authority");
    assert_eq!(
        editor
            .coordinator()
            .accepted_materialization()
            .expect("accepted computed bootstrap")
            .features,
        features
    );

    let point = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::SketchPoint,
        },
        key("post_migration_point"),
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        },
        LeafField::X,
        IntentLiteral::Quantity {
            value: 12.0,
            unit: IntentUnit::Length,
        },
    )
    .with_instance_leaf(
        IntentPortSelector::Node {
            role: IntentPortRole::Primary,
            index: 0,
        },
        LeafField::Y,
        IntentLiteral::Quantity {
            value: 4.0,
            unit: IntentUnit::Length,
        },
    );
    editor
        .apply_patch(IntentPatch::new(
            editor.coordinator().intent().identity(),
            IntentPatchPolicy::RequireAccepted,
            vec![IntentPatchOperation::CreateNode {
                alias: key("post_migration_point"),
                draft: Box::new(point),
                cell: None,
            }],
        ))
        .expect("post-migration point");
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .expect("mixed computed authority");
    assert_eq!(accepted.features, features);
    assert_eq!(accepted.feature_lifecycle_high_water, lifecycle);

    let viewport = Viewport::new([900.0, 600.0], [6.0, 2.0], 35.0).expect("viewport");
    let native = editor
        .coordinator()
        .presentation_session()
        .expect("native presentation session");
    let authoring_snapshot =
        ComputedFeatureAuthoringSnapshot::capture(native).expect("authoring snapshot");
    let scene = editor
        .scene(viewport, 0.5)
        .expect("computed bootstrap scene");
    let mut authoring = FeatureAuthoringState::default();
    assert!(matches!(
        authoring.activate(
            &authoring_snapshot,
            authoring_snapshot.sketch_document(),
            FeatureAuthoringTool::Fillet,
            &[],
        ),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));
    assert!(matches!(
        authoring.pick_at(
            &authoring_snapshot,
            authoring_snapshot.sketch_document(),
            &scene,
            viewport.model_to_screen([11.0, 0.0]),
            PickTolerance::default(),
        ),
        FeatureAuthoringOutcome::Collecting { .. }
    ));
    let candidate = match authoring.pick_at(
        &authoring_snapshot,
        authoring_snapshot.sketch_document(),
        &scene,
        viewport.model_to_screen([12.0, 1.0]),
        PickTolerance::default(),
    ) {
        FeatureAuthoringOutcome::PreviewRequested { candidate, .. }
        | FeatureAuthoringOutcome::Apply(candidate) => candidate,
        other => panic!("expected post-migration Fillet candidate, got {other:?}"),
    };
    editor
        .apply_computed_fillet(key("post migration fillet"), &candidate)
        .expect("author computed Fillet above historical high-water");
    let after_fillet = editor
        .coordinator()
        .accepted_materialization()
        .expect("mixed computed authority");
    assert_eq!(after_fillet.features.features().len(), 2);
    assert_eq!(after_fillet.features.features()[0], features.features()[0]);
    let new_feature = &after_fillet.features.features()[1];
    assert!(
        new_feature.id.raw() >= lifecycle.allocator.next_feature_id.raw(),
        "new feature identity must allocate above the historical lifecycle high-water"
    );
    let ComputedFeatureDefinition::FilletSet(new_fillet) = &new_feature.definition;
    assert!(
        new_fillet.corners[0].id.raw() >= lifecycle.allocator.next_corner_id.raw(),
        "new corner identity must allocate above the historical lifecycle high-water"
    );
    assert!(after_fillet.feature_lifecycle_high_water.revision.raw() > lifecycle.revision.raw());
    let new_feature_id = new_feature.id;
    let new_corner_id = new_fillet.corners[0].id;
    assert!(editor.undo().expect("undo new Fillet").is_some());
    assert_eq!(
        editor
            .coordinator()
            .accepted_materialization()
            .expect("undo computed authority")
            .features,
        features
    );
    assert!(editor.redo().expect("redo new Fillet").is_some());
    let redone = editor
        .coordinator()
        .accepted_materialization()
        .expect("redo computed authority");
    assert_eq!(redone.features.features()[1].id, new_feature_id);
    let ComputedFeatureDefinition::FilletSet(redone_fillet) =
        &redone.features.features()[1].definition;
    assert_eq!(redone_fillet.corners[0].id, new_corner_id);

    let canonical = editor
        .coordinator()
        .intent()
        .to_canonical_json()
        .expect("canonical mixed computed intent");
    let restored = ProjectionalEditorSession::restore_with_bootstrap_prefix(
        IntentSession::from_json(&canonical).expect("restore mixed computed intent"),
    )
    .expect("cold restore mixed computed intent");
    assert_eq!(
        restored
            .coordinator()
            .accepted_materialization()
            .expect("restored computed authority")
            .features,
        redone.features
    );
    assert_eq!(
        restored
            .coordinator()
            .accepted_materialization()
            .expect("restored computed authority")
            .feature_lifecycle_high_water,
        redone.feature_lifecycle_high_water
    );
}

fn normalized() -> IntentSession {
    let (document, features, lifecycle) = fixture();
    normalize_flat_sketch_intent(
        IntentSessionId::from_raw(0x83_b100),
        &document,
        &features,
        lifecycle,
    )
    .expect("normalized bootstrap")
}

fn object_with_codec(session: &IntentSession, codec: &str) -> IntentBootstrapObject {
    session
        .graph()
        .nodes()
        .values()
        .find_map(|node| match &node.kind {
            IntentNodeKind::Bootstrap { object } if object.codec.as_str() == codec => {
                Some(object.clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing bootstrap codec {codec}"))
}

fn session_from_drafts(drafts: Vec<(&str, IntentNodeDraft)>) -> IntentSession {
    let mut session = IntentSession::with_id(IntentSessionId::from_raw(0x83_b200)).unwrap();
    let operations = drafts
        .into_iter()
        .map(|(alias, draft)| IntentPatchOperation::CreateNode {
            alias: key(alias),
            draft: Box::new(draft),
            cell: None,
        })
        .collect();
    let patch = IntentPatch::new(
        session.identity(),
        IntentPatchPolicy::RequireAccepted,
        operations,
    );
    let plan = session
        .plan_patch(patch, |candidate| IntentEvaluation::Accepted {
            evidence: MaterializationEvidence::new_host_artifacts(
                candidate.external_inputs().identity(),
                b"test-flat-materialization".to_vec(),
                b"test-bootstrap-ownership".to_vec(),
                b"test-feature-sidecar".to_vec(),
            )
            .unwrap(),
        })
        .expect("test bootstrap plan");
    session
        .commit_initialization_plan(plan)
        .expect("test bootstrap publication");
    session
}

fn draft(alias: &str, object: IntentBootstrapObject) -> IntentNodeDraft {
    IntentNodeDraft::new(IntentNodeKind::Bootstrap { object }, key(alias))
}

#[test]
fn every_flat_object_and_side_table_round_trips_without_recipe_inference() {
    let (document, features, lifecycle) = fixture();
    let session = normalize_flat_sketch_intent(
        IntentSessionId::from_raw(0x83_b101),
        &document,
        &features,
        lifecycle,
    )
    .expect("normalization");
    let decoded = decode_flat_intent_bootstrap(&session).expect("strict decode");

    assert_eq!(
        decoded.document.to_draft_v5_json().unwrap(),
        document.to_draft_v5_json().unwrap()
    );
    assert_eq!(
        decoded.features.to_json().unwrap(),
        features.to_json().unwrap()
    );
    assert_eq!(decoded.feature_lifecycle_high_water, lifecycle);
    assert_eq!(
        session.undo_len(),
        0,
        "migration must not become user history"
    );
    assert_eq!(session.redo_len(), 0);
    assert!(
        session
            .graph()
            .nodes()
            .values()
            .all(|node| matches!(node.kind, IntentNodeKind::Bootstrap { .. }))
    );
    assert_eq!(
        session
            .graph()
            .nodes()
            .values()
            .filter(|node| matches!(
                &node.kind,
                IntentNodeKind::Bootstrap { object }
                    if object.codec.as_str() == BOOTSTRAP_COMPUTED_FEATURE_CODEC_V1
            ))
            .count(),
        features.features().len(),
        "one persisted native feature must remain one bootstrap node"
    );

    let canonical = session.to_canonical_json().unwrap();
    let restored = IntentSession::from_json(&canonical).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), canonical);
    let restored_flat = decode_flat_intent_bootstrap(&restored).expect("restored strict decode");
    assert_eq!(restored_flat.document, document);
    assert_eq!(restored_flat.features, features);
}

#[test]
fn normalization_is_byte_deterministic_and_every_declaration_has_one_document_root() {
    let (document, features, lifecycle) = fixture();
    let first = normalize_flat_sketch_intent(
        IntentSessionId::from_raw(0x83_b102),
        &document,
        &features,
        lifecycle,
    )
    .unwrap();
    let second = normalize_flat_sketch_intent(first.id(), &document, &features, lifecycle).unwrap();
    assert_eq!(
        first.to_canonical_json().unwrap(),
        second.to_canonical_json().unwrap()
    );

    let root = first
        .graph()
        .nodes()
        .values()
        .find(|node| {
            matches!(
                &node.kind,
                IntentNodeKind::Bootstrap { object }
                    if object.codec.as_str() == BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1
            )
        })
        .expect("document root");
    for node in first.graph().nodes().values() {
        let identity_inputs = node
            .inputs
            .iter()
            .filter(|(slot, _)| slot.role == InputRole::Identity)
            .map(|(_, source)| source.node)
            .collect::<Vec<_>>();
        if node.id == root.id {
            assert!(identity_inputs.is_empty());
        } else {
            assert_eq!(
                identity_inputs,
                [root.id],
                "node {} root membership",
                node.id
            );
        }
    }
}

#[test]
fn strict_decoder_rejects_missing_duplicate_malformed_and_wrong_kind_payloads() {
    let valid = normalized();
    let header = object_with_codec(&valid, BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1);
    let point = object_with_codec(&valid, BOOTSTRAP_POINT_CODEC_V1);

    let missing = session_from_drafts(vec![("point", draft("point", point.clone()))]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&missing),
        Err(IntentBootstrapError::InvalidDocumentHeader)
    ));

    let duplicate = session_from_drafts(vec![
        ("header-a", draft("header-a", header.clone())),
        ("header-b", draft("header-b", header.clone())),
    ]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&duplicate),
        Err(IntentBootstrapError::InvalidDocumentHeader)
    ));

    let duplicate_native = session_from_drafts(vec![
        ("header", draft("header", header.clone())),
        ("point-a", draft("point-a", point.clone())),
        ("point-b", draft("point-b", point.clone())),
    ]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&duplicate_native),
        Err(IntentBootstrapError::Document(_))
    ));

    let mut noncanonical = point.clone();
    noncanonical.payload.push(b'\n');
    let malformed = session_from_drafts(vec![
        ("header", draft("header", header.clone())),
        ("point", draft("point", noncanonical)),
    ]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&malformed),
        Err(IntentBootstrapError::InvalidPayload)
    ));

    let mut wrong_kind = point;
    wrong_kind.kind = BootstrapNativeKind::Curve;
    let mismatched = session_from_drafts(vec![
        ("header", draft("header", header)),
        ("wrong-kind", draft("wrong-kind", wrong_kind)),
    ]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&mismatched),
        Err(IntentBootstrapError::CodecKindMismatch)
    ));
}

#[test]
fn strict_decoder_rejects_unknown_mixed_duplicate_side_table_and_noncanonical_edges() {
    let valid = normalized();
    let header = object_with_codec(&valid, BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1);
    let point = object_with_codec(&valid, BOOTSTRAP_POINT_CODEC_V1);
    let source_order = object_with_codec(&valid, BOOTSTRAP_SOURCE_ORDER_ENTRY_CODEC_V1);

    let mut unknown = point.clone();
    unknown.codec = key("geosolve-bootstrap-unknown-v1");
    let unknown_session = session_from_drafts(vec![
        ("header", draft("header", header.clone())),
        ("unknown", draft("unknown", unknown)),
    ]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&unknown_session),
        Err(IntentBootstrapError::InvalidPayload)
    ));

    let mixed = session_from_drafts(vec![
        ("header", draft("header", header.clone())),
        (
            "annotation",
            IntentNodeDraft::new(IntentNodeKind::Annotation, key("annotation")),
        ),
    ]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&mixed),
        Err(IntentBootstrapError::MixedDeclarationGraph)
    ));

    let duplicate_side_table = session_from_drafts(vec![
        ("header", draft("header", header.clone())),
        ("source-a", draft("source-a", source_order.clone())),
        ("source-b", draft("source-b", source_order)),
    ]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&duplicate_side_table),
        Err(IntentBootstrapError::DuplicateSideTableState)
    ));

    let missing_root_edge = session_from_drafts(vec![
        ("header", draft("header", header)),
        ("point", draft("point", point)),
    ]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&missing_root_edge),
        Err(IntentBootstrapError::NonCanonicalBootstrap)
    ));
}

#[test]
fn bootstrap_codecs_remain_closed_and_do_not_admit_annotation_or_recipe_payloads() {
    let session = normalized();
    let codecs = session
        .graph()
        .nodes()
        .values()
        .filter_map(|node| match &node.kind {
            IntentNodeKind::Bootstrap { object } => Some(object.codec.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(codecs.contains(&BOOTSTRAP_POINT_CODEC_V1));
    assert!(codecs.contains(&BOOTSTRAP_COMPUTED_FEATURE_CODEC_V1));
    assert!(!codecs.iter().any(|codec| codec.contains("rectangle")));
    assert!(!codecs.iter().any(|codec| codec.contains("fillet-set")));

    let object = IntentBootstrapObject::new(
        BootstrapNativeKind::AnnotationPlacement,
        key("geosolve-bootstrap-annotation-placement-v1"),
        b"{}".to_vec(),
    )
    .unwrap();
    let annotation = session_from_drafts(vec![("annotation", draft("annotation", object))]);
    assert!(matches!(
        decode_flat_intent_bootstrap(&annotation),
        Err(IntentBootstrapError::InvalidPayload)
            | Err(IntentBootstrapError::InvalidDocumentHeader)
    ));
}
