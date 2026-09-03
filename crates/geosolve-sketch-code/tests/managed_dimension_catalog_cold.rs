// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CODE_AUTHORING_FAMILIES, CodeAuthoringDeclarationKind, CodeProject, CompiledManagedSource,
    KeyedReconcileState, ManagedStatement, ProjectKey, SemanticOutputPath, SemanticSymbol,
    materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{DimensionKind, IntentNodeKind, IntentSessionId};

const DIMENSION_CATALOG_SOURCE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-dimension-catalog.sketch.ts"
);
const DIMENSION_CATALOG_ENVELOPE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-dimension-catalog.json"
);

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one compiled cold gate keeps the complete native dimension catalog together"
)]
fn compiled_v3_named_dimension_catalog_cold_materializes_all_eight_variants() {
    let unchecked: CompiledManagedSource = serde_json::from_str(DIMENSION_CATALOG_ENVELOPE)
        .expect("named-dimension compiler-envelope JSON");
    assert_eq!(
        serde_json::to_string(&unchecked.artifact).expect("Rust artifact JSON"),
        unchecked.canonical_artifact_json,
        "TypeScript and Rust must share the exact canonical executed artifact",
    );
    let compiled = CompiledManagedSource::from_json(DIMENSION_CATALOG_ENVELOPE)
        .expect("authenticated V3 named-dimension compiler envelope");
    compiled
        .validate_input_source(DIMENSION_CATALOG_SOURCE)
        .expect("named-dimension source digest");

    assert_eq!(DimensionKind::ALL.len(), 8);
    let expected_families = CODE_AUTHORING_FAMILIES
        .iter()
        .filter(|family| {
            matches!(
                family.declaration,
                CodeAuthoringDeclarationKind::Dimension(_)
            )
        })
        .map(|family| format!("{}.{}", family.namespace, family.method))
        .collect::<BTreeSet<_>>();
    assert_eq!(expected_families.len(), DimensionKind::ALL.len());
    let executed_families = compiled
        .artifact
        .declarations
        .iter()
        .filter(|declaration| declaration.family.starts_with("dimension."))
        .map(|declaration| declaration.family.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(executed_families, expected_families);

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
    let executed_results = compiled
        .artifact
        .declarations
        .iter()
        .flat_map(|declaration| {
            assert!(!declaration.result.is_empty());
            declaration.result.iter().map(|leaf| {
                (
                    SemanticSymbol(declaration.declaration.clone()),
                    SemanticOutputPath(leaf.path.clone()),
                    leaf.kind,
                )
            })
        })
        .collect::<BTreeSet<_>>();

    let project = CodeProject::managed(ProjectKey("managed-v3-dimension-catalog".into()), compiled)
        .expect("authenticated V3 named-dimension project");
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

    let desired = required_generated_members(&project).expect("dimension member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("dimension member reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_c0_08),
        DocumentId(PersistentId::from_u128(0x89_c0_08)),
        1.0,
    )
    .expect("all eight named dimensions must cold materialize");

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
    let lowered_dimensions = graph
        .nodes()
        .values()
        .filter_map(|node| match node.kind {
            IntentNodeKind::Dimension { dimension } => {
                assert!(
                    node.suppressed,
                    "catalog dimensions are deliberately suppressed"
                );
                Some(dimension)
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(lowered_dimensions, DimensionKind::ALL.into_iter().collect());

    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("cold dimension catalog owns accepted authority");
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
}
