// SPDX-License-Identifier: GPL-3.0-or-later
//! Thin native/WASM preparation handles over the shared headless authoring owner.
//! Server hosts independently call these in their domain worker; browser results
//! remain provisional and cannot supply server completion authority.

use crate::{EngineAdapter, decode_session_request};
use geosolve_sketch_code::{
    CodeSessionIdentity, ManagedSketchMutation, PreparedManagedMutationReceipt,
};
use geosolve_sketch_engine::{
    AuthoringValueWrite, PreparedAuthoringMutation, PreparedAuthoringSource,
    PreparedAuthoringValues,
};
use serde::Deserialize;

const MAX_PREPARED: usize = 32;
const MAX_PREPARED_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Action {
    Values {
        writes: Vec<AuthoringValueWrite>,
    },
    Mutation {
        mutation: ManagedSketchMutation,
        candidate_name_high_water: u64,
    },
    Source {
        source: String,
    },
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrepareRequest {
    session: u64,
    expected: CodeSessionIdentity,
    action: Action,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ApplyRequest {
    session: u64,
    ticket: String,
    receipt: PreparedManagedMutationReceipt,
}
#[derive(Debug)]
enum Prepared {
    Values(PreparedAuthoringValues),
    Mutation(PreparedAuthoringMutation),
    Source(PreparedAuthoringSource),
}
#[derive(Debug)]
pub(super) struct HeldAuthoring {
    pub session: u64,
    prepared: Prepared,
    bytes: usize,
}

impl EngineAdapter {
    /// Prepare a compiler operation from authentic current native session state.
    ///
    /// # Errors
    /// Rejects stale sessions/targets, malformed source and exhausted handle bounds.
    pub fn prepare_editable_authoring(&mut self, json: &str) -> Result<String, String> {
        if self.authoring.len() >= MAX_PREPARED {
            return Err("release an authoring ticket before preparing more than 32".into());
        }
        let input: PrepareRequest = decode_session_request(json)?;
        let session = self
            .sessions
            .get(&input.session)
            .ok_or("unknown editable session")?;
        if session.token() != &input.expected {
            return Err("authoring expected input is stale or foreign".into());
        }
        let prepared = match input.action {
            Action::Values { writes } => Prepared::Values(
                session
                    .prepare_managed_values(writes)
                    .map_err(|e| e.to_string())?,
            ),
            Action::Mutation {
                mutation,
                candidate_name_high_water,
            } => Prepared::Mutation(
                session
                    .prepare_managed_mutation(mutation, candidate_name_high_water)
                    .map_err(|e| e.to_string())?,
            ),
            Action::Source { source } => Prepared::Source(
                session
                    .prepare_managed_source(source)
                    .map_err(|e| e.to_string())?,
            ),
        };
        let (ticket, kind, request) = match &prepared {
            Prepared::Values(value) => (
                value.request().ticket.ticket_digest.clone(),
                "values",
                serde_json::to_value(value.request()),
            ),
            Prepared::Mutation(value) => (
                value.request().ticket.ticket_digest.clone(),
                "mutation",
                serde_json::to_value(value.request()),
            ),
            Prepared::Source(value) => (
                value.request().ticket.ticket_digest.clone(),
                "source",
                serde_json::to_value(value.request()),
            ),
        };
        let encoded = serde_json::to_string(&serde_json::json!({ "ticket": ticket, "kind": kind, "request": request.map_err(|e| e.to_string())? })).map_err(|e| e.to_string())?;
        let retained: usize = self.authoring.values().map(|item| item.bytes).sum();
        if retained.saturating_add(encoded.len()) > MAX_PREPARED_BYTES {
            return Err("retained authoring preparation byte limit".into());
        }
        self.authoring.insert(
            ticket,
            HeldAuthoring {
                session: input.session,
                prepared,
                bytes: encoded.len(),
            },
        );
        Ok(encoded)
    }

    /// Authenticate a compiler receipt against the held real native ticket and
    /// independently validate geometry. Worker-owned state is still provisional
    /// until the trusted host journals and publishes its collaboration transaction.
    ///
    /// # Errors
    /// Foreign/replayed/stale tickets and invalid geometry retain accepted state.
    pub fn apply_editable_authoring(&mut self, json: &str) -> Result<String, String> {
        self.reserve()?;
        let input: ApplyRequest = decode_session_request(json)?;
        let held = self
            .authoring
            .get(&input.ticket)
            .filter(|held| held.session == input.session)
            .ok_or("unknown or foreign authoring ticket")?;
        let session = self
            .sessions
            .get_mut(&input.session)
            .ok_or("unknown editable session")?;
        let (accepted, changes) = match &held.prepared {
            Prepared::Values(value) => {
                let (accepted, inverse) = session
                    .apply_managed_values(value, input.receipt)
                    .map_err(|e| e.to_string())?;
                let changes = inverse
                    .changes()
                    .map(|(write, before)| serde_json::json!({ "write": write, "before": before }))
                    .collect::<Vec<_>>();
                (accepted, changes)
            }
            Prepared::Mutation(value) => (
                session
                    .apply_managed_mutation(value, input.receipt)
                    .map_err(|e| e.to_string())?,
                Vec::new(),
            ),
            Prepared::Source(value) => (
                session
                    .apply_managed_source(value, input.receipt)
                    .map_err(|e| e.to_string())?,
                Vec::new(),
            ),
        };
        let encoded = serde_json::to_string(
            &serde_json::json!({ "state": session.state(), "valueChanges": changes }),
        )
        .map_err(|e| e.to_string())?;
        self.authoring.remove(&input.ticket);
        self.accepted = Some(accepted.clone());
        self.retained
            .insert(accepted.result().result_id.clone(), accepted);
        Ok(encoded)
    }

    pub fn release_editable_authoring(&mut self, ticket: &str) -> bool {
        self.authoring.remove(ticket).is_some()
    }

    /// # Errors
    /// Rejects absent sessions or invalid project serialization.
    pub fn export_editable_project(&self, id: &str) -> Result<String, String> {
        let id: u64 = id.parse().map_err(|_| "invalid session ID")?;
        self.sessions
            .get(&id)
            .ok_or("unknown editable session")?
            .export_project_json()
            .map_err(|e| e.to_string())
    }

    /// # Errors
    /// Rejects absent sessions or invalid source/design serialization.
    pub fn editable_source_design_digest(&self, id: &str) -> Result<String, String> {
        let id: u64 = id.parse().map_err(|_| "invalid session ID")?;
        self.sessions
            .get(&id)
            .ok_or("unknown editable session")?
            .source_design_digest()
            .map_err(|e| e.to_string())
    }
}
