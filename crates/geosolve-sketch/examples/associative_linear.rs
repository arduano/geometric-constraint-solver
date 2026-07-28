// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentAngleOrientation, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentLineOffsetOrientation, DocumentLineSide, DocumentSolveRequest,
    ScalarDomain, ScalarUnit, SketchDocument, SketchDocumentSession,
};

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: [f64; 2],
    end: [f64; 2],
) -> Result<CurveSpan, geosolve_sketch::DocumentError> {
    let start_id = document.add_point(format!("{label} start"), start)?;
    let end_id = document.add_point(format!("{label} end"), end)?;
    let length = (end[0] - start[0]).hypot(end[1] - start[1]);
    let curve = document.add_curve(
        label,
        CurveDefinition::Line {
            start: start_id,
            end: end_id,
            branch_direction: [(end[0] - start[0]) / length, (end[1] - start[1]) / length],
        },
    )?;
    Ok(CurveSpan::line(curve))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut document = SketchDocument::new(4.0)?;
    let axis = line(&mut document, "mirror axis", [-6.0, 0.0], [6.0, 0.0])?;
    let source = line(&mut document, "source", [0.0, 1.0], [4.0, 1.0])?;
    let translated = line(&mut document, "translated", [0.0, 3.0], [4.0, 3.0])?;
    let angled = line(&mut document, "angled", [0.0, 1.0], [4.0, 5.0])?;

    let offset = document.add_scalar(
        "offset target",
        2.0,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "exact translated segment",
        DocumentDimensionDefinition::ExactTranslatedSegmentOffset {
            source,
            target_segment: translated,
            target: offset,
            side: DocumentLineSide::Left,
            orientation: DocumentLineOffsetOrientation::Same,
        },
        DocumentDimensionMode::Driving,
    )?;
    let angle = document.add_scalar(
        "angle target",
        std::f64::consts::FRAC_PI_4,
        ScalarUnit::Angle,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "directed source angle",
        DocumentDimensionDefinition::OrientedAngle {
            first: source,
            second: angled,
            target: angle,
            orientation: DocumentAngleOrientation::CounterClockwise,
        },
        DocumentDimensionMode::Driving,
    )?;
    let mirror = document.add_mirrored_curve("source mirror", source.curve, axis)?;

    let session = SketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )?;
    assert!(session.runtime().accepted_result().accepted());
    println!(
        "document v{}: mirrored {} controls with {} ordinary symmetry constraints; rank {}, DOF {}",
        session.document().version(),
        mirror.point_pairs.len(),
        mirror.symmetry_constraints.len(),
        session
            .runtime()
            .accepted_result()
            .unstable_core_report()
            .rank,
        session
            .runtime()
            .accepted_result()
            .unstable_core_report()
            .local_degrees_of_freedom,
    );
    Ok(())
}
