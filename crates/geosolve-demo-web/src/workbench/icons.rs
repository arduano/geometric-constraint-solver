// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared vector language for constraint and dimension concepts.

use geosolve_constraint_editor::{
    AuthoringTool, ConstraintIntent, DimensionKind, EditorTool, FeatureAuthoringTool,
    SceneConstraintGlyph,
};

const SELECT: &str = "<path d=\"M-7-8L6 1 0 3 3 9-1 10-4 4-8 8Z\"/><path d=\"M0 3l-4 1\"/>";
const POINT: &str = "<circle r=\"2\"/><path d=\"M-8 0h4M4 0h4M0-8v4M0 4v4\"/>";
const LINE: &str = "<path d=\"M-7 6L7-6\"/><circle cx=\"-7\" cy=\"6\" r=\"1.5\"/><circle cx=\"7\" cy=\"-6\" r=\"1.5\"/>";
const POLYLINE: &str = concat!(
    "<path d=\"M-8 5L-2-4 3 3 8-6\"/>",
    "<circle cx=\"-8\" cy=\"5\" r=\"1.3\"/><circle cx=\"-2\" cy=\"-4\" r=\"1.3\"/>",
    "<circle cx=\"3\" cy=\"3\" r=\"1.3\"/><circle cx=\"8\" cy=\"-6\" r=\"1.3\"/>",
);
const RECTANGLE: &str = "<rect x=\"-7\" y=\"-5\" width=\"14\" height=\"10\"/><circle cx=\"-7\" cy=\"5\" r=\"1.2\"/><circle cx=\"7\" cy=\"-5\" r=\"1.2\"/>";
const CIRCLE: &str = "<circle r=\"7\"/><circle r=\"1.3\"/><path d=\"M0 0L5-5\"/>";
const ARC: &str = "<path d=\"M-7 5A9 9 0 0 1 7-5\"/><circle cx=\"-7\" cy=\"5\" r=\"1.4\"/><circle cx=\"7\" cy=\"-5\" r=\"1.4\"/>";
const QUADRATIC_BEZIER: &str = concat!(
    "<path d=\"M-8 5L0-7 8 5\" stroke-dasharray=\"2 2\"/>",
    "<path d=\"M-8 5Q0-7 8 5\"/>",
    "<circle cx=\"-8\" cy=\"5\" r=\"1.2\"/><circle cy=\"-7\" r=\"1.2\"/><circle cx=\"8\" cy=\"5\" r=\"1.2\"/>",
);
const CUBIC_BEZIER: &str = concat!(
    "<path d=\"M-8 5L-3-7M3 7L8-5\" stroke-dasharray=\"2 2\"/>",
    "<path d=\"M-8 5C-3-7 3 7 8-5\"/>",
    "<circle cx=\"-8\" cy=\"5\" r=\"1.1\"/><circle cx=\"-3\" cy=\"-7\" r=\"1.1\"/>",
    "<circle cx=\"3\" cy=\"7\" r=\"1.1\"/><circle cx=\"8\" cy=\"-5\" r=\"1.1\"/>",
);
const ELLIPSE: &str = "<ellipse rx=\"8\" ry=\"5\"/><path d=\"M-8 0H8M0-5V5\" stroke-dasharray=\"2 2\"/><circle r=\"1.2\"/>";
const ELLIPTICAL_ARC: &str = concat!(
    "<path d=\"M-8 2A8 5 0 0 1 6-4\"/>",
    "<path d=\"M-8 2H0V-5\" stroke-dasharray=\"2 2\"/>",
    "<circle cx=\"-8\" cy=\"2\" r=\"1.3\"/><circle cx=\"6\" cy=\"-4\" r=\"1.3\"/>",
);
const RATIONAL_CONIC: &str = concat!(
    "<path d=\"M-8 5L0-7 8 5\" stroke-dasharray=\"2 2\"/>",
    "<path d=\"M-8 5Q0-4 8 5\"/>",
    "<path d=\"M0-9L2-7 0-5-2-7Z\"/>",
);
const PARABOLA: &str = "<path d=\"M-8-5Q0 9 8-5\"/><path d=\"M0-8V5\" stroke-dasharray=\"2 2\"/><circle cy=\"1\" r=\"1.3\"/>";
const HYPERBOLA: &str = concat!(
    "<path d=\"M-8-7Q-2 0-4 7M8-7Q2 0 4 7\"/>",
    "<path d=\"M-8 8L8-8M-8-8L8 8\" stroke-dasharray=\"2 2\"/>",
);
const NURBS: &str = concat!(
    "<path d=\"M-9 5L-5-6 0-2 5 6 9-5\" stroke-dasharray=\"2 2\"/>",
    "<path d=\"M-9 5C-6-7-1-5 0-2S6 7 9-5\"/>",
    "<circle cx=\"-9\" cy=\"5\" r=\"1\"/><circle cx=\"-5\" cy=\"-6\" r=\"1\"/>",
    "<circle cy=\"-2\" r=\"1\"/><circle cx=\"5\" cy=\"6\" r=\"1\"/><circle cx=\"9\" cy=\"-5\" r=\"1\"/>",
);
const CONSTRUCTION_ROLE: &str = concat!(
    "<path d=\"M-8 5L8-5\" stroke-dasharray=\"3 2\"/>",
    "<path d=\"M-7-5H7M-4-8v6M0-8v6M4-8v6\"/>",
);

