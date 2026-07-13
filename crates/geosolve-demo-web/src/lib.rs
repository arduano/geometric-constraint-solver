//! Minimal WASM/SVG visual harness for hardcoded solver scenarios.

#[cfg(any(target_arch = "wasm32", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoScenario {
    UnderconstrainedTriangle,
    FourBar,
    SliderCrank,
}

#[cfg(any(target_arch = "wasm32", test))]
impl DemoScenario {
    fn from_value(value: &str) -> Self {
        match value {
            "four-bar" => Self::FourBar,
            "slider-crank" => Self::SliderCrank,
            _ => Self::UnderconstrainedTriangle,
        }
    }

    fn svg_markup(self) -> &'static str {
        match self {
            Self::UnderconstrainedTriangle => {
                r#"<g class="geometry">
                    <path d="M 150 330 L 390 330 L 310 130 Z" />
                    <circle cx="150" cy="330" r="7" />
                    <circle cx="390" cy="330" r="7" />
                    <circle cx="310" cy="130" r="7" class="draggable" />
                    <text x="24" y="42">Sketch scaffold: underconstrained triangle</text>
                </g>"#
            }
            Self::FourBar => {
                r#"<g class="geometry linkage">
                    <path d="M 125 330 L 245 215 L 430 245 L 500 330" />
                    <path d="M 125 330 L 500 330" class="ground" />
                    <circle cx="125" cy="330" r="8" />
                    <circle cx="245" cy="215" r="8" />
                    <circle cx="430" cy="245" r="8" />
                    <circle cx="500" cy="330" r="8" />
                    <text x="24" y="42">Linkage scaffold: four-bar, open assembly</text>
                </g>"#
            }
            Self::SliderCrank => {
                r#"<g class="geometry linkage">
                    <path d="M 145 280 L 270 195 L 465 280" />
                    <path d="M 100 280 L 520 280" class="ground" />
                    <rect x="440" y="252" width="50" height="56" rx="5" />
                    <circle cx="145" cy="280" r="8" />
                    <circle cx="270" cy="195" r="8" />
                    <circle cx="465" cy="280" r="8" />
                    <text x="24" y="42">Linkage scaffold: slider-crank</text>
                </g>"#
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::DemoScenario;
    use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
    use web_sys::{Document, Event, HtmlSelectElement};

    fn render(document: &Document, scenario: DemoScenario) -> Result<(), JsValue> {
        let viewport = document
            .get_element_by_id("viewport")
            .ok_or_else(|| JsValue::from_str("missing #viewport SVG element"))?;
        viewport.set_inner_html(scenario.svg_markup());
        Ok(())
    }

    #[wasm_bindgen(start)]
    pub fn start() -> Result<(), JsValue> {
        console_error_panic_hook::set_once();
        let document = web_sys::window()
            .and_then(|window| window.document())
            .ok_or_else(|| JsValue::from_str("browser document is unavailable"))?;

        let select = document
            .get_element_by_id("scenario")
            .ok_or_else(|| JsValue::from_str("missing #scenario select"))?
            .dyn_into::<HtmlSelectElement>()?;

        render(&document, DemoScenario::UnderconstrainedTriangle)?;

        let callback_document = document.clone();
        let callback_select = select.clone();
        let callback = Closure::<dyn FnMut(Event)>::new(move |_| {
            let scenario = DemoScenario::from_value(&callback_select.value());
            let _ = render(&callback_document, scenario);
        });

        select.add_event_listener_with_callback("change", callback.as_ref().unchecked_ref())?;
        callback.forget();
        Ok(())
    }
}

/// Allows native checks to verify that this crate remains a portable library.
#[must_use]
pub const fn scenario_count() -> usize {
    3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_required_initial_demo_scenarios_exist() {
        assert_eq!(scenario_count(), 3);
        assert!(DemoScenario::FourBar.svg_markup().contains("four-bar"));
        assert_eq!(
            DemoScenario::from_value("slider-crank"),
            DemoScenario::SliderCrank
        );
    }
}
