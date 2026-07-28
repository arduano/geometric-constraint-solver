// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::fmt::Write as _;

use geosolve_constraint_editor::{
    ConstructionPreview, ConstructionPreviewGeometry, EditorScene, ScreenPoint, SelectionItem,
    Viewport,
};
use geosolve_sketch::{
    DesignScalarId, DocumentDimensionDefinition, DocumentDimensionMode, GeometryRole,
    SketchAcceptedDocumentState,
};

pub(crate) fn viewport() -> Viewport {
    Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("static viewport is valid")
}

pub(crate) fn svg_markup(
    scene: Option<&EditorScene>,
    accepted: Option<&SketchAcceptedDocumentState>,
    selection: &[SelectionItem],
    construction_preview: Option<&ConstructionPreview>,
) -> String {
    let mut output = String::new();
    if let Some(accepted) = accepted {
        let identity = accepted.identity();
        let input = accepted.input();
        let _ = write!(
            output,
            "<g class=\"wb-accepted-scene\" data-scene-provenance=\"accepted\" data-accepted-document=\"{}\" data-accepted-revision=\"{}\" data-accepted-parameter-revision=\"{}\" data-accepted-parameter-digest=\"{}\" data-accepted-external-revision=\"{}\" data-accepted-external-digest=\"{}\" data-accepted-activation-revision=\"{}\" data-accepted-activation-digest=\"{}\">",
            identity.document(),
            identity.revision().get(),
            input.parameter_revision(),
            digest(input.parameter_digest().bytes()),
            input.external_snapshot_set_revision(),
            digest(input.external_snapshot_set_digest().bytes()),
            input.effective_activation_revision(),
            digest(input.activation_digest().bytes()),
        );
    } else {
        output.push_str("<g class=\"wb-accepted-scene\" data-scene-provenance=\"none\">");
    }
    output.push_str(
        "<g class=\"wb-grid\"><path d=\"M0 350H1000M500 0V700\"/></g><g class=\"wb-geometry\">",
    );
    if let Some(scene) = scene {
        for curve in &scene.curves {
            if curve.screen_polyline.len() < 2 {
                continue;
            }
            let path = polyline_path(&curve.screen_polyline);
            let selected = selection.contains(&SelectionItem::Curve(curve.span));
            let role = accepted
                .and_then(|state| state.document().geometry_role(curve.span.curve))
                .unwrap_or(GeometryRole::Profile);
            let _ = write!(
                output,
                "<path class=\"wb-curve{}{}\" d=\"{path}\" data-persistent-id=\"{}\" data-editor-segment=\"{}\" data-role=\"{}\"/>",
                if selected { " selected" } else { "" },
                if role == GeometryRole::Construction {
                    " construction"
                } else {
                    ""
                },
                curve.span.curve,
                curve.span.segment,
                if role == GeometryRole::Construction {
                    "construction"
                } else {
                    "profile"
                },
            );
        }
        output.push_str("</g><g class=\"wb-points\">");
        for point in &scene.points {
            let selected = selection.contains(&SelectionItem::Point(point.id));
            let _ = write!(
                output,
                "<circle class=\"wb-point{}\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"5\" data-persistent-id=\"{}\"/>",
                if selected { " selected" } else { "" },
                point.screen_position.x,
                point.screen_position.y,
                point.id,
            );
        }
    } else {
        output.push_str("</g><g class=\"wb-points\">");
    }
    output.push_str("</g><g class=\"wb-annotations\">");
    if let Some(accepted) = accepted {
        annotations(&mut output, accepted, selection);
    }
    output.push_str("</g>");
    if let Some(preview) = construction_preview {
        output.push_str(&construction_markup(preview, viewport()));
    }
    output.push_str("</g>");
    output
}

