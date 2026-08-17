// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::RetainedEditorCoordinator;
use geosolve_sketch::{
    CurveDefinition, DocumentCommandEffect, DocumentEdit, DocumentRationalConicControl,
    DocumentSolveRequest, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, RetainedSketchDocumentSession,
    ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
};

#[test]
fn rational_control_edit_replays_through_the_ordinary_durable_edit_action() {
    let mut document = SketchDocument::new(4.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let end = document.add_point("end", [4.0, 0.0]).unwrap();
    let weight = document
        .add_scalar(
            "weight",
            0.5,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
                upper: f64::MAX,
            },
        )
        .unwrap();
    let curve = document
        .add_curve(
            "rational",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: [1.0, 1.0],
                middle_weight: weight,
                end,
            },
        )
        .unwrap();
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let replay_baseline = session.clone();
    let mut source = RetainedEditorCoordinator::new(session).unwrap();
    let outcome = source
        .apply_edit(
            source.session().design_identity(),
            DocumentEdit::SetRationalConicControl {
                curve,
                control: DocumentRationalConicControl::Euclidean {
                    middle: [2.25, 1.75],
                    weight: 0.8,
                },
            },
        )
        .unwrap();
    assert_eq!(
        outcome.value,
        DocumentCommandEffect::UpdatedRationalConicControl(curve)
    );
    let action = source.transcript().last().unwrap().clone();
    let expected = source
        .session()
        .accepted_state_for_current_input()
        .unwrap()
        .document()
        .to_canonical_json()
        .unwrap();

    let mut replayed = RetainedEditorCoordinator::new(replay_baseline).unwrap();
    replayed.replay(&action).unwrap();
    assert_eq!(
        replayed
            .session()
            .accepted_state_for_current_input()
            .unwrap()
            .document()
            .to_canonical_json()
            .unwrap(),
        expected
    );
    assert_eq!(replayed.transcript(), [action]);
}
