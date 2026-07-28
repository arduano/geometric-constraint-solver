// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(target_arch = "wasm32")]
pub(crate) fn window() -> Result<web_sys::Window, wasm_bindgen::JsValue> {
    web_sys::window().ok_or_else(|| wasm_bindgen::JsValue::from_str("browser window unavailable"))
}
