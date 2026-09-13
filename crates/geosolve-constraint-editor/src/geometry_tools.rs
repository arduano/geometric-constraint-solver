// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact geometry-authoring tool identities and their legacy projections.

use crate::EditorTool;
use geosolve_sketch_intent::GeometryRecipeKind;

/// Closed native geometry inventory shared with host catalog projections.
///
/// This callback macro is an implementation seam for generated adapters; native
/// consumers should use [`GeometryToolVariant`] and [`GeometryToolFamily`].
#[doc(hidden)]
#[macro_export]
macro_rules! geometry_tool_catalog {
    ($consumer:ident) => {
        $consumer! {
            Point => ("point", SketchPoint) {
                SketchPoint => "sketch-point",
            }
            Lines => ("lines", Segment) {
                Segment => "segment",
                Polyline => "polyline",
                MidpointLine => "midpoint-line",
            }
            Rectangles => ("rectangles", TwoPointAlignedRectangle) {
                TwoPointAlignedRectangle => "two-point-aligned-rectangle",
                ThreePointCornerRectangle => "three-point-corner-rectangle",
                CenterRectangle => "center-rectangle",
                ThreePointCenterRectangle => "three-point-center-rectangle",
            }
            Circles => ("circles", CenterRadiusCircle) {
                CenterRadiusCircle => "center-radius-circle",
                TwoPointDiameterCircle => "two-point-diameter-circle",
                ThreePointCircle => "three-point-circle",
            }
            Arcs => ("arcs", CenterArc) {
                CenterArc => "center-arc",
                ThreePointArc => "three-point-arc",
                TangentArc => "tangent-arc",
            }
            Ellipses => ("ellipses", CenterAxesEllipse) {
                CenterAxesEllipse => "center-axes-ellipse",
                AxisEndpointsEllipse => "axis-endpoints-ellipse",
                CenterAxesEllipticalArc => "center-axes-elliptical-arc",
                AxisEndpointsEllipticalArc => "axis-endpoints-elliptical-arc",
            }
            Beziers => ("beziers", QuadraticBezier) {
                QuadraticBezier => "quadratic-bezier",
                CubicBezier => "cubic-bezier",
            }
            Conics => ("conics", RationalQuadraticConic) {
                RationalQuadraticConic => "rational-quadratic-conic",
                Parabola => "parabola",
                Hyperbola => "hyperbola",
            }
            Splines => ("splines", OpenControlNurbs) {
                OpenControlNurbs => "open-control-nurbs",
                PeriodicControlNurbs => "periodic-control-nurbs",
            }
        }
    };
}

macro_rules! define_geometry_tools {
    ($( $family:ident => ($family_key:literal, $default:ident) {
        $( $variant:ident => $key:literal, )*
    } )*) => {
        /// A stable palette family for related geometry-authoring recipes.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[non_exhaustive]
        pub enum GeometryToolFamily { $( $family, )* }

        /// An exact geometry-authoring recipe.
        ///
        /// [`EditorTool`] remains the coarse compatibility projection. New hosts
        /// retain this identity through presentation, drafting and publication.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[non_exhaustive]
        pub enum GeometryToolVariant { $( $( $variant, )* )* }

        impl GeometryToolFamily {
            /// Complete family inventory in stable palette order.
            pub const ALL: [Self; [$(stringify!($family)),*].len()] = [$( Self::$family, )*];
            /// Stable family key for persistence and host presentation identity.
            #[must_use]
            pub const fn key(self) -> &'static str {
                match self { $( Self::$family => $family_key, )* }
            }
            /// Exact variants in stable palette order.
            #[must_use]
            pub const fn variants(self) -> &'static [GeometryToolVariant] {
                match self { $( Self::$family => &[$( GeometryToolVariant::$variant, )*], )* }
            }
            /// Variant selected when the host activates this family.
            #[must_use]
            pub const fn default_variant(self) -> GeometryToolVariant {
                match self { $( Self::$family => GeometryToolVariant::$default, )* }
            }
        }
        impl GeometryToolVariant {
            /// Complete recipe inventory in stable palette order.
            pub const ALL: [Self; [$( $(stringify!($variant),)* )*].len()] = [$( $( Self::$variant, )* )*];
            /// Stable globally unique recipe key.
            #[must_use]
            pub const fn key(self) -> &'static str {
                match self { $( $( Self::$variant => $key, )* )* }
            }
            /// Palette family containing this recipe.
            #[must_use]
            pub const fn family(self) -> GeometryToolFamily {
                match self { $( $( Self::$variant => GeometryToolFamily::$family, )* )* }
            }
            /// Canonical persistent Intent recipe implemented by this authoring tool.
            #[must_use]
            pub const fn intent_recipe(self) -> GeometryRecipeKind {
                match self { $( $( Self::$variant => GeometryRecipeKind::$variant, )* )* }
            }

            /// Construction adapter for a persistent Intent recipe.
            ///
            /// Non-rational B-splines share NURBS drafting stages. Their caller
            /// must restore the original recipe and remove weights/gauge state.
            #[must_use]
            pub const fn construction_variant_for_intent_recipe(recipe: GeometryRecipeKind) -> Self {
                match recipe {
                    $( $( GeometryRecipeKind::$variant => Self::$variant, )* )*
                    GeometryRecipeKind::OpenControlBSpline => Self::OpenControlNurbs,
                    GeometryRecipeKind::PeriodicControlBSpline => Self::PeriodicControlNurbs,
                }
            }
        }
    };
}
geometry_tool_catalog!(define_geometry_tools);

