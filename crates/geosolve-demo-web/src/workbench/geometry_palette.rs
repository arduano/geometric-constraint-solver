// SPDX-License-Identifier: GPL-3.0-or-later

//! Thin presentation metadata for the headless geometry-tool catalog.
//!
//! Recipe progression, geometry and commit behavior remain owned by
//! `geosolve-constraint-editor`. This module only supplies human-facing labels,
//! session-local family selection and accessible menu markup.

use std::fmt::Write as _;

use geosolve_constraint_editor::{
    GeometryDraftMeasurement, GeometryDraftStage, GeometryDraftStatus, GeometryToolFamily,
    GeometryToolVariant,
};
use geosolve_sketch::DocumentArcSweep;

#[derive(Clone, Debug)]
pub(super) struct GeometryPaletteState {
    remembered: Vec<(GeometryToolFamily, GeometryToolVariant)>,
}

impl Default for GeometryPaletteState {
    fn default() -> Self {
        Self {
            remembered: GeometryToolFamily::ALL
                .into_iter()
                .map(|family| (family, family.default_variant()))
                .collect(),
        }
    }
}

impl GeometryPaletteState {
    pub(super) fn selected(&self, family: GeometryToolFamily) -> GeometryToolVariant {
        self.remembered
            .iter()
            .find_map(|(candidate, variant)| (*candidate == family).then_some(*variant))
            .unwrap_or_else(|| family.default_variant())
    }

    pub(super) fn remember(&mut self, variant: GeometryToolVariant) {
        let family = variant.family();
        if let Some((_, selected)) = self
            .remembered
            .iter_mut()
            .find(|(candidate, _)| *candidate == family)
        {
            *selected = variant;
        } else {
            self.remembered.push((family, variant));
        }
    }
}

pub(super) fn family_from_key(key: &str) -> Option<GeometryToolFamily> {
    GeometryToolFamily::ALL
        .into_iter()
        .find(|family| family.key() == key)
}

pub(super) fn variant_from_key(key: &str) -> Option<GeometryToolVariant> {
    GeometryToolVariant::ALL
        .into_iter()
        .find(|variant| variant.key() == key)
}

pub(super) const fn family_label(family: GeometryToolFamily) -> &'static str {
    match family {
        GeometryToolFamily::Point => "Point",
        GeometryToolFamily::Lines => "Lines",
        GeometryToolFamily::Rectangles => "Rectangles",
        GeometryToolFamily::Circles => "Circles",
        GeometryToolFamily::Arcs => "Arcs",
        GeometryToolFamily::Ellipses => "Ellipses",
        GeometryToolFamily::Beziers => "Béziers",
        GeometryToolFamily::Conics => "Conics",
        GeometryToolFamily::Splines => "Splines",
        _ => "Geometry",
    }
}

pub(super) const fn variant_label(variant: GeometryToolVariant) -> &'static str {
    match variant {
        GeometryToolVariant::SketchPoint => "Sketch Point",
        GeometryToolVariant::Segment => "Segment",
        GeometryToolVariant::Polyline => "Polyline",
        GeometryToolVariant::MidpointLine => "Midpoint Line",
        GeometryToolVariant::TwoPointAlignedRectangle => "2-Point Aligned",
        GeometryToolVariant::ThreePointCornerRectangle => "3-Point Corner",
        GeometryToolVariant::CenterRectangle => "Center Rectangle",
        GeometryToolVariant::ThreePointCenterRectangle => "3-Point Center",
        GeometryToolVariant::CenterRadiusCircle => "Center–Radius",
        GeometryToolVariant::TwoPointDiameterCircle => "2-Point Diameter",
        GeometryToolVariant::ThreePointCircle => "3-Point Circle",
        GeometryToolVariant::CenterArc => "Center Arc",
        GeometryToolVariant::ThreePointArc => "3-Point Arc",
        GeometryToolVariant::TangentArc => "Tangent Arc",
        GeometryToolVariant::CenterAxesEllipse => "Center–Axes Ellipse",
        GeometryToolVariant::AxisEndpointsEllipse => "Axis-Endpoints Ellipse",
        GeometryToolVariant::CenterAxesEllipticalArc => "Center–Axes Elliptical Arc",
        GeometryToolVariant::AxisEndpointsEllipticalArc => "Axis-Endpoints Elliptical Arc",
        GeometryToolVariant::QuadraticBezier => "Quadratic",
        GeometryToolVariant::CubicBezier => "Cubic",
        GeometryToolVariant::RationalQuadraticConic => "Rational Quadratic",
        GeometryToolVariant::Parabola => "Parabola",
        GeometryToolVariant::Hyperbola => "Hyperbola",
        GeometryToolVariant::OpenControlNurbs => "Open Control NURBS",
        GeometryToolVariant::PeriodicControlNurbs => "Periodic Control NURBS",
        _ => "Geometry variant",
    }
}

