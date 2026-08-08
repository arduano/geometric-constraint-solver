// SPDX-License-Identifier: GPL-3.0-or-later

use std::hint::black_box;
use std::time::{Duration, Instant};

use geosolve_core::{HardValidity, SolverConfig};
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, DocumentCommand, DocumentEdit, SketchDocument,
    SketchDocumentSession, VisualProfileAnalysis, VisualProfileBudgetCounter,
    VisualProfileBudgetReport, VisualProfileOptions, VisualProfileStatus, alpha_scenario,
};

const WARMUPS: usize = 2;
const SAMPLES: usize = 12;

const CONSTRUCTION_LOAD_BUDGET: Duration = Duration::from_secs(1);
const CONSTRUCTION_EDIT_BUDGET: Duration = Duration::from_secs(1);
const NURBS_LOAD_BUDGET: Duration = Duration::from_secs(2);
const NURBS_EDIT_BUDGET: Duration = Duration::from_secs(2);
const ALL_FAMILY_PROFILE_BUDGET: Duration = Duration::from_secs(10);
const NURBS_SELF_PROFILE_BUDGET: Duration = Duration::from_secs(5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ProfileResourceSignature {
    status: VisualProfileStatus,
    families: usize,
    faces: usize,
    contours: usize,
    edges: usize,
    intersections: usize,
    self_intersections: usize,
    issues: usize,
    budgets: VisualProfileBudgetReport,
}

fn main() {
    println!("m32/config: warmups={WARMUPS} samples={SAMPLES} statistic=p95-nearest-rank");
    construction_workload();
    nurbs_workload();
    profile_workload(
        AlphaScenarioKind::ProfileAllFamilies,
        "profile-all-families",
        ALL_FAMILY_PROFILE_BUDGET,
    );
    profile_workload(
        AlphaScenarioKind::ProfileNurbsSelfIntersection,
        "profile-nurbs-self-intersection",
        NURBS_SELF_PROFILE_BUDGET,
    );
    match peak_rss_kib() {
        Some(value) => println!("process/peak-rss: {value}KiB observational=true"),
        None => println!("process/peak-rss: unavailable observational=true"),
    }
}

fn construction_workload() {
    let fixture = alpha_scenario(AlphaScenarioKind::SupportingOffset, 1.0).unwrap();
    let AlphaScenarioIds::SupportingOffset(ids) = fixture.ids else {
        panic!("supporting-offset fixture returned mismatched IDs");
    };
    let accepted =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    validate_session(&accepted);
    let canonical = accepted.export_json().unwrap();
    validate_canonical(&canonical);
    print_document_resources(
        "construction-supporting-offset",
        accepted.document(),
        &canonical,
    );

    let load = measure(
        || (),
        |()| {
            let document = SketchDocument::from_json(black_box(&canonical)).unwrap();
            SketchDocumentSession::new(document, fixture.request, SolverConfig::default()).unwrap()
        },
        validate_session,
    );
    report(
        "construction-supporting-offset",
        "load-solve",
        &load,
        CONSTRUCTION_LOAD_BUDGET,
    );

    let initial_end = accepted
        .document()
        .point(ids.target_points[1])
        .unwrap()
        .position;
    let edit = measure(
        || accepted.clone(),
        |mut session| {
            let outcome = session
                .apply(DocumentCommand::new(
                    session.revision(),
                    DocumentEdit::SetPointPosition {
                        point: ids.target_points[1],
                        position: [3.5, 0.0],
                    },
                ))
                .unwrap();
            assert!(outcome.accepted(), "supporting-offset edit was rejected");
            session
        },
        |session| {
            validate_session(session);
            assert_eq!(session.history_len(), 1);
            let moved_end = session
                .document()
                .point(ids.target_points[1])
                .unwrap()
                .position;
            assert!(
                (moved_end[0] - initial_end[0]).hypot(moved_end[1] - initial_end[1]) > 0.1,
                "supporting-offset edit did not move the edited endpoint"
            );
            validate_canonical(&session.export_json().unwrap());
        },
    );
    report(
        "construction-supporting-offset",
        "edit-solve",
        &edit,
        CONSTRUCTION_EDIT_BUDGET,
    );
}

fn nurbs_workload() {
    let fixture = alpha_scenario(AlphaScenarioKind::NurbsLocalSupport, 1.0).unwrap();
    let AlphaScenarioIds::NurbsLocalSupport(ids) = fixture.ids.clone() else {
        panic!("NURBS local-support fixture returned mismatched IDs");
    };
    let accepted =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    validate_session(&accepted);
    let canonical = accepted.export_json().unwrap();
    validate_canonical(&canonical);
    print_document_resources("nurbs-local-support", accepted.document(), &canonical);

    let load = measure(
        || (),
        |()| {
            let document = SketchDocument::from_json(black_box(&canonical)).unwrap();
            SketchDocumentSession::new(document, fixture.request, SolverConfig::default()).unwrap()
        },
        validate_session,
    );
    report(
        "nurbs-local-support",
        "load-solve",
        &load,
        NURBS_LOAD_BUDGET,
    );

    let initial_spans = accepted.document().curve_spans(ids.curve).unwrap().len();
    let edit = measure(
        || accepted.clone(),
        |mut session| {
            let outcome = session
                .apply(DocumentCommand::new(
                    session.revision(),
                    DocumentEdit::InsertNurbsKnot {
                        curve: ids.curve,
                        parameter: 0.5,
                    },
                ))
                .unwrap();
            assert!(outcome.accepted(), "NURBS knot insertion was rejected");
            session
        },
        |session| {
            validate_session(session);
            assert_eq!(session.history_len(), 1);
            assert_eq!(
                session.document().curve_spans(ids.curve).unwrap().len(),
                initial_spans + 1
            );
            validate_canonical(&session.export_json().unwrap());
        },
    );
    report(
        "nurbs-local-support",
        "knot-insert-solve",
        &edit,
        NURBS_EDIT_BUDGET,
    );
}

fn profile_workload(kind: AlphaScenarioKind, name: &str, budget: Duration) {
    let fixture = alpha_scenario(kind, 1.0).unwrap();
    let self_curve = match &fixture.ids {
        AlphaScenarioIds::ProfileNurbsSelfIntersection(ids) => Some(ids.curve),
        AlphaScenarioIds::ProfileAllFamilies(_) => None,
        _ => panic!("profile workload returned mismatched IDs"),
    };
    let uat = kind.profile_uat().expect("profile workload UAT metadata");
    let accepted =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    validate_session(&accepted);
    let document = accepted.document();
    let canonical = accepted.export_json().unwrap();
    validate_canonical(&canonical);
    print_document_resources(name, document, &canonical);

    let baseline = document.analyze_visual_profiles(uat.options);
    validate_profile(
        &baseline,
        uat.options,
        uat.expected_status,
        uat.expected_family_count,
        uat.expected_minimum_face_count,
        self_curve,
    );
    let signature = profile_signature(&baseline, self_curve);
    print_profile_resources(name, signature);

    let samples = measure(
        || (),
        |()| black_box(document).analyze_visual_profiles(uat.options),
        |analysis| {
            validate_profile(
                analysis,
                uat.options,
                uat.expected_status,
                uat.expected_family_count,
                uat.expected_minimum_face_count,
                self_curve,
            );
            assert_eq!(
                profile_signature(analysis, self_curve),
                signature,
                "profile resource consumption changed for identical input"
            );
            assert_eq!(document.to_canonical_json().unwrap(), canonical);
        },
    );
    report(name, "analyze", &samples, budget);
}

fn measure<Input, Output>(
    mut setup: impl FnMut() -> Input,
    mut operation: impl FnMut(Input) -> Output,
    mut validate: impl FnMut(&Output),
) -> Vec<Duration> {
    for _ in 0..WARMUPS {
        let output = operation(setup());
        validate(&output);
        black_box(output);
    }
    (0..SAMPLES)
        .map(|_| {
            let input = setup();
            let started = Instant::now();
            let output = operation(input);
            let elapsed = started.elapsed();
            validate(&output);
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

fn validate_session(session: &SketchDocumentSession) {
    let accepted = session.accepted_result();
    let result = accepted.accepted_view();
    let report = &result.unstable_core_report();
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max.is_finite());
    assert!(report.hard_residual_max <= 1.0e-9);
    assert!(report.rank_is_valid);
    assert!(result.acceptance_hard_residual_max.unwrap() <= 1.0e-9);
}

fn validate_canonical(json: &str) {
    let restored = SketchDocument::from_json(json).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), json);
}

fn validate_profile(
    analysis: &VisualProfileAnalysis,
    options: VisualProfileOptions,
    expected_status: VisualProfileStatus,
    expected_families: usize,
    minimum_faces: usize,
    self_curve: Option<geosolve_sketch::CurveId>,
) {
    assert_eq!(analysis.status, expected_status, "{analysis:#?}");
    assert_eq!(analysis.families.len(), expected_families, "{analysis:#?}");
    assert!(analysis.faces.len() >= minimum_faces, "{analysis:#?}");
    assert!(analysis.issues.is_empty(), "{analysis:#?}");
    assert_eq!(
        analysis.budgets.candidate_pairs.limit,
        options.max_candidate_pairs
    );
    assert_eq!(
        analysis.budgets.intersection_subdivisions.limit,
        options.max_intersection_subdivisions
    );
    assert_eq!(
        analysis.budgets.intersection_roots.limit,
        options.max_intersection_roots
    );
    assert_eq!(analysis.budgets.fragments.limit, options.max_fragments);
    assert_eq!(
        analysis.budgets.integration_subdivisions.limit,
        options.max_integration_subdivisions
    );
    assert_eq!(
        analysis.budgets.containment_tests.limit,
        options.max_containment_tests
    );
    assert_eq!(analysis.budgets.faces.limit, options.max_faces);
    for counter in profile_counters(analysis.budgets) {
        assert!(counter.consumed <= counter.limit);
    }
    for root in &analysis.intersections {
        assert!(
            root.first_parameter_enclosure
                .into_iter()
                .all(f64::is_finite)
        );
        assert!(
            root.second_parameter_enclosure
                .into_iter()
                .all(f64::is_finite)
        );
        assert!(
            root.position_enclosure
                .into_iter()
                .flatten()
                .all(f64::is_finite)
        );
    }
    for face in &analysis.faces {
        assert!(face.visual_area.is_finite());
        assert!(face.area_uncertainty.is_finite());
        for contour in &face.contours {
            assert!(contour.signed_area.is_finite());
            assert!(contour.area_uncertainty.is_finite());
            for edge in &contour.edges {
                assert!(edge.start.into_iter().all(f64::is_finite));
                assert!(edge.end.into_iter().all(f64::is_finite));
                assert!(edge.source_parameters.into_iter().all(f64::is_finite));
                assert!(
                    edge.source_parameter_enclosures
                        .into_iter()
                        .flatten()
                        .all(f64::is_finite)
                );
            }
        }
    }
    if let Some(curve) = self_curve {
        assert_eq!(
            analysis
                .intersections
                .iter()
                .filter(|root| root.first_span.curve == curve && root.second_span.curve == curve)
                .count(),
            1,
            "{analysis:#?}"
        );
    }
}

fn profile_signature(
    analysis: &VisualProfileAnalysis,
    self_curve: Option<geosolve_sketch::CurveId>,
) -> ProfileResourceSignature {
    ProfileResourceSignature {
        status: analysis.status,
        families: analysis.families.len(),
        faces: analysis.faces.len(),
        contours: analysis.faces.iter().map(|face| face.contours.len()).sum(),
        edges: analysis
            .faces
            .iter()
            .flat_map(|face| &face.contours)
            .map(|contour| contour.edges.len())
            .sum(),
        intersections: analysis.intersections.len(),
        self_intersections: self_curve.map_or(0, |curve| {
            analysis
                .intersections
                .iter()
                .filter(|root| root.first_span.curve == curve && root.second_span.curve == curve)
                .count()
        }),
        issues: analysis.issues.len(),
        budgets: analysis.budgets,
    }
}

fn print_document_resources(name: &str, document: &SketchDocument, json: &str) {
    println!(
        "{name}/document: points={} scalars={} curves={} contacts={} constraints={} dimensions={} trim-views={} json={}B",
        document.points().len(),
        document.scalars().len(),
        document.curves().len(),
        document.contacts().len(),
        document.constraints().len(),
        document.dimensions().len(),
        document.trim_views().len(),
        json.len(),
    );
}

fn print_profile_resources(name: &str, signature: ProfileResourceSignature) {
    println!(
        "{name}/result: status={:?} families={} faces={} contours={} edges={} intersections={} self-intersections={} issues={}",
        signature.status,
        signature.families,
        signature.faces,
        signature.contours,
        signature.edges,
        signature.intersections,
        signature.self_intersections,
        signature.issues,
    );
    let labels = [
        "candidate-pairs",
        "intersection-subdivisions",
        "intersection-roots",
        "fragments",
        "integration-subdivisions",
        "containment-tests",
        "faces",
    ];
    for (label, counter) in labels.into_iter().zip(profile_counters(signature.budgets)) {
        println!(
            "{name}/resource/{label}: consumed={} limit={}",
            counter.consumed, counter.limit
        );
    }
}

fn profile_counters(report: VisualProfileBudgetReport) -> [VisualProfileBudgetCounter; 7] {
    [
        report.candidate_pairs,
        report.intersection_subdivisions,
        report.intersection_roots,
        report.fragments,
        report.integration_subdivisions,
        report.containment_tests,
        report.faces,
    ]
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