const FIXED: &str = concat!(
    "<rect x=\"-5.5\" y=\"-1\" width=\"11\" height=\"8\" rx=\"1.5\"/>",
    "<path d=\"M-3.5-1V-4a3.5 3.5 0 0 1 7 0v3M0 2v2.5\"/>",
);
const COINCIDENT: &str = concat!(
    "<circle r=\"5.5\"/><circle r=\"1.8\"/>",
    "<path d=\"M-8 0h2M6 0h2M0-8v2M0 6v2\"/>",
);
const HORIZONTAL: &str = "<path d=\"M-8 0H8M-6-3V3M6-3V3\"/>";
const VERTICAL: &str = "<path d=\"M0-8V8M-3-6H3M-3 6H3\"/>";
const POINT_ON_CURVE: &str = "<path d=\"M-8 4Q0-6 8 4\"/><circle cx=\"0\" cy=\"-1\" r=\"2\"/>";
const PARALLEL: &str = concat!(
    "<path d=\"M-8 3L2-7M-2 7L8-3\"/>",
    "<path d=\"M-5-1h3v-3M1 3h3V0\"/>",
);
const PERPENDICULAR: &str = "<path d=\"M-7-7V6H7M-7 2h4v4\"/>";
const CONCENTRIC: &str = "<circle r=\"7\"/><circle r=\"3.5\"/><circle r=\"1\"/>";
const COLLINEAR: &str = concat!(
    "<path d=\"M-8 5L8-5\"/>",
    "<circle cx=\"-4\" cy=\"2.5\" r=\"1.35\"/>",
    "<circle r=\"1.35\"/><circle cx=\"4\" cy=\"-2.5\" r=\"1.35\"/>",
);
const EQUAL_LENGTH: &str = "<path d=\"M-8-4H8M-8 4H8M-1-7L1-1M-1 1L1 7\"/>";
const EQUAL_RADIUS: &str = concat!(
    "<circle cx=\"-4.5\" r=\"4\"/><circle cx=\"4.5\" r=\"4\"/>",
    "<path d=\"M-4.5 0l2.8-2.8M4.5 0l2.8-2.8\"/>",
);
const MIDPOINT: &str = concat!(
    "<path d=\"M-8 4H8M-4 1v6M4 1v6\"/>",
    "<path d=\"M0-4L4 4H-4Z\"/>",
);
const SYMMETRY: &str = concat!(
    "<path d=\"M0-9v4M0-2v4M0 5v4\"/>",
    "<path d=\"M-3-6L-7 0l4 6M3-6L7 0 3 6\"/>",
);
const CONTACT: &str = concat!(
    "<path d=\"M-8-5Q-2-5 0 0M0 0Q2 5 8 5\"/>",
    "<circle r=\"1.6\"/>",
);
const TANGENCY: &str = "<circle cy=\"-2\" r=\"5\"/><path d=\"M-8 3H8\"/>";
const DIRECTION: &str = concat!(
    "<path d=\"M-8 5Q0-1 8 5M-6-4H5\"/>",
    "<path d=\"M2-7L6-4 2-1\"/>",
);
const NORMAL: &str = concat!(
    "<path d=\"M-8 5Q0-1 8 5M0-1V-8\"/>",
    "<path d=\"M0-5h3v3\"/>",
);
const EQUAL_CURVATURE: &str = concat!(
    "<path d=\"M-9 5Q-7-5-1-5M1 5Q3-5 9-5\"/>",
    "<path d=\"M-2-1H2M-2 2H2\"/>",
);
const CONTINUITY: &str = concat!(
    "<path d=\"M-9 5Q-4 0 0 0Q4 0 9-5M-5 0H5\"/>",
    "<circle r=\"1.5\"/>",
);
const FILLET: &str = "<path d=\"M-8 7H-4A11 11 0 0 1 7-4V-8\"/>";
const EQUAL: &str = "<path d=\"M-7-3H7M-7 3H7\"/>";
const POINT_DISTANCE: &str = concat!(
    "<circle cx=\"-8\" r=\"1.5\"/><circle cx=\"8\" r=\"1.5\"/>",
    "<path d=\"M-5 0H5M-5 0l2-2M-5 0l2 2M5 0L3-2M5 0L3 2\"/>",
);
const SEGMENT_LENGTH: &str = "<path d=\"M-8 2L8-2M-8-2v8M8-6v8M-2-1L0 3M0-3l2 4\"/>";
const RADIUS: &str = "<circle r=\"7\"/><circle r=\"1.2\"/><path d=\"M0 0L5-5M5-5H1M5-5v4\"/>";
const DIAMETER: &str = concat!(
    "<circle r=\"7\"/><path d=\"M-5 5L5-5\"/>",
    "<path d=\"M-5 5h4M-5 5V1M5-5H1M5-5v4\"/>",
);
const ANGLE: &str = concat!(
    "<path d=\"M-8 6H7M-8 6L3-6M-3 6A5 5 0 0 1 0-1\"/>",
    "<path d=\"M-1-1H3v4\"/>",
);
pub(crate) const GEOMETRY_TOOLS: [(&str, EditorTool); 15] = [
    ("select", EditorTool::Select),
    ("point", EditorTool::Point),
    ("line", EditorTool::Line),
    ("polyline", EditorTool::Polyline),
    ("rectangle", EditorTool::Rectangle),
    ("circle", EditorTool::Circle),
    ("arc", EditorTool::CounterClockwiseArc),
    ("quadratic-bezier", EditorTool::QuadraticBezier),
    ("cubic-bezier", EditorTool::CubicBezier),
    ("ellipse", EditorTool::Ellipse),
    ("elliptical-arc", EditorTool::EllipticalArc),
    ("rational-conic", EditorTool::RationalQuadraticConic),
    ("parabola", EditorTool::Parabola),
    ("hyperbola", EditorTool::Hyperbola),
    ("nurbs", EditorTool::Nurbs),
];

