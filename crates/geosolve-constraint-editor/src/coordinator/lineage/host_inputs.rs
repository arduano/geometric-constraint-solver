// SPDX-License-Identifier: GPL-3.0-or-later

//! Authenticated exact host-input provenance for chronological lineage steps.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{ExternalSnapshotSet, ParameterBatch};
use geosolve_sketch_lineage::{
    LineageOpaqueId, MAX_LINEAGE_HISTORY_ENTRIES, MAX_LINEAGE_SESSION_JSON_BYTES,
};
use serde::{Deserialize, Serialize};

use super::{LineageBridgeError, external_input_stamp};

const HOST_INPUT_PROVENANCE_VERSION: u32 = 1;
const MAX_HOST_INPUT_PROVENANCE_COMPONENT_BYTES: usize = 16 * 1024 * 1024;
const HOST_INPUT_LEDGER_VERSION: u32 = 1;
const MAX_HOST_INPUT_LEDGER_ENTRIES: usize = MAX_LINEAGE_HISTORY_ENTRIES * 2 + 1;
const MAX_HOST_INPUT_LEDGER_JSON_BYTES: usize = MAX_LINEAGE_SESSION_JSON_BYTES;

/// Canonical exact host inputs retained beside one bridge action or imported
/// baseline. The individual owning-domain payloads authenticate their own
/// contents; `external_inputs` binds the pair used by the lineage evaluator.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LineageHostInputProvenance {
    version: u32,
    parameter_batch_json: String,
    external_snapshot_set_json: String,
    external_inputs: LineageOpaqueId,
}

/// Independently decoded exact input pair for one chronological prefix.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::coordinator) struct DecodedLineageHostInputs {
    parameters: ParameterBatch,
    snapshots: ExternalSnapshotSet,
}

/// Exact payloads needed only by accepted authority retained in Undo/Redo.
/// The generic lineage session stores opaque input stamps; this private
/// coordinator ledger retains the domain payloads needed after a workspace
/// restart without making host concepts part of the generic lineage API.
#[derive(Clone, Debug, Default)]
pub(super) struct LineageHostInputLedger {
    entries: BTreeMap<LineageOpaqueId, LineageHostInputProvenance>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct LineageHostInputLedgerWire {
    version: u32,
    entries: Vec<LineageHostInputProvenance>,
}

impl LineageHostInputProvenance {
    pub(super) fn capture(
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
    ) -> Result<Self, LineageBridgeError> {
        let parameter_batch_json = parameters.to_canonical_json()?;
        let external_snapshot_set_json = snapshots
            .to_canonical_json()
            .map_err(|error| LineageBridgeError::InvalidExternalSnapshot(error.to_string()))?;
        validate_component_bounds(&parameter_batch_json, &external_snapshot_set_json)?;
        Ok(Self {
            version: HOST_INPUT_PROVENANCE_VERSION,
            parameter_batch_json,
            external_snapshot_set_json,
            external_inputs: external_input_stamp(parameters, snapshots)?,
        })
    }

    pub(super) fn decode(&self) -> Result<DecodedLineageHostInputs, LineageBridgeError> {
        if self.version != HOST_INPUT_PROVENANCE_VERSION {
            return Err(LineageBridgeError::InvalidMaterialization(
                "unsupported historical host-input provenance version",
            ));
        }
        validate_component_bounds(&self.parameter_batch_json, &self.external_snapshot_set_json)?;
        let parameters = ParameterBatch::from_json(&self.parameter_batch_json)?;
        if parameters.to_canonical_json()? != self.parameter_batch_json {
            return Err(LineageBridgeError::InvalidMaterialization(
                "historical parameter input is not canonical",
            ));
        }
        let snapshots = ExternalSnapshotSet::from_json(&self.external_snapshot_set_json)
            .map_err(|error| LineageBridgeError::InvalidExternalSnapshot(error.to_string()))?;
        let canonical_snapshots = snapshots
            .to_canonical_json()
            .map_err(|error| LineageBridgeError::InvalidExternalSnapshot(error.to_string()))?;
        if canonical_snapshots != self.external_snapshot_set_json {
            return Err(LineageBridgeError::InvalidMaterialization(
                "historical external snapshot input is not canonical",
            ));
        }
        let external_inputs = external_input_stamp(&parameters, &snapshots)?;
        if external_inputs != self.external_inputs {
            return Err(LineageBridgeError::InvalidMaterialization(
                "historical host-input provenance stamp does not match its exact payloads",
            ));
        }
        Ok(DecodedLineageHostInputs {
            parameters,
            snapshots,
        })
    }

