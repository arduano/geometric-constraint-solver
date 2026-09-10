// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use geosolve_sketch_engine::{AcceptedEvaluation, EditableSession, SketchEngine as NativeEngine};
use serde::Deserialize;
use std::collections::BTreeMap;

mod authoring;
mod point_gesture;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenEditableRequest {
    project: String,
    design: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EditableRequest {
    session: u64,
    expected: geosolve_sketch_code::CodeSessionIdentity,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    overlay: Option<geosolve_sketch_code::CodeInteractionOverlay>,
}

/// Dedicated equation-free adapter. Native builds exercise the same string wire
/// methods; wasm-bindgen only translates errors and owns JavaScript handles.
#[derive(Debug, Default)]
pub struct EngineAdapter {
    engine: NativeEngine,
    retained: BTreeMap<String, AcceptedEvaluation>,
    sessions: BTreeMap<u64, EditableSession>,
    authoring: BTreeMap<String, authoring::HeldAuthoring>,
    point_gestures: BTreeMap<String, point_gesture::HeldGesture>,
    point_commits: BTreeMap<String, point_gesture::HeldPointCommit>,
    point_sequence: u64,
    accepted: Option<AcceptedEvaluation>,
}

impl EngineAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    /// # Errors
    /// Rejects invalid managed projects/designs or exhausted session/result capacity.
    pub fn open_editable_session(&mut self, json: &str) -> Result<String, String> {
        self.reserve()?;
        if self.sessions.len() >= 8 {
            return Err("close an editable session before opening more than eight".into());
        }
        let request: OpenEditableRequest = decode_session_request(json)?;
        let session = EditableSession::open(&request.project, request.design.as_deref())
            .map_err(|error| error.to_string())?;
        let state = serde_json::to_string(&session.state()).map_err(|error| error.to_string())?;
        let accepted = session.accepted().clone();
        self.sessions.insert(session.token().session, session);
        self.accepted = Some(accepted.clone());
        self.retained
            .insert(accepted.result().result_id.clone(), accepted);
        Ok(state)
    }

    /// # Errors
    /// Rejects closed/foreign session IDs or an encoding error.
    pub fn editable_session_state(&self, session_id: &str) -> Result<String, String> {
        let id: u64 = session_id.parse().map_err(|_| "invalid session ID")?;
        serde_json::to_string(
            &self
                .sessions
                .get(&id)
                .ok_or("unknown or closed session ID")?
                .state(),
        )
        .map_err(|error| error.to_string())
    }

    /// # Errors
    /// Rejects stale tokens, foreign projects, invalid updates or exhausted result capacity.
    pub fn apply_editable_project(&mut self, json: &str) -> Result<String, String> {
        self.edit_session(json, |session, request| {
            if request.overlay.is_some() {
                return Err("project update cannot carry overlay".into());
            }
            session
                .apply_project(
                    &request.expected,
                    request
                        .project
                        .as_deref()
                        .ok_or("project update requires project")?,
                )
                .map_err(|error| error.to_string())
        })
    }

    /// # Errors
    /// Rejects stale tokens, invalid semantic overrides or exhausted result capacity.
    pub fn apply_editable_overlay(&mut self, json: &str) -> Result<String, String> {
        self.edit_session(json, |session, request| {
            if request.project.is_some() {
                return Err("overlay update cannot carry project".into());
            }
            session
                .apply_overlay(
                    &request.expected,
                    request.overlay.ok_or("overlay update requires overlay")?,
                )
                .map_err(|error| error.to_string())
        })
    }

    /// # Errors
    /// Rejects stale/foreign tokens or exhausted result capacity.
    pub fn undo_editable(&mut self, json: &str) -> Result<String, String> {
        self.step_editable(json, true)
    }

    /// # Errors
    /// Rejects stale/foreign tokens or exhausted result capacity.
    pub fn redo_editable(&mut self, json: &str) -> Result<String, String> {
        self.step_editable(json, false)
    }

    /// # Errors
    /// Rejects closed/foreign session IDs or an encoding error.
    pub fn export_editable_design(&self, session_id: &str) -> Result<String, String> {
        let id: u64 = session_id.parse().map_err(|_| "invalid session ID")?;
        serde_json::to_string(
            &self
                .sessions
                .get(&id)
                .ok_or("unknown or closed session ID")?
                .design(),
        )
        .map_err(|error| error.to_string())
    }

    pub fn close_editable_session(&mut self, session_id: &str) -> bool {
        session_id.parse::<u64>().ok().is_some_and(|id| {
            self.authoring.retain(|_, prepared| prepared.session != id);
            self.point_gestures.retain(|_, held| held.session != id);
            self.point_commits.retain(|_, held| held.session != id);
            self.sessions.remove(&id).is_some()
        })
    }

