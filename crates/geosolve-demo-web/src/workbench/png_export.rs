// SPDX-License-Identifier: GPL-3.0-or-later

//! Presentation-only PNG export for the authoritative sketch canvas.
//!
//! This module deliberately consumes the already composed SVG scene. It does
//! not inspect the sketch document, recreate geometry, or participate in
//! accepted-scene authority.

pub(crate) use geosolve_sketch_render::standalone_export_svg;
#[cfg(target_arch = "wasm32")]
use geosolve_sketch_render::{PNG_EXPORT_HEIGHT, PNG_EXPORT_WIDTH};

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
        assert!(svg.contains(".wb-draft"));
        assert!(svg.contains(".wb-inference-guides"));
        assert!(svg.contains(".wb-inference-candidates"));
        assert!(svg.contains(".wb-right-angle"));
        assert!(svg.contains(".wb-dimension.reference"));
        assert!(svg.contains(".wb-annotation.suppressed"));
        assert!(svg.contains("<path class=\"wb-curve\""));
        assert!(svg.ends_with("</svg>"));
    }
}
