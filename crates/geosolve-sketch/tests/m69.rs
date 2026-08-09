// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveSpan, DocumentCommand, DocumentCommandEffect,
    DocumentEdit, DocumentSolveRequest, GeometryRole, GeometryRoleEdit, SketchDocument,
    SketchDocumentSession, SolverConfig, TangentOrientation,
};

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: [f64; 2],
    end: [f64; 2],
    role: GeometryRole,
) -> geosolve_sketch::CurveId {
    let start_id = document.add_point(format!("{label}.start"), start).unwrap();
    let end_id = document.add_point(format!("{label}.end"), end).unwrap();
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length = delta[0].hypot(delta[1]);
    document
        .add_curve_with_role(
            label,
            CurveDefinition::Line {
                start: start_id,
                end: end_id,
                branch_direction: [delta[0] / length, delta[1] / length],
            },
            role,
        )
        .unwrap()
}

#[test]
fn geometry_role_batches_reject_empty_unknown_duplicate_and_conflicting_input_atomically() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let first = line(
        &mut document,
        "first",
        [0.0, 0.0],
        [2.0, 0.0],
        GeometryRole::Profile,
    );
    let second = line(
        &mut document,
        "second",
        [0.0, 1.0],
        [2.0, 1.0],
        GeometryRole::Profile,
    );
    let mut foreign = SketchDocument::new(10.0).unwrap();
    let unknown = line(
        &mut foreign,
        "foreign",
        [0.0, 0.0],
        [1.0, 0.0],
        GeometryRole::Profile,
    );
    let before = document.clone();

    for edits in [
        Vec::new(),
        vec![GeometryRoleEdit::new(unknown, GeometryRole::Construction)],
        vec![
            GeometryRoleEdit::new(first, GeometryRole::Construction),
            GeometryRoleEdit::new(first, GeometryRole::Construction),
        ],
        vec![
            GeometryRoleEdit::new(second, GeometryRole::Construction),
            GeometryRoleEdit::new(second, GeometryRole::Profile),
        ],
    ] {
        assert!(document.set_geometry_roles(&edits).is_err());
        assert_eq!(document, before);
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one atomic role-edit sequence compares exact solve and branch evidence through apply, Undo and Redo"
)]
fn retained_batch_role_edit_is_one_history_step_and_changes_no_geometry_or_branch_state() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let first = line(
        &mut document,
        "first",
        [0.0, 0.0],
        [2.0, 0.0],
        GeometryRole::Profile,
    );
    let second = line(
        &mut document,
        "second",
        [0.0, 1.0],
        [2.0, 1.0],
        GeometryRole::Profile,
    );
    let contact = document
        .add_curve_contact(
            "first local contact",
            CurveSpan::line(first),
            0.5,
            0,
            ContactNeighborhood::Local {
                lower: 0.25,
                upper: 0.75,
            },
            Some(TangentOrientation::Aligned),
        )
        .unwrap();
    let points_before = document.points().to_vec();
    let curves_before = document.curves().to_vec();
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted_signature = |session: &SketchDocumentSession| {
        let result = session.accepted_result();
        let report = result.accepted_view().unstable_core_report();
        (
            result.accepted_view().geometry.clone(),
            report.hard_residuals_validated,
            report.hard_residual_max.to_bits(),
            report.rank,
            report.right_nullity,
        )
    };
    let branch_signature = |document: &SketchDocument| {
        let slot = document.contact(contact).expect("contact remains present");
        (
            slot.curve,
            document
                .scalar(slot.parameter)
                .expect("contact parameter remains present")
                .value
                .to_bits(),
            slot.winding,
            slot.neighborhood,
            slot.tangent_orientation,
        )
    };
    let accepted_before = accepted_signature(&session);
    let branch_before = branch_signature(session.document());

    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetGeometryRoles {
                edits: vec![
                    GeometryRoleEdit::new(first, GeometryRole::Construction),
                    GeometryRoleEdit::new(second, GeometryRole::Construction),
                ],
            },
        ))
        .unwrap();
    assert_eq!(
        outcome.effect,
        Some(DocumentCommandEffect::UpdatedGeometryRoles(vec![
            first, second
        ]))
    );
    assert_eq!(session.history_len(), 1);
    assert_eq!(session.document().points(), points_before);
    assert_eq!(session.document().curves(), curves_before);
    assert_eq!(accepted_signature(&session), accepted_before);
    assert_eq!(branch_signature(session.document()), branch_before);
    assert_eq!(
        session.document().geometry_role(first),
        Some(GeometryRole::Construction)
    );

    session.undo(session.revision()).unwrap();
    assert_eq!(accepted_signature(&session), accepted_before);
    assert_eq!(branch_signature(session.document()), branch_before);
    assert_eq!(
        session.document().geometry_role(first),
        Some(GeometryRole::Profile)
    );
    session.redo(session.revision()).unwrap();
    assert_eq!(accepted_signature(&session), accepted_before);
    assert_eq!(branch_signature(session.document()), branch_before);
    assert_eq!(
        session.document().geometry_role(second),
        Some(GeometryRole::Construction)
    );
}

#[test]
fn role_aware_creation_defaults_to_profile_and_assigns_compound_roles_atomically() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let profile = line(
        &mut document,
        "profile",
        [0.0, 0.0],
        [1.0, 0.0],
        GeometryRole::Profile,
    );
    let construction = line(
        &mut document,
        "construction",
        [0.0, 1.0],
        [1.0, 1.0],
        GeometryRole::Construction,
    );
    assert_eq!(document.geometry_role(profile), Some(GeometryRole::Profile));
    assert_eq!(
        document.geometry_role(construction),
        Some(GeometryRole::Construction)
    );

    let rectangle = document
        .add_rectangle_with_role(
            "guide rectangle",
            [3.0, 0.0],
            2.0,
            1.0,
            GeometryRole::Construction,
        )
        .unwrap();
    assert!(
        rectangle
            .curves
            .into_iter()
            .all(|curve| { document.geometry_role(curve) == Some(GeometryRole::Construction) })
    );

    let ordinary = document
        .add_rectangle("ordinary rectangle", [6.0, 0.0], 2.0, 1.0)
        .unwrap();
    assert!(
        ordinary
            .curves
            .into_iter()
            .all(|curve| document.geometry_role(curve) == Some(GeometryRole::Profile))
    );

    let axis = line(
        &mut document,
        "axis",
        [0.0, -2.0],
        [0.0, 3.0],
        GeometryRole::Profile,
    );
    let mirrored = document
        .add_mirrored_curve("mirrored guide", construction, CurveSpan::line(axis))
        .unwrap();
    assert_eq!(
        document.geometry_role(mirrored.mirrored_curve),
        Some(GeometryRole::Construction)
    );
}
