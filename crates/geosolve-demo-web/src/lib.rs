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

    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| JsValue::from_str("browser document is unavailable"))?;
        crate::workbench::wasm::install(&document)
    }
}
