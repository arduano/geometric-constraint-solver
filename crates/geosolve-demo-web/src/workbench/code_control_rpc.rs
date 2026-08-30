// SPDX-License-Identifier: GPL-3.0-or-later

//! Strict RPC over the optional code workbench's outer managed authority.
//!
//! This protocol is deliberately separate from Intent RPC. Managed control
//! edits and history move [`SketchCodeSession`](geosolve_sketch_code::SketchCodeSession),
//! whose checkpoint contains the delegated projectional editor. Routing them
//! through Intent RPC would expose the wrong Undo stack and bypass managed
//! source authentication.

use std::cell::RefCell;
use std::rc::Rc;

use geosolve_constraint_editor::ProjectionalEditorSession;
use geosolve_sketch_code::{
    CodeSessionIdentity, CodeSessionReceipt, MAX_CODE_SESSION_WIRE_INTEGER,
    ManagedControlEditBatch, ManagedControlManifest,
};
use serde::{Deserialize, Serialize};

use super::code_projects::{CodeApplyOutcome, CodeProjectWorkbench};

pub(crate) const MAX_CODE_CONTROL_RPC_REQUEST_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_CODE_CONTROL_RPC_MUTATION_RECEIPT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_CODE_CONTROL_RPC_RESPONSE_BYTES: usize = 64 * 1024 * 1024;
const MAX_FAILURE_MESSAGE_BYTES: usize = 64 * 1024;

type InstalledCodeControlRpc = dyn Fn(&str) -> String;

std::thread_local! {
    static INSTALLED_CODE_CONTROL_RPC: RefCell<Option<Rc<InstalledCodeControlRpc>>> =
        RefCell::new(None);
}

/// Closed version-one request vocabulary for managed controls and outer
/// code-project history.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "method", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum CodeControlRpcRequest {
    InspectManagedControls {},
    EditManagedControls {
        expected: Box<CodeSessionIdentity>,
        batch: Box<ManagedControlEditBatch>,
    },
    Undo {
        expected: Box<CodeSessionIdentity>,
    },
    Redo {
        expected: Box<CodeSessionIdentity>,
    },
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

/// Accepted or retained-failure result of one source-backed control batch.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CodeControlRpcEditReceipt {
    pub(crate) receipt: CodeSessionReceipt,
    pub(crate) diagnostic: Option<String>,
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
    ManagedControlEdit {
        receipt: Box<CodeControlRpcEditReceipt>,
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
/// The WASM adapter installs this editor only after the outer code session has
/// accepted the matching checkpoint.
pub(crate) struct CodeControlRpcApplication {
    pub(crate) response: String,
    pub(crate) editor: Option<Box<ProjectionalEditorSession>>,
    pub(crate) identity_changed: bool,
}

/// Installs the callback which closes over the browser's one code workbench.
#[cfg_attr(test, allow(dead_code, reason = "installed only by the WASM adapter"))]
pub(crate) fn install(handler: impl Fn(&str) -> String + 'static) {
    INSTALLED_CODE_CONTROL_RPC.with(|installed| {
        *installed.borrow_mut() = Some(Rc::new(handler));
    });
}

/// Retires a prior callback before another workbench is installed.
#[cfg_attr(
    test,
    allow(dead_code, reason = "called only by the WASM startup adapter")
)]
pub(crate) fn clear() {
    INSTALLED_CODE_CONTROL_RPC.with(|installed| {
        installed.borrow_mut().take();
    });
}