pub(crate) const fn geometry_tool_key(tool: EditorTool) -> &'static str {
    match tool {
        EditorTool::Select => "select",
        EditorTool::Point => "point",
        EditorTool::Line => "line",
        EditorTool::Polyline => "polyline",
        EditorTool::Rectangle => "rectangle",
        EditorTool::Circle => "circle",
        EditorTool::CounterClockwiseArc => "arc",
        EditorTool::QuadraticBezier => "quadratic-bezier",
        EditorTool::CubicBezier => "cubic-bezier",
        EditorTool::Ellipse => "ellipse",
        EditorTool::EllipticalArc => "elliptical-arc",
        EditorTool::RationalQuadraticConic => "rational-conic",
        EditorTool::Parabola => "parabola",
        EditorTool::Hyperbola => "hyperbola",
        EditorTool::Nurbs => "nurbs",
    }
}

const fn geometry_tool_fragment(tool: EditorTool) -> &'static str {
    match tool {
        EditorTool::Select => SELECT,
        EditorTool::Point => POINT,
        EditorTool::Line => LINE,
        EditorTool::Polyline => POLYLINE,
        EditorTool::Rectangle => RECTANGLE,
        EditorTool::Circle => CIRCLE,
        EditorTool::CounterClockwiseArc => ARC,
        EditorTool::QuadraticBezier => QUADRATIC_BEZIER,
        EditorTool::CubicBezier => CUBIC_BEZIER,
        EditorTool::Ellipse => ELLIPSE,
        EditorTool::EllipticalArc => ELLIPTICAL_ARC,
        EditorTool::RationalQuadraticConic => RATIONAL_CONIC,
        EditorTool::Parabola => PARABOLA,
        EditorTool::Hyperbola => HYPERBOLA,
        EditorTool::Nurbs => NURBS,
    }
}

