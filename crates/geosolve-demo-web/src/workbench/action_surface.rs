// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use geosolve_constraint_editor::{ConstraintIntent, DimensionKind};

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

pub(crate) fn dimension_key(kind: DimensionKind) -> &'static str {
    DIMENSION_ACTIONS
        .iter()
        .find_map(|(key, _, candidate)| (*candidate == kind).then_some(*key))
        .expect("complete dimension action catalog")
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use geosolve_constraint_editor::{ConstraintIntent, DimensionKind};

    use super::{
        CONSTRAINT_ACTIONS, DIMENSION_ACTIONS, constraint_from_key, dimension_from_key,
        dimension_key,
    };

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
    }
}
