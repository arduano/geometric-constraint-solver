// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    clippy::float_cmp,
    reason = "exact fixture scalar and transactional state assertions"
)]

use geosolve_sketch::{CurveDefinition, DesignScalarId};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, ManagedPathSegment, ManagedSketchMutation, ManagedValue,
    PreparedManagedMutationReceipt, PreparedManagedMutationRequest, ProjectKey, SemanticOutputPath,
    SemanticSymbol, UnitLiteral,
};
use geosolve_sketch_engine::{
    AuthoringPrediction, AuthoringValueWrite, EditableSession, EngineAcceptedResult,
};

fn compiled(radius: u32) -> CompiledManagedSource {
    CompiledManagedSource::from_json(match radius {
        2 => include_str!("fixtures/authoring-radius-2.json"),
        5 => include_str!("fixtures/authoring-radius-5.json"),
        _ => panic!("unsupported fixture"),
    })
    .unwrap()
}

fn project(radius: u32) -> String {
    CodeProject::managed(ProjectKey("headless-authoring".into()), compiled(radius))
        .unwrap()
        .to_canonical_json()
        .unwrap()
}

fn write(radius: f64) -> AuthoringValueWrite {
    AuthoringValueWrite {
        declaration: SemanticSymbol("bore".into()),
        path: SemanticOutputPath(vec![ManagedPathSegment::Field("radius".into())]),
        value: ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: radius,
        }),
    }
}

fn receipt(
    request: &PreparedManagedMutationRequest,
    radius: u32,
) -> PreparedManagedMutationReceipt {
    let compiled = compiled(radius);
    PreparedManagedMutationReceipt {
        ticket_digest: request.ticket.ticket_digest.clone(),
        base_source_digest: request.ticket.accepted_source_digest.clone(),
        candidate_source_digest: compiled.ir.source_digest.clone(),
        compiled,
    }
}

fn radius(result: &EngineAcceptedResult) -> f64 {
    let CurveDefinition::Circle { radius, .. } = result.geometry.curves[0].curve.definition else {
        panic!("circle expected")
    };
    scalar(result, radius)
}

fn scalar(result: &EngineAcceptedResult, id: DesignScalarId) -> f64 {
    result
        .geometry
        .scalars
        .iter()
        .find(|scalar| scalar.id == id)
        .unwrap()
        .value
}

