// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Write as _;

use geosolve_constraint_editor::RetainedEditorCoordinator;

#[cfg(test)]
pub(crate) fn serialize_typed_host_evidence(
    coordinator: &RetainedEditorCoordinator,
    captured_unix_ms: &str,
    location: &str,
    user_agent: &str,
    host_state_evidence: &str,
) -> Result<String, String> {
    serialize_typed_host_evidence_with_workspace_detail(
        coordinator,
        captured_unix_ms,
        location,
        user_agent,
        host_state_evidence,
        WorkspaceDetail::CanonicalDocuments,
    )
}

pub(crate) fn serialize_m52_typed_host_evidence(
    coordinator: &RetainedEditorCoordinator,
    captured_unix_ms: &str,
    location: &str,
    user_agent: &str,
    host_state_evidence: &str,
) -> Result<String, String> {
    serialize_typed_host_evidence_with_workspace_detail(
        coordinator,
        captured_unix_ms,
        location,
        user_agent,
        host_state_evidence,
        WorkspaceDetail::IdentitiesOnly,
    )
}

#[derive(Clone, Copy)]
enum WorkspaceDetail {
    #[cfg(test)]
    CanonicalDocuments,
    IdentitiesOnly,
}

fn serialize_typed_host_evidence_with_workspace_detail(
    coordinator: &RetainedEditorCoordinator,
    captured_unix_ms: &str,
    location: &str,
    user_agent: &str,
    host_state_evidence: &str,
    workspace_detail: WorkspaceDetail,
) -> Result<String, String> {
    let batch = coordinator.session().parameter_batch();
    let entries = batch
        .entries()
        .iter()
        .map(|entry| {
            format!(
                "{{\"parameter\":{},\"value\":{}}}",
                json_string(&entry.parameter.to_string()),
                parameter_value_json(entry.value)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let external_snapshot_set = coordinator
        .session()
        .external_snapshot_set()
        .to_canonical_json()
        .map_err(|error| error.to_string())?;
    let transcript = coordinator
        .transcript()
        .iter()
        .map(|value| json_string(&format!("{value:?}")))
        .collect::<Vec<_>>()
        .join(",");
    let audit = |audit: Option<geosolve_constraint_editor::AuditDto<'_>>, absent: &str| {
        audit.map_or_else(
            || absent.to_owned(),
            |audit| {
                format!(
                    "provenance={:?}\ndesign={:?}\n{:#?}",
                    audit.provenance, audit.design, audit.solve_result
                )
            },
        )
    };
    let accepted_audit = audit(coordinator.accepted_audit(), "No accepted audit");
    let attempted_audit = audit(coordinator.attempt_audit(), "No attempted audit");
    let workspace = workspace_json(coordinator, workspace_detail);
    let payload = format!(
        concat!(
            "{{\"candidate\":{},\"captured_unix_ms\":{},\"location\":{},\"user_agent\":{},",
            "\"workspace\":{},\"parameter_batch\":{{\"revision\":{},\"digest\":{},\"entries\":[{}]}},",
            "\"external_snapshot_set\":{},\"lifecycle\":{},\"transcript\":[{}],",
            "\"accepted_audit\":{},\"attempted_audit\":{},\"host_state_evidence\":{}}}"
        ),
        json_string(concat!("geosolve-demo-web/", env!("CARGO_PKG_VERSION"))),
        json_string(captured_unix_ms),
        json_string(location),
        json_string(user_agent),
        workspace,
        batch.revision(),
        json_string(&hex_digest(batch.digest().bytes())),
        entries,
        external_snapshot_set,
        json_string(&format!("{:?}", coordinator.lifecycle())),
        transcript,
        json_string(&accepted_audit),
        json_string(&attempted_audit),
        json_string(host_state_evidence),
    );
    Ok(format!(
        "{{\"format\":\"geosolve-typed-host-finding-v1\",\"checksum\":{},\"payload\":{payload}}}",
        json_string(&checksum(payload.as_bytes()))
    ))
}

fn workspace_json(coordinator: &RetainedEditorCoordinator, detail: WorkspaceDetail) -> String {
    let checkpoint = coordinator.checkpoint();
    let revisions = checkpoint.revisions();
    let accepted_revision = revisions
        .accepted()
        .map_or_else(|| "null".to_owned(), |revision| revision.get().to_string());
    match detail {
        #[cfg(test)]
        WorkspaceDetail::CanonicalDocuments => format!(
            "{{\"design_json\":{},\"accepted_json\":{},\"design_revision\":{},\"attempt_revision\":{},\"accepted_revision\":{}}}",
            json_string(checkpoint.design_json()),
            checkpoint
                .accepted_json()
                .map_or_else(|| "null".to_owned(), json_string),
            revisions.design().get(),
            revisions.attempt().get(),
            accepted_revision,
        ),
        WorkspaceDetail::IdentitiesOnly => {
            let design = coordinator.session().design_identity();
            let design_identity = format!("{}@{}", design.document(), design.revision().get());
            let accepted_identity = coordinator.session().accepted_state().map_or_else(
                || "null".to_owned(),
                |accepted| {
                    let identity = accepted.identity();
                    json_string(&format!(
                        "{}@{}",
                        identity.document(),
                        identity.revision().get()
                    ))
                },
            );
            format!(
                "{{\"design_identity\":{},\"accepted_identity\":{},\"design_revision\":{},\"attempt_revision\":{},\"accepted_revision\":{}}}",
                json_string(&design_identity),
                accepted_identity,
                revisions.design().get(),
                revisions.attempt().get(),
                accepted_revision,
            )
        }
    }
}

fn parameter_value_json(value: geosolve_sketch::ParameterValue) -> String {
    match value {
        geosolve_sketch::ParameterValue::Length(value) => {
            format!("{{\"kind\":\"length\",\"value\":{value}}}")
        }
        geosolve_sketch::ParameterValue::Angle(value) => {
            format!("{{\"kind\":\"angle\",\"value\":{value}}}")
        }
        geosolve_sketch::ParameterValue::Dimensionless(value) => {
            format!("{{\"kind\":\"dimensionless\",\"value\":{value}}}")
        }
        geosolve_sketch::ParameterValue::Activation(value) => {
            format!("{{\"kind\":\"activation\",\"value\":{value}}}")
        }
    }
}

fn json_string(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character <= '\u{1f}' => {
                let _ = write!(output, "\\u{:04x}", u32::from(character));
            }
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn hex_digest(bytes: [u8; 32]) -> String {
    bytes.iter().fold(String::new(), |mut output, byte| {
        let _ = write!(output, "{byte:02x}");
        output
    })
}

fn checksum(bytes: &[u8]) -> String {
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        value ^= u64::from(*byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64-{value:016x}")
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::RetainedEditorCoordinator;
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        DocumentConstraintDefinition, DocumentExternalPointRef, DocumentParameterKind,
        DocumentParameterTarget, DocumentSolveRequest, ExternalFeatureKindV1,
        ExternalSnapshotDigest, ExternalSnapshotEntry, ExternalSnapshotFeatureV1,
        ExternalSnapshotResourcesV1, ExternalSnapshotSet, ParameterBatch, ParameterBatchEntry,
        ParameterValue, RetainedSketchDocumentSession, SketchDocument,
    };

    use super::{checksum, serialize_typed_host_evidence};
    use crate::workbench::panels::host_state_markup;

    #[test]
    #[allow(clippy::too_many_lines)]
    fn typed_host_evidence_serializer_contains_inputs_attempt_and_accepted_evidence() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let rectangle = document
            .add_rectangle("captured", [0.0, 0.0], 4.0, 3.0)
            .unwrap();
        let parameter = document
            .add_parameter("captured width", DocumentParameterKind::Length)
            .unwrap();
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .unwrap();
        let external_point = document.add_point("external point", [0.0, 0.0]).unwrap();
        let binding = document
            .add_external_binding("origin", ExternalFeatureKindV1::Point, None)
            .unwrap();
        document
            .add_constraint(
                "external coincidence",
                DocumentConstraintDefinition::ExternalPointCoincident {
                    point: external_point,
                    external: DocumentExternalPointRef { binding },
                },
            )
            .unwrap();
        let parameter_batch = |revision, value| {
            ParameterBatch::new(revision, vec![ParameterBatchEntry { parameter, value }]).unwrap()
        };
        let snapshots = ExternalSnapshotSet::new(
            31,
            vec![ExternalSnapshotEntry {
                binding,
                source_revision: 29,
                source_digest: ExternalSnapshotDigest::from_bytes([0x5c; 32]),
                feature: ExternalSnapshotFeatureV1::Point {
                    position: [0.0, 0.0],
                    scale: 1.0,
                    resources: ExternalSnapshotResourcesV1 {
                        point_count: 1,
                        control_count: 0,
                        span_count: 0,
                    },
                },
            }],
        )
        .unwrap();
        let session = RetainedSketchDocumentSession::new_with_inputs(
            document,
            parameter_batch(21, ParameterValue::Length(4.0)),
            snapshots,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
        let expected = coordinator.session().design_identity();
        coordinator
            .replace_parameter_batch(
                expected,
                parameter_batch(22, ParameterValue::Angle(4.0)),
                DocumentSolveRequest::default(),
            )
            .unwrap();
        let host_evidence = host_state_markup(coordinator.session());
        let json = serialize_typed_host_evidence(
            &coordinator,
            "1785157200000",
            "https://example.invalid/",
            "typed-host-serializer-test",
            &host_evidence,
        )
        .unwrap();
        assert!(
            json.starts_with(
                "{\"format\":\"geosolve-typed-host-finding-v1\",\"checksum\":\"fnv1a64-"
            )
        );
        for expected in [
            "\"captured_unix_ms\":\"1785157200000\"",
            "\"location\":\"https://example.invalid/\"",
            "\"parameter_batch\":{\"revision\":22",
            "\"value\":{\"kind\":\"angle\",\"value\":4}",
            "\"revision\":31",
            "\"kind\":\"point\"",
            "\"lifecycle\":\"LifecycleDto { status: RejectedAttempt",
            "\"accepted_audit\":\"provenance=",
            "\"attempted_audit\":\"No attempted audit\"",
        ] {
            assert!(json.contains(expected), "missing typed evidence {expected}");
        }
        let revisions = coordinator.checkpoint().revisions();
        assert!(json.contains(&format!(
            "\"accepted_revision\":{}",
            revisions.accepted().unwrap().get()
        )));
        assert!(json.contains(&format!(
            "\"attempt_revision\":{}",
            revisions.attempt().get()
        )));
        assert!(json.contains("\"transcript\":[]"));
        assert!(json.contains("\"host_state_evidence\":\"<section"));
        assert!(json.contains(&super::json_string(&host_evidence)));
        let checksum_start = json.find("\"checksum\":\"").unwrap() + "\"checksum\":\"".len();
        let checksum_end = json[checksum_start..].find('"').unwrap() + checksum_start;
        let payload_start = json.find("\"payload\":").unwrap() + "\"payload\":".len();
        let payload = &json[payload_start..json.len() - 1];
        assert_eq!(
            &json[checksum_start..checksum_end],
            checksum(payload.as_bytes())
        );
    }
}
