// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::Viewport;
use geosolve_sketch::{CurveDefinition, DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, CodeSessionIdentity, CompiledManagedSource, KeyedReconcileState,
    ManagedControlEdit, ManagedControlEditBatch, ManagedMutationAuthority, ManagedPathSegment,
    ManagedSketchMutation, ManagedValue, SampleCategory, UnitLiteral, bundled_sample,
    managed_control_authority, materialize_code_project_cold, prepare_managed_mutation,
    required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;

struct ExpectedSample {
    ordinal: usize,
    key: &'static str,
    category: SampleCategory,
    raw_dof: usize,
    effective_dof: usize,
    minimum_scene_curves: usize,
    exact_circles: Option<usize>,
    exact_computed_curves: usize,
    required_families: &'static [&'static str],
}

const SAMPLES: [ExpectedSample; 4] = [
    ExpectedSample {
        ordinal: 17,
        key: "curves-contact-continuity-atlas",
        category: SampleCategory::ReferenceLab,
        raw_dof: 81,
        effective_dof: 81,
        minimum_scene_curves: 16,
        exact_circles: None,
        exact_computed_curves: 0,
        required_families: &[],
    },
    ExpectedSample {
        ordinal: 18,
        key: "fabrication-operations-atlas",
        category: SampleCategory::ReferenceLab,
        raw_dof: 255,
        effective_dof: 255,
        minimum_scene_curves: 84,
        exact_circles: None,
        exact_computed_curves: 0,
        required_families: &[
            "operation.rectangle",
            "operation.regularPolygon",
            "operation.slot",
            "operation.split",
            "operation.break",
            "operation.trim",
            "operation.extend",
            "operation.mirror",
            "operation.chamfer",
            "operation.associativeFillet",
            "operation.profileOffset",
            "operation.linearPattern",
            "constraint.fixedPoint",
            "constraint.horizontal",
            "constraint.vertical",
            "constraint.coincident",
            "constraint.equalLength",
            "constraint.midpoint",
            "constraint.horizontalPoints",
            "constraint.equalRadius",
            "dimension.curveLength",
            "dimension.pointDistance",
            "dimension.orientedAngle",
            "dimension.diameter",
            "dimension.radius",
        ],
    },
    ExpectedSample {
        ordinal: 19,
        key: "perforated-fixture-field",
        category: SampleCategory::ScaleStudy,
        raw_dof: 772,
        effective_dof: 772,
        minimum_scene_curves: 577,
        exact_circles: Some(384),
        exact_computed_curves: 0,
        required_families: &[],
    },
    ExpectedSample {
        ordinal: 20,
        key: "robotic-harness-backplane",
        category: SampleCategory::ScaleStudy,
        raw_dof: 268,
        effective_dof: 268,
        minimum_scene_curves: 164,
        exact_circles: Some(88),
        exact_computed_curves: 64,
        required_families: &[],
    },
];

fn witness_path(value: &serde_json::Value) -> Vec<ManagedPathSegment> {
    value
        .as_array()
        .expect("representative edit path")
        .iter()
        .map(|segment| match segment {
            serde_json::Value::String(field) => ManagedPathSegment::Field(field.clone()),
            serde_json::Value::Number(index) => ManagedPathSegment::Index(
                usize::try_from(index.as_u64().expect("non-negative edit-path index"))
                    .expect("edit-path index fits usize"),
            ),
            _ => panic!("representative edit path has an unsupported segment"),
        })
        .collect()
}

fn witness_value(value: &serde_json::Value) -> ManagedValue {
    match value {
        serde_json::Value::Null => ManagedValue::Null,
        serde_json::Value::Bool(value) => ManagedValue::Bool(*value),
        serde_json::Value::Number(value) => {
            ManagedValue::Number(value.as_f64().expect("finite witness number"))
        }
        serde_json::Value::String(value) => ManagedValue::String(value.clone()),
        serde_json::Value::Array(values) => {
            ManagedValue::Array(values.iter().map(witness_value).collect())
        }
        serde_json::Value::Object(value)
            if value.len() == 2 && value.contains_key("unit") && value.contains_key("value") =>
        {
            ManagedValue::Unit(UnitLiteral {
                unit: value["unit"].as_str().expect("witness unit").to_owned(),
                value: value["value"].as_f64().expect("witness unit value"),
            })
        }
        serde_json::Value::Object(value) => ManagedValue::Object(
            value
                .iter()
                .map(|(key, value)| (key.clone(), witness_value(value)))
                .collect(),
        ),
    }
}

