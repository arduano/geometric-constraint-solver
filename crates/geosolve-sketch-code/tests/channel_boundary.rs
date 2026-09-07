// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeCompositionError, CodeHostRequest, CodeInteractionOverlay, CodeProject,
    CompiledManagedSource, KeyedReconcileState, bundled_sample, materialize_code_project_cold,
    materialize_code_project_incremental_for_structural_edit, rehydrate_materialized_code_project,
    required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

const VALID: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-channel-boundary.json"
);
const CROSSING: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-channel-boundary-self-intersection.json"
);
const DRIVEN: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-channel-boundary-driven.json"
);
const DRIVEN_EDITED: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-channel-boundary-driven-edited.json"
);

fn project(envelope: &str) -> CodeProject {
    let mut project = bundled_sample("pc-water-manifold").unwrap().project();
    project.managed = CompiledManagedSource::from_json(envelope)
        .unwrap()
        .into_managed_document()
        .unwrap();
    project.validate().unwrap();
    project
}

fn generated(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged()
}

#[test]
fn channel_cap_crossing_rejects_cold_and_incremental_publication_and_retains_previous() {
    let invalid = project(CROSSING);
    let error = materialize_code_project_cold(
        &invalid,
        &generated(&invalid),
        IntentSessionId::from_raw(0x96_b001),
        DocumentId(PersistentId::from_u128(0x96_b001)),
        1.0,
    )
    .expect_err("the R3 outlet cap crosses the first y=3 wall at x=5±sqrt(5)");
    assert!(
        matches!(error, CodeCompositionError::InvalidChannelBoundary { ref diagnostic, .. }
        if diagnostic.contains("intersects")),
        "{error}"
    );

    let valid = project(VALID);
    let state = generated(&valid);
    let previous = materialize_code_project_cold(
        &valid,
        &state,
        IntentSessionId::from_raw(0x96_b002),
        DocumentId(PersistentId::from_u128(0x96_b002)),
        1.0,
    )
    .unwrap();
    let identity = previous.editor.coordinator().intent().identity();
    let validation = previous
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .validation
        .clone();
    let expansion = previous.expansion.clone();
    let error = materialize_code_project_incremental_for_structural_edit(
        &previous,
        &invalid,
        &state,
        &CodeInteractionOverlay::empty(),
    )
    .expect_err("source edit cannot publish individually valid but intersecting boundaries");
    assert!(
        matches!(error, CodeCompositionError::InvalidChannelBoundary { .. }),
        "{error}"
    );
    assert_eq!(previous.editor.coordinator().intent().identity(), identity);
    assert_eq!(
        previous
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .validation,
        validation
    );
    assert_eq!(previous.expansion, expansion);
}

#[test]
fn valid_channel_rehydrates_native_operations_host_fillets_and_boundary_validation() {
    let project = project(VALID);
    let restored_project = CodeProject::from_json(&project.to_canonical_json().unwrap()).unwrap();
    let materialized = materialize_code_project_cold(
        &restored_project,
        &generated(&restored_project),
        IntentSessionId::from_raw(0x96_b003),
        DocumentId(PersistentId::from_u128(0x96_b003)),
        1.0,
    )
    .unwrap();
    assert!(materialized.expansion.host_requests.iter().any(|request|
        matches!(request, CodeHostRequest::ChannelBoundaryCheck { supports, expected_components: 1, expected_open_ends: 2, .. } if supports.len() == 9)));
    let validation = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap()
        .validation
        .clone();
    assert!(validation.hard_residuals_validated && validation.all_active_features_current);
    assert!(
        validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    );
    assert_eq!(validation.feature_count, 6);
    let host_outputs = materialized.host_outputs.clone();
    let encoded = materialized
        .editor
        .coordinator()
        .intent()
        .to_canonical_json()
        .unwrap();
    let intent = IntentSession::from_json(&encoded).unwrap();
    let editor = ProjectionalEditorSession::restore(intent, validation.document, 1.0).unwrap();
    let restored =
        rehydrate_materialized_code_project(Box::new(editor), materialized.expansion).unwrap();
    assert_eq!(restored.host_outputs, host_outputs);
    assert_eq!(
        restored
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .validation,
        validation
    );
}

#[test]
fn channel_dimension_edit_qualifies_solved_boundaries_instead_of_design_seeds() {
    let original = project(DRIVEN);
    let state = generated(&original);
    let previous = materialize_code_project_cold(
        &original,
        &state,
        IntentSessionId::from_raw(0x96_b004),
        DocumentId(PersistentId::from_u128(0x96_b004)),
        1.0,
    )
    .unwrap();
    let edited = project(DRIVEN_EDITED);
    let (incremental, _) = materialize_code_project_incremental_for_structural_edit(
        &previous,
        &edited,
        &state,
        &CodeInteractionOverlay::empty(),
    )
    .expect("length 30→32 moves accepted walls and fillets together");
    let cold = materialize_code_project_cold(
        &edited,
        &state,
        IntentSessionId::from_raw(0x96_b005),
        DocumentId(PersistentId::from_u128(0x96_b005)),
        1.0,
    )
    .expect("cold replay with original seeds must validate the solved boundary");
    for materialized in [Box::new(incremental), Box::new(cold)] {
        let accepted = materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap();
        let state = accepted.session.accepted_state_for_current_input().unwrap();
        let document = state.document();
        assert!(accepted.validation.hard_residuals_validated);
        assert!(accepted.validation.all_active_features_current);
        assert!(
            state
                .solve_result()
                .unstable_core_report()
                .audit
                .sources
                .iter()
                .flat_map(|source| &source.rows)
                .all(|row| row.normalized_residual.is_finite()
                    && row.normalized_residual.abs() <= 1e-9)
        );
        assert_eq!(
            state.diagnostics().rank.unwrap().numerical_right_nullity,
            Some(0)
        );
        assert!(
            document
                .points()
                .iter()
                .any(|point| (point.position[0] - 32.0).abs() < 1e-7
                    && (point.position[1] - 30.0).abs() < 1e-7)
        );
        assert!(document.points().iter().any(|point| {
            accepted
                .session
                .design_document()
                .point(point.id)
                .is_some_and(|seed| (seed.position[0] - point.position[0]).abs() > 1.0)
        }));
        let arcs = accepted
            .computed
            .edges()
            .iter()
            .filter_map(|edge| {
                if let geosolve_sketch_features::ComputedEdgeGeometry::CircularArc(arc) =
                    &edge.geometry
                {
                    Some(arc)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(arcs.len(), 2);
        for arc in arcs {
            assert!((arc.center[0] - 27.0).abs() < 1e-7);
            assert!((arc.center[1] - 5.0).abs() < 1e-7);
            assert!((arc.radius - 2.0).abs() < 1e-7 || (arc.radius - 8.0).abs() < 1e-7);
        }
        let encoded = materialized
            .editor
            .coordinator()
            .intent()
            .to_canonical_json()
            .unwrap();
        let intent = IntentSession::from_json(&encoded).unwrap();
        let editor =
            ProjectionalEditorSession::restore(intent, accepted.validation.document, 1.0).unwrap();
        rehydrate_materialized_code_project(Box::new(editor), materialized.expansion)
            .expect("restored solved boundaries retain native/computed endpoint authority");
    }
}