pub(super) const fn variant_description(variant: GeometryToolVariant) -> &'static str {
    match variant {
        GeometryToolVariant::SketchPoint => "Place or reuse one persistent point.",
        GeometryToolVariant::Segment => "Start and End.",
        GeometryToolVariant::Polyline => "Connected edges; Enter or double-click finishes.",
        GeometryToolVariant::MidpointLine => "Center and one End; the opposite end is reflected.",
        GeometryToolVariant::TwoPointAlignedRectangle => {
            "Corner and opposite Corner; Shift makes a square."
        }
        GeometryToolVariant::ThreePointCornerRectangle => {
            "Corner, baseline Corner and height; Shift makes a square."
        }
        GeometryToolVariant::CenterRectangle => "Center and Corner; Shift makes a square.",
        GeometryToolVariant::ThreePointCenterRectangle => {
            "Center, side midpoint and Corner; Shift makes a square."
        }
        GeometryToolVariant::CenterRadiusCircle => "Center and radius point.",
        GeometryToolVariant::TwoPointDiameterCircle => "Two diameter endpoints.",
        GeometryToolVariant::ThreePointCircle => "Three non-collinear rim samples.",
        GeometryToolVariant::CenterArc => "Center, Start and End; F flips the sweep.",
        GeometryToolVariant::ThreePointArc => "Start, Through and End samples.",
        GeometryToolVariant::TangentArc => "Eligible open-curve endpoint, then End.",
        GeometryToolVariant::CenterAxesEllipse => "Center, major endpoint and minor extent.",
        GeometryToolVariant::AxisEndpointsEllipse => "Major endpoints, then minor extent.",
        GeometryToolVariant::CenterAxesEllipticalArc => {
            "Center frame, then spatial Start and End; F flips."
        }
        GeometryToolVariant::AxisEndpointsEllipticalArc => {
            "Axis-endpoint frame, then spatial Start and End; F flips."
        }
        GeometryToolVariant::QuadraticBezier => "Start, Control and End.",
        GeometryToolVariant::CubicBezier => "Start, two Controls and End.",
        GeometryToolVariant::RationalQuadraticConic => "Start, ordinary middle control and End.",
        GeometryToolVariant::Parabola => "Vertex and Focus with explicit trim options.",
        GeometryToolVariant::Hyperbola => {
            "Center and transverse endpoint with explicit branch options."
        }
        GeometryToolVariant::OpenControlNurbs => {
            "Open control curve; Enter or double-click finishes."
        }
        GeometryToolVariant::PeriodicControlNurbs => {
            "Explicit periodic control curve; Enter or double-click finishes."
        }
        _ => "Headless geometry recipe.",
    }
}

pub(super) fn variant_button_id(variant: GeometryToolVariant) -> String {
    format!("wb-geometry-variant-{}", variant.key())
}

pub(super) fn variant_menu_markup(
    family: GeometryToolFamily,
    selected: GeometryToolVariant,
) -> String {
    let mut output = String::new();
    for &variant in family.variants() {
        let checked = variant == selected;
        let _ = write!(
            output,
            "<button id=\"{}\" type=\"button\" role=\"radio\" aria-checked=\"{}\" tabindex=\"{}\" data-wb-geometry-variant=\"{}\"><strong>{}</strong><span>{}</span></button>",
            variant_button_id(variant),
            checked,
            if checked { 0 } else { -1 },
            variant.key(),
            variant_label(variant),
            variant_description(variant),
        );
    }
    output
}

pub(super) const fn stage_label(stage: GeometryDraftStage) -> &'static str {
    match stage {
        GeometryDraftStage::Point => "Point",
        GeometryDraftStage::Start => "Start",
        GeometryDraftStage::End => "End",
        GeometryDraftStage::Center => "Center",
        GeometryDraftStage::Corner => "Corner",
        GeometryDraftStage::AdjacentCorner => "Adjacent corner",
        GeometryDraftStage::OppositeCorner => "Opposite corner",
        GeometryDraftStage::SideMidpoint => "Side midpoint",
        GeometryDraftStage::DiameterStart => "Diameter start",
        GeometryDraftStage::DiameterEnd => "Diameter end",
        GeometryDraftStage::ThroughPoint => "Through point",
        GeometryDraftStage::SourceEndpoint => "Source endpoint",
        GeometryDraftStage::MajorAxisEndpoint => "Major-axis endpoint",
        GeometryDraftStage::OppositeAxisEndpoint => "Opposite axis endpoint",
        GeometryDraftStage::MinorExtent => "Minor extent",
        GeometryDraftStage::ControlPoint => "Control point",
        GeometryDraftStage::Vertex => "Vertex",
        GeometryDraftStage::Focus => "Focus",
        GeometryDraftStage::TransverseAxisEndpoint => "Transverse-axis endpoint",
        GeometryDraftStage::ConjugateExtent => "Conjugate extent",
        GeometryDraftStage::TrimStart => "Arc start",
        GeometryDraftStage::TrimEnd => "Arc end",
        _ => "Next input",
    }
}

