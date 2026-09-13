// SPDX-License-Identifier: GPL-3.0-or-later

//! Native action identities shared by authoring and host projections.

use crate::AuthoringTool;

/// Closed native action inventory; adapters project it without owning another list.
#[doc(hidden)]
#[macro_export]
macro_rules! authoring_tool_catalog {
    ($consumer:ident) => {
        $consumer! {
            constraints {
                Lock => ("lock", "Lock"),
                Coincident => ("coincident", "Coincident"),
                Horizontal => ("horizontal", "Horizontal"),
                Vertical => ("vertical", "Vertical"),
                Concentric => ("concentric", "Concentric"),
                Collinear => ("collinear", "Collinear"),
                Parallel => ("parallel", "Parallel"),
                Perpendicular => ("perpendicular", "Perpendicular / Normal"),
                Equal => ("equal", "Equal"),
                Midpoint => ("midpoint", "Midpoint"),
                Symmetric => ("symmetric", "Symmetric"),
                Tangent => ("tangent", "Tangent"),
                Continuity => ("continuity", "Continuity"),
            }
            dimensions {
                PointDistance => ("point-distance", "Point distance"),
                SegmentLength => ("segment-length", "Segment length"),
                Radius => ("radius", "Radius"),
                Diameter => ("diameter", "Diameter"),
                OrientedAngle => ("oriented-angle", "Oriented angle"),
            }
            modify {
                Fillet => ("fillet", "Fillet"),
                Offset => ("offset", "Offset"),
            }
            auxiliary {
                ToggleGeometryRole => ("geometry-role", "Toggle geometry role", "toggle_geometry_role"),
            }
        }
    };
}

macro_rules! define_authoring_tools {
    (constraints { $( $constraint:ident => ($constraint_key:literal, $constraint_label:literal), )* }
     dimensions { $( $dimension:ident => ($dimension_key:literal, $dimension_label:literal), )* }
     modify { $( $modify:ident => ($modify_key:literal, $modify_label:literal), )* }
     auxiliary { $( $auxiliary:ident => ($auxiliary_key:literal, $auxiliary_label:literal, $auxiliary_command:literal), )* }) => {
        /// Compact selection-sensitive authoring vocabulary.
        ///
        /// An intent is not an equation identity. The headless coordinator resolves
        /// it to a [`crate::ResolvedConstraintKind`] from typed selected operands.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum ConstraintIntent { $( $constraint, )* }

        impl ConstraintIntent {
            /// Complete relation tool inventory in native palette order.
            pub const ALL: [Self; [$(stringify!($constraint)),*].len()] = [$( Self::$constraint, )*];
            #[must_use]
            pub const fn key(self) -> &'static str {
                match self { $( Self::$constraint => $constraint_key, )* }
            }
            #[must_use]
            pub const fn label(self) -> &'static str {
                match self { $( Self::$constraint => $constraint_label, )* }
            }
            #[must_use]
            pub fn from_key(key: &str) -> Option<Self> {
                match key { $( $constraint_key => Some(Self::$constraint), )* _ => None }
            }
        }

        /// Complete dimension action vocabulary.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum DimensionKind { $( $dimension, )* }

        impl DimensionKind {
            /// Complete dimension tool inventory in native palette order.
            pub const ALL: [Self; [$(stringify!($dimension)),*].len()] = [$( Self::$dimension, )*];
            #[must_use]
            pub const fn key(self) -> &'static str {
                match self { $( Self::$dimension => $dimension_key, )* }
            }
            #[must_use]
            pub const fn label(self) -> &'static str {
                match self { $( Self::$dimension => $dimension_label, )* }
            }
            #[must_use]
            pub fn from_key(key: &str) -> Option<Self> {
                match key { $( $dimension_key => Some(Self::$dimension), )* _ => None }
            }
        }

        /// Tool identities whose native state machines are separate from relation
        /// and dimension operand collection. This identity does not unify execution.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum ModifyTool { $( $modify, )* $( $auxiliary, )* }

        impl ModifyTool {
            /// Palette actions followed by contextual actions.
            pub const ALL: [Self; [$(stringify!($modify),)* $(stringify!($auxiliary),)*].len()] = [$( Self::$modify, )* $( Self::$auxiliary, )*];
            /// Actions exposed in the Modify palette.
            pub const PALETTE: [Self; [$(stringify!($modify)),*].len()] = [$( Self::$modify, )*];
            #[must_use]
            pub const fn key(self) -> &'static str {
                match self { $( Self::$modify => $modify_key, )* $( Self::$auxiliary => $auxiliary_key, )* }
            }
            #[must_use]
            pub const fn label(self) -> &'static str {
                match self { $( Self::$modify => $modify_label, )* $( Self::$auxiliary => $auxiliary_label, )* }
            }
            /// Stable command spelling, including contextual actions whose toolbar
            /// identity predates their wire command.
            #[must_use]
            pub fn command_key(self) -> String {
                match self {
                    $( Self::$modify => $modify_key.replace('-', "_"), )*
                    $( Self::$auxiliary => $auxiliary_command.into(), )*
                }
            }
            #[must_use]
            pub fn from_key(key: &str) -> Option<Self> {
                match key { $( $modify_key => Some(Self::$modify), )* $( $auxiliary_key => Some(Self::$auxiliary), )* _ => None }
            }
        }

        impl AuthoringTool {
            /// Complete native relation and dimension palette.
            pub const ALL: [Self; ConstraintIntent::ALL.len() + DimensionKind::ALL.len()] = [$( Self::Constraint(ConstraintIntent::$constraint), )* $( Self::Dimension(DimensionKind::$dimension), )*];
            #[must_use]
            pub const fn key(self) -> &'static str {
                match self { Self::Constraint(tool) => tool.key(), Self::Dimension(tool) => tool.key() }
            }
            #[must_use]
            pub fn from_key(key: &str) -> Option<Self> {
                ConstraintIntent::from_key(key).map(Self::Constraint)
                    .or_else(|| DimensionKind::from_key(key).map(Self::Dimension))
            }
        }
    };
}
authoring_tool_catalog!(define_authoring_tools);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn native_action_catalog_preserves_all_keys_and_contextual_roles() {
        let keys: Vec<_> = AuthoringTool::ALL
            .into_iter()
            .map(AuthoringTool::key)
            .chain(ModifyTool::ALL.into_iter().map(ModifyTool::key))
            .collect();
        assert_eq!(
            keys,
            [
                "lock",
                "coincident",
                "horizontal",
                "vertical",
                "concentric",
                "collinear",
                "parallel",
                "perpendicular",
                "equal",
                "midpoint",
                "symmetric",
                "tangent",
                "continuity",
                "point-distance",
                "segment-length",
                "radius",
                "diameter",
                "oriented-angle",
                "fillet",
                "offset",
                "geometry-role"
            ]
        );
        assert_eq!(keys.iter().collect::<BTreeSet<_>>().len(), keys.len());
        assert_eq!(
            ModifyTool::PALETTE,
            [ModifyTool::Fillet, ModifyTool::Offset]
        );
        for tool in AuthoringTool::ALL {
            assert_eq!(AuthoringTool::from_key(tool.key()), Some(tool));
        }
        for tool in ModifyTool::ALL {
            assert_eq!(ModifyTool::from_key(tool.key()), Some(tool));
        }
        assert_eq!(AuthoringTool::from_key("geometry-role"), None);
        assert_eq!(ModifyTool::from_key("unknown"), None);
    }
}
