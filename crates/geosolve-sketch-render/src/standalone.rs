// SPDX-License-Identifier: GPL-3.0-or-later

/// Logical width of the canonical sketch canvas.
pub const SVG_LOGICAL_WIDTH: u32 = 1_000;
/// Logical height of the canonical sketch canvas.
pub const SVG_LOGICAL_HEIGHT: u32 = 700;
/// Default native/browser raster export width.
pub const PNG_EXPORT_WIDTH: u32 = 2_000;
/// Default native/browser raster export height.
pub const PNG_EXPORT_HEIGHT: u32 = 1_400;

const EXPORT_STYLE: &str = r#"
.wb-grid { pointer-events: none; }
.wb-grid path { fill: none; vector-effect: non-scaling-stroke; }
.wb-grid-minor { stroke: #1b2223; stroke-width: .75; }
.wb-grid-major { stroke: #263031; stroke-width: 1; }
.wb-datum-hit,.wb-computed-hit,.wb-fillet-action,.wb-fillet-action-hit,
.wb-fillet-radius-rail,.wb-fillet-radius-spoke,.wb-fillet-radius-grip,
.wb-fillet-alternative-ghost,.wb-curve-control-guides,.wb-curve-control-cage,
.wb-annotation-hit,.wb-annotation-path-hit,.wb-annotation-label-hit,
.wb-annotation-move-hit,.wb-error-overlay,.wb-offset-chain-cues,
.wb-draft,.wb-inference-guides,.wb-inference-candidates,
.authoring-pending,.offset-provisional { display: none; }
.wb-datum-line { fill: none; stroke: #6f7b7b; stroke-width: 1.25; vector-effect: non-scaling-stroke; }
.wb-datum-x-axis .wb-datum-line { stroke: #8c5b55; }
.wb-datum-y-axis .wb-datum-line { stroke: #4f8273; }
.wb-datum-label { fill: #879390; font: 700 11px ui-monospace,monospace; paint-order: stroke; stroke: #121617; stroke-width: 3px; }
.wb-datum-x-axis .wb-datum-label { fill: #ba7770; }
.wb-datum-y-axis .wb-datum-label { fill: #72ad9c; }
.wb-curve { fill: none; stroke: #e5e8df; stroke-linecap: round; stroke-linejoin: round; stroke-width: 2.2; vector-effect: non-scaling-stroke; }
.wb-computed-fillet { stroke: #8ed5ca; stroke-width: 2.6; }
.wb-curve[data-role="construction"],.wb-curve.construction { stroke: #86a0a2; stroke-dasharray: 9 3 2 3; }
.wb-curve[data-construction-origin="implicit"],.wb-curve.implicit-construction { stroke: #70888b; stroke-dasharray: 4 5; opacity: .72; }
.wb-point { fill: #131718; stroke: #8fd2ca; stroke-width: 2; }
.wb-constraint-symbol { fill: none; stroke: #d7a654; stroke-linecap: round; stroke-linejoin: round; stroke-width: 1.8; vector-effect: non-scaling-stroke; }
.wb-right-angle { fill: none; stroke: #d7a654; stroke-linecap: round; stroke-linejoin: round; stroke-width: 1.8; vector-effect: non-scaling-stroke; }
.wb-annotation-leader { fill: none; stroke: #8d774f; stroke-dasharray: 2 2; stroke-width: 1; vector-effect: non-scaling-stroke; }
.wb-dimension { color: #79bfc4; fill: #79bfc4; font: 500 12px ui-monospace,monospace; }
.wb-dimension text { text-anchor: middle; }
.wb-dimension-label-mask { fill: #121617; stroke: none; }
.wb-dimension-line,.wb-dimension-witness,.wb-angle-arc { fill: none; stroke: #79bfc4; stroke-linecap: round; stroke-width: 1.25; vector-effect: non-scaling-stroke; }
.wb-dimension-witness { opacity: .62; }
.wb-dimension-arrow { fill: inherit; stroke: none; }
.wb-dimension.reference .wb-dimension-line,.wb-dimension.reference .wb-angle-arc { stroke-dasharray: 4 3; }
.wb-dimension.reference :is(.wb-dimension-line,.wb-angle-arc,.wb-dimension-arrow,text) { opacity: .72; }
.wb-dimension.reference .wb-dimension-witness { opacity: .45; }
.wb-annotation.suppressed :is(.wb-constraint-symbol,.wb-right-angle,.wb-annotation-leader,.wb-dimension-line,.wb-dimension-witness,.wb-angle-arc,.wb-dimension-arrow,text) { opacity: .48; }
"#;

const STATIC_EXPORT_STYLE: &str = r"
.wb-grid { pointer-events: none; }
.wb-grid path { fill: none; vector-effect: non-scaling-stroke; }
.wb-grid-minor { stroke: #1b2223; stroke-width: .75; }
.wb-grid-major { stroke: #263031; stroke-width: 1; }
.wb-datum-line { fill: none; stroke: #6f7b7b; stroke-width: 1.25; vector-effect: non-scaling-stroke; }
.wb-datum-x-axis .wb-datum-line { stroke: #8c5b55; }
.wb-datum-y-axis .wb-datum-line { stroke: #4f8273; }
.wb-datum-label { fill: #879390; font: 700 11px ui-monospace,monospace; paint-order: stroke; stroke: #121617; stroke-width: 3px; }
.wb-datum-x-axis .wb-datum-label { fill: #ba7770; }
.wb-datum-y-axis .wb-datum-label { fill: #72ad9c; }
.wb-curve { fill: none; stroke: #e5e8df; stroke-linecap: round; stroke-linejoin: round; stroke-width: 2.2; vector-effect: non-scaling-stroke; }
.wb-computed-fillet { stroke: #8ed5ca; stroke-width: 2.6; }
.wb-curve.construction { stroke: #86a0a2; stroke-dasharray: 9 3 2 3; }
.wb-curve.implicit-construction { stroke: #70888b; stroke-dasharray: 4 5; opacity: .72; }
.wb-point { fill: #131718; stroke: #8fd2ca; stroke-width: 2; }
.wb-constraint-symbol { fill: none; stroke: #d7a654; stroke-linecap: round; stroke-linejoin: round; stroke-width: 1.8; vector-effect: non-scaling-stroke; }
.wb-right-angle { fill: none; stroke: #d7a654; stroke-linecap: round; stroke-linejoin: round; stroke-width: 1.8; vector-effect: non-scaling-stroke; }
.wb-annotation-leader { fill: none; stroke: #8d774f; stroke-dasharray: 2 2; stroke-width: 1; vector-effect: non-scaling-stroke; }
.wb-dimension { color: #79bfc4; fill: #79bfc4; font: 500 12px ui-monospace,monospace; }
.wb-dimension text { text-anchor: middle; }
.wb-dimension-label-mask { fill: #121617; stroke: none; }
.wb-dimension-line,.wb-dimension-witness,.wb-angle-arc { fill: none; stroke: #79bfc4; stroke-linecap: round; stroke-width: 1.25; vector-effect: non-scaling-stroke; }
.wb-dimension-witness { opacity: .62; }
.wb-dimension-arrow { fill: inherit; stroke: none; }
.wb-dimension.reference .wb-dimension-line,.wb-dimension.reference .wb-angle-arc { stroke-dasharray: 4 3; }
.wb-dimension.reference :is(.wb-dimension-line,.wb-angle-arc,.wb-dimension-arrow,text) { opacity: .72; }
.wb-dimension.reference .wb-dimension-witness { opacity: .45; }
.wb-annotation.suppressed :is(.wb-constraint-symbol,.wb-right-angle,.wb-annotation-leader,.wb-dimension-line,.wb-dimension-witness,.wb-angle-arc,.wb-dimension-arrow,text) { opacity: .48; }
";

/// Produces a self-contained deterministic SVG from canonical scene markup.
///
/// The wrapper supplies only the fixed background and paint rules. It does not
/// inspect or reconstruct sketch geometry and hides transient hit, draft,
/// inference, provisional, and error-only presentation.
#[must_use]
pub fn standalone_export_svg(scene_markup: &str) -> String {
    standalone_export_svg_with_size(scene_markup, PNG_EXPORT_WIDTH, PNG_EXPORT_HEIGHT)
        .unwrap_or_default()
}

/// Produces a self-contained deterministic SVG with explicit positive output dimensions.
///
/// Returns `None` for a zero dimension; the logical canvas and view box stay
/// fixed so the scene markup remains byte-identical between native and WASM.
#[must_use]
pub fn standalone_export_svg_with_size(
    scene_markup: &str,
    width: u32,
    height: u32,
) -> Option<String> {
    standalone_svg_with_style(scene_markup, width, height, EXPORT_STYLE)
}

/// Produces deterministic standalone SVG from paint-only static scene markup.
///
/// Unlike [`standalone_export_svg`], this wrapper contains no browser
/// interaction-hiding selectors or interaction metadata vocabulary.
#[must_use]
pub fn standalone_static_export_svg(scene_markup: &str) -> String {
    standalone_static_export_svg_with_size(scene_markup, PNG_EXPORT_WIDTH, PNG_EXPORT_HEIGHT)
        .unwrap_or_default()
}

/// Produces deterministic paint-only standalone SVG with explicit dimensions.
#[must_use]
pub fn standalone_static_export_svg_with_size(
    scene_markup: &str,
    width: u32,
    height: u32,
) -> Option<String> {
    standalone_svg_with_style(scene_markup, width, height, STATIC_EXPORT_STYLE)
}

fn standalone_svg_with_style(
    scene_markup: &str,
    width: u32,
    height: u32,
    style: &str,
) -> Option<String> {
    (width > 0 && height > 0).then(|| {
        format!(
            concat!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" ",
                "viewBox=\"0 0 1000 700\" preserveAspectRatio=\"xMidYMid meet\">",
                "<style>{}</style><rect width=\"1000\" height=\"700\" fill=\"#121617\"/>{}</svg>"
            ),
            width, height, style, scene_markup
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_export_is_self_contained_and_deterministic() {
        let markup = "<path class=\"wb-curve\" d=\"M 1 2 L 3 4\"/>";
        let first = standalone_export_svg(markup);
        let second = standalone_export_svg(markup);
        assert_eq!(first, second);
        assert!(first.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(first.contains("width=\"2000\" height=\"1400\""));
        assert!(first.contains("viewBox=\"0 0 1000 700\""));
        assert!(first.contains(".wb-computed-hit"));
        assert!(first.contains(".wb-inference-candidates"));
        assert!(first.ends_with("</svg>"));
        assert!(standalone_export_svg_with_size(markup, 0, 1).is_none());
    }

    #[test]
    fn static_export_contains_only_paint_vocabulary() {
        let markup = concat!(
            "<g class=\"wb-accepted-scene\"><g class=\"wb-geometry\">",
            "<path class=\"wb-curve construction\" d=\"M 1 2 L 3 4\"/>",
            "</g></g>"
        );
        let first = standalone_static_export_svg(markup);
        let second = standalone_static_export_svg(markup);
        assert_eq!(first, second);
        assert!(first.contains(".wb-curve.construction"));
        assert!(!first.contains("data-"));
        for transient in [
            "wb-computed-hit",
            "wb-fillet-action",
            "wb-curve-control",
            "wb-annotation-hit",
            "wb-draft",
            "wb-inference",
            "wb-error-overlay",
        ] {
            assert!(!first.contains(transient));
        }
    }
}
