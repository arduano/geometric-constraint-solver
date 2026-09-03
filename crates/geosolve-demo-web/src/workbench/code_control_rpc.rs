// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict RPC over the optional code workbench's outer managed authority.
//!
//! This protocol is deliberately separate from Intent RPC. Read-only managed
//! control inspection and outer code history address
//! [`SketchCodeSession`](geosolve_sketch_code::SketchCodeSession), whose
//! checkpoint contains the delegated projectional editor. Source mutations do
//! not belong here: the presentation bridge routes them through the prepared
//! compiler transaction.

use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch_code::{
    CodeSessionIdentity, CodeSessionReceipt, MAX_CODE_SESSION_WIRE_INTEGER, ManagedControlManifest,
};
use serde::{Deserialize, Serialize};

use super::code_projects::CodeProjectWorkbench;

pub(crate) const MAX_CODE_CONTROL_RPC_REQUEST_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_CODE_CONTROL_RPC_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_FAILURE_MESSAGE_BYTES: usize = 64 * 1024;

/// Closed request vocabulary for read-only managed controls and outer
/// code-project history. Source edits use the prepared compiler bridge.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CodeControlRpcRequest {
    InspectManagedControls {},
    Undo { expected: Box<CodeSessionIdentity> },
    Redo { expected: Box<CodeSessionIdentity> },
}

/// Read-only control manifest plus exact outer-history availability.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodeControlRpcSnapshot {
    pub(crate) identity: CodeSessionIdentity,
    pub(crate) manifest: ManagedControlManifest,
    pub(crate) can_undo: bool,
    pub(crate) can_redo: bool,
}