impl GeometryToolVariant {
    /// Coarse compatibility projection used by the pre-M78 editor API.
    #[must_use]
    pub const fn editor_tool(self) -> EditorTool {
        match self {
            Self::SketchPoint => EditorTool::Point,
            Self::Segment | Self::MidpointLine => EditorTool::Line,
            Self::Polyline => EditorTool::Polyline,
            Self::TwoPointAlignedRectangle
            | Self::ThreePointCornerRectangle
            | Self::CenterRectangle
            | Self::ThreePointCenterRectangle => EditorTool::Rectangle,
            Self::CenterRadiusCircle | Self::TwoPointDiameterCircle | Self::ThreePointCircle => {
                EditorTool::Circle
            }
            Self::CenterArc | Self::ThreePointArc | Self::TangentArc => {
                EditorTool::CounterClockwiseArc
            }
            Self::CenterAxesEllipse | Self::AxisEndpointsEllipse => EditorTool::Ellipse,
            Self::CenterAxesEllipticalArc | Self::AxisEndpointsEllipticalArc => {
                EditorTool::EllipticalArc
            }
            Self::QuadraticBezier => EditorTool::QuadraticBezier,
            Self::CubicBezier => EditorTool::CubicBezier,
            Self::RationalQuadraticConic => EditorTool::RationalQuadraticConic,
            Self::Parabola => EditorTool::Parabola,
            Self::Hyperbola => EditorTool::Hyperbola,
            Self::OpenControlNurbs | Self::PeriodicControlNurbs => EditorTool::Nurbs,
        }
    }

