// SPDX-License-Identifier: GPL-3.0-or-later
use super::*;
use geosolve_sketch_code::{CompiledManagedSource, ManagedMetadataTarget};

fn fixture_bridge(compiled: CompiledManagedSource) -> WorkbenchBridge {
    let (code, editor) =
        CodeProjectWorkbench::open_managed_test_compiled("metadata-standalone", compiled).unwrap();
    let authority = WorkbenchDocumentAuthority::from_projectional_editor(*editor).unwrap();
    let mut bridge = WorkbenchBridge::from_parts(
        authority,
        Some(code),
        crate::workbench::samples::SampleCatalogState::default(),
        "Source properties".into(),
        "Ready".into(),
    )
    .unwrap();
    bridge.refresh_current_scene();
    bridge
}

fn accepted_geometry(bridge: &WorkbenchBridge) -> serde_json::Value {
    let accepted = bridge
        .editor()
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1e-9)
    );
    let document = accepted
        .session
        .accepted_state_for_current_input()
        .unwrap()
        .document();
    assert!(
        document
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    serde_json::json!({"points": document.points(), "scalars": document.scalars(), "curves": document.curves()})
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one adapter transaction witness binds metadata, exact source history and independently validated geometry"
)]
fn m97_source_properties_publish_atomically_without_catalog_and_restore_exactly() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../geosolve-sketch-code/tests/fixtures/m97-authoring-lifecycle.json"
    )))
    .unwrap();
    let initial: CompiledManagedSource =
        serde_json::from_value(fixture["initial"].clone()).unwrap();
    let mut bridge = fixture_bridge(initial.clone());
    let baseline = accepted_geometry(&bridge);
    assert_eq!(
        bridge.authoring_document_snapshot().unwrap().title,
        "Properties"
    );
    let snapshot: serde_json::Value = serde_json::to_value(bridge.dimensions_snapshot()).unwrap();
    assert_eq!(snapshot["allMeasurements"].as_array().unwrap().len(), 2);
    assert_eq!(
        snapshot["entries"].as_array().unwrap().len(),
        1,
        "false overrides the document default"
    );
    let public: Vec<_> = bridge
        .parameter_snapshot()
        .into_iter()
        .filter(|row| {
            row.metadata
                .as_ref()
                .is_some_and(|metadata| metadata.editable)
        })
        .collect();
    assert_eq!(
        public
            .iter()
            .map(|row| row.label.as_str())
            .collect::<Vec<_>>(),
        ["Shared width", "spareWidth"]
    );
    assert_ne!(
        public[0].id, public[1].id,
        "equal values retain separate controls"
    );
    assert_eq!(public[0].consumers, ["Length"]);
    assert!(public[1].consumers.is_empty());

    for step in fixture["steps"].as_array().unwrap() {
        let before = bridge
            .code_project
            .as_ref()
            .unwrap()
            .to_persistence_json()
            .unwrap();
        let source = bridge
            .code_project
            .as_ref()
            .unwrap()
            .managed_source()
            .to_owned();
        let authority = bridge.authoring_metadata_authority();
        let mutation: ManagedSketchMutation =
            serde_json::from_value(step["mutation"].clone()).unwrap();
        match mutation {
            ManagedSketchMutation::SetMetadata {
                target,
                property,
                value,
            } => {
                let target = match target {
                    ManagedMetadataTarget::Document => MetadataTarget::Document,
                    ManagedMetadataTarget::Declaration { declaration } => {
                        MetadataTarget::Dimension { id: declaration }
                    }
                    ManagedMetadataTarget::Parameter { declaration } => {
                        let manifest = bridge
                            .code_project
                            .as_ref()
                            .unwrap()
                            .managed_controls_cached()
                            .unwrap();
                        let id = manifest
                            .controls
                            .iter()
                            .find(|control| control.source.declaration.0 == declaration)
                            .unwrap()
                            .id
                            .0
                            .clone();
                        MetadataTarget::Parameter { id }
                    }
                };
                let value = match value {
                    None => serde_json::Value::Null,
                    Some(ManagedValue::String(value)) => value.into(),
                    Some(ManagedValue::Bool(value)) => value.into(),
                    _ => panic!("metadata scalar"),
                };
                bridge.dispatch_authoring_metadata("authoring.metadata.set", serde_json::json!({"authority":authority,"target":target,"changes":{property:value}})).unwrap();
            }
            ManagedSketchMutation::ExtractParameter {
                declaration,
                presentation,
                ..
            } => {
                let manifest = bridge
                    .code_project
                    .as_ref()
                    .unwrap()
                    .managed_controls_cached()
                    .unwrap();
                let control = manifest
                    .controls
                    .iter()
                    .find(|control| {
                        control.source.declaration.0 == declaration
                            && control.source.path.0.is_empty()
                    })
                    .unwrap();
                bridge.dispatch_authoring_metadata("authoring.parameter.extract", serde_json::json!({"authority":authority,"id":control.id.0,"label":presentation.label,"isKeyParameter":presentation.is_key_parameter})).unwrap();
            }
            _ => panic!("metadata fixture"),
        }
        assert_eq!(
            bridge
                .code_project
                .as_ref()
                .unwrap()
                .to_persistence_json()
                .unwrap(),
            before,
            "preparation cannot publish source or history"
        );
        assert_eq!(accepted_geometry(&bridge), baseline);
        assert!(!bridge.authoring_document_snapshot().unwrap().editable);
        let PendingManagedMutation::Managed { prepared, .. } =
            bridge.pending_managed_mutation.as_ref().unwrap()
        else {
            panic!("prepared metadata mutation")
        };
        let compiled: CompiledManagedSource =
            serde_json::from_value(step["compiled"].clone()).unwrap();
        let receipt = PreparedManagedMutationReceipt {
            ticket_digest: prepared.request.ticket.ticket_digest.clone(),
            base_source_digest: prepared.request.current.ir.source_digest.clone(),
            candidate_source_digest: compiled.ir.source_digest.clone(),
            compiled: compiled.clone(),
        };
        bridge.resolve_pending_managed_mutation(receipt).unwrap();
        assert!(
            bridge.pending_managed_mutation.is_none(),
            "mutation {}: {:?}",
            step["mutation"],
            bridge.last_error
        );
        assert!(
            bridge.last_error.is_none(),
            "mutation {}: {:?}",
            step["mutation"],
            bridge.last_error
        );
        bridge.refresh_current_scene();
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            compiled.normalized_source
        );
        assert_eq!(accepted_geometry(&bridge), baseline);
        assert!(bridge.dispatch_authoring_metadata("authoring.metadata.set", serde_json::json!({"authority":authority,"target":{"kind":"document"},"changes":{"title":"Stale"}})).is_err());
        bridge.step_history(true).unwrap();
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            source
        );
        assert_eq!(accepted_geometry(&bridge), baseline);
        bridge.step_history(false).unwrap();
        assert_eq!(
            bridge.code_project.as_ref().unwrap().managed_source(),
            compiled.normalized_source
        );
    }
    assert_eq!(
        bridge.code_project.as_ref().unwrap().title(),
        "Named design"
    );
    bridge.refresh_current_scene();
    let snapshot = serde_json::to_value(bridge.dimensions_snapshot()).unwrap();
    assert!(
        snapshot["allMeasurements"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["label"] == "Main span"),
        "dimension projection follows the accepted authored label after a rename"
    );
    let persisted: serde_json::Value =
        serde_json::from_str(&bridge.persistence_json().unwrap()).unwrap();
    let restored = WorkbenchBridge::construct_json(
        &serde_json::json!({"version":2,"persistedProject":persisted["contents"]}).to_string(),
    )
    .unwrap();
    assert_eq!(
        restored.code_project.as_ref().unwrap().managed_source(),
        bridge.code_project.as_ref().unwrap().managed_source()
    );
    assert_eq!(
        restored.authoring_document_snapshot().unwrap().title,
        "Named design"
    );
    assert_eq!(accepted_geometry(&restored), baseline);
    assert!(restored.parameter_snapshot().iter().any(|row| {
        row.label == "Radius"
            && row
                .metadata
                .as_ref()
                .is_some_and(|metadata| metadata.is_key_parameter == Some(true))
    }));
}