pub(super) fn status_text(status: &GeometryDraftStatus) -> String {
    let progress = status.required_stages.map_or_else(
        || format!("{} placed", status.completed_stages),
        |required| format!("{}/{}", status.completed_stages, required),
    );
    let mut pieces = vec![
        variant_label(status.variant).to_owned(),
        format!("{} · {progress}", stage_label(status.stage)),
    ];
    if status.regularized {
        pieces.push("Square".to_owned());
    }
    pieces.extend(status.measurements.iter().map(measurement_text));
    if status.can_finish {
        pieces.push("Enter finishes".to_owned());
    }
    if let Some(sweep) = status.branch.sweep {
        pieces.push(format!(
            "{} sweep · F flips",
            match sweep {
                DocumentArcSweep::CounterClockwise => "Counter-clockwise",
                DocumentArcSweep::Clockwise => "Clockwise",
            }
        ));
    }
    pieces.join(" · ")
}

fn measurement_text(measurement: &GeometryDraftMeasurement) -> String {
    match *measurement {
        GeometryDraftMeasurement::Length(value) => format!("L {value:.3}"),
        GeometryDraftMeasurement::Radius(value) => format!("R {value:.3}"),
        GeometryDraftMeasurement::Diameter(value) => format!("Ø {value:.3}"),
        GeometryDraftMeasurement::AngleRadians(value) => {
            format!("∠ {:.1}°", value.to_degrees())
        }
        GeometryDraftMeasurement::Ratio(value) => format!("ratio {value:.3}"),
        GeometryDraftMeasurement::WidthHeight { width, height } => {
            format!("W {width:.3} × H {height:.3}")
        }
        GeometryDraftMeasurement::ControlCount(value) => format!("{value} controls"),
        _ => "Live measurement".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_uses_the_complete_headless_catalog_and_remembers_each_family() {
        let mut palette = GeometryPaletteState::default();
        assert_eq!(GeometryToolFamily::ALL.len(), 9);
        assert_eq!(GeometryToolVariant::ALL.len(), 25);
        for family in GeometryToolFamily::ALL {
            assert_eq!(palette.selected(family), family.default_variant());
        }

        palette.remember(GeometryToolVariant::Polyline);
        palette.remember(GeometryToolVariant::ThreePointCircle);
        assert_eq!(
            palette.selected(GeometryToolFamily::Lines),
            GeometryToolVariant::Polyline
        );
        assert_eq!(
            palette.selected(GeometryToolFamily::Circles),
            GeometryToolVariant::ThreePointCircle
        );
        assert_eq!(
            palette.selected(GeometryToolFamily::Rectangles),
            GeometryToolFamily::Rectangles.default_variant()
        );
    }

    #[test]
    fn stable_keys_round_trip_through_thin_palette_mapping() {
        for family in GeometryToolFamily::ALL {
            assert_eq!(family_from_key(family.key()), Some(family));
        }
        for variant in GeometryToolVariant::ALL {
            assert_eq!(variant_from_key(variant.key()), Some(variant));
        }
    }

    #[test]
    fn variant_menu_is_accessible_and_contains_only_the_requested_family() {
        let markup = variant_menu_markup(GeometryToolFamily::Arcs, GeometryToolVariant::TangentArc);
        assert_eq!(markup.matches("role=\"radio\"").count(), 3);
        assert!(markup.contains("data-wb-geometry-variant=\"center-arc\""));
        assert!(markup.contains("data-wb-geometry-variant=\"three-point-arc\""));
        assert!(markup.contains("data-wb-geometry-variant=\"tangent-arc\""));
        assert!(markup.contains("aria-checked=\"true\""));
        assert_eq!(markup.matches("tabindex=\"0\"").count(), 1);
        assert_eq!(markup.matches("tabindex=\"-1\"").count(), 2);
        assert!(!markup.contains("center-radius-circle"));
    }

    #[test]
    fn semantic_status_copy_uses_headless_stage_progress_and_measurements() {
        let status = GeometryDraftStatus {
            variant: GeometryToolVariant::TwoPointAlignedRectangle,
            stage: GeometryDraftStage::OppositeCorner,
            completed_stages: 1,
            required_stages: Some(2),
            can_finish: false,
            regularized: true,
            branch: geosolve_constraint_editor::GeometryDraftBranch::default(),
            measurements: vec![GeometryDraftMeasurement::WidthHeight {
                width: 4.0,
                height: 3.0,
            }],
        };
        let copy = status_text(&status);
        assert!(copy.contains("2-Point Aligned"));
        assert!(copy.contains("Opposite corner · 1/2"));
        assert!(copy.contains("Square"));
        assert!(copy.contains("W 4.000 × H 3.000"));
    }

    #[test]
    fn semantic_status_copy_names_the_current_explicit_sweep() {
        let status = GeometryDraftStatus {
            variant: GeometryToolVariant::CenterArc,
            stage: GeometryDraftStage::End,
            completed_stages: 2,
            required_stages: Some(3),
            can_finish: false,
            regularized: false,
            branch: geosolve_constraint_editor::GeometryDraftBranch {
                sweep: Some(DocumentArcSweep::Clockwise),
                ..geosolve_constraint_editor::GeometryDraftBranch::default()
            },
            measurements: Vec::new(),
        };
        assert!(status_text(&status).contains("Clockwise sweep · F flips"));
    }
}
