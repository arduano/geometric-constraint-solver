// SPDX-License-Identifier: GPL-3.0-or-later

//! Authenticated exact host-input provenance for chronological lineage steps.

use geosolve_sketch::{ExternalSnapshotSet, ParameterBatch};
use geosolve_sketch_lineage::LineageOpaqueId;
use serde::{Deserialize, Serialize};

use super::{LineageBridgeError, external_input_stamp};

const HOST_INPUT_PROVENANCE_VERSION: u32 = 1;
const MAX_HOST_INPUT_PROVENANCE_COMPONENT_BYTES: usize = 16 * 1024 * 1024;

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
}

impl DecodedLineageHostInputs {
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
    use geosolve_sketch::{ExternalSnapshotSet, ParameterBatch};
    use geosolve_sketch_lineage::LineageOpaqueId;

    use super::{LineageHostInputProvenance, MAX_HOST_INPUT_PROVENANCE_COMPONENT_BYTES};

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
}
