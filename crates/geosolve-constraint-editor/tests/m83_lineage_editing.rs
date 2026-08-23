// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstraintActionRequest, ConstraintIntent, ConstructionPoint, ConstructionProposal,
    LineageReorderBlockReason, RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    DocumentDimensionMode, DocumentEdit, DocumentSolveRequest, RetainedSketchDocumentSession,
    SketchDocument, SolverConfig,
};
use geosolve_sketch_lineage::{
    LineageActionDefinition, LineageActionKind, LineageStepId, LineageStepState,
};

fn empty_coordinator() -> RetainedEditorCoordinator {
    let session = RetainedSketchDocumentSession::new(
        SketchDocument::new(10.0).expect("document"),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("accepted empty session");
    RetainedEditorCoordinator::new(session).expect("coordinator")
}

fn add_point(
    coordinator: &mut RetainedEditorCoordinator,
    position: [f64; 2],
) -> (geosolve_sketch::DesignPointId, LineageStepId) {
    let point = coordinator
        .apply_construction(
            coordinator.session().design_identity(),
            &ConstructionProposal::Point {
                point: ConstructionPoint::New(position),
            },
        )
        .expect("authored point")
        .value
        .points[0];
    let owner = coordinator
        .lineage_document()
        .steps()
        .last()
        .expect("authored owner")
        .id;
    (point, owner)
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one identity stress keeps reorder, Undo/Redo and same-owner direct editing evidence adjacent"
)]
fn independent_authored_steps_reorder_with_stable_identity_and_one_history_position() {
    let mut coordinator = empty_coordinator();
    let (first_point, first_owner) = add_point(&mut coordinator, [1.0, 2.0]);
    let (_, second_owner) = add_point(&mut coordinator, [4.0, 5.0]);
    let baseline = coordinator.lineage_document().steps()[0].id;
    let first = coordinator
        .lineage_document()
        .step(first_owner)
        .expect("first owner")
        .clone();
    let second = coordinator
        .lineage_document()
        .step(second_owner)
        .expect("second owner")
        .clone();
    let history = (coordinator.history_len(), coordinator.history_cursor());

    let availability = coordinator
        .lineage_reorder_availability(second_owner)
        .expect("reorder availability");
    assert!(
        availability
            .lanes
            .iter()
            .any(|lane| lane.before == Some(first_owner) && lane.is_legal())
    );
    assert!(availability.lanes.iter().any(|lane| {
        lane.before == Some(baseline)
            && lane.blocked_by == Some(LineageReorderBlockReason::ImportedBaselinePinned)
    }));

    let outcome = coordinator
        .reorder_lineage_step(
            coordinator.lineage_identity(),
            second_owner,
            Some(first_owner),
        )
        .expect("independent reorder");
    assert!(outcome.changed, "{outcome:#?}");
    assert!(outcome.accepted);
    assert!(!outcome.clamped);
    assert_eq!(coordinator.history_len(), history.0 + 1);
    assert_eq!(coordinator.history_cursor(), history.1 + 1);
    assert_eq!(
        coordinator
            .lineage_document()
            .steps()
            .iter()
            .map(|step| step.id)
            .collect::<Vec<_>>(),
        vec![baseline, second_owner, first_owner]
    );
    assert_eq!(
        coordinator.lineage_document().step(first_owner),
        Some(&first),
        "reorder must preserve the complete first owner manifest"
    );
    assert_eq!(
        coordinator.lineage_document().step(second_owner),
        Some(&second),
        "reorder must preserve the complete second owner manifest"
    );

    coordinator.undo().expect("undo reorder");
    assert_eq!(
        coordinator
            .lineage_document()
            .steps()
            .iter()
            .map(|step| step.id)
            .collect::<Vec<_>>(),
        vec![baseline, first_owner, second_owner]
    );
    coordinator.redo().expect("redo reorder");
    assert_eq!(
        coordinator
            .lineage_document()
            .steps()
            .iter()
            .map(|step| step.id)
            .collect::<Vec<_>>(),
        vec![baseline, second_owner, first_owner]
    );

    let owner_before_edit = coordinator
        .lineage_document()
        .step(first_owner)
        .expect("owner after reorder")
        .clone();
    coordinator
        .apply_edit(
            coordinator.session().design_identity(),
            DocumentEdit::SetPointPosition {
                point: first_point,
                position: [2.5, 3.5],
            },
        )
        .expect("direct edit after reorder");
    let owner_after_edit = coordinator
        .lineage_document()
        .step(first_owner)
        .expect("same owner after edit");
    assert_eq!(owner_after_edit.id, owner_before_edit.id);
    assert_eq!(owner_after_edit.outputs, owner_before_edit.outputs);
    assert_eq!(
        owner_after_edit.reservations,
        owner_before_edit.reservations
    );
    assert_ne!(owner_after_edit.action, owner_before_edit.action);
}

