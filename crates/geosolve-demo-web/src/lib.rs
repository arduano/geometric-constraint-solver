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

    /// Instance-scoped, DOM-free workbench authority for presentation hosts.
    #[wasm_bindgen]
    pub struct WorkbenchHandle {
        bridge: crate::workbench::bridge::WorkbenchBridge,
    }

    impl std::fmt::Debug for WorkbenchHandle {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("WorkbenchHandle")
                .finish_non_exhaustive()
        }
    }

    #[wasm_bindgen]
    impl WorkbenchHandle {
        /// Constructs a fresh or atomically restored versioned workbench.
        #[wasm_bindgen(constructor)]
        pub fn new(request: &str) -> Result<WorkbenchHandle, JsValue> {
            crate::workbench::bridge::WorkbenchBridge::construct_json(request)
                .map(|bridge| Self { bridge })
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn snapshot(&mut self) -> Result<String, JsValue> {
            self.bridge
                .snapshot_json()
                .map_err(|error| JsValue::from_str(&error))
        }

        /// Returns the immutable CAD command/icon catalog for this host.
        #[wasm_bindgen(js_name = toolCatalog)]
        pub fn tool_catalog(&self) -> Result<String, JsValue> {
            crate::workbench::bridge::WorkbenchBridge::tool_catalog_json()
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn dispatch(&mut self, request: &str) -> Result<String, JsValue> {
            self.bridge
                .dispatch_json(request)
                .map_err(|error| JsValue::from_str(&error))
        }

        /// On-demand managed compiler context. This intentionally stays out
        /// of ordinary workbench snapshots and pointer frames.
        #[wasm_bindgen(js_name = managedCompilerContext)]
        pub fn managed_compiler_context(&self) -> Result<String, JsValue> {
            self.bridge
                .managed_compiler_context_json()
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn pointer(&mut self, request: &str) -> Result<String, JsValue> {
            self.bridge
                .pointer_json(request)
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn wheel(&mut self, request: &str) -> Result<String, JsValue> {
            self.bridge
                .wheel_json(request)
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn resize(&mut self, request: &str) -> Result<String, JsValue> {
            self.bridge
                .resize_json(request)
                .map_err(|error| JsValue::from_str(&error))
        }

        pub fn cancel(&mut self, request: &str) -> Result<String, JsValue> {
            self.bridge
                .cancel_json(request)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = exportProject)]
        pub fn export_project(&self) -> Result<String, JsValue> {
            self.bridge
                .export_project_json()
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = persistProject)]
        pub fn persist_project(&self) -> Result<String, JsValue> {
            self.bridge
                .persistence_json()
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = exportReproduction)]
        pub fn export_reproduction(&self) -> Result<String, JsValue> {
            self.bridge
                .reproduction_json()
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = exportInteractionTrace)]
        pub fn export_interaction_trace(&self) -> Result<String, JsValue> {
            self.bridge
                .interaction_trace_json()
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = intentRpc)]
        pub fn intent_rpc(&mut self, request: &str) -> String {
            self.bridge.apply_intent_rpc_json(request)
        }

        #[wasm_bindgen(js_name = codeControlRpc)]
        pub fn code_control_rpc(&mut self, request: &str) -> String {
            self.bridge.apply_code_control_rpc_json(request)
        }
    }

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
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use geosolve_constraint_editor::{
            IntentRpcOutcome, IntentRpcRequest, IntentRpcSession, IntentRpcSuccess,
            MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES, MAX_INTENT_RPC_REQUEST_BYTES,
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
                let wasm_response = handle.apply(&request);
                assert_eq!(wasm_response, rust.apply_json(&request), "{request}");
                if request.contains("\"method\":\"apply_patch\"") {
                    assert!(!wasm_response.contains("\"snapshot\":"));
                    assert!(wasm_response.len() < MAX_INTENT_RPC_MUTATION_RECEIPT_BYTES);
                }
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
                    value: IntentRpcSuccess::Patch { ref receipt }
                } if receipt.disposition == IntentPlanDisposition::RetainedFailed
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

        #[wasm_bindgen_test]
        #[allow(
            clippy::too_many_lines,
            reason = "one exhaustive adapter-boundary matrix keeps each production-frame parity invariant adjacent"
        )]
        fn actual_wasm_all_thirty_seven_samples_match_independently_composed_production_frames() {
            const PRODUCTION_CHORD_TOLERANCE_PIXELS: f64 = 0.25;

            fn normalize_allocator_ids(frame: &str) -> String {
                let mut aliases = std::collections::BTreeMap::<&str, usize>::new();
                let mut normalized = String::with_capacity(frame.len());
                let bytes = frame.as_bytes();
                let mut cursor = 0;
                while cursor < bytes.len() {
                    if bytes[cursor].is_ascii_hexdigit() {
                        let start = cursor;
                        while cursor < bytes.len() && bytes[cursor].is_ascii_hexdigit() {
                            cursor += 1;
                        }
                        let token = &frame[start..cursor];
                        if token.len() == 32 {
                            let next = aliases.len();
                            let alias = *aliases.entry(token).or_insert(next);
                            normalized.push_str("{backend-id-");
                            normalized.push_str(&alias.to_string());
                            normalized.push('}');
                        } else {
                            normalized.push_str(token);
                        }
                    } else {
                        let start = cursor;
                        cursor += 1;
                        while cursor < bytes.len() && !bytes[cursor].is_ascii_hexdigit() {
                            cursor += 1;
                        }
                        normalized.push_str(&frame[start..cursor]);
                    }
                }
                normalized
            }

            let projects = geosolve_sketch_code::bundled_code_projects();
            assert_eq!(projects.len(), 37);
            for project in projects {
                let key = project.key();
                let mut handle = super::WorkbenchHandle::new(r#"{"version":1}"#)
                    .unwrap_or_else(|error| panic!("{key} WASM workbench: {error:?}"));
                let open = serde_json::json!({
                    "version": 1,
                    "command": "sample.open",
                    "payload": { "key": key },
                })
                .to_string();
                handle
                    .dispatch(&open)
                    .unwrap_or_else(|error| panic!("{key} WASM sample open: {error:?}"));
                let snapshot: serde_json::Value = serde_json::from_str(
                    &handle
                        .snapshot()
                        .unwrap_or_else(|error| panic!("{key} WASM snapshot: {error:?}")),
                )
                .unwrap_or_else(|error| panic!("{key} snapshot JSON: {error}"));
                let actual_frame = snapshot["frame"]["svg"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{key} WASM frame SVG"))
                    .to_owned();
                let actual_source = snapshot["source"]["files"]
                    .as_array()
                    .and_then(|files| files.iter().find(|file| file["path"] == "sketch.ts"))
                    .and_then(|file| file["contents"].as_str())
                    .unwrap_or_else(|| panic!("{key} WASM managed source"))
                    .to_owned();
                drop(handle);

                // Compose the expected scene directly through the accepted
                // projectional owner and target-neutral renderer. This bypasses
                // WorkbenchBridge, its frame cache, snapshot serialization and
                // the exported WASM wrapper, so the equality below detects any
                // adapter loss of native geometry, computed output, annotations,
                // accepted authority stamps or renderer bytes.
                let (code_project, editor) =
                    crate::workbench::code_projects::CodeProjectWorkbench::open_key(key)
                        .unwrap_or_else(|error| panic!("{key} direct code project: {error}"));
                let mut camera = geosolve_sketch_render::CanvasCamera::default();
                let initial_scene = editor
                    .scene(camera.viewport(), PRODUCTION_CHORD_TOLERANCE_PIXELS)
                    .unwrap_or_else(|error| panic!("{key} direct initial scene: {error}"));
                camera.fit_scene_or_reset(Some(&initial_scene));
                let viewport = camera.viewport();
                let mut scene = editor
                    .scene(viewport, PRODUCTION_CHORD_TOLERANCE_PIXELS)
                    .unwrap_or_else(|error| panic!("{key} direct fitted scene: {error}"));
                scene.set_annotations_visible(true);
                scene.set_show_all_constraint_annotations(false);
                let accepted = editor
                    .presentation_session()
                    .and_then(
                        geosolve_sketch::RetainedSketchDocumentSession::accepted_state_for_current_input,
                    )
                    .unwrap_or_else(|| panic!("{key} direct accepted authority"));
                let markup = geosolve_sketch_render::svg_markup_with_computed_context_action_stamp_and_display(
                    Some(&scene),
                    Some(accepted),
                    &[],
                    editor.editor().selection(),
                    &[],
                    editor.editor().hover_state(),
                    None,
                    editor.editor().draft_inference_resolution(),
                    None,
                    None,
                    None,
                    editor.editor().geometry_interaction_policy(),
                    geosolve_sketch_render::CanvasDisplayOptions {
                        grid_visible: true,
                        retain_contextual_annotations: true,
                    },
                    viewport,
                );
                let aria_label = format!("{} accepted sketch viewport", code_project.title());
                let expected_frame = geosolve_sketch_render::interactive_scene_svg(
                    &markup,
                    viewport.screen_size,
                    &aria_label,
                )
                .unwrap_or_else(|| panic!("{key} direct SVG frame"));

                assert_eq!(
                    normalize_allocator_ids(&actual_frame),
                    normalize_allocator_ids(&expected_frame),
                    "{key} actual-WASM frame and independently composed owner frame",
                );
                assert_eq!(
                    actual_source,
                    code_project.managed_draft(),
                    "{key} actual-WASM managed-source projection",
                );
                assert_eq!(snapshot["project"]["sampleKey"], key, "{key}");
                assert_eq!(snapshot["project"]["status"], "accepted", "{key}");
                assert!(
                    snapshot["problems"].as_array().is_some_and(Vec::is_empty),
                    "{key} has no hidden production-adapter problem",
                );
                assert!(
                    actual_frame.contains("wb-accepted-scene"),
                    "{key} accepted frame"
                );
            }
        }
    }
}
