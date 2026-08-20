// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, ScalarDomain, ScalarUnit, SketchDocument, VisualProfileIssueKind,
    VisualProfileOptions, VisualProfilePointContainment, VisualProfilePointContainmentError,
    VisualProfileStatus,
};

fn add_ellipse(document: &mut SketchDocument, label: &str, semi_major: f64, ratio: f64) {
    let center = document
        .add_point(format!("{label}.center"), [0.0, 0.0])
        .unwrap();
    let major_axis_point = document
        .add_point(format!("{label}.major"), [semi_major, 0.0])
        .unwrap();
    let minor_axis_ratio = document
        .add_scalar(
            format!("{label}.ratio"),
            ratio,
            ScalarUnit::Parameter,
            ScalarDomain::Bounded {
                lower: f64::from_bits(1),
                upper: 1.0,
            },
        )
        .unwrap();
    document
        .add_curve(
            label,
            CurveDefinition::Ellipse {
                center,
                major_axis_point,
                minor_axis_ratio,
            },
        )
        .unwrap();
}

#[test]
fn certified_general_face_containment_respects_nested_holes_and_boundaries() {
    let mut document = SketchDocument::new(10.0).unwrap();
    add_ellipse(&mut document, "outer", 4.0, 0.5);
    add_ellipse(&mut document, "inner", 2.0, 0.5);
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.issues.is_empty());
    assert_eq!(analysis.faces.len(), 2);

    let annulus = analysis
        .faces
        .iter()
        .find(|face| face.contours.len() == 2)
        .expect("nested ellipses publish an annulus");
    let disk = analysis
        .faces
        .iter()
        .find(|face| face.contours.len() == 1)
        .expect("nested ellipses publish the inner disk");
    let options = VisualProfileOptions::default();
    assert_eq!(
        annulus.classify_point([3.0, 0.0], options),
        Ok(VisualProfilePointContainment::Inside)
    );
    assert_eq!(
        annulus.classify_point([0.0, 0.0], options),
        Ok(VisualProfilePointContainment::Outside)
    );
    assert_eq!(
        disk.classify_point([0.0, 0.0], options),
        Ok(VisualProfilePointContainment::Inside)
    );
    assert_eq!(
        annulus.classify_point([2.0, 0.0], options),
        Ok(VisualProfilePointContainment::Boundary)
    );
    assert_eq!(
        disk.classify_point([2.0, 0.0], options),
        Ok(VisualProfilePointContainment::Boundary)
    );
    assert_eq!(
        disk.classify_point([f64::NAN, 0.0], options),
        Err(VisualProfilePointContainmentError::NonFinitePoint)
    );
}

#[test]
fn certified_mixed_face_boundary_is_exact_and_budget_exhaustion_is_typed() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let start = document.add_point("start", [0.0, 0.0]).unwrap();
    let middle = document.add_point("middle", [2.0, 2.0]).unwrap();
    let end = document.add_point("end", [4.0, 0.0]).unwrap();
    document
        .add_curve(
            "quadratic",
            CurveDefinition::QuadraticBezier {
                controls: [start, middle, end],
            },
        )
        .unwrap();
    document
        .add_curve(
            "closing line",
            CurveDefinition::Line {
                start: end,
                end: start,
                branch_direction: [-1.0, 0.0],
            },
        )
        .unwrap();
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    assert!(analysis.issues.is_empty());
    let face = analysis.faces.first().expect("mixed face");
    let options = VisualProfileOptions::default();
    assert_eq!(
        face.classify_point([2.0, 0.5], options),
        Ok(VisualProfilePointContainment::Inside)
    );
    assert_eq!(
        face.classify_point([2.0, -0.5], options),
        Ok(VisualProfilePointContainment::Outside)
    );
    assert_eq!(
        face.classify_point([2.0, 1.0], options),
        Ok(VisualProfilePointContainment::Boundary)
    );

    let exhausted = face.classify_point(
        [2.0, 0.5],
        VisualProfileOptions {
            max_containment_tests: 1,
            ..VisualProfileOptions::default()
        },
    );
    assert!(matches!(
        exhausted,
        Err(VisualProfilePointContainmentError::Uncertified {
            kind: VisualProfileIssueKind::ContainmentBudgetExceeded { limit: 1, .. }
        })
    ));
}

#[test]
fn ray_aligned_boundary_uses_a_bounded_alternate_certificate() {
    let mut document = SketchDocument::new(10.0).unwrap();
    let first = document.add_point("first", [0.0, 0.0]).unwrap();
    let second = document.add_point("second", [4.0, 2.0]).unwrap();
    let third = document.add_point("third", [4.0, 0.0]).unwrap();
    for (label, start, end, branch_direction) in [
        (
            "skew boundary",
            first,
            second,
            [2.0 / 5.0_f64.sqrt(), 1.0 / 5.0_f64.sqrt()],
        ),
        ("vertical boundary", second, third, [0.0, -1.0]),
        ("horizontal boundary", third, first, [-1.0, 0.0]),
    ] {
        document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction,
                },
            )
            .unwrap();
    }
    let analysis = document.analyze_visual_profiles(VisualProfileOptions::default());
    assert_eq!(analysis.status, VisualProfileStatus::Complete);
    let face = analysis.faces.first().expect("triangular face");
    assert_eq!(
        face.classify_point(
            [2.0, 1.0],
            VisualProfileOptions {
                max_containment_tests: 50,
                ..VisualProfileOptions::default()
            },
        ),
        Ok(VisualProfilePointContainment::Boundary)
    );
}
