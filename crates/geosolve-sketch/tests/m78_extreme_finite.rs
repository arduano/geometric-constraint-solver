// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_geometry::Point2;
use geosolve_sketch::{Sketch, SketchSolveRequest};

#[test]
fn extreme_finite_midpoint_segment_solves_and_validates() {
    let mut sketch = Sketch::new(10.0).expect("sketch");
    let midpoint = sketch
        .add_point(Point2::new(1.0e308, 0.0))
        .expect("midpoint");
    let start = sketch.add_point(Point2::new(8.0e307, 0.0)).expect("start");
    let end = sketch.add_point(Point2::new(1.2e308, 0.0)).expect("end");
    let segment = sketch.add_segment(start, end).expect("finite segment");
    sketch
        .add_midpoint(midpoint, segment)
        .expect("midpoint relation");

    let result = sketch
        .solve(SketchSolveRequest::default(), SolverConfig::default())
        .expect("extreme finite sketch must compile and solve");
    assert!(result.accepted(), "{:#?}", result.rejection);
    assert!(
        result
            .geometry
            .points
            .iter()
            .all(|point| point.position.iter().all(|value| value.is_finite()))
    );
    assert!(
        result
            .acceptance_hard_residual_max
            .is_some_and(|residual| residual.is_finite() && residual <= 1.0e-9)
    );
}
