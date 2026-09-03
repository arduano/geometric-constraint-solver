// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_sketch_code::{
    CodeProject, CodeProjectDemoId, ExpandedCodeProject, KeyedReconcileState,
    MANAGED_CONTROL_LIMIT, ManagedControlAccess, ManagedControlEdit, ManagedControlEditBatch,
    ManagedControlError, ManagedControlSchema, ManagedSketchMutation, ManagedValue, ProjectKey,
    UnitLiteral, bundled_code_project_demos, expand_code_project, managed_control_manifest,
    prepare_managed_control_mutation, required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

fn project(id: CodeProjectDemoId) -> CodeProject {
    bundled_code_project_demos()
        .into_iter()
        .find(|demo| demo.id == id)
        .expect("bundled project")
        .project()
}

fn expansion(project: &CodeProject, seed: u128) -> ExpandedCodeProject {
    let desired = required_generated_members(project).expect("generated inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("generated reconciliation")
        .into_staged();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(seed)).expect("intent session");
    expand_code_project(project, &generated, intent.identity()).expect("project expansion")
}

fn path_text(control: &geosolve_sketch_code::ManagedControl) -> String {
    control
        .source
        .path
        .0
        .iter()
        .map(|segment| match segment {
            geosolve_sketch_code::ManagedPathSegment::Field(value) => value.clone(),
            geosolve_sketch_code::ManagedPathSegment::Index(index) => format!("[{index}]"),
            geosolve_sketch_code::ManagedPathSegment::Member { member } => member.clone(),
        })
        .collect::<Vec<_>>()
        .join(".")
}

#[test]
fn bundled_v3_controls_are_runtime_derived_typed_and_source_authenticated() {
    for (ordinal, demo) in bundled_code_project_demos().into_iter().enumerate() {
        let project = demo.project();
        assert!(project.managed.compiled.is_some());
        let expansion = expansion(&project, 0x90_10 + ordinal as u128);
        let manifest = managed_control_manifest(&project, &expansion).expect("control manifest");
        assert_eq!(manifest.project, project.project);
        assert_eq!(manifest.source_digest, project.managed.source_digest);
        assert_eq!(manifest.expansion_digest, expansion.digest);
        assert!(!manifest.controls.is_empty(), "{}", demo.id.key());

        for control in &manifest.controls {
            assert!(control.source.span.start < control.source.span.end);
            assert_eq!(
                project
                    .managed
                    .source
                    .get(control.source.span.start..control.source.span.end),
                Some(control.source.source_text.as_str()),
            );
            match &control.access {
                ManagedControlAccess::Editable { token } => {
                    assert!(control.schema.is_some());
                    assert_eq!(token.project, project.project);
                    assert_eq!(token.source_digest, project.managed.source_digest);
                    assert_eq!(token.expected, control.value);
                    assert!(!token.authentication.is_empty());
                }
                ManagedControlAccess::ReadOnly { .. } => assert!(control.schema.is_none()),
            }
        }
    }
}

#[test]
fn typed_panel_shared_radius_prepares_one_exact_runtime_provenance_mutation() {
    let project = project(CodeProjectDemoId::TypedPanel);
    let expansion = expansion(&project, 0x90_20);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let radius = manifest
        .editable()
        .find(|control| {
            control.source.declaration.0 == "cornerFillets" && path_text(control) == "radius"
        })
        .expect("shared Fillet radius");
    assert!(matches!(
        radius.schema,
        Some(ManagedControlSchema::Unit { ref unit, .. }) if unit == "mm"
    ));
    assert_eq!(radius.consumers.len(), 2);
    let mutation = prepare_managed_control_mutation(
        &project,
        &expansion,
        &ManagedControlEditBatch::new([ManagedControlEdit {
            token: radius.token().unwrap().clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: 3.0,
            }),
        }]),
    )
    .expect("authenticated semantic mutation");
    let ManagedSketchMutation::SetValues { values } = mutation else {
        panic!("controls must prepare one atomic SetValues mutation")
    };
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].declaration, "cornerFillets");
    assert_eq!(values[0].expected, radius.value);
    assert_eq!(
        values[0].value,
        ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 3.0,
        })
    );
}

#[test]
fn prepared_control_batches_reject_stale_duplicate_invalid_and_oversized_input_atomically() {
    let project = project(CodeProjectDemoId::TypedPanel);
    let expansion = expansion(&project, 0x90_30);
    let manifest = managed_control_manifest(&project, &expansion).unwrap();
    let control = manifest.editable().next().expect("editable control");
    let edit = ManagedControlEdit {
        token: control.token().unwrap().clone(),
        value: control.value.clone(),
    };

    assert!(matches!(
        prepare_managed_control_mutation(
            &project,
            &expansion,
            &ManagedControlEditBatch::new([edit.clone(), edit.clone()]),
        ),
        Err(ManagedControlError::DuplicateControl(_))
    ));

    let mut stale = edit.clone();
    stale.token.source_digest = "0".repeat(64);
    assert!(matches!(
        prepare_managed_control_mutation(
            &project,
            &expansion,
            &ManagedControlEditBatch::new([stale]),
        ),
        Err(ManagedControlError::StaleToken(_))
    ));

    let invalid = ManagedControlEdit {
        token: edit.token.clone(),
        value: ManagedValue::Number(f64::NAN),
    };
    assert!(matches!(
        prepare_managed_control_mutation(
            &project,
            &expansion,
            &ManagedControlEditBatch::new([invalid]),
        ),
        Err(ManagedControlError::InvalidReplacement { .. })
    ));

    let oversized = ManagedControlEditBatch::new(std::iter::repeat_n(
        edit,
        MANAGED_CONTROL_LIMIT.saturating_add(1),
    ));
    assert!(matches!(
        prepare_managed_control_mutation(&project, &expansion, &oversized),
        Err(ManagedControlError::ResourceLimit { .. })
    ));
}

#[test]
fn control_manifest_is_transient_and_compiled_project_wire_has_no_legacy_lenses() {
    let project = project(CodeProjectDemoId::CompassRose);
    let before = project.to_canonical_json().unwrap();
    let expansion = expansion(&project, 0x90_40);
    let _manifest = managed_control_manifest(&project, &expansion).unwrap();
    assert_eq!(project.to_canonical_json().unwrap(), before);
    assert!(!before.contains("editLens"));
    assert!(!before.contains("edit_lens"));
    assert!(!before.contains("managed-v1"));
    assert!(!before.contains("managed-v2"));
    assert_eq!(
        project.project,
        ProjectKey("geosolve-demo-compass-rose".into())
    );
}
