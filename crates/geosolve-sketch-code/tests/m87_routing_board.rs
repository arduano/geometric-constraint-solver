// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{
    ComputedCornerRef, ComputedFeatureDefinition, IntentNativeBinding, ProjectionalEditorSession,
};
use geosolve_sketch::{CurveDefinition, DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeDraftProvenance, CodeInteractionOverlay, CodeProject, CodeProjectDemoId,
    ExpandedCodeProject, ExpandedSemanticTarget, GeneratedMemberAddress, KeyedReconcileState,
    ManagedControl, ManagedControlAccess, ManagedControlConsumerTarget, ManagedControlEdit,
    ManagedControlEditBatch, ManagedControlError, ManagedControlManifest,
    ManagedControlReadOnlyReason, ManagedPathSegment, ManagedValue, MaterializedCodeProject,
    SemanticOutputPath, UnitLiteral, apply_managed_control_batch, bundled_code_project_demos,
    expand_code_project, managed_control_manifest, materialize_code_project_cold,
    materialize_code_project_incremental,
    materialize_code_project_incremental_with_overlay_audited, parse_managed_source,
    required_generated_members,
};
use geosolve_sketch_intent::{
    IntentReservation, IntentSession, IntentSessionId, NodeId, PortId, ReservationId,
};

const SERVICE_SHARED: &str = concat!(
    "const serviceHarness = $.use(\"serviceHarness\", harnessRoute, { ",
    "vertices: serviceRoute.vertices, corners: serviceRoute.filletableCorners, ",
    "clipRadius: sharedClipRadius, bendRadius: sharedBendRadius });"
);
const SERVICE_LOCAL: &str = concat!(
    "const serviceHarness = $.use(\"serviceHarness\", harnessRoute, { ",
    "vertices: serviceRoute.vertices, corners: serviceRoute.filletableCorners, ",
    "clipRadius: mm(2.1), bendRadius: mm(4.25) });"
);
const INSPECTION_CLIP_NEEDLE: &str = concat!(
    "      { key: \"strainReliefB\", position: [118, -86] },\n",
    "      { key: \"sink\", position: [156, -86] },"
);
const INSPECTION_CLIP_REPLACEMENT: &str = concat!(
    "      { key: \"strainReliefB\", position: [118, -86] },\n",
    "      { key: \"inspectionClip\", position: [138, -98] },\n",
    "      { key: \"sink\", position: [156, -86] },"
);
const ROUTING_INVOCATIONS: [&str; 8] = [
    "powerHarness",
    "servoAHarness",
    "servoBHarness",
    "sensorAHarness",
    "sensorBHarness",
    "gripperHarness",
    "visionHarness",
    "serviceHarness",
];

#[derive(Clone, Debug, PartialEq)]
struct DirectNativeIdentity {
    node: NodeId,
    port_ids: BTreeSet<PortId>,
    reservations: BTreeMap<ReservationId, IntentReservation>,
    native: IntentNativeBinding,
}

#[derive(Clone, Debug, PartialEq)]
struct HostNativeIdentity {
    node: NodeId,
    port_ids: BTreeSet<PortId>,
    reservations: BTreeMap<ReservationId, IntentReservation>,
    owner: ComputedCornerRef,
}

fn routing_board_project() -> CodeProject {
    bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::RoboticRoutingBoard)
        .expect("routing-board demo")
        .project()
}

fn expand(project: &CodeProject, seed: u128) -> (KeyedReconcileState, ExpandedCodeProject) {
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).expect("generated inventory"),
            &BTreeSet::new(),
        )
        .expect("generated reconciliation")
        .into_staged();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(seed)).expect("intent session");
    let expansion = expand_code_project(project, &generated, intent.identity())
        .expect("routing-board expansion");
    (generated, expansion)
}

