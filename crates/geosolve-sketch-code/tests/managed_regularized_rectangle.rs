// SPDX-License-Identifier: GPL-3.0-or-later
use geosolve_sketch::{DocumentId, GeometryRole, PersistentId};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, KeyedReconcileState, ProjectKey,
    materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{IntentFieldKey, IntentKey, IntentLiteral, IntentSessionId};
use std::collections::BTreeSet;

#[test]
fn authored_regularized_rectangle_preserves_square_recipe_and_construction_role() {
    let compiled = CompiledManagedSource::from_json(include_str!(
        "fixtures/managed-regularized-rectangle.json"
    ))
    .unwrap();
    let project =
        CodeProject::managed(ProjectKey("regularized-rectangle".into()), compiled).unwrap();
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x0098_0e01),
        DocumentId(PersistentId::from_u128(0x0098_0e01)),
        1.0,
    )
    .expect("source regularization must reach shared sample plan");
    let intent = materialized.editor.coordinator().intent();
    let field = IntentFieldKey(IntentKey::new("regularized").unwrap());
    assert!(
        intent
            .graph()
            .nodes()
            .values()
            .any(|node| node.fields.get(&field) == Some(&IntentLiteral::Boolean(true)))
    );
    let authority = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    let accepted = authority
        .session
        .accepted_state_for_current_input()
        .unwrap();
    assert!(
        accepted
            .diagnostics()
            .solve
            .unwrap()
            .hard_residuals_validated
    );
    let document = accepted.document();
    let points = document
        .points()
        .iter()
        .map(|point| point.position)
        .collect::<Vec<_>>();
    assert_eq!(points.len(), 4);
    for expected in [[40.0, 40.0], [60.0, 40.0], [60.0, 60.0], [40.0, 60.0]] {
        assert!(
            points
                .iter()
                .any(|point| (0..2).all(|axis| (point[axis] - expected[axis]).abs() < 1e-9))
        );
    }
    assert_eq!(document.curves().len(), 4);
    assert!(
        document
            .curves()
            .iter()
            .all(|curve| document.geometry_role(curve.id) == Some(GeometryRole::Construction))
    );
}
