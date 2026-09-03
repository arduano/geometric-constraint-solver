// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_sketch::{DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, FeatureKind, KeyedReconcileState, ManagedPathSegment,
    ProjectKey, SemanticOutputPath, SemanticSymbol, materialize_code_project_cold,
    required_generated_members,
};
use geosolve_sketch_intent::{AggregateKind, IntentNodeKind, IntentSessionId, OperationKind};

const SOURCE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-operation-catalog.sketch.ts"
);
const ENVELOPE: &str = include_str!(
    "../../../packages/geosolve-sketch-code/test/fixtures/managed-clean-operation-catalog.json"
);

fn path(fields: &[&str]) -> SemanticOutputPath {
    SemanticOutputPath(
        fields
            .iter()
            .map(|field| ManagedPathSegment::Field((*field).into()))
            .collect(),
    )
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end gate keeps the closed operation catalog, aggregates, chaining, and accepted native authority together"
)]
fn compiled_v3_operation_catalog_cold_materializes_all_operations_and_aggregates() {
    let compiled = CompiledManagedSource::from_json(ENVELOPE)
        .expect("authenticated V3 operation-catalog compiler envelope");
    compiled
        .validate_input_source(SOURCE)
        .expect("operation-catalog source digest");
    let project = CodeProject::managed(ProjectKey("managed-v3-operation-catalog".into()), compiled)
        .expect("authenticated operation-catalog project");
    let desired = required_generated_members(&project).expect("operation member inventory");
    let generated = KeyedReconcileState::empty()
        .plan(desired, &BTreeSet::new())
        .expect("operation member reconciliation")
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x90_0c_a7),
        DocumentId(PersistentId::from_u128(0x90_0c_a7)),
        1.0,
    )
    .expect("all named operations and aggregates must cold materialize");

    assert_eq!(
        materialized
            .expansion
            .operation_plans
            .iter()
            .map(|retained| retained.plan.operation)
            .collect::<Vec<_>>(),
        OperationKind::ALL,
        "native operation plans must retain exact source declaration order",
    );
    for retained in &materialized.expansion.operation_plans {
        let mutates_existing_geometry = matches!(
            retained.plan.operation,
            OperationKind::Split
                | OperationKind::Break
                | OperationKind::Trim
                | OperationKind::Extend
        );
        assert_eq!(
            retained.plan.outputs.is_empty(),
            mutates_existing_geometry,
            "only the four in-place operations have no newly reserved native outputs: {:?}",
            retained.plan.operation,
        );
    }

    let graph = materialized.editor.coordinator().intent().graph();
    let operation_kinds = graph
        .nodes()
        .values()
        .filter_map(|node| match node.kind {
            IntentNodeKind::Operation { operation } => Some(operation),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        operation_kinds,
        OperationKind::ALL.into_iter().collect(),
        "every public operation family must reach the native intent graph",
    );
    let aggregate_kinds = graph
        .nodes()
        .values()
        .filter_map(|node| match node.kind {
            IntentNodeKind::Aggregate { aggregate } => Some(aggregate),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        aggregate_kinds,
        BTreeSet::from([AggregateKind::OpenChain, AggregateKind::ClosedProfile]),
    );

    let intent_symbol = |source_symbol: &str| {
        materialized
            .expansion
            .declaration_provenance
            .iter()
            .find_map(|(intent, source)| (source.0 == source_symbol).then_some(intent))
            .unwrap_or_else(|| panic!("missing intent owner for `{source_symbol}`"))
    };
    let rectangle = graph
        .node_by_symbol(intent_symbol("rectangle"))
        .expect("rectangle operation node");
    let closed_profile = graph
        .node_by_symbol(intent_symbol("closedProfile"))
        .expect("closed-profile aggregate node");
    assert_eq!(closed_profile.inputs.len(), 4);
    assert!(
        closed_profile
            .inputs
            .values()
            .all(|source| source.node == rectangle.id),
        "all four named rectangle spans must feed the closed-profile aggregate",
    );

    let semantic_paths = materialized
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
    for (symbol, output, kind) in [
        ("mirror", path(&["span"]), FeatureKind::CurveSpan),
        ("associativeFillet", path(&["span"]), FeatureKind::CurveSpan),
        (
            "rectangle",
            path(&["spans", "bottom"]),
            FeatureKind::CurveSpan,
        ),
        ("openChain", path(&["chain"]), FeatureKind::Chain),
        ("closedProfile", path(&["profile"]), FeatureKind::Profile),
    ] {
        assert!(
            semantic_paths.contains(&(SemanticSymbol(symbol.into()), output, kind)),
            "missing semantic result path for {symbol}",
        );
    }

    let plans_by_symbol = materialized
        .expansion
        .operation_plans
        .iter()
        .map(|retained| {
            let source = materialized
                .expansion
                .declaration_provenance
                .get(&retained.symbol)
                .unwrap_or_else(|| {
                    panic!(
                        "operation plan `{}` has no source declaration owner",
                        retained.symbol
                    )
                });
            (source.0.as_str(), retained.plan.outputs.len())
        })
        .collect::<BTreeMap<_, _>>();
    for symbol in ["regularPolygon", "slot", "linearPattern", "profileOffset"] {
        assert!(
            plans_by_symbol.get(symbol).is_some_and(|count| *count > 1),
            "{symbol} must retain its full topology-dependent native result plan",
        );
    }

    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .expect("cold operation catalog owns accepted authority");
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
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
}
