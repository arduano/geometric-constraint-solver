// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use geosolve_constraint_editor::{
    AuthoringTool, ConstraintIntent, DimensionKind, FeatureAuthoringTool, ModifyTool,
};

macro_rules! project_action_surface {
    (constraints { $( $constraint:ident => ($constraint_key:literal, $constraint_label:literal), )* }
     dimensions { $( $dimension:ident => ($dimension_key:literal, $dimension_label:literal), )* }
     modify { $( $modify:ident => ($modify_key:literal, $modify_label:literal), )* }
     auxiliary { $( $auxiliary:ident => ($auxiliary_key:literal, $auxiliary_label:literal, $auxiliary_command:literal), )* }) => {
        pub(crate) const CONSTRAINT_ACTIONS: [(&str, &str, ConstraintIntent); ConstraintIntent::ALL.len()] =
            [$( ($constraint_key, $constraint_label, ConstraintIntent::$constraint), )*];
        pub(crate) const DIMENSION_ACTIONS: [(&str, &str, DimensionKind); DimensionKind::ALL.len()] =
            [$( ($dimension_key, $dimension_label, DimensionKind::$dimension), )*];
    };
}
geosolve_constraint_editor::authoring_tool_catalog!(project_action_surface);

pub(crate) const FEATURE_ACTIONS: [(&str, &str, FeatureAuthoringTool); 1] = [(
    ModifyTool::Fillet.key(),
    ModifyTool::Fillet.label(),
    FeatureAuthoringTool::Fillet,
)];

/// Native Modify actions that deliberately do not share computed-feature authoring.
pub(crate) const OFFSET_ACTIONS: [(&str, &str); 1] =
    [(ModifyTool::Offset.key(), ModifyTool::Offset.label())];

pub(crate) fn constraint_from_key(key: &str) -> Option<ConstraintIntent> {
    ConstraintIntent::from_key(key)
}

pub(crate) fn dimension_from_key(key: &str) -> Option<DimensionKind> {
    DimensionKind::from_key(key)
}

#[cfg(test)]
pub(crate) fn dimension_key(kind: DimensionKind) -> &'static str {
    kind.key()
}

pub(crate) fn authoring_tool_from_key(key: &str) -> Option<AuthoringTool> {
    AuthoringTool::from_key(key)
}

pub(crate) fn feature_tool_from_key(key: &str) -> Option<FeatureAuthoringTool> {
    FEATURE_ACTIONS
        .iter()
        .find_map(|(candidate, _, tool)| (*candidate == key).then_some(*tool))
}

pub(crate) fn is_offset_tool_key(key: &str) -> bool {
    OFFSET_ACTIONS
        .iter()
        .any(|(candidate, _)| *candidate == key)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use geosolve_constraint_editor::{ConstraintIntent, DimensionKind, FeatureAuthoringTool};

    use super::{
        CONSTRAINT_ACTIONS, DIMENSION_ACTIONS, FEATURE_ACTIONS, OFFSET_ACTIONS,
        authoring_tool_from_key, constraint_from_key, dimension_from_key, dimension_key,
        feature_tool_from_key, is_offset_tool_key,
    };

    #[test]
    fn wasm_action_identity_catalog_is_complete_unique_and_round_trips() {
        let expected_constraints = [
            ConstraintIntent::Lock,
            ConstraintIntent::Coincident,
            ConstraintIntent::Horizontal,
            ConstraintIntent::Vertical,
            ConstraintIntent::Concentric,
            ConstraintIntent::Collinear,
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
    fn feature_action_identity_catalog_is_closed_unique_and_headless() {
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
        assert_eq!(feature_tool_from_key("unknown"), None);
        assert_eq!(OFFSET_ACTIONS, [("offset", "Offset")]);
        assert!(is_offset_tool_key("offset"));
        assert!(!is_offset_tool_key("fillet"));
    }
}