#[test]
fn managed_receipts_publish_atomically_and_reject_replay_forgery_and_semantic_mismatch() {
    let mut session = EditableSession::open(&project(2), None).unwrap();
    let initial = session.state();
    let prepared = session.prepare_managed_values(vec![write(5.0)]).unwrap();
    assert_eq!(session.state(), initial, "preparation must not publish");
    assert!(
        session
            .apply_managed_values(&prepared, receipt(prepared.request(), 2))
            .is_err()
    );
    assert_eq!(session.state(), initial);
    let mut forged = receipt(prepared.request(), 5);
    forged.ticket_digest = "0".repeat(64);
    assert!(session.apply_managed_values(&prepared, forged).is_err());
    assert_eq!(session.state(), initial);
    let (accepted, _) = session
        .apply_managed_values(&prepared, receipt(prepared.request(), 5))
        .unwrap();
    assert_eq!(radius(accepted.result()), 5.0);
    assert!(accepted.result().validation.hard_residuals_validated);
    assert!(
        accepted
            .result()
            .geometry
            .scalars
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    let updated = session.state();
    assert!(
        session
            .apply_managed_values(&prepared, receipt(prepared.request(), 5))
            .is_err()
    );
    assert_eq!(session.state(), updated);
}

#[test]
fn semantic_value_preparation_reads_latest_source_and_inverse_uses_checked_new_transaction() {
    let mut session = EditableSession::open(&project(2), None).unwrap();
    let prepared = session.prepare_managed_values(vec![write(5.0)]).unwrap();
    let (_, inverse) = session
        .apply_managed_values(&prepared, receipt(prepared.request(), 5))
        .unwrap();
    let undo = session.prepare_managed_value_inverse(&inverse).unwrap();
    let ManagedSketchMutation::SetValues { values } = &undo.request().ticket.mutation else {
        panic!("batch expected")
    };
    assert_eq!(values[0].expected, write(5.0).value);
    assert_eq!(values[0].value, write(2.0).value);
    let revision = session.token().revision;
    let (accepted, redo) = session
        .apply_managed_values(&undo, receipt(undo.request(), 2))
        .unwrap();
    assert_eq!(radius(accepted.result()), 2.0);
    assert!(
        session.token().revision > revision,
        "inverse is a new contribution"
    );
    assert!(
        session.prepare_managed_value_inverse(&inverse).is_err(),
        "cannot overwrite different later property"
    );
    let redo = session.prepare_managed_value_inverse(&redo).unwrap();
    let (accepted, _) = session
        .apply_managed_values(&redo, receipt(redo.request(), 5))
        .unwrap();
    assert_eq!(radius(accepted.result()), 5.0);
    let latest = session.prepare_managed_values(vec![write(2.0)]).unwrap();
    let ManagedSketchMutation::SetValues { values } = &latest.request().ticket.mutation else {
        panic!("batch expected")
    };
    assert_eq!(values[0].expected, write(5.0).value);
    let other = EditableSession::open(&project(5), None).unwrap();
    assert!(other.prepare_managed_value_inverse(&inverse).is_err());
}

#[test]
fn pending_receipt_cannot_cross_an_intervening_accepted_edit_or_another_session() {
    let mut session = EditableSession::open(&project(2), None).unwrap();
    let prepared = session
        .prepare_managed_mutation(
            ManagedSketchMutation::SetValue {
                declaration: "bore".into(),
                path: write(5.0).path.0,
                expected: write(2.0).value,
                value: write(5.0).value,
            },
            0,
        )
        .unwrap();
    let mut other = EditableSession::open(&project(2), None).unwrap();
    let before = other.state();
    assert!(
        other
            .apply_managed_mutation(&prepared, receipt(prepared.request(), 5))
            .is_err()
    );
    assert_eq!(other.state(), before);
    let token = session.token().clone();
    session.apply_project(&token, &project(5)).unwrap();
    let updated = session.state();
    assert!(
        session
            .apply_managed_mutation(&prepared, receipt(prepared.request(), 5))
            .is_err()
    );
    assert_eq!(session.state(), updated);
}

#[test]
fn captured_apply_authenticates_exact_source_and_preserves_state_after_unrelated_receipt() {
    let mut session = EditableSession::open(&project(2), None).unwrap();
    let mut draft = compiled(5).normalized_source;
    let prepared = session.prepare_managed_source(draft.clone()).unwrap();
    draft.push_str("// later typing");
    assert_ne!(prepared.request().candidate_source, draft);
    let request = prepared.request();
    let make_receipt = |radius| {
        let compiled = compiled(radius);
        PreparedManagedMutationReceipt {
            ticket_digest: request.ticket.ticket_digest.clone(),
            base_source_digest: request.ticket.accepted_source_digest.clone(),
            candidate_source_digest: compiled.ir.source_digest.clone(),
            compiled,
        }
    };
    let before = session.state();
    assert!(
        session
            .apply_managed_source(&prepared, make_receipt(2))
            .is_err()
    );
    assert_eq!(session.state(), before);
    let accepted = session
        .apply_managed_source(&prepared, make_receipt(5))
        .unwrap();
    assert_eq!(radius(accepted.result()), 5.0);
    assert!(accepted.result().validation.hard_residuals_validated);
}

#[test]
fn invalid_property_batch_keeps_accepted_state_and_prediction_does_not_publish_to_server() {
    let mut server = EditableSession::open(&project(2), None).unwrap();
    let before = server.state();
    assert!(
        server
            .prepare_managed_values(vec![
                write(5.0);
                geosolve_sketch_code::MANAGED_MUTATION_BATCH_LIMIT + 1
            ])
            .is_err()
    );
    let mut absent = write(5.0);
    absent.declaration = SemanticSymbol("missing".into());
    assert!(
        server
            .prepare_managed_values(vec![write(5.0), absent])
            .is_err()
    );
    assert!(
        server
            .prepare_managed_values(vec![write(5.0), write(2.0)])
            .is_err()
    );
    let mut invalid = write(5.0);
    invalid.value = ManagedValue::Number(f64::NAN);
    assert!(server.prepare_managed_values(vec![invalid]).is_err());
    assert_eq!(server.state(), before);
    let mut prediction = AuthoringPrediction::open(&project(2), None).unwrap();
    let prepared = prediction.prepare_managed_values(vec![write(5.0)]).unwrap();
    assert!(
        server
            .apply_managed_values(&prepared, receipt(prepared.request(), 5))
            .is_err()
    );
    let result = prediction
        .apply_managed_values(&prepared, receipt(prepared.request(), 5))
        .unwrap();
    assert_eq!(radius(result), 5.0);
    assert!(result.validation.hard_residuals_validated);
    assert_eq!(server.state(), before);
    assert_eq!(radius(&server.state().result), 2.0);
}
