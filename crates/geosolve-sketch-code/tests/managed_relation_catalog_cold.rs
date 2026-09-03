// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CODE_AUTHORING_FAMILIES, CodeAuthoringAvailability, CodeAuthoringDeclarationKind, CodeProject,
    CompiledManagedSource, FeatureKind, KeyedReconcileState, ManagedStatement, ProjectKey,
    SemanticOutputPath, SemanticSymbol, materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{ConstraintKind, IntentNodeKind, IntentSessionId};

const RELATION_CATALOG_SOURCE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-relation-catalog.sketch.ts"
);
const RELATION_CATALOG_ENVELOPE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-relation-catalog.json"
);

fn standalone_constraints() -> BTreeSet<ConstraintKind> {
    ConstraintKind::ALL
        .into_iter()
        .filter(|kind| {
            !matches!(
                kind,
                ConstraintKind::ExternalPointCoincident | ConstraintKind::ExternalLineCollinear
            )
        })
        .collect()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one compiled cold gate keeps every standalone constraint together"
)]
fn compiled_v3_named_constraint_catalog_cold_materializes_all_standalone_variants() {
    let unchecked: CompiledManagedSource = serde_json::from_str(RELATION_CATALOG_ENVELOPE)
        .expect("named-relation compiler-envelope JSON");
    assert_eq!(
        serde_json::to_string(&unchecked.artifact).expect("Rust artifact JSON"),
        unchecked.canonical_artifact_json,
        "TypeScript and Rust must share the exact canonical executed artifact",
    );
    let compiled = CompiledManagedSource::from_json(RELATION_CATALOG_ENVELOPE)
        .expect("authenticated V3 named-relation compiler envelope");
    compiled
        .validate_input_source(RELATION_CATALOG_SOURCE)
        .expect("named-relation source digest");

    let standalone_constraints = standalone_constraints();
    assert_eq!(standalone_constraints.len(), 33);
    let expected_constraint_families = CODE_AUTHORING_FAMILIES
        .iter()
        .filter(|family| {
            matches!(
                family.declaration,
                CodeAuthoringDeclarationKind::Constraint(_)
            )
        })
        .filter(|family| family.availability == CodeAuthoringAvailability::Public)
        .map(|family| format!("{}.{}", family.namespace, family.method))
        .collect::<BTreeSet<_>>();
    assert_eq!(expected_constraint_families.len(), 33);

    let executed_constraint_families = compiled
        .artifact
        .declarations
        .iter()
        .filter(|declaration| declaration.family.starts_with("constraint."))
        .map(|declaration| declaration.family.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(executed_constraint_families, expected_constraint_families);

    let ir_declarations = compiled
        .ir
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Declaration { symbol, .. } => Some(symbol.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(ir_declarations.len(), compiled.artifact.declarations.len());
    assert!(compiled.artifact.declarations.iter().all(|declaration| {
        ir_declarations.contains(&declaration.declaration) && !declaration.result.is_empty()
    }));
    let executed_results = compiled
        .artifact
        .declarations
        .iter()
        .flat_map(|declaration| {
            declaration.result.iter().map(|leaf| {
                (
                    SemanticSymbol(declaration.declaration.clone()),
                    SemanticOutputPath(leaf.path.clone()),
                    leaf.kind,
                )
            })
        })
        .collect::<BTreeSet<_>>();

    let project = CodeProject::managed(ProjectKey("managed-v3-relation-catalog".into()), compiled)
        .expect("authenticated V3 named-relation project");
    let projected_results = project
        .managed
        .program
        .outputs
        .iter()
        .map(|output| (output.declaration.clone(), output.path.clone()))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        projected_results,
        executed_results
            .iter()
            .map(|(declaration, path, _)| (declaration.clone(), path.clone()))
            .collect(),
        "Rust must project every runtime-authenticated semantic result path",
    );

    let desired = required_generated_members(&project).expect("relation member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("relation member reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_c0_41),
        DocumentId(PersistentId::from_u128(0x89_c0_41)),
        1.0,
    )
    .expect("all named standalone constraints must cold materialize");

    let expanded_results = materialized
        .expansion
        .semantic_outputs
        .values()
        .map(|output| {
            (
                output.reference.declaration.clone(),
                output.reference.output.clone(),
                output.reference.expected_kind,
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(expanded_results, executed_results);

    let graph = materialized.editor.coordinator().intent().graph();
    let lowered_constraints = graph
        .nodes()
        .values()
        .filter_map(|node| match node.kind {
            IntentNodeKind::Constraint { constraint } => {
                assert!(
                    node.suppressed,
                    "catalog constraints are deliberately suppressed"
                );
                Some(constraint)
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(lowered_constraints, standalone_constraints);

    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("cold relation catalog owns accepted authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
    let document = accepted.session.design_document();
    assert!(!document.points().is_empty());
    assert!(!document.curves().is_empty());
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
    assert!(
        materialized
            .expansion
            .semantic_outputs
            .values()
            .filter(|output| { matches!(output.reference.expected_kind, FeatureKind::Constraint) })
            .count()
            >= standalone_constraints.len()
    );
}