pub(crate) fn geometry_tool_icon_markup(tool: EditorTool) -> String {
    let key = geometry_tool_key(tool);
    let fragment = geometry_tool_fragment(tool);
    format!(
        "<svg class=\"wb-palette-icon\" viewBox=\"-10 -10 20 20\" aria-hidden=\"true\" focusable=\"false\" data-icon-key=\"geometry-{key}\">{fragment}</svg>"
    )
}

pub(crate) fn construction_role_icon_markup() -> String {
    format!(
        "<svg class=\"wb-palette-icon\" viewBox=\"-10 -10 20 20\" aria-hidden=\"true\" focusable=\"false\" data-icon-key=\"geometry-role-construction\">{CONSTRUCTION_ROLE}</svg>"
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TreeIconKind {
    DatumOrigin,
    DatumAxis,
    Point,
    Curve,
    Constraint,
    Dimension,
    Feature,
    FeatureCorner,
    External,
}

pub(crate) fn tree_icon_markup(kind: TreeIconKind) -> String {
    let (key, fragment) = match kind {
        TreeIconKind::DatumOrigin => (
            "datum-origin",
            "<circle r=\"3.5\"/><path d=\"M-8 0h16M0-8v16\"/>",
        ),
        TreeIconKind::DatumAxis => (
            "datum-axis",
            "<path d=\"M-8 0H8M5-3l3 3-3 3\"/><circle r=\"1.5\"/>",
        ),
        TreeIconKind::Point => ("point", POINT),
        TreeIconKind::Curve => (
            "curve",
            "<path d=\"M-8 4Q0-6 8 2\"/><circle cx=\"-8\" cy=\"4\" r=\"1.2\"/><circle cx=\"8\" cy=\"2\" r=\"1.2\"/>",
        ),
        TreeIconKind::Constraint => (
            "constraint",
            "<circle cx=\"-5\" r=\"2.3\"/><circle cx=\"5\" r=\"2.3\"/><path d=\"M-2.7 0H2.7\"/>",
        ),
        TreeIconKind::Dimension => ("dimension", POINT_DISTANCE),
        TreeIconKind::Feature => (
            "feature",
            "<path d=\"M-7 5A9 9 0 001-5\"/><path d=\"M1-5H7V1\"/><circle cx=\"-7\" cy=\"5\" r=\"1.3\"/><circle cx=\"7\" cy=\"1\" r=\"1.3\"/>",
        ),
        TreeIconKind::FeatureCorner => (
            "feature-corner",
            "<path d=\"M-7 6V-5H5\"/><path d=\"M-2 6A8 8 0 005-1\"/>",
        ),
        TreeIconKind::External => (
            "external",
            "<rect x=\"-7\" y=\"-6\" width=\"11\" height=\"12\" stroke-dasharray=\"2 2\"/><path d=\"M-1 0H8M5-3L8 0 5 3\"/>",
        ),
    };
    format!(
        "<svg class=\"wb-tree-symbol\" viewBox=\"-10 -10 20 20\" aria-hidden=\"true\" focusable=\"false\" data-tree-icon=\"{key}\">{fragment}</svg>"
    )
}

pub(crate) const PROBLEM_ICON: &str = "<path class=\"wb-error-marker-icon\" d=\"M0-6V1M0 5v.2\"/>";

pub(crate) const fn constraint_icon_key(glyph: SceneConstraintGlyph) -> &'static str {
    match glyph {
        SceneConstraintGlyph::Fixed => "fixed",
        SceneConstraintGlyph::Coincident => "coincident",
        SceneConstraintGlyph::Horizontal => "horizontal",
        SceneConstraintGlyph::Vertical => "vertical",
        SceneConstraintGlyph::PointOnCurve => "point-on-curve",
        SceneConstraintGlyph::Parallel => "parallel",
        SceneConstraintGlyph::Perpendicular => "perpendicular",
        SceneConstraintGlyph::Concentric => "concentric",
        SceneConstraintGlyph::Collinear => "collinear",
        SceneConstraintGlyph::EqualLength => "equal-length",
        SceneConstraintGlyph::EqualRadius => "equal-radius",
        SceneConstraintGlyph::Midpoint => "midpoint",
        SceneConstraintGlyph::Symmetry => "symmetry",
        SceneConstraintGlyph::Contact => "generic-contact",
        SceneConstraintGlyph::Tangency => "tangency",
        SceneConstraintGlyph::Direction => "curve-direction",
        SceneConstraintGlyph::Normal => "normal",
        SceneConstraintGlyph::EqualCurvature => "equal-curvature",
        SceneConstraintGlyph::Continuity => "continuity",
        SceneConstraintGlyph::Fillet => "fillet",
    }
}

pub(crate) const fn constraint_icon_fragment(glyph: SceneConstraintGlyph) -> &'static str {
    match glyph {
        SceneConstraintGlyph::Fixed => FIXED,
        SceneConstraintGlyph::Coincident => COINCIDENT,
        SceneConstraintGlyph::Horizontal => HORIZONTAL,
        SceneConstraintGlyph::Vertical => VERTICAL,
        SceneConstraintGlyph::PointOnCurve => POINT_ON_CURVE,
        SceneConstraintGlyph::Parallel => PARALLEL,
        SceneConstraintGlyph::Perpendicular => PERPENDICULAR,
        SceneConstraintGlyph::Concentric => CONCENTRIC,
        SceneConstraintGlyph::Collinear => COLLINEAR,
        SceneConstraintGlyph::EqualLength => EQUAL_LENGTH,
        SceneConstraintGlyph::EqualRadius => EQUAL_RADIUS,
        SceneConstraintGlyph::Midpoint => MIDPOINT,
        SceneConstraintGlyph::Symmetry => SYMMETRY,
        SceneConstraintGlyph::Contact => CONTACT,
        SceneConstraintGlyph::Tangency => TANGENCY,
        SceneConstraintGlyph::Direction => DIRECTION,
        SceneConstraintGlyph::Normal => NORMAL,
        SceneConstraintGlyph::EqualCurvature => EQUAL_CURVATURE,
        SceneConstraintGlyph::Continuity => CONTINUITY,
        SceneConstraintGlyph::Fillet => FILLET,
    }
}

