// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, SketchDocument, VisualProfileIssueKind, VisualProfileOptions,
    VisualProfileStatus,
};

fn add_line_then_quadratic(document: &mut SketchDocument, shared_endpoint: bool) {
    let start = document.add_point("line start", [-2.0, 0.0]).unwrap();
    let line_end = document.add_point("line end", [0.0, 0.0]).unwrap();
    let curve_start = if shared_endpoint {
        line_end
    } else {
        document
            .add_point("unowned curve start", [0.0, 0.0])
            .unwrap()
    };
    let control = document.add_point("quadratic control", [2.0, 0.0]).unwrap();
    let end = document.add_point("quadratic end", [4.0, 2.0]).unwrap();
    document
        .add_curve(
            "line",
            CurveDefinition::Line {
                start,
                end: line_end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
    document
        .add_curve(
            "quadratic",
            CurveDefinition::QuadraticBezier {
                controls: [curve_start, control, end],
            },
        )
        .unwrap();
}

#[test]
fn owned_opposite_inward_tangent_endpoint_is_local_but_unowned_contact_stays_incomplete() {
    let mut owned = SketchDocument::new(10.0).unwrap();
    add_line_then_quadratic(&mut owned, true);
    let owned_analysis = owned.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(
        owned_analysis.status,
        VisualProfileStatus::Complete,
        "an owned smooth endpoint must remain local: {owned_analysis:#?}"
    );
    assert!(owned_analysis.issues.is_empty(), "{owned_analysis:#?}");

    let mut unowned = SketchDocument::new(10.0).unwrap();
    add_line_then_quadratic(&mut unowned, false);
    let unowned_analysis = unowned.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(unowned_analysis.status, VisualProfileStatus::Skipped);
    assert!(unowned_analysis.issues.iter().any(|issue| matches!(
        issue.kind,
        VisualProfileIssueKind::TangentIntersection { .. }
    )));
}
