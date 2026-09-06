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

        /// Strict bounded recovery for historical v4 browser saves (M92-F013).
        #[wasm_bindgen(js_name = restoreLegacyCodeWorkbench)]
        pub fn restore_legacy_code_workbench(persisted: &str) -> Result<WorkbenchHandle, JsValue> {
            crate::workbench::bridge::WorkbenchBridge::restore_legacy_code_workbench(persisted)
                .map(|bridge| WorkbenchHandle { bridge })
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
        use geosolve_sketch_code::{
            CompiledManagedSource, ManagedPathSegment, ManagedSketchMutation, ManagedValue,
            PreparedManagedMutationReceipt, UnitLiteral,
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
        fn actual_wasm_all_bundled_samples_match_independently_composed_production_frames() {
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

            let samples = geosolve_sketch_code::bundled_sample_catalog();
            let reviewed: serde_json::Value = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-code/assets/bundled-sample-catalog.json"
            )))
            .expect("reviewed sample catalog contract");
            assert_eq!(reviewed["schema"], 1);
            assert_eq!(
                serde_json::json!(
                    samples
                        .iter()
                        .map(|sample| serde_json::json!({
                            "key": sample.key, "title": sample.title, "category": sample.category,
                        }))
                        .collect::<Vec<_>>()
                ),
                reviewed["samples"],
                "runtime catalog must match independent reviewed order and metadata"
            );
            for sample in samples {
                let key = sample.key;
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

        #[wasm_bindgen_test]
        #[allow(
            clippy::too_many_lines,
            reason = "one focused adapter oracle keeps scale selection, prepared compilation, validation and exact history adjacent"
        )]
        fn actual_wasm_scale_samples_select_edit_validate_and_retain_history() {
            struct ScaleEditCase {
                key: &'static str,
                declaration: &'static str,
                path: &'static [&'static str],
                original: f64,
                replacement: f64,
                compiled_fixture: &'static [u8],
            }

            const CASES: &[ScaleEditCase] = &[
                ScaleEditCase {
                    key: "perforated-fixture-field",
                    declaration: "northernCells",
                    path: &["pilotRadius"],
                    original: 2.5,
                    replacement: 2.7,
                    compiled_fixture: include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/tests/fixtures/m92-perforated-fixture-field-edit.compiled.json.zlib"
                    )),
                },
                ScaleEditCase {
                    key: "robotic-harness-backplane",
                    declaration: "bendRadius",
                    path: &[],
                    original: 5.0,
                    replacement: 4.5,
                    compiled_fixture: include_bytes!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/tests/fixtures/m92-robotic-harness-backplane-edit.compiled.json.zlib"
                    )),
                },
            ];

            fn parse_snapshot(encoded: &str, context: &str) -> serde_json::Value {
                let snapshot = serde_json::from_str(encoded)
                    .unwrap_or_else(|error| panic!("{context} snapshot JSON: {error}"));
                assert_no_presentation_shortcut_keys(&snapshot, context);
                snapshot
            }

            fn is_presentation_shortcut_key(field: &str) -> bool {
                let normalized = field
                    .chars()
                    .filter(|character| *character != '_' && *character != '-')
                    .flat_map(char::to_lowercase)
                    .collect::<String>();
                let has_millisecond_suffix = normalized.ends_with("ms");
                let mut metric = normalized.strip_suffix("ms").unwrap_or(&normalized);
                while let Some(remainder) = [
                    "current",
                    "frame",
                    "render",
                    "solve",
                    "compile",
                    "load",
                    "open",
                    "edit",
                    "pointer",
                    "interaction",
                    "mutation",
                    "history",
                    "viewport",
                    "scene",
                    "total",
                ]
                .iter()
                .find_map(|prefix| metric.strip_prefix(prefix).filter(|rest| !rest.is_empty()))
                {
                    metric = remainder;
                }
                (has_millisecond_suffix
                    && matches!(
                        metric,
                        "frame"
                            | "render"
                            | "solve"
                            | "compile"
                            | "load"
                            | "open"
                            | "edit"
                            | "pointer"
                            | "interaction"
                            | "mutation"
                            | "history"
                            | "total"
                    ))
                    || matches!(
                        metric,
                        "lod"
                            | "lodenabled"
                            | "lodlevel"
                            | "lodmode"
                            | "lodpolicy"
                            | "lodtier"
                            | "timing"
                            | "timings"
                            | "elapsed"
                            | "duration"
                            | "wallclock"
                    )
            }

            fn first_presentation_shortcut_key(value: &serde_json::Value) -> Option<&str> {
                match value {
                    serde_json::Value::Object(fields) => {
                        fields.iter().find_map(|(field, child)| {
                            is_presentation_shortcut_key(field)
                                .then_some(field.as_str())
                                .or_else(|| first_presentation_shortcut_key(child))
                        })
                    }
                    serde_json::Value::Array(values) => {
                        values.iter().find_map(first_presentation_shortcut_key)
                    }
                    _ => None,
                }
            }

            fn assert_no_presentation_shortcut_keys(value: &serde_json::Value, context: &str) {
                let shortcut = first_presentation_shortcut_key(value);
                assert!(
                    shortcut.is_none(),
                    "{context} semantic output contains forbidden presentation field `{}`",
                    shortcut.unwrap_or_default()
                );
            }

            fn managed_source<'a>(snapshot: &'a serde_json::Value, key: &str) -> &'a str {
                snapshot["source"]["files"]
                    .as_array()
                    .and_then(|files| files.iter().find(|file| file["path"] == "sketch.ts"))
                    .and_then(|file| file["contents"].as_str())
                    .unwrap_or_else(|| panic!("{key} managed source"))
            }

            fn frame_svg<'a>(snapshot: &'a serde_json::Value, key: &str) -> &'a str {
                snapshot["frame"]["svg"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{key} frame SVG"))
            }

            fn svg_attribute<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
                let marker = format!("{name}=\"");
                let value = tag.get(tag.find(&marker)? + marker.len()..)?;
                value.get(..value.find('"')?)
            }

            fn first_interactive_point(svg: &str, key: &str) -> ([f64; 2], String) {
                svg.split("<circle")
                    .skip(1)
                    .find_map(|suffix| {
                        let tag = suffix.get(..suffix.find("/>")?)?;
                        if !tag.contains("class=\"wb-point")
                            || !tag.contains("data-interactive=\"true\"")
                        {
                            return None;
                        }
                        Some((
                            [
                                svg_attribute(tag, "cx")?.parse::<f64>().ok()?,
                                svg_attribute(tag, "cy")?.parse::<f64>().ok()?,
                            ],
                            svg_attribute(tag, "data-persistent-id")?.to_owned(),
                        ))
                    })
                    .unwrap_or_else(|| panic!("{key} has no interactive SVG point"))
            }

            fn persistent_point_tag<'a>(svg: &'a str, persistent_id: &str) -> Option<&'a str> {
                svg.split("<circle").skip(1).find_map(|suffix| {
                    let tag = suffix.get(..suffix.find("/>")?)?;
                    (svg_attribute(tag, "data-persistent-id") == Some(persistent_id)).then_some(tag)
                })
            }

            fn click(
                handle: &mut super::WorkbenchHandle,
                pointer_id: u64,
                position: [f64; 2],
                control: bool,
                key: &str,
            ) -> (serde_json::Value, serde_json::Value) {
                let request = |phase: &str, buttons: u16| {
                    serde_json::json!({
                        "version": 1,
                        "phase": phase,
                        "pointerId": pointer_id,
                        "x": position[0],
                        "y": position[1],
                        "buttons": buttons,
                        "modifiers": {
                            "alt": false,
                            "ctrl": control,
                            "meta": false,
                            "shift": false,
                        },
                    })
                    .to_string()
                };
                let down = parse_snapshot(
                    &handle
                        .pointer(&request("down", 1))
                        .unwrap_or_else(|error| panic!("{key} pointer down: {error:?}")),
                    key,
                );
                let up = parse_snapshot(
                    &handle
                        .pointer(&request("up", 0))
                        .unwrap_or_else(|error| panic!("{key} pointer up: {error:?}")),
                    key,
                );
                (down, up)
            }

            fn inspect_controls(
                handle: &mut super::WorkbenchHandle,
                key: &str,
            ) -> serde_json::Value {
                let response = handle.code_control_rpc(r#"{"method":"inspect_managed_controls"}"#);
                let response: serde_json::Value = serde_json::from_str(&response)
                    .unwrap_or_else(|error| panic!("{key} control response JSON: {error}"));
                assert_no_presentation_shortcut_keys(&response, key);
                assert_eq!(response["outcome"], "success", "{key} controls");
                assert_eq!(
                    response["value"]["result"], "managed_controls",
                    "{key} controls"
                );
                response["value"]["snapshot"].clone()
            }

            fn code_revision(snapshot: &serde_json::Value, key: &str) -> u64 {
                snapshot["identity"]["revision"]
                    .as_u64()
                    .unwrap_or_else(|| panic!("{key} code revision"))
            }

            #[derive(serde::Serialize)]
            struct ResolveMutationCommand<'a> {
                version: u8,
                command: &'static str,
                payload: &'a PreparedManagedMutationReceipt,
            }

            fn assert_complete_frame(snapshot: &serde_json::Value, key: &str) {
                let svg = frame_svg(snapshot, key);
                assert!(svg.starts_with("<svg"), "{key} SVG root");
                assert!(svg.ends_with("</svg>"), "{key} complete SVG");
                assert!(svg.contains("wb-accepted-scene"), "{key} accepted frame");
                assert!(svg.contains("wb-geometry"), "{key} complete geometry frame");
                assert!(!svg.contains("NaN"), "{key} frame has no NaN");
                assert!(!svg.contains("Infinity"), "{key} frame has no infinity");
            }

            let forbidden_key_fixture = serde_json::json!({
                "outer": [{ "nested": { "renderDurationMs": 1 } }],
            });
            assert_eq!(
                first_presentation_shortcut_key(&forbidden_key_fixture),
                Some("renderDurationMs")
            );
            for field in [
                "lodEnabled",
                "viewport_lod_level",
                "timingMs",
                "frameTiming",
                "elapsed_ms",
                "renderDurationMs",
                "wallClock",
                "wall_clock_ms",
                "renderMs",
            ] {
                assert!(is_presentation_shortcut_key(field), "forbid `{field}`");
            }
            for field in [
                "pilotRadius",
                "timingBeltPitch",
                "durationAngle",
                "wallClockwiseBranch",
            ] {
                assert!(!is_presentation_shortcut_key(field), "allow `{field}`");
            }

            fn assert_accepted_invariants(
                handle: &super::WorkbenchHandle,
                key: &str,
                expected_raw_dof: usize,
                expected_effective_dof: usize,
            ) {
                let accepted = handle
                    .bridge
                    .accepted_editor_for_adapter_test()
                    .coordinator()
                    .accepted_materialization()
                    .unwrap_or_else(|| panic!("{key} accepted materialization"));
                assert!(
                    accepted.validation.hard_residuals_validated,
                    "{key} independently validated hard residuals"
                );
                assert!(
                    accepted.validation.all_active_features_current,
                    "{key} current computed features"
                );
                assert!(
                    accepted
                        .validation
                        .maximum_normalized_hard_residual
                        .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9),
                    "{key} normalized hard residual"
                );
                let state = accepted
                    .session
                    .accepted_state_for_current_input()
                    .unwrap_or_else(|| panic!("{key} current accepted state"));
                let diagnostics = state.diagnostics();
                assert_eq!(
                    diagnostics
                        .rank
                        .and_then(|rank| rank.numerical_right_nullity),
                    Some(expected_raw_dof),
                    "{key} numerical right-nullity"
                );
                let mobility = diagnostics
                    .mobility
                    .unwrap_or_else(|| panic!("{key} mobility diagnostics"));
                assert_eq!(
                    mobility.equality_degrees_of_freedom,
                    Some(expected_raw_dof),
                    "{key} equality DOF"
                );
                assert_eq!(
                    mobility.bidirectional_bounded_degrees_of_freedom,
                    Some(expected_effective_dof),
                    "{key} bidirectional bounded DOF"
                );
                assert!(
                    state
                        .document()
                        .points()
                        .iter()
                        .flat_map(|point| point.position)
                        .chain(state.document().scalars().iter().map(|scalar| scalar.value))
                        .all(f64::is_finite),
                    "{key} finite accepted points and scalars"
                );
            }

            for (index, case) in CASES.iter().enumerate() {
                let sample = geosolve_sketch_code::bundled_sample(case.key)
                    .unwrap_or_else(|| panic!("{} canonical sample", case.key));
                let mut handle = super::WorkbenchHandle::new(r#"{"version":1}"#)
                    .unwrap_or_else(|error| panic!("{} WASM workbench: {error:?}", case.key));
                let opened = parse_snapshot(
                    &handle
                        .dispatch(
                            &serde_json::json!({
                                "version": 1,
                                "command": "sample.open",
                                "payload": { "key": case.key },
                            })
                            .to_string(),
                        )
                        .unwrap_or_else(|error| panic!("{} sample open: {error:?}", case.key)),
                    case.key,
                );
                assert_eq!(opened["project"]["status"], "accepted", "{}", case.key);
                assert!(
                    opened["problems"].as_array().is_some_and(Vec::is_empty),
                    "{} initial Problems",
                    case.key
                );
                assert_complete_frame(&opened, case.key);
                assert_accepted_invariants(
                    &handle,
                    case.key,
                    sample.expected.numerical_right_nullity(),
                    sample.expected.bidirectional_bounded_degrees_of_freedom(),
                );
                let base_source = managed_source(&opened, case.key).to_owned();
                assert_eq!(base_source, sample.managed_source(), "{}", case.key);
                let base_frame = frame_svg(&opened, case.key).to_owned();
                let base_controls = inspect_controls(&mut handle, case.key);
                assert_eq!(base_controls["can_undo"], false, "{}", case.key);
                assert_eq!(base_controls["can_redo"], false, "{}", case.key);
                let base_identity = base_controls["identity"].clone();
                assert_eq!(code_revision(&base_controls, case.key), 0, "{}", case.key);

                let controls = base_controls["manifest"]["controls"]
                    .as_array()
                    .unwrap_or_else(|| panic!("{} managed controls", case.key));
                let expected_path = serde_json::to_value(case.path).unwrap();
                let control = controls
                    .iter()
                    .find(|control| {
                        control["source"]["declaration"] == case.declaration
                            && control["source"]["path"] == expected_path
                    })
                    .unwrap_or_else(|| {
                        panic!(
                            "{} exact managed control {}.{:?}",
                            case.key, case.declaration, case.path
                        )
                    });
                assert_eq!(control["access"]["access"], "editable", "{}", case.key);
                assert_eq!(
                    control["value"],
                    serde_json::json!({
                        "kind": "unit",
                        "value": { "unit": "mm", "value": case.original },
                    }),
                    "{} exact base control value",
                    case.key
                );
                let control_id = control["id"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{} control id", case.key))
                    .to_owned();
                drop(base_controls);

                let (point, point_id) = first_interactive_point(&base_frame, case.key);
                let (selected_down, selected) = click(
                    &mut handle,
                    u64::try_from(index).unwrap() + 92_100,
                    point,
                    false,
                    case.key,
                );
                let selected_point_tag =
                    persistent_point_tag(frame_svg(&selected_down, case.key), &point_id)
                        .unwrap_or_else(|| panic!("{} selected persistent point paint", case.key));
                assert!(
                    selected_point_tag.contains("class=\"wb-point selected"),
                    "{} pointer down must select the exact hit point {point_id}",
                    case.key
                );
                assert!(selected["selection"].is_object(), "{} selection", case.key);
                let selected_release_tag =
                    persistent_point_tag(frame_svg(&selected, case.key), &point_id)
                        .unwrap_or_else(|| panic!("{} released persistent point paint", case.key));
                assert!(selected_release_tag.contains("class=\"wb-point selected"));
                assert_eq!(
                    managed_source(&selected, case.key),
                    base_source,
                    "{}",
                    case.key
                );
                assert_eq!(
                    selected["presentation"]["canUndo"], false,
                    "{} selection leaves Undo history unchanged",
                    case.key
                );
                assert_eq!(selected["presentation"]["canRedo"], false, "{}", case.key);

                // Selection is presentation-only and deliberately absent from
                // outer code checkpoints. Clear it through the public pointer
                // adapter before freezing exact base/edited history frames.
                let (deselected_down, deselected) = click(
                    &mut handle,
                    u64::try_from(index).unwrap() + 92_200,
                    point,
                    true,
                    case.key,
                );
                assert!(deselected_down["selection"].is_null(), "{}", case.key);
                assert!(
                    deselected["selection"].is_null(),
                    "{} deselection",
                    case.key
                );
                assert_eq!(frame_svg(&deselected, case.key), base_frame, "{}", case.key);
                assert_eq!(
                    managed_source(&deselected, case.key),
                    base_source,
                    "{}",
                    case.key
                );
                let pending = parse_snapshot(
                    &handle
                        .dispatch(
                            &serde_json::json!({
                                "version": 1,
                                "command": "parameter.edit",
                                "payload": {
                                    "id": control_id,
                                    "value": case.replacement,
                                },
                            })
                            .to_string(),
                        )
                        .unwrap_or_else(|error| {
                            panic!("{} prepare parameter edit: {error:?}", case.key)
                        }),
                    case.key,
                );
                assert_eq!(pending["pendingManagedMutation"]["kind"], "managed");
                assert_eq!(
                    managed_source(&pending, case.key),
                    base_source,
                    "{}",
                    case.key
                );
                assert_eq!(frame_svg(&pending, case.key), base_frame, "{}", case.key);
                assert_eq!(pending["presentation"]["canUndo"], false, "{}", case.key);
                assert_eq!(pending["presentation"]["canRedo"], false, "{}", case.key);
                assert!(
                    pending["problems"].as_array().is_some_and(Vec::is_empty),
                    "{} pending Problems",
                    case.key
                );

                let request = &pending["pendingManagedMutation"]["request"];
                assert_eq!(
                    request["current"]["normalizedSource"], base_source,
                    "{}",
                    case.key
                );
                assert_eq!(
                    request["ticket"]["session"], base_identity,
                    "{} pending compilation retains exact code history identity",
                    case.key
                );
                let mutation: ManagedSketchMutation =
                    serde_json::from_value(request["ticket"]["mutation"].clone())
                        .unwrap_or_else(|error| panic!("{} prepared mutation: {error}", case.key));
                let ManagedSketchMutation::SetValues { values } = &mutation else {
                    panic!("{} representative edit must prepare set_values", case.key)
                };
                assert_eq!(values.len(), 1, "{}", case.key);
                assert_eq!(values[0].declaration, case.declaration, "{}", case.key);
                assert_eq!(
                    values[0].path,
                    case.path
                        .iter()
                        .map(|field| ManagedPathSegment::Field((*field).to_owned()))
                        .collect::<Vec<_>>(),
                    "{}",
                    case.key
                );
                assert_eq!(
                    values[0].expected,
                    ManagedValue::Unit(UnitLiteral {
                        unit: "mm".into(),
                        value: case.original,
                    }),
                    "{}",
                    case.key
                );
                assert_eq!(
                    values[0].value,
                    ManagedValue::Unit(UnitLiteral {
                        unit: "mm".into(),
                        value: case.replacement,
                    }),
                    "{}",
                    case.key
                );
                let ticket_digest = request["ticket"]["ticketDigest"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{} prepared ticket digest", case.key))
                    .to_owned();
                let base_source_digest = request["current"]["ir"]["source_digest"]
                    .as_str()
                    .unwrap_or_else(|| panic!("{} prepared base source digest", case.key))
                    .to_owned();
                drop(pending);

                let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(
                    case.compiled_fixture,
                    geosolve_sketch_code::MANAGED_WIRE_LIMIT,
                )
                .unwrap_or_else(|error| panic!("{} compressed fixture: {error:?}", case.key));
                let fixture = std::str::from_utf8(&decompressed)
                    .unwrap_or_else(|error| panic!("{} fixture UTF-8: {error}", case.key));
                let candidate = CompiledManagedSource::from_json(fixture)
                    .unwrap_or_else(|error| panic!("{} candidate fixture: {error}", case.key));
                assert_eq!(
                    candidate
                        .artifact
                        .groups
                        .iter()
                        .map(|group| group.name.as_str())
                        .collect::<Vec<_>>(),
                    sample.functional_groups,
                    "{} exact candidate group order",
                    case.key
                );
                let candidate_source = candidate.normalized_source.clone();
                let receipt = PreparedManagedMutationReceipt {
                    ticket_digest,
                    base_source_digest,
                    candidate_source_digest: candidate.ir.source_digest.clone(),
                    compiled: candidate,
                };
                let resolve_command = serde_json::to_string(&ResolveMutationCommand {
                    version: 1,
                    command: "managed.mutation.resolve",
                    payload: &receipt,
                })
                .unwrap_or_else(|error| panic!("{} resolve command: {error}", case.key));
                let accepted = parse_snapshot(
                    &handle.dispatch(&resolve_command).unwrap_or_else(|error| {
                        panic!("{} resolve parameter edit: {error:?}", case.key)
                    }),
                    case.key,
                );
                assert!(
                    accepted.get("pendingManagedMutation").is_none(),
                    "{}",
                    case.key
                );
                assert_eq!(accepted["project"]["status"], "accepted", "{}", case.key);
                assert!(
                    accepted["problems"].as_array().is_some_and(Vec::is_empty),
                    "{} accepted Problems",
                    case.key
                );
                assert!(accepted["selection"].is_null(), "{}", case.key);
                assert_eq!(
                    managed_source(&accepted, case.key),
                    candidate_source,
                    "{} exact candidate source publication",
                    case.key
                );
                assert_eq!(accepted["presentation"]["canUndo"], true, "{}", case.key);
                assert_eq!(accepted["presentation"]["canRedo"], false, "{}", case.key);
                assert_complete_frame(&accepted, case.key);
                let edited_frame = frame_svg(&accepted, case.key).to_owned();
                let repeated = parse_snapshot(
                    &handle
                        .snapshot()
                        .unwrap_or_else(|error| panic!("{} repeat snapshot: {error:?}", case.key)),
                    case.key,
                );
                assert_eq!(
                    repeated["frame"], accepted["frame"],
                    "{} deterministic edited frame",
                    case.key
                );
                let visible_groups = accepted["explorer"]
                    .as_array()
                    .expect("Explorer groups")
                    .iter()
                    .filter_map(|group| group["label"].as_str())
                    .filter(|label| sample.functional_groups.contains(label))
                    .collect::<Vec<_>>();
                assert_eq!(
                    visible_groups, sample.functional_groups,
                    "{} exact Explorer functional-group order",
                    case.key
                );
                assert_accepted_invariants(
                    &handle,
                    case.key,
                    sample.expected.numerical_right_nullity(),
                    sample.expected.bidirectional_bounded_degrees_of_freedom(),
                );
                let edited_controls = inspect_controls(&mut handle, case.key);
                assert_eq!(
                    edited_controls["identity"]["session"], base_identity["session"],
                    "{} code session",
                    case.key
                );
                assert_eq!(
                    code_revision(&edited_controls, case.key),
                    base_identity["revision"].as_u64().unwrap() + 1,
                    "{} exactly one accepted code revision",
                    case.key
                );
                assert_eq!(edited_controls["can_undo"], true, "{}", case.key);
                assert_eq!(edited_controls["can_redo"], false, "{}", case.key);
                drop(edited_controls);

                let undone = parse_snapshot(
                    &handle
                        .dispatch(r#"{"version":1,"command":"history.undo"}"#)
                        .unwrap_or_else(|error| panic!("{} Undo: {error:?}", case.key)),
                    case.key,
                );
                assert_eq!(
                    managed_source(&undone, case.key),
                    base_source,
                    "{}",
                    case.key
                );
                assert_eq!(frame_svg(&undone, case.key), base_frame, "{}", case.key);
                assert_eq!(undone["presentation"]["canUndo"], false, "{}", case.key);
                assert_eq!(undone["presentation"]["canRedo"], true, "{}", case.key);
                assert!(undone["problems"].as_array().is_some_and(Vec::is_empty));
                assert_accepted_invariants(
                    &handle,
                    case.key,
                    sample.expected.numerical_right_nullity(),
                    sample.expected.bidirectional_bounded_degrees_of_freedom(),
                );
                let redone = parse_snapshot(
                    &handle
                        .dispatch(r#"{"version":1,"command":"history.redo"}"#)
                        .unwrap_or_else(|error| panic!("{} Redo: {error:?}", case.key)),
                    case.key,
                );
                assert_eq!(
                    managed_source(&redone, case.key),
                    candidate_source,
                    "{} Redo source",
                    case.key
                );
                assert_eq!(frame_svg(&redone, case.key), edited_frame, "{}", case.key);
                assert_eq!(redone["presentation"]["canUndo"], true, "{}", case.key);
                assert_eq!(redone["presentation"]["canRedo"], false, "{}", case.key);
                assert!(redone["problems"].as_array().is_some_and(Vec::is_empty));
                assert_accepted_invariants(
                    &handle,
                    case.key,
                    sample.expected.numerical_right_nullity(),
                    sample.expected.bidirectional_bounded_degrees_of_freedom(),
                );
                // Exercise the real browser transport, including its nested
                // persistence string and unchanged request-size guard (M92-F013).
                let persistence: serde_json::Value = serde_json::from_str(
                    &handle
                        .persist_project()
                        .unwrap_or_else(|error| panic!("{} persistence: {error:?}", case.key)),
                )
                .unwrap();
                let mut restored = super::WorkbenchHandle::new(
                    &serde_json::json!({
                        "version": 1,
                        "persistedProject": persistence["contents"],
                    })
                    .to_string(),
                )
                .unwrap_or_else(|error| panic!("{} browser reload: {error:?}", case.key));
                let restored_snapshot = parse_snapshot(&restored.snapshot().unwrap(), case.key);
                assert_eq!(
                    managed_source(&restored_snapshot, case.key),
                    candidate_source
                );
                assert_eq!(
                    restored.export_project().unwrap(),
                    handle.export_project().unwrap()
                );
                assert_eq!(
                    restored.persist_project().unwrap(),
                    handle.persist_project().unwrap()
                );
                assert_accepted_invariants(
                    &restored,
                    case.key,
                    sample.expected.numerical_right_nullity(),
                    sample.expected.bidirectional_bounded_degrees_of_freedom(),
                );
                let restored_undo = parse_snapshot(
                    &restored
                        .dispatch(r#"{"version":1,"command":"history.undo"}"#)
                        .unwrap(),
                    case.key,
                );
                assert_eq!(managed_source(&restored_undo, case.key), base_source);
                let restored_redo = parse_snapshot(
                    &restored
                        .dispatch(r#"{"version":1,"command":"history.redo"}"#)
                        .unwrap(),
                    case.key,
                );
                assert_eq!(managed_source(&restored_redo, case.key), candidate_source);
                assert_eq!(
                    restored.export_project().unwrap(),
                    handle.export_project().unwrap()
                );
                assert_accepted_invariants(
                    &restored,
                    case.key,
                    sample.expected.numerical_right_nullity(),
                    sample.expected.bidirectional_bounded_degrees_of_freedom(),
                );
            }
        }
    }
}
