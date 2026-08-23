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
        use geosolve_constraint_editor::{IntentRpcRequest, IntentRpcSession};
        use geosolve_sketch_intent::{
            GeometryRecipeKind, IntentKey, IntentLiteral, IntentNodeDraft, IntentNodeKind,
            IntentPatch, IntentPatchOperation, IntentPatchPolicy, IntentPortRole,
            IntentPortSelector, IntentUnit, LeafField,
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

        #[wasm_bindgen_test]
        fn actual_wasm_handle_matches_dom_free_rust_session_for_transition_matrix() {
            let session = "00000000000000000000000083005001";
            let document = "00000000000000000000830050010000";
            let mut handle = IntentRpcHandle::new(session, document, 1.0).unwrap();
            let mut rust =
                crate::intent_rpc::empty_session_from_raw(0x8300_5001, 0x8300_5001_0000, 1.0)
                    .unwrap();
            let patch = serde_json::to_string(&point_patch(&rust)).unwrap();
            for request in [
                r#"{"method":"snapshot"}"#.to_owned(),
                patch,
                r#"{"method":"undo"}"#.to_owned(),
                r#"{"method":"redo"}"#.to_owned(),
                r#"{"method":"inspector","node":"0000000000000063"}"#.to_owned(),
                r#"{"method":"execute_typescript","source":"solve()"}"#.to_owned(),
            ] {
                assert_eq!(
                    handle.apply(&request),
                    rust.apply_json(&request),
                    "{request}"
                );
            }
        }
    }
}