/// Fixed outer-history result. A moved history cursor always includes the
/// exact code-session receipt which produced `identity`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodeControlRpcHistoryReceipt {
    pub(crate) identity: CodeSessionIdentity,
    pub(crate) moved: bool,
    pub(crate) receipt: Option<CodeSessionReceipt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CodeControlRpcSuccess {
    ManagedControls {
        snapshot: Box<CodeControlRpcSnapshot>,
    },
    History {
        receipt: Box<CodeControlRpcHistoryReceipt>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodeControlRpcFailure {
    pub(crate) code: String,
    pub(crate) message: String,
    pub(crate) identity: Option<CodeSessionIdentity>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CodeControlRpcOutcome {
    Success { value: CodeControlRpcSuccess },
    Failure { failure: Box<CodeControlRpcFailure> },
}

/// Non-serializable publication returned beside one already encoded receipt.
/// The instance-scoped workbench bridge publishes this editor only after the
/// outer code session has accepted the matching checkpoint.
pub(crate) struct CodeControlRpcApplication {
    pub(crate) response: String,
    pub(crate) editor: Option<Box<ProjectionalEditorSession>>,
    pub(crate) identity_changed: bool,
}

/// Returns whether one strict bounded request may move outer authority.
pub(crate) fn request_may_change_identity(request: &str) -> bool {
    if request.len() > MAX_CODE_CONTROL_RPC_REQUEST_BYTES {
        return false;
    }
    serde_json::from_str::<CodeControlRpcRequest>(request).is_ok_and(|request| {
        request_expected_identity(&request).is_some_and(code_session_identity_is_wire_safe)
    })
}

/// Applies strict JSON to one outer code workbench. Decode, authentication and
/// history failures publish neither source nor history.
// Keep request authentication, dispatch, bounded response encoding, and the
// resulting identity comparison together as one auditable RPC transaction.
#[allow(clippy::too_many_lines)]
pub(crate) fn apply_to_code_project(
    code: &mut CodeProjectWorkbench,
    request: &str,
) -> CodeControlRpcApplication {
    let before = code.code_session_identity().clone();
    if request.len() > MAX_CODE_CONTROL_RPC_REQUEST_BYTES {
        return application_failure(
            "request_too_large",
            &format!("code-control RPC request exceeds {MAX_CODE_CONTROL_RPC_REQUEST_BYTES} bytes"),
            Some(before),
        );
    }
    let request = match serde_json::from_str::<CodeControlRpcRequest>(request) {
        Ok(request) => request,
        Err(error) => {
            return application_failure(
                "invalid_request",
                &format!("invalid code-control RPC request: {error}"),
                Some(before),
            );
        }
    };
    if request_expected_identity(&request)
        .is_some_and(|identity| !code_session_identity_is_wire_safe(identity))
    {
        return application_failure(
            "invalid_request",
            "code-control session and revision identities must be JavaScript-safe unsigned integers",
            Some(before),
        );
    }
    let mut editor = None;
    let mutating = !matches!(&request, CodeControlRpcRequest::InspectManagedControls {});
    let outcome =
        match request {
            CodeControlRpcRequest::InspectManagedControls {} => code
                .managed_controls()
                .map(|manifest| CodeControlRpcSuccess::ManagedControls {
                    snapshot: Box::new(CodeControlRpcSnapshot {
                        identity: code.code_session_identity().clone(),
                        manifest,
                        can_undo: code.can_undo(),
                        can_redo: code.can_redo(),
                    }),
                })
                .map_err(|error| ("control_inspection_rejected", error)),
            CodeControlRpcRequest::Undo { expected } => authenticate_expected(code, &expected)
                .and_then(|()| history(code, true, &mut editor)),
            CodeControlRpcRequest::Redo { expected } => authenticate_expected(code, &expected)
                .and_then(|()| history(code, false, &mut editor)),
        };
    let outcome = outcome.map_or_else(
        |(code_name, message)| {
            failure(
                code_name,
                &message,
                Some(code.code_session_identity().clone()),
            )
        },
        |value| CodeControlRpcOutcome::Success { value },
    );
    let after = code.code_session_identity().clone();
    let response_limit = if mutating {
        MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES
    } else {
        MAX_CODE_CONTROL_RPC_RESPONSE_BYTES
    };
    let response = encode_with_limit(&outcome, response_limit, Some(after.clone()));
    CodeControlRpcApplication {
        response,
        editor,
        identity_changed: before != after,
    }
}

fn request_expected_identity(request: &CodeControlRpcRequest) -> Option<&CodeSessionIdentity> {
    match request {
        CodeControlRpcRequest::InspectManagedControls {} => None,
        CodeControlRpcRequest::Undo { expected } | CodeControlRpcRequest::Redo { expected } => {
            Some(expected)
        }
    }
}

const fn code_session_identity_is_wire_safe(identity: &CodeSessionIdentity) -> bool {
    identity.session <= MAX_CODE_SESSION_WIRE_INTEGER
        && identity.revision <= MAX_CODE_SESSION_WIRE_INTEGER
}

fn authenticate_expected(
    code: &CodeProjectWorkbench,
    expected: &CodeSessionIdentity,
) -> Result<(), (&'static str, String)> {
    if code.code_session_identity() == expected {
        Ok(())
    } else {
        Err((
            "stale_code_session",
            "code-control request belongs to stale or foreign outer authority".into(),
        ))
    }
}

fn history(
    code: &mut CodeProjectWorkbench,
    undo: bool,
    editor: &mut Option<Box<ProjectionalEditorSession>>,
) -> Result<CodeControlRpcSuccess, (&'static str, String)> {
    let publication = code
        .step_history(undo)
        .map_err(|error| ("history_rejected", error))?;
    let receipt = publication.map(|publication| {
        *editor = Some(publication.editor);
        publication.receipt
    });
    Ok(CodeControlRpcSuccess::History {
        receipt: Box::new(CodeControlRpcHistoryReceipt {
            identity: code.code_session_identity().clone(),
            moved: receipt.is_some(),
            receipt,
        }),
    })
}

fn application_failure(
    code: &str,
    message: &str,
    identity: Option<CodeSessionIdentity>,
) -> CodeControlRpcApplication {
    CodeControlRpcApplication {
        response: encode_failure(code, message, identity),
        editor: None,
        identity_changed: false,
    }
}

fn failure(
    code: &str,
    message: &str,
    identity: Option<CodeSessionIdentity>,
) -> CodeControlRpcOutcome {
    CodeControlRpcOutcome::Failure {
        failure: Box::new(CodeControlRpcFailure {
            code: code.to_owned(),
            message: bounded_text(message),
            identity,
        }),
    }
}

pub(crate) fn encode_failure(
    code: &str,
    message: &str,
    identity: Option<CodeSessionIdentity>,
) -> String {
    serde_json::to_string(&failure(code, message, identity))
        .expect("closed code-control failure is infallibly serializable")
}

fn encode_with_limit(
    outcome: &CodeControlRpcOutcome,
    maximum: usize,
    identity: Option<CodeSessionIdentity>,
) -> String {
    let encoded = serde_json::to_string(outcome)
        .expect("closed code-control response is infallibly serializable");
    if encoded.len() <= maximum {
        return encoded;
    }
    encode_failure(
        "response_too_large",
        &format!("code-control RPC response exceeds {maximum} bytes"),
        identity,
    )
}

fn bounded_text(value: &str) -> String {
    if value.len() <= MAX_FAILURE_MESSAGE_BYTES {
        return value.to_owned();
    }
    let mut end = MAX_FAILURE_MESSAGE_BYTES;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

#[cfg(test)]
mod tests {
    use super::{
        CodeControlRpcOutcome, CodeControlRpcSuccess, MAX_CODE_CONTROL_RPC_REQUEST_BYTES,
        apply_to_code_project, request_may_change_identity,
    };
    use crate::workbench::code_projects::CodeProjectWorkbench;

    #[test]
    fn strict_rpc_inspects_controls_without_moving_outer_authority() {
        let (mut code, _editor) = CodeProjectWorkbench::open_key("typed-panel").unwrap();
        let before = code.code_session_identity().clone();
        let inspected =
            apply_to_code_project(&mut code, r#"{"method":"inspect_managed_controls"}"#);
        assert!(!inspected.identity_changed);
        assert!(inspected.editor.is_none());
        let inspected: CodeControlRpcOutcome = serde_json::from_str(&inspected.response).unwrap();
        let CodeControlRpcOutcome::Success {
            value: CodeControlRpcSuccess::ManagedControls { snapshot },
        } = inspected
        else {
            panic!("managed controls must inspect successfully")
        };
        assert_eq!(snapshot.identity, before);
        assert!(snapshot
            .manifest
            .controls
            .iter()
            .any(|control| {
                control.source.declaration.0 == "cornerFillets"
                    && control.source.path.0.iter().any(|segment| {
                        matches!(segment, geosolve_sketch_code::ManagedPathSegment::Field(field) if field == "radius")
                    })
            }));
    }

    #[test]
    fn request_vocabulary_is_strict_bounded_and_classifies_only_mutations() {
        for request in [
            r#"{"method":"edit_managed_controls","expected":{},"batch":{}}"#,
            r#"{"method":"undo","expected":{}}"#,
            r#"{"method":"redo","expected":{}}"#,
        ] {
            // Malformed payloads are not admitted as authenticated mutations.
            assert!(!request_may_change_identity(request));
        }
        assert!(!request_may_change_identity(
            r#"{"method":"inspect_managed_controls"}"#
        ));
        assert!(!request_may_change_identity(
            &" ".repeat(MAX_CODE_CONTROL_RPC_REQUEST_BYTES + 1)
        ));

        let (code, _editor) = CodeProjectWorkbench::open_key("typed-panel").unwrap();
        let mut code = Box::new(code);
        for request in [
            r#"{"method":"inspect_managed_controls","extra":true}"#,
            r#"{"method":"intent_patch"}"#,
            "not json",
        ] {
            let before = code.code_session_identity().clone();
            let result = apply_to_code_project(&mut code, request);
            assert!(!result.identity_changed);
            assert!(result.editor.is_none());
            assert_eq!(code.code_session_identity(), &before);
            assert!(matches!(
                serde_json::from_str::<CodeControlRpcOutcome>(&result.response).unwrap(),
                CodeControlRpcOutcome::Failure { failure }
                    if failure.code == "invalid_request"
            ));
        }
    }

    #[test]
    fn unsafe_integer_session_identities_match_the_typescript_wire_rejection() {
        let (code, _editor) = CodeProjectWorkbench::open_key("typed-panel").unwrap();
        let mut code = Box::new(code);
        let before = code.code_session_identity().clone();
        for (session, revision) in [
            (9_007_199_254_740_992_u64, before.revision),
            (before.session, 9_007_199_254_740_992_u64),
        ] {
            let request = serde_json::json!({
                "method": "undo",
                "expected": {
                    "session": session,
                    "revision": revision,
                    "digest": before.digest.clone(),
                },
            })
            .to_string();
            assert!(!request_may_change_identity(&request));
            let result = apply_to_code_project(&mut code, &request);
            assert!(!result.identity_changed);
            assert!(result.editor.is_none());
            assert_eq!(code.code_session_identity(), &before);
            assert!(matches!(
                serde_json::from_str::<CodeControlRpcOutcome>(&result.response).unwrap(),
                CodeControlRpcOutcome::Failure { failure }
                    if failure.code == "invalid_request"
                        && failure.message.contains("JavaScript-safe")
            ));
        }
    }
}