pub(crate) fn authoring_icon_markup(tool: AuthoringTool) -> String {
    let (key, fragment) = match tool {
        AuthoringTool::Constraint(intent) => constraint_intent_icon(intent),
        AuthoringTool::Dimension(kind) => dimension_icon(kind),
    };
    format!(
        "<svg class=\"wb-palette-icon\" viewBox=\"-10 -10 20 20\" aria-hidden=\"true\" focusable=\"false\" data-icon-key=\"{key}\">{fragment}</svg>"
    )
}

pub(crate) fn feature_icon_markup(tool: FeatureAuthoringTool) -> String {
    let (key, fragment) = match tool {
        FeatureAuthoringTool::Fillet => ("fillet", FILLET),
    };
    format!(
        "<svg class=\"wb-palette-icon\" viewBox=\"-10 -10 20 20\" aria-hidden=\"true\" focusable=\"false\" data-icon-key=\"feature-{key}\">{fragment}</svg>"
    )
}

const fn constraint_intent_icon(intent: ConstraintIntent) -> (&'static str, &'static str) {
    match intent {
        ConstraintIntent::Lock => ("lock", FIXED),
        ConstraintIntent::Coincident => ("coincident", COINCIDENT),
        ConstraintIntent::Horizontal => ("horizontal", HORIZONTAL),
        ConstraintIntent::Vertical => ("vertical", VERTICAL),
        ConstraintIntent::Parallel => ("parallel", PARALLEL),
        ConstraintIntent::Perpendicular => ("perpendicular", PERPENDICULAR),
        ConstraintIntent::Equal => ("equal", EQUAL),
        ConstraintIntent::Midpoint => ("midpoint", MIDPOINT),
        ConstraintIntent::Symmetric => ("symmetric", SYMMETRY),
        ConstraintIntent::Tangent => ("tangent", TANGENCY),
        ConstraintIntent::Continuity => ("continuity", CONTINUITY),
        ConstraintIntent::Concentric => ("concentric", CONCENTRIC),
        ConstraintIntent::Collinear => ("collinear", COLLINEAR),
    }
}

