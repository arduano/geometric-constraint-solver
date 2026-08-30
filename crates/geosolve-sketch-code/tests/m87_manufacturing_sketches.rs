// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{
    ComputedFeatureDefinition, ComputedFeatureEvaluationState, IntentNativeBinding,
};
use geosolve_sketch::{
    CurveDefinition, DesignPointId, DocumentConstraintDefinition, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentId, PersistentId,
};
use geosolve_sketch_code::{
    CodeHostRequest, CodeProject, CodeProjectDemoId, ExpandedCodeProject, ExpandedSemanticTarget,
    GeneratedMemberAddress, GeneratedMemberIdentity, KeyedReconcileState, ManagedControl,
    ManagedControlConsumerTarget, ManagedControlEdit, ManagedControlEditBatch, ManagedControlError,
    ManagedControlManifest, ManagedControlToken, ManagedPathSegment, ManagedValue,
    MaterializedCodeProject, SemanticOutputPath, UnitLiteral, apply_managed_control_batch,
    bundled_code_project_demos, expand_code_project, managed_control_manifest,
    materialize_code_project_cold, parse_managed_source, required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

type AcceptedInventory = (
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AcceptedRadiusState {
    identity: GeneratedMemberIdentity,
    radius_bits: u64,
}

fn demo_project(id: CodeProjectDemoId) -> CodeProject {
    bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == id)
        .expect("bundled manufacturing demo")
        .project()
}

fn reconciliation(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).expect("generated-member inventory"),
            &BTreeSet::new(),
        )
        .expect("generated-member reconciliation")
        .into_staged()
}

fn expand_project(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    seed: u128,
) -> ExpandedCodeProject {
    let intent = IntentSession::with_id(IntentSessionId::from_raw(seed)).expect("intent session");
    expand_code_project(project, generated, intent.identity()).expect("manufacturing expansion")
}

fn materialize(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    seed: u128,
) -> MaterializedCodeProject {
    materialize_code_project_cold(
        project,
        generated,
        IntentSessionId::from_raw(seed),
        DocumentId(PersistentId::from_u128(seed)),
        1.0,
    )
    .unwrap_or_else(|error| panic!("manufacturing cold materialization failed: {error:#?}"))
}

#[allow(
    clippy::too_many_lines,
    reason = "one oracle keeps accepted validity, zero mobility, and minimal datum invariants together"
)]
fn assert_accepted_inventory(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    materialized: &MaterializedCodeProject,
    expected: AcceptedInventory,
) {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("cold materialization owns accepted authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    let actual = (
        project.managed.program.declarations.len(),
        generated.active().len(),
        materialized.expansion.semantic_outputs.len(),
        accepted.validation.point_count,
        accepted.validation.curve_count,
        accepted.validation.constraint_count,
        materialized.host_outputs.len(),
        accepted.validation.feature_count,
        accepted.validation.computed_edge_count,
    );
    assert_eq!(actual, expected, "accepted manufacturing inventory");

    let accepted_state = accepted
        .session
        .accepted_state_for_current_input()
        .expect("success-like materialization owns its exact accepted input");
    assert!(
        accepted_state
            .document()
            .points()
            .iter()
            .all(|point| { point.position.into_iter().all(f64::is_finite) })
    );
    assert!(
        accepted_state
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    assert_eq!(
        accepted.computed.feature_evaluations().len(),
        accepted.validation.feature_count
    );
    assert!(
        accepted
            .computed
            .feature_evaluations()
            .iter()
            .all(|feature| {
                matches!(
                    feature.state,
                    ComputedFeatureEvaluationState::Current { .. }
                )
            })
    );
    materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .validate()
        .expect("accepted manufacturing graph");

    let diagnostics = accepted_state.diagnostics();
    let rank = diagnostics
        .rank
        .expect("manufacturing rank diagnostics must be available");
    assert_eq!(rank.numerical_left_nullity, Some(0));
    assert_eq!(rank.numerical_right_nullity, Some(0));
    assert_eq!(rank.structural_left_nullity, 0);
    assert_eq!(rank.structural_right_nullity, 0);
    let mobility = diagnostics
        .mobility
        .expect("manufacturing mobility diagnostics must be available");
    assert_eq!(mobility.equality_degrees_of_freedom, Some(0));
    assert_eq!(mobility.bidirectional_bounded_degrees_of_freedom, Some(0));

    let fixed_points = accepted_state
        .document()
        .constraints()
        .iter()
        .filter(|constraint| {
            matches!(
                constraint.definition,
                DocumentConstraintDefinition::FixedPoint { .. }
            )
        })
        .count();
    let fixed_coordinates = accepted_state
        .document()
        .constraints()
        .iter()
        .filter(|constraint| {
            matches!(
                constraint.definition,
                DocumentConstraintDefinition::FixedCoordinate { .. }
            )
        })
        .count();
    assert!(
        fixed_points <= 1,
        "at most one complete point lock is allowed"
    );
    assert!(
        fixed_coordinates <= 1,
        "at most one scalar datum lock is allowed"
    );
    assert_eq!(
        fixed_points + fixed_coordinates,
        1,
        "one minimal absolute datum must anchor the manufacturing sketch"
    );
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

fn assert_unit_value(control: &ManagedControl, expected: f64) {
    assert!(matches!(
        &control.value,
        ManagedValue::Unit(UnitLiteral { unit, value })
            if unit == "mm" && value.to_bits() == expected.to_bits()
    ));
}

fn assert_generated_fan_out(
    control: &ManagedControl,
    count: usize,
    invocations: &[&str],
    template: &str,
) {
    assert_eq!(control.consumers.len(), count);
    let mut actual = BTreeMap::<String, usize>::new();
    for consumer in &control.consumers {
        let ManagedControlConsumerTarget::Generated { address, .. } = &consumer.target else {
            panic!("patch input must route only to generated consumers")
        };
        assert_eq!(address.template, [template]);
        assert!(invocations.contains(&address.invocation.as_str()));
        *actual.entry(address.invocation.clone()).or_default() += 1;
    }
    assert_eq!(actual.keys().len(), invocations.len());
}

fn assert_generated_fan_out_by_template(
    control: &ManagedControl,
    invocations: &[&str],
    expected_templates: &[(&str, usize)],
) {
    assert_eq!(
        control.consumers.len(),
        expected_templates
            .iter()
            .map(|(_, count)| *count)
            .sum::<usize>()
    );
    let mut actual_templates = BTreeMap::<String, usize>::new();
    let mut actual_invocations = BTreeMap::<String, usize>::new();
    for consumer in &control.consumers {
        let ManagedControlConsumerTarget::Generated { address, .. } = &consumer.target else {
            panic!("patch input must route only to generated consumers")
        };
        let [template] = address.template.as_slice() else {
            panic!("manufacturing consumer must own one template path segment")
        };
        assert!(invocations.contains(&address.invocation.as_str()));
        *actual_templates.entry(template.clone()).or_default() += 1;
        *actual_invocations
            .entry(address.invocation.clone())
            .or_default() += 1;
    }
    assert_eq!(
        actual_templates,
        expected_templates
            .iter()
            .map(|(template, count)| ((*template).to_owned(), *count))
            .collect()
    );
    assert_eq!(actual_invocations.keys().len(), invocations.len());
}

fn assert_absolute_datums(
    materialized: &MaterializedCodeProject,
    expected_fixed_points: usize,
    expected_fixed_coordinates: usize,
) {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted manufacturing materialization");
    let document = accepted
        .session
        .accepted_state_for_current_input()
        .expect("accepted manufacturing state")
        .document();
    assert_eq!(
        document
            .constraints()
            .iter()
            .filter(|constraint| matches!(
                constraint.definition,
                DocumentConstraintDefinition::FixedPoint { .. }
            ))
            .count(),
        expected_fixed_points
    );
    assert_eq!(
        document
            .constraints()
            .iter()
            .filter(|constraint| matches!(
                constraint.definition,
                DocumentConstraintDefinition::FixedCoordinate { .. }
            ))
            .count(),
        expected_fixed_coordinates
    );
}

fn assert_driving_radius_dimensions(materialized: &MaterializedCodeProject, expected: usize) {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted manufacturing materialization");
    let document = accepted
        .session
        .accepted_state_for_current_input()
        .expect("accepted manufacturing state")
        .document();
    assert_eq!(
        document
            .dimensions()
            .iter()
            .filter(|dimension| {
                !dimension.suppressed
                    && dimension.mode == DocumentDimensionMode::Driving
                    && matches!(
                        dimension.definition,
                        DocumentDimensionDefinition::Radius { .. }
                    )
            })
            .count(),
        expected
    );
}

fn assert_dimension_count(materialized: &MaterializedCodeProject, expected: usize) {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted manufacturing materialization");
    assert_eq!(
        accepted
            .session
            .accepted_state_for_current_input()
            .expect("accepted manufacturing state")
            .document()
            .dimensions()
            .len(),
        expected
    );
}

fn unit_edit(control: &ManagedControl, value: f64) -> ManagedControlEdit {
    ManagedControlEdit {
        token: control.token().expect("modifiable unit control").clone(),
        value: ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value,
        }),
    }
}