fn digest(bytes: [u8; 32]) -> String {
    let mut output = String::new();
    for byte in &bytes[..6] {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[allow(clippy::cast_precision_loss)]
fn annotations(
    output: &mut String,
    accepted: &SketchAcceptedDocumentState,
    selection: &[SelectionItem],
) {
    let document = accepted.document();
    for (index, constraint) in document.constraints().iter().enumerate() {
        let x = 26.0 + (index % 8) as f64 * 24.0;
        let selected = selection.contains(&SelectionItem::Constraint(constraint.id));
        let _ = write!(
            output,
            "<text class=\"wb-glyph{}\" x=\"{x}\" y=\"32\" data-persistent-id=\"{}\">C</text>",
            if selected { " selected" } else { "" },
            constraint.id,
        );
    }
    for (index, dimension) in document.dimensions().iter().enumerate() {
        let y = 58.0 + index as f64 * 20.0;
        let label = escape(&dimension.label);
        let mode = match dimension.mode {
            DocumentDimensionMode::Driving => "driving",
            DocumentDimensionMode::Reference => "reference",
        };
        let value = match dimension.mode {
            DocumentDimensionMode::Driving => document
                .scalar(dimension_target(&dimension.definition))
                .map(|scalar| scalar.value),
            DocumentDimensionMode::Reference => accepted.reference_value(dimension.id),
        }
        .filter(|value| value.is_finite());
        let value_attribute = value.map_or_else(String::new, |value| value.to_string());
        let displayed = value.map_or_else(|| "unavailable".into(), |value| value.to_string());
        let selected = selection.contains(&SelectionItem::Dimension(dimension.id));
        let _ = write!(
            output,
            "<text class=\"wb-dimension{}\" x=\"26\" y=\"{y}\" data-persistent-id=\"{}\" data-dimension-mode=\"{mode}\" data-dimension-value=\"{value_attribute}\">{label} = {displayed}</text>",
            if selected { " selected" } else { "" },
            dimension.id,
        );
    }
}

fn dimension_target(definition: &DocumentDimensionDefinition) -> DesignScalarId {
    match definition {
        DocumentDimensionDefinition::PointDistance { target, .. }
        | DocumentDimensionDefinition::CurveLength { target, .. }
        | DocumentDimensionDefinition::Radius { target, .. }
        | DocumentDimensionDefinition::Diameter { target, .. }
        | DocumentDimensionDefinition::OrientedAngle { target, .. }
        | DocumentDimensionDefinition::SupportingLineOffset { target, .. }
        | DocumentDimensionDefinition::ExactTranslatedSegmentOffset { target, .. } => *target,
    }
}

fn polyline_path(points: &[ScreenPoint]) -> String {
    let mut path = String::new();
    for (index, point) in points.iter().enumerate() {
        let _ = write!(
            path,
            "{} {:.3} {:.3} ",
            if index == 0 { 'M' } else { 'L' },
            point.x,
            point.y,
        );
    }
    path
}

fn construction_markup(preview: &ConstructionPreview, viewport: Viewport) -> String {
    let mut output = String::from("<g class=\"wb-draft\">");
    match preview {
        ConstructionPreview::Complete { geometry, .. } => {
            construction_geometry_markup(&mut output, geometry, viewport);
        }
        ConstructionPreview::Anchor { position } => {
            marker(&mut output, viewport, *position, "wb-draft-center");
        }
        ConstructionPreview::ArcRadiusGuide { center, start } => {
            line(&mut output, viewport, &[*center, *start]);
            marker(&mut output, viewport, *center, "wb-draft-center");
            marker(&mut output, viewport, *start, "wb-draft-start");
        }
    }
    output.push_str("</g>");
    output
}

fn construction_geometry_markup(
    output: &mut String,
    geometry: &ConstructionPreviewGeometry,
    viewport: Viewport,
) {
    match geometry {
        ConstructionPreviewGeometry::Point { position } => {
            marker(output, viewport, *position, "wb-draft-point");
        }
        ConstructionPreviewGeometry::Polyline { points } => {
            line(output, viewport, points);
        }
        ConstructionPreviewGeometry::Rectangle { first, second } => {
            let first = viewport.model_to_screen(*first);
            let second = viewport.model_to_screen(*second);
            let _ = write!(
                output,
                "<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{:.3}\" height=\"{:.3}\"/>",
                first.x.min(second.x),
                first.y.min(second.y),
                (second.x - first.x).abs(),
                (second.y - first.y).abs(),
            );
        }
        ConstructionPreviewGeometry::Circle { center, radius } => {
            let screen = viewport.model_to_screen(*center);
            let _ = write!(
                output,
                "<circle class=\"wb-draft-circle\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{:.3}\"/>",
                screen.x,
                screen.y,
                radius * viewport.pixels_per_model_unit,
            );
            marker(output, viewport, *center, "wb-draft-center");
        }
        ConstructionPreviewGeometry::CounterClockwiseArc {
            center,
            start,
            end,
            radius,
            large_arc,
            ..
        } => {
            let start_screen = viewport.model_to_screen(*start);
            let end_screen = viewport.model_to_screen(*end);
            let radius = radius * viewport.pixels_per_model_unit;
            let large = u8::from(*large_arc);
            let _ = write!(
                output,
                "<path d=\"M {:.3} {:.3} A {radius:.3} {radius:.3} 0 {large} 0 {:.3} {:.3}\"/>",
                start_screen.x, start_screen.y, end_screen.x, end_screen.y
            );
            marker(output, viewport, *center, "wb-draft-center");
            marker(output, viewport, *start, "wb-draft-start");
            marker(output, viewport, *end, "wb-draft-end");
        }
    }
}

fn line(output: &mut String, viewport: Viewport, points: &[[f64; 2]]) {
    let points = points
        .iter()
        .copied()
        .map(|point| viewport.model_to_screen(point))
        .collect::<Vec<_>>();
    if points.len() >= 2 {
        let _ = write!(output, "<path d=\"{}\"/>", polyline_path(&points));
    }
    for point in points {
        let _ = write!(
            output,
            "<circle class=\"wb-draft-point\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"4\"/>",
            point.x, point.y,
        );
    }
}

fn marker(output: &mut String, viewport: Viewport, point: [f64; 2], class: &str) {
    let point = viewport.model_to_screen(point);
    let _ = write!(
        output,
        "<circle class=\"wb-cursor-point {class}\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"7\"/>",
        point.x, point.y,
    );
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{
        ConstructionPreviewGeometry, EditorScene, RetainedEditorCoordinator, SelectionItem,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        DocumentDimensionMode, DocumentParameterId, DocumentParameterKind, DocumentParameterTarget,
        DocumentSolveRequest, ParameterBatch, ParameterBatchEntry, ParameterValue,
        RetainedSketchDocumentSession, SketchDocument,
    };

    use super::{construction_geometry_markup, svg_markup, viewport};
    use crate::workbench::panels::{
        accepted_redundancy_markup, host_state_markup, lifecycle_presentation, problem_markup,
    };

    fn arc_geometry(large_arc: bool, sweep_radians: f64) -> ConstructionPreviewGeometry {
        ConstructionPreviewGeometry::CounterClockwiseArc {
            center: [0.0, 0.0],
            start: [1.0, 0.0],
            end: [0.0, 1.0],
            radius: 1.0,
            sweep_radians,
            large_arc,
        }
    }

    fn parameter_batch(
        parameter: DocumentParameterId,
        revision: u64,
        value: ParameterValue,
    ) -> ParameterBatch {
        ParameterBatch::new(revision, vec![ParameterBatchEntry { parameter, value }]).unwrap()
    }

    #[test]
    fn serializes_explicit_minor_and_major_counterclockwise_arc_flags() {
        let mut minor = String::new();
        construction_geometry_markup(
            &mut minor,
            &arc_geometry(false, std::f64::consts::FRAC_PI_2),
            viewport(),
        );
        let mut major = String::new();
        construction_geometry_markup(
            &mut major,
            &arc_geometry(true, 3.0 * std::f64::consts::FRAC_PI_2),
            viewport(),
        );
        assert!(minor.contains("A 50.000 50.000 0 0 0"));
        assert!(major.contains("A 50.000 50.000 0 1 0"));
    }

    #[test]
    fn accepted_scene_glyphs_and_dimensions_keep_persistent_identity_and_domain_values() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let rectangle = document
            .add_rectangle("qualified", [0.0, 0.0], 4.0, 3.0)
            .unwrap();
        document
            .set_dimension_mode(rectangle.dimensions[1], DocumentDimensionMode::Reference)
            .unwrap();
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let accepted = session.accepted_state().unwrap();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            session.design_identity(),
            accepted.document(),
            session.design_document(),
            viewport(),
            0.8,
        )
        .unwrap();
        let selection = [SelectionItem::Point(rectangle.points[0])];
        let markup = svg_markup(Some(&scene), Some(accepted), &selection, None);
        let tree = crate::workbench::panels::tree_markup(accepted.document(), &selection);
        let point_identity = format!("data-persistent-id=\"{}\"", rectangle.points[0]);
        assert!(markup.contains("class=\"wb-point selected\""));
        assert!(markup.contains(&point_identity));
        assert!(tree.contains(&point_identity));
        assert!(tree.contains("aria-selected=\"true\""));
        for constraint in accepted.document().constraints() {
            assert_eq!(
                markup
                    .matches(&format!("data-persistent-id=\"{}\"", constraint.id))
                    .count(),
                1,
                "constraint glyph identity must be unique"
            );
        }
        assert!(markup.contains(&format!(
            "data-persistent-id=\"{}\" data-dimension-mode=\"driving\" data-dimension-value=\"4\"",
            rectangle.dimensions[0]
        )));
        assert!(markup.contains(&format!(
            "data-persistent-id=\"{}\" data-dimension-mode=\"reference\" data-dimension-value=\"3\"",
            rectangle.dimensions[1]
        )));
    }

    #[test]
    fn m47_lifecycle_attempt_and_accepted_identity_never_leak_attempt_into_scene() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let rectangle = document
            .add_rectangle("lifecycle", [0.0, 0.0], 4.0, 3.0)
            .unwrap();
        let parameter = document
            .add_parameter("width", DocumentParameterKind::Length)
            .unwrap();
        document
            .add_parameter_binding(
                parameter,
                DocumentParameterTarget::DrivingDimension(rectangle.dimensions[0]),
            )
            .unwrap();
        let session = RetainedSketchDocumentSession::new_with_parameter_batch(
            document,
            parameter_batch(parameter, 7, ParameterValue::Length(4.0)),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let mut coordinator = RetainedEditorCoordinator::new(session).unwrap();
        let accepted_before = coordinator.session().accepted_state().unwrap().identity();
        let accepted_geometry = coordinator
            .session()
            .accepted_state()
            .unwrap()
            .solve_result()
            .geometry
            .clone();

        let expected = coordinator.session().design_identity();
        coordinator
            .replace_parameter_batch(
                expected,
                parameter_batch(parameter, 8, ParameterValue::Angle(4.0)),
                DocumentSolveRequest::default(),
            )
            .unwrap();
        let lifecycle = host_state_markup(coordinator.session());
        let attempt = coordinator.session().last_attempt().identity();
        assert!(lifecycle.contains(&format!(
            "data-design-revision=\"{}\"",
            coordinator.session().design_identity().revision().get()
        )));
        assert!(lifecycle.contains(&format!(
            "data-attempt-revision=\"{}\"",
            attempt.revision().get()
        )));
        assert!(lifecycle.contains(&format!(
            "data-accepted-revision=\"{}\"",
            accepted_before.revision().get()
        )));
        assert!(lifecycle.contains("data-attempt-status=\"failed\""));
        assert_ne!(attempt.revision().get(), accepted_before.revision().get());

        let accepted = coordinator.session().accepted_state().unwrap();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            coordinator.session().design_identity(),
            accepted.document(),
            coordinator.session().design_document(),
            viewport(),
            0.8,
        )
        .unwrap();
        let markup = svg_markup(Some(&scene), Some(accepted), &[], None);
        assert!(markup.contains("data-scene-provenance=\"accepted\""));
        assert!(markup.contains(&format!(
            "data-accepted-revision=\"{}\"",
            accepted_before.revision().get()
        )));
        assert!(markup.contains("data-accepted-parameter-revision=\"7\""));
        assert!(!markup.contains("data-attempt-revision="));
        assert!(!markup.contains(&format!(
            "data-accepted-revision=\"{}\"",
            attempt.revision().get()
        )));
        assert_eq!(accepted.solve_result().geometry, accepted_geometry);
        assert_eq!(
            lifecycle_presentation(coordinator.lifecycle().status),
            ("rejected-attempt", "Rejected attempt")
        );
        let problems = coordinator.problems();
        assert!(problems.failure.is_some() || problems.rejection.is_some());
        let problem = problem_markup("Rejected attempt: parameter value has the wrong kind");
        assert!(problem.contains("Rejected attempt"));
        let unavailable = accepted_redundancy_markup(None);
        assert!(unavailable.contains("unavailable"));
        assert!(!unavailable.contains("rank zero"));
        assert!(!unavailable.contains("No sources published"));

        let expected = coordinator.session().design_identity();
        coordinator
            .replace_parameter_batch(
                expected,
                parameter_batch(parameter, 9, ParameterValue::Length(5.0)),
                DocumentSolveRequest::default(),
            )
            .unwrap();
        let recovered = coordinator.session().accepted_state().unwrap();
        assert_ne!(recovered.identity(), accepted_before);
        assert_eq!(recovered.input().parameter_revision(), 9);
    }
}
