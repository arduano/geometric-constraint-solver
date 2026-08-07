// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use geosolve_constraint_editor::{
    AuthoringTool, ConstraintIntent, DimensionKind, FeatureAuthoringTool,
};

pub(crate) const CONSTRAINT_ACTIONS: [(&str, &str, ConstraintIntent); 11] = [
    ("lock", "Lock", ConstraintIntent::Lock),
    ("coincident", "Coincident", ConstraintIntent::Coincident),
    ("horizontal", "Horizontal", ConstraintIntent::Horizontal),
    ("vertical", "Vertical", ConstraintIntent::Vertical),
    ("parallel", "Parallel", ConstraintIntent::Parallel),
    (
        "perpendicular",
        "Perpendicular / Normal",
        ConstraintIntent::Perpendicular,
    ),
    ("equal", "Equal", ConstraintIntent::Equal),
    ("midpoint", "Midpoint", ConstraintIntent::Midpoint),
    ("symmetric", "Symmetric", ConstraintIntent::Symmetric),
    ("tangent", "Tangent", ConstraintIntent::Tangent),
    ("continuity", "Continuity", ConstraintIntent::Continuity),
];

pub(crate) const DIMENSION_ACTIONS: [(&str, &str, DimensionKind); 5] = [
    (
        "point-distance",
        "Point distance",
        DimensionKind::PointDistance,
    ),
    (
        "segment-length",
        "Segment length",
        DimensionKind::SegmentLength,
    ),
    ("radius", "Radius", DimensionKind::Radius),
    ("diameter", "Diameter", DimensionKind::Diameter),
    (
        "oriented-angle",
        "Oriented angle",
        DimensionKind::OrientedAngle,
    ),
];

pub(crate) const FEATURE_ACTIONS: [(&str, &str, FeatureAuthoringTool); 1] =
    [("fillet", "Fillet", FeatureAuthoringTool::Fillet)];

pub(crate) fn constraint_from_key(key: &str) -> Option<ConstraintIntent> {
    CONSTRAINT_ACTIONS
        .iter()
        .find_map(|(candidate, _, kind)| (*candidate == key).then_some(*kind))
}

pub(crate) fn dimension_from_key(key: &str) -> Option<DimensionKind> {
    DIMENSION_ACTIONS
        .iter()
        .find_map(|(candidate, _, kind)| (*candidate == key).then_some(*kind))
}

#[cfg(test)]
pub(crate) fn dimension_key(kind: DimensionKind) -> &'static str {
    DIMENSION_ACTIONS
        .iter()
        .find_map(|(key, _, candidate)| (*candidate == kind).then_some(*key))
        .expect("complete dimension action catalog")
}

pub(crate) fn authoring_tool_from_key(key: &str) -> Option<AuthoringTool> {
    constraint_from_key(key)
        .map(AuthoringTool::Constraint)
        .or_else(|| dimension_from_key(key).map(AuthoringTool::Dimension))
}

