// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact geometry-authoring tool identities and their legacy projections.

use crate::EditorTool;

/// A stable palette family for related geometry-authoring recipes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeometryToolFamily {
    Point,
    Lines,
    Rectangles,
    Circles,
    Arcs,
    Ellipses,
    Beziers,
    Conics,
    Splines,
}

/// An exact geometry-authoring recipe.
///
/// [`EditorTool`] remains the coarse compatibility projection. New hosts should
/// retain this identity so variants that share one legacy implementation do not
/// become indistinguishable in presentation or drafting state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeometryToolVariant {
    SketchPoint,
    Segment,
    Polyline,
    MidpointLine,
    TwoPointAlignedRectangle,
    ThreePointCornerRectangle,
    CenterRectangle,
    ThreePointCenterRectangle,
    CenterRadiusCircle,
    TwoPointDiameterCircle,
    ThreePointCircle,
    CenterArc,
    ThreePointArc,
    TangentArc,
    CenterAxesEllipse,
    AxisEndpointsEllipse,
    CenterAxesEllipticalArc,
    AxisEndpointsEllipticalArc,
    QuadraticBezier,
    CubicBezier,
    RationalQuadraticConic,
    Parabola,
    Hyperbola,
    OpenControlNurbs,
    PeriodicControlNurbs,
}

const POINT_VARIANTS: [GeometryToolVariant; 1] = [GeometryToolVariant::SketchPoint];
const LINE_VARIANTS: [GeometryToolVariant; 3] = [
    GeometryToolVariant::Segment,
    GeometryToolVariant::Polyline,
    GeometryToolVariant::MidpointLine,
];
const RECTANGLE_VARIANTS: [GeometryToolVariant; 4] = [
    GeometryToolVariant::TwoPointAlignedRectangle,
    GeometryToolVariant::ThreePointCornerRectangle,
    GeometryToolVariant::CenterRectangle,
    GeometryToolVariant::ThreePointCenterRectangle,
];
const CIRCLE_VARIANTS: [GeometryToolVariant; 3] = [
    GeometryToolVariant::CenterRadiusCircle,
    GeometryToolVariant::TwoPointDiameterCircle,
    GeometryToolVariant::ThreePointCircle,
];
const ARC_VARIANTS: [GeometryToolVariant; 3] = [
    GeometryToolVariant::CenterArc,
    GeometryToolVariant::ThreePointArc,
    GeometryToolVariant::TangentArc,
];
const ELLIPSE_VARIANTS: [GeometryToolVariant; 4] = [
    GeometryToolVariant::CenterAxesEllipse,
    GeometryToolVariant::AxisEndpointsEllipse,
    GeometryToolVariant::CenterAxesEllipticalArc,
    GeometryToolVariant::AxisEndpointsEllipticalArc,
];
const BEZIER_VARIANTS: [GeometryToolVariant; 2] = [
    GeometryToolVariant::QuadraticBezier,
    GeometryToolVariant::CubicBezier,
];
const CONIC_VARIANTS: [GeometryToolVariant; 3] = [
    GeometryToolVariant::RationalQuadraticConic,
    GeometryToolVariant::Parabola,
    GeometryToolVariant::Hyperbola,
];
const SPLINE_VARIANTS: [GeometryToolVariant; 2] = [
    GeometryToolVariant::OpenControlNurbs,
    GeometryToolVariant::PeriodicControlNurbs,
];

impl GeometryToolFamily {
    /// Complete family inventory in stable palette order.
    pub const ALL: [Self; 9] = [
        Self::Point,
        Self::Lines,
        Self::Rectangles,
        Self::Circles,
        Self::Arcs,
        Self::Ellipses,
        Self::Beziers,
        Self::Conics,
        Self::Splines,
    ];

    /// Stable family key for persistence and host presentation identity.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Point => "point",
            Self::Lines => "lines",
            Self::Rectangles => "rectangles",
            Self::Circles => "circles",
            Self::Arcs => "arcs",
            Self::Ellipses => "ellipses",
            Self::Beziers => "beziers",
            Self::Conics => "conics",
            Self::Splines => "splines",
        }
    }

    /// Exact variants in stable palette order.
    #[must_use]
    pub const fn variants(self) -> &'static [GeometryToolVariant] {
        match self {
            Self::Point => &POINT_VARIANTS,
            Self::Lines => &LINE_VARIANTS,
            Self::Rectangles => &RECTANGLE_VARIANTS,
            Self::Circles => &CIRCLE_VARIANTS,
            Self::Arcs => &ARC_VARIANTS,
            Self::Ellipses => &ELLIPSE_VARIANTS,
            Self::Beziers => &BEZIER_VARIANTS,
            Self::Conics => &CONIC_VARIANTS,
            Self::Splines => &SPLINE_VARIANTS,
        }
    }

    /// Variant selected when a host activates only this family or its legacy
    /// [`EditorTool`] projection.
    #[must_use]
    pub const fn default_variant(self) -> GeometryToolVariant {
        match self {
            Self::Point => GeometryToolVariant::SketchPoint,
            Self::Lines => GeometryToolVariant::Segment,
            Self::Rectangles => GeometryToolVariant::TwoPointAlignedRectangle,
            Self::Circles => GeometryToolVariant::CenterRadiusCircle,
            Self::Arcs => GeometryToolVariant::CenterArc,
            Self::Ellipses => GeometryToolVariant::CenterAxesEllipse,
            Self::Beziers => GeometryToolVariant::QuadraticBezier,
            Self::Conics => GeometryToolVariant::RationalQuadraticConic,
            Self::Splines => GeometryToolVariant::OpenControlNurbs,
        }
    }
}