fn project_with_source(project: &CodeProject, source: &str) -> CodeProject {
    let mut changed = project.clone();
    changed.managed = parse_managed_source(source).expect("changed managed source");
    changed.validate().expect("changed manufacturing project");
    changed
}

fn replace_source_once(project: &CodeProject, needle: &str, replacement: &str) -> CodeProject {
    assert_eq!(
        project.managed.source.matches(needle).count(),
        1,
        "{needle}"
    );
    project_with_source(
        project,
        &project.managed.source.replacen(needle, replacement, 1),
    )
}

fn replace_in_declaration(
    project: &CodeProject,
    declaration: &str,
    needle: &str,
    replacement: &str,
) -> CodeProject {
    let mut source = project.managed.source.clone();
    let declaration_start = format!("  const {declaration} = ");
    let start = source
        .find(&declaration_start)
        .unwrap_or_else(|| panic!("missing declaration {declaration}"));
    let terminator = "\n  });";
    let end = start
        + source[start..]
            .find(terminator)
            .unwrap_or_else(|| panic!("unterminated declaration {declaration}"))
        + terminator.len();
    let block = &source[start..end];
    assert_eq!(block.matches(needle).count(), 1);
    let changed = block.replacen(needle, replacement, 1);
    source.replace_range(start..end, &changed);
    project_with_source(project, &source)
}

fn host_fillet_counts(expansion: &ExpandedCodeProject) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for request in &expansion.host_requests {
        let CodeHostRequest::FilletAtCorner(request) = request else {
            continue;
        };
        *counts.entry(request.invocation.0.clone()).or_default() += 1;
    }
    counts
}

fn accepted_circle_radii(
    materialized: &MaterializedCodeProject,
) -> BTreeMap<GeneratedMemberAddress, AcceptedRadiusState> {
    let intent = materialized.editor.coordinator().intent();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted manufacturing materialization");
    let accepted_state = accepted
        .session
        .accepted_state_for_current_input()
        .expect("accepted solved manufacturing state");
    let document = accepted_state.document();
    materialized
        .expansion
        .generated_provenance
        .iter()
        .filter(|(address, _)| address.template == ["circle"])
        .map(|(address, provenance)| {
            let ExpandedSemanticTarget::Port { port } = &provenance.target else {
                panic!("generated relief must lower to one native curve port")
            };
            let node = intent
                .graph()
                .node_by_symbol(&port.alias)
                .expect("generated relief node");
            let semantic_port = node
                .port_by_selector(port.selector)
                .expect("generated relief port")
                .as_ref(node.id);
            let Some(IntentNativeBinding::Curve(curve)) = accepted.ownership.port(semantic_port)
            else {
                panic!("generated relief must own one native circle")
            };
            let CurveDefinition::Circle { radius, .. } = document
                .curve(curve)
                .expect("generated relief curve")
                .definition
            else {
                panic!("generated relief native curve must be a circle")
            };
            let radius = document
                .scalar(radius)
                .expect("generated relief radius")
                .value;
            (
                address.clone(),
                AcceptedRadiusState {
                    identity: provenance.identity,
                    radius_bits: radius.to_bits(),
                },
            )
        })
        .collect()
}

