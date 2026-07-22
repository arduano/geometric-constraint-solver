// SPDX-License-Identifier: GPL-3.0-or-later

use std::hint::black_box;
use std::time::{Duration, Instant};

use criterion::{Criterion, criterion_group, criterion_main};
use geosolve_core::SolverConfig;
use geosolve_sketch::{SketchDocumentSession, VisualProfileAnalysis};

#[path = "support/m33_representative.rs"]
mod representative;

use representative::{
    PreparedWorkload, WorkloadSize, expected_representative_signature, workloads,
};

fn production_cold_compile(criterion: &mut Criterion, workloads: &[PreparedWorkload]) {
    let mut group = criterion.benchmark_group("production_cold_compile");
    configure(&mut group);
    for workload in workloads {
        group.bench_function(workload.definition.kind.key(), |bencher| {
            bencher.iter_custom(|iterations| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let document = workload.definition.document.clone();
                    let started = Instant::now();
                    let output = document.lower().map(|lowered| {
                        let compiled = lowered
                            .sketch()
                            .compile(workload.definition.runtime_request());
                        (lowered, compiled)
                    });
                    elapsed += started.elapsed();
                    let _ = black_box(output);
                }
                elapsed
            });
        });
    }
    group.finish();
}

fn production_warm_edit_solve(criterion: &mut Criterion, workloads: &[PreparedWorkload]) {
    let mut group = criterion.benchmark_group("production_warm_edit_solve");
    configure(&mut group);
    for workload in workloads {
        group.bench_function(workload.definition.kind.key(), |bencher| {
            bencher.iter_custom(|iterations| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let mut session = workload.accepted.clone();
                    let command = workload.definition.edit_command(session.revision());
                    let started = Instant::now();
                    let output = session.apply(command);
                    elapsed += started.elapsed();
                    let _ = black_box((session, output));
                }
                elapsed
            });
        });
    }
    group.finish();
}

fn production_solve_diagnostics(criterion: &mut Criterion, workloads: &[PreparedWorkload]) {
    let mut group = criterion.benchmark_group("production_solve_diagnostics");
    configure(&mut group);
    for workload in workloads {
        group.bench_function(workload.definition.kind.key(), |bencher| {
            bencher.iter_custom(|iterations| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let document = workload.definition.document.clone();
                    let started = Instant::now();
                    let output = SketchDocumentSession::new(
                        document,
                        workload.definition.request,
                        SolverConfig::default(),
                    );
                    elapsed += started.elapsed();
                    let _ = black_box(output);
                }
                elapsed
            });
        });
    }
    group.finish();
}

fn production_visual_profile(criterion: &mut Criterion, workloads: &[PreparedWorkload]) {
    let mut group = criterion.benchmark_group("production_visual_profile");
    configure(&mut group);
    for workload in workloads {
        group.bench_function(workload.definition.kind.key(), |bencher| {
            bencher.iter_custom(|iterations| {
                let mut elapsed = Duration::ZERO;
                for _ in 0..iterations {
                    let started = Instant::now();
                    let output: VisualProfileAnalysis = workload
                        .accepted
                        .document()
                        .analyze_visual_profiles(workload.profile_options);
                    elapsed += started.elapsed();
                    black_box(output);
                }
                elapsed
            });
        });
    }
    group.finish();
}

fn configure(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>) {
    group
        .sample_size(10)
        .warm_up_time(Duration::from_millis(200))
        .measurement_time(Duration::from_millis(750));
}

fn m33_representative(criterion: &mut Criterion) {
    // All correctness, audit, edit, and completeness validation runs once before
    // Criterion enters any timed iteration.
    let workloads = workloads(WorkloadSize::Representative)
        .into_iter()
        .map(PreparedWorkload::prepare)
        .collect::<Vec<_>>();
    for workload in &workloads {
        assert_eq!(
            workload.signature,
            expected_representative_signature(workload.definition.kind),
            "{} representative signature changed",
            workload.definition.kind.key()
        );
        println!(
            "m33/signature/{key}: shape={shape} signature={signature:#?}",
            key = workload.definition.kind.key(),
            shape = workload.definition.kind.shape_name(),
            signature = workload.signature,
        );
        black_box((workload.definition.kind.shape_name(), workload.signature));
    }
    production_cold_compile(criterion, &workloads);
    production_warm_edit_solve(criterion, &workloads);
    production_solve_diagnostics(criterion, &workloads);
    production_visual_profile(criterion, &workloads);
    match peak_rss_kib() {
        Some(value) => println!("m33/process/peak-rss-kib: {value} observational=true"),
        None => println!("m33/process/peak-rss-kib: unavailable observational=true"),
    }
}

#[cfg(target_os = "linux")]
fn peak_rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")?
                .split_ascii_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

#[cfg(not(target_os = "linux"))]
fn peak_rss_kib() -> Option<u64> {
    None
}

criterion_group!(benches, m33_representative);
criterion_main!(benches);