pub(crate) fn feature_tool_from_key(key: &str) -> Option<FeatureAuthoringTool> {
    FEATURE_ACTIONS
        .iter()
        .find_map(|(candidate, _, tool)| (*candidate == key).then_some(*tool))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use geosolve_constraint_editor::{ConstraintIntent, DimensionKind, FeatureAuthoringTool};

    use super::{
        CONSTRAINT_ACTIONS, DIMENSION_ACTIONS, FEATURE_ACTIONS, authoring_tool_from_key,
        constraint_from_key, dimension_from_key, dimension_key, feature_tool_from_key,
    };
    use crate::workbench::icons::GEOMETRY_TOOLS;

    #[test]
    fn wasm_action_identity_catalog_is_complete_unique_and_round_trips() {
        let expected_constraints = [
            ConstraintIntent::Lock,
            ConstraintIntent::Coincident,
            ConstraintIntent::Horizontal,
            ConstraintIntent::Vertical,
            ConstraintIntent::Parallel,
            ConstraintIntent::Perpendicular,
            ConstraintIntent::Equal,
            ConstraintIntent::Midpoint,
            ConstraintIntent::Symmetric,
            ConstraintIntent::Tangent,
            ConstraintIntent::Continuity,
        ];
        let expected_dimensions = [
            DimensionKind::PointDistance,
            DimensionKind::SegmentLength,
            DimensionKind::Radius,
            DimensionKind::Diameter,
            DimensionKind::OrientedAngle,
        ];
        assert_eq!(
            CONSTRAINT_ACTIONS
                .iter()
                .map(|(_, _, kind)| *kind)
                .collect::<Vec<_>>(),
            expected_constraints
        );
        assert_eq!(
            DIMENSION_ACTIONS
                .iter()
                .map(|(_, _, kind)| *kind)
                .collect::<Vec<_>>(),
            expected_dimensions
        );
        assert_eq!(
            CONSTRAINT_ACTIONS
                .iter()
                .map(|(key, _, _)| *key)
                .collect::<HashSet<_>>()
                .len(),
            CONSTRAINT_ACTIONS.len()
        );
        for (key, label, kind) in CONSTRAINT_ACTIONS {
            assert!(!label.is_empty());
            assert_eq!(constraint_from_key(key), Some(kind));
        }
        for (key, label, kind) in DIMENSION_ACTIONS {
            assert!(!label.is_empty());
            assert_eq!(dimension_from_key(key), Some(kind));
            assert_eq!(dimension_key(kind), key);
        }
        assert_eq!(constraint_from_key("unknown"), None);
        for retired in [
            "fixed",
            "point-on-curve",
            "equal-length",
            "equal-radius",
            "symmetry",
            "generic-contact",
            "generic-tangency",
        ] {
            assert_eq!(
                constraint_from_key(retired),
                None,
                "{retired} must not survive as a hidden equation-shaped alias"
            );
        }
        assert_eq!(dimension_from_key("unknown"), None);
        for (key, _, intent) in CONSTRAINT_ACTIONS {
            assert_eq!(
                authoring_tool_from_key(key),
                Some(geosolve_constraint_editor::AuthoringTool::Constraint(
                    intent
                ))
            );
        }
        for (key, _, kind) in DIMENSION_ACTIONS {
            assert_eq!(
                authoring_tool_from_key(key),
                Some(geosolve_constraint_editor::AuthoringTool::Dimension(kind))
            );
        }
        assert_eq!(authoring_tool_from_key("unknown"), None);
    }

    #[test]
    fn m66_modify_action_identity_catalog_is_closed_unique_and_headless() {
        assert_eq!(
            FEATURE_ACTIONS.map(|(_, _, tool)| tool),
            [FeatureAuthoringTool::Fillet]
        );
        assert_eq!(
            FEATURE_ACTIONS
                .iter()
                .map(|(key, _, _)| *key)
                .collect::<HashSet<_>>()
                .len(),
            FEATURE_ACTIONS.len()
        );
        for (key, label, tool) in FEATURE_ACTIONS {
            assert!(!label.is_empty());
            assert_eq!(feature_tool_from_key(key), Some(tool));
        }
        assert_eq!(feature_tool_from_key("split"), None);
        assert_eq!(feature_tool_from_key("line-offset"), None);
        assert_eq!(feature_tool_from_key("mirror"), None);
        assert_eq!(feature_tool_from_key("curve-offset"), None);
        assert_eq!(feature_tool_from_key("multi-mirror"), None);
    }

    #[test]
    fn m62_palette_replaces_inspector_creation_without_restoring_a_harness() {
        let html = include_str!("../../index.html");
        for (key, _, _) in CONSTRAINT_ACTIONS {
            assert!(
                html.contains(&format!("data-wb-authoring=\"{key}\"")),
                "missing constraint palette action {key}"
            );
        }
        for (key, _, _) in DIMENSION_ACTIONS {
            assert!(
                html.contains(&format!("data-wb-authoring=\"{key}\"")),
                "missing dimension palette action {key}"
            );
        }
        assert_eq!(
            html.matches("class=\"wb-authoring-icon\"").count(),
            CONSTRAINT_ACTIONS.len() + DIMENSION_ACTIONS.len(),
            "every authoring action needs exactly one shared vector-icon host"
        );
        assert_eq!(
            html.matches("<span class=\"wb-authoring-icon\"").count(),
            CONSTRAINT_ACTIONS.len() + DIMENSION_ACTIONS.len(),
            "authoring vectors are icons, not keyboard hints"
        );
        assert!(!html.contains("<kbd class=\"wb-authoring-icon\""));
        assert_eq!(
            html.matches("class=\"wb-geometry-icon\"").count(),
            GEOMETRY_TOOLS.len(),
            "every geometry tool needs exactly one shared vector-icon host"
        );
        for (key, _) in GEOMETRY_TOOLS {
            assert!(
                html.contains(&format!("data-wb-tool=\"{key}\"")),
                "missing geometry palette action {key}"
            );
        }
        let geometry_markup = html
            .split("<strong>Geometry</strong>")
            .nth(1)
            .and_then(|markup| markup.split("<strong>Constraints</strong>").next())
            .expect("geometry palette section");
        assert!(
            !geometry_markup.contains("<kbd>"),
            "geometry icon slots must not regress to placeholder letters"
        );
        assert!(html.contains("title=\"Perpendicular / normal\""));
        assert!(html.contains("id=\"wb-dimension-target-editor\""));
        assert!(html.contains("class=\"wb-palette-flyout\""));
        for retired in [
            "wb-constraint-kind",
            "wb-dimension-kind",
            "data-wb-action=\"constraint\"",
            "data-wb-action=\"dimension\"",
            "/#/dev/lab",
        ] {
            assert!(
                !html.contains(retired),
                "{retired} must not survive in the M62 workbench"
            );
        }
    }

    #[test]
    fn m66_modify_palette_is_present_without_operation_semantics_in_markup() {
        let html = include_str!("../../index.html");
        for (key, _, _) in FEATURE_ACTIONS {
            assert!(
                html.contains(&format!("data-wb-feature=\"{key}\"")),
                "missing Modify action {key}"
            );
        }
        assert_eq!(
            html.matches("class=\"wb-feature-icon\"").count(),
            FEATURE_ACTIONS.len()
        );
        assert!(html.contains("<strong>Modify</strong>"));
        assert!(html.contains("data-wb-action=\"feature-apply\""));
        assert!(html.contains("id=\"wb-feature-branch-scope\""));
        assert!(html.contains("Branch choices set defaults for the next corner"));
        assert!(html.contains("Select a preview arc to edit one completed corner"));
        for forbidden in [
            "data-wb-feature=\"line-offset\"",
            "data-wb-feature=\"mirror\"",
            "wb-operation-offset-distance",
            "wb-operation-offset-mode",
            "wb-operation-offset-semantics",
            "operation-split",
            "operation-pattern",
            "curve-offset",
            "multi-source-mirror",
            "/#/dev/lab",
        ] {
            assert!(!html.contains(forbidden), "{forbidden} leaked into M66 UI");
        }
    }
}
