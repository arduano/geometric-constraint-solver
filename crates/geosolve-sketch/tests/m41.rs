// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_sketch::{
    AlphaScenarioKind, CurveDefinition, DocumentCommand, DocumentCommandEffect,
    DocumentConstraintDefinition, DocumentEdit, DocumentElementId, DocumentSolveRequest,
    GeometryRole, HostActivationOverride, HostConfigurationActivation, InactivityReason,
    RetainedSketchDocumentSession, SketchDocument, SketchDocumentSession, VisualProfileOptions,
    VisualProfileStatus, alpha_scenario,
};

fn square(
    document: &mut SketchDocument,
) -> (
    geosolve_sketch::CurveId,
    [geosolve_sketch::DesignPointId; 4],
) {
    let points = [[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]
        .map(|position| document.add_point("corner", position).unwrap());
    let branch_directions = vec![[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];
    let curve = document
        .add_curve(
            "square",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: true,
                branch_directions,
            },
        )
        .unwrap();
    (curve, points)
}

#[test]
fn construction_geometry_solves_constraints_but_is_not_a_default_profile() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let (curve, points) = square(&mut document);
    assert_eq!(
        document
            .analyze_visual_profiles(VisualProfileOptions::default())
            .faces
            .len(),
        1
    );

    document
        .set_geometry_role(curve, GeometryRole::Construction)
        .unwrap();
    document
        .add_constraint(
            "move construction point",
            DocumentConstraintDefinition::FixedPoint {
                point: points[0],
                target: [1.0, 1.0],
            },
        )
        .unwrap();
    assert_eq!(
        document.geometry_role(curve),
        Some(GeometryRole::Construction)
    );
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.faces.is_empty());
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted = session.accepted_state().unwrap();
    assert!(accepted.mappings().runtime_curve(curve).is_some());
    let solved = accepted
        .solve_result()
        .geometry
        .point(accepted.mappings().runtime_point(points[0]).unwrap())
        .unwrap();
    assert!((solved.x - 1.0).abs() < 1e-9 && (solved.y - 1.0).abs() < 1e-9);
}

#[test]
fn dependency_inactivity_is_typed_and_precedes_lowering_and_profiles() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let (curve, points) = square(&mut document);
    document
        .set_element_user_suppressed(DocumentElementId::Point(points[0]), true)
        .unwrap();

    let activity = document.effective_activity();
    assert_eq!(
        activity.reason(points[0]),
        Some(InactivityReason::UserSuppressed)
    );
    assert_eq!(
        activity.reason(curve),
        Some(InactivityReason::UnavailableDependency {
            dependency: DocumentElementId::Point(points[0]),
        })
    );
    assert!(
        document
            .lower()
            .unwrap()
            .mappings()
            .runtime_curve(curve)
            .is_none()
    );
    assert!(
        document
            .analyze_visual_profiles(VisualProfileOptions::default())
            .faces
            .is_empty()
    );
}

#[test]
fn host_payload_is_canonical_revision_checked_and_dependency_closed() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let (curve, points) = square(&mut document);
    let first = HostConfigurationActivation::new(
        1,
        vec![
            HostActivationOverride::Inactive(DocumentElementId::Point(points[1])),
            HostActivationOverride::Inactive(DocumentElementId::Point(points[0])),
        ],
    )
    .unwrap();
    let reordered = HostConfigurationActivation::new(
        1,
        vec![
            HostActivationOverride::Inactive(DocumentElementId::Point(points[0])),
            HostActivationOverride::Inactive(DocumentElementId::Point(points[1])),
        ],
    )
    .unwrap();
    assert_eq!(first, reordered);
    document.set_host_configuration_activation(first).unwrap();
    let before = document.effective_activity();
    assert_eq!(
        before.reason(points[0]),
        Some(InactivityReason::HostConfigurationInactive)
    );
    assert!(matches!(
        before.reason(curve),
        Some(InactivityReason::UnavailableDependency { .. })
    ));

    let stale = HostConfigurationActivation::new(1, Vec::new()).unwrap();
    assert!(document.set_host_configuration_activation(stale).is_err());
    assert_eq!(document.effective_activity(), before);
}

