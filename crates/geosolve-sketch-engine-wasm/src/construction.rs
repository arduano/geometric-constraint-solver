// SPDX-License-Identifier: GPL-3.0-or-later
//! Retained construction, independent replay and two-phase compiler/durability binding.
use crate::{EngineAdapter, decode_session_request};
use geosolve_sketch_code::{CodeSessionIdentity, PreparedManagedMutationReceipt};
use geosolve_sketch_engine::{
    ConstructionCommand, ConstructionPrediction, ConstructionSample, ConstructionTool,
    PreparedConstruction, PreparedConstructionCommit,
};
use serde::Deserialize;
use serde_json::{Value, json};

const MAX_HELD: usize = 24;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const TRACE_BYTES: usize = 1024 * 1024;
#[derive(Debug)]
enum Held {
    Prediction {
        gesture_id: u64,
        prediction: Box<ConstructionPrediction>,
    },
    Preparation(Box<PreparedConstruction>),
    Commit(Box<PreparedConstructionCommit>),
}
#[derive(Debug)]
pub(super) struct HeldConstruction {
    pub session: u64,
    expected: CodeSessionIdentity,
    held: Held,
    bytes: usize,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Begin {
    session: u64,
    expected: CodeSessionIdentity,
    tool: ConstructionTool,
    gesture_id: u64,
    viewport: Value,
    role: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Advance {
    session: u64,
    ticket: String,
    gesture_id: u64,
    sample: ConstructionSample,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PredictionRequest {
    session: u64,
    ticket: String,
    gesture_id: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Prepare {
    session: u64,
    expected: CodeSessionIdentity,
    command: ConstructionCommand,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Resolve {
    session: u64,
    ticket: String,
    receipt: PreparedManagedMutationReceipt,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Ticket {
    session: u64,
    ticket: String,
}

impl EngineAdapter {
    /// # Errors
    /// Rejects invalid or stale inputs, exhausted count/byte bounds and native failures.
    pub fn begin_editable_construction(&mut self, json: &str) -> Result<String, String> {
        self.reserve_construction(TRACE_BYTES, None)?;
        let input: Begin = decode_session_request(json)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("construction input is stale or foreign".into());
        }
        let bytes = session
            .export_project_json()
            .map_err(|e| e.to_string())?
            .len()
            .saturating_add(encode(&session.design())?.len())
            .saturating_add(TRACE_BYTES);
        self.reserve_construction(bytes, None)?;
        let prediction = session
            .begin_construction(
                input.tool,
                input.gesture_id,
                serde_json::from_value(input.viewport).map_err(|e| e.to_string())?,
                serde_json::from_value(input.role).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        let ticket = self.construction_ticket(input.session)?;
        let encoded = encode(&json!({"ticket":ticket}))?;
        self.constructions.insert(
            ticket,
            HeldConstruction {
                session: input.session,
                expected: input.expected,
                held: Held::Prediction {
                    gesture_id: input.gesture_id,
                    prediction: Box::new(prediction),
                },
                bytes,
            },
        );
        Ok(encoded)
    }
    /// # Errors
    /// Invalid routing and sequence preserve the retained draft and all accepted state.
    pub fn advance_editable_construction(&mut self, json: &str) -> Result<String, String> {
        if json.len() > TRACE_BYTES {
            return Err("construction sample byte limit".into());
        }
        let input: Advance = decode_session_request(json)?;
        let held = self
            .constructions
            .get_mut(&input.ticket)
            .filter(|held| held.session == input.session)
            .ok_or("foreign or consumed construction")?;
        let Held::Prediction {
            gesture_id,
            prediction,
        } = &mut held.held
        else {
            return Err("not a retained construction".into());
        };
        if *gesture_id != input.gesture_id {
            return Err("foreign construction gesture".into());
        }
        encode(
            &prediction
                .advance(input.gesture_id, input.sample)
                .map_err(|e| e.to_string())?,
        )
    }
    /// # Errors
    /// Rejects foreign/consumed prediction handles or unavailable detached scene.
    pub fn editable_construction_presentation(&self, json: &str) -> Result<String, String> {
        let input: PredictionRequest = decode_session_request(json)?;
        self.construction_prediction(&input)?
            .presentation_json()
            .map_err(|e| e.to_string())
    }

    /// # Errors
    /// Rejects foreign/consumed handles or unavailable presentation.
    pub fn editable_construction_scene(&self, json: &str) -> Result<String, String> {
        let input: PredictionRequest = decode_session_request(json)?;
        self.construction_prediction(&input)?
            .scene_json()
            .map_err(|e| e.to_string())
    }
    /// # Errors
    /// Invalid routing preserves handles; unfinished terminal consumes only the prediction.
    pub fn finish_editable_construction(&mut self, json: &str) -> Result<String, String> {
        let input: PredictionRequest = decode_session_request(json)?;
        self.construction_prediction(&input)?;
        let held = self
            .constructions
            .remove(&input.ticket)
            .ok_or("unknown construction")?;
        let Held::Prediction { prediction, .. } = held.held else {
            return Err("not a prediction".into());
        };
        let terminal = prediction
            .finish(input.gesture_id)
            .map_err(|e| e.to_string())?;
        encode(terminal.command())
    }
    /// # Errors
    /// Rejects foreign gesture identity without dropping another user's draft.
    pub fn cancel_editable_construction(&mut self, json: &str) -> Result<(), String> {
        let input: PredictionRequest = decode_session_request(json)?;
        self.construction_prediction(&input)?;
        self.constructions.remove(&input.ticket);
        Ok(())
    }
    /// Trusted server replay prepares an exact managed compiler request.
    ///
    /// # Errors
    /// Stale inputs, divergent operands/branches or failed replay preserve all state.
    pub fn prepare_editable_construction(&mut self, json: &str) -> Result<String, String> {
        self.reserve_construction(json.len(), None)?;
        let input: Prepare = decode_session_request(json)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("construction input is stale or foreign".into());
        }
        let prepared = session
            .prepare_construction(&input.command)
            .map_err(|e| e.to_string())?;
        let ticket = self.construction_ticket(input.session)?;
        let encoded = encode(
            &json!({"ticket":ticket,"request":prepared.request(),"declarations":prepared.declarations().collect::<Vec<_>>()}),
        )?;
        self.reserve_construction(encoded.len(), None)?;
        self.constructions.insert(
            ticket,
            HeldConstruction {
                session: input.session,
                expected: input.expected,
                held: Held::Preparation(Box::new(prepared)),
                bytes: encoded.len(),
            },
        );
        Ok(encoded)
    }
    /// Independently resolves the compiler receipt into an unpublished durable candidate.
    ///
    /// # Errors
    /// Failed receipts/parity preserve preparation and accepted state; success consumes preparation.
    pub fn resolve_editable_construction(&mut self, json: &str) -> Result<String, String> {
        let input: Resolve = decode_session_request(json)?;
        let held = self
            .constructions
            .get(&input.ticket)
            .filter(|held| held.session == input.session)
            .ok_or("foreign or consumed construction preparation")?;
        let Held::Preparation(prepared) = &held.held else {
            return Err("not a construction preparation".into());
        };
        let expected = held.expected.clone();
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        let candidate = session
            .resolve_construction(prepared, input.receipt)
            .map_err(|e| e.to_string())?;
        let ticket = self.construction_ticket(input.session)?;
        let encoded = encode(
            &json!({"ticket":ticket,"project":candidate.project_json(),"design":candidate.design(),
            "source_design_digest":candidate.source_design_digest(),"result":candidate.result(),"declarations":candidate.declarations()}),
        )?;
        self.reserve_construction(encoded.len(), Some(&input.ticket))?;
        self.constructions.remove(&input.ticket);
        self.constructions.insert(
            ticket,
            HeldConstruction {
                session: input.session,
                expected,
                held: Held::Commit(Box::new(candidate)),
                bytes: encoded.len(),
            },
        );
        Ok(encoded)
    }
    /// Trusted synchronous install only after the host has durably persisted the candidate.
    ///
    /// # Errors
    /// Stale/foreign/replayed tickets preserve the complete accepted session.
    pub fn apply_editable_construction_commit(&mut self, json: &str) -> Result<String, String> {
        self.reserve()?;
        let input: Ticket = decode_session_request(json)?;
        let held = self
            .constructions
            .get(&input.ticket)
            .filter(|held| held.session == input.session)
            .ok_or("foreign or consumed construction commit")?;
        if !matches!(held.held, Held::Commit(_)) {
            return Err("not a construction commit".into());
        }
        let session = self
            .sessions
            .get_mut(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &held.expected {
            return Err("construction commit is stale or foreign".into());
        }
        let held = self
            .constructions
            .remove(&input.ticket)
            .ok_or("unknown construction commit")?;
        let Held::Commit(prepared) = held.held else {
            return Err("not a construction commit".into());
        };
        let accepted = session
            .apply_construction_commit(*prepared)
            .map_err(|e| e.to_string())?;
        let encoded = encode(&session.state())?;
        self.accepted = Some(accepted.clone());
        self.retained
            .insert(accepted.result().result_id.clone(), accepted);
        Ok(encoded)
    }
    /// # Errors
    /// Rejects foreign handles; explicit release may discard either compiler or commit stage.
    pub fn release_editable_construction(&mut self, json: &str) -> Result<(), String> {
        let input: Ticket = decode_session_request(json)?;
        self.constructions
            .get(&input.ticket)
            .filter(|held| {
                held.session == input.session && !matches!(held.held, Held::Prediction { .. })
            })
            .ok_or("foreign or consumed construction preparation")?;
        self.constructions.remove(&input.ticket);
        Ok(())
    }
    fn construction_prediction(
        &self,
        input: &PredictionRequest,
    ) -> Result<&ConstructionPrediction, String> {
        let held = self
            .constructions
            .get(&input.ticket)
            .filter(|held| held.session == input.session)
            .ok_or("foreign or consumed construction")?;
        let Held::Prediction {
            gesture_id,
            prediction,
        } = &held.held
        else {
            return Err("not a retained construction".into());
        };
        if *gesture_id != input.gesture_id {
            return Err("foreign construction gesture".into());
        }
        Ok(prediction)
    }
    fn construction_ticket(&mut self, session: u64) -> Result<String, String> {
        self.point_sequence = self
            .point_sequence
            .checked_add(1)
            .ok_or("construction ticket sequence exhausted")?;
        Ok(format!("construction:{session}:{}", self.point_sequence))
    }
    fn reserve_construction(&self, bytes: usize, replacing: Option<&str>) -> Result<(), String> {
        if self
            .constructions
            .len()
            .saturating_sub(usize::from(replacing.is_some()))
            >= MAX_HELD
        {
            return Err("release a construction handle before retaining more than 24".into());
        }
        let retained = self
            .constructions
            .iter()
            .filter(|(ticket, _)| replacing != Some(ticket.as_str()))
            .map(|(_, held)| held.bytes)
            .fold(0usize, usize::saturating_add);
        if retained.saturating_add(bytes) > MAX_BYTES {
            return Err("retained construction semantic byte limit".into());
        }
        Ok(())
    }
}
fn encode(value: &impl serde::Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| e.to_string())
}