fn control_at<'a>(
    manifest: &'a ManagedControlManifest,
    declaration: &str,
    fields: &[&str],
) -> &'a ManagedControl {
    let path = SemanticOutputPath(
        fields
            .iter()
            .map(|field| ManagedPathSegment::Field((*field).into()))
            .collect(),
    );
    manifest
        .controls
        .iter()
        .find(|control| control.source.declaration.0 == declaration && control.source.path == path)
        .unwrap_or_else(|| panic!("missing managed control {declaration}.{fields:?}"))
}

fn project_with_source(project: &CodeProject, source: &str) -> CodeProject {
    let mut changed = project.clone();
    changed.managed = parse_managed_source(source).expect("localized managed source");
    changed.validate().expect("localized project");
    changed
}

fn unit_edit(control: &ManagedControl, value: f64) -> ManagedControlEdit {
    ManagedControlEdit {
        token: control.token().expect("modifiable control").clone(),
        value: ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value,
        }),
    }
}

fn assert_generated_fan_out(
    control: &ManagedControl,
    count: usize,
    invocation: impl Fn(&str) -> bool,
    template: &str,
) {
    assert_eq!(control.consumers.len(), count);
    assert!(control.consumers.iter().all(|consumer| matches!(
        &consumer.target,
        ManagedControlConsumerTarget::Generated { address, .. }
            if invocation(&address.invocation) && address.template == [template]
    )));
}

fn localized_routing_board_project() -> CodeProject {
    let project = routing_board_project();
    assert_eq!(project.managed.source.matches(SERVICE_SHARED).count(), 1);
    project_with_source(
        &project,
        &project
            .managed
            .source
            .replacen(SERVICE_SHARED, SERVICE_LOCAL, 1),
    )
}

fn shared_local_edited_routing_board_project() -> CodeProject {
    let localized = localized_routing_board_project();
    let (_, expansion) = expand(&localized, 0x87_8820);
    let manifest = managed_control_manifest(&localized, &expansion)
        .expect("localized routing-board managed controls");
    let batch = ManagedControlEditBatch::new([
        unit_edit(control_at(&manifest, "sharedClipRadius", &[]), 2.6),
        unit_edit(control_at(&manifest, "sharedBendRadius", &[]), 4.5),
        unit_edit(
            control_at(&manifest, "serviceHarness", &["clipRadius"]),
            2.2,
        ),
        unit_edit(
            control_at(&manifest, "serviceHarness", &["bendRadius"]),
            4.0,
        ),
    ]);
    apply_managed_control_batch(&localized, &expansion, &batch)
        .expect("atomic shared/local routing-board edit")
}

fn routing_board_reconciliation(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).expect("routing-board generated inventory"),
            &BTreeSet::new(),
        )
        .expect("routing-board generated reconciliation")
        .into_staged()
}

fn cold_materialize(
    project: &CodeProject,
    reconciliation: &KeyedReconcileState,
    seed: u128,
) -> MaterializedCodeProject {
    materialize_code_project_cold(
        project,
        reconciliation,
        IntentSessionId::from_raw(seed),
        DocumentId(PersistentId::from_u128(seed)),
        1.0,
    )
    .unwrap_or_else(|error| panic!("routing-board cold materialization failed: {error:#?}"))
}

fn with_inspection_clip(project: &CodeProject) -> CodeProject {
    assert_eq!(
        project
            .managed
            .source
            .matches(INSPECTION_CLIP_NEEDLE)
            .count(),
        1
    );
    project_with_source(
        project,
        &project
            .managed
            .source
            .replacen(INSPECTION_CLIP_NEEDLE, INSPECTION_CLIP_REPLACEMENT, 1),
    )
}

fn inspection_clip_addresses() -> BTreeSet<GeneratedMemberAddress> {
    BTreeSet::from([
        GeneratedMemberAddress::new(
            "serviceRoute",
            ["polyline", "vertex"],
            ["inspectionClip"],
            ["point"],
        ),
        GeneratedMemberAddress::new(
            "serviceRoute",
            ["polyline", "segment"],
            ["inspectionClip"],
            ["span"],
        ),
        GeneratedMemberAddress::new("serviceHarness", ["circle"], ["inspectionClip"], ["circle"]),
        GeneratedMemberAddress::new("serviceHarness", ["fillet"], ["inspectionClip"], ["arc"]),
    ])
}

