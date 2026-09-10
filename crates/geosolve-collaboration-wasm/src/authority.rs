// SPDX-License-Identifier: GPL-3.0-or-later
//! Trusted server-host staging across asynchronous durability. This adapter never
//! turns a client completion body into independent domain validation.

use std::sync::atomic::{AtomicU32, Ordering};

use geosolve_collaboration::{
    authority::{Completion, DocumentAuthority, PreparedOperation},
    protocol::{Connection, JournalRecord, Limits, MAX_REVISION, Principal, Receipt, Role, Submit},
};
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use crate::{error, json, parse};

const MAX_CHECKPOINT_BYTES: usize = 128 * 1024 * 1024;
static NEXT_HOST: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Configuration {
    document_id: String,
    document_epoch: String,
    server_epoch: String,
    initial_input: String,
    #[serde(default)]
    limits: HostLimits,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
#[allow(clippy::struct_field_names)] // Preserve the existing public core limit names.
struct HostLimits {
    max_sessions: usize,
    max_pending: usize,
    max_pending_per_client: usize,
    max_operations: usize,
    max_command_bytes: usize,
    max_ledger_bytes: usize,
    max_resume_records: usize,
}
impl Default for HostLimits {
    fn default() -> Self {
        let limits = Limits::default();
        Self {
            max_sessions: limits.max_sessions,
            max_pending: limits.max_pending,
            max_pending_per_client: limits.max_pending_per_client,
            max_operations: limits.max_operations,
            max_command_bytes: limits.max_command_bytes,
            max_ledger_bytes: limits.max_ledger_bytes,
            max_resume_records: limits.max_resume_records,
        }
    }
}
impl From<HostLimits> for Limits {
    fn from(limits: HostLimits) -> Self {
        Self {
            max_sessions: limits.max_sessions,
            max_pending: limits.max_pending,
            max_pending_per_client: limits.max_pending_per_client,
            max_operations: limits.max_operations,
            max_command_bytes: limits.max_command_bytes,
            max_ledger_bytes: limits.max_ledger_bytes,
            max_resume_records: limits.max_resume_records,
        }
    }
}

/// Only the authenticated server's independently validated worker constructs this.
#[derive(Debug, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum ValidatedCompletion {
    Accepted {
        #[serde(rename = "acceptedInput")]
        accepted_input: String,
        summary: String,
    },
    Rejected {
        code: String,
        message: String,
    },
}
impl From<ValidatedCompletion> for Completion {
    fn from(completion: ValidatedCompletion) -> Self {
        match completion {
            ValidatedCompletion::Accepted {
                accepted_input,
                summary,
            } => Self::Accepted {
                accepted_input,
                summary,
            },
            ValidatedCompletion::Rejected { code, message } => Self::Rejected { code, message },
        }
    }
}

#[derive(Debug)]
struct PendingWrite {
    stage_digest: String,
    basis_sequence: u64,
    basis_revision: u64,
    basis_input: String,
    candidate: DocumentAuthority,
    record: JournalRecord,
    receipt: Receipt,
    terminal: bool,
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Staging<'a> {
    Duplicate {
        receipt: &'a Receipt,
    },
    Staged {
        #[serde(rename = "stageDigest")]
        stage_digest: &'a str,
        #[serde(rename = "recordJson")]
        record_json: String,
    },
}

/// Server-only bridge. All visible records/receipts remain committed while the
/// host asynchronously persists one staged record and associated model snapshot.
/// Keep this handle server-side; never route client commands to worker completion.
#[derive(Debug)]
#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
pub struct TrustedDocumentHost {
    authority: DocumentAuthority,
    records: Vec<JournalRecord>,
    server_epoch: String,
    instance: u32,
    ticket_sequence: u64,
    active_ticket: Option<(String, PreparedOperation)>,
    pending: Option<PendingWrite>,
    poisoned: bool,
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen)]
impl TrustedDocumentHost {
    /// Starts an empty ledger after independently reconstructing initial geometry.
    ///
    /// # Errors
    /// Rejects malformed config or unusable core resource bounds.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(constructor))]
    pub fn new(configuration_json: &str) -> Result<Self, String> {
        Self::restore(configuration_json, "[]")
    }