    fn step_editable(&mut self, json: &str, undo: bool) -> Result<String, String> {
        self.edit_session(json, |session, request| {
            if request.project.is_some() || request.overlay.is_some() {
                return Err("history step only accepts expected token".into());
            }
            if undo {
                session.undo(&request.expected)
            } else {
                session.redo(&request.expected)
            }
            .map_err(|error| error.to_string())
        })
    }

    fn edit_session(
        &mut self,
        json: &str,
        action: impl FnOnce(&mut EditableSession, EditableRequest) -> Result<AcceptedEvaluation, String>,
    ) -> Result<String, String> {
        self.reserve()?;
        let request: EditableRequest = decode_session_request(json)?;
        let session = self
            .sessions
            .get_mut(&request.session)
            .ok_or("unknown or closed session ID")?;
        let accepted = action(session, request)?;
        let state = serde_json::to_string(&session.state()).map_err(|error| error.to_string())?;
        self.accepted = Some(accepted.clone());
        self.retained
            .insert(accepted.result().result_id.clone(), accepted);
        Ok(state)
    }

    /// # Errors
    /// Rejects untrusted compiler/source/artifact authority before native evaluation.
    pub fn compile_project_json(&self, json: &str) -> Result<String, String> {
        geosolve_sketch_engine::compile_project_json(json).map_err(|error| error.to_string())
    }

    /// # Errors
    /// Rejects malformed or unaccepted generator data, or a full retained-result table.
    pub fn evaluate_generated(&mut self, json: &str) -> Result<String, String> {
        self.reserve()?;
        let result = self
            .engine
            .evaluate_generated_json(json)
            .map_err(|error| error.to_string())?;
        self.retain(result)
    }

    /// # Errors
    /// Rejects unaccepted managed projects or a full retained-result table.
    pub fn evaluate_managed(&mut self, json: &str) -> Result<String, String> {
        self.reserve()?;
        let result = self
            .engine
            .evaluate_managed_json(json)
            .map_err(|error| error.to_string())?;
        self.retain(result)
    }

    /// # Errors
    /// Returns an encoding error rather than publishing a partial snapshot.
    pub fn last_accepted(&self) -> Result<Option<String>, String> {
        self.accepted
            .as_ref()
            .map(|result| serde_json::to_string(result.result()).map_err(|error| error.to_string()))
            .transpose()
    }

    /// # Errors
    /// Rejects released/foreign result IDs, invalid tolerance or incomplete topology.
    pub fn export_profiles(&self, result_id: &str, chord_error: f64) -> Result<String, String> {
        let result = self
            .retained
            .get(result_id)
            .ok_or("unknown or released result ID")?;
        let profile = result
            .export_profiles(chord_error)
            .map_err(|error| error.to_string())?;
        serde_json::to_string(&profile).map_err(|error| error.to_string())
    }

    /// # Errors
    /// Rejects released/foreign result IDs, unknown output names or incomplete topology.
    pub fn export_profiles_for_output(
        &self,
        result_id: &str,
        chord_error: f64,
        output: &str,
    ) -> Result<String, String> {
        let result = self
            .retained
            .get(result_id)
            .ok_or("unknown or released result ID")?;
        let profile = result
            .export_profiles_for_output(chord_error, output)
            .map_err(|error| error.to_string())?;
        serde_json::to_string(&profile).map_err(|error| error.to_string())
    }

    pub fn release_result(&mut self, result_id: &str) -> bool {
        self.retained.remove(result_id).is_some()
    }

    fn reserve(&self) -> Result<(), String> {
        if self.retained.len() >= 64 {
            return Err(
                "release an accepted result before evaluating more than 64 retained results".into(),
            );
        }
        Ok(())
    }

    fn retain(&mut self, result: AcceptedEvaluation) -> Result<String, String> {
        let json = serde_json::to_string(result.result()).map_err(|error| error.to_string())?;
        self.accepted = Some(result.clone());
        self.retained
            .insert(result.result().result_id.clone(), result);
        Ok(json)
    }
}

