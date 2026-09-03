// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared strict-request helpers for the instance-scoped workbench bridge.

use geosolve_constraint_editor::{
    IntentRpcFailure, IntentRpcOutcome, IntentRpcRequest, MAX_INTENT_RPC_REQUEST_BYTES,
};

/// Returns whether a strict, bounded request can alter durable intent/history.
///
/// This parses only the closed Rust request tag so malformed or oversized
/// messages cannot cancel an in-progress canvas interaction.
pub(super) fn request_may_change_identity(request: &str) -> bool {
    if request.len() > MAX_INTENT_RPC_REQUEST_BYTES {
        return false;
    }
    let Ok(decoded) = serde_json::from_str::<IntentRpcRequest>(request) else {
        return false;
    };
    let expected_fields: &[&str] = match decoded {
        IntentRpcRequest::ApplyPatch { .. } => &["method", "patch"],
        IntentRpcRequest::Undo | IntentRpcRequest::Redo => &["method"],
        IntentRpcRequest::EditSourceToken { .. } => &["method", "expected", "token", "replacement"],
        IntentRpcRequest::Snapshot | IntentRpcRequest::Inspector { .. } => return false,
    };
    let Ok(serde_json::Value::Object(object)) = serde_json::from_str(request) else {
        return false;
    };
    object.len() == expected_fields.len()
        && expected_fields
            .iter()
            .all(|field| object.contains_key(*field))
}

/// Rejects nested Intent mutations while an outer code project owns the
/// delegated editor. Read-only Snapshot and Inspector requests remain valid;
/// malformed requests still flow to the ordinary strict Intent decoder.
pub(super) fn code_authority_rejection(code_project_active: bool, request: &str) -> Option<String> {
    (code_project_active && request_may_change_identity(request)).then(|| {
        failure_response(
            "code_authority_required",
            "mutating a code-owned workbench requires the managed code-control RPC",
        )
    })
}

pub(super) fn failure_response(code: &str, message: &str) -> String {
    serde_json::to_string(&IntentRpcOutcome::Failure {
        failure: Box::new(IntentRpcFailure {
            code: code.to_owned(),
            message: message.to_owned(),
            identity: None,
        }),
    })
    .expect("closed RPC failure response is infallibly serializable")
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{
        IntentRpcOutcome, IntentRpcRequest, IntentSourceTokenId, MAX_INTENT_RPC_REQUEST_BYTES,
    };
    use geosolve_sketch_intent::{IntentPatch, IntentPatchPolicy};

    use super::{code_authority_rejection, request_may_change_identity};

    #[test]
    fn only_strict_bounded_mutation_requests_can_cancel_transient_work() {
        let session =
            crate::intent_rpc::empty_session_from_raw(0x8300_5101, 0x8300_5101_0000, 1.0).unwrap();
        let patch = serde_json::to_string(&IntentRpcRequest::ApplyPatch {
            patch: Box::new(IntentPatch::new(
                session.coordinator().intent().identity(),
                IntentPatchPolicy::RequireAccepted,
                Vec::new(),
            )),
        })
        .unwrap();
        let source_edit = serde_json::to_string(&IntentRpcRequest::EditSourceToken {
            expected: Box::new(session.coordinator().intent().identity()),
            token: IntentSourceTokenId(1),
            replacement: "2".to_owned(),
        })
        .unwrap();
        for request in [
            patch.as_str(),
            r#"{"method":"undo"}"#,
            r#"{"method":"redo"}"#,
            source_edit.as_str(),
        ] {
            assert!(request_may_change_identity(request), "{request}");
            let rejection: IntentRpcOutcome =
                serde_json::from_str(&code_authority_rejection(true, request).unwrap()).unwrap();
            assert!(matches!(
                rejection,
                IntentRpcOutcome::Failure { failure }
                    if failure.code == "code_authority_required"
            ));
        }
        for request in [
            r#"{"method":"snapshot"}"#,
            r#"{"method":"inspector","node":"0000000000000001"}"#,
            r#"{"method":"execute_typescript","source":"solve()"}"#,
            r#"{"method":"apply_patch","patch":{}}"#,
            "not json",
        ] {
            assert!(!request_may_change_identity(request), "{request}");
        }
        assert!(!request_may_change_identity(
            &" ".repeat(MAX_INTENT_RPC_REQUEST_BYTES + 1)
        ));
    }

    #[test]
    fn code_authority_rejects_only_strict_nested_intent_mutations() {
        for request in [r#"{"method":"undo"}"#, r#"{"method":"redo"}"#] {
            let response: IntentRpcOutcome =
                serde_json::from_str(&code_authority_rejection(true, request).unwrap()).unwrap();
            assert!(matches!(
                response,
                IntentRpcOutcome::Failure { failure }
                    if failure.code == "code_authority_required"
                        && failure.identity.is_none()
            ));
            assert!(code_authority_rejection(false, request).is_none());
        }
        for request in [
            r#"{"method":"snapshot"}"#,
            r#"{"method":"inspector","node":"0000000000000001"}"#,
            r#"{"method":"undo","extra":true}"#,
            "not json",
        ] {
            assert!(
                code_authority_rejection(true, request).is_none(),
                "{request}"
            );
        }
    }
}
