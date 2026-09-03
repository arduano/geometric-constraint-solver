// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CODE_AUTHORING_FAMILIES, CodeAuthoringDeclarationKind, CodeOwnerAddress, CodePointSeedSource,
    CodeProject, CompiledManagedSource, FeatureKind, KeyedReconcileState, ManagedPathSegment,
    ManagedStatement, ProjectKey, SemanticOutputPath, SemanticSymbol,
    managed_point_value_mutations, materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::{GeometryRecipeKind, IntentNodeKind, IntentSessionId};

const GEOMETRY_CATALOG_SOURCE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-geometry-catalog.sketch.ts"
);
const GEOMETRY_CATALOG_ENVELOPE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-geometry-catalog.json"
);

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end gate keeps the closed geometry catalog, executed result paths, and finite cold authority together"
)]
fn compiled_v3_named_geometry_catalog_cold_materializes_all_27_variants() {
    let unchecked: CompiledManagedSource = serde_json::from_str(GEOMETRY_CATALOG_ENVELOPE)
        .expect("named-geometry compiler-envelope JSON");
    let rust_artifact = serde_json::to_string(&unchecked.artifact).expect("Rust artifact JSON");
    if rust_artifact != unchecked.canonical_artifact_json {
        let mismatch = rust_artifact
            .bytes()
            .zip(unchecked.canonical_artifact_json.bytes())
            .position(|(rust, typescript)| rust != typescript)
            .unwrap_or_else(|| {
                rust_artifact
                    .len()
                    .min(unchecked.canonical_artifact_json.len())
            });
        let start = mismatch.saturating_sub(80);
        let rust_end = (mismatch + 120).min(rust_artifact.len());
        let typescript_end = (mismatch + 120).min(unchecked.canonical_artifact_json.len());
        panic!(
            "TypeScript/Rust artifact mismatch at byte {mismatch}\nRust: {}\nTypeScript: {}",
            &rust_artifact[start..rust_end],
            &unchecked.canonical_artifact_json[start..typescript_end],
        );
    }
    let compiled = CompiledManagedSource::from_json(GEOMETRY_CATALOG_ENVELOPE)
        .expect("authenticated V3 named-geometry compiler envelope");
    compiled
        .validate_input_source(GEOMETRY_CATALOG_SOURCE)
        .expect("named-geometry source digest");

    let expected_families = CODE_AUTHORING_FAMILIES
        .iter()
        .filter(|family| {
            matches!(
                family.declaration,
                CodeAuthoringDeclarationKind::Geometry(_)
            )
        })
        .map(|family| format!("{}.{}", family.namespace, family.method))
        .collect::<BTreeSet<_>>();
    assert_eq!(GeometryRecipeKind::ALL.len(), 27);
    assert_eq!(expected_families.len(), GeometryRecipeKind::ALL.len());

    let ir_declarations = compiled
        .ir
        .statements
        .iter()
        .filter_map(|statement| match statement {
            ManagedStatement::Declaration { symbol, .. } => Some(symbol.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(ir_declarations.len(), GeometryRecipeKind::ALL.len());

    let executed_families = compiled
        .artifact
        .declarations
        .iter()
        .map(|declaration| declaration.family.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(executed_families, expected_families);
    assert_eq!(compiled.artifact.declarations.len(), ir_declarations.len());
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
    assert_eq!(
        executed_results.len(),
        compiled
            .artifact
            .declarations
            .iter()
            .map(|declaration| declaration.result.len())
            .sum::<usize>(),
        "executed geometry result paths must be unique within their declaration",
    );

    let project = CodeProject::managed(ProjectKey("managed-v3-geometry-catalog".into()), compiled)
        .expect("authenticated V3 named-geometry project");
    assert_eq!(
        project.managed.program.declarations.len(),
        GeometryRecipeKind::ALL.len()
    );
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

    let desired = required_generated_members(&project).expect("geometry member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("geometry member reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x89_c0_25),
        DocumentId(PersistentId::from_u128(0x89_c0_25)),
        1.0,
    )
    .expect("all named geometry variants must cold materialize");

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

    let newly_writable = materialized
        .expansion
        .writable_points
        .iter()
        .filter(|point| {
            matches!(
                &point.edit.writable_addresses()[0].owner.address,
                CodeOwnerAddress::DirectDeclaration { declaration }
                    if [
                        "tangentArc",
                        "openControlBSpline",
                        "periodicControlBSpline",
                        "openControlNurbs",
                        "periodicControlNurbs",
                    ]
                    .contains(&declaration.0.as_str())
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        newly_writable.len(),
        19,
        "Tangent Arc and all four keyed spline families must publish every source-backed point",
    );
    let tangent_start = newly_writable
        .iter()
        .find(|point| {
            let addresses = point.edit.writable_addresses();
            let [address] = addresses.as_slice() else {
                return false;
            };
            matches!(
                &address.owner.address,
                CodeOwnerAddress::DirectDeclaration { declaration }
                    if declaration.0 == "tangentArc"
            ) && address.output
                == SemanticOutputPath(vec![ManagedPathSegment::Field("start".into())])
        })
        .expect("Tangent Arc start source point");
    assert!(matches!(
        tangent_start.source,
        CodePointSeedSource::Reference { .. }
    ));

    let placements = newly_writable
        .iter()
        .enumerate()
        .flat_map(|(index, point)| {
            let offset = f64::from(u16::try_from(index).expect("bounded writable inventory"));
            point.edit.point_updates([100.0 + offset, 200.0 + offset])
        })
        .collect::<Vec<_>>();
    let compiler_authority = project
        .managed
        .compiled
        .as_deref()
        .expect("named geometry compiler authority");
    for (address, target) in &placements {
        managed_point_value_mutations(
            &project.project,
            compiler_authority,
            &[(address.clone(), *target)],
        )
        .unwrap_or_else(|error| {
            panic!("{} is not source-writable: {error}", address.display_path())
        });
    }
    let source_mutations =
        managed_point_value_mutations(&project.project, compiler_authority, &placements)
            .expect("every Tangent Arc and NURBS point maps back to authenticated source");
    let mutation_paths = source_mutations
        .iter()
        .map(|mutation| (mutation.declaration.as_str(), mutation.path.clone()))
        .collect::<BTreeSet<_>>();
    let mut expected_mutation_paths = ["center", "start", "end"]
        .into_iter()
        .map(|field| ("tangentArc", vec![ManagedPathSegment::Field(field.into())]))
        .collect::<BTreeSet<_>>();
    for declaration in [
        "openControlBSpline",
        "periodicControlBSpline",
        "openControlNurbs",
        "periodicControlNurbs",
    ] {
        for index in 0..4 {
            expected_mutation_paths.insert((
                declaration,
                vec![
                    ManagedPathSegment::Field("controls".into()),
                    ManagedPathSegment::Index(index),
                    ManagedPathSegment::Field("position".into()),
                ],
            ));
        }
    }
    assert_eq!(mutation_paths, expected_mutation_paths);

    let expanded_kinds = materialized.expansion.semantic_outputs.values().fold(
        BTreeMap::<FeatureKind, usize>::new(),
        |mut kinds, output| {
            *kinds.entry(output.reference.expected_kind).or_default() += 1;
            kinds
        },
    );
    assert!(
        expanded_kinds
            .get(&FeatureKind::Point)
            .is_some_and(|count| *count > 0)
    );
    assert!(
        expanded_kinds
            .get(&FeatureKind::CurveSpan)
            .is_some_and(|count| *count > 0)
    );
    assert!(
        expanded_kinds
            .get(&FeatureKind::Scalar)
            .is_some_and(|count| *count > 0)
    );

    let lowered_recipes = materialized
        .editor
        .coordinator()
        .intent()
        .graph()
        .nodes()
        .values()
        .filter_map(|node| match node.kind {
            IntentNodeKind::Geometry { recipe } => Some(recipe),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        lowered_recipes,
        GeometryRecipeKind::ALL.into_iter().collect(),
        "the public cold boundary must lower every named geometry recipe",
    );

    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("cold geometry catalog owns accepted authority");
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