fn accepted_fillet_radii(
    materialized: &MaterializedCodeProject,
) -> BTreeMap<GeneratedMemberAddress, AcceptedRadiusState> {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted manufacturing materialization");
    materialized
        .host_outputs
        .iter()
        .map(|(address, outputs)| {
            let [output] = outputs.as_slice() else {
                panic!("each generated Fillet address must own exactly one output")
            };
            let ComputedFeatureDefinition::FilletSet(fillet) = &accepted
                .features
                .feature(output.owner.feature)
                .expect("accepted generated Fillet")
                .definition;
            (
                address.clone(),
                AcceptedRadiusState {
                    identity: output.identity,
                    radius_bits: fillet.radius.to_bits(),
                },
            )
        })
        .collect()
}

fn assert_exact_radius_partition(
    states: &BTreeMap<GeneratedMemberAddress, AcceptedRadiusState>,
    invocations: &[&str],
    count: usize,
    expected: f64,
) {
    let matching = states
        .iter()
        .filter(|(address, _)| invocations.contains(&address.invocation.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(matching.len(), count);
    assert!(
        matching
            .iter()
            .all(|(_, state)| state.radius_bits == expected.to_bits())
    );
}

fn assert_only_radius_partition_changed(
    before: &BTreeMap<GeneratedMemberAddress, AcceptedRadiusState>,
    after: &BTreeMap<GeneratedMemberAddress, AcceptedRadiusState>,
    changed_invocations: &[&str],
    changed_count: usize,
    expected: f64,
) {
    assert_eq!(
        before.keys().collect::<Vec<_>>(),
        after.keys().collect::<Vec<_>>()
    );
    let mut actual_changed = 0;
    for (address, before_state) in before {
        let after_state = after.get(address).expect("stable generated radius address");
        assert_eq!(after_state.identity, before_state.identity, "{address:?}");
        if changed_invocations.contains(&address.invocation.as_str()) {
            actual_changed += 1;
            assert_eq!(after_state.radius_bits, expected.to_bits(), "{address:?}");
            assert_ne!(
                after_state.radius_bits, before_state.radius_bits,
                "{address:?}"
            );
        } else {
            assert_eq!(
                after_state.radius_bits, before_state.radius_bits,
                "{address:?}"
            );
        }
    }
    assert_eq!(actual_changed, changed_count);
}

fn assert_stale_unit_token(
    project: &CodeProject,
    expansion: &ExpandedCodeProject,
    token: &ManagedControlToken,
    replacement: f64,
) {
    assert!(matches!(
        apply_managed_control_batch(
            project,
            expansion,
            &ManagedControlEditBatch::new([ManagedControlEdit {
                token: token.clone(),
                value: ManagedValue::Unit(UnitLiteral {
                    unit: "mm".into(),
                    value: replacement,
                }),
            }]),
        ),
        Err(ManagedControlError::StaleToken(_))
    ));
}

fn generated_point_positions(
    materialized: &MaterializedCodeProject,
) -> BTreeMap<GeneratedMemberAddress, [u64; 2]> {
    let intent = materialized.editor.coordinator().intent();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted manufacturing materialization");
    let accepted_state = accepted
        .session
        .accepted_state_for_current_input()
        .expect("accepted solved manufacturing state");
    materialized
        .expansion
        .generated_provenance
        .iter()
        .filter_map(|(address, provenance)| {
            let port = match &provenance.target {
                ExpandedSemanticTarget::Port { port } => port,
                ExpandedSemanticTarget::FeatureCorner { corner } => &corner.point,
                _ => return None,
            };
            let node = intent
                .graph()
                .node_by_symbol(&port.alias)
                .expect("generated point node");
            let semantic_port = node
                .port_by_selector(port.selector)
                .expect("generated point port")
                .as_ref(node.id);
            let Some(IntentNativeBinding::Point(point)) = accepted.ownership.port(semantic_port)
            else {
                return None;
            };
            let position = accepted_state
                .document()
                .point(point)
                .expect("generated native point")
                .position;
            Some((address.clone(), position.map(f64::to_bits)))
        })
        .collect()
}

fn accepted_point_positions(
    materialized: &MaterializedCodeProject,
) -> BTreeMap<DesignPointId, [u64; 2]> {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted manufacturing materialization");
    accepted
        .session
        .accepted_state_for_current_input()
        .expect("accepted solved manufacturing state")
        .document()
        .points()
        .iter()
        .map(|point| (point.id, point.position.map(f64::to_bits)))
        .collect()
}

fn polyline_point(invocation: &str, key: &str) -> GeneratedMemberAddress {
    GeneratedMemberAddress::new(invocation, ["polyline", "vertex"], [key], ["point"])
}

fn point_value(
    positions: &BTreeMap<GeneratedMemberAddress, [u64; 2]>,
    invocation: &str,
    key: &str,
) -> [f64; 2] {
    positions[&polyline_point(invocation, key)].map(f64::from_bits)
}

fn assert_near(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1.0e-9,
        "expected {expected}, got {actual}"
    );
}

fn point_bits_near(actual: [u64; 2], expected: [u64; 2]) -> bool {
    point_bits_within(actual, expected, 1.0e-9)
}

fn point_bits_within(actual: [u64; 2], expected: [u64; 2], tolerance: f64) -> bool {
    actual.into_iter().zip(expected).all(|(actual, expected)| {
        (f64::from_bits(actual) - f64::from_bits(expected)).abs() <= tolerance
    })
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one CNC owner regression keeps source fan-out, localization and a dimension edit under the same accepted product"
)]
fn cnc_coupon_fan_out_localization_and_nominal_height_edit_are_exact() {
    let project = demo_project(CodeProjectDemoId::CncJoineryFitCoupon);
    let generated = reconciliation(&project);
    assert_eq!(generated.active().len(), 69);
    let base = materialize(&project, &generated, 0x87c0_0100);
    assert_accepted_inventory(
        &project,
        &generated,
        &base,
        (62, 69, 14, 29, 47, 36, 10, 10, 23),
    );
    assert_absolute_datums(&base, 1, 0);
    assert_dimension_count(&base, 33);
    assert_driving_radius_dimensions(&base, 12);

    let manifest =
        managed_control_manifest(&project, &base.expansion).expect("CNC managed-control manifest");
    let cutter_radius = control_at(&manifest, "cutterRadius", &[]);
    assert_unit_value(cutter_radius, 3.175);
    let original_cutter_token = cutter_radius
        .token()
        .expect("original shared cutter-radius token")
        .clone();
    assert_generated_fan_out_by_template(
        cutter_radius,
        &["looseReliefs", "nominalReliefs", "pressReliefs"],
        &[("circle", 12), ("radius", 12)],
    );
    let edge_radius = control_at(&manifest, "edgeRadius", &[]);
    assert_unit_value(edge_radius, 6.0);
    let original_edge_token = edge_radius
        .token()
        .expect("original shared edge-radius token")
        .clone();
    assert_generated_fan_out(
        edge_radius,
        10,
        &[
            "blankHandling",
            "looseTabHandling",
            "nominalTabHandling",
            "pressTabHandling",
        ],
        "fillet",
    );
    assert_eq!(
        host_fillet_counts(&base.expansion),
        BTreeMap::from([
            ("blankHandling".into(), 4),
            ("looseTabHandling".into(), 2),
            ("nominalTabHandling".into(), 2),
            ("pressTabHandling".into(), 2),
        ])
    );
    let original_circles = accepted_circle_radii(&base);
    assert_exact_radius_partition(
        &original_circles,
        &["looseReliefs", "nominalReliefs", "pressReliefs"],
        12,
        3.175,
    );
    let original_fillets = accepted_fillet_radii(&base);
    assert_exact_radius_partition(
        &original_fillets,
        &[
            "blankHandling",
            "looseTabHandling",
            "nominalTabHandling",
            "pressTabHandling",
        ],
        10,
        6.0,
    );

    let localized = replace_in_declaration(
        &project,
        "pressReliefs",
        "radius: cutterRadius",
        "radius: mm(2.9)",
    );
    let localized = replace_in_declaration(
        &localized,
        "pressTabHandling",
        "radius: edgeRadius",
        "radius: mm(5)",
    );
    assert_eq!(
        required_generated_members(&localized).expect("localized inventory"),
        required_generated_members(&project).expect("base inventory")
    );
    assert_eq!(localized.artifacts, project.artifacts);
    assert_eq!(localized.custom_files, project.custom_files);
    let localized_base = materialize(&localized, &generated, 0x87c0_0101);
    assert_accepted_inventory(
        &localized,
        &generated,
        &localized_base,
        (62, 69, 14, 29, 47, 36, 10, 10, 23),
    );
    assert_stale_unit_token(
        &localized,
        &localized_base.expansion,
        &original_cutter_token,
        3.0,
    );
    assert_stale_unit_token(
        &localized,
        &localized_base.expansion,
        &original_edge_token,
        5.5,
    );
    let localized_manifest = managed_control_manifest(&localized, &localized_base.expansion)
        .expect("localized CNC controls");
    let localized_cutter = control_at(&localized_manifest, "cutterRadius", &[]);
    assert_generated_fan_out_by_template(
        localized_cutter,
        &["looseReliefs", "nominalReliefs"],
        &[("circle", 8), ("radius", 8)],
    );
    let local_relief = control_at(&localized_manifest, "pressReliefs", &["radius"]);
    assert_unit_value(local_relief, 2.9);
    assert_generated_fan_out_by_template(
        local_relief,
        &["pressReliefs"],
        &[("circle", 4), ("radius", 4)],
    );
    let localized_edge = control_at(&localized_manifest, "edgeRadius", &[]);
    assert_generated_fan_out(
        localized_edge,
        8,
        &["blankHandling", "looseTabHandling", "nominalTabHandling"],
        "fillet",
    );
    let local_tab = control_at(&localized_manifest, "pressTabHandling", &["radius"]);
    assert_unit_value(local_tab, 5.0);
    assert_generated_fan_out(local_tab, 2, &["pressTabHandling"], "fillet");

    let localized_circles = accepted_circle_radii(&localized_base);
    assert_only_radius_partition_changed(
        &original_circles,
        &localized_circles,
        &["pressReliefs"],
        4,
        2.9,
    );
    assert_exact_radius_partition(
        &localized_circles,
        &["looseReliefs", "nominalReliefs"],
        8,
        3.175,
    );
    let localized_fillets = accepted_fillet_radii(&localized_base);
    assert_only_radius_partition_changed(
        &original_fillets,
        &localized_fillets,
        &["pressTabHandling"],
        2,
        5.0,
    );
    assert_exact_radius_partition(
        &localized_fillets,
        &["blankHandling", "looseTabHandling", "nominalTabHandling"],
        8,
        6.0,
    );

    let shared_edited = apply_managed_control_batch(
        &localized,
        &localized_base.expansion,
        &ManagedControlEditBatch::new([
            unit_edit(localized_cutter, 3.0),
            unit_edit(localized_edge, 5.5),
        ]),
    )
    .expect("independent shared CNC radius edit");
    assert_eq!(shared_edited.artifacts, localized.artifacts);
    assert_eq!(shared_edited.custom_files, localized.custom_files);
    let shared_materialized = materialize(&shared_edited, &generated, 0x87c0_0101);
    assert_accepted_inventory(
        &shared_edited,
        &generated,
        &shared_materialized,
        (62, 69, 14, 29, 47, 36, 10, 10, 23),
    );
    assert_only_radius_partition_changed(
        &localized_circles,
        &accepted_circle_radii(&shared_materialized),
        &["looseReliefs", "nominalReliefs"],
        8,
        3.0,
    );
    assert_only_radius_partition_changed(
        &localized_fillets,
        &accepted_fillet_radii(&shared_materialized),
        &["blankHandling", "looseTabHandling", "nominalTabHandling"],
        8,
        5.5,
    );
    let shared_manifest = managed_control_manifest(&shared_edited, &shared_materialized.expansion)
        .expect("shared-edited CNC controls");
    assert_eq!(
        control_at(&shared_manifest, "cutterRadius", &[]).id,
        localized_cutter.id
    );
    assert_eq!(
        control_at(&shared_manifest, "edgeRadius", &[]).id,
        localized_edge.id
    );
    assert_eq!(
        control_at(&shared_manifest, "pressReliefs", &["radius"]).id,
        local_relief.id
    );
    assert_eq!(
        control_at(&shared_manifest, "pressTabHandling", &["radius"]).id,
        local_tab.id
    );

    let local_edited = apply_managed_control_batch(
        &localized,
        &localized_base.expansion,
        &ManagedControlEditBatch::new([unit_edit(local_relief, 2.7), unit_edit(local_tab, 4.5)]),
    )
    .expect("independent local CNC radius edit");
    assert_eq!(local_edited.artifacts, localized.artifacts);
    assert_eq!(local_edited.custom_files, localized.custom_files);
    let local_materialized = materialize(&local_edited, &generated, 0x87c0_0101);
    assert_accepted_inventory(
        &local_edited,
        &generated,
        &local_materialized,
        (62, 69, 14, 29, 47, 36, 10, 10, 23),
    );
    assert_only_radius_partition_changed(
        &localized_circles,
        &accepted_circle_radii(&local_materialized),
        &["pressReliefs"],
        4,
        2.7,
    );
    assert_only_radius_partition_changed(
        &localized_fillets,
        &accepted_fillet_radii(&local_materialized),
        &["pressTabHandling"],
        2,
        4.5,
    );
    let local_manifest = managed_control_manifest(&local_edited, &local_materialized.expansion)
        .expect("local-edited CNC controls");
    assert_eq!(
        control_at(&local_manifest, "cutterRadius", &[]).id,
        localized_cutter.id
    );
    assert_eq!(
        control_at(&local_manifest, "edgeRadius", &[]).id,
        localized_edge.id
    );
    assert_eq!(
        control_at(&local_manifest, "pressReliefs", &["radius"]).id,
        local_relief.id
    );
    assert_eq!(
        control_at(&local_manifest, "pressTabHandling", &["radius"]).id,
        local_tab.id
    );

    let height = control_at(&manifest, "nominalMortiseHeight", &["target"]);
    assert_unit_value(height, 18.0);
    let edited = apply_managed_control_batch(
        &project,
        &base.expansion,
        &ManagedControlEditBatch::new([unit_edit(height, 18.2)]),
    )
    .expect("nominal mortise height edit");
    assert_eq!(edited.artifacts, project.artifacts);
    assert_eq!(edited.custom_files, project.custom_files);
    assert!(edited.managed.source.contains(concat!(
        "const nominalMortiseHeight = $.dimension.curveLength(\"nominalMortiseHeight\", ",
        "{ curve: nominalMortise.segments.byKey.lowerRight, target: mm(18.2) });"
    )));
    let edited_materialized = materialize(&edited, &generated, 0x87c0_0100);
    assert_accepted_inventory(
        &edited,
        &generated,
        &edited_materialized,
        (62, 69, 14, 29, 47, 36, 10, 10, 23),
    );
    let edited_manifest = managed_control_manifest(&edited, &edited_materialized.expansion)
        .expect("edited CNC controls");
    assert_eq!(
        edited_manifest.editable().count(),
        manifest.editable().count()
    );
    for before in manifest.editable() {
        let after = edited_manifest
            .control(&before.id)
            .expect("stable managed-control identity");
        if before.id != height.id {
            assert_eq!(after.value, before.value, "{} value", before.id.0);
            assert_eq!(
                after.source.source_text, before.source.source_text,
                "{} source text",
                before.id.0
            );
            assert_eq!(after.consumers, before.consumers, "{} fan-out", before.id.0);
        }
    }
    let edited_height = control_at(&edited_manifest, "nominalMortiseHeight", &["target"]);
    assert_eq!(edited_height.id, height.id);
    assert_unit_value(edited_height, 18.2);

    let before_points = generated_point_positions(&base);
    let after_points = generated_point_positions(&edited_materialized);
    assert_eq!(before_points.len(), 12);
    assert_eq!(after_points.len(), 12);
    let changed = BTreeSet::from([
        polyline_point("nominalMortise", "upperRight"),
        polyline_point("nominalMortise", "upperLeft"),
    ]);
    for (address, position) in &before_points {
        if !changed.contains(address) {
            assert!(
                after_points
                    .get(address)
                    .is_some_and(|after| point_bits_near(*after, *position)),
                "{address:?}"
            );
        }
    }
    for key in ["upperRight", "upperLeft"] {
        assert_near(point_value(&after_points, "nominalMortise", key)[1], 9.2);
    }
    assert_near(
        point_value(&after_points, "nominalMortise", "lowerLeft")[1],
        -9.0,
    );
    let before_native_points = accepted_point_positions(&base);
    let after_native_points = accepted_point_positions(&edited_materialized);
    assert_eq!(before_native_points.len(), 29);
    assert_eq!(after_native_points.len(), 29);
    assert_eq!(
        before_native_points.keys().collect::<Vec<_>>(),
        after_native_points.keys().collect::<Vec<_>>()
    );
    assert_eq!(
        before_native_points
            .iter()
            .filter(|(point, position)| {
                after_native_points
                    .get(point)
                    .is_none_or(|after| !point_bits_near(*after, **position))
            })
            .count(),
        2,
        "only the nominal mortise's two upper corners may move"
    );

    let displaced = replace_source_once(&project, "upperRight: [-10, 70]", "upperRight: [-12, 67]");
    let displaced = replace_source_once(
        &displaced,
        "{ key: \"lowerLeft\", position: [-105, 32.8] }",
        "{ key: \"lowerLeft\", position: [-103.5, 31.6] }",
    );
    let displaced = replace_source_once(
        &displaced,
        "const nominalTab = $.geometry.rectangle(\"nominalTab\", { lowerLeft: [20, -9]",
        "const nominalTab = $.geometry.rectangle(\"nominalTab\", { lowerLeft: [22, -7.5]",
    );
    let displaced_materialized = materialize(&displaced, &generated, 0x87c0_0100);
    assert_accepted_inventory(
        &displaced,
        &generated,
        &displaced_materialized,
        (62, 69, 14, 29, 47, 36, 10, 10, 23),
    );
    assert_absolute_datums(&displaced_materialized, 1, 0);
    let displaced_points = accepted_point_positions(&displaced_materialized);
    assert_eq!(
        before_native_points.keys().collect::<Vec<_>>(),
        displaced_points.keys().collect::<Vec<_>>()
    );
    for (point, expected) in &before_native_points {
        assert!(
            displaced_points
                .get(point)
                .is_some_and(|actual| point_bits_near(*actual, *expected)),
            "literal CNC seed must not own accepted point {point:?}"
        );
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one keyed-relief lifecycle test keeps removal, reinsertion, identity, and token authority together"
)]
fn cnc_keyed_relief_reinsertion_advances_only_its_identity_and_stales_old_tokens() {
    let project = demo_project(CodeProjectDemoId::CncJoineryFitCoupon);
    let initial_members = required_generated_members(&project).expect("initial CNC inventory");
    let initial = reconciliation(&project);
    let initial_expansion = expand_project(&project, &initial, 0x87c0_0200);
    let initial_manifest =
        managed_control_manifest(&project, &initial_expansion).expect("initial CNC controls");
    let old_token = control_at(&initial_manifest, "cutterRadius", &[])
        .token()
        .expect("shared cutter-radius token")
        .clone();

    let relief_line = "      upperLeft: nominalMortise.vertices.byKey.upperLeft,\n";
    assert_eq!(project.managed.source.matches(relief_line).count(), 1);
    let removed_project = project_with_source(
        &project,
        &project.managed.source.replacen(relief_line, "", 1),
    );
    let removal = initial
        .plan(
            required_generated_members(&removed_project).expect("removed CNC inventory"),
            &BTreeSet::new(),
        )
        .expect("relief removal reconciliation");
    let relief_address =
        GeneratedMemberAddress::new("nominalReliefs", ["circle"], ["upperLeft"], ["circle"]);
    let radius_address =
        GeneratedMemberAddress::new("nominalReliefs", ["radius"], ["upperLeft"], ["dimension"]);
    let removed_addresses = BTreeSet::from([relief_address.clone(), radius_address.clone()]);
    assert!(removal.created.is_empty());
    assert_eq!(removal.retained.len(), 67);
    assert_eq!(removal.removed.len(), 2);
    assert_eq!(
        removal
            .removed
            .iter()
            .map(|member| member.address.clone())
            .collect::<BTreeSet<_>>(),
        removed_addresses
    );
    let retired = removal
        .removed
        .iter()
        .map(|member| (member.address.clone(), member.identity))
        .collect::<BTreeMap<_, _>>();
    let removed = removal.into_staged();
    for (address, identity) in initial.active() {
        if !removed_addresses.contains(address) {
            assert_eq!(removed.active().get(address), Some(identity), "{address:?}");
        }
    }
    let removed_materialized = materialize(&removed_project, &removed, 0x87c0_0201);
    assert_accepted_inventory(
        &removed_project,
        &removed,
        &removed_materialized,
        (62, 67, 14, 29, 46, 36, 10, 10, 23),
    );

    let reinsertion = removed
        .plan(initial_members, &BTreeSet::new())
        .expect("relief reinsertion reconciliation");
    assert_eq!(reinsertion.created.len(), 2);
    assert_eq!(reinsertion.retained.len(), 67);
    assert!(reinsertion.removed.is_empty());
    assert_eq!(
        reinsertion
            .created
            .iter()
            .map(|member| member.address.clone())
            .collect::<BTreeSet<_>>(),
        removed_addresses
    );
    for recreated in &reinsertion.created {
        let retired_identity = retired[&recreated.address];
        assert!(recreated.identity.allocation > retired_identity.allocation);
        assert_eq!(
            recreated.identity.generation,
            retired_identity.generation + 1
        );
        assert_eq!(
            reinsertion.staged().tombstones()[&recreated.address],
            retired_identity
        );
    }
    for (address, identity) in initial.active() {
        if !removed_addresses.contains(address) {
            assert_eq!(
                reinsertion.staged().active().get(address),
                Some(identity),
                "{address:?}"
            );
        }
    }

    let reinserted = materialize(&project, reinsertion.staged(), 0x87c0_0200);
    assert_accepted_inventory(
        &project,
        reinsertion.staged(),
        &reinserted,
        (62, 69, 14, 29, 47, 36, 10, 10, 23),
    );
    let reinserted_manifest =
        managed_control_manifest(&project, &reinserted.expansion).expect("reinserted CNC controls");
    let reinserted_cutter = control_at(&reinserted_manifest, "cutterRadius", &[]);
    assert_ne!(
        reinserted_cutter
            .token()
            .expect("reinserted cutter token")
            .generation_digest,
        old_token.generation_digest
    );
    assert!(matches!(
        apply_managed_control_batch(
            &project,
            &reinserted.expansion,
            &ManagedControlEditBatch::new([ManagedControlEdit {
                token: old_token,
                value: ManagedValue::Unit(UnitLiteral {
                    unit: "mm".into(),
                    value: 3.0,
                }),
            }]),
        ),
        Err(ManagedControlError::StaleToken(_))
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one Gridfinity profile oracle keeps its closed accepted contour, canonical dimensions and radius fan-outs together"
)]
fn gridfinity_section_is_one_symmetric_standard_profile_with_two_radius_owners() {
    let project = demo_project(CodeProjectDemoId::GridfinityBinSection);
    let generated = reconciliation(&project);
    assert_eq!(generated.active().len(), 66);
    let materialized = materialize(&project, &generated, 0x876f_0100);
    assert_accepted_inventory(
        &project,
        &generated,
        &materialized,
        (62, 66, 3, 31, 36, 31, 4, 4, 11),
    );
    assert_absolute_datums(&materialized, 0, 1);
    assert_dimension_count(&materialized, 18);

    let accepted_document = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted Gridfinity materialization")
        .session
        .accepted_state_for_current_input()
        .expect("accepted Gridfinity state")
        .document();
    assert_eq!(
        accepted_document
            .constraints()
            .iter()
            .filter(|constraint| {
                matches!(
                    constraint.definition,
                    DocumentConstraintDefinition::SymmetricAboutDatumAxis { .. }
                )
            })
            .count(),
        13
    );

    let manifest = managed_control_manifest(&project, &materialized.expansion)
        .expect("Gridfinity managed controls");
    let floor_radius = control_at(&manifest, "cavityFloorRadius", &[]);
    assert_unit_value(floor_radius, 2.8);
    let floor_token = floor_radius
        .token()
        .expect("Gridfinity floor-radius token")
        .clone();
    assert_generated_fan_out(floor_radius, 2, &["floorFillets"], "fillet");
    let lip_radius = control_at(&manifest, "stackingLipRadius", &[]);
    assert_unit_value(lip_radius, 0.6);
    let lip_token = lip_radius
        .token()
        .expect("Gridfinity lip-radius token")
        .clone();
    assert_generated_fan_out(lip_radius, 2, &["lipFillets"], "fillet");
    assert_eq!(
        host_fillet_counts(&materialized.expansion),
        BTreeMap::from([("floorFillets".into(), 2), ("lipFillets".into(), 2)])
    );
    let baseline_fillets = accepted_fillet_radii(&materialized);
    assert_exact_radius_partition(&baseline_fillets, &["floorFillets"], 2, 2.8);
    assert_exact_radius_partition(&baseline_fillets, &["lipFillets"], 2, 0.6);

    let floor_edited = apply_managed_control_batch(
        &project,
        &materialized.expansion,
        &ManagedControlEditBatch::new([unit_edit(floor_radius, 3.0)]),
    )
    .expect("independent Gridfinity floor-radius edit");
    assert_eq!(floor_edited.artifacts, project.artifacts);
    assert_eq!(floor_edited.custom_files, project.custom_files);
    let floor_materialized = materialize(&floor_edited, &generated, 0x876f_0100);
    assert_accepted_inventory(
        &floor_edited,
        &generated,
        &floor_materialized,
        (62, 66, 3, 31, 36, 31, 4, 4, 11),
    );
    let floor_fillets = accepted_fillet_radii(&floor_materialized);
    assert_only_radius_partition_changed(
        &baseline_fillets,
        &floor_fillets,
        &["floorFillets"],
        2,
        3.0,
    );
    assert_exact_radius_partition(&floor_fillets, &["lipFillets"], 2, 0.6);
    let floor_manifest = managed_control_manifest(&floor_edited, &floor_materialized.expansion)
        .expect("floor-edited Gridfinity controls");
    assert_eq!(
        control_at(&floor_manifest, "cavityFloorRadius", &[]).id,
        floor_radius.id
    );
    assert_eq!(
        control_at(&floor_manifest, "stackingLipRadius", &[]).id,
        lip_radius.id
    );
    assert_stale_unit_token(
        &floor_edited,
        &floor_materialized.expansion,
        &floor_token,
        3.1,
    );

    let lip_edited = apply_managed_control_batch(
        &project,
        &materialized.expansion,
        &ManagedControlEditBatch::new([unit_edit(lip_radius, 0.8)]),
    )
    .expect("independent Gridfinity lip-radius edit");
    assert_eq!(lip_edited.artifacts, project.artifacts);
    assert_eq!(lip_edited.custom_files, project.custom_files);
    let lip_materialized = materialize(&lip_edited, &generated, 0x876f_0100);
    assert_accepted_inventory(
        &lip_edited,
        &generated,
        &lip_materialized,
        (62, 66, 3, 31, 36, 31, 4, 4, 11),
    );
    let lip_fillets = accepted_fillet_radii(&lip_materialized);
    assert_only_radius_partition_changed(&baseline_fillets, &lip_fillets, &["lipFillets"], 2, 0.8);
    assert_exact_radius_partition(&lip_fillets, &["floorFillets"], 2, 2.8);
    let lip_manifest = managed_control_manifest(&lip_edited, &lip_materialized.expansion)
        .expect("lip-edited Gridfinity controls");
    assert_eq!(
        control_at(&lip_manifest, "cavityFloorRadius", &[]).id,
        floor_radius.id
    );
    assert_eq!(
        control_at(&lip_manifest, "stackingLipRadius", &[]).id,
        lip_radius.id
    );
    assert_stale_unit_token(&lip_edited, &lip_materialized.expansion, &lip_token, 0.9);

    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("accepted Gridfinity authority");
    assert_eq!(accepted.ownership.aggregates.len(), 1);
    assert!(accepted.ownership.aggregates[0].closed);
    assert_eq!(accepted.ownership.aggregates[0].spans.len(), 26);

    let positions = generated_point_positions(&materialized);
    assert_eq!(positions.len(), 26);
    let expected = [
        ("baseBottomLeft", [-17.8, 0.0]),
        ("baseBottomRight", [17.8, 0.0]),
        ("rightBaseLowerChamferEnd", [18.6, 0.8]),
        ("rightBaseVerticalEnd", [18.6, 2.6]),
        ("rightBaseProfileTop", [20.75, 4.75]),
        ("rightBaseTop", [20.75, 7.0]),
        ("rightBodyTop", [20.75, 21.0]),
        ("rightLipCrown", [20.75, 25.4]),
        ("rightLipUpperShoulder", [18.85, 23.5]),
        ("rightLipLowerShoulder", [18.85, 21.7]),
        ("rightLipInnerTip", [18.15, 21.0]),
        ("rightLipSupportInner", [18.15, 19.8]),
        ("rightLipSupportWall", [19.8, 18.15]),
        ("rightCavityFloorCorner", [19.8, 7.0]),
        ("leftCavityFloorCorner", [-19.8, 7.0]),
        ("leftLipSupportWall", [-19.8, 18.15]),
        ("leftLipSupportInner", [-18.15, 19.8]),
        ("leftLipInnerTip", [-18.15, 21.0]),
        ("leftLipLowerShoulder", [-18.85, 21.7]),
        ("leftLipUpperShoulder", [-18.85, 23.5]),
        ("leftLipCrown", [-20.75, 25.4]),
        ("leftBodyTop", [-20.75, 21.0]),
        ("leftBaseTop", [-20.75, 7.0]),
        ("leftBaseProfileTop", [-20.75, 4.75]),
        ("leftBaseVerticalEnd", [-18.6, 2.6]),
        ("leftBaseLowerChamferEnd", [-18.6, 0.8]),
    ];
    for (key, expected_position) in expected {
        let actual = point_value(&positions, "section", key);
        assert_near(actual[0], expected_position[0]);
        assert_near(actual[1], expected_position[1]);
    }

    let symmetric_pairs = [
        ("baseBottomLeft", "baseBottomRight"),
        ("leftBaseLowerChamferEnd", "rightBaseLowerChamferEnd"),
        ("leftBaseVerticalEnd", "rightBaseVerticalEnd"),
        ("leftBaseProfileTop", "rightBaseProfileTop"),
        ("leftBaseTop", "rightBaseTop"),
        ("leftBodyTop", "rightBodyTop"),
        ("leftLipCrown", "rightLipCrown"),
        ("leftLipUpperShoulder", "rightLipUpperShoulder"),
        ("leftLipLowerShoulder", "rightLipLowerShoulder"),
        ("leftLipInnerTip", "rightLipInnerTip"),
        ("leftLipSupportInner", "rightLipSupportInner"),
        ("leftLipSupportWall", "rightLipSupportWall"),
        ("leftCavityFloorCorner", "rightCavityFloorCorner"),
    ];
    for (left, right) in symmetric_pairs {
        let left = point_value(&positions, "section", left);
        let right = point_value(&positions, "section", right);
        assert_near(left[0], -right[0]);
        assert_near(left[1], right[1]);
    }

    let bottom_left = point_value(&positions, "section", "baseBottomLeft");
    let bottom_right = point_value(&positions, "section", "baseBottomRight");
    let profile_top = point_value(&positions, "section", "rightBaseProfileTop");
    let lower_chamfer = point_value(&positions, "section", "rightBaseLowerChamferEnd");
    let vertical_end = point_value(&positions, "section", "rightBaseVerticalEnd");
    let base_top = point_value(&positions, "section", "rightBaseTop");
    let body_top = point_value(&positions, "section", "rightBodyTop");
    let lip_crown = point_value(&positions, "section", "rightLipCrown");
    let cavity_right = point_value(&positions, "section", "rightCavityFloorCorner");
    let cavity_left = point_value(&positions, "section", "leftCavityFloorCorner");
    assert_near(bottom_right[0] - bottom_left[0], 35.6);
    assert_near(profile_top[0] * 2.0, 41.5);
    assert_near(lower_chamfer[1], 0.8);
    assert_near(vertical_end[1] - lower_chamfer[1], 1.8);
    assert_near(profile_top[1] - vertical_end[1], 2.15);
    assert_near(profile_top[1], 4.75);
    assert_near(base_top[1], 7.0);
    assert_near(body_top[1], 21.0);
    assert_near(profile_top[0] - cavity_right[0], 0.95);
    assert_near(cavity_right[0] - cavity_left[0], 39.6);
    assert_near(lip_crown[1] - body_top[1], 4.4);

    let displaced = replace_source_once(
        &project,
        "{ key: \"baseBottomLeft\", position: [-17.8, 0] }",
        "{ key: \"baseBottomLeft\", position: [-16.9, 0.4] }",
    );
    let displaced = replace_source_once(
        &displaced,
        "{ key: \"rightLipCrown\", position: [20.75, 25.4] }",
        "{ key: \"rightLipCrown\", position: [20.1, 24.8] }",
    );
    let displaced = replace_source_once(
        &displaced,
        "{ key: \"rightCavityFloorCorner\", position: [19.8, 7] }",
        "{ key: \"rightCavityFloorCorner\", position: [19.1, 7.6] }",
    );
    let displaced = replace_source_once(&displaced, "end: [18.6, 0]", "end: [18.1, 0.3]");
    let displaced_materialized = materialize(&displaced, &generated, 0x876f_0100);
    assert_accepted_inventory(
        &displaced,
        &generated,
        &displaced_materialized,
        (62, 66, 3, 31, 36, 31, 4, 4, 11),
    );
    assert_absolute_datums(&displaced_materialized, 0, 1);
    let displaced_positions = generated_point_positions(&displaced_materialized);
    assert_eq!(
        positions.keys().collect::<Vec<_>>(),
        displaced_positions.keys().collect::<Vec<_>>()
    );
    for (address, expected) in &positions {
        let actual = displaced_positions
            .get(address)
            .expect("displaced Gridfinity point");
        assert!(
            point_bits_within(*actual, *expected, 1.0e-8),
            "literal Gridfinity seed must not own accepted point {address:?}: expected {:?}, got {:?}",
            expected.map(f64::from_bits),
            actual.map(f64::from_bits)
        );
    }
}