fn staged_reconciliation(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).expect("generated member inventory"),
            &BTreeSet::new(),
        )
        .expect("generated member reconciliation")
        .into_staged()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table-driven pass keeps each atlas/scale sample's source, native scene, mobility, and edit authority adjacent"
)]
fn atlas_and_scale_samples_are_complete_native_scene_authorities() {
    for (index, expected) in SAMPLES.iter().enumerate() {
        let sample = bundled_sample(expected.key).expect("canonical sample key");
        assert_eq!(sample.ordinal, expected.ordinal, "{} ordinal", expected.key);
        assert_eq!(
            sample.category, expected.category,
            "{} category",
            expected.key
        );
        assert_eq!(
            sample.expected.numerical_right_nullity(),
            expected.raw_dof,
            "{} manifest right nullity",
            expected.key
        );
        assert_eq!(
            sample.expected.bidirectional_bounded_degrees_of_freedom(),
            expected.effective_dof,
            "{} manifest bounded DOF",
            expected.key
        );
        assert!(
            !sample.managed_source().contains("$.lod") && !sample.managed_source().contains("lod:"),
            "{} must not encode a presentation LOD shortcut",
            expected.key
        );

        let compiled = CompiledManagedSource::from_json(sample.compiled_source())
            .unwrap_or_else(|error| panic!("{} compiler envelope: {error}", expected.key));
        compiled
            .validate_input_source(sample.managed_source())
            .unwrap_or_else(|error| panic!("{} source authority: {error}", expected.key));
        assert_eq!(
            compiled.normalized_source,
            sample.managed_source(),
            "{}",
            expected.key
        );
        assert_eq!(
            compiled
                .artifact
                .groups
                .iter()
                .map(|group| group.name.as_str())
                .collect::<Vec<_>>(),
            sample.functional_groups,
            "{} ordered functional groups",
            expected.key
        );

        let declarations = compiled
            .artifact
            .declarations
            .iter()
            .map(|declaration| declaration.declaration.as_str())
            .collect::<BTreeSet<_>>();
        let families = compiled
            .artifact
            .declarations
            .iter()
            .map(|declaration| declaration.family.as_str())
            .collect::<BTreeSet<_>>();
        for family in expected.required_families {
            assert!(
                families.contains(family),
                "{} is missing required atlas family {family}",
                expected.key
            );
        }
        if expected.key == "fabrication-operations-atlas" {
            let declaration_families = compiled
                .artifact
                .declarations
                .iter()
                .map(|declaration| {
                    (
                        declaration.declaration.as_str(),
                        declaration.family.as_str(),
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let annotations = compiled
                .artifact
                .groups
                .iter()
                .find(|group| group.name == "Fabrication annotations")
                .expect("explicit fabrication annotation group");
            assert_eq!(annotations.declarations.len(), 2);
            assert!(annotations.declarations.iter().all(|member| {
                declaration_families
                    .get(member.declaration.as_str())
                    .is_some_and(|family| family.starts_with("dimension."))
            }));
        }
        let mut grouped = BTreeMap::<&str, usize>::new();
        for group in &compiled.artifact.groups {
            assert!(
                !group.declarations.is_empty(),
                "{} empty group",
                expected.key
            );
            for member in &group.declarations {
                assert!(
                    member.path.is_empty(),
                    "{} non-flat group member",
                    expected.key
                );
                assert!(
                    declarations.contains(member.declaration.as_str()),
                    "{} unknown group member {}",
                    expected.key,
                    member.declaration
                );
                *grouped.entry(member.declaration.as_str()).or_default() += 1;
            }
        }
        assert_eq!(
            grouped.len(),
            declarations.len(),
            "{} grouped roots",
            expected.key
        );
        assert!(
            grouped.values().all(|occurrences| *occurrences == 1),
            "{} declarations must belong to exactly one group",
            expected.key
        );

        let project = sample.project();
        let project_json = project.to_canonical_json().expect("canonical project");
        assert_eq!(
            CodeProject::from_json(&project_json).expect("project round-trip"),
            project,
            "{} project round-trip",
            expected.key
        );
        let seed = 0x92_17_u128 + index as u128;
        let materialized = materialize_code_project_cold(
            &project,
            &staged_reconciliation(&project),
            IntentSessionId::from_raw(seed),
            DocumentId(PersistentId::from_u128(seed)),
            1.0,
        )
        .unwrap_or_else(|error| panic!("{} cold materialization: {error}", expected.key));
        let accepted = materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .expect("accepted native authority");
        assert!(
            accepted.validation.hard_residuals_validated,
            "{}",
            expected.key
        );
        assert!(
            accepted.validation.all_active_features_current,
            "{}",
            expected.key
        );
        assert!(
            accepted
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9),
            "{} normalized hard residual",
            expected.key
        );
        let state = accepted
            .session
            .accepted_state_for_current_input()
            .expect("accepted solve state");
        assert!(
            state
                .document()
                .points()
                .iter()
                .flat_map(|point| point.position)
                .all(f64::is_finite),
            "{} finite points",
            expected.key
        );
        assert!(
            state
                .document()
                .scalars()
                .iter()
                .all(|scalar| scalar.value.is_finite()),
            "{} finite scalars",
            expected.key
        );
        assert!(
            state
                .document()
                .curves()
                .iter()
                .all(|curve| match &curve.definition {
                    CurveDefinition::Line {
                        branch_direction, ..
                    } => branch_direction
                        .iter()
                        .all(|component| component.is_finite()),
                    CurveDefinition::Polyline {
                        points,
                        closed,
                        branch_directions,
                    } => {
                        let spans = points.len() - usize::from(!*closed);
                        branch_directions.len() == spans
                            && branch_directions
                                .iter()
                                .flatten()
                                .all(|component| component.is_finite())
                    }
                    _ => true,
                }),
            "{} explicit finite curve branches",
            expected.key
        );
        assert!(
            state.document().contacts().iter().all(|contact| {
                state
                    .document()
                    .scalar(contact.parameter)
                    .is_some_and(|parameter| parameter.value.is_finite())
            }),
            "{} explicit finite contact state",
            expected.key
        );
        let diagnostics = state.diagnostics();
        assert_eq!(
            diagnostics
                .rank
                .expect("rank diagnostics")
                .numerical_right_nullity,
            Some(expected.raw_dof),
            "{} numerical right nullity",
            expected.key
        );
        let mobility = diagnostics.mobility.expect("mobility diagnostics");
        assert_eq!(
            mobility.equality_degrees_of_freedom,
            Some(expected.raw_dof),
            "{} equality DOF",
            expected.key
        );
        assert_eq!(
            mobility.bidirectional_bounded_degrees_of_freedom,
            Some(expected.effective_dof),
            "{} bounded DOF",
            expected.key
        );
        if let Some(circle_count) = expected.exact_circles {
            assert_eq!(
                state
                    .document()
                    .curves()
                    .iter()
                    .filter(|curve| matches!(&curve.definition, CurveDefinition::Circle { .. }))
                    .count(),
                circle_count,
                "{} complete native Circle inventory",
                expected.key
            );
        }

        let scene = materialized
            .editor
            .scene(
                Viewport::new([1_000.0, 700.0], [0.0, 0.0], 1.0)
                    .expect("finite scale-study viewport"),
                0.25,
            )
            .unwrap_or_else(|error| panic!("{} complete EditorScene: {error}", expected.key));
        assert!(
            scene.curves.len() >= expected.minimum_scene_curves,
            "{} complete native scene has {} curves, expected at least {}",
            expected.key,
            scene.curves.len(),
            expected.minimum_scene_curves
        );
        assert_eq!(
            scene.computed_curves.len(),
            expected.exact_computed_curves,
            "{} complete computed scene",
            expected.key
        );

        let witnesses: serde_json::Value =
            serde_json::from_str(sample.witnesses_json()).expect("witness JSON");
        let edit = &witnesses["representative_edit"];
        let declaration = edit["declaration"]
            .as_str()
            .expect("representative edit declaration");
        let path = witness_path(&edit["path"]);
        let replacement = witness_value(&edit["replacement"]);
        let controls = managed_control_authority(&project, &materialized.expansion)
            .unwrap_or_else(|error| panic!("{} managed controls: {error}", expected.key));
        let control = controls
            .manifest()
            .editable()
            .find(|control| {
                control.source.declaration.0 == declaration && control.source.path.0 == path
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} representative edit {}.{path:?} is not editable",
                    expected.key, declaration
                )
            });
        assert_ne!(
            control.value, replacement,
            "{} edit must change value",
            expected.key
        );
        let mutation = controls
            .prepare_mutation(&ManagedControlEditBatch::new([ManagedControlEdit {
                token: control.token().expect("editable control token").clone(),
                value: replacement,
            }]))
            .unwrap_or_else(|error| panic!("{} representative edit: {error}", expected.key));
        assert!(
            matches!(mutation, ManagedSketchMutation::SetValues { ref values } if values.len() == 1),
            "{} representative edit is one exact source mutation",
            expected.key
        );
        let authority = ManagedMutationAuthority::new(
            project.project.clone(),
            CodeSessionIdentity {
                session: 1,
                revision: 0,
                digest: "0".repeat(64),
            },
            materialized.expansion.digest.clone(),
            project.managed.declaration_name_high_water,
            project
                .managed
                .compiled
                .as_deref()
                .expect("compiled managed authority"),
        )
        .expect("accepted mutation authority");
        let request = prepare_managed_mutation(
            &authority,
            project
                .managed
                .compiled
                .as_deref()
                .expect("compiled managed authority"),
            mutation,
            project.managed.declaration_name_high_water,
        )
        .unwrap_or_else(|error| panic!("{} prepared compiler request: {error}", expected.key));
        assert_eq!(request.ticket.project, project.project, "{}", expected.key);
    }
}
