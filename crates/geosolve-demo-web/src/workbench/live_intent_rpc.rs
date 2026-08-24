// SPDX-License-Identifier: GPL-3.0-or-later

//! Bounded process-local bridge to the projectional workbench installed by WASM.
//!
//! The registry stores only a callback into the one browser workbench. It owns
//! no intent session, solver, history, or accepted-scene authority.

use std::cell::RefCell;
use std::rc::Rc;

use geosolve_constraint_editor::{
    IntentRpcFailure, IntentRpcOutcome, IntentRpcRequest, MAX_INTENT_RPC_REQUEST_BYTES,
};

type InstalledIntentRpc = dyn Fn(&str) -> String;

std::thread_local! {
    static INSTALLED_INTENT_RPC: RefCell<Option<Rc<InstalledIntentRpc>>> = RefCell::new(None);
}

/// Presentation work admitted after one live RPC request.
///
/// A transient cancellation may repaint the canvas, but only an exact durable
/// intent/history identity change may rebuild panels or save the workspace.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct LiveIntentRpcPresentationPolicy {
    pub(super) render_transient: bool,
    pub(super) render_durable: bool,
    pub(super) save_workspace: bool,
}

impl LiveIntentRpcPresentationPolicy {
    pub(super) const fn after_request(
        identity_changed: bool,
        transient_reconciliation_required: bool,
    ) -> Self {
        if identity_changed {
            Self {
                render_transient: false,
                render_durable: true,
                save_workspace: true,
            }
        } else {
            Self {
                render_transient: transient_reconciliation_required,
                render_durable: false,
                save_workspace: false,
            }
        }
    }
}

/// Installs the callback that closes over the browser's one live workbench.
#[cfg_attr(test, allow(dead_code, reason = "installed only by the WASM adapter"))]
pub(crate) fn install(handler: impl Fn(&str) -> String + 'static) {
    INSTALLED_INTENT_RPC.with(|installed| {
        *installed.borrow_mut() = Some(Rc::new(handler));
    });
}

/// Retires any callback left by an earlier startup attempt before installing a
/// new browser workbench.
#[cfg_attr(
    test,
    allow(dead_code, reason = "called only by the WASM startup adapter")
)]
pub(super) fn clear() {
    INSTALLED_INTENT_RPC.with(|installed| {
        installed.borrow_mut().take();
    });
}

/// Applies a request to the installed callback, or returns the canonical RPC
/// failure shape when startup has not installed a projectional workbench yet.
pub(crate) fn apply_installed(request: &str) -> String {
    let handler = INSTALLED_INTENT_RPC.with(|installed| installed.borrow().clone());
    handler.map_or_else(
        || {
            failure_response(
                "workbench_unavailable",
                "the projectional workbench is not installed",
            )
        },
        |handler| handler(request),
    )
}

/// Returns whether a strict, bounded request can alter durable intent/history.
///
/// This parses only the closed Rust request tag so malformed or oversized
/// messages cannot cancel an in-progress canvas interaction.
pub(super) fn request_may_change_identity(request: &str) -> bool {
    if request.len() > MAX_INTENT_RPC_REQUEST_BYTES {
        return false;
    }
    matches!(
        serde_json::from_str::<IntentRpcRequest>(request),
        Ok(IntentRpcRequest::ApplyPatch { .. }
            | IntentRpcRequest::Undo
            | IntentRpcRequest::Redo
            | IntentRpcRequest::EditSourceToken { .. })
    )
}

pub(super) fn failure_response(code: &str, message: &str) -> String {
    serde_json::to_string(&IntentRpcOutcome::Failure {
        failure: IntentRpcFailure {
            code: code.to_owned(),
            message: message.to_owned(),
            identity: None,
        },
    })
    .expect("closed RPC failure response is infallibly serializable")
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{
        IntentRpcOutcome, IntentRpcRequest, IntentSourceTokenId, MAX_INTENT_RPC_REQUEST_BYTES,
    };
    use geosolve_sketch_intent::{IntentPatch, IntentPatchPolicy};

    use super::{LiveIntentRpcPresentationPolicy, apply_installed, request_may_change_identity};

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
    fn durable_identity_is_the_only_save_and_panel_rebuild_gate() {
        assert_eq!(
            LiveIntentRpcPresentationPolicy::after_request(false, false),
            LiveIntentRpcPresentationPolicy::default()
        );
        assert_eq!(
            LiveIntentRpcPresentationPolicy::after_request(false, true),
            LiveIntentRpcPresentationPolicy {
                render_transient: true,
                render_durable: false,
                save_workspace: false,
            }
        );
        assert_eq!(
            LiveIntentRpcPresentationPolicy::after_request(true, true),
            LiveIntentRpcPresentationPolicy {
                render_transient: false,
                render_durable: true,
                save_workspace: true,
            }
        );
    }

    #[test]
    fn unavailable_bridge_returns_the_existing_canonical_rpc_envelope() {
        let response: IntentRpcOutcome =
            serde_json::from_str(&apply_installed(r#"{"method":"snapshot"}"#)).unwrap();
        assert!(matches!(
            response,
            IntentRpcOutcome::Failure { failure }
                if failure.code == "workbench_unavailable" && failure.identity.is_none()
        ));
    }
}
