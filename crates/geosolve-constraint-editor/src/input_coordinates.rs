// SPDX-License-Identifier: GPL-3.0-or-later
//! Target-independent mapping from a fitted client rectangle into native canvas coordinates.
use crate::ScreenPoint;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClientRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// Maps CSS client coordinates into the editor's current screen coordinate system.
/// The live canvas uses matching CSS extents; mismatched embedded views retain
/// uniform fitting. Device scale deliberately does not enter this conversion.
pub fn normalize_client_point(
    rect: ClientRect,
    screen_size: [f64; 2],
    client: [f64; 2],
) -> Option<ScreenPoint> {
    normalize_client_point_inner(rect, screen_size, client, true)
}

/// Preserves the pre-M74 coordinate translation for an already captured
/// pointer. Capture owns move and terminal samples even when the pointer
/// crosses an SVG letterbox band or leaves the mapped sketch plane.
pub fn normalize_captured_client_point(
    rect: ClientRect,
    screen_size: [f64; 2],
    client: [f64; 2],
) -> Option<ScreenPoint> {
    normalize_client_point_inner(rect, screen_size, client, false)
}

/// Lifecycle action for a browser sample that cannot be mapped into the
/// fitted sketch plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnmappedCanvasPointerAction {
    /// An uncaptured pointer entered an SVG letterbox band. Its previous hover
    /// and stationary sample no longer describe the pointer's semantic owner.
    RevokePointerContext,
    /// A captured interaction retains ownership across the fitted-plane edge.
    PreserveCapturedGesture,
}

#[must_use]
pub const fn unmapped_canvas_pointer_action(
    pointer_is_captured: bool,
) -> UnmappedCanvasPointerAction {
    if pointer_is_captured {
        UnmappedCanvasPointerAction::PreserveCapturedGesture
    } else {
        UnmappedCanvasPointerAction::RevokePointerContext
    }
}

fn normalize_client_point_inner(
    rect: ClientRect,
    screen_size: [f64; 2],
    client: [f64; 2],
    reject_letterbox: bool,
) -> Option<ScreenPoint> {
    let [screen_width, screen_height] = screen_size;
    if !rect.left.is_finite()
        || !rect.top.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || !screen_width.is_finite()
        || !screen_height.is_finite()
        || !client.into_iter().all(f64::is_finite)
        || rect.width <= 0.0
        || rect.height <= 0.0
        || screen_width <= 0.0
        || screen_height <= 0.0
    {
        return None;
    }
    let scale = (rect.width / screen_width).min(rect.height / screen_height);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let left = rect.left + (rect.width - screen_width * scale) * 0.5;
    let top = rect.top + (rect.height - screen_height * scale) * 0.5;
    let right = left + screen_width * scale;
    let bottom = top + screen_height * scale;
    if reject_letterbox
        && (client[0] < left || client[0] > right || client[1] < top || client[1] > bottom)
    {
        return None;
    }
    Some(ScreenPoint {
        x: (client[0] - left) / scale,
        y: (client[1] - top) / scale,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_normalization_rejects_non_positive_extents() {
        for rect in [
            ClientRect {
                left: 0.0,
                top: 0.0,
                width: 0.0,
                height: 100.0,
            },
            ClientRect {
                left: 0.0,
                top: 0.0,
                width: 100.0,
                height: -1.0,
            },
        ] {
            assert_eq!(
                normalize_client_point(rect, [1000.0, 700.0], [0.0, 0.0]),
                None
            );
        }
    }

    #[test]
    fn client_normalization_accounts_for_letterboxing_and_css_size_only() {
        let widescreen = ClientRect {
            left: 10.0,
            top: 20.0,
            width: 2000.0,
            height: 700.0,
        };
        assert_eq!(
            normalize_client_point(widescreen, [1000.0, 700.0], [510.0, 20.0]),
            Some(crate::ScreenPoint { x: 0.0, y: 0.0 })
        );
        assert_eq!(
            normalize_client_point(widescreen, [1000.0, 700.0], [1510.0, 720.0]),
            Some(crate::ScreenPoint {
                x: 1000.0,
                y: 700.0
            })
        );
        for point in [[509.999, 350.0], [1510.001, 350.0]] {
            assert_eq!(
                normalize_client_point(widescreen, [1000.0, 700.0], point),
                None,
                "horizontal letterbox band at {point:?} must not become canvas input"
            );
            assert!(
                normalize_captured_client_point(widescreen, [1000.0, 700.0], point).is_some(),
                "captured interaction must retain its historical translated sample at {point:?}"
            );
            assert_eq!(
                unmapped_canvas_pointer_action(false),
                UnmappedCanvasPointerAction::RevokePointerContext,
            );
            assert_eq!(
                unmapped_canvas_pointer_action(true),
                UnmappedCanvasPointerAction::PreserveCapturedGesture,
            );
        }
        let alternate_css = ClientRect {
            left: 100.0,
            top: 50.0,
            width: 500.0,
            height: 350.0,
        };
        assert_eq!(
            normalize_client_point(alternate_css, [1000.0, 700.0], [350.0, 225.0]),
            Some(crate::ScreenPoint { x: 500.0, y: 350.0 })
        );

        let portrait = ClientRect {
            left: 40.0,
            top: 10.0,
            width: 500.0,
            height: 700.0,
        };
        // The fitted viewBox is 500x350 CSS pixels, vertically centred at y=185.
        for point in [[290.0, 184.999], [290.0, 535.001]] {
            assert_eq!(
                normalize_client_point(portrait, [1000.0, 700.0], point),
                None,
                "vertical letterbox band at {point:?} must not become canvas input"
            );
            assert!(
                normalize_captured_client_point(portrait, [1000.0, 700.0], point).is_some(),
                "captured interaction must retain its historical translated sample at {point:?}"
            );
        }
        assert_eq!(
            normalize_client_point(portrait, [1000.0, 700.0], [40.0, 185.0]),
            Some(crate::ScreenPoint { x: 0.0, y: 0.0 })
        );
    }
}