#[test]
fn dependency_inversion_clamps_at_the_authoritative_consumer_boundary() {
    let mut coordinator = empty_coordinator();
    let (first_point, first_owner) = add_point(&mut coordinator, [0.0, 0.0]);
    let (second_point, second_owner) = add_point(&mut coordinator, [2.0, 1.0]);
    coordinator
        .apply_constraint_action_for(
            coordinator.session().design_identity(),
            &[
                SelectionItem::Point(first_point),
                SelectionItem::Point(second_point),
            ],
            ConstraintActionRequest {
                intent: ConstraintIntent::Coincident,
                label: "dependent relation".into(),
                contacts: Vec::new(),
                relation: None,
            },
        )
        .expect("dependent relation");
    let relation_owner = coordinator.lineage_document().steps()[3].id;
    let history = (coordinator.history_len(), coordinator.history_cursor());

    let outcome = coordinator
        .reorder_lineage_step(coordinator.lineage_identity(), first_owner, None)
        .expect("clamped dependency reorder");
    assert!(outcome.changed);
    assert!(outcome.clamped);
    assert_eq!(outcome.applied_before, Some(relation_owner));
    assert_eq!(
        outcome.boundary,
        Some(LineageReorderBlockReason::RequiredByDependent {
            dependent: relation_owner
        })
    );
    assert_eq!(coordinator.history_len(), history.0 + 1);
    assert_eq!(coordinator.history_cursor(), history.1 + 1);
    assert_eq!(
        coordinator
            .lineage_document()
            .steps()
            .iter()
            .map(|step| step.id)
            .collect::<Vec<_>>(),
        vec![
            coordinator.lineage_document().steps()[0].id,
            second_owner,
            first_owner,
            relation_owner,
        ]
    );

    let session_json = coordinator
        .lineage_session_json()
        .expect("persist reordered lineage");
    let ledger_json = coordinator
        .lineage_host_input_ledger_json()
        .expect("persist host inputs");
    let mut restored = RetainedEditorCoordinator::new(coordinator.session().clone())
        .expect("same-document restore target");
    restored
        .restore_lineage_session_and_host_input_ledger_json(&session_json, &ledger_json)
        .expect("cold reload reordered lineage");
    assert_eq!(
        restored
            .lineage_document()
            .steps()
            .iter()
            .map(|step| step.id)
            .collect::<Vec<_>>(),
        coordinator
            .lineage_document()
            .steps()
            .iter()
            .map(|step| step.id)
            .collect::<Vec<_>>()
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one chronology stress keeps constraint/dimension identity and accepted-byte evidence adjacent"
)]
fn independent_constraint_and_dimension_reorder_without_retargeting_identity() {
    let mut coordinator = empty_coordinator();
    let (first, _) = add_point(&mut coordinator, [0.0, 0.0]);
    let (second, _) = add_point(&mut coordinator, [2.0, 0.0]);
    let (third, _) = add_point(&mut coordinator, [0.0, 3.0]);
    let (fourth, _) = add_point(&mut coordinator, [4.0, 3.0]);
    let relation = coordinator
        .apply_constraint_action_for(
            coordinator.session().design_identity(),
            &[SelectionItem::Point(first), SelectionItem::Point(second)],
            ConstraintActionRequest {
                intent: ConstraintIntent::Horizontal,
                label: "independent horizontal relation".into(),
                contacts: Vec::new(),
                relation: None,
            },
        )
        .expect("independent relation")
        .value;
    let relation_owner = coordinator
        .lineage_document()
        .steps()
        .last()
        .expect("relation owner")
        .id;
    coordinator.set_selection([SelectionItem::Point(third), SelectionItem::Point(fourth)]);
    let dimension = coordinator
        .add_point_distance_dimension(
            coordinator.session().design_identity(),
            DocumentDimensionMode::Reference,
            "independent reference distance",
        )
        .expect("independent dimension")
        .value;
    let dimension_owner = coordinator
        .lineage_document()
        .steps()
        .last()
        .expect("dimension owner")
        .id;
    let relation_step = coordinator
        .lineage_document()
        .step(relation_owner)
        .expect("relation step")
        .clone();
    let dimension_step = coordinator
        .lineage_document()
        .step(dimension_owner)
        .expect("dimension step")
        .clone();
    let relation_source = coordinator
        .session()
        .design_document()
        .constraint(relation)
        .expect("relation before reorder")
        .source_id;
    let dimension_source = coordinator
        .session()
        .design_document()
        .dimension(dimension)
        .expect("dimension before reorder")
        .source_id;
    assert_eq!(
        coordinator.session().design_document().source_order(),
        &[relation_source, dimension_source],
        "native source chronology initially follows authored step order"
    );
    let history = (coordinator.history_len(), coordinator.history_cursor());

    let outcome = coordinator
        .reorder_lineage_step(
            coordinator.lineage_identity(),
            dimension_owner,
            Some(relation_owner),
        )
        .expect("dependency-aware semantic reorder");
    assert!(outcome.changed, "{outcome:#?}");
    assert!(outcome.accepted);
    assert!(!outcome.clamped, "{outcome:#?}");
    assert_eq!(outcome.boundary, None);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        (history.0 + 1, history.1 + 1),
        "an independent reorder is one history position"
    );
    let steps = coordinator.lineage_document().steps();
    let relation_ordinal = steps
        .iter()
        .position(|step| step.id == relation_owner)
        .expect("relation owner after reorder");
    let dimension_ordinal = steps
        .iter()
        .position(|step| step.id == dimension_owner)
        .expect("dimension owner after reorder");
    assert!(dimension_ordinal < relation_ordinal);
    assert_eq!(
        coordinator.lineage_document().step(relation_owner),
        Some(&relation_step),
        "relation identity and complete ownership manifest remain exact"
    );
    assert_eq!(
        coordinator.lineage_document().step(dimension_owner),
        Some(&dimension_step),
        "dimension identity and complete ownership manifest remain exact"
    );
    assert!(
        coordinator
            .session()
            .design_document()
            .constraint(relation)
            .is_some()
    );
    assert!(
        coordinator
            .session()
            .design_document()
            .dimension(dimension)
            .is_some()
    );
    assert_eq!(
        coordinator.session().design_document().source_order(),
        &[dimension_source, relation_source],
        "reordering independent owners must reorder the exact surviving native source IDs"
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .constraint(relation)
            .expect("stable relation after reorder")
            .source_id,
        relation_source
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .dimension(dimension)
            .expect("stable dimension after reorder")
            .source_id,
        dimension_source
    );

    coordinator.undo().expect("undo independent reorder");
    assert_eq!(
        coordinator.lineage_document().step(relation_owner),
        Some(&relation_step)
    );
    assert_eq!(
        coordinator.lineage_document().step(dimension_owner),
        Some(&dimension_step)
    );
    assert_eq!(
        coordinator.session().design_document().source_order(),
        &[relation_source, dimension_source],
        "Undo restores native source chronology without reallocating IDs"
    );
    coordinator.redo().expect("redo independent reorder");
    assert_eq!(
        coordinator.lineage_document().step(relation_owner),
        Some(&relation_step)
    );
    assert_eq!(
        coordinator.lineage_document().step(dimension_owner),
        Some(&dimension_step)
    );
    assert_eq!(
        coordinator.session().design_document().source_order(),
        &[dimension_source, relation_source],
        "Redo reapplies native source chronology without reallocating IDs"
    );

    let session_json = coordinator
        .lineage_session_json()
        .expect("persist reordered lineage");
    let ledger_json = coordinator
        .lineage_host_input_ledger_json()
        .expect("persist reordered host inputs");
    let mut restored = RetainedEditorCoordinator::new(coordinator.session().clone())
        .expect("same-document restore target");
    restored
        .restore_lineage_session_and_host_input_ledger_json(&session_json, &ledger_json)
        .expect("cold reload independent reorder");
    assert_eq!(
        restored.lineage_document().step(relation_owner),
        Some(&relation_step)
    );
    assert_eq!(
        restored.lineage_document().step(dimension_owner),
        Some(&dimension_step)
    );
    let restored_steps = restored.lineage_document().steps();
    assert!(
        restored_steps
            .iter()
            .position(|step| step.id == dimension_owner)
            .expect("restored dimension owner")
            < restored_steps
                .iter()
                .position(|step| step.id == relation_owner)
                .expect("restored relation owner")
    );
    assert!(
        restored
            .session()
            .design_document()
            .constraint(relation)
            .is_some()
    );
    assert!(
        restored
            .session()
            .design_document()
            .dimension(dimension)
            .is_some()
    );
    assert_eq!(
        restored.session().design_document().source_order(),
        &[dimension_source, relation_source],
        "cold reload preserves reordered native chronology and exact source identities"
    );
    assert_eq!(
        restored
            .session()
            .design_document()
            .constraint(relation)
            .expect("restored relation")
            .source_id,
        relation_source
    );
    assert_eq!(
        restored
            .session()
            .design_document()
            .dimension(dimension)
            .expect("restored dimension")
            .source_id,
        dimension_source
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed matrix keeps pinned, stale, malformed and hostile edit authority adjacent"
)]
fn pinned_rows_and_debug_rewrite_fail_closed_while_label_edit_is_atomic() {
    let mut coordinator = empty_coordinator();
    let baseline = coordinator.lineage_document().steps()[0].id;
    let (_, owner) = add_point(&mut coordinator, [1.0, 1.0]);
    let history = (coordinator.history_len(), coordinator.history_cursor());

    let pinned = coordinator
        .reorder_lineage_step(coordinator.lineage_identity(), baseline, None)
        .expect("pinned baseline clamps");
    assert!(!pinned.changed);
    assert!(pinned.clamped);
    assert_eq!(
        pinned.boundary,
        Some(LineageReorderBlockReason::ImportedBaselinePinned)
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        history
    );

    let inspection = coordinator
        .lineage_step_inspection(owner)
        .expect("inspect authored point");
    assert_eq!(inspection.action_kind, LineageActionKind::GeometryRecipe);
    let mut replacement = serde_json::from_str::<serde_json::Value>(&inspection.replacement_json)
        .expect("strict replacement JSON");
    replacement["label"] = serde_json::Value::String("Renamed point action".into());
    let outcome = coordinator
        .rewrite_lineage_step_json(
            inspection.lineage,
            owner,
            &serde_json::to_string(&replacement).expect("replacement JSON"),
        )
        .expect("label rewrite");
    assert!(outcome.changed);
    assert!(outcome.accepted);
    assert_eq!(
        coordinator
            .lineage_document()
            .step(owner)
            .map(|step| step.label.as_str()),
        Some("Renamed point action")
    );
    assert_eq!(coordinator.history_len(), history.0 + 1);

    let current_inspection = coordinator
        .lineage_step_inspection(owner)
        .expect("inspect renamed action");
    let renamed_history = (coordinator.history_len(), coordinator.history_cursor());
    let no_op = coordinator
        .rewrite_lineage_step_json(
            current_inspection.lineage,
            owner,
            &current_inspection.replacement_json,
        )
        .expect("exact rewrite no-op");
    assert!(!no_op.changed);
    assert!(no_op.accepted);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        renamed_history,
        "an exact rewrite no-op must not create history"
    );
    assert!(
        coordinator
            .rewrite_lineage_step_json(
                inspection.lineage,
                owner,
                &current_inspection.replacement_json,
            )
            .is_err(),
        "the pre-rename lineage identity must be stale"
    );
    assert!(
        coordinator
            .reorder_lineage_step(
                coordinator.lineage_identity(),
                owner,
                Some(LineageStepId::from_raw(u64::MAX)),
            )
            .is_err(),
        "an unknown stable anchor must fail atomically"
    );
    assert!(
        coordinator
            .reorder_lineage_step(coordinator.lineage_identity(), owner, Some(owner))
            .is_err(),
        "a step cannot be its own insertion anchor"
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        renamed_history
    );

    let retained = coordinator
        .lineage_session_json()
        .expect("retained before hostile edits");
    let retained_history = (coordinator.history_len(), coordinator.history_cursor());
    assert!(
        coordinator
            .rewrite_lineage_step_json(coordinator.lineage_identity(), owner, "{")
            .is_err()
    );
    assert_eq!(
        coordinator
            .lineage_session_json()
            .expect("unchanged malformed"),
        retained
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        retained_history
    );

    let inspection = coordinator
        .lineage_step_inspection(owner)
        .expect("inspect renamed action");
    let mut hostile = serde_json::from_str::<serde_json::Value>(&inspection.replacement_json)
        .expect("strict replacement JSON");
    hostile["action"] = serde_json::to_value(LineageActionDefinition::Annotation {
        action: geosolve_sketch_lineage::VersionedActionPayload::empty(
            geosolve_sketch_lineage::LineageSemanticKey::new("hostile.annotation").expect("schema"),
            1,
        ),
    })
    .expect("hostile action JSON");
    assert!(
        coordinator
            .rewrite_lineage_step_json(
                inspection.lineage,
                owner,
                &serde_json::to_string(&hostile).expect("hostile replacement"),
            )
            .is_err()
    );
    assert_eq!(
        coordinator
            .lineage_session_json()
            .expect("unchanged hostile"),
        retained
    );
    assert_eq!(
        coordinator
            .lineage_document()
            .step(owner)
            .map(|step| step.state),
        Some(LineageStepState::Live)
    );

    let (deleted_point, deleted_owner) = add_point(&mut coordinator, [3.0, 3.0]);
    coordinator.set_selection([SelectionItem::Point(deleted_point)]);
    coordinator
        .delete_selected(coordinator.session().design_identity())
        .expect("delete authored point owner");
    assert_eq!(
        coordinator
            .lineage_document()
            .step(deleted_owner)
            .map(|step| step.state),
        Some(LineageStepState::Tombstoned)
    );
    let deleted_history = (coordinator.history_len(), coordinator.history_cursor());
    let deleted_reorder = coordinator
        .reorder_lineage_step(coordinator.lineage_identity(), deleted_owner, Some(owner))
        .expect("tombstoned owner clamps to its pinned position");
    assert!(!deleted_reorder.changed);
    assert!(deleted_reorder.clamped);
    assert_eq!(
        deleted_reorder.boundary,
        Some(LineageReorderBlockReason::TombstonedPinned)
    );
    let deleted_inspection = coordinator
        .lineage_step_inspection(deleted_owner)
        .expect("inspect tombstoned owner");
    assert!(
        coordinator
            .rewrite_lineage_step_json(
                deleted_inspection.lineage,
                deleted_owner,
                &deleted_inspection.replacement_json,
            )
            .is_err(),
        "tombstoned owners are read-only"
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        deleted_history
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one positive raw-action rewrite contract keeps strict-cold, history and reload evidence adjacent"
)]
fn compatible_raw_action_rewrite_rematerializes_the_same_owner_and_survives_history() {
    let mut coordinator = empty_coordinator();
    let (point, owner) = add_point(&mut coordinator, [1.0, 2.0]);
    let original = coordinator
        .lineage_document()
        .step(owner)
        .expect("original point owner")
        .clone();
    let original_steps = coordinator.lineage_document().steps().len();

    // Produce a genuinely compatible material action through the ordinary
    // owner-rewrite path, then submit that action to a separate coordinator
    // through the raw Inspector API. This avoids reproducing the private
    // semantic-output compiler in the test while proving that the public raw
    // action path accepts more than label-only edits.
    let retained = coordinator
        .lineage_session_json()
        .expect("initial retained lineage");
    let ledger = coordinator
        .lineage_host_input_ledger_json()
        .expect("initial host-input ledger");
    let mut donor = RetainedEditorCoordinator::new(coordinator.session().clone())
        .expect("compatible rewrite donor");
    donor
        .restore_lineage_session_and_host_input_ledger_json(&retained, &ledger)
        .expect("restore exact donor lineage");
    donor
        .apply_edit(
            donor.session().design_identity(),
            DocumentEdit::SetPointPosition {
                point,
                position: [3.0, 4.0],
            },
        )
        .expect("ordinary compatible owner rewrite");
    let replacement = donor
        .lineage_step_inspection(owner)
        .expect("inspect compatible donor action");
    let rewritten = donor
        .lineage_document()
        .step(owner)
        .expect("rewritten donor owner")
        .clone();
    assert_ne!(rewritten.action, original.action);
    assert_eq!(rewritten.outputs, original.outputs);
    assert_eq!(rewritten.output_identities, original.output_identities);
    assert_eq!(rewritten.reservations, original.reservations);

    let history = (coordinator.history_len(), coordinator.history_cursor());
    let outcome = coordinator
        .rewrite_lineage_step_json(
            coordinator.lineage_identity(),
            owner,
            &replacement.replacement_json,
        )
        .expect("compatible material action rewrite");
    assert!(outcome.changed);
    assert!(outcome.accepted);
    assert_eq!(coordinator.lineage_document().steps().len(), original_steps);
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        (history.0 + 1, history.1 + 1),
        "one raw action rewrite is exactly one history position"
    );
    let live = coordinator
        .lineage_document()
        .step(owner)
        .expect("same live owner after raw rewrite");
    assert_eq!(live, &rewritten);
    let moved = coordinator
        .session()
        .design_document()
        .point(point)
        .expect("materialized rewritten point")
        .position;
    assert_eq!(
        moved.map(f64::to_bits),
        [3.0_f64.to_bits(), 4.0_f64.to_bits()]
    );
    assert!(moved.into_iter().all(f64::is_finite));

    let rewritten_session = coordinator
        .lineage_session_json()
        .expect("rewritten retained lineage");
    let rewritten_ledger = coordinator
        .lineage_host_input_ledger_json()
        .expect("rewritten host-input ledger");
    let cold = RetainedEditorCoordinator::lineage_cold_current_accepted_evidence_checkpoint(
        &rewritten_session,
        Some(&rewritten_ledger),
        coordinator.session().parameter_batch(),
        coordinator.session().external_snapshot_set(),
    )
    .expect("strict-cold compatible rewrite evidence");
    let accepted = coordinator
        .session()
        .accepted_state_for_current_input()
        .expect("current accepted compatible rewrite");
    let accepted_json = if cold.accepted_uses_draft_v5() {
        accepted.document().to_draft_v5_json()
    } else {
        accepted.document().to_canonical_json()
    }
    .expect("current accepted JSON");
    assert_eq!(cold.accepted_json(), Some(accepted_json.as_str()));
    assert!(cold.accepted_belongs_to_current_design());

    coordinator.undo().expect("undo raw action rewrite");
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .point(point)
            .expect("point after Undo")
            .position
            .map(f64::to_bits),
        [1.0_f64.to_bits(), 2.0_f64.to_bits()]
    );
    assert_eq!(coordinator.lineage_document().step(owner), Some(&original));
    coordinator.redo().expect("redo raw action rewrite");
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .point(point)
            .expect("point after Redo")
            .position
            .map(f64::to_bits),
        [3.0_f64.to_bits(), 4.0_f64.to_bits()]
    );
    assert_eq!(coordinator.lineage_document().step(owner), Some(&rewritten));

    let mut restored = RetainedEditorCoordinator::new(coordinator.session().clone())
        .expect("compatible rewrite restore target");
    restored
        .restore_lineage_session_and_host_input_ledger_json(&rewritten_session, &rewritten_ledger)
        .expect("cold reload compatible raw rewrite");
    assert_eq!(restored.lineage_document().step(owner), Some(&rewritten));
    assert_eq!(
        restored
            .session()
            .design_document()
            .point(point)
            .expect("cold-reloaded point")
            .position
            .map(f64::to_bits),
        [3.0_f64.to_bits(), 4.0_f64.to_bits()]
    );
}