fn service_predecessor_segment_address() -> GeneratedMemberAddress {
    GeneratedMemberAddress::new(
        "serviceRoute",
        ["polyline", "segment"],
        ["strainReliefB"],
        ["span"],
    )
}

fn direct_native_identities(
    materialized: &MaterializedCodeProject,
) -> BTreeMap<GeneratedMemberAddress, DirectNativeIdentity> {
    let intent = materialized.editor.coordinator().intent();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted routing-board materialization");
    materialized
        .expansion
        .generated_provenance
        .iter()
        .filter_map(|(address, provenance)| {
            let ExpandedSemanticTarget::Port { port } = &provenance.target else {
                return None;
            };
            let node = intent
                .graph()
                .node_by_symbol(&port.alias)
                .expect("generated routing-board node");
            let semantic_port = node
                .port_by_selector(port.selector)
                .expect("generated routing-board port")
                .as_ref(node.id);
            let native = accepted
                .ownership
                .port(semantic_port)
                .expect("generated routing-board native owner");
            Some((
                address.clone(),
                DirectNativeIdentity {
                    node: node.id,
                    port_ids: node.ports.keys().copied().collect(),
                    reservations: node.reservations.clone(),
                    native,
                },
            ))
        })
        .collect()
}

fn host_native_identities(
    materialized: &MaterializedCodeProject,
) -> BTreeMap<(GeneratedMemberAddress, Vec<String>), HostNativeIdentity> {
    let intent = materialized.editor.coordinator().intent();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted routing-board materialization");
    materialized
        .host_outputs
        .iter()
        .flat_map(|(address, outputs)| {
            outputs.iter().map(|output| {
                let node = accepted
                    .ownership
                    .exact_owner(IntentNativeBinding::ComputedFeature(output.owner.feature))
                    .expect("generated routing-board Fillet owner");
                let node = intent
                    .graph()
                    .node(node)
                    .expect("generated routing-board Fillet node");
                (
                    (address.clone(), output.member_key.clone()),
                    HostNativeIdentity {
                        node: node.id,
                        port_ids: node.ports.keys().copied().collect(),
                        reservations: node.reservations.clone(),
                        owner: output.owner,
                    },
                )
            })
        })
        .collect()
}

fn generated_clip_radius_partitions(
    materialized: &MaterializedCodeProject,
) -> (Vec<f64>, Vec<f64>) {
    let intent = materialized.editor.coordinator().intent();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted routing-board materialization");
    let document = accepted.session.design_document();
    let mut shared = Vec::new();
    let mut service = Vec::new();
    for (address, provenance) in &materialized.expansion.generated_provenance {
        if address.template != ["circle"] {
            continue;
        }
        let ExpandedSemanticTarget::Port { port } = &provenance.target else {
            panic!("generated harness clip must lower to one native curve port")
        };
        let node = intent
            .graph()
            .node_by_symbol(&port.alias)
            .expect("generated clip node");
        let semantic_port = node
            .port_by_selector(port.selector)
            .expect("generated clip port")
            .as_ref(node.id);
        let Some(IntentNativeBinding::Curve(curve)) = accepted.ownership.port(semantic_port) else {
            panic!("generated harness clip must own one native circle")
        };
        let CurveDefinition::Circle { radius, .. } = document
            .curve(curve)
            .expect("generated clip curve")
            .definition
        else {
            panic!("generated harness clip native curve must be a circle")
        };
        let radius = document
            .scalar(radius)
            .expect("generated clip radius")
            .value;
        if address.invocation == "serviceHarness" {
            service.push(radius);
        } else {
            shared.push(radius);
        }
    }
    (shared, service)
}

