// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, DocumentSolveRequest, OperationControl, OperationOutcome,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument, SolverConfig,
};
use geosolve_sketch_topology::{TopologyProductionProfile, TopologyRequest, TopologySnapshot};

fn session(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .unwrap()
}

fn profile(session: &RetainedSketchDocumentSession) -> TopologyProductionProfile {
    let OperationOutcome::Completed { value, .. } = TopologySnapshot::capture(session)
        .unwrap()
        .prepare(TopologyRequest::default())
        .execute(OperationControl::default())
        .unwrap()
    else {
        panic!("topology must complete");
    };
    value.production_profile.unwrap()
}

fn circles() -> RetainedSketchDocumentSession {
    let mut document = SketchDocument::new(10.0).unwrap();
    let center = document.add_point("center", [0.0, 0.0]).unwrap();
    for (label, value) in [("outer", 12.0), ("inner", 2.0)] {
        let radius = document
            .add_scalar(label, value, ScalarUnit::Length, ScalarDomain::Positive)
            .unwrap();
        document
            .add_curve(label, CurveDefinition::Circle { center, radius })
            .unwrap();
    }
    session(document)
}

#[test]
fn circles_and_holes_sample_accepted_support_with_bounded_sagitta() {
    let session = circles();
    let profile = profile(&session);
    let coarse = profile.sample_polygons(&session, 0.02).unwrap();
    let fine = profile.sample_polygons(&session, 0.002).unwrap();
    assert_eq!(coarse.len(), 2); // Production faces: annulus and its interior disk.
    let ring = coarse
        .iter()
        .find(|region| !region.holes.is_empty())
        .unwrap();
    let fine_ring = fine.iter().find(|region| !region.holes.is_empty()).unwrap();
    assert!(fine_ring.outer.len() > ring.outer.len());
    for (points, radius) in [(&ring.outer, 12.0), (&ring.holes[0], 2.0)] {
        assert!(points.len() > 8);
        for (a, b) in points
            .iter()
            .zip(points.iter().cycle().skip(1))
            .take(points.len())
        {
            assert!((a[0].hypot(a[1]) - radius).abs() < 1e-10);
            let midpoint_radius = a[0].midpoint(b[0]).hypot(a[1].midpoint(b[1]));
            assert!(radius - midpoint_radius <= 0.02);
        }
    }
}

#[test]
fn stale_session_and_unrepresentable_targets_refuse_sampling() {
    let mut session = circles();
    let profile = profile(&session);
    for error in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e-30] {
        assert!(profile.sample_polygons(&session, error).is_err());
    }
    session
        .transact(session.design_identity(), |document| {
            document.add_point("new input", [40.0, 20.0])?;
            Ok(())
        })
        .unwrap();
    assert!(profile.sample_polygons(&session, 0.02).is_err());
}

#[test]
fn straight_rectangle_needs_no_curvature_subdivision() {
    let mut document = SketchDocument::new(100.0).unwrap();
    document
        .add_rectangle("board", [0.0, 0.0], 85.0, 56.0)
        .unwrap();
    let session = session(document);
    let sampled = profile(&session)
        .sample_polygons(&session, f64::MIN_POSITIVE)
        .unwrap();
    assert_eq!(sampled.len(), 1);
    assert_eq!(sampled[0].outer.len(), 4);
    assert!(sampled[0].holes.is_empty());
    let area = sampled[0]
        .outer
        .iter()
        .zip(sampled[0].outer.iter().cycle().skip(1))
        .take(4)
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>()
        / 2.0;
    assert!((area - 85.0 * 56.0).abs() < 1e-10);
}
