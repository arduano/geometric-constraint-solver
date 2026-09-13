// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_constraint_editor::{SelectionPresentationState, Viewport};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, ManagedControlSubmission, ProjectKey,
};
use geosolve_sketch_engine::EditableSession;

fn fixture() -> EditableSession {
    let compiled =
        CompiledManagedSource::from_json(include_str!("fixtures/point-gesture-constrained.json"))
            .unwrap();
    let project = CodeProject::managed(ProjectKey("accepted-browsing".into()), compiled).unwrap();
    EditableSession::open(&project.to_canonical_json().unwrap(), None).unwrap()
}

#[test]
fn independent_browsing_retains_exact_source_native_authority_and_personal_selection() {
    let session = fixture();
    let project = session.export_project_json().unwrap();
    let design = session.design();
    let token = session.token().clone();
    let mut first = session.browsing().unwrap();
    let second = session.browsing().unwrap();
    let original = first.editor().coordinator().intent().identity();
    let materialized = std::ptr::from_ref(
        first
            .editor()
            .coordinator()
            .accepted_materialization()
            .unwrap(),
    );
    let entries = first.source().unwrap().navigation().entries.clone();
    for (index, entry) in (0_u32..).zip(&entries) {
        let offset = f64::from(index);
        let viewport = Viewport::new([800.0, 600.0], [offset, 0.0], 5.0 + offset).unwrap();
        let scene = first.scene(viewport).unwrap();
        let items = entry.exact_bindings.clone().map_or_else(
            || {
                first
                    .editor()
                    .navigation_selection_items(entry.nodes.iter().copied())
            },
            |bindings| {
                first
                    .editor()
                    .navigation_selection_items_for_bindings(bindings)
            },
        );
        let selection = SelectionPresentationState {
            items,
            curve_picks: Vec::new(),
        };
        first
            .restore_presentation(
                &scene,
                selection.clone(),
                first.editor().editor().geometry_interaction_policy(),
            )
            .unwrap();
        assert_eq!(first.editor().editor().selection(), selection.items);
        assert!(second.editor().editor().selection().is_empty());
        assert_eq!(first.editor().coordinator().intent().identity(), original);
        assert_eq!(
            std::ptr::from_ref(
                first
                    .editor()
                    .coordinator()
                    .accepted_materialization()
                    .unwrap()
            ),
            materialized
        );
        assert_eq!(session.export_project_json().unwrap(), project);
        assert_eq!(session.design(), design);
        assert_eq!(session.token(), &token);
        assert_eq!(first.source().unwrap().token(), &token);
        assert_eq!(
            first.accepted().result().result_id,
            session.accepted().result().result_id
        );
        assert_eq!(
            first.declarations().unwrap().source_digest,
            first.source().unwrap().navigation().source_digest
        );
    }
}

#[test]
fn browsing_describes_exact_control_change_and_rejects_invalid_values_without_publication() {
    let session = fixture();
    let browsing = session.browsing().unwrap();
    let project = session.export_project_json().unwrap();
    let control = browsing
        .source()
        .unwrap()
        .controls()
        .controls
        .iter()
        .find(|control| control.token().is_some() && control.source.declaration.0 == "length")
        .unwrap();
    let mutation = browsing
        .describe_control(&control.id.0, ManagedControlSubmission::Number(24.0))
        .unwrap()
        .unwrap();
    let geosolve_sketch_code::ManagedSketchMutation::SetValues { values } = mutation else {
        panic!("exact source control");
    };
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].declaration, "length");
    assert_eq!(values[0].expected, control.value);
    assert!(
        browsing
            .describe_control(&control.id.0, ManagedControlSubmission::Number(f64::NAN))
            .is_err()
    );
    assert!(
        browsing
            .describe_control("foreign-control", ManagedControlSubmission::Number(24.0))
            .is_err()
    );
    assert_eq!(session.export_project_json().unwrap(), project);
    assert_eq!(
        browsing.source().unwrap().snapshot(),
        session.browsing().unwrap().source().unwrap().snapshot()
    );
}