    fn stamp(&self) -> &LineageOpaqueId {
        &self.external_inputs
    }
}

impl LineageHostInputLedger {
    pub(super) fn from_json(json: &str) -> Result<Self, LineageBridgeError> {
        if json.len() > MAX_HOST_INPUT_LEDGER_JSON_BYTES {
            return Err(LineageBridgeError::InvalidMaterialization(
                "historical host-input ledger exceeds its bounded byte limit",
            ));
        }
        let wire = serde_json::from_str::<LineageHostInputLedgerWire>(json)?;
        if wire.version != HOST_INPUT_LEDGER_VERSION {
            return Err(LineageBridgeError::InvalidMaterialization(
                "unsupported historical host-input ledger version",
            ));
        }
        if wire.entries.len() > MAX_HOST_INPUT_LEDGER_ENTRIES {
            return Err(LineageBridgeError::InvalidMaterialization(
                "historical host-input ledger exceeds its bounded entry limit",
            ));
        }
        let mut entries = BTreeMap::new();
        for entry in wire.entries {
            entry.decode()?;
            let stamp = entry.stamp().clone();
            if entries.insert(stamp, entry).is_some() {
                return Err(LineageBridgeError::InvalidMaterialization(
                    "historical host-input ledger contains a duplicate stamp",
                ));
            }
        }
        let ledger = Self { entries };
        if ledger.to_canonical_json()? != json {
            return Err(LineageBridgeError::InvalidMaterialization(
                "historical host-input ledger is not canonical",
            ));
        }
        Ok(ledger)
    }

    pub(super) fn to_canonical_json(&self) -> Result<String, LineageBridgeError> {
        if self.entries.len() > MAX_HOST_INPUT_LEDGER_ENTRIES {
            return Err(LineageBridgeError::InvalidMaterialization(
                "historical host-input ledger exceeds its bounded entry limit",
            ));
        }
        let json = serde_json::to_string(&LineageHostInputLedgerWire {
            version: HOST_INPUT_LEDGER_VERSION,
            entries: self.entries.values().cloned().collect(),
        })?;
        if json.len() > MAX_HOST_INPUT_LEDGER_JSON_BYTES {
            return Err(LineageBridgeError::InvalidMaterialization(
                "historical host-input ledger exceeds its bounded byte limit",
            ));
        }
        Ok(json)
    }

    pub(super) fn retain_accepted_pair(
        &mut self,
        parameters: &ParameterBatch,
        snapshots: &ExternalSnapshotSet,
        required: &BTreeSet<LineageOpaqueId>,
    ) -> Result<(), LineageBridgeError> {
        let entry = LineageHostInputProvenance::capture(parameters, snapshots)?;
        let stamp = entry.stamp().clone();
        let mut candidate = self.clone();
        if let Some(existing) = candidate.entries.get(&stamp)
            && existing != &entry
        {
            return Err(LineageBridgeError::AcceptedExternalInputMismatch);
        }
        candidate.entries.insert(stamp, entry);
        candidate
            .entries
            .retain(|stamp, _| required.contains(stamp));
        candidate.to_canonical_json()?;
        *self = candidate;
        Ok(())
    }

    pub(super) fn retain_required(
        &mut self,
        required: &BTreeSet<LineageOpaqueId>,
    ) -> Result<(), LineageBridgeError> {
        let mut candidate = self.clone();
        candidate
            .entries
            .retain(|stamp, _| required.contains(stamp));
        candidate.to_canonical_json()?;
        *self = candidate;
        Ok(())
    }

    pub(super) fn decoded_entries(
        &self,
    ) -> Result<Vec<DecodedLineageHostInputs>, LineageBridgeError> {
        self.entries
            .values()
            .map(LineageHostInputProvenance::decode)
            .collect()
    }

    pub(super) fn decoded_entry(
        &self,
        stamp: &LineageOpaqueId,
    ) -> Result<Option<DecodedLineageHostInputs>, LineageBridgeError> {
        self.entries
            .get(stamp)
            .map(LineageHostInputProvenance::decode)
            .transpose()
    }
}

impl DecodedLineageHostInputs {
    pub(in crate::coordinator) const fn new(
        parameters: ParameterBatch,
        snapshots: ExternalSnapshotSet,
    ) -> Self {
        Self {
            parameters,
            snapshots,
        }
    }

