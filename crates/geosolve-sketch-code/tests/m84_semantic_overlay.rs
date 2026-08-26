// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::float_cmp,
    clippy::too_many_lines,
    reason = "semantic-overlay regressions compare exact persisted seed values and complete atomic lifecycles"
)]

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::IntentNativeBinding;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeDraftProvenance, CodeExpansionError, CodeInteractionOverlay, CodeOverlayError,
    CodePointEdit, CodePointSeedSource, CodeProject, CodeProjectDemoId, CodeRectangleCorner,
    ExpandedCodeProject, ExpandedPort, ExpandedWritablePoint, KeyedReconcileState, ManagedEdit,
    ManagedPathSegment, ProjectKey, SemanticOutputPath, SemanticSymbol, SketchCodeSession,
    apply_managed_edit, bundled_code_project_demos, expand_code_project,
    expand_code_project_for_structural_edit, expand_code_project_with_overlay,
    materialize_code_project_cold_with_overlay,
    materialize_code_project_incremental_for_structural_edit, parse_managed_source,
    plan_managed_edit, required_generated_members, stage_point_drags,
};
use geosolve_sketch_intent::{IntentPatchOperation, IntentSession, IntentSessionId};

fn direct_project() -> CodeProject {
    let source = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const diagonal = $.geometry.line("diagonal", {
    start: frame.corners.lowerLeft,
    end: frame.corners.upperRight,
  });
  const leader = $.geometry.line("leader", {
    start: [-12, 7],
    end: [8, 11],
  });
  return $.outputs({ frame, diagonal, leader });
});
"#;
    CodeProject {
        project: ProjectKey("m84-overlay-direct".into()),
        managed: parse_managed_source(source).unwrap(),
        custom_files: BTreeMap::new(),
        artifacts: BTreeMap::new(),
        lock: serde_json::json!({ "format": "geosolve-lock-v1", "modules": {} }),
    }
}

fn reconciled(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged()
}

fn expansion(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    seed: u128,
) -> ExpandedCodeProject {
    let intent = IntentSession::with_id(IntentSessionId::from_raw(seed)).unwrap();
    expand_code_project(project, generated, intent.identity()).unwrap()
}

fn point_position(
    materialized: &geosolve_sketch_code::MaterializedCodeProject,
    port: &ExpandedPort,
) -> [f64; 2] {
    let intent = materialized.editor.coordinator().intent();
    let node = intent.graph().node_by_symbol(&port.alias).unwrap();
    let reference = node
        .port_by_selector(port.selector)
        .unwrap()
        .as_ref(node.id);
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    let IntentNativeBinding::Point(point) = accepted.ownership.port(reference).unwrap() else {
        panic!("writable port must own a native point")
    };
    accepted
        .session
        .design_document()
        .point(point)
        .unwrap()
        .position
}

fn point_binding(
    materialized: &geosolve_sketch_code::MaterializedCodeProject,
    port: &ExpandedPort,
) -> IntentNativeBinding {
    let intent = materialized.editor.coordinator().intent();
    let node = intent.graph().node_by_symbol(&port.alias).unwrap();
    let reference = node
        .port_by_selector(port.selector)
        .unwrap()
        .as_ref(node.id);
    materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .ownership
        .port(reference)
        .unwrap()
}

fn assert_valid(materialized: &geosolve_sketch_code::MaterializedCodeProject) {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
}

fn point_for_field<'a>(
    expansion: &'a ExpandedCodeProject,
    declaration: &str,
    field: &str,
) -> &'a ExpandedWritablePoint {
    expansion
        .writable_points
        .iter()
        .find(|point| {
            expansion.declaration_for_alias(&point.handle.alias)
                == Some(&SemanticSymbol(declaration.into()))
                && matches!(
                    &point.edit,
                    CodePointEdit::Point { address }
                        if address.output
                            == SemanticOutputPath(vec![ManagedPathSegment::Field(field.into())])
                )
        })
        .unwrap()
}