    pub(crate) const fn default_for_editor_tool(tool: EditorTool) -> Option<Self> {
        match tool {
            EditorTool::Select => None,
            EditorTool::Point => Some(Self::SketchPoint),
            EditorTool::Line => Some(Self::Segment),
            EditorTool::Polyline => Some(Self::Polyline),
            EditorTool::Rectangle => Some(Self::TwoPointAlignedRectangle),
            EditorTool::Circle => Some(Self::CenterRadiusCircle),
            EditorTool::CounterClockwiseArc => Some(Self::CenterArc),
            EditorTool::QuadraticBezier => Some(Self::QuadraticBezier),
            EditorTool::CubicBezier => Some(Self::CubicBezier),
            EditorTool::Ellipse => Some(Self::CenterAxesEllipse),
            EditorTool::EllipticalArc => Some(Self::CenterAxesEllipticalArc),
            EditorTool::RationalQuadraticConic => Some(Self::RationalQuadraticConic),
            EditorTool::Parabola => Some(Self::Parabola),
            EditorTool::Hyperbola => Some(Self::Hyperbola),
            EditorTool::Nurbs => Some(Self::OpenControlNurbs),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::ConstraintEditor;

    #[test]
    fn exact_catalog_has_nine_families_and_twenty_five_unique_variants() {
        assert_eq!(GeometryToolFamily::ALL.len(), 9);
        assert_eq!(GeometryToolVariant::ALL.len(), 25);

        let mut family_keys = BTreeSet::new();
        let mut variant_keys = BTreeSet::new();
        let mut catalog_variants = Vec::new();
        for family in GeometryToolFamily::ALL {
            assert!(family_keys.insert(family.key()));
            assert!(family.variants().contains(&family.default_variant()));
            for &variant in family.variants() {
                assert_eq!(variant.family(), family);
                assert!(variant_keys.insert(variant.key()));
                catalog_variants.push(variant);
            }
        }

        assert_eq!(catalog_variants, GeometryToolVariant::ALL);
    }

    #[test]
    fn every_variant_has_a_non_select_legacy_projection() {
        for variant in GeometryToolVariant::ALL {
            assert_ne!(variant.editor_tool(), EditorTool::Select);
        }
    }

    #[test]
    fn intent_recipe_construction_adapter_declares_its_only_lossy_pairs() {
        for recipe in GeometryRecipeKind::ALL {
            let variant = GeometryToolVariant::construction_variant_for_intent_recipe(recipe);
            match recipe {
                GeometryRecipeKind::OpenControlBSpline => {
                    assert_eq!(variant, GeometryToolVariant::OpenControlNurbs);
                    assert_eq!(
                        variant.intent_recipe(),
                        GeometryRecipeKind::OpenControlNurbs
                    );
                }
                GeometryRecipeKind::PeriodicControlBSpline => {
                    assert_eq!(variant, GeometryToolVariant::PeriodicControlNurbs);
                    assert_eq!(
                        variant.intent_recipe(),
                        GeometryRecipeKind::PeriodicControlNurbs
                    );
                }
                _ => assert_eq!(variant.intent_recipe(), recipe),
            }
        }
    }

    #[test]
    fn stable_keys_freeze_the_public_palette_identity() {
        assert_eq!(
            GeometryToolFamily::ALL.map(GeometryToolFamily::key),
            [
                "point",
                "lines",
                "rectangles",
                "circles",
                "arcs",
                "ellipses",
                "beziers",
                "conics",
                "splines",
            ]
        );
        assert_eq!(
            GeometryToolVariant::ALL.map(GeometryToolVariant::key),
            [
                "sketch-point",
                "segment",
                "polyline",
                "midpoint-line",
                "two-point-aligned-rectangle",
                "three-point-corner-rectangle",
                "center-rectangle",
                "three-point-center-rectangle",
                "center-radius-circle",
                "two-point-diameter-circle",
                "three-point-circle",
                "center-arc",
                "three-point-arc",
                "tangent-arc",
                "center-axes-ellipse",
                "axis-endpoints-ellipse",
                "center-axes-elliptical-arc",
                "axis-endpoints-elliptical-arc",
                "quadratic-bezier",
                "cubic-bezier",
                "rational-quadratic-conic",
                "parabola",
                "hyperbola",
                "open-control-nurbs",
                "periodic-control-nurbs",
            ]
        );
    }

    #[test]
    fn exact_activation_preserves_variant_and_legacy_activation_uses_default() {
        let mut editor = ConstraintEditor::default();
        assert_eq!(editor.geometry_tool_variant(), None);

        editor.activate_geometry_tool(GeometryToolVariant::ThreePointCenterRectangle);
        assert_eq!(editor.tool(), EditorTool::Rectangle);
        assert_eq!(
            editor.geometry_tool_variant(),
            Some(GeometryToolVariant::ThreePointCenterRectangle)
        );

        editor.activate_tool(EditorTool::Rectangle);
        assert_eq!(
            editor.geometry_tool_variant(),
            Some(GeometryToolVariant::TwoPointAlignedRectangle)
        );

        editor.activate_geometry_tool(GeometryToolVariant::PeriodicControlNurbs);
        assert_eq!(editor.tool(), EditorTool::Nurbs);
        assert_eq!(
            editor.geometry_tool_variant(),
            Some(GeometryToolVariant::PeriodicControlNurbs)
        );

        editor.activate_tool(EditorTool::Select);
        assert_eq!(editor.geometry_tool_variant(), None);
    }
}