    /// Replays exact durable records under a fresh process epoch. This validates
    /// the ordered ledger only; the host separately rebuilds accepted geometry.
    ///
    /// # Errors
    /// Rejects corrupt/partial/reordered histories, wrong identities and bounds.
    pub fn restore(configuration_json: &str, records_json: &str) -> Result<Self, String> {
        let config: Configuration = parse(configuration_json)?;
        if config.limits.max_ledger_bytes > MAX_CHECKPOINT_BYTES / 2 {
            return Err("host ledger limit exceeds adapter checkpoint capacity".into());
        }
        if records_json.len() > MAX_CHECKPOINT_BYTES {
            return Err("authority checkpoint exceeds adapter byte limit".into());
        }
        let records: Vec<JournalRecord> = serde_json::from_str(records_json).map_err(error)?;
        if records.len() > config.limits.max_operations.saturating_mul(2) {
            return Err("authority checkpoint exceeds operation limit".into());
        }
        let server_epoch = config.server_epoch.clone();
        let authority = DocumentAuthority::restore(
            config.document_id,
            config.document_epoch,
            config.server_epoch,
            config.initial_input,
            config.limits.into(),
            &records,
        )
        .map_err(error)?;
        let instance = NEXT_HOST
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| "host instance counter exhausted")?;
        Ok(Self {
            authority,
            instance,
            records,
            server_epoch,
            ticket_sequence: 0,
            active_ticket: None,
            pending: None,
            poisoned: false,
        })
    }

    /// Connects a host-authenticated identity and fresh unguessable session ID.
    /// None of these arguments may come directly from an untrusted join body.
    ///
    /// # Errors
    /// Rejects pending persistence/recovery, invalid identities or core limits.
    pub fn connect(
        &mut self,
        user_id: &str,
        role: &str,
        client_id: &str,
        session_id: &str,
    ) -> Result<String, String> {
        self.require_mutable()?;
        let role = match role {
            "editor" => Role::Editor,
            "viewer" => Role::Viewer,
            _ => return Err("invalid host role".into()),
        };
        let connection = self
            .authority
            .connect(
                Principal {
                    user_id: user_id.into(),
                    role,
                },
                client_id.into(),
                session_id.into(),
            )
            .map_err(error)?;
        json(&connection)
    }

    /// Disconnects disposable session state without deleting admitted operations.
    ///
    /// # Errors
    /// Rejects mutation while persistence is pending or requires recovery.
    pub fn disconnect(&mut self, session_id: &str) -> Result<(), String> {
        self.require_mutable()?;
        self.authority.disconnect(session_id);
        Ok(())
    }

    /// Reads only committed state; a pending candidate never advances these values.
    ///
    /// # Errors
    /// Reports JSON serialization errors.
    pub fn snapshot(&self) -> Result<String, String> {
        json(&serde_json::json!({
            "acceptedRevision":self.authority.accepted_revision(),
            "acceptedInput":self.authority.accepted_input(),
            "latestSequence":self.authority.latest_sequence(),
            "pendingCount":self.authority.pending_count(),
            "ledgerBytes":self.authority.ledger_bytes(),
            "needsRecovery":self.poisoned || self.authority.needs_recovery(),
            "hasPendingStage":self.pending.is_some(),
        }))
    }

    /// Serializes committed journal records only. Config and source/model snapshots
    /// remain separately durable host inputs, not inferred from these records.
    ///
    /// # Errors
    /// Reports serialization errors or exceeded adapter checkpoint capacity.
    pub fn checkpoint(&self) -> Result<String, String> {
        let encoded = json(&self.records)?;
        if encoded.len() > MAX_CHECKPOINT_BYTES {
            return Err("authority checkpoint exceeds adapter byte limit".into());
        }
        Ok(encoded)
    }

    /// Stages admission through a private cloned core. A duplicate returns its
    /// original committed receipt without another append or speculative ACK.
    ///
    /// # Errors
    /// Rejects pending stages, forged/stale requests, changed duplicates or bounds.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = stageAdmission))]
    pub fn stage_admission(&mut self, request_json: &str) -> Result<String, String> {
        self.require_mutable()?;
        let request: Submit = parse(request_json)?;
        let mut candidate = self.authority.clone();
        let mut captured = None;
        let receipt = candidate
            .admit(&request, |record| {
                captured = Some(record.clone());
                Ok(())
            })
            .map_err(error)?;
        if let Some(record) = captured {
            self.install_stage(candidate, record, receipt, false)
        } else {
            json(&Staging::Duplicate { receipt: &receipt })
        }
    }

    /// Issues the next real core ticket. Returned command/identity values are
    /// descriptors; completion requires the retained opaque ticket in this handle.
    ///
    /// # Errors
    /// Rejects pending persistence, recovery and ticket counter exhaustion.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = beginNext))]
    pub fn begin_next(&mut self) -> Result<Option<String>, String> {
        self.require_mutable()?;
        if self.active_ticket.is_some() {
            return Ok(None);
        }
        let next = self
            .ticket_sequence
            .checked_add(1)
            .filter(|value| *value <= MAX_REVISION)
            .ok_or("host ticket counter exhausted")?;
        let Some(prepared) = self.authority.begin_next().map_err(error)? else {
            return Ok(None);
        };
        let ticket = format!("{}:{}:{next}", self.server_epoch, self.instance);
        let description = json(&serde_json::json!({
            "ticket":ticket,
            "operation":prepared.operation(),
            "command":prepared.command(),
            "acceptedRevision":prepared.accepted_revision(),
            "acceptedInput":prepared.accepted_input(),
        }))?;
        self.ticket_sequence = next;
        self.active_ticket = Some((ticket, prepared));
        Ok(Some(description))
    }

    /// Stages an independently validated trusted worker outcome. Persist its
    /// source/design/model snapshot recoverably with the terminal record before
    /// calling `commitStage`. Never route client completion bodies here.
    ///
    /// # Errors
    /// Rejects unknown/stale tickets, malformed results or core exact-input failure.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = stageValidatedCompletion))]
    pub fn stage_validated_completion(
        &mut self,
        ticket: &str,
        completion_json: &str,
    ) -> Result<String, String> {
        self.require_mutable()?;
        let completion: ValidatedCompletion = parse(completion_json)?;
        let (held_ticket, prepared) = self.active_ticket.as_ref().ok_or("no active host ticket")?;
        if held_ticket != ticket {
            return Err("unknown or stale host ticket".into());
        }
        let mut candidate = self.authority.clone();
        let mut captured = None;
        let receipt = candidate
            .complete(prepared, completion.into(), |record| {
                captured = Some(record.clone());
                Ok(())
            })
            .map_err(error)?;
        self.install_stage(
            candidate,
            captured.ok_or("core completion omitted journal record")?,
            receipt,
            true,
        )
    }

    /// Installs exactly one candidate after the host's asynchronous append+sync
    /// succeeds. The returned receipt is the first visible new acknowledgement.
    ///
    /// # Errors
    /// Rejects recovery, wrong/reused digest or changed committed basis.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = commitStage))]
    pub fn commit_stage(&mut self, stage_digest: &str) -> Result<String, String> {
        self.require_healthy()?;
        let stage = self.stage(stage_digest)?;
        if stage.basis_sequence != self.authority.latest_sequence()
            || stage.basis_revision != self.authority.accepted_revision()
            || stage.basis_input != self.authority.accepted_input()
        {
            return Err("staged authority basis changed".into());
        }
        let response = json(&stage.receipt)?;
        let stage = self.pending.take().ok_or("no pending authority stage")?;
        self.authority = stage.candidate;
        self.records.push(stage.record);
        if stage.terminal {
            self.active_ticket = None;
        }
        Ok(response)
    }

    /// Marks any uncertain append as recovery-required. It is unsafe to retry a
    /// possibly durable operation against this handle; reconstruct from disk.
    ///
    /// # Errors
    /// Rejects unknown/reused stages without discarding the actual pending stage.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen(js_name = failStage))]
    pub fn fail_stage(&mut self, stage_digest: &str) -> Result<(), String> {
        self.stage(stage_digest)?;
        self.poisoned = true;
        self.pending = None;
        self.active_ticket = None;
        Ok(())
    }

    /// Reads this authenticated session's committed receipt during slow writes.
    ///
    /// # Errors
    /// Rejects forged/stale connections or malformed request IDs.
    pub fn receipt(&self, connection_json: &str, request_id: &str) -> Result<String, String> {
        let connection: Connection = parse(connection_json)?;
        json(
            &self
                .authority
                .receipt(&connection, request_id)
                .map_err(error)?,
        )
    }

    /// Reads a bounded committed event page independent of presence and staging.
    ///
    /// # Errors
    /// Rejects forged/stale connections and invalid/ahead cursors.
    pub fn resume(&self, connection_json: &str, after: f64) -> Result<String, String> {
        if !after.is_finite()
            || after < 0.0
            || after.fract() != 0.0
            || after > 9_007_199_254_740_991.0
        {
            return Err("resume sequence must be a nonnegative safe integer".into());
        }
        let connection: Connection = parse(connection_json)?;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let sequence = after as u64;
        json(
            &self
                .authority
                .resume(&connection, sequence)
                .map_err(error)?,
        )
    }
}

