// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact decoding for opaque host-input bytes retained by sketch intent.
//!
//! `geosolve-sketch-intent` deliberately treats these payloads as opaque. The
//! editor materializer is the first layer that understands their native Rust
//! types, so it also owns strict canonical decoding before the payloads reach
//! the retained sketch session.

use geosolve_sketch::{
    DocumentError, ExternalSnapshotInputError, ExternalSnapshotSet, ParameterBatch,
};
use geosolve_sketch_intent::IntentExternalInputs;
use thiserror::Error;

#[derive(Clone, Debug)]
pub(crate) struct DecodedIntentExternalInputs {
    pub(crate) parameters: ParameterBatch,
    pub(crate) external_snapshots: ExternalSnapshotSet,
}

#[derive(Debug, Error)]
pub(crate) enum IntentHostInputError {
    #[error("intent {component} payload is not UTF-8")]
    NonUtf8 { component: &'static str },
    #[error("invalid intent parameter batch: {0}")]
    Parameter(#[source] DocumentError),
    #[error("invalid intent external snapshot set: {0}")]
    External(#[source] ExternalSnapshotInputError),
    #[error("intent {component} payload is valid but not canonical")]
    NonCanonical { component: &'static str },
}

pub(crate) fn decode_intent_external_inputs(
    inputs: &IntentExternalInputs,
) -> Result<DecodedIntentExternalInputs, IntentHostInputError> {
    let parameters = if inputs.parameter_batch.is_empty() {
        ParameterBatch::default()
    } else {
        let json = std::str::from_utf8(&inputs.parameter_batch).map_err(|_| {
            IntentHostInputError::NonUtf8 {
                component: "parameter batch",
            }
        })?;
        let decoded = ParameterBatch::from_json(json).map_err(IntentHostInputError::Parameter)?;
        let canonical = decoded
            .to_canonical_json()
            .map_err(IntentHostInputError::Parameter)?;
        if canonical.as_bytes() != inputs.parameter_batch {
            return Err(IntentHostInputError::NonCanonical {
                component: "parameter batch",
            });
        }
        decoded
    };

    let external_snapshots = if inputs.external_snapshots.is_empty() {
        ExternalSnapshotSet::default()
    } else {
        let json = std::str::from_utf8(&inputs.external_snapshots).map_err(|_| {
            IntentHostInputError::NonUtf8 {
                component: "external snapshot set",
            }
        })?;
        let decoded =
            ExternalSnapshotSet::from_json(json).map_err(IntentHostInputError::External)?;
        let canonical = decoded
            .to_canonical_json()
            .map_err(IntentHostInputError::External)?;
        if canonical.as_bytes() != inputs.external_snapshots {
            return Err(IntentHostInputError::NonCanonical {
                component: "external snapshot set",
            });
        }
        decoded
    };

    Ok(DecodedIntentExternalInputs {
        parameters,
        external_snapshots,
    })
}

#[cfg(test)]
mod tests {
    use geosolve_sketch::{ExternalSnapshotSet, ParameterBatch};
    use geosolve_sketch_intent::{ExternalInputRevision, IntentExternalInputs};

    use super::{IntentHostInputError, decode_intent_external_inputs};

    fn inputs(parameters: Vec<u8>, external: Vec<u8>) -> IntentExternalInputs {
        IntentExternalInputs::new(ExternalInputRevision::from_raw(7), parameters, external)
            .expect("bounded host inputs")
    }

    #[test]
    fn empty_payloads_are_the_exact_native_defaults() {
        let decoded = decode_intent_external_inputs(&inputs(Vec::new(), Vec::new()))
            .expect("default host inputs");
        assert_eq!(decoded.parameters, ParameterBatch::default());
        assert_eq!(decoded.external_snapshots, ExternalSnapshotSet::default());
    }

    #[test]
    fn canonical_native_payloads_round_trip_exactly() {
        let parameters = ParameterBatch::default()
            .to_canonical_json()
            .expect("parameter JSON")
            .into_bytes();
        let external = ExternalSnapshotSet::default()
            .to_canonical_json()
            .expect("external JSON")
            .into_bytes();
        let decoded = decode_intent_external_inputs(&inputs(parameters, external))
            .expect("canonical host inputs");
        assert_eq!(decoded.parameters, ParameterBatch::default());
        assert_eq!(decoded.external_snapshots, ExternalSnapshotSet::default());
    }

    #[test]
    fn noncanonical_or_non_utf8_payloads_fail_closed() {
        let mut parameter = ParameterBatch::default()
            .to_canonical_json()
            .expect("parameter JSON")
            .into_bytes();
        parameter.push(b' ');
        assert!(matches!(
            decode_intent_external_inputs(&inputs(parameter, Vec::new())),
            Err(IntentHostInputError::NonCanonical {
                component: "parameter batch"
            })
        ));

        assert!(matches!(
            decode_intent_external_inputs(&inputs(Vec::new(), vec![0xff])),
            Err(IntentHostInputError::NonUtf8 {
                component: "external snapshot set"
            })
        ));
    }
}
