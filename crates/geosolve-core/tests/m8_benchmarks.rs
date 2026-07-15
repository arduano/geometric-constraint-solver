#[path = "../benches/support/representative.rs"]
mod representative;

use geosolve_core::{AuditEvaluationStatus, SolverConfig};
use representative::{
    Family, RepresentativeDefinition, validate_assemblies, validate_compiled_workload,
    validate_definition_shape, validate_report,
};

#[test]
fn representative_case_sizes_are_exact_and_deterministic() {
    let expected = [
        (Family::CadLike, 100, 50, 5),
        (Family::CadLike, 1_000, 500, 50),
        (Family::CadLike, 10_000, 5_000, 500),
        (Family::LinkageLike, 99, 33, 3),
        (Family::LinkageLike, 999, 333, 31),
        (Family::LinkageLike, 9_999, 3_333, 303),
    ];
    assert_eq!(representative::CASES.len(), expected.len());
    for ((family, tangent_variables), expected_case) in
        representative::CASES.into_iter().zip(expected)
    {
        assert_eq!(
            (family, tangent_variables),
            (expected_case.0, expected_case.1)
        );
        let definition = RepresentativeDefinition::new(family, tangent_variables);
        validate_definition_shape(&definition, family, tangent_variables);
        assert_eq!(definition.variable_blocks(), expected_case.2);
        assert_eq!(definition.component_count(), expected_case.3);
    }
}

#[test]
fn representative_evaluators_have_valid_jacobians_audits_and_reports() {
    assert_eq!(Family::CadLike.label(), "cad_like");
    assert_eq!(Family::LinkageLike.label(), "linkage_like");
    for (family, tangent_variables) in [(Family::CadLike, 20), (Family::LinkageLike, 33)] {
        let definition = RepresentativeDefinition::new(family, tangent_variables);
        assert_eq!(definition.tangent_variables(), tangent_variables);
        assert_eq!(
            definition.variable_blocks(),
            tangent_variables / if family == Family::CadLike { 2 } else { 3 }
        );
        assert_eq!(definition.component_count(), 1);

        let mut shards = definition.compile_component_shards().unwrap();
        let assemblies = shards
            .iter()
            .map(|problem| problem.assemble_dense().unwrap())
            .collect::<Vec<_>>();
        validate_assemblies(&definition, &assemblies);
        let problem = &mut shards[0];
        let jacobians = problem.check_jacobians(1.0e-6).unwrap();
        assert!(
            jacobians.all_within(1.0e-6),
            "{family:?} maximum relative error was {}",
            jacobians.max_relative_error()
        );
        let report = problem.solve(SolverConfig::default()).unwrap();
        validate_report(&definition, &report, 0);
        assert_eq!(report.audit.sources.len(), definition.variable_blocks());
        for source in &report.audit.sources {
            assert!(!source.source_label.is_empty());
            assert!(!source.rows.is_empty());
            for row in &source.rows {
                assert_eq!(row.evaluation_status, AuditEvaluationStatus::Evaluated);
                assert!(!row.template.is_empty());
                assert!(!row.bindings.is_empty());
                assert!(row.scale.is_finite() && row.scale > 0.0);
                assert!(row.raw_residual.is_finite());
                assert!(row.normalized_residual.is_finite());
            }
        }
    }
}

#[test]
fn representative_edit_resolve_reuses_every_unedited_component() {
    for (family, tangent_variables) in [(Family::CadLike, 40), (Family::LinkageLike, 66)] {
        let definition = RepresentativeDefinition::new(family, tangent_variables);
        assert_eq!(definition.component_count(), 2);
        let mut workload = definition.compile().unwrap();
        validate_compiled_workload(&definition, &workload);
        let initial = workload
            .problem
            .solve_decomposed(SolverConfig::default(), &[])
            .unwrap();
        validate_report(&definition, &initial, 0);

        workload.perturb_edit_variable().unwrap();
        let edited = workload
            .problem
            .solve_decomposed(SolverConfig::default(), &[workload.edit_variable])
            .unwrap();
        validate_report(&definition, &edited, 1);
    }
}
