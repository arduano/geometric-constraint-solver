// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    BOOTSTRAP_COMPUTED_FEATURE_CODEC_V1, BOOTSTRAP_DOCUMENT_HEADER_CODEC_V1,
    BOOTSTRAP_POINT_CODEC_V1, BOOTSTRAP_SOURCE_ORDER_ENTRY_CODEC_V1, IntentBootstrapError,
    decode_flat_intent_bootstrap, normalize_flat_sketch_intent,
};
use geosolve_sketch::{
    ContactDefinition, ContactDomain, ContactNeighborhood, CurveSpan, DocumentCurveNormalSide,
    DocumentCurveTrimView, DocumentDimensionDefinition, DocumentDimensionMode, DocumentElementId,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentParameterKind,
    DocumentParameterTarget, DocumentTrimBoundary, DocumentTrimParameter, ExternalFeatureKindV1,
    GeometryRole, HostActivationOverride, HostConfigurationActivation, ScalarDomain, ScalarUnit,
    SketchDocument, SketchMaterializationBatch, SketchMaterializationReservationAllocator,
    SketchMaterializationSemanticCatalog,
};
use geosolve_sketch_features::{
    ComputedFeatureAllocatorHighWater, ComputedFeatureCornerId, ComputedFeatureDocument,
    ComputedFeatureId, ComputedFeatureLifecycleHighWater, ComputedFeatureRevision,
    ComputedFilletParent, NativeCurveSpanSource, NewComputedFilletCorner,
};
use geosolve_sketch_intent::{
    BootstrapNativeKind, InputRole, IntentBootstrapObject, IntentEvaluation, IntentKey,
    IntentNodeDraft, IntentNodeKind, IntentPatch, IntentPatchOperation, IntentPatchPolicy,
    IntentSession, IntentSessionId, MaterializationEvidence,
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