const fn dimension_icon(kind: DimensionKind) -> (&'static str, &'static str) {
    match kind {
        DimensionKind::PointDistance => ("point-distance", POINT_DISTANCE),
        DimensionKind::SegmentLength => ("segment-length", SEGMENT_LENGTH),
        DimensionKind::Radius => ("radius", RADIUS),
        DimensionKind::Diameter => ("diameter", DIAMETER),
        DimensionKind::OrientedAngle => ("oriented-angle", ANGLE),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use geosolve_constraint_editor::{
        AuthoringTool, ConstraintIntent, DimensionKind, EditorTool, FeatureAuthoringTool,
        SceneConstraintGlyph,
    };

    use super::{
        GEOMETRY_TOOLS, TreeIconKind, authoring_icon_markup, constraint_icon_fragment,
        constraint_icon_key, construction_role_icon_markup, feature_icon_markup,
        geometry_tool_icon_markup, geometry_tool_key, tree_icon_markup,
    };

    #[test]
    fn every_geometry_tool_has_a_distinct_text_free_vector_symbol() {
        assert_eq!(GEOMETRY_TOOLS.len(), 15);
        assert_eq!(
            GEOMETRY_TOOLS
                .iter()
                .map(|(key, _)| *key)
                .collect::<HashSet<_>>()
                .len(),
            GEOMETRY_TOOLS.len()
        );
        let markup = GEOMETRY_TOOLS
            .iter()
            .map(|(key, tool)| {
                assert_eq!(*key, geometry_tool_key(*tool));
                geometry_tool_icon_markup(*tool)
            })
            .collect::<Vec<_>>();
        assert_eq!(markup.iter().collect::<HashSet<_>>().len(), markup.len());
        for icon in markup {
            assert!(icon.starts_with("<svg class=\"wb-palette-icon\""));
            assert!(icon.contains("data-icon-key=\"geometry-"));
            assert!(icon.contains("viewBox=\"-10 -10 20 20\""));
            assert!(icon.contains("aria-hidden=\"true\""));
            assert!(!icon.contains("<text"));
        }
        assert_eq!(geometry_tool_key(EditorTool::Nurbs), "nurbs");
    }

    #[test]
    fn tree_object_categories_have_distinct_text_free_vector_symbols() {
        let markup = [
            TreeIconKind::DatumOrigin,
            TreeIconKind::DatumAxis,
            TreeIconKind::Point,
            TreeIconKind::Curve,
            TreeIconKind::Constraint,
            TreeIconKind::Dimension,
            TreeIconKind::External,
        ]
        .map(tree_icon_markup);
        assert_eq!(markup.iter().collect::<HashSet<_>>().len(), markup.len());
        for icon in markup {
            assert!(icon.starts_with("<svg class=\"wb-tree-symbol\""));
            assert!(icon.contains("data-tree-icon=\""));
            assert!(icon.contains("aria-hidden=\"true\""));
            assert!(!icon.contains("<text"));
        }
    }

    #[test]
    fn every_canvas_constraint_has_a_distinct_text_free_vector_symbol() {
        let glyphs = [
            SceneConstraintGlyph::Fixed,
            SceneConstraintGlyph::Coincident,
            SceneConstraintGlyph::Horizontal,
            SceneConstraintGlyph::Vertical,
            SceneConstraintGlyph::PointOnCurve,
            SceneConstraintGlyph::Parallel,
            SceneConstraintGlyph::Perpendicular,
            SceneConstraintGlyph::Concentric,
            SceneConstraintGlyph::Collinear,
            SceneConstraintGlyph::EqualLength,
            SceneConstraintGlyph::EqualRadius,
            SceneConstraintGlyph::Midpoint,
            SceneConstraintGlyph::Symmetry,
            SceneConstraintGlyph::Contact,
            SceneConstraintGlyph::Tangency,
            SceneConstraintGlyph::Direction,
            SceneConstraintGlyph::Normal,
            SceneConstraintGlyph::EqualCurvature,
            SceneConstraintGlyph::Continuity,
            SceneConstraintGlyph::Fillet,
        ];
        assert_eq!(
            glyphs
                .iter()
                .map(|glyph| constraint_icon_key(*glyph))
                .collect::<HashSet<_>>()
                .len(),
            glyphs.len(),
        );
        assert_eq!(
            glyphs
                .iter()
                .map(|glyph| constraint_icon_fragment(*glyph))
                .collect::<HashSet<_>>()
                .len(),
            glyphs.len(),
        );
        for glyph in glyphs {
            let fragment = constraint_icon_fragment(glyph);
            assert!(!fragment.is_empty());
            assert!(!fragment.contains("<text"));
            assert!(fragment.contains("<path") || fragment.contains("<circle"));
        }
    }

    #[test]
    fn complete_authoring_palette_uses_accessible_shared_svg_markup() {
        let tools = [
            AuthoringTool::Constraint(ConstraintIntent::Lock),
            AuthoringTool::Constraint(ConstraintIntent::Coincident),
            AuthoringTool::Constraint(ConstraintIntent::Horizontal),
            AuthoringTool::Constraint(ConstraintIntent::Vertical),
            AuthoringTool::Constraint(ConstraintIntent::Concentric),
            AuthoringTool::Constraint(ConstraintIntent::Collinear),
            AuthoringTool::Constraint(ConstraintIntent::Parallel),
            AuthoringTool::Constraint(ConstraintIntent::Perpendicular),
            AuthoringTool::Constraint(ConstraintIntent::Equal),
            AuthoringTool::Constraint(ConstraintIntent::Midpoint),
            AuthoringTool::Constraint(ConstraintIntent::Symmetric),
            AuthoringTool::Constraint(ConstraintIntent::Tangent),
            AuthoringTool::Constraint(ConstraintIntent::Continuity),
            AuthoringTool::Dimension(DimensionKind::PointDistance),
            AuthoringTool::Dimension(DimensionKind::SegmentLength),
            AuthoringTool::Dimension(DimensionKind::Radius),
            AuthoringTool::Dimension(DimensionKind::Diameter),
            AuthoringTool::Dimension(DimensionKind::OrientedAngle),
        ];
        let markup = tools
            .iter()
            .map(|tool| authoring_icon_markup(*tool))
            .collect::<Vec<_>>();
        assert_eq!(markup.iter().collect::<HashSet<_>>().len(), tools.len());
        for icon in markup {
            assert!(icon.starts_with("<svg class=\"wb-palette-icon\""));
            assert!(icon.contains("viewBox=\"-10 -10 20 20\""));
            assert!(icon.contains("aria-hidden=\"true\""));
            assert!(!icon.contains("<text"));
        }
    }

    #[test]
    fn fillet_modify_tool_has_a_text_free_vector_symbol() {
        let icon = feature_icon_markup(FeatureAuthoringTool::Fillet);
        assert!(icon.starts_with("<svg class=\"wb-palette-icon\""));
        assert!(icon.contains("data-icon-key=\"feature-fillet\""));
        assert!(icon.contains("aria-hidden=\"true\""));
        assert!(!icon.contains("<text"));

        let construction = construction_role_icon_markup();
        assert!(construction.starts_with("<svg class=\"wb-palette-icon\""));
        assert!(construction.contains("data-icon-key=\"geometry-role-construction\""));
        assert!(construction.contains("stroke-dasharray=\"3 2\""));
        assert!(!construction.contains("<text"));
    }

    #[test]
    fn palette_and_canvas_share_the_same_basic_constraint_language() {
        for (intent, glyph) in [
            (ConstraintIntent::Lock, SceneConstraintGlyph::Fixed),
            (
                ConstraintIntent::Coincident,
                SceneConstraintGlyph::Coincident,
            ),
            (
                ConstraintIntent::Horizontal,
                SceneConstraintGlyph::Horizontal,
            ),
            (ConstraintIntent::Vertical, SceneConstraintGlyph::Vertical),
            (
                ConstraintIntent::Concentric,
                SceneConstraintGlyph::Concentric,
            ),
            (ConstraintIntent::Collinear, SceneConstraintGlyph::Collinear),
            (ConstraintIntent::Parallel, SceneConstraintGlyph::Parallel),
            (
                ConstraintIntent::Perpendicular,
                SceneConstraintGlyph::Perpendicular,
            ),
            (ConstraintIntent::Midpoint, SceneConstraintGlyph::Midpoint),
            (ConstraintIntent::Symmetric, SceneConstraintGlyph::Symmetry),
            (ConstraintIntent::Tangent, SceneConstraintGlyph::Tangency),
            (
                ConstraintIntent::Continuity,
                SceneConstraintGlyph::Continuity,
            ),
        ] {
            assert!(
                authoring_icon_markup(AuthoringTool::Constraint(intent))
                    .contains(constraint_icon_fragment(glyph))
            );
        }
    }
}