impl GeometryToolVariant {
    /// Complete recipe inventory in stable palette order.
    pub const ALL: [Self; 25] = [
        Self::SketchPoint,
        Self::Segment,
        Self::Polyline,
        Self::MidpointLine,
        Self::TwoPointAlignedRectangle,
        Self::ThreePointCornerRectangle,
        Self::CenterRectangle,
        Self::ThreePointCenterRectangle,
        Self::CenterRadiusCircle,
        Self::TwoPointDiameterCircle,
        Self::ThreePointCircle,
        Self::CenterArc,
        Self::ThreePointArc,
        Self::TangentArc,
        Self::CenterAxesEllipse,
        Self::AxisEndpointsEllipse,
        Self::CenterAxesEllipticalArc,
        Self::AxisEndpointsEllipticalArc,
        Self::QuadraticBezier,
        Self::CubicBezier,
        Self::RationalQuadraticConic,
        Self::Parabola,
        Self::Hyperbola,
        Self::OpenControlNurbs,
        Self::PeriodicControlNurbs,
    ];

    /// Stable globally unique recipe key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::SketchPoint => "sketch-point",
            Self::Segment => "segment",
            Self::Polyline => "polyline",
            Self::MidpointLine => "midpoint-line",
            Self::TwoPointAlignedRectangle => "two-point-aligned-rectangle",
            Self::ThreePointCornerRectangle => "three-point-corner-rectangle",
            Self::CenterRectangle => "center-rectangle",
            Self::ThreePointCenterRectangle => "three-point-center-rectangle",
            Self::CenterRadiusCircle => "center-radius-circle",
            Self::TwoPointDiameterCircle => "two-point-diameter-circle",
            Self::ThreePointCircle => "three-point-circle",
            Self::CenterArc => "center-arc",
            Self::ThreePointArc => "three-point-arc",
            Self::TangentArc => "tangent-arc",
            Self::CenterAxesEllipse => "center-axes-ellipse",
            Self::AxisEndpointsEllipse => "axis-endpoints-ellipse",
            Self::CenterAxesEllipticalArc => "center-axes-elliptical-arc",
            Self::AxisEndpointsEllipticalArc => "axis-endpoints-elliptical-arc",
            Self::QuadraticBezier => "quadratic-bezier",
            Self::CubicBezier => "cubic-bezier",
            Self::RationalQuadraticConic => "rational-quadratic-conic",
            Self::Parabola => "parabola",
            Self::Hyperbola => "hyperbola",
            Self::OpenControlNurbs => "open-control-nurbs",
            Self::PeriodicControlNurbs => "periodic-control-nurbs",
        }
    }

    /// Palette family containing this recipe.
    #[must_use]
    pub const fn family(self) -> GeometryToolFamily {
        match self {
            Self::SketchPoint => GeometryToolFamily::Point,
            Self::Segment | Self::Polyline | Self::MidpointLine => GeometryToolFamily::Lines,
            Self::TwoPointAlignedRectangle
            | Self::ThreePointCornerRectangle
            | Self::CenterRectangle
            | Self::ThreePointCenterRectangle => GeometryToolFamily::Rectangles,
            Self::CenterRadiusCircle | Self::TwoPointDiameterCircle | Self::ThreePointCircle => {
                GeometryToolFamily::Circles
            }
            Self::CenterArc | Self::ThreePointArc | Self::TangentArc => GeometryToolFamily::Arcs,
            Self::CenterAxesEllipse
            | Self::AxisEndpointsEllipse
            | Self::CenterAxesEllipticalArc
            | Self::AxisEndpointsEllipticalArc => GeometryToolFamily::Ellipses,
            Self::QuadraticBezier | Self::CubicBezier => GeometryToolFamily::Beziers,
            Self::RationalQuadraticConic | Self::Parabola | Self::Hyperbola => {
                GeometryToolFamily::Conics
            }
            Self::OpenControlNurbs | Self::PeriodicControlNurbs => GeometryToolFamily::Splines,
        }
    }

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