fn generated_fillet_radius_partitions(
    materialized: &MaterializedCodeProject,
) -> (Vec<f64>, Vec<f64>) {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted routing-board materialization");
    let mut shared = Vec::new();
    let mut service = Vec::new();
    for (address, outputs) in &materialized.host_outputs {
        for output in outputs {
            let ComputedFeatureDefinition::FilletSet(fillet) = &accepted
                .features
                .feature(output.owner.feature)
                .expect("generated routing-board Fillet")
                .definition;
            if address.invocation == "serviceHarness" {
                service.push(fillet.radius);
            } else {
                shared.push(fillet.radius);
            }
        }
    }
    (shared, service)
}

fn assert_exact_radii(values: &[f64], count: usize, expected: f64, label: &str) {
    assert_eq!(values.len(), count, "{label} count");
    assert!(
        values
            .iter()
            .all(|value| value.to_bits() == expected.to_bits()),
        "{label} radii"
    );
}

fn generated_point_position(
    materialized: &MaterializedCodeProject,
    address: &GeneratedMemberAddress,
) -> [f64; 2] {
    let ExpandedSemanticTarget::Port { port } =
        &materialized.expansion.generated_provenance[address].target
    else {
        panic!("generated routing-board point must lower to one native point port")
    };
    let intent = materialized.editor.coordinator().intent();
    let node = intent
        .graph()
        .node_by_symbol(&port.alias)
        .expect("generated routing-board point node");
    let semantic_port = node
        .port_by_selector(port.selector)
        .expect("generated routing-board point port")
        .as_ref(node.id);
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted routing-board materialization");
    let Some(IntentNativeBinding::Point(point)) = accepted.ownership.port(semantic_port) else {
        panic!("generated routing-board point must own one native point")
    };
    accepted
        .session
        .design_document()
        .point(point)
        .expect("generated routing-board native point")
        .position
}

fn assert_history_neutral(materialized: &MaterializedCodeProject) {
    assert_eq!(materialized.editor.coordinator().intent().undo_len(), 0);
    assert_eq!(materialized.editor.coordinator().intent().redo_len(), 0);
    assert_eq!(
        materialized.base_outcome.identity,
        materialized.editor.coordinator().intent().identity()
    );
}

fn assert_valid_native_authority(materialized: &MaterializedCodeProject, feature_count: usize) {
    let intent = materialized.editor.coordinator().intent();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted routing-board materialization");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert_eq!(accepted.validation.feature_count, feature_count);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    intent
        .graph()
        .validate()
        .expect("valid routing-board graph");
    accepted
        .ownership
        .validate_against(
            intent.semantic_identity(),
            intent.graph(),
            intent.instance(),
            intent.reservations(),
            accepted
                .session
                .accepted_state_for_current_input()
                .expect("accepted routing-board solve")
                .document(),
            &accepted.features,
        )
        .expect("valid routing-board native ownership");
}

#[test]
fn routing_board_base_expansion_publishes_accepted_native_authority() {
    let project = routing_board_project();
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("generated inventory"),
            &BTreeSet::new(),
        )
        .expect("generated reconciliation")
        .into_staged();
    let intent =
        IntentSession::with_id(IntentSessionId::from_raw(0x87_8801)).expect("intent session");
    let expansion = expand_code_project(&project, &generated, intent.identity())
        .expect("routing-board expansion");
    let mut editor = ProjectionalEditorSession::restore(
        intent,
        DocumentId(PersistentId::from_u128(0x87_8801)),
        1.0,
    )
    .expect("empty editor");
    let outcome = editor
        .apply_patch(expansion.patch)
        .unwrap_or_else(|error| panic!("routing-board base patch rejected: {error:#?}"));
    assert!(matches!(
        outcome.disposition,
        geosolve_sketch_intent::IntentPlanDisposition::Accepted
    ));
    assert_eq!(
        editor
            .coordinator()
            .intent()
            .graph()
            .nodes()
            .values()
            .filter(|node| matches!(
                node.kind,
                geosolve_sketch_intent::IntentNodeKind::Aggregate {
                    aggregate: geosolve_sketch_intent::AggregateKind::OpenChain,
                }
            ))
            .count(),
        8,
        "every explicit harness route publishes one accepted native open chain",
    );
}

