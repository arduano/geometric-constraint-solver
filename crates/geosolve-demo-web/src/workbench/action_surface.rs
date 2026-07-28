// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use geosolve_constraint_editor::{ConstraintKind, DimensionKind};

pub(crate) const CONSTRAINT_ACTIONS: [(&str, &str, ConstraintKind); 13] = [
    ("fixed", "Fixed", ConstraintKind::Fixed),
    ("coincident", "Coincident", ConstraintKind::Coincident),
    ("horizontal", "Horizontal", ConstraintKind::Horizontal),
    ("vertical", "Vertical", ConstraintKind::Vertical),
    (
        "point-on-curve",
        "Point on curve",
        ConstraintKind::PointOnCurve,
    ),
    ("parallel", "Parallel", ConstraintKind::Parallel),
    (
        "perpendicular",
        "Perpendicular",
        ConstraintKind::Perpendicular,
    ),
    ("equal-length", "Equal length", ConstraintKind::EqualLength),
    ("equal-radius", "Equal radius", ConstraintKind::EqualRadius),
    ("midpoint", "Midpoint", ConstraintKind::Midpoint),
    ("symmetry", "Symmetry", ConstraintKind::Symmetry),
    (
        "generic-contact",
        "Generic contact",
        ConstraintKind::GenericContact,
    ),
    (
        "generic-tangency",
        "Generic tangency",
        ConstraintKind::GenericTangency,
    ),
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

pub(crate) fn constraint_from_key(key: &str) -> Option<ConstraintKind> {
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

    use geosolve_constraint_editor::{ConstraintKind, DimensionKind};

    use super::{
        CONSTRAINT_ACTIONS, DIMENSION_ACTIONS, constraint_from_key, dimension_from_key,
        dimension_key,
    };

    #[test]
    fn wasm_action_identity_catalog_is_complete_unique_and_round_trips() {
        let expected_constraints = [
            ConstraintKind::Fixed,
            ConstraintKind::Coincident,
            ConstraintKind::Horizontal,
            ConstraintKind::Vertical,
            ConstraintKind::PointOnCurve,
            ConstraintKind::Parallel,
            ConstraintKind::Perpendicular,
            ConstraintKind::EqualLength,
            ConstraintKind::EqualRadius,
            ConstraintKind::Midpoint,
            ConstraintKind::Symmetry,
            ConstraintKind::GenericContact,
            ConstraintKind::GenericTangency,
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
        assert_eq!(dimension_from_key("unknown"), None);
    }
}
