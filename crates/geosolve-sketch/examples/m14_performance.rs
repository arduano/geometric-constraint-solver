// SPDX-License-Identifier: GPL-3.0-or-later

use std::hint::black_box;
use std::time::{Duration, Instant};

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_sketch::{
    AlphaPerformanceSize, DocumentCommand, DocumentDimensionDefinition, DocumentEdit,
    DocumentSolveRequest, SketchDocument, SketchDocumentSession, alpha_performance_document,
};

const SAMPLES: usize = 12;

#[derive(Clone, Copy)]
struct Budgets {
    import: Duration,
    first_solve: Duration,
    edit_solve: Duration,
}

fn main() {
    for (size, name, budgets) in [
        (
            AlphaPerformanceSize::Small,
            "small",
            Budgets {
                import: Duration::from_millis(20),
                first_solve: Duration::from_millis(500),
                edit_solve: Duration::from_millis(300),
            },
        ),
        (
            AlphaPerformanceSize::Medium,
            "medium",
            Budgets {
                import: Duration::from_millis(150),
                first_solve: Duration::from_secs(4),
                edit_solve: Duration::from_millis(1_500),
            },
        ),
    ] {
        run(size, name, budgets);
    }
}

fn run(size: AlphaPerformanceSize, name: &str, budgets: Budgets) {
    let document = alpha_performance_document(size).unwrap();
    let json = document.to_canonical_json().unwrap();
    println!(
        "{name}/document: points={} scalars={} curves={} contacts={} constraints={} dimensions={} json={}B",
        document.points().len(),
        document.scalars().len(),
        document.curves().len(),
        document.contacts().len(),
        document.constraints().len(),
        document.dimensions().len(),
        json.len(),
    );
    let parsed = SketchDocument::from_json(&json).unwrap();
    let accepted = SketchDocumentSession::new(
        parsed.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap();
    validate(&accepted);
    let width = accepted
        .document()
        .dimensions()
        .iter()
        .find_map(|dimension| match dimension.definition {
            DocumentDimensionDefinition::CurveLength { target, .. }
                if dimension.label == "width-4" =>
            {
                Some(target)
            }
            _ => None,
        })
        .unwrap();
    let edited_value = accepted.document().scalar(width).unwrap().value * 1.01;

    for _ in 0..2 {
        black_box(SketchDocument::from_json(black_box(&json)).unwrap());
        let warm = SketchDocumentSession::new(
            parsed.clone(),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        validate(&warm);
        let mut edit = accepted.clone();
        let outcome = edit
            .apply(DocumentCommand::new(
                edit.revision(),
                DocumentEdit::SetScalarValue {
                    scalar: width,
                    value: edited_value,
                },
            ))
            .unwrap();
        assert!(outcome.accepted());
        validate(&edit);
    }

    let import = measure(
        || SketchDocument::from_json(black_box(&json)).unwrap(),
        |imported| imported.validate().unwrap(),
    );
    let mut first_inputs = vec![parsed.clone(); SAMPLES].into_iter();
    let first_solve = measure(
        || {
            SketchDocumentSession::new(
                black_box(first_inputs.next().unwrap()),
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .unwrap()
        },
        validate,
    );
    let mut edit_inputs = vec![accepted.clone(); SAMPLES].into_iter();
    let edit_solve = measure(
        || {
            let mut candidate = black_box(edit_inputs.next().unwrap());
            let outcome = candidate
                .apply(DocumentCommand::new(
                    candidate.revision(),
                    DocumentEdit::SetScalarValue {
                        scalar: width,
                        value: edited_value,
                    },
                ))
                .unwrap();
            assert!(outcome.accepted());
            candidate
        },
        validate,
    );

    report(name, "import", &import, budgets.import);
    report(name, "first-solve", &first_solve, budgets.first_solve);
    report(
        name,
        "incremental-edit-solve",
        &edit_solve,
        budgets.edit_solve,
    );
}

fn measure<T>(
    mut operation: impl FnMut() -> T,
    mut validate_output: impl FnMut(&T),
) -> Vec<Duration> {
    (0..SAMPLES)
        .map(|_| {
            let started = Instant::now();
            let output = operation();
            let elapsed = started.elapsed();
            validate_output(&output);
            black_box(output);
            elapsed
        })
        .collect()
}

fn report(name: &str, measurement: &str, samples: &[Duration], budget: Duration) {
    let mut ordered = samples.to_vec();
    ordered.sort_unstable();
    let median = ordered[ordered.len() / 2];
    let p95_index = (ordered.len() * 95).div_ceil(100).saturating_sub(1);
    let p95 = ordered[p95_index];
    println!(
        "{name}/{measurement}: median={:.3}ms p95={:.3}ms budget={:.3}ms",
        median.as_secs_f64() * 1_000.0,
        p95.as_secs_f64() * 1_000.0,
        budget.as_secs_f64() * 1_000.0,
    );
    assert!(
        p95 <= budget,
        "{name}/{measurement} p95 {p95:?} exceeded {budget:?}"
    );
}

fn validate(session: &SketchDocumentSession) {
    let accepted = session.accepted_result();
    let report = &accepted.accepted_view().core_report;
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max <= 1.0e-9);
    assert!(report.rank_is_valid);
}