#[test]
fn routing_board_shared_controls_localize_to_one_harness_with_exact_cas_fan_out() {
    let project = routing_board_project();
    let custom_files = project.custom_files.clone();
    let artifacts = project.artifacts.clone();
    let (_, expansion) = expand(&project, 0x87_8810);
    let manifest = managed_control_manifest(&project, &expansion).expect("managed controls");
    let shared_clip = control_at(&manifest, "sharedClipRadius", &[]);
    let shared_bend = control_at(&manifest, "sharedBendRadius", &[]);
    assert_eq!(shared_clip.source.source_text, "mm(2.4)");
    assert_eq!(shared_bend.source.source_text, "mm(5)");
    assert_generated_fan_out(
        shared_clip,
        80,
        |invocation| ROUTING_INVOCATIONS.contains(&invocation),
        "circle",
    );
    assert_generated_fan_out(
        shared_bend,
        64,
        |invocation| ROUTING_INVOCATIONS.contains(&invocation),
        "fillet",
    );
    for invocation in ROUTING_INVOCATIONS {
        for (field, owner) in [
            ("clipRadius", "sharedClipRadius"),
            ("bendRadius", "sharedBendRadius"),
        ] {
            let reference = control_at(&manifest, invocation, &[field]);
            assert!(reference.consumers.is_empty());
            assert!(matches!(
                &reference.access,
                ManagedControlAccess::ReadOnly {
                    reason: ManagedControlReadOnlyReason::Reference,
                    navigation: Some(navigation),
                } if navigation.declaration.0 == owner && navigation.path.0.is_empty()
            ));
        }
    }

    assert_eq!(project.managed.source.matches(SERVICE_SHARED).count(), 1);
    let localized = project_with_source(
        &project,
        &project
            .managed
            .source
            .replacen(SERVICE_SHARED, SERVICE_LOCAL, 1),
    );
    assert_eq!(localized.custom_files, custom_files);
    assert_eq!(localized.artifacts, artifacts);
    let (_, localized_expansion) = expand(&localized, 0x87_8811);
    let localized_manifest = managed_control_manifest(&localized, &localized_expansion)
        .expect("localized managed controls");
    let shared_clip = control_at(&localized_manifest, "sharedClipRadius", &[]);
    let shared_bend = control_at(&localized_manifest, "sharedBendRadius", &[]);
    let service_clip = control_at(&localized_manifest, "serviceHarness", &["clipRadius"]);
    let service_bend = control_at(&localized_manifest, "serviceHarness", &["bendRadius"]);
    assert_generated_fan_out(
        shared_clip,
        70,
        |invocation| invocation != "serviceHarness",
        "circle",
    );
    assert_generated_fan_out(
        shared_bend,
        56,
        |invocation| invocation != "serviceHarness",
        "fillet",
    );
    assert_generated_fan_out(
        service_clip,
        10,
        |invocation| invocation == "serviceHarness",
        "circle",
    );
    assert_generated_fan_out(
        service_bend,
        8,
        |invocation| invocation == "serviceHarness",
        "fillet",
    );

    let stale_batch = ManagedControlEditBatch::new([
        unit_edit(shared_clip, 2.6),
        unit_edit(shared_bend, 4.5),
        unit_edit(service_clip, 2.2),
        unit_edit(service_bend, 4.0),
    ]);
    let edited = apply_managed_control_batch(&localized, &localized_expansion, &stale_batch)
        .expect("atomic shared/local control edit");
    assert_eq!(edited.custom_files, custom_files);
    assert_eq!(edited.artifacts, artifacts);
    for expected in [
        "const sharedClipRadius = mm(2.6);",
        "const sharedBendRadius = mm(4.5);",
        "clipRadius: mm(2.2), bendRadius: mm(4)",
    ] {
        assert!(edited.managed.source.contains(expected), "{expected}");
    }
    let (_, edited_expansion) = expand(&edited, 0x87_8812);
    assert!(matches!(
        apply_managed_control_batch(&edited, &edited_expansion, &stale_batch),
        Err(ManagedControlError::StaleToken(_))
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one routing-board dogfood regression keeps cold controls, overlay locality and the keyed insertion lifecycle under one accepted authority"
)]
fn routing_board_edited_project_cold_overlay_and_inspection_clip_lifecycle_are_exact() {
    let project = shared_local_edited_routing_board_project();
    for expected in [
        "const sharedClipRadius = mm(2.6);",
        "const sharedBendRadius = mm(4.5);",
        "clipRadius: mm(2.2), bendRadius: mm(4)",
    ] {
        assert!(project.managed.source.contains(expected), "{expected}");
    }
    let accepted_source = project.managed.source.clone();
    let accepted_custom_files = project.custom_files.clone();
    let accepted_artifacts = project.artifacts.clone();
    let initial = routing_board_reconciliation(&project);
    assert_eq!(initial.active().len(), 317);
    let before = cold_materialize(&project, &initial, 0x87_8830);
    assert_valid_native_authority(&before, 64);
    assert_eq!(before.host_outputs.len(), 64);

    let (shared_clips, service_clips) = generated_clip_radius_partitions(&before);
    assert_exact_radii(&shared_clips, 70, 2.6, "shared harness clips");
    assert_exact_radii(&service_clips, 10, 2.2, "service harness clips");
    let (shared_fillets, service_fillets) = generated_fillet_radius_partitions(&before);
    assert_exact_radii(&shared_fillets, 56, 4.5, "shared harness Fillets");
    assert_exact_radii(&service_fillets, 8, 4.0, "service harness Fillets");

    let before_direct = direct_native_identities(&before);
    let before_hosts = host_native_identities(&before);
    assert_eq!(before_direct.len(), 232);
    assert_eq!(before_hosts.len(), 64);

    let service_loop_address = GeneratedMemberAddress::new(
        "serviceRoute",
        ["polyline", "vertex"],
        ["serviceLoop"],
        ["point"],
    );
    let ExpandedSemanticTarget::Port {
        port: service_loop_port,
    } = &before.expansion.generated_provenance[&service_loop_address].target
    else {
        panic!("serviceLoop must publish one generated point port")
    };
    let service_loop = before
        .expansion
        .writable_points
        .iter()
        .find(|point| point.handle == *service_loop_port)
        .expect("serviceLoop semantic drag lens")
        .clone();
    assert_eq!(
        service_loop.draft_provenance(),
        CodeDraftProvenance::GeneratedOverride,
    );
    let service_loop_identity = initial.active()[&service_loop_address];
    let first_target = [-28.0, -78.0];
    let overlay = service_loop
        .stage_drag(&CodeInteractionOverlay::empty(), first_target)
        .expect("serviceLoop terminal overlay");
    assert_eq!(overlay.drafts().len(), 1);
    assert!(
        overlay
            .drafts()
            .values()
            .all(|draft| { draft.provenance == CodeDraftProvenance::GeneratedOverride })
    );
    let audited = materialize_code_project_incremental_with_overlay_audited(
        &before, &project, &initial, &overlay,
    );
    assert_eq!(audited.work.managed_parse_attempts(), 0);
    assert_eq!(audited.work.expansion_attempts(), 1);
    assert_eq!(audited.work.accepted_publications(), 0);
    let overlaid = audited
        .outcome
        .expect("serviceLoop incremental overlay materialization");
    assert_eq!(project.managed.source, accepted_source);
    assert_eq!(project.custom_files, accepted_custom_files);
    assert_eq!(project.artifacts, accepted_artifacts);
    assert_eq!(
        overlaid.expansion.generated_provenance[&service_loop_address].identity,
        service_loop_identity,
    );
    assert_eq!(
        generated_point_position(&overlaid, &service_loop_address).map(f64::to_bits),
        first_target.map(f64::to_bits),
    );
    assert_eq!(host_native_identities(&overlaid), before_hosts);
    assert_history_neutral(&overlaid);
    assert_valid_native_authority(&overlaid, 64);

    let second_target = [-28.0, -76.0];
    let second_overlay = service_loop
        .stage_drag(&overlay, second_target)
        .expect("second serviceLoop terminal overlay");
    assert_eq!(second_overlay.drafts().len(), 1);
    assert!(
        second_overlay
            .drafts()
            .values()
            .all(|draft| { draft.provenance == CodeDraftProvenance::GeneratedOverride })
    );
    let second_audited = materialize_code_project_incremental_with_overlay_audited(
        &overlaid,
        &project,
        &initial,
        &second_overlay,
    );
    assert_eq!(second_audited.work.managed_parse_attempts(), 0);
    assert_eq!(second_audited.work.expansion_attempts(), 1);
    assert_eq!(second_audited.work.accepted_publications(), 0);
    let second_overlaid = second_audited
        .outcome
        .expect("second serviceLoop incremental overlay materialization");
    assert_eq!(project.managed.source, accepted_source);
    assert_eq!(
        second_overlaid.expansion.generated_provenance[&service_loop_address].identity,
        service_loop_identity,
    );
    assert_eq!(
        generated_point_position(&second_overlaid, &service_loop_address).map(f64::to_bits),
        second_target.map(f64::to_bits),
    );
    assert_eq!(host_native_identities(&second_overlaid), before_hosts);
    assert_history_neutral(&second_overlaid);
    assert_valid_native_authority(&second_overlaid, 64);
    drop(second_overlaid);
    drop(overlaid);

    let inserted_project = with_inspection_clip(&project);
    assert_eq!(inserted_project.custom_files, accepted_custom_files);
    assert_eq!(inserted_project.artifacts, accepted_artifacts);
    let expected_created = inspection_clip_addresses();
    let insertion_plan = initial
        .plan(
            required_generated_members(&inserted_project)
                .expect("inserted routing-board generated inventory"),
            &BTreeSet::new(),
        )
        .expect("inspectionClip insertion plan");
    assert_eq!(insertion_plan.created.len(), 4);
    assert_eq!(insertion_plan.retained.len(), 317);
    assert!(insertion_plan.removed.is_empty());
    assert_eq!(
        insertion_plan
            .created
            .iter()
            .map(|member| member.address.clone())
            .collect::<BTreeSet<_>>(),
        expected_created,
    );
    let first_created_identities = insertion_plan
        .created
        .iter()
        .map(|member| (member.address.clone(), member.identity))
        .collect::<BTreeMap<_, _>>();
    let inserted_state = insertion_plan.staged().clone();
    let inserted =
        materialize_code_project_incremental(&before, &inserted_project, &inserted_state)
            .expect("incremental inspectionClip insertion");
    assert_valid_native_authority(&inserted, 65);
    assert_history_neutral(&inserted);
    let inserted_direct = direct_native_identities(&inserted);
    let inserted_hosts = host_native_identities(&inserted);
    assert_eq!(inserted_direct.len(), before_direct.len() + 3);
    assert_eq!(inserted_hosts.len(), before_hosts.len() + 1);
    let rebound_segment = service_predecessor_segment_address();
    for (address, identity) in &before_direct {
        if address != &rebound_segment {
            assert_eq!(inserted_direct.get(address), Some(identity), "{address:?}");
        }
    }
    for (key, identity) in &before_hosts {
        assert_eq!(inserted_hosts.get(key), Some(identity), "{key:?}");
    }
    assert!(expected_created.iter().all(|address| {
        if address.template == ["fillet"] {
            inserted_hosts.keys().any(|key| &key.0 == address)
        } else {
            inserted_direct.contains_key(address)
        }
    }));

    let removal_plan = inserted_state
        .plan(
            required_generated_members(&project)
                .expect("restored routing-board generated inventory"),
            &BTreeSet::new(),
        )
        .expect("inspectionClip removal plan");
    assert!(removal_plan.created.is_empty());
    assert_eq!(removal_plan.removed.len(), 4);
    assert_eq!(removal_plan.retained.len(), 317);
    assert_eq!(
        removal_plan
            .removed
            .iter()
            .map(|member| member.address.clone())
            .collect::<BTreeSet<_>>(),
        expected_created,
    );
    let removed_state = removal_plan.staged().clone();
    for address in &expected_created {
        assert_eq!(
            removed_state.tombstones()[address],
            first_created_identities[address],
        );
        assert_eq!(
            removed_state.generation_high_water(address),
            Some(first_created_identities[address].generation),
        );
    }
    let removed = materialize_code_project_incremental(&inserted, &project, &removed_state)
        .expect("incremental inspectionClip removal");
    assert_valid_native_authority(&removed, 64);
    assert_history_neutral(&removed);
    let removed_direct = direct_native_identities(&removed);
    let removed_hosts = host_native_identities(&removed);
    assert_eq!(removed_direct.len(), before_direct.len());
    assert_eq!(removed_hosts, before_hosts);
    for (address, identity) in &before_direct {
        if address != &rebound_segment {
            assert_eq!(removed_direct.get(address), Some(identity), "{address:?}");
        }
    }
    assert!(expected_created.iter().all(|address| {
        !removed_direct.contains_key(address) && !removed_hosts.keys().any(|key| &key.0 == address)
    }));
    drop(inserted);

    let reinsertion_plan = removed_state
        .plan(
            required_generated_members(&inserted_project)
                .expect("reinserted routing-board generated inventory"),
            &BTreeSet::new(),
        )
        .expect("inspectionClip reinsertion plan");
    assert_eq!(reinsertion_plan.created.len(), 4);
    assert_eq!(reinsertion_plan.retained.len(), 317);
    assert!(reinsertion_plan.removed.is_empty());
    assert_eq!(
        reinsertion_plan
            .created
            .iter()
            .map(|member| member.address.clone())
            .collect::<BTreeSet<_>>(),
        expected_created,
    );
    for created in &reinsertion_plan.created {
        let retired = first_created_identities[&created.address];
        assert!(created.identity.allocation > retired.allocation);
        assert!(created.identity.generation > retired.generation);
        assert_eq!(
            reinsertion_plan
                .staged()
                .generation_high_water(&created.address),
            Some(created.identity.generation),
        );
    }
    let reinserted = materialize_code_project_incremental(
        &removed,
        &inserted_project,
        reinsertion_plan.staged(),
    )
    .expect("incremental inspectionClip reinsertion");
    assert_valid_native_authority(&reinserted, 65);
    assert_history_neutral(&reinserted);
    let reinserted_direct = direct_native_identities(&reinserted);
    let reinserted_hosts = host_native_identities(&reinserted);
    assert_eq!(reinserted_direct.len(), removed_direct.len() + 3);
    assert_eq!(reinserted_hosts.len(), removed_hosts.len() + 1);
    for (address, identity) in &removed_direct {
        if address != &rebound_segment {
            assert_eq!(
                reinserted_direct.get(address),
                Some(identity),
                "{address:?}"
            );
        }
    }
    for (key, identity) in &removed_hosts {
        assert_eq!(reinserted_hosts.get(key), Some(identity), "{key:?}");
    }
    for address in &expected_created {
        if address.template == ["fillet"] {
            let first = inserted_hosts
                .iter()
                .find(|(key, _)| &key.0 == address)
                .map(|(_, identity)| identity)
                .expect("first inspectionClip Fillet identity");
            let second = reinserted_hosts
                .iter()
                .find(|(key, _)| &key.0 == address)
                .map(|(_, identity)| identity)
                .expect("reinserted inspectionClip Fillet identity");
            assert_ne!(second, first);
        } else {
            assert_ne!(reinserted_direct.get(address), inserted_direct.get(address));
        }
    }
}
