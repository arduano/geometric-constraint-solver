//! Non-authoritative WASM sketch workbench.

/// Bounded text transport for complete workbench reproduction checkpoints.
pub mod reproduction;

/// DOM-free projectional-intent RPC constructors shared by native and WASM
/// hosts. The returned session executes the Rust-owned typed protocol and
/// existing native solver; this web crate adds no equations.
pub mod intent_rpc {
    use geosolve_constraint_editor::{
        ColdIntentMaterializer, IntentRpcSession, ProjectionalIntentCoordinator,
    };
    use geosolve_sketch::{DocumentId, PersistentId};
    use geosolve_sketch_intent::{IntentSessionId, IntentSessionIdentity};

    /// Creates an empty DOM-free RPC session in explicit persistent namespaces.
    ///
    /// # Errors
    ///
    /// Returns a typed construction failure for an invalid document namespace
    /// or model scale.
    pub fn empty_session(
        session: IntentSessionId,
        document: DocumentId,
        model_scale: f64,
    ) -> Result<IntentRpcSession, String> {
        let materializer = ColdIntentMaterializer::with_default_policy(document, model_scale)
            .map_err(|error| error.to_string())?;
        ProjectionalIntentCoordinator::empty(session, materializer)
            .map(IntentRpcSession::new)
            .map_err(|error| error.to_string())
    }

    /// Convenience constructor for text-only hosts which own no persistent-ID
    /// wrapper types. Zero namespaces remain rejected by their domain owners.
    ///
    /// # Errors
    ///
    /// Returns the same typed construction failure as [`empty_session`].
    pub fn empty_session_from_raw(
        session: u128,
        document: u128,
        model_scale: f64,
    ) -> Result<IntentRpcSession, String> {
        empty_session(
            IntentSessionId::from_raw(session),
            DocumentId(PersistentId::from_u128(document)),
            model_scale,
        )
    }

    /// Stable helper used by native/WASM parity tests.
    #[must_use]
    pub fn identity(session: &IntentRpcSession) -> IntentSessionIdentity {
        session.coordinator().intent().identity()
    }
}

