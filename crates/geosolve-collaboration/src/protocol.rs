// SPDX-License-Identifier: GPL-3.0-or-later
//! Versioned transport values. No wire value is an accepted-scene capability.

use serde::{Deserialize, Serialize};

/// Increment for incompatible command, reconciliation or indexing semantics.
pub const PROTOCOL_VERSION: u32 = 1;
/// Counters are exact in both native Rust and JavaScript hosts.
pub const MAX_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    Editor,
}

/// Host-authenticated identity. Never deserialize this from a client's join body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Principal {
    pub user_id: String,
    pub role: Role,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationId {
    pub user_id: String,
    pub client_id: String,
    pub request_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connection {
    pub protocol: u32,
    pub document_id: String,
    pub document_epoch: String,
    pub server_epoch: String,
    pub session_id: String,
    pub client_id: String,
    pub user_id: String,
    pub role: Role,
}

/// Immutable client intent. The domain adapter validates the typed payload and
/// resolves explicit stable targets against the latest accepted input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Command {
    pub kind: CommandKind,
    pub basis_revision: u64,
    pub payload: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandKind {
    Semantic,
    Apply,
    Undo,
    Redo,
    File,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Submit {
    pub connection: Connection,
    pub request_id: String,
    pub command: Command,
}

/// A receipt describes server publication; it cannot recreate its authority.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Outcome {
    Accepted {
        revision: u64,
        accepted_input: String,
        summary: String,
    },
    Rejected {
        code: String,
        message: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub operation: OperationId,
    pub admission: u64,
    pub outcome: Option<Outcome>,
}

/// Durable events can be resumed independently from disposable presence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
pub enum DocumentEvent {
    Admitted {
        operation: OperationId,
        admission: u64,
        command: Command,
        digest: String,
    },
    Finished {
        operation: OperationId,
        outcome: Outcome,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct JournalRecord {
    pub protocol: u32,
    pub document_id: String,
    pub document_epoch: String,
    pub sequence: u64,
    pub accepted_revision: u64,
    pub accepted_input: String,
    pub previous_digest: String,
    pub event: DocumentEvent,
    pub digest: String,
}

/// Resume cursors older than retained data must receive a checkpoint, never a
/// misleading empty delta. Transport buffers need not retain these full records.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum Resume {
    Events { records: Vec<JournalRecord> },
    CheckpointRequired { latest_sequence: u64 },
}

/// Hard ingress bounds. Exhaustion preserves all admitted outcomes; nothing is
/// silently evicted from the deduplication ledger.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Limits {
    pub max_sessions: usize,
    pub max_pending: usize,
    pub max_pending_per_client: usize,
    pub max_operations: usize,
    pub max_command_bytes: usize,
    pub max_ledger_bytes: usize,
    pub max_resume_records: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_sessions: 64,
            max_pending: 128,
            max_pending_per_client: 16,
            max_operations: 16_384,
            max_command_bytes: 4 * 1024 * 1024,
            max_ledger_bytes: 64 * 1024 * 1024,
            max_resume_records: 256,
        }
    }
}
