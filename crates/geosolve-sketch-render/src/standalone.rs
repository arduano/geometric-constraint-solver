// SPDX-License-Identifier: GPL-3.0-or-later

/// Logical width of the canonical sketch canvas.
pub const SVG_LOGICAL_WIDTH: u32 = 1_000;
/// Logical height of the canonical sketch canvas.
pub const SVG_LOGICAL_HEIGHT: u32 = 700;
/// Default native/browser raster export width.
pub const PNG_EXPORT_WIDTH: u32 = 2_000;
/// Default native/browser raster export height.
pub const PNG_EXPORT_HEIGHT: u32 = 1_400;

/// Browser-interactive paint rules for authoritative sketch scene markup.
///
/// The scene renderer owns this narrowly scoped presentation vocabulary so a
/// DOM host does not have to duplicate geometry, selection, authoring, or hit
/// semantics in an application-wide stylesheet. Hosts should embed these
/// rules in the authoritative SVG that contains scene markup from this crate.
/// The stylesheet deliberately contains no workbench layout or HTML control
/// rules.
const INTERACTIVE_SCENE_STYLE: &str = r#"
.wb-grid { pointer-events: none; }
.wb-grid path { fill: none; vector-effect: non-scaling-stroke; }
.wb-grid-minor { stroke: #1b2223; stroke-width: .75; }
.wb-grid-major { stroke: #263031; stroke-width: 1; }
.wb-point { transform: scale(var(--wb-camera-inverse-scale, 1)); transform-box: view-box; }
.wb-camera-fixed-size,
.wb-curve-control-hit,
.wb-curve-control-mark,
.wb-curve-control-tooltip,
.wb-fillet-radius-grip,
.wb-annotations text,
.wb-dimension-label-mask,
.wb-annotation-label-hit,
.wb-annotation-move-hit,
.wb-dimension-arrow {
  transform: scale(var(--wb-camera-inverse-scale, 1));
  transform-box: fill-box;
  transform-origin: center;
}
.wb-reference-geometry { isolation: isolate; }
.wb-datum { outline: none; cursor: pointer; }
.wb-datum-hit { fill: transparent; stroke: transparent; pointer-events: all; vector-effect: non-scaling-stroke; }
.wb-datum-axis .wb-datum-hit { stroke-width: 8; }
.wb-datum-line { fill: none; stroke: #6f7b7b; stroke-width: 1.25; vector-effect: non-scaling-stroke; }
.wb-datum-x-axis .wb-datum-line { stroke: #8c5b55; }
.wb-datum-y-axis .wb-datum-line { stroke: #4f8273; }
.wb-datum-label { fill: #879390; font: 700 11px ui-monospace,monospace; letter-spacing: .04em; pointer-events: none; paint-order: stroke; stroke: #121617; stroke-width: 3px; }
.wb-datum-x-axis .wb-datum-label { fill: #ba7770; }
.wb-datum-y-axis .wb-datum-label { fill: #72ad9c; }
.wb-datum.geometry-hovered .wb-datum-line,
.wb-datum:focus-visible .wb-datum-line,
.wb-datum.selected .wb-datum-line { stroke: #efb856; stroke-width: 2.2; filter: drop-shadow(0 0 3px rgb(239 184 86 / 50%)); }
.wb-datum.related .wb-datum-line { stroke: #79bfc4; filter: drop-shadow(0 0 3px rgb(121 191 196 / 42%)); }
.wb-datum.geometry-hovered .wb-datum-label,
.wb-datum:focus-visible .wb-datum-label,
.wb-datum.selected .wb-datum-label { fill: #f2ca82; }
.wb-curve { fill: none; stroke: #e5e8df; stroke-linecap: round; stroke-linejoin: round; stroke-width: 2.2; vector-effect: non-scaling-stroke; cursor: pointer; }
.wb-computed-fillet { stroke: #8ed5ca; stroke-width: 2.6; }
.wb-computed-item.geometry-hovered .wb-computed-fillet,
.wb-computed-item.selected .wb-computed-fillet { stroke: #efb856; filter: drop-shadow(0 0 3px rgb(239 184 86 / 45%)); }
.wb-computed-item.shared-radius-affected .wb-computed-fillet { stroke: #f0c66d; filter: drop-shadow(0 0 3px rgb(240 198 109 / 48%)); }
.wb-computed-item.has-problem .wb-computed-fillet { stroke: #ff7666; }
.wb-computed-hit { fill: none; stroke: transparent; stroke-linecap: round; stroke-linejoin: round; stroke-width: 14; pointer-events: stroke; cursor: ew-resize; vector-effect: non-scaling-stroke; }
.wb-fillet-radius-rail,
.wb-fillet-radius-spoke,
.wb-fillet-alternative-ghost { fill: none; vector-effect: non-scaling-stroke; }
.wb-fillet-radius-rail { stroke: #e6c785; stroke-width: 1.4; stroke-dasharray: 3 3; pointer-events: stroke; cursor: ew-resize; }
.wb-fillet-radius-spoke { stroke: #8ed5ca; stroke-width: 1.25; pointer-events: stroke; cursor: ew-resize; }
.wb-fillet-radius-grip { fill: #171c1d; stroke: #f0c66d; stroke-width: 2; cursor: ew-resize; }
.wb-fillet-action { cursor: pointer; }
.wb-fillet-action[data-fillet-action-input="canvas"]:focus { outline: none; }
.wb-fillet-action.disabled { opacity: .35; pointer-events: none; }
.wb-fillet-alternative-ghost { stroke: #d9b86e; stroke-width: 2; stroke-dasharray: 6 4; opacity: .92; pointer-events: stroke; }
.wb-fillet-retained-direction { fill: none; stroke: #e7ce8d; stroke-width: 2; stroke-linecap: round; vector-effect: non-scaling-stroke; }
.wb-fillet-action-hit { fill: none; stroke: transparent; stroke-width: 24; pointer-events: stroke; vector-effect: non-scaling-stroke; }
.wb-fillet-action-control path { fill: none; stroke: #f1d899; stroke-width: 1.4; stroke-linecap: round; stroke-linejoin: round; vector-effect: non-scaling-stroke; }
.wb-fillet-action.previewed .wb-fillet-retained-direction,
.wb-fillet-action.previewed .wb-fillet-action-control path:not(.wb-fillet-action-hit) { stroke: #fff0bd; filter: drop-shadow(0 0 3px rgb(239 184 86 / 65%)); }
.wb-fillet-action.previewed .wb-fillet-retained-direction { stroke-width: 3; }
.wb-curve.geometry-hovered,
.wb-point.geometry-hovered { stroke: #efb856; }
.wb-curve.selected,
.wb-point.selected { stroke: #efb856; filter: drop-shadow(0 0 3px rgb(239 184 86 / 45%)); }
.wb-curve.related,
.wb-point.related { stroke: #79bfc4; filter: drop-shadow(0 0 4px rgb(121 191 196 / 48%)); }
.wb-curve.authoring-pending,
.wb-point.authoring-pending { stroke: #79bfc4; stroke-dasharray: 4 3; filter: drop-shadow(0 0 4px rgb(121 191 196 / 55%)); pointer-events: none; }
.wb-annotation.authoring-pending { pointer-events: none; opacity: .78; }
.wb-curve.offset-provisional { stroke: #63d6ce; stroke-dasharray: 7 4; opacity: .84; pointer-events: none; }
.wb-point.offset-provisional { fill: #173d3b; stroke: #7ae8df; opacity: .9; pointer-events: none; }
.wb-annotation.offset-provisional { color: #7ae8df; opacity: .82; pointer-events: none; }
.wb-curve[data-role="construction"],
.wb-curve.construction { stroke: #86a0a2; stroke-dasharray: 9 3 2 3; }
.wb-curve[data-role="construction"].selected,
.wb-curve[data-role="construction"].geometry-hovered { stroke: #efb856; }
.wb-curve[data-role="construction"].related,
.wb-curve[data-role="construction"].authoring-pending { stroke: #79bfc4; }
.wb-curve[data-construction-origin="implicit"],
.wb-curve.implicit-construction { stroke: #70888b; stroke-dasharray: 4 5; opacity: .72; }
.wb-curve[data-construction-origin="implicit"].selected,
.wb-curve[data-construction-origin="implicit"].geometry-hovered { stroke: #efb856; opacity: 1; }
.wb-curve.offset-unavailable,
.wb-curve[data-role="construction"].offset-unavailable,
.wb-curve[data-construction-origin="implicit"].offset-unavailable { stroke: #c18778; stroke-dasharray: 3 4; opacity: .72; filter: drop-shadow(0 0 3px rgb(193 135 120 / 38%)); cursor: not-allowed; }
.wb-offset-chain-cues { pointer-events: none; }
.wb-offset-chain-direction { fill: none; stroke: #f0c66d; stroke-linecap: round; stroke-width: 2.1; vector-effect: non-scaling-stroke; filter: drop-shadow(0 0 2px rgb(240 198 109 / 48%)); }
.wb-offset-chain-terminal circle { fill: #162021; stroke: #f0c66d; stroke-width: 1.6; vector-effect: non-scaling-stroke; }
.wb-offset-chain-terminal.end circle { stroke: #79bfc4; }
.wb-offset-chain-terminal text { fill: #f4e3b4; dominant-baseline: central; font: 700 8px/1 ui-sans-serif,system-ui,sans-serif; text-anchor: middle; }
.wb-offset-chain-terminal.end text { fill: #bde7e4; }
.wb-point { fill: #131718; stroke: #8fd2ca; stroke-width: 2; cursor: grab; }
.wb-curve-control-guides,
.wb-curve-control-guides *,
.wb-curve-control-cage,
.wb-curve-control-cage * { pointer-events: none; }
.wb-curve-control-guide,
.wb-curve-control-rail { fill: none; vector-effect: non-scaling-stroke; }
.wb-curve-control-guide { stroke: #728486; stroke-width: 1.15; stroke-dasharray: 4 4; }
.wb-curve-control-rail { stroke: #b49b67; stroke-width: 1.25; stroke-dasharray: 2.5 3; }
.wb-curve-control-hit { fill: transparent; stroke: none; pointer-events: all; }
.wb-curve-control-mark { fill: #151a1b; stroke: #8fd2ca; stroke-width: 1.8; vector-effect: non-scaling-stroke; pointer-events: none; }
.wb-curve-control[data-control-role="trim-start"] .wb-curve-control-mark,
.wb-curve-control[data-control-role="trim-end"] .wb-curve-control-mark { stroke: #e6c785; }
.wb-curve-control[data-control-role="rational-middle"] .wb-curve-control-mark { stroke: #b7a0df; }
.wb-curve-control.hovered .wb-curve-control-mark,
.wb-curve-control.active .wb-curve-control-mark { stroke: #ffd27d; stroke-width: 2.4; filter: drop-shadow(0 0 4px rgb(239 184 86 / 65%)); }
.wb-curve-control.read-only .wb-curve-control-mark { stroke: #77817f; stroke-dasharray: 2 2; }
.wb-curve-control-tooltip { fill: #e6ece9; stroke: #151a1b; stroke-width: 4px; stroke-linejoin: round; paint-order: stroke fill; font: 600 10px/1.2 ui-sans-serif,system-ui,sans-serif; letter-spacing: .01em; }
.wb-curve-control-tooltip.context-hidden { display: none; }
.wb-curve[data-interactive="false"],
.wb-point[data-interactive="false"] { pointer-events: none; cursor: default; }
.wb-curve.offset-unavailable[data-interactive="false"] { pointer-events: stroke; cursor: not-allowed; }
.wb-curve.has-problem,
.wb-curve.has-problem[data-role="construction"] { stroke: #ff7666; filter: drop-shadow(0 0 5px rgb(255 82 69 / 65%)); }
.wb-point.has-problem { fill: #4a1d1b; stroke: #ff7666; filter: drop-shadow(0 0 5px rgb(255 82 69 / 65%)); }
.wb-draft { pointer-events: none; }
.wb-draft > path,
.wb-draft > rect,
.wb-draft > circle:not(.wb-draft-point,.wb-cursor-point) { fill: none; stroke: #efb856; stroke-dasharray: 8 5; stroke-linecap: round; stroke-linejoin: round; stroke-width: 2; vector-effect: non-scaling-stroke; }
.wb-draft-point,
.wb-cursor-point { fill: none; stroke: #efb856; stroke-width: 2; vector-effect: non-scaling-stroke; }
.wb-cursor-point { opacity: .55; }
.wb-inference-guides,
.wb-inference-candidates,
.wb-inference-state { pointer-events: none; }
.wb-inference-guide-point,
.wb-inference-guide-segment { fill: none; stroke-linecap: round; stroke-linejoin: round; vector-effect: non-scaling-stroke; }
.wb-inference-guide.constraint-backed .wb-inference-guide-point,
.wb-inference-guide.constraint-backed .wb-inference-guide-segment { stroke: #79d6ca; stroke-width: 1.8; filter: drop-shadow(0 0 3px rgb(121 214 202 / 38%)); }
.wb-inference-guide.constraint-backed .wb-inference-guide-segment { stroke-dasharray: 7 4; }
.wb-inference-guide.tracking-only .wb-inference-guide-point,
.wb-inference-guide.tracking-only .wb-inference-guide-segment { stroke: #8a9897; stroke-width: 1.25; stroke-dasharray: 2 5; opacity: .82; }
.wb-inference-glyph-background { fill: rgb(23 28 29 / 94%); stroke: #79d6ca; stroke-width: 1.25; vector-effect: non-scaling-stroke; }
.wb-inference-glyph-symbol { fill: none; stroke: #c8fff7; stroke-linecap: round; stroke-linejoin: round; stroke-width: 1.65; vector-effect: non-scaling-stroke; }
.wb-inference-candidates.ambiguous .wb-inference-glyph-background { stroke: #e2ad5d; }
.wb-inference-candidates.ambiguous .wb-inference-glyph-symbol { stroke: #ffe0a1; }
.wb-inference-state rect { fill: rgb(41 34 24 / 96%); stroke: #b88743; stroke-width: 1; vector-effect: non-scaling-stroke; }
.wb-inference-state text { fill: #f2d59c; font: 600 12px ui-sans-serif,system-ui,sans-serif; }
.wb-inference-state[data-inference-status="suppressed"] rect { fill: rgb(30 35 35 / 94%); stroke: #718080; }
.wb-inference-state[data-inference-status="suppressed"] text { fill: #b9c5c4; }
.wb-annotation { cursor: pointer; outline: none; }
.wb-annotation.context-hidden { display: none; }
.wb-dimension text { text-anchor: middle; pointer-events: none; }
.wb-dimension-label-mask { fill: #121617; stroke: none; pointer-events: none; }
.wb-dimension.reference .wb-dimension-line,
.wb-dimension.reference .wb-angle-arc { stroke-dasharray: 4 3; }
.wb-dimension.reference :is(.wb-dimension-line,.wb-angle-arc,.wb-dimension-arrow,text) { opacity: .72; }
.wb-dimension.reference .wb-dimension-witness { opacity: .45; }
.wb-annotation.suppressed :is(.wb-constraint-symbol,.wb-right-angle,.wb-annotation-leader,.wb-dimension-line,.wb-dimension-witness,.wb-angle-arc,.wb-dimension-arrow,text) { opacity: .48; }
.wb-annotation-hit { fill: transparent; stroke: none; pointer-events: all; }
.wb-annotation-path-hit { fill: none; stroke: transparent; stroke-linecap: round; stroke-linejoin: round; stroke-width: 20; pointer-events: stroke; cursor: pointer; vector-effect: non-scaling-stroke; }
.wb-annotation-label-hit { cursor: pointer; }
.wb-annotation-move-hit,
.wb-annotation.movable .wb-constraint-symbol { cursor: grab; }
.wb-annotation-move-hit:active,
.wb-annotation.movable .wb-constraint-symbol:active { cursor: grabbing; }
.wb-constraint-symbol,
.wb-right-angle { fill: none; stroke: #d7a654; stroke-linecap: round; stroke-linejoin: round; stroke-width: 1.8; vector-effect: non-scaling-stroke; }
.wb-annotation-leader { fill: none; stroke: #8d774f; stroke-dasharray: 2 2; stroke-width: 1; vector-effect: non-scaling-stroke; pointer-events: none; }
.wb-dimension { color: #79bfc4; fill: #79bfc4; font: 500 12px ui-monospace,monospace; }
.wb-dimension-line,
.wb-dimension-witness,
.wb-angle-arc { fill: none; stroke: #79bfc4; stroke-linecap: round; stroke-width: 1.25; vector-effect: non-scaling-stroke; }
.wb-dimension-witness { opacity: .62; }
.wb-dimension-arrow { fill: inherit; stroke: none; pointer-events: none; }
.wb-constraint-symbol.hovered,
.wb-annotation.hovered .wb-constraint-symbol,
.wb-annotation:focus-visible .wb-constraint-symbol,
.wb-annotation.selected .wb-constraint-symbol { fill: none; stroke: #fff2c8; filter: drop-shadow(0 0 3px rgb(239 184 86 / 50%)); }
.wb-dimension.hovered,
.wb-dimension:focus-visible,
.wb-dimension.selected { color: #fff2c8; fill: #fff2c8; stroke: #fff2c8; filter: drop-shadow(0 0 3px rgb(239 184 86 / 50%)); }
.wb-dimension.hovered :is(.wb-dimension-line,.wb-dimension-witness,.wb-angle-arc),
.wb-dimension:focus-visible :is(.wb-dimension-line,.wb-dimension-witness,.wb-angle-arc),
.wb-dimension.selected :is(.wb-dimension-line,.wb-dimension-witness,.wb-angle-arc) { stroke: currentcolor; }
.wb-annotation.has-problem .wb-constraint-symbol { fill: none; stroke: #ff8b7d; filter: drop-shadow(0 0 4px rgb(255 82 69 / 60%)); }
.wb-dimension.has-problem { color: #ff8b7d; fill: #ff8b7d; stroke: #ff8b7d; filter: drop-shadow(0 0 4px rgb(255 82 69 / 60%)); }
.wb-dimension.has-problem :is(.wb-dimension-line,.wb-dimension-witness,.wb-angle-arc) { stroke: currentcolor; }
.wb-error-overlay { pointer-events: none; }
.wb-error-marker { cursor: help; pointer-events: auto; }
.wb-error-marker > circle { fill: #b83d32; stroke: #ffd7d1; stroke-width: 1.5; vector-effect: non-scaling-stroke; }
.wb-error-marker.global > circle { fill: #d14b3f; }
.wb-error-marker-icon { fill: none; pointer-events: none; stroke: #fff; stroke-linecap: round; stroke-width: 2; vector-effect: non-scaling-stroke; }
.wb-error-marker:focus-visible { outline: none; }
.wb-error-marker:focus-visible > circle { stroke: #8de2ea; stroke-width: 3; }
.wb-error-tooltip { opacity: 0; overflow: visible; pointer-events: none; }
.wb-error-tooltip div { max-width: 22rem; padding: .48rem .58rem; border: 1px solid #a55a51; border-radius: 4px; color: #ffe5e1; background: rgb(53 29 27 / 98%); box-shadow: 0 8px 24px rgb(0 0 0 / 45%); font: .7rem/1.35 ui-sans-serif,system-ui,sans-serif; }
.wb-error-marker:hover .wb-error-tooltip,
.wb-error-marker:focus-visible .wb-error-tooltip { opacity: 1; }
"#;

/// Wraps authoritative interactive scene markup in its target-neutral SVG
/// presentation.
///
/// This is the browser/WASM counterpart to the standalone export wrappers
/// below. It retains transient hit and authoring markup and embeds the
/// renderer-owned interactive paint contract. Invalid or non-positive screen
/// dimensions are rejected rather than serialized into a broken viewport.
#[must_use]
pub fn interactive_scene_svg(
    scene_markup: &str,
    screen_size: [f64; 2],
    aria_label: &str,
) -> Option<String> {
    let [width, height] = screen_size;
    (width.is_finite() && width > 0.0 && height.is_finite() && height > 0.0).then(|| {
        format!(
            concat!(
                "<svg class=\"geosolve-authoritative-frame\" ",
                "xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" ",
                "role=\"img\" aria-label=\"{}\" preserveAspectRatio=\"xMidYMid meet\">",
                "<style>{}</style>{}</svg>"
            ),
            width,
            height,
            escape_attribute(aria_label),
            INTERACTIVE_SCENE_STYLE,
            scene_markup,
        )
    })
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

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

    #[test]
    fn interactive_scene_svg_owns_complete_scene_presentation_without_shell_css() {
        let markup = concat!(
            "<g class=\"wb-accepted-scene\">",
            "<path class=\"wb-curve selected\" d=\"M1 2L3 4\"/>",
            "<circle class=\"wb-point geometry-hovered\" cx=\"1\" cy=\"2\" r=\"5\"/>",
            "</g>"
        );
        let svg = interactive_scene_svg(markup, [1_000.0, 700.0], "A & <B>")
            .expect("finite positive viewport");

        assert!(svg.starts_with("<svg class=\"geosolve-authoritative-frame\""));
        assert!(svg.contains("aria-label=\"A &amp; &lt;B&gt;\""));
        assert!(svg.contains("<style>"));
        assert!(svg.contains(markup));
        for selector in [
            ".wb-curve {",
            ".wb-datum.geometry-hovered",
            ".wb-point.geometry-hovered",
            ".wb-curve.selected",
            ".wb-point.related",
            ".wb-curve.authoring-pending",
            ".wb-computed-hit {",
            ".wb-computed-item.selected",
            ".wb-computed-item.shared-radius-affected",
            ".wb-fillet-radius-grip {",
            ".wb-fillet-action-hit {",
            ".wb-fillet-action.disabled",
            ".wb-fillet-action.previewed",
            ".wb-curve.offset-provisional {",
            ".wb-curve.offset-unavailable",
            ".wb-offset-chain-direction {",
            ".wb-curve-control.hovered",
            ".wb-annotation-path-hit {",
            ".wb-annotation.context-hidden",
            ".wb-annotation.suppressed",
            ".wb-dimension {",
            ".wb-dimension.reference",
            ".wb-annotation.has-problem",
            ".wb-curve[data-role=\"construction\"]",
            ".wb-draft > path",
            ".wb-inference-guide.constraint-backed",
            ".wb-inference-candidates.ambiguous",
            ".wb-inference-state[data-inference-status=\"suppressed\"]",
            ".wb-error-marker > circle",
        ] {
            assert!(
                INTERACTIVE_SCENE_STYLE.contains(selector),
                "missing interactive scene selector {selector}"
            );
        }
        for shell_rule in [
            ".workbench",
            ".wb-app-bar",
            ".wb-tool-rail",
            ".wb-inspector-section",
            ".wb-code-editor",
        ] {
            assert!(
                !INTERACTIVE_SCENE_STYLE.contains(shell_rule),
                "interactive scene style leaked shell rule {shell_rule}"
            );
        }
        assert!(interactive_scene_svg(markup, [0.0, 700.0], "invalid").is_none());
        assert!(interactive_scene_svg(markup, [f64::NAN, 700.0], "invalid").is_none());
        assert!(interactive_scene_svg(markup, [1_000.0, f64::INFINITY], "invalid").is_none());
    }
}
