// SPDX-License-Identifier: GPL-3.0-or-later
//! Opaque retained prediction and independently replayed server preparation handles.
//! No detached scene or client acceptance claim enters the publication path.

use crate::{EngineAdapter, decode_session_request};
use geosolve_sketch_code::CodeSessionIdentity;
use geosolve_sketch_engine::{
    PointGestureCommand, PointGestureSample, PointGestureTarget, PreparedPointGestureCommit,
    RetainedPointGesture,
};
use serde::Deserialize;
use serde_json::{Value, json};

const MAX_GESTURES: usize = 8;
const MAX_COMMITS: usize = 8;
const MAX_SEMANTIC_BYTES: usize = 64 * 1024 * 1024;
const MAX_COMMAND_BYTES: usize = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BeginRequest {
    session: u64,
    expected: CodeSessionIdentity,
    target: PointGestureTarget,
    gesture_id: u64,
    viewport: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GestureRequest {
    session: u64,
    ticket: String,
    gesture_id: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AdvanceRequest {
    session: u64,
    ticket: String,
    gesture_id: u64,
    sample: PointGestureSample,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PrepareRequest {
    session: u64,
    expected: CodeSessionIdentity,
    command: PointGestureCommand,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommitRequest {
    session: u64,
    ticket: String,
}
#[derive(Debug)]
pub(super) struct HeldGesture {
    pub session: u64,
    gesture_id: u64,
    gesture: RetainedPointGesture,
    bytes: usize,
}
#[derive(Debug)]
pub(super) struct HeldPointCommit {
    pub session: u64,
    expected: CodeSessionIdentity,
    prepared: PreparedPointGestureCommit,
    bytes: usize,
}

impl EngineAdapter {
    /// # Errors
    /// Rejects absent sessions or unavailable semantic point ownership.
    pub fn editable_point_gesture_targets(&self, id: &str) -> Result<String, String> {
        let id: u64 = id.parse().map_err(|_| "invalid session ID")?;
        let targets = self
            .sessions
            .get(&id)
            .ok_or("unknown editable session")?
            .point_gesture_targets()
            .map_err(|e| e.to_string())?;
        encode(&targets)
    }

    /// Starts one retained provisional coordinator, independent of accepted navigation.
    ///
    /// # Errors
    /// Rejects stale tokens, invalid targets/camera and bounded handle exhaustion.
    pub fn begin_editable_point_gesture(&mut self, json: &str) -> Result<String, String> {
        if self.point_gestures.len() >= MAX_GESTURES {
            return Err("cancel a point gesture before opening more than eight".into());
        }
        let input: BeginRequest = decode_point_request(json)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("point gesture expected input is stale or foreign".into());
        }
        let bytes = session
            .export_project_json()
            .map_err(|e| e.to_string())?
            .len()
            .saturating_add(encode(&session.design())?.len())
            .saturating_add(MAX_COMMAND_BYTES);
        self.reserve_point_bytes(bytes)?;
        let gesture = session
            .begin_point_gesture(
                input.target,
                input.gesture_id,
                serde_json::from_value(input.viewport).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        let ticket = self.next_point_ticket(input.session, "gesture")?;
        let encoded =
            encode(&json!({"ticket":ticket,"accepted_position":gesture.accepted_position()}))?;
        self.point_gestures.insert(
            ticket,
            HeldGesture {
                session: input.session,
                gesture_id: input.gesture_id,
                gesture,
                bytes,
            },
        );
        Ok(encoded)
    }

    /// # Errors
    /// Rejects foreign/consumed gestures, malformed samples or native preview errors.
    pub fn advance_editable_point_gesture(&mut self, json: &str) -> Result<String, String> {
        let input: AdvanceRequest = decode_point_request(json)?;
        let held = self
            .point_gestures
            .get_mut(&input.ticket)
            .filter(|held| held.session == input.session && held.gesture_id == input.gesture_id)
            .ok_or("unknown, foreign or consumed point gesture")?;
        let frame = held
            .gesture
            .advance(input.gesture_id, input.sample)
            .map_err(|e| e.to_string())?;
        encode(
            &json!({"sequence":frame.sequence,"accepted":frame.accepted,"accepted_position":frame.accepted_position,
            "work":{"native_preview_attempts":frame.work.native_preview_attempts(),
                "intent_materialization_attempts":frame.work.intent_materialization_attempts(),
                "computed_evaluation_attempts":frame.work.computed_evaluation_attempts(),
                "history_publications":frame.work.history_publications()}}),
        )
    }

    /// Detached presentation only; this payload cannot be imported as accepted authority.
    ///
    /// # Errors
    /// Rejects foreign/consumed gestures and failed provisional scene serialization.
    pub fn editable_point_gesture_scene(&self, json: &str) -> Result<String, String> {
        let input: GestureRequest = decode_point_request(json)?;
        self.gesture(&input)?
            .gesture
            .scene_json()
            .map_err(|e| e.to_string())
    }

    /// Consumes the retained prediction and returns only replayable semantic input.
    ///
    /// # Errors
    /// Invalid routing preserves the handle; terminal failure consumes it without publication.
    pub fn finish_editable_point_gesture(&mut self, json: &str) -> Result<String, String> {
        let input: GestureRequest = decode_point_request(json)?;
        self.gesture(&input)?;
        let held = self
            .point_gestures
            .remove(&input.ticket)
            .ok_or("unknown point gesture")?;
        let terminal = held
            .gesture
            .finish(input.gesture_id)
            .map_err(|e| e.to_string())?;
        encode(
            &json!({"command":terminal.command(),"accepted_position":terminal.accepted_position()}),
        )
    }

    /// # Errors
    /// Rejects foreign/consumed handles without cancelling another session's gesture.
    pub fn cancel_editable_point_gesture(&mut self, json: &str) -> Result<(), String> {
        let input: GestureRequest = decode_point_request(json)?;
        self.gesture(&input)?;
        self.point_gestures.remove(&input.ticket);
        Ok(())
    }

    /// Trusted host preparation: independently replay the command and validate complete
    /// source/design/feature parity. Persist candidate project/design/digest before apply.
    ///
    /// # Errors
    /// Rejects stale inputs, infeasible terminals and preparation count/byte exhaustion.
    pub fn prepare_editable_point_commit(&mut self, json: &str) -> Result<String, String> {
        if self.point_commits.len() >= MAX_COMMITS {
            return Err("release a point commit before preparing more than eight".into());
        }
        let input: PrepareRequest = decode_point_request(json)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("point commit expected input is stale or foreign".into());
        }
        let project = session.export_project_json().map_err(|e| e.to_string())?;
        self.reserve_point_bytes(project.len().saturating_add(json.len()))?;
        let prepared = session
            .prepare_point_gesture_commit(&input.command)
            .map_err(|e| e.to_string())?;
        let ticket = self.next_point_ticket(input.session, "point-commit")?;
        let encoded = encode(
            &json!({"ticket":ticket,"project":project,"design":prepared.design(),
            "source_design_digest":prepared.source_design_digest(),"result":prepared.result()}),
        )?;
        self.reserve_point_bytes(encoded.len())?;
        self.point_commits.insert(
            ticket,
            HeldPointCommit {
                session: input.session,
                expected: input.expected,
                prepared,
                bytes: encoded.len(),
            },
        );
        Ok(encoded)
    }

    /// Trusted synchronous install after the host's durable transaction has completed.
    /// No persistence callback or client-provided acceptance evidence is used.
    ///
    /// # Errors
    /// Rejects foreign/replayed/stale preparations without publishing or consuming them.
    pub fn apply_editable_point_commit(&mut self, json: &str) -> Result<String, String> {
        self.reserve()?;
        let input: CommitRequest = decode_point_request(json)?;
        let held = self
            .point_commits
            .get(&input.ticket)
            .filter(|held| held.session == input.session)
            .ok_or("unknown, foreign or consumed point commit")?;
        let session = self
            .sessions
            .get_mut(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &held.expected {
            return Err("point commit expected input is stale or foreign".into());
        }
        let held = self
            .point_commits
            .remove(&input.ticket)
            .ok_or("unknown point commit")?;
        let accepted = session
            .apply_point_gesture_commit(held.prepared)
            .map_err(|e| e.to_string())?;
        let encoded = encode(&session.state())?;
        self.accepted = Some(accepted.clone());
        self.retained
            .insert(accepted.result().result_id.clone(), accepted);
        Ok(encoded)
    }

    /// # Errors
    /// Rejects foreign/released preparations without modifying another session.
    pub fn release_editable_point_commit(&mut self, json: &str) -> Result<(), String> {
        let input: CommitRequest = decode_point_request(json)?;
        self.point_commits
            .get(&input.ticket)
            .filter(|held| held.session == input.session)
            .ok_or("unknown, foreign or consumed point commit")?;
        self.point_commits.remove(&input.ticket);
        Ok(())
    }

    fn gesture(&self, input: &GestureRequest) -> Result<&HeldGesture, String> {
        self.point_gestures
            .get(&input.ticket)
            .filter(|held| held.session == input.session && held.gesture_id == input.gesture_id)
            .ok_or_else(|| "unknown, foreign or consumed point gesture".into())
    }
    fn next_point_ticket(&mut self, session: u64, kind: &str) -> Result<String, String> {
        self.point_sequence = self
            .point_sequence
            .checked_add(1)
            .ok_or("point ticket sequence exhausted")?;
        Ok(format!("{kind}:{session}:{}", self.point_sequence))
    }
    fn reserve_point_bytes(&self, bytes: usize) -> Result<(), String> {
        let retained = self
            .point_gestures
            .values()
            .map(|held| held.bytes)
            .chain(self.point_commits.values().map(|held| held.bytes))
            .fold(0usize, usize::saturating_add);
        if retained.saturating_add(bytes) > MAX_SEMANTIC_BYTES {
            return Err("retained point gesture semantic byte limit".into());
        }
        Ok(())
    }
}

fn encode(value: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| e.to_string())
}
fn decode_point_request<T: serde::de::DeserializeOwned>(json: &str) -> Result<T, String> {
    if json.len() > MAX_COMMAND_BYTES {
        return Err("point gesture request byte limit exceeded".into());
    }
    decode_session_request(json)
}
