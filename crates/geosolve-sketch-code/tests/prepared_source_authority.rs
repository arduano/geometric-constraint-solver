// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_sketch_code::{
    CodeProject, KeyedReconcileState, ManagedMutationAuthority, PreparedManagedMutationError,
    PreparedManagedMutationReceipt, ProjectKey, SketchCodeSession, expand_code_project,
    prepare_managed_source, required_generated_members, validate_prepared_managed_source,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

#[test]
fn altered_raw_source_request_rejects_without_advancing_session_or_history() {
    let project = CodeProject::empty(ProjectKey("prepared-source-retention".into()))
        .expect("empty executed V3 project");
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).expect("empty generated inventory"),
            &BTreeSet::new(),
        )
        .expect("empty reconciliation")
        .into_staged();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x90_6001))
        .expect("deterministic intent session");
    let expansion = expand_code_project(&project, &generated, intent.identity())
        .expect("empty project expansion");
    let session = SketchCodeSession::new_project(
        project.clone(),
        generated,
        expansion.clone(),
        serde_json::json!({"accepted": "empty"}),
    )
    .expect("accepted code session");
    let compiled = project
        .managed
        .compiled
        .as_deref()
        .expect("empty project compiler authority");
    let authority = ManagedMutationAuthority::new(
        project.project.clone(),
        session.identity().clone(),
        expansion.digest,
        project.managed.declaration_name_high_water,
        compiled,
    )
    .expect("accepted mutation authority");

    let retained_session = session.to_canonical_json().expect("session before refusal");
    let retained_identity = session.identity().clone();
    let retained_snapshot = session.snapshot().clone();
    let mut request = prepare_managed_source(
        &authority,
        compiled,
        format!(
            "{}\n// exact prepared candidate",
            compiled.normalized_source
        ),
    )
    .expect("prepared raw-source request");
    request
        .candidate_source
        .push_str("\n// altered after preparation");
    let receipt = PreparedManagedMutationReceipt {
        ticket_digest: request.ticket.ticket_digest.clone(),
        base_source_digest: compiled.ir.source_digest.clone(),
        candidate_source_digest: compiled.ir.source_digest.clone(),
        compiled: compiled.clone(),
    };

    assert!(matches!(
        validate_prepared_managed_source(&authority, &request, receipt),
        Err(PreparedManagedMutationError::InvalidTicket(message))
            if message == "candidate source bytes do not match the prepared digest"
    ));
    assert_eq!(session.identity(), &retained_identity);
    assert_eq!(session.snapshot(), &retained_snapshot);
    assert!(!session.can_undo());
    assert!(!session.can_redo());
    assert_eq!(
        session.to_canonical_json().expect("session after refusal"),
        retained_session
    );
}
