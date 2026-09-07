// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_constraint_editor::{ProjectionalEditorSession, Viewport};
use geosolve_sketch::{CurveDefinition, DocumentId, GeometryRole, PersistentId};
use geosolve_sketch_code::{
    CodeCompositionError, CodeHostRequest, CodeProject, KeyedReconcileState, ManagedValue,
    MaterializedCodeProject, PatchModuleArtifact, TemplateArgument, UnitLiteral,
    materialize_code_project_cold, rehydrate_materialized_code_project, required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle regression keeps source, accepted geometry, and serialized native restore evidence together"
)]
fn qualify(project_json: &str, expected_fillets: usize, expected_caps: usize, id: u128) {
    let project = CodeProject::from_json(project_json).expect("authenticated channel project");
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(id),
        DocumentId(PersistentId::from_u128(id)),
        1.0,
    )
    .expect("finite channel materializes through public native owner");
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| value.is_finite() && value <= 1e-9)
    );
    let document = accepted.session.design_document();
    assert!(
        document
            .points()
            .iter()
            .flat_map(|point| point.position)
            .all(f64::is_finite)
    );
    assert!(
        document
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    // Caps are genuine circular arcs with radius half the declared 6 mm width.
    let radii = document
        .curves()
        .iter()
        .filter_map(|curve| match curve.definition {
            CurveDefinition::CircularArc { radius, .. } => {
                Some(document.scalar(radius).unwrap().value)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(radii.len(), expected_caps);
    assert!(radii.iter().all(|radius| (radius - 3.0).abs() <= 1e-9));
    let view = Viewport::new([600.0, 400.0], [0.0, 0.0], 1.0).unwrap();
    let scene = materialized.editor.scene(view, 0.25).unwrap();
    assert_eq!(scene.computed_curves.len(), expected_fillets);
    for radius in [2.0, 8.0] {
        assert_eq!(
            scene
                .computed_curves
                .iter()
                .filter(|curve| (curve.radius - radius).abs() <= 1e-9)
                .count(),
            expected_fillets / 2
        );
    }
    // The source and both walls retain native finite line geometry. Computed
    // corner replacements must not turn any parent into a degenerate line.
    assert!(
        document
            .curves()
            .iter()
            .filter(|curve| document.geometry_role(curve.id) == Some(GeometryRole::Profile))
            .all(|curve| match curve.definition {
                CurveDefinition::Line { start, end, .. } => {
                    let a = document.point(start).unwrap().position;
                    let b = document.point(end).unwrap().position;
                    (b[0] - a[0]).hypot(b[1] - a[1]) > 1e-9
                }
                _ => true,
            })
    );
    let document_id = document.id();
    let native = materialized
        .editor
        .coordinator()
        .intent()
        .to_canonical_json()
        .unwrap();
    let restored_native = IntentSession::from_json(&native).unwrap();
    let restored_editor = ProjectionalEditorSession::restore(restored_native, document_id, 1.0)
        .expect("serialized native checkpoint independently cold restores");
    let restored =
        rehydrate_materialized_code_project(Box::new(restored_editor), materialized.expansion)
            .expect("generated channel ownership reauthenticates after real native restore");
    assert_eq!(
        restored
            .editor
            .coordinator()
            .intent()
            .to_canonical_json()
            .unwrap(),
        native
    );
    let restored_scene = restored.editor.scene(view, 0.25).unwrap();
    assert_eq!(restored_scene.computed_curves, scene.computed_curves);
    assert_eq!(restored_scene.curves, scene.curves);
}

#[test]
fn open_channel_rounds_both_turn_directions_and_restores_rounded_caps() {
    qualify(
        include_str!("fixtures/m96/channel-zigzag.project.json"),
        4,
        2,
        0x96_1100,
    );
}

#[test]
fn closed_silicone_channel_has_two_rounded_boundaries_and_restores() {
    qualify(
        include_str!("fixtures/m96/channel-loop.project.json"),
        8,
        0,
        0x96_1101,
    );
}

fn channel_project_with_units(width: (&str, f64), radius: (&str, f64)) -> CodeProject {
    let mut project =
        CodeProject::from_json(include_str!("fixtures/m96/channel-zigzag.project.json")).unwrap();
    let mut artifact: PatchModuleArtifact =
        serde_json::from_value(project.artifacts.values().next().unwrap().clone()).unwrap();
    let TemplateArgument::Object(arguments) = &mut artifact.templates[0].arguments else {
        panic!("channel template has named arguments");
    };
    for (name, (unit, value)) in [("width", width), ("bendRadius", radius)] {
        arguments.insert(
            name.into(),
            TemplateArgument::Literal(ManagedValue::Unit(UnitLiteral {
                unit: unit.into(),
                value,
            })),
        );
    }
    // Handcrafted structural artifacts exercise the Rust owner directly. Keep
    // their content pins authentic without editing managed source authority.
    let validated = artifact.validate().unwrap();
    let digest = validated.digest().to_owned();
    project.artifacts.clear();
    project.artifacts.insert(
        digest.clone(),
        serde_json::from_str(validated.canonical_json()).unwrap(),
    );
    project.lock["modules"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["artifact"] = serde_json::Value::String(digest);
    project
}

fn materialize_unit_channel(
    project: &CodeProject,
) -> Result<MaterializedCodeProject, CodeCompositionError> {
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    materialize_code_project_cold(
        project,
        &generated,
        IntentSessionId::from_raw(0x96_1103),
        DocumentId(PersistentId::from_u128(0x96_1103)),
        1.0,
    )
}

#[test]
fn channel_length_units_produce_identical_native_geometry_and_host_requests() {
    let baseline = materialize_unit_channel(&channel_project_with_units(("mm", 6.0), ("mm", 5.0)))
        .expect("millimetre channel");
    let normalize_requests = |mut requests: Vec<CodeHostRequest>| {
        for request in &mut requests {
            if let CodeHostRequest::FilletAtCorner(request) = request {
                // Each unit spelling intentionally has a different artifact
                // pin; all actual geometry and explicit branch inputs agree.
                request.artifact_digest.clear();
            }
        }
        requests
    };
    let expected_requests = normalize_requests(baseline.expansion.host_requests.clone());
    for (unit, factor) in [("model", 1.0), ("cm", 10.0), ("m", 1_000.0), ("inch", 25.4)] {
        let project = channel_project_with_units((unit, 6.0 / factor), (unit, 5.0 / factor));
        let actual = materialize_unit_channel(&project)
            .unwrap_or_else(|error| panic!("equivalent {unit} channel: {error}"));
        assert_eq!(actual.expansion.patch, baseline.expansion.patch, "{unit}");
        assert_eq!(
            normalize_requests(actual.expansion.host_requests),
            expected_requests,
            "{unit}"
        );
    }
}

#[test]
fn channel_length_arguments_reject_angle_units() {
    for (name, width, radius) in [
        ("width", ("deg", 6.0), ("mm", 5.0)),
        ("bendRadius", ("mm", 6.0), ("rad", 5.0)),
    ] {
        let failure =
            materialize_unit_channel(&channel_project_with_units(width, radius)).unwrap_err();
        assert!(
            failure.to_string().contains(name)
                && failure.to_string().contains("incompatible with Length"),
            "invalid {name} must reject: {failure}"
        );
    }
}

#[test]
fn channel_artifact_missing_a_wall_output_rejects_without_panicking() {
    let mut project =
        CodeProject::from_json(include_str!("fixtures/m96/channel-loop.project.json")).unwrap();
    let value = project.artifacts.values().next().unwrap().clone();
    let mut artifact: geosolve_sketch_code::PatchModuleArtifact =
        serde_json::from_value(value).unwrap();
    artifact.templates[0].outputs.retain(|output| {
        output.path.0
            != vec![geosolve_sketch_code::ManagedPathSegment::Field(
                "left".into(),
            )]
    });
    let validated = artifact.validate().unwrap();
    let digest = validated.digest().to_owned();
    project.artifacts.clear();
    project.artifacts.insert(
        digest.clone(),
        serde_json::from_str(validated.canonical_json()).unwrap(),
    );
    project.lock["modules"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["artifact"] = serde_json::Value::String(digest);
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let failure = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x96_1102),
        DocumentId(PersistentId::from_u128(0x96_1102)),
        1.0,
    )
    .unwrap_err();
    assert!(
        failure
            .to_string()
            .contains("channel requires its fixed six typed outputs"),
        "{failure}"
    );
}
