use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use geosolve_core::SolverConfig;

#[path = "support/representative.rs"]
mod representative;

use representative::{
    CASES, RepresentativeDefinition, validate_assemblies, validate_compiled_workload,
    validate_definition_shape, validate_report,
};

fn configure(group: &mut BenchmarkGroup<'_, WallTime>, tangent_variables: usize) {
    let (sample_size, warm_up_millis, measurement_millis) = if tangent_variables >= 9_999 {
        (10, 100, 750)
    } else if tangent_variables >= 999 {
        (15, 150, 850)
    } else {
        (25, 250, 1_000)
    };
    group
        .sample_size(sample_size)
        .warm_up_time(Duration::from_millis(warm_up_millis))
        .measurement_time(Duration::from_millis(measurement_millis))
        .throughput(Throughput::Elements(
            u64::try_from(tangent_variables).expect("benchmark size fits u64"),
        ));
}

fn definition_compile(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("representative_definition_compile");
    for &(family, tangent_variables) in &CASES {
        configure(&mut group, tangent_variables);
        validate_definition_shape(
            &RepresentativeDefinition::new(family, tangent_variables),
            family,
            tangent_variables,
        );
        group.bench_function(
            BenchmarkId::new(family.label(), tangent_variables),
            |bencher| {
                bencher.iter_custom(|iterations| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let start = Instant::now();
                        let definition = RepresentativeDefinition::new(family, tangent_variables);
                        let workload = definition.compile().expect("definition must compile");
                        elapsed += start.elapsed();

                        validate_definition_shape(&definition, family, tangent_variables);
                        validate_compiled_workload(&definition, &workload);
                        black_box((&definition, &workload));
                        drop(workload);
                        drop(definition);
                    }
                    elapsed
                });
            },
        );
    }
    group.finish();
}

fn linearization_assembly(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("representative_linearization_assembly");
    for &(family, tangent_variables) in &CASES {
        configure(&mut group, tangent_variables);
        let definition = RepresentativeDefinition::new(family, tangent_variables);
        let shards = definition
            .compile_component_shards()
            .expect("component shards must compile");
        group.bench_function(
            BenchmarkId::new(family.label(), tangent_variables),
            |bencher| {
                bencher.iter_custom(|iterations| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let start = Instant::now();
                        let assemblies = shards
                            .iter()
                            .map(|problem| {
                                problem.assemble_dense().expect("assembly must be finite")
                            })
                            .collect::<Vec<_>>();
                        elapsed += start.elapsed();

                        validate_assemblies(&definition, &assemblies);
                        black_box(&assemblies);
                        drop(assemblies);
                    }
                    elapsed
                });
            },
        );
    }
    group.finish();
}

fn decomposition_solve_diagnostics(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("representative_decomposition_solve_diagnostics");
    for &(family, tangent_variables) in &CASES {
        configure(&mut group, tangent_variables);
        let definition = RepresentativeDefinition::new(family, tangent_variables);
        group.bench_function(
            BenchmarkId::new(family.label(), tangent_variables),
            |bencher| {
                bencher.iter_custom(|iterations| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let mut workload = definition.compile().expect("definition must compile");
                        let start = Instant::now();
                        let report = workload
                            .problem
                            .solve_decomposed(SolverConfig::default(), &[])
                            .expect("representative solve must produce a report");
                        elapsed += start.elapsed();

                        validate_report(&definition, &report, 0);
                        black_box(&report);
                        drop(report);
                        drop(workload);
                    }
                    elapsed
                });
            },
        );
    }
    group.finish();
}

fn component_edit_resolve(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("representative_component_edit_resolve");
    for &(family, tangent_variables) in &CASES {
        configure(&mut group, tangent_variables);
        let definition = RepresentativeDefinition::new(family, tangent_variables);
        let mut workload = definition.compile().expect("definition must compile");
        let initial_report = workload
            .problem
            .solve_decomposed(SolverConfig::default(), &[])
            .expect("initial solve must produce a report");
        validate_report(&definition, &initial_report, 0);
        black_box(&initial_report);
        drop(initial_report);
        group.bench_function(
            BenchmarkId::new(family.label(), tangent_variables),
            |bencher| {
                bencher.iter_custom(|iterations| {
                    let mut elapsed = Duration::ZERO;
                    for _ in 0..iterations {
                        let start = Instant::now();
                        workload
                            .perturb_edit_variable()
                            .expect("benchmark edit must remain finite");
                        let report = workload
                            .problem
                            .solve_decomposed(SolverConfig::default(), &[workload.edit_variable])
                            .expect("edited solve must produce a report");
                        elapsed += start.elapsed();

                        validate_report(&definition, &report, definition.component_count() - 1);
                        black_box(&report);
                        drop(report);
                    }
                    elapsed
                });
            },
        );
    }
    group.finish();
}

fn representative_benchmarks(criterion: &mut Criterion) {
    definition_compile(criterion);
    linearization_assembly(criterion);
    decomposition_solve_diagnostics(criterion);
    component_edit_resolve(criterion);
}

criterion_group!(benches, representative_benchmarks);
criterion_main!(benches);