#[test]
fn v4_bytes_stay_stable_for_default_state_and_reject_nondefault_m41_state() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let (curve, _) = square(&mut document);
    let canonical = document.to_canonical_json().unwrap();
    document
        .set_geometry_role(curve, GeometryRole::Profile)
        .unwrap();
    assert_eq!(document.to_canonical_json().unwrap(), canonical);

    document
        .set_geometry_role(curve, GeometryRole::Construction)
        .unwrap();
    assert!(document.to_canonical_json().is_err());
    let draft = document.to_draft_v5_json().unwrap();
    let restored = SketchDocument::from_draft_v5_json(&draft).unwrap();
    assert_eq!(
        restored.geometry_role(curve),
        Some(GeometryRole::Construction)
    );
}

#[test]
fn role_edit_replays_through_accepted_history() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let (curve, _) = square(&mut document);
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();

    let outcome = session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetGeometryRole {
                curve,
                role: GeometryRole::Construction,
            },
        ))
        .unwrap();
    assert_eq!(
        outcome.effect,
        Some(DocumentCommandEffect::UpdatedGeometryRole(curve))
    );
    assert_eq!(
        session.document().geometry_role(curve),
        Some(GeometryRole::Construction)
    );
    assert_eq!(session.history_len(), 1);

    session.undo(session.revision()).unwrap();
    assert_eq!(
        session.document().geometry_role(curve),
        Some(GeometryRole::Profile)
    );
    session.redo(session.revision()).unwrap();
    assert_eq!(
        session.document().geometry_role(curve),
        Some(GeometryRole::Construction)
    );
}

#[test]
fn typed_edits_cover_all_inactivity_vectors_and_transitive_reactivation() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let (curve, points) = square(&mut document);
    let mut session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();

    let suppress = |session: &mut SketchDocumentSession, suppressed| {
        session
            .apply(DocumentCommand::new(
                session.revision(),
                DocumentEdit::SetElementUserSuppressed {
                    element: DocumentElementId::Point(points[0]),
                    suppressed,
                },
            ))
            .unwrap()
    };
    let outcome = suppress(&mut session, true);
    assert_eq!(
        outcome.effect,
        Some(DocumentCommandEffect::UpdatedElementUserSuppression(
            DocumentElementId::Point(points[0])
        ))
    );
    assert_eq!(
        session.document().effective_activity().reason(points[0]),
        Some(InactivityReason::UserSuppressed)
    );
    assert!(matches!(
        session.document().effective_activity().reason(curve),
        Some(InactivityReason::UnavailableDependency { .. })
    ));
    suppress(&mut session, false);
    assert!(session.document().effective_activity().is_active(curve));

    let host_inactive = HostConfigurationActivation::new(
        1,
        vec![HostActivationOverride::Inactive(DocumentElementId::Point(
            points[0],
        ))],
    )
    .unwrap();
    session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetHostConfigurationActivation {
                activation: host_inactive,
            },
        ))
        .unwrap();
    assert_eq!(
        session.document().effective_activity().reason(points[0]),
        Some(InactivityReason::HostConfigurationInactive)
    );

    let unavailable = HostConfigurationActivation::new(
        2,
        vec![HostActivationOverride::UnavailableExternalReference(
            DocumentElementId::Point(points[0]),
        )],
    )
    .unwrap();
    session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetHostConfigurationActivation {
                activation: unavailable,
            },
        ))
        .unwrap();
    assert_eq!(
        session.document().effective_activity().reason(points[0]),
        Some(InactivityReason::UnavailableExternalReference)
    );

    // A newer empty immutable payload clears host inactivity without touching geometry.
    let clear = HostConfigurationActivation::new(3, Vec::new()).unwrap();
    session
        .apply(DocumentCommand::new(
            session.revision(),
            DocumentEdit::SetHostConfigurationActivation { activation: clear },
        ))
        .unwrap();
    assert!(session.document().effective_activity().is_active(curve));
}