    pub(in crate::coordinator) fn parameters(&self) -> &ParameterBatch {
        &self.parameters
    }

    pub(in crate::coordinator) fn snapshots(&self) -> &ExternalSnapshotSet {
        &self.snapshots
    }
}

fn validate_component_bounds(
    parameter_batch_json: &str,
    external_snapshot_set_json: &str,
) -> Result<(), LineageBridgeError> {
    if parameter_batch_json.len() > MAX_HOST_INPUT_PROVENANCE_COMPONENT_BYTES {
        return Err(LineageBridgeError::InvalidMaterialization(
            "historical parameter input exceeds its bounded byte limit",
        ));
    }
    if external_snapshot_set_json.len() > MAX_HOST_INPUT_PROVENANCE_COMPONENT_BYTES {
        return Err(LineageBridgeError::InvalidMaterialization(
            "historical external snapshot input exceeds its bounded byte limit",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use geosolve_sketch::{ExternalSnapshotSet, ParameterBatch};
    use geosolve_sketch_lineage::LineageOpaqueId;

    use super::{
        HOST_INPUT_LEDGER_VERSION, LineageHostInputLedger, LineageHostInputLedgerWire,
        LineageHostInputProvenance, MAX_HOST_INPUT_LEDGER_ENTRIES,
        MAX_HOST_INPUT_PROVENANCE_COMPONENT_BYTES,
    };

    #[test]
    fn exact_host_input_provenance_is_canonical_bounded_and_pair_authenticated() {
        let parameters = ParameterBatch::default();
        let snapshots = ExternalSnapshotSet::default();
        let provenance = LineageHostInputProvenance::capture(&parameters, &snapshots)
            .expect("canonical provenance");
        let decoded = provenance.decode().expect("authenticated provenance");
        assert_eq!(decoded.parameters(), &parameters);
        assert_eq!(decoded.snapshots(), &snapshots);

        let mut noncanonical = provenance.clone();
        noncanonical.parameter_batch_json.insert(0, ' ');
        assert!(
            noncanonical
                .decode()
                .expect_err("noncanonical parameter JSON must fail closed")
                .to_string()
                .contains("not canonical")
        );

        let mut mismatched = provenance.clone();
        mismatched.external_inputs =
            LineageOpaqueId::new("external-inputs:mismatched").expect("bounded mismatched stamp");
        assert!(
            mismatched
                .decode()
                .expect_err("a caller-certified pair stamp must fail closed")
                .to_string()
                .contains("stamp does not match")
        );

        let mut oversized = provenance;
        oversized.external_snapshot_set_json =
            " ".repeat(MAX_HOST_INPUT_PROVENANCE_COMPONENT_BYTES + 1);
        assert!(
            oversized
                .decode()
                .expect_err("oversized snapshot provenance must fail before decoding")
                .to_string()
                .contains("bounded byte limit")
        );
    }

    #[test]
    fn historical_host_input_ledger_is_canonical_bounded_and_pair_authenticated() {
        let parameters = ParameterBatch::default();
        let snapshots = ExternalSnapshotSet::default();
        let provenance = LineageHostInputProvenance::capture(&parameters, &snapshots)
            .expect("canonical provenance");
        let required = BTreeSet::from([provenance.stamp().clone()]);
        let mut ledger = LineageHostInputLedger::default();
        ledger
            .retain_accepted_pair(&parameters, &snapshots, &required)
            .expect("retain accepted pair");
        let json = ledger.to_canonical_json().expect("canonical ledger");
        let restored = LineageHostInputLedger::from_json(&json).expect("restore ledger");
        assert_eq!(
            restored.decoded_entries().expect("decode restored entries"),
            vec![super::DecodedLineageHostInputs::new(parameters, snapshots)]
        );

        let mut noncanonical = json;
        noncanonical.insert(0, ' ');
        assert!(
            LineageHostInputLedger::from_json(&noncanonical)
                .expect_err("noncanonical ledger must fail closed")
                .to_string()
                .contains("not canonical")
        );

        let oversized = serde_json::to_string(&LineageHostInputLedgerWire {
            version: HOST_INPUT_LEDGER_VERSION,
            entries: vec![provenance; MAX_HOST_INPUT_LEDGER_ENTRIES + 1],
        })
        .expect("oversized ledger wire");
        assert!(
            LineageHostInputLedger::from_json(&oversized)
                .expect_err("oversized ledger must fail before entry decoding")
                .to_string()
                .contains("bounded entry limit")
        );
    }
}