/// Applies one request to the installed workbench callback.
pub(crate) fn apply_installed(request: &str) -> String {
    let handler = INSTALLED_CODE_CONTROL_RPC.with(|installed| installed.borrow().clone());
    handler.map_or_else(
        || {
            encode_failure(
                "code_workbench_unavailable",
                "the managed code workbench is not installed",
                None,
            )
        },
        |handler| handler(request),
    )
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

/// Applies strict JSON to one outer code workbench. Parse, authentication and
/// control failures publish neither source nor history. A retained native
/// failure deliberately advances outer history while returning no replacement
/// editor, leaving the prior accepted canvas authoritative.
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
            CodeControlRpcRequest::EditManagedControls { expected, batch } => {
                authenticate_expected(code, &expected).and_then(|()| {
                    code.apply_managed_controls(&expected, &batch)
                        .map(|result| match result {
                            CodeApplyOutcome::Accepted(publication) => {
                                let receipt = publication.receipt;
                                editor = Some(publication.editor);
                                CodeControlRpcSuccess::ManagedControlEdit {
                                    receipt: Box::new(CodeControlRpcEditReceipt {
                                        receipt,
                                        diagnostic: None,
                                    }),
                                }
                            }
                            CodeApplyOutcome::RetainedFailure {
                                receipt,
                                diagnostic,
                            } => CodeControlRpcSuccess::ManagedControlEdit {
                                receipt: Box::new(CodeControlRpcEditReceipt {
                                    receipt,
                                    diagnostic: Some(bounded_text(&diagnostic)),
                                }),
                            },
                        })
                        .map_err(|error| ("control_edit_rejected", error))
                })
            }
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
        CodeControlRpcRequest::EditManagedControls { expected, .. }
        | CodeControlRpcRequest::Undo { expected }
        | CodeControlRpcRequest::Redo { expected } => Some(expected),
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
    use geosolve_sketch_code::{
        CodeSessionIdentity, ManagedControlAccess, ManagedControlEdit, ManagedControlEditBatch,
        ManagedPathSegment, ManagedValue, UnitLiteral,
    };

    use super::{
        CodeControlRpcOutcome, CodeControlRpcRequest, CodeControlRpcSuccess,
        MAX_CODE_CONTROL_RPC_REQUEST_BYTES, apply_installed, apply_to_code_project,
        request_may_change_identity,
    };
    use crate::workbench::code_projects::CodeProjectWorkbench;

    fn typed_panel_radius_edit(
        code: &mut CodeProjectWorkbench,
    ) -> (CodeSessionIdentity, ManagedControlEditBatch) {
        let inspected = apply_to_code_project(code, r#"{"method":"inspect_managed_controls"}"#);
        assert!(!inspected.identity_changed);
        assert!(inspected.editor.is_none());
        let inspected: CodeControlRpcOutcome = serde_json::from_str(&inspected.response).unwrap();
        let CodeControlRpcOutcome::Success {
            value: CodeControlRpcSuccess::ManagedControls { snapshot },
        } = inspected
        else {
            panic!("managed controls must inspect successfully")
        };
        let control = snapshot
            .manifest
            .controls
            .iter()
            .find(|control| {
                control.source.declaration.0 == "cornerFillets"
                    && control.source.path.0 == [ManagedPathSegment::Field("radius".into())]
            })
            .expect("Typed Panel radius control");
        assert_eq!(control.consumers.len(), 2);
        let ManagedControlAccess::Editable { token } = &control.access else {
            panic!("Typed Panel radius must be editable")
        };
        let batch = ManagedControlEditBatch::new([ManagedControlEdit {
            token: token.clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: 2.0,
            }),
        }]);
        (snapshot.identity, batch)
    }

    #[test]
    fn strict_rpc_edits_typed_panel_and_moves_only_outer_history() {
        let (code, _editor) = CodeProjectWorkbench::open_key("typed-panel").unwrap();
        let mut code = Box::new(code);
        let (original_identity, batch) = typed_panel_radius_edit(&mut code);
        let edit = CodeControlRpcRequest::EditManagedControls {
            expected: Box::new(original_identity.clone()),
            batch: Box::new(batch.clone()),
        };
        let encoded = serde_json::to_string(&edit).unwrap();
        let edited = apply_to_code_project(&mut code, &encoded);
        assert!(edited.identity_changed);
        assert!(edited.editor.is_some());
        assert_eq!(
            edited
                .editor
                .as_ref()
                .unwrap()
                .coordinator()
                .intent()
                .undo_len(),
            0
        );
        assert!(code.managed_source().contains("radius: mm(2)"));
        assert!(!code.managed_source().contains("radius: mm(4)"));
        assert!(matches!(
            serde_json::from_str::<CodeControlRpcOutcome>(&edited.response).unwrap(),
            CodeControlRpcOutcome::Success {
                value: CodeControlRpcSuccess::ManagedControlEdit { receipt }
            } if receipt.receipt.before == original_identity
                && receipt.receipt.after == *code.code_session_identity()
                && !receipt.receipt.retained_failure
                && receipt.diagnostic.is_none()
        ));

        let stale = apply_to_code_project(&mut code, &encoded);
        assert!(!stale.identity_changed);
        assert!(stale.editor.is_none());
        assert!(matches!(
            serde_json::from_str::<CodeControlRpcOutcome>(&stale.response).unwrap(),
            CodeControlRpcOutcome::Failure { failure }
                if failure.code == "stale_code_session"
        ));

        let stale_token = serde_json::to_string(&CodeControlRpcRequest::EditManagedControls {
            expected: Box::new(code.code_session_identity().clone()),
            batch: Box::new(batch),
        })
        .unwrap();
        let stale_token = apply_to_code_project(&mut code, &stale_token);
        assert!(!stale_token.identity_changed);
        assert!(stale_token.editor.is_none());
        assert!(matches!(
            serde_json::from_str::<CodeControlRpcOutcome>(&stale_token.response).unwrap(),
            CodeControlRpcOutcome::Failure { failure }
                if failure.code == "control_edit_rejected"
                    && failure.message.contains("stale")
        ));

        let undo = serde_json::to_string(&CodeControlRpcRequest::Undo {
            expected: Box::new(code.code_session_identity().clone()),
        })
        .unwrap();
        let undone = apply_to_code_project(&mut code, &undo);
        assert!(undone.identity_changed);
        assert!(undone.editor.is_some());
        assert!(code.managed_source().contains("radius: mm(4)"));
        assert!(matches!(
            serde_json::from_str::<CodeControlRpcOutcome>(&undone.response).unwrap(),
            CodeControlRpcOutcome::Success {
                value: CodeControlRpcSuccess::History { receipt }
            } if receipt.moved && receipt.receipt.is_some()
        ));

        let redo = serde_json::to_string(&CodeControlRpcRequest::Redo {
            expected: Box::new(code.code_session_identity().clone()),
        })
        .unwrap();
        let redone = apply_to_code_project(&mut code, &redo);
        assert!(redone.identity_changed);
        assert!(redone.editor.is_some());
        assert!(code.managed_source().contains("radius: mm(2)"));
    }

    #[test]
    fn outer_history_rejects_dirty_source_without_erasing_undo_or_redo_drafts() {
        let (code, _editor) = CodeProjectWorkbench::open_key("typed-panel").unwrap();
        let mut code = Box::new(code);
        let (identity, batch) = typed_panel_radius_edit(&mut code);
        let edit = serde_json::to_string(&CodeControlRpcRequest::EditManagedControls {
            expected: Box::new(identity),
            batch: Box::new(batch),
        })
        .unwrap();
        assert!(apply_to_code_project(&mut code, &edit).identity_changed);

        let dirty_undo = code.managed_source().replacen("mm(2)", "mm(3)", 1);
        code.set_managed_draft(dirty_undo);
        let before_undo = code.code_session_identity().clone();
        let undo = serde_json::to_string(&CodeControlRpcRequest::Undo {
            expected: Box::new(before_undo.clone()),
        })
        .unwrap();
        let rejected = apply_to_code_project(&mut code, &undo);
        assert!(!rejected.identity_changed);
        assert_eq!(code.code_session_identity(), &before_undo);
        assert!(code.is_dirty());
        assert!(code.panel_markup().contains("mm(3)"));
        assert!(matches!(
            serde_json::from_str::<CodeControlRpcOutcome>(&rejected.response).unwrap(),
            CodeControlRpcOutcome::Failure { failure }
                if failure.code == "history_rejected"
                    && failure.message.contains("Apply or Revert")
        ));

        assert!(code.revert_managed_draft());
        assert!(apply_to_code_project(&mut code, &undo).identity_changed);
        let dirty_redo = code.managed_source().replacen("mm(4)", "mm(3)", 1);
        code.set_managed_draft(dirty_redo);
        let before_redo = code.code_session_identity().clone();
        let redo = serde_json::to_string(&CodeControlRpcRequest::Redo {
            expected: Box::new(before_redo.clone()),
        })
        .unwrap();
        let rejected = apply_to_code_project(&mut code, &redo);
        assert!(!rejected.identity_changed);
        assert_eq!(code.code_session_identity(), &before_redo);
        assert!(code.is_dirty());
        assert!(code.panel_markup().contains("mm(3)"));
    }

    #[test]
    fn retained_failure_advances_outer_source_and_undo_recovers_accepted_authority() {
        let (code, _editor) = CodeProjectWorkbench::open_key("typed-panel").unwrap();
        let mut code = Box::new(code);
        let (identity, mut batch) = typed_panel_radius_edit(&mut code);
        batch.edits[0].value = ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 400.0,
        });
        let edit = serde_json::to_string(&CodeControlRpcRequest::EditManagedControls {
            expected: Box::new(identity),
            batch: Box::new(batch),
        })
        .unwrap();
        let retained = apply_to_code_project(&mut code, &edit);
        assert!(retained.identity_changed);
        assert!(retained.editor.is_none());
        assert!(code.managed_source().contains("radius: mm(400)"));
        assert!(matches!(
            serde_json::from_str::<CodeControlRpcOutcome>(&retained.response).unwrap(),
            CodeControlRpcOutcome::Success {
                value: CodeControlRpcSuccess::ManagedControlEdit { receipt }
            } if receipt.receipt.retained_failure
                && receipt.diagnostic.as_ref().is_some_and(|value| !value.is_empty())
        ));

        let undo = serde_json::to_string(&CodeControlRpcRequest::Undo {
            expected: Box::new(code.code_session_identity().clone()),
        })
        .unwrap();
        let recovered = apply_to_code_project(&mut code, &undo);
        assert!(recovered.identity_changed);
        assert!(recovered.editor.is_some());
        assert!(code.managed_source().contains("radius: mm(4)"));
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

    #[test]
    fn unavailable_bridge_returns_the_code_control_failure_envelope() {
        let response: CodeControlRpcOutcome =
            serde_json::from_str(&apply_installed(r#"{"method":"inspect_managed_controls"}"#))
                .unwrap();
        assert!(matches!(
            response,
            CodeControlRpcOutcome::Failure { failure }
                if failure.code == "code_workbench_unavailable"
                    && failure.identity.is_none()
        ));
    }
}