#[test]
fn direct_rectangle_line_and_reference_drags_use_semantic_seed_ownership() {
    let project = direct_project();
    let generated = reconciled(&project);
    let expanded = expansion(&project, &generated, 0x84f0_5001);

    let corners = expanded
        .writable_points
        .iter()
        .filter_map(|point| match &point.edit {
            CodePointEdit::RectangleCorner { corner, .. } => Some((*corner, point)),
            CodePointEdit::Point { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(corners.len(), 4);
    let expected = [
        (
            CodeRectangleCorner::LowerLeft,
            [-3.0, -4.0],
            [-3.0, -4.0],
            [60.0, 35.0],
        ),
        (
            CodeRectangleCorner::LowerRight,
            [62.0, -4.0],
            [0.0, -4.0],
            [62.0, 35.0],
        ),
        (
            CodeRectangleCorner::UpperRight,
            [62.0, 38.0],
            [0.0, 0.0],
            [62.0, 38.0],
        ),
        (
            CodeRectangleCorner::UpperLeft,
            [-3.0, 38.0],
            [-3.0, 0.0],
            [60.0, 38.0],
        ),
    ];
    for (corner, target, lower, upper) in expected {
        let overlay = corners[&corner]
            .stage_drag(&CodeInteractionOverlay::empty(), target)
            .unwrap();
        let values = overlay
            .drafts()
            .values()
            .map(|draft| draft.value)
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&geosolve_sketch_code::CodeDraftValue::Point(lower)));
        assert!(values.contains(&geosolve_sketch_code::CodeDraftValue::Point(upper)));
    }

    let referenced = point_for_field(&expanded, "diagonal", "start");
    assert!(matches!(
        referenced.source,
        CodePointSeedSource::Reference { .. }
    ));
    let literal = point_for_field(&expanded, "leader", "start");
    assert_eq!(literal.source, CodePointSeedSource::Literal);
    let overlay = stage_point_drags(
        &CodeInteractionOverlay::empty(),
        [(referenced, [5.0, 6.0]), (literal, [-9.0, 4.0])],
    )
    .unwrap();
    assert_eq!(overlay.drafts().len(), 2);
    assert!(overlay.drafts().values().any(|draft| {
        draft.provenance == CodeDraftProvenance::DetachedReference
            && draft.value == geosolve_sketch_code::CodeDraftValue::Point([5.0, 6.0])
    }));

    let materialized = materialize_code_project_cold_with_overlay(
        &project,
        &generated,
        &overlay,
        IntentSessionId::from_raw(0x84f0_5002),
        DocumentId(PersistentId::from_u128(0x84f0_5002)),
        1.0,
    )
    .unwrap();
    assert_valid(&materialized);
    let detached = materialized
        .expansion
        .writable_points
        .iter()
        .find(|point| point.edit == referenced.edit)
        .unwrap();
    let moved_literal = materialized
        .expansion
        .writable_points
        .iter()
        .find(|point| point.edit == literal.edit)
        .unwrap();
    assert_eq!(point_position(&materialized, &detached.handle), [5.0, 6.0]);
    assert_eq!(
        point_position(&materialized, &moved_literal.handle),
        [-9.0, 4.0]
    );
    assert_eq!(
        point_position(
            &materialized,
            &corners[&CodeRectangleCorner::LowerLeft].handle
        ),
        [0.0, 0.0]
    );
    assert_ne!(
        point_binding(&materialized, &detached.handle),
        point_binding(
            &materialized,
            &corners[&CodeRectangleCorner::LowerLeft].handle
        ),
        "detaching a referenced consumer must allocate a distinct native point"
    );
}

#[test]
fn terminal_bundles_are_atomic_order_independent_and_session_authenticates_lenses() {
    let project = direct_project();
    let generated = reconciled(&project);
    let expanded = expansion(&project, &generated, 0x84f0_5010);
    let point = point_for_field(&expanded, "leader", "start").clone();
    let corners = expanded
        .writable_points
        .iter()
        .filter_map(|point| match &point.edit {
            CodePointEdit::RectangleCorner { corner, .. } => Some((*corner, point)),
            CodePointEdit::Point { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    let empty = CodeInteractionOverlay::empty();
    for drags in [
        vec![(&point, [1.0, 2.0]), (&point, [3.0, 4.0])],
        vec![(&point, [3.0, 4.0]), (&point, [1.0, 2.0])],
    ] {
        assert!(matches!(
            stage_point_drags(&empty, drags),
            Err(CodeOverlayError::ConflictingDraft(_))
        ));
        assert!(empty.drafts().is_empty());
    }
    let equal = stage_point_drags(&empty, [(&point, [1.0, 2.0]), (&point, [1.0, 2.0])]).unwrap();
    assert_eq!(equal.drafts().len(), 1);
    for signed_zero in [
        [(&point, [0.0, 2.0]), (&point, [-0.0, 2.0])],
        [(&point, [-0.0, 2.0]), (&point, [0.0, 2.0])],
    ] {
        assert!(matches!(
            stage_point_drags(&empty, signed_zero),
            Err(CodeOverlayError::ConflictingDraft(_))
        ));
    }
    assert!(matches!(
        stage_point_drags(&empty, [(&point, [f64::NAN, 0.0])]),
        Err(CodeOverlayError::NonFinite(_))
    ));

    for (left, left_target, right, right_target, expected_lower, expected_upper) in [
        (
            CodeRectangleCorner::LowerLeft,
            [-3.0, -4.0],
            CodeRectangleCorner::UpperRight,
            [63.0, 39.0],
            [-3.0, -4.0],
            [63.0, 39.0],
        ),
        (
            CodeRectangleCorner::LowerRight,
            [63.0, -4.0],
            CodeRectangleCorner::UpperLeft,
            [-3.0, 39.0],
            [-3.0, -4.0],
            [63.0, 39.0],
        ),
    ] {
        let forward = stage_point_drags(
            &empty,
            [
                (corners[&left], left_target),
                (corners[&right], right_target),
            ],
        )
        .unwrap();
        let reverse = stage_point_drags(
            &empty,
            [
                (corners[&right], right_target),
                (corners[&left], left_target),
            ],
        )
        .unwrap();
        assert_eq!(forward, reverse);
        let values = forward
            .drafts()
            .values()
            .map(|draft| draft.value)
            .collect::<Vec<_>>();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&geosolve_sketch_code::CodeDraftValue::Point(expected_lower)));
        assert!(values.contains(&geosolve_sketch_code::CodeDraftValue::Point(expected_upper)));
    }
    assert!(matches!(
        stage_point_drags(
            &empty,
            [
                (corners[&CodeRectangleCorner::LowerLeft], [-3.0, -4.0]),
                (corners[&CodeRectangleCorner::UpperLeft], [-5.0, 39.0]),
            ],
        ),
        Err(CodeOverlayError::ConflictingDraft(_))
    ));

    let session = SketchCodeSession::new_project(
        project,
        generated,
        expanded,
        serde_json::json!({ "checkpoint": "base" }),
    )
    .unwrap();
    session.stage_point_drag(&point, [2.0, 3.0]).unwrap();
    let mut forged = point.clone();
    if let CodePointEdit::Point { address } = &mut forged.edit {
        address.owner.generation = address.owner.generation.saturating_add(1);
    }
    assert!(matches!(
        session.stage_point_drag(&forged, [2.0, 3.0]),
        Err(geosolve_sketch_code::CodeSessionError::UnknownSemanticOwner(_))
    ));
}

#[test]
fn generated_points_and_host_children_are_writable_suppressible_and_fully_provenanced() {
    let demos = bundled_code_project_demos();
    let project = demos
        .iter()
        .find(|demo| demo.id == CodeProjectDemoId::RoundedPolyline)
        .unwrap()
        .project();
    let generated = reconciled(&project);
    let expanded = expansion(&project, &generated, 0x84f0_5020);

    let emitted = expanded
        .patch
        .operations()
        .iter()
        .filter_map(|operation| match operation {
            IntentPatchOperation::CreateNode { alias, .. } => Some(alias.clone()),
            _ => None,
        })
        .chain(
            expanded
                .generated_children
                .iter()
                .map(|child| child.alias.clone()),
        )
        .collect::<BTreeSet<_>>();
    assert_eq!(
        emitted,
        expanded
            .declaration_provenance
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>()
    );

    let rise = expanded
        .writable_points
        .iter()
        .find(|point| match &point.edit {
            CodePointEdit::Point { address } => matches!(
                &address.owner.address,
                geosolve_sketch_code::CodeOwnerAddress::GeneratedMember { address }
                    if address.invocation == "path" && address.member_key == ["rise"]
            ),
            CodePointEdit::RectangleCorner { .. } => false,
        })
        .unwrap();
    let child = expanded.generated_children.first().unwrap();
    let mut overlay = rise
        .stage_drag(&CodeInteractionOverlay::empty(), [22.0, 2.0])
        .unwrap();
    overlay
        .set_generated_child_suppressed(child.address.clone(), true)
        .unwrap();
    let materialized = materialize_code_project_cold_with_overlay(
        &project,
        &generated,
        &overlay,
        IntentSessionId::from_raw(0x84f0_5021),
        DocumentId(PersistentId::from_u128(0x84f0_5021)),
        1.0,
    )
    .unwrap();
    assert_valid(&materialized);
    let moved = materialized
        .expansion
        .writable_points
        .iter()
        .find(|point| point.edit == rise.edit)
        .unwrap();
    assert_eq!(point_position(&materialized, &moved.handle), [22.0, 2.0]);
    let suppressed = materialized
        .expansion
        .generated_children
        .iter()
        .find(|candidate| candidate.address == child.address)
        .unwrap();
    assert!(suppressed.suppressed);
    let owner = materialized.host_outputs[match &child.address.owner.address {
        geosolve_sketch_code::CodeOwnerAddress::GeneratedMember { address } => address,
        geosolve_sketch_code::CodeOwnerAddress::DirectDeclaration { .. } => unreachable!(),
    }][0]
        .owner;
    assert!(
        materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .features
            .feature(owner.feature)
            .unwrap()
            .suppressed
    );

    let mounting = demos
        .iter()
        .find(|demo| demo.id == CodeProjectDemoId::MountingPlate)
        .unwrap()
        .project();
    let mounting_generated = reconciled(&mounting);
    let mounting_expansion = expansion(&mounting, &mounting_generated, 0x84f0_5022);
    let mounting_point = mounting_expansion
        .writable_points
        .iter()
        .find(|point| {
            matches!(&point.edit, CodePointEdit::Point { address }
            if matches!(&address.owner.address,
                geosolve_sketch_code::CodeOwnerAddress::GeneratedMember { address }
                    if address.invocation == "plate" && address.output != ["profile"]))
        })
        .unwrap();
    let mounting_overlay = mounting_point
        .stage_drag(&CodeInteractionOverlay::empty(), [-28.0, 17.0])
        .unwrap();
    let mounting_materialized = materialize_code_project_cold_with_overlay(
        &mounting,
        &mounting_generated,
        &mounting_overlay,
        IntentSessionId::from_raw(0x84f0_5023),
        DocumentId(PersistentId::from_u128(0x84f0_5023)),
        1.0,
    )
    .unwrap();
    let moved_mounting_point = mounting_materialized
        .expansion
        .writable_points
        .iter()
        .find(|point| point.edit == mounting_point.edit)
        .unwrap();
    assert_eq!(
        point_position(&mounting_materialized, &moved_mounting_point.handle),
        [-28.0, 17.0]
    );
}

#[test]
fn same_owner_seed_precedence_resets_overlay_then_legacy_override_then_source() {
    let project = bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == CodeProjectDemoId::RoundedPolyline)
        .unwrap()
        .project();
    let mut generated = reconciled(&project);
    let shoulder_address = generated
        .active()
        .keys()
        .find(|address| {
            address.template == ["polyline", "vertex"] && address.member_key == ["shoulder"]
        })
        .unwrap()
        .clone();
    generated
        .set_override(
            &shoulder_address,
            geosolve_sketch_code::ManagedValue::Array(vec![
                geosolve_sketch_code::ManagedValue::Number(25.0),
                geosolve_sketch_code::ManagedValue::Number(13.0),
            ]),
        )
        .unwrap();
    let legacy_expansion = expansion(&project, &generated, 0x84f0_5024);
    let shoulder = legacy_expansion
        .writable_points
        .iter()
        .find(|point| {
            matches!(&point.edit, CodePointEdit::Point { address }
                if matches!(&address.owner.address,
                    geosolve_sketch_code::CodeOwnerAddress::GeneratedMember { address }
                        if address == &shoulder_address))
        })
        .unwrap()
        .clone();
    let overlay = shoulder
        .stage_drag(&CodeInteractionOverlay::empty(), [26.0, 14.0])
        .unwrap();

    let materialize =
        |generated: &KeyedReconcileState, overlay: &CodeInteractionOverlay, seed: u128| {
            materialize_code_project_cold_with_overlay(
                &project,
                generated,
                overlay,
                IntentSessionId::from_raw(seed),
                DocumentId(PersistentId::from_u128(seed)),
                1.0,
            )
            .unwrap()
        };
    let overlaid = materialize(&generated, &overlay, 0x84f0_5025);
    let overlaid_shoulder = overlaid
        .expansion
        .writable_points
        .iter()
        .find(|point| point.edit == shoulder.edit)
        .unwrap();
    assert_eq!(
        point_position(&overlaid, &overlaid_shoulder.handle),
        [26.0, 14.0],
        "semantic overlay must win over the same owner's legacy override",
    );

    let legacy = materialize(&generated, &CodeInteractionOverlay::empty(), 0x84f0_5026);
    let legacy_shoulder = legacy
        .expansion
        .writable_points
        .iter()
        .find(|point| point.edit == shoulder.edit)
        .unwrap();
    assert_eq!(
        point_position(&legacy, &legacy_shoulder.handle),
        [25.0, 13.0],
        "resetting only the overlay must reveal the legacy override tier",
    );

    assert!(generated.reset_to_code(&shoulder_address).unwrap());
    let source = materialize(&generated, &CodeInteractionOverlay::empty(), 0x84f0_5027);
    let source_shoulder = source
        .expansion
        .writable_points
        .iter()
        .find(|point| {
            matches!(&point.edit, CodePointEdit::Point { address }
                if matches!(&address.owner.address,
                    geosolve_sketch_code::CodeOwnerAddress::GeneratedMember { address }
                        if address == &shoulder_address))
        })
        .unwrap();
    assert_eq!(
        point_position(&source, &source_shoulder.handle),
        [24.0, 12.0],
        "resetting the legacy override must reveal the managed source seed",
    );
}

#[test]
fn stale_unknown_and_structurally_removed_overlay_rows_fail_or_prune_exactly() {
    let project = direct_project();
    let generated = reconciled(&project);
    let expanded = expansion(&project, &generated, 0x84f0_5030);
    let diagonal = point_for_field(&expanded, "diagonal", "start");
    let frame = expanded
        .writable_points
        .iter()
        .find(|point| {
            matches!(
                &point.edit,
                CodePointEdit::RectangleCorner {
                    corner: CodeRectangleCorner::LowerLeft,
                    ..
                }
            )
        })
        .unwrap();
    let overlay = stage_point_drags(
        &CodeInteractionOverlay::empty(),
        [(diagonal, [4.0, 5.0]), (frame, [-2.0, -3.0])],
    )
    .unwrap();

    let mut stale = CodeInteractionOverlay::empty();
    let CodePointEdit::Point { mut address } = diagonal.edit.clone() else {
        unreachable!()
    };
    address.owner.generation = address.owner.generation.saturating_add(1);
    stale
        .set_point(address, [4.0, 5.0], CodeDraftProvenance::DetachedReference)
        .unwrap();
    assert!(matches!(
        expand_code_project_with_overlay(&project, &generated, &stale, expanded.patch.expected,),
        Err(CodeExpansionError::StaleDraftGeneration { .. })
    ));

    let CodePointEdit::Point {
        address: referenced_address,
    } = &diagonal.edit
    else {
        unreachable!()
    };
    let mut wrong_reference_provenance = CodeInteractionOverlay::empty();
    wrong_reference_provenance
        .set_point(
            referenced_address.clone(),
            [4.0, 5.0],
            CodeDraftProvenance::CanvasDrag,
        )
        .unwrap();
    assert!(matches!(
        expand_code_project_with_overlay(
            &project,
            &generated,
            &wrong_reference_provenance,
            expanded.patch.expected,
        ),
        Err(CodeExpansionError::Unsupported(_))
    ));

    let leader = point_for_field(&expanded, "leader", "start");
    let CodePointEdit::Point {
        address: literal_address,
    } = &leader.edit
    else {
        unreachable!()
    };
    let mut wrong_literal_provenance = CodeInteractionOverlay::empty();
    wrong_literal_provenance
        .set_point(
            literal_address.clone(),
            [-7.0, 3.0],
            CodeDraftProvenance::DetachedReference,
        )
        .unwrap();
    assert!(matches!(
        expand_code_project_with_overlay(
            &project,
            &generated,
            &wrong_literal_provenance,
            expanded.patch.expected,
        ),
        Err(CodeExpansionError::Unsupported(_))
    ));

    let deletion = plan_managed_edit(
        &project.managed,
        ManagedEdit::DeleteDeclaration {
            declaration: SemanticSymbol("diagonal".into()),
        },
    )
    .unwrap();
    let mut candidate = project.clone();
    candidate.managed = apply_managed_edit(&project.managed, &deletion).unwrap();
    let plan = generated
        .plan(
            required_generated_members(&candidate).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    let (candidate_expansion, retained) = expand_code_project_for_structural_edit(
        &candidate,
        plan.staged(),
        &overlay,
        expanded.patch.expected,
    )
    .unwrap();
    assert_eq!(
        retained.drafts().len(),
        2,
        "the rectangle's two coupled seeds survive"
    );
    assert!(retained.drafts().keys().all(|address| {
        matches!(&address.owner.address,
            geosolve_sketch_code::CodeOwnerAddress::DirectDeclaration { declaration }
                if declaration.0 == "frame")
    }));
    assert!(candidate_expansion.writable_points.iter().all(|point| {
        candidate_expansion.declaration_for_alias(&point.handle.alias)
            != Some(&SemanticSymbol("diagonal".into()))
    }));

    let mut session = SketchCodeSession::new_project(
        project.clone(),
        generated.clone(),
        expanded.clone(),
        serde_json::json!({ "checkpoint": "base" }),
    )
    .unwrap();
    let overlaid_expansion =
        expand_code_project_with_overlay(&project, &generated, &overlay, expanded.patch.expected)
            .unwrap();
    let prepared = session
        .prepare_project_overlay(
            session.identity(),
            overlay.clone(),
            overlaid_expansion,
            serde_json::json!({ "checkpoint": "dragged" }),
            "Drag coupled frame corner and diagonal",
        )
        .unwrap();
    session.apply_prepared(prepared).unwrap();
    let retained_plan = session
        .plan_structural_reconciliation(
            session.identity(),
            required_generated_members(&candidate).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    let error = session
        .prepare_project_retained_failure(
            session.identity(),
            candidate.clone(),
            Some(retained_plan),
            overlay.clone(),
            Some(candidate_expansion.clone()),
            serde_json::json!({ "checkpoint": "retained failure" }),
            "native validation",
            "candidate deliberately rejected after expansion",
            "Reject structural candidate",
        )
        .unwrap_err();
    assert!(error.to_string().contains("deterministic owner-pruned"));
    let retained_plan = session
        .plan_structural_reconciliation(
            session.identity(),
            required_generated_members(&candidate).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    session
        .prepare_project_retained_failure(
            session.identity(),
            candidate.clone(),
            Some(retained_plan),
            retained.clone(),
            Some(candidate_expansion.clone()),
            serde_json::json!({ "checkpoint": "retained failure" }),
            "native validation",
            "candidate deliberately rejected after expansion",
            "Reject structural candidate",
        )
        .unwrap();

    let literal_overlay = leader
        .stage_drag(&CodeInteractionOverlay::empty(), [-7.0, 3.0])
        .unwrap();
    let reference_edit = plan_managed_edit(
        &project.managed,
        ManagedEdit::SetInvocationArgument {
            declaration: SemanticSymbol("leader".into()),
            path: vec!["start".into()],
            value: geosolve_sketch_code::ManagedValue::Reference {
                declaration: SemanticSymbol("frame".into()),
                path: SemanticOutputPath(vec![
                    ManagedPathSegment::Field("corners".into()),
                    ManagedPathSegment::Field("lowerLeft".into()),
                ]),
            },
        },
    )
    .unwrap();
    let mut referenced_candidate = project.clone();
    referenced_candidate.managed = apply_managed_edit(&project.managed, &reference_edit).unwrap();
    let referenced_plan = generated
        .plan(
            required_generated_members(&referenced_candidate).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    assert!(matches!(
        expand_code_project_with_overlay(
            &referenced_candidate,
            referenced_plan.staged(),
            &literal_overlay,
            expanded.patch.expected,
        ),
        Err(CodeExpansionError::Unsupported(_))
    ));
    let (_, retained) = expand_code_project_for_structural_edit(
        &referenced_candidate,
        referenced_plan.staged(),
        &literal_overlay,
        expanded.patch.expected,
    )
    .unwrap();
    assert!(retained.drafts().is_empty());
}

#[test]
fn overlay_publication_persistence_undo_and_redo_are_one_atomic_session_authority() {
    let project = direct_project();
    let generated = reconciled(&project);
    let expanded = expansion(&project, &generated, 0x84f0_5040);
    let point = point_for_field(&expanded, "leader", "start").clone();
    let mut session = SketchCodeSession::new_project(
        project.clone(),
        generated.clone(),
        expanded,
        serde_json::json!({ "checkpoint": "base" }),
    )
    .unwrap();
    let overlay = session.stage_point_drag(&point, [-8.0, 6.0]).unwrap();
    let accepted = session.snapshot().accepted_expansion.as_ref().unwrap();
    let next =
        expand_code_project_with_overlay(&project, &generated, &overlay, accepted.patch.expected)
            .unwrap();
    let prepared = session
        .prepare_project_overlay(
            session.identity(),
            overlay.clone(),
            next,
            serde_json::json!({ "checkpoint": "dragged" }),
            "Drag code point",
        )
        .unwrap();
    session.apply_prepared(prepared).unwrap();
    assert_eq!(session.snapshot().interaction_overlay, overlay);
    let persisted = session.to_canonical_json().unwrap();
    assert_eq!(
        SketchCodeSession::from_json(&persisted)
            .unwrap()
            .snapshot()
            .interaction_overlay,
        overlay
    );
    session.undo().unwrap();
    assert_eq!(
        session.snapshot().interaction_overlay,
        CodeInteractionOverlay::empty()
    );
    assert_eq!(
        session.snapshot().editor_checkpoint,
        serde_json::json!({ "checkpoint": "base" })
    );
    session.redo().unwrap();
    assert_eq!(session.snapshot().interaction_overlay, overlay);
    assert_eq!(
        session.snapshot().editor_checkpoint,
        serde_json::json!({ "checkpoint": "dragged" })
    );
}

#[test]
fn rectangle_lenses_are_generation_exact_and_reset_the_coupled_seed_bundle() {
    let project = direct_project();
    let generated = reconciled(&project);
    let expanded = expansion(&project, &generated, 0x84f0_5050);
    let stale_corner = expanded
        .writable_points
        .iter()
        .find(|point| {
            matches!(
                point.edit,
                CodePointEdit::RectangleCorner {
                    corner: CodeRectangleCorner::LowerRight,
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    let mut session = SketchCodeSession::new_project(
        project.clone(),
        generated.clone(),
        expanded,
        serde_json::json!({ "checkpoint": "base" }),
    )
    .unwrap();
    let overlay = session
        .stage_point_drag(&stale_corner, [64.0, -6.0])
        .unwrap();
    assert_eq!(overlay.drafts().len(), 2);
    let accepted = session.snapshot().accepted_expansion.as_ref().unwrap();
    let moved =
        expand_code_project_with_overlay(&project, &generated, &overlay, accepted.patch.expected)
            .unwrap();
    let prepared = session
        .prepare_project_overlay(
            session.identity(),
            overlay,
            moved,
            serde_json::json!({ "checkpoint": "moved" }),
            "Move rectangle corner",
        )
        .unwrap();
    session.apply_prepared(prepared).unwrap();

    assert!(matches!(
        session.stage_point_drag(&stale_corner, [65.0, -7.0]),
        Err(geosolve_sketch_code::CodeSessionError::UnknownSemanticOwner(_))
    ));
    let fresh_corner = session
        .snapshot()
        .accepted_expansion
        .as_ref()
        .unwrap()
        .writable_points
        .iter()
        .find(|point| {
            matches!(
                point.edit,
                CodePointEdit::RectangleCorner {
                    corner: CodeRectangleCorner::LowerRight,
                    ..
                }
            )
        })
        .unwrap()
        .clone();
    session
        .stage_point_drag(&fresh_corner, [65.0, -7.0])
        .unwrap();

    let reset_expansion = expand_code_project_with_overlay(
        &project,
        &generated,
        &CodeInteractionOverlay::empty(),
        session
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap()
            .patch
            .expected,
    )
    .unwrap();
    let address = fresh_corner.edit.writable_addresses()[0].clone();
    let prepared = session
        .prepare_project_reset_draft(
            session.identity(),
            &address,
            reset_expansion,
            serde_json::json!({ "checkpoint": "reset" }),
            "Reset rectangle placement",
        )
        .unwrap()
        .unwrap();
    session.apply_prepared(prepared).unwrap();
    assert!(session.snapshot().interaction_overlay.drafts().is_empty());
}

#[test]
fn deleting_the_final_managed_declaration_materializes_an_authenticated_empty_project() {
    let source = r#"// SPDX-License-Identifier: GPL-3.0-or-later

"use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const line = $.geometry.line("line", { start: [0, 0], end: [20, 10] });
  return $.outputs({ line });
});
"#;
    let project = CodeProject::managed_only(ProjectKey("m84-empty-delete".into()), source).unwrap();
    let generated = reconciled(&project);
    let initial = materialize_code_project_cold_with_overlay(
        &project,
        &generated,
        &CodeInteractionOverlay::empty(),
        IntentSessionId::from_raw(0x84f0_5060),
        DocumentId(PersistentId::from_u128(0x84f0_5060)),
        1.0,
    )
    .unwrap();
    let line_alias = initial
        .expansion
        .declaration_provenance
        .iter()
        .find_map(|(alias, declaration)| (declaration.0 == "line").then(|| alias.clone()))
        .unwrap();
    let deletion = plan_managed_edit(
        &project.managed,
        ManagedEdit::DeleteDeclaration {
            declaration: SemanticSymbol("line".into()),
        },
    )
    .unwrap();
    let mut empty_project = project.clone();
    empty_project.managed = apply_managed_edit(&project.managed, &deletion).unwrap();
    let plan = generated
        .plan(
            required_generated_members(&empty_project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    let (empty, retained) = materialize_code_project_incremental_for_structural_edit(
        &initial,
        &empty_project,
        plan.staged(),
        &CodeInteractionOverlay::empty(),
    )
    .unwrap();
    assert_eq!(retained, CodeInteractionOverlay::empty());
    assert!(empty.expansion.patch.operations().is_empty());
    assert!(empty.expansion.declaration_provenance.is_empty());
    assert!(
        empty
            .editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&line_alias)
            .is_none()
    );
    assert_valid(&empty);
}