fn decode_session_request<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, String> {
    if json.len() > geosolve_sketch_code::CODE_PROJECT_LIMIT {
        return Err("session request byte limit exceeded".into());
    }
    serde_json::from_str(json).map_err(|error| error.to_string())
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::EngineAdapter;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(js_name = SketchEngine)]
    #[derive(Debug)]
    pub struct WasmSketchEngine(EngineAdapter);

    #[wasm_bindgen(js_class = SketchEngine)]
    impl WasmSketchEngine {
        #[wasm_bindgen(constructor)]
        pub fn new() -> Self {
            console_error_panic_hook::set_once();
            Self(EngineAdapter::new())
        }

        #[wasm_bindgen(js_name = editablePointGestureTargets)]
        pub fn editable_point_gesture_targets(&self, json: &str) -> Result<String, JsValue> {
            self.0
                .editable_point_gesture_targets(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = beginEditablePointGesture)]
        pub fn begin_editable_point_gesture(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .begin_editable_point_gesture(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = advanceEditablePointGesture)]
        pub fn advance_editable_point_gesture(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .advance_editable_point_gesture(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = editablePointGestureScene)]
        pub fn editable_point_gesture_scene(&self, json: &str) -> Result<String, JsValue> {
            self.0
                .editable_point_gesture_scene(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = finishEditablePointGesture)]
        pub fn finish_editable_point_gesture(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .finish_editable_point_gesture(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = cancelEditablePointGesture)]
        pub fn cancel_editable_point_gesture(&mut self, json: &str) -> Result<(), JsValue> {
            self.0
                .cancel_editable_point_gesture(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = prepareEditablePointCommit)]
        pub fn prepare_editable_point_commit(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .prepare_editable_point_commit(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = applyEditablePointCommit)]
        pub fn apply_editable_point_commit(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .apply_editable_point_commit(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = releaseEditablePointCommit)]
        pub fn release_editable_point_commit(&mut self, json: &str) -> Result<(), JsValue> {
            self.0
                .release_editable_point_commit(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = prepareEditableAuthoring)]
        pub fn prepare_editable_authoring(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .prepare_editable_authoring(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = applyEditableAuthoring)]
        pub fn apply_editable_authoring(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .apply_editable_authoring(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = releaseEditableAuthoring)]
        pub fn release_editable_authoring(&mut self, ticket: &str) -> bool {
            self.0.release_editable_authoring(ticket)
        }

        #[wasm_bindgen(js_name = exportEditableProject)]
        pub fn export_editable_project(&self, id: &str) -> Result<String, JsValue> {
            self.0
                .export_editable_project(id)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = editableSourceDesignDigest)]
        pub fn editable_source_design_digest(&self, id: &str) -> Result<String, JsValue> {
            self.0
                .editable_source_design_digest(id)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = openEditableSession)]
        pub fn open_editable_session(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .open_editable_session(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = editableSessionState)]
        pub fn editable_session_state(&self, id: &str) -> Result<String, JsValue> {
            self.0
                .editable_session_state(id)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = applyEditableProject)]
        pub fn apply_editable_project(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .apply_editable_project(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = applyEditableOverlay)]
        pub fn apply_editable_overlay(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .apply_editable_overlay(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = undoEditable)]
        pub fn undo_editable(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .undo_editable(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = redoEditable)]
        pub fn redo_editable(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .redo_editable(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = exportEditableDesign)]
        pub fn export_editable_design(&self, id: &str) -> Result<String, JsValue> {
            self.0
                .export_editable_design(id)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = closeEditableSession)]
        pub fn close_editable_session(&mut self, id: &str) -> bool {
            self.0.close_editable_session(id)
        }

        #[wasm_bindgen(js_name = compileProjectJson)]
        pub fn compile_project_json(&self, json: &str) -> Result<String, JsValue> {
            self.0
                .compile_project_json(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = evaluateGenerated)]
        pub fn evaluate_generated(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .evaluate_generated(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = evaluateManaged)]
        pub fn evaluate_managed(&mut self, json: &str) -> Result<String, JsValue> {
            self.0
                .evaluate_managed(json)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = lastAccepted)]
        pub fn last_accepted(&self) -> Result<Option<String>, JsValue> {
            self.0
                .last_accepted()
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = exportProfiles)]
        pub fn export_profiles(
            &self,
            result_id: &str,
            chord_error: f64,
        ) -> Result<String, JsValue> {
            self.0
                .export_profiles(result_id, chord_error)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = exportProfilesForOutput)]
        pub fn export_profiles_for_output(
            &self,
            result_id: &str,
            chord_error: f64,
            output: &str,
        ) -> Result<String, JsValue> {
            self.0
                .export_profiles_for_output(result_id, chord_error, output)
                .map_err(|error| JsValue::from_str(&error))
        }

        #[wasm_bindgen(js_name = releaseResult)]
        pub fn release_result(&mut self, result_id: &str) -> bool {
            self.0.release_result(result_id)
        }
    }
}