#[test]
fn m97_metadata_rejects_invalid_targets_flags_and_unapplied_source() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../geosolve-sketch-code/tests/fixtures/m97-authoring-lifecycle.json"
    )))
    .unwrap();
    let mut bridge = fixture_bridge(serde_json::from_value(fixture["initial"].clone()).unwrap());
    let before = bridge
        .code_project
        .as_ref()
        .unwrap()
        .to_persistence_json()
        .unwrap();
    let authority = bridge.authoring_metadata_authority();
    for (target, changes) in [
        (
            serde_json::json!({"kind":"dimension","id":"missing"}),
            serde_json::json!({"isKeyConstraint":true}),
        ),
        (
            serde_json::json!({"kind":"dimension","id":"edge"}),
            serde_json::json!({"isKeyConstraint":true}),
        ),
        (
            serde_json::json!({"kind":"dimension","id":"length"}),
            serde_json::json!({"key":true}),
        ),
        (
            serde_json::json!({"kind":"document"}),
            serde_json::json!({"title":12}),
        ),
    ] {
        assert!(
            bridge
                .dispatch_authoring_metadata(
                    "authoring.metadata.set",
                    serde_json::json!({"authority":authority,"target":target,"changes":changes})
                )
                .is_err()
        );
        assert!(bridge.pending_managed_mutation.is_none());
        assert_eq!(
            bridge
                .code_project
                .as_ref()
                .unwrap()
                .to_persistence_json()
                .unwrap(),
            before
        );
    }
    bridge
        .code_project
        .as_mut()
        .unwrap()
        .set_managed_draft("invalid source".into());
    assert!(!bridge.authoring_document_snapshot().unwrap().editable);
    assert!(bridge.dispatch_authoring_metadata("authoring.metadata.set", serde_json::json!({"authority":authority,"target":{"kind":"document"},"changes":{"title":"Must not replace draft"}})).is_err());
    assert!(bridge.pending_managed_mutation.is_none());
}