#[test]
fn host_activation_rejections_roll_back() {
    let mut document = SketchDocument::new(2.0).unwrap();
    let (curve, _) = square(&mut document);
    let mut accepted = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let first = HostConfigurationActivation::new(
        1,
        vec![HostActivationOverride::Inactive(DocumentElementId::Curve(
            curve,
        ))],
    )
    .unwrap();
    accepted
        .apply(DocumentCommand::new(
            accepted.revision(),
            DocumentEdit::SetHostConfigurationActivation { activation: first },
        ))
        .unwrap();
    let before = accepted.document().clone();
    let stale = HostConfigurationActivation::new(1, Vec::new()).unwrap();
    assert!(
        accepted
            .apply(DocumentCommand::new(
                accepted.revision(),
                DocumentEdit::SetHostConfigurationActivation { activation: stale },
            ))
            .is_err()
    );
    assert_eq!(accepted.document(), &before);

    let duplicate = HostConfigurationActivation::new(
        2,
        vec![
            HostActivationOverride::Inactive(DocumentElementId::Curve(curve)),
            HostActivationOverride::UnavailableExternalReference(DocumentElementId::Curve(curve)),
        ],
    );
    assert!(duplicate.is_err());
    let unknown = geosolve_sketch::CurveId(geosolve_sketch::PersistentId::from_u128(999));
    let invalid = HostConfigurationActivation::new(
        2,
        vec![HostActivationOverride::Inactive(DocumentElementId::Curve(
            unknown,
        ))],
    )
    .unwrap();
    assert!(
        accepted
            .apply(DocumentCommand::new(
                accepted.revision(),
                DocumentEdit::SetHostConfigurationActivation {
                    activation: invalid,
                },
            ))
            .is_err()
    );
    assert_eq!(accepted.document(), &before);
}

#[test]
fn rejected_reactivation_keeps_retained_views_separate() {
    let mut retained_document = SketchDocument::new(2.0).unwrap();
    let point = retained_document.add_point("point", [0.0, 0.0]).unwrap();
    retained_document
        .add_constraint(
            "fixed zero",
            DocumentConstraintDefinition::FixedPoint {
                point,
                target: [0.0, 0.0],
            },
        )
        .unwrap();
    let conflicting = retained_document
        .add_constraint(
            "fixed one",
            DocumentConstraintDefinition::FixedPoint {
                point,
                target: [1.0, 0.0],
            },
        )
        .unwrap();
    retained_document
        .set_element_user_suppressed(DocumentElementId::Constraint(conflicting), true)
        .unwrap();
    let mut retained = RetainedSketchDocumentSession::new(
        retained_document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    let accepted_identity = retained.accepted_state().unwrap().identity();
    let accepted_input = retained.accepted_state().unwrap().input();
    let initial_attempt_input = retained.last_attempt().input();
    assert_eq!(
        initial_attempt_input.effective_activation_revision(),
        accepted_input.effective_activation_revision()
    );
    assert_eq!(
        initial_attempt_input.activation_digest(),
        accepted_input.activation_digest()
    );
    let outcome = retained
        .apply(
            retained.design_identity(),
            DocumentEdit::SetElementUserSuppressed {
                element: DocumentElementId::Constraint(conflicting),
                suppressed: false,
            },
        )
        .unwrap();
    assert!(outcome.published_accepted_identity().is_none());
    assert_ne!(
        outcome.design_identity(),
        accepted_identity_to_design(retained.accepted_state().unwrap())
    );
    assert_eq!(
        retained.accepted_state().unwrap().identity(),
        accepted_identity
    );
    assert_eq!(retained.accepted_state().unwrap().input(), accepted_input);
    let attempt = retained.last_attempt();
    assert!(
        attempt
            .solve_result()
            .is_some_and(|solve| solve.rejection.is_some())
            || attempt.failure().is_some()
    );
}

#[test]
fn reactivation_preserves_discrete_curve_contact_and_trim_state_exactly() {
    let fixture = alpha_scenario(AlphaScenarioKind::M28TrimmedFillet, 1.0).unwrap();
    let geosolve_sketch::AlphaScenarioIds::M28TrimmedFillet(ids) = fixture.ids else {
        unreachable!("M28 fixture must expose M28 identities");
    };
    let mut retained = RetainedSketchDocumentSession::new(
        fixture.document,
        fixture.request,
        SolverConfig::default(),
    )
    .unwrap();
    let before = retained.design_document().to_draft_v5_json().unwrap();

    for suppressed in [true, false] {
        retained
            .apply(
                retained.design_identity(),
                DocumentEdit::SetElementUserSuppressed {
                    element: DocumentElementId::Curve(ids.line),
                    suppressed,
                },
            )
            .unwrap();
    }

    // This is a serialized-value comparison: no coordinate recovery is used.
    assert_eq!(
        retained.design_document().to_draft_v5_json().unwrap(),
        before
    );
}

fn accepted_identity_to_design(
    accepted: &geosolve_sketch::SketchAcceptedDocumentState,
) -> geosolve_sketch::SketchDesignIdentity {
    accepted.design_identity()
}
