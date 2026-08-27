// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation-only PNG export for the authoritative sketch canvas.
//!
//! This module deliberately consumes the already composed SVG scene. It does
//! not inspect the sketch document, recreate geometry, or participate in
//! accepted-scene authority.

pub(crate) const PNG_EXPORT_WIDTH: u32 = 2_000;
pub(crate) const PNG_EXPORT_HEIGHT: u32 = 1_400;

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
.authoring-pending,.offset-provisional { display: none; }
.wb-datum-line { fill: none; stroke: #6f7b7b; stroke-width: 1.25; vector-effect: non-scaling-stroke; }
.wb-datum-x-axis .wb-datum-line { stroke: #8c5b55; }
.wb-datum-y-axis .wb-datum-line { stroke: #4f8273; }
.wb-datum-label { fill: #879390; font: 700 11px ui-monospace,monospace; paint-order: stroke; stroke: #121617; stroke-width: 3px; }
.wb-datum-x-axis .wb-datum-label { fill: #ba7770; }
.wb-datum-y-axis .wb-datum-label { fill: #72ad9c; }
.wb-curve { fill: none; stroke: #e5e8df; stroke-linecap: round; stroke-linejoin: round; stroke-width: 2.2; vector-effect: non-scaling-stroke; }
.wb-computed-fillet { stroke: #8ed5ca; stroke-width: 2.6; }
.wb-curve[data-role="construction"] { stroke: #86a0a2; stroke-dasharray: 9 3 2 3; }
.wb-curve[data-construction-origin="implicit"] { stroke: #70888b; stroke-dasharray: 4 5; opacity: .72; }
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

/// Produces a self-contained, deterministic SVG rasterization source.
///
/// The fitted workbench viewport is already authoritative presentation. The
/// wrapper supplies only background and paint rules so browser rasterization
/// never depends on the page stylesheet or exports invisible hit targets.
#[must_use]
pub(crate) fn standalone_export_svg(scene_markup: &str) -> String {
    format!(
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" ",
            "viewBox=\"0 0 1000 700\" preserveAspectRatio=\"xMidYMid meet\">",
            "<style>{}</style><rect width=\"1000\" height=\"700\" fill=\"#121617\"/>{}</svg>"
        ),
        PNG_EXPORT_WIDTH, PNG_EXPORT_HEIGHT, EXPORT_STYLE, scene_markup
    )
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn export_viewport_png(
    document: &web_sys::Document,
) -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::{JsCast, JsValue, closure::Closure};
    use web_sys::{
        Blob, BlobPropertyBag, CanvasRenderingContext2d, HtmlAnchorElement, HtmlCanvasElement,
        HtmlImageElement, Url,
    };

    let viewport = document
        .get_element_by_id("wb-viewport")
        .ok_or_else(|| JsValue::from_str("sketch viewport unavailable"))?;
    let svg = standalone_export_svg(&viewport.inner_html());
    let parts = js_sys::Array::new();
    parts.push(&JsValue::from_str(&svg));
    let options = BlobPropertyBag::new();
    options.set_type("image/svg+xml;charset=utf-8");
    let source = Blob::new_with_str_sequence_and_options(&parts, &options)?;
    let source_url = Url::create_object_url_with_blob(&source)?;
    let image = HtmlImageElement::new()?;
    image.set_alt("GeoSolve PNG export rasterization source");

    let loaded_document = document.clone();
    let loaded_image = image.clone();
    let loaded_source_url = source_url.clone();
    let onload = Closure::<dyn FnMut()>::once(move || {
        let outcome = (|| -> Result<(), JsValue> {
            Url::revoke_object_url(&loaded_source_url)?;
            let canvas = loaded_document
                .create_element("canvas")?
                .dyn_into::<HtmlCanvasElement>()?;
            canvas.set_width(PNG_EXPORT_WIDTH);
            canvas.set_height(PNG_EXPORT_HEIGHT);
            let context = canvas
                .get_context("2d")?
                .ok_or_else(|| JsValue::from_str("2D canvas context unavailable"))?
                .dyn_into::<CanvasRenderingContext2d>()?;
            context.draw_image_with_html_image_element_and_dw_and_dh(
                &loaded_image,
                0.0,
                0.0,
                f64::from(PNG_EXPORT_WIDTH),
                f64::from(PNG_EXPORT_HEIGHT),
            )?;

            let callback_document = loaded_document.clone();
            let callback = Closure::<dyn FnMut(Option<Blob>)>::once(move |png: Option<Blob>| {
                let outcome = (|| -> Result<(), JsValue> {
                    let png = png.ok_or_else(|| JsValue::from_str("PNG encoding failed"))?;
                    let png_url = Url::create_object_url_with_blob(&png)?;
                    let anchor = callback_document
                        .create_element("a")?
                        .dyn_into::<HtmlAnchorElement>()?;
                    anchor.set_href(&png_url);
                    anchor.set_download("geosolve-sketch.png");
                    anchor.set_hidden(true);
                    callback_document
                        .body()
                        .ok_or_else(|| JsValue::from_str("document body unavailable"))?
                        .append_child(&anchor)?;
                    anchor.click();
                    // Keep the object URL alive through the browser's next
                    // task. Revoking it synchronously after `click()` can
                    // cancel a valid download in some engines.
                    let cleanup_anchor = anchor.clone();
                    let cleanup_url = png_url.clone();
                    let cleanup = Closure::<dyn FnMut()>::once(move || {
                        cleanup_anchor.remove();
                        let _ = Url::revoke_object_url(&cleanup_url);
                    });
                    let window = callback_document
                        .default_view()
                        .ok_or_else(|| JsValue::from_str("browser window unavailable"))?;
                    if let Err(error) = window
                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                            cleanup.as_ref().unchecked_ref(),
                            0,
                        )
                    {
                        anchor.remove();
                        let _ = Url::revoke_object_url(&png_url);
                        return Err(error);
                    }
                    cleanup.forget();
                    set_export_status(&callback_document, "PNG exported · 2000 × 1400")
                })();
                if let Err(error) = outcome {
                    let _ = set_export_status(
                        &callback_document,
                        &format!("PNG export failed: {}", js_error_text(&error)),
                    );
                }
            });
            canvas.to_blob_with_type(callback.as_ref().unchecked_ref(), "image/png")?;
            callback.forget();
            Ok(())
        })();
        if let Err(error) = outcome {
            let _ = set_export_status(
                &loaded_document,
                &format!("PNG export failed: {}", js_error_text(&error)),
            );
        }
    });

    let failed_document = document.clone();
    let failed_source_url = source_url.clone();
    let onerror = Closure::<dyn FnMut()>::once(move || {
        let _ = Url::revoke_object_url(&failed_source_url);
        let _ = set_export_status(
            &failed_document,
            "PNG export failed: SVG rasterization failed",
        );
    });
    image.set_onload(Some(onload.as_ref().unchecked_ref()));
    image.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    image.set_src(&source_url);
    onload.forget();
    onerror.forget();
    set_export_status(document, "Preparing PNG export…")
}

#[cfg(target_arch = "wasm32")]
fn set_export_status(
    document: &web_sys::Document,
    message: &str,
) -> Result<(), wasm_bindgen::JsValue> {
    let status = document
        .get_element_by_id("wb-status-message")
        .ok_or_else(|| wasm_bindgen::JsValue::from_str("workbench status unavailable"))?;
    status.set_text_content(Some(message));
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn js_error_text(error: &wasm_bindgen::JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| "browser API rejected the export".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_export_is_self_contained_fixed_size_and_hides_hit_geometry() {
        let svg = standalone_export_svg(
            "<path class=\"wb-curve\" d=\"M 1 2 L 3 4\"/><circle class=\"wb-point\"/>",
        );
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.contains("width=\"2000\" height=\"1400\""));
        assert!(svg.contains("viewBox=\"0 0 1000 700\""));
        assert!(svg.contains("fill=\"#121617\""));
        assert!(svg.contains(".wb-computed-hit"));
        assert!(svg.contains(".wb-annotation-hit"));
        assert!(svg.contains(".wb-right-angle"));
        assert!(svg.contains(".wb-dimension.reference"));
        assert!(svg.contains(".wb-annotation.suppressed"));
        assert!(svg.contains("<path class=\"wb-curve\""));
        assert!(svg.ends_with("</svg>"));
    }
}