impl TrustedDocumentHost {
    fn require_healthy(&self) -> Result<(), String> {
        if self.poisoned || self.authority.needs_recovery() {
            return Err("authority requires durable recovery".into());
        }
        Ok(())
    }
    fn require_mutable(&self) -> Result<(), String> {
        self.require_healthy()?;
        if self.pending.is_some() {
            return Err("authority persistence is pending".into());
        }
        Ok(())
    }
    fn stage(&self, digest: &str) -> Result<&PendingWrite, String> {
        self.pending
            .as_ref()
            .filter(|stage| stage.stage_digest == digest)
            .ok_or_else(|| "unknown or stale authority stage".into())
    }
    fn install_stage(
        &mut self,
        candidate: DocumentAuthority,
        record: JournalRecord,
        receipt: Receipt,
        terminal: bool,
    ) -> Result<String, String> {
        let stage_digest = format!("{}:{}:{}", self.server_epoch, self.instance, record.digest);
        let response = json(&Staging::Staged {
            stage_digest: &stage_digest,
            record_json: json(&record)?,
        })?;
        self.pending = Some(PendingWrite {
            stage_digest,
            basis_sequence: self.authority.latest_sequence(),
            basis_revision: self.authority.accepted_revision(),
            basis_input: self.authority.accepted_input().into(),
            candidate,
            record,
            receipt,
            terminal,
        });
        Ok(response)
    }
}