#[cfg(any(target_arch = "wasm32", test))]
mod workbench;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;

    /// DOM-free intent RPC handle. It is independent of the workbench and may
    /// be used by TypeScript hosts or workers without a browser document.
    #[wasm_bindgen]
    #[derive(Debug)]
    pub struct IntentRpcHandle {
        session: geosolve_constraint_editor::IntentRpcSession,
    }

    #[wasm_bindgen]
    impl IntentRpcHandle {
        /// Creates one empty typed RPC session from strict lowercase-hex IDs.
        #[wasm_bindgen(constructor)]
        pub fn new(
            session_id: &str,
            document_id: &str,
            model_scale: f64,
        ) -> Result<IntentRpcHandle, JsValue> {
            let session = session_id
                .parse::<geosolve_sketch_intent::IntentSessionId>()
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            let document = document_id
                .parse::<geosolve_sketch::PersistentId>()
                .map(geosolve_sketch::DocumentId)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            crate::intent_rpc::empty_session(session, document, model_scale)
                .map(|session| Self { session })
                .map_err(|error| JsValue::from_str(&error))
        }

        /// Applies one strict JSON request and returns one compact response.
        pub fn apply(&mut self, request: &str) -> String {
            self.session.apply_json(request)
        }
    }

    #[cfg_attr(not(test), wasm_bindgen(start))]
    pub fn start() -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| JsValue::from_str("browser document is unavailable"))?;
        crate::workbench::wasm::install(&document)
    }

    #[cfg(test)]
    mod tests {
        use geosolve_constraint_editor::{
            IntentRpcOutcome, IntentRpcRequest, IntentRpcSession, IntentRpcSuccess,
            MAX_INTENT_RPC_REQUEST_BYTES,
        };
        use geosolve_sketch_intent::{
            GeometryRecipeKind, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind,
            IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPlanDisposition,
            IntentPortRole, IntentPortSelector, IntentUnit, LeafField,
        };
        use wasm_bindgen_test::wasm_bindgen_test;

        use super::IntentRpcHandle;

        fn point_patch(session: &IntentRpcSession) -> IntentRpcRequest {
            let selector = IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            };
            let draft = IntentNodeDraft::new(
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::SketchPoint,
                },
                IntentKey::new("wasm.rpc.point").unwrap(),
            )
            .with_instance_leaf(
                selector,
                LeafField::X,
                IntentLiteral::Quantity {
                    value: 1.0,
                    unit: IntentUnit::Length,
                },
            )
            .with_instance_leaf(
                selector,
                LeafField::Y,
                IntentLiteral::Quantity {
                    value: 2.0,
                    unit: IntentUnit::Length,
                },
            );
            IntentRpcRequest::ApplyPatch {
                patch: Box::new(IntentPatch::new(
                    session.coordinator().intent().identity(),
                    IntentPatchPolicy::RequireAccepted,
                    vec![IntentPatchOperation::CreateNode {
                        alias: IntentKey::new("point").unwrap(),
                        draft: Box::new(draft),
                        cell: None,
                    }],
                )),
            }
        }

        fn retained_invalid_patch(session: &IntentRpcSession) -> IntentRpcRequest {
            let start = IntentPortSelector::Node {
                role: IntentPortRole::Start,
                index: 0,
            };
            let end = IntentPortSelector::Node {
                role: IntentPortRole::End,
                index: 0,
            };
            let draft = IntentNodeDraft::new(
                IntentNodeKind::Geometry {
                    recipe: GeometryRecipeKind::Segment,
                },
                IntentKey::new("wasm.rpc.invalid.segment").unwrap(),
            )
            .with_instance_leaf(
                start,
                LeafField::X,
                IntentLiteral::Quantity {
                    value: 0.0,
                    unit: IntentUnit::Length,
                },
            )
            .with_instance_leaf(
                start,
                LeafField::Y,
                IntentLiteral::Quantity {
                    value: 0.0,
                    unit: IntentUnit::Length,
                },
            )
            .with_instance_leaf(
                end,
                LeafField::X,
                IntentLiteral::Quantity {
                    value: 1.0,
                    unit: IntentUnit::Length,
                },
            )
            .with_instance_leaf(
                end,
                LeafField::Y,
                IntentLiteral::Quantity {
                    value: 0.0,
                    unit: IntentUnit::Length,
                },
            )
            .with_field(
                geosolve_sketch_intent::IntentFieldKey(IntentKey::new("branch_direction").unwrap()),
                IntentLiteral::Point([f64::MAX, f64::MAX]),
            );
            IntentRpcRequest::ApplyPatch {
                patch: Box::new(IntentPatch::new(
                    session.coordinator().intent().identity(),
                    IntentPatchPolicy::RetainFailedIntent,
                    vec![IntentPatchOperation::CreateNode {
                        alias: IntentKey::new("invalid").unwrap(),
                        draft: Box::new(draft),
                        cell: None,
                    }],
                )),
            }
        }

        #[wasm_bindgen_test]
        fn actual_wasm_handle_matches_dom_free_rust_session_for_transition_matrix() {
            let session = "00000000000000000000000083005001";
            let document = "00000000000000000000830050010000";
            let mut handle = IntentRpcHandle::new(session, document, 1.0).unwrap();
            let mut rust =
                crate::intent_rpc::empty_session_from_raw(0x8300_5001, 0x8300_5001_0000, 1.0)
                    .unwrap();
            let pristine_identity = rust.coordinator().intent().identity();
            let patch = serde_json::to_string(&point_patch(&rust)).unwrap();
            for request in [
                r#"{"method":"snapshot"}"#.to_owned(),
                patch,
                r#"{"method":"undo"}"#.to_owned(),
                r#"{"method":"redo"}"#.to_owned(),
                r#"{"method":"inspector","node":"0000000000000063"}"#.to_owned(),
            ] {
                assert_eq!(
                    handle.apply(&request),
                    rust.apply_json(&request),
                    "{request}"
                );
            }

            let accepted_identity = rust.coordinator().intent().identity();
            let accepted_evidence = handle
                .session
                .coordinator()
                .accepted_materialization()
                .expect("accepted WASM authority")
                .evidence
                .clone();
            assert_eq!(
                rust.coordinator()
                    .accepted_materialization()
                    .expect("accepted native authority")
                    .evidence,
                accepted_evidence
            );

            let retained_request = serde_json::to_string(&retained_invalid_patch(&rust)).unwrap();
            let retained_response = handle.apply(&retained_request);
            assert_eq!(retained_response, rust.apply_json(&retained_request));
            let retained_response: IntentRpcOutcome =
                serde_json::from_str(&retained_response).unwrap();
            assert!(matches!(
                retained_response,
                IntentRpcOutcome::Success {
                    value: IntentRpcSuccess::Patch {
                        disposition: IntentPlanDisposition::RetainedFailed,
                        ..
                    }
                }
            ));
            assert_ne!(rust.coordinator().intent().identity(), accepted_identity);
            assert_eq!(
                handle
                    .session
                    .coordinator()
                    .accepted_materialization()
                    .expect("retained WASM authority")
                    .evidence,
                accepted_evidence
            );
            assert_eq!(
                rust.coordinator()
                    .accepted_materialization()
                    .expect("retained native authority")
                    .evidence,
                accepted_evidence
            );

            let retained_identity = rust.coordinator().intent().identity();
            let retained_intent = rust.coordinator().intent().to_canonical_json().unwrap();
            let retained_snapshot = rust.snapshot();
            assert_eq!(handle.session.snapshot(), retained_snapshot);

            let stale = serde_json::to_string(&IntentRpcRequest::ApplyPatch {
                patch: Box::new(IntentPatch::new(
                    pristine_identity,
                    IntentPatchPolicy::RequireAccepted,
                    Vec::new(),
                )),
            })
            .unwrap();
            let malformed = r#"{"method":"execute_typescript","source":"solve()"}"#.to_owned();
            let oversized = " ".repeat(MAX_INTENT_RPC_REQUEST_BYTES + 1);
            for (request, expected_code) in [
                (stale, "patch_rejected"),
                (malformed, "invalid_request"),
                (oversized, "request_too_large"),
            ] {
                let wasm_response = handle.apply(&request);
                assert_eq!(wasm_response, rust.apply_json(&request), "{expected_code}");
                let response: IntentRpcOutcome = serde_json::from_str(&wasm_response).unwrap();
                assert!(matches!(
                    response,
                    IntentRpcOutcome::Failure { ref failure }
                        if failure.code == expected_code
                            && failure.identity == Some(retained_identity)
                ));
                assert_eq!(
                    handle
                        .session
                        .coordinator()
                        .intent()
                        .to_canonical_json()
                        .unwrap(),
                    retained_intent
                );
                assert_eq!(
                    rust.coordinator().intent().to_canonical_json().unwrap(),
                    retained_intent
                );
                assert_eq!(handle.session.snapshot(), retained_snapshot);
                assert_eq!(rust.snapshot(), retained_snapshot);
                assert_eq!(
                    handle
                        .session
                        .coordinator()
                        .accepted_materialization()
                        .expect("rejection retains WASM authority")
                        .evidence,
                    accepted_evidence
                );
                assert_eq!(
                    rust.coordinator()
                        .accepted_materialization()
                        .expect("rejection retains native authority")
                        .evidence,
                    accepted_evidence
                );
            }
        }
    }
}
