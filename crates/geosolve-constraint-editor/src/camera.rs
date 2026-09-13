// SPDX-License-Identifier: GPL-3.0-or-later
//! Finite canvas navigation shared by detached editors and rendering hosts.
use crate::{EditorScene, ScreenPoint, Viewport};

pub const SCREEN_SIZE: [f64; 2] = [1000.0, 700.0];
pub const DEFAULT_PIXELS_PER_MODEL_UNIT: f64 = 50.0;
pub const MIN_PIXELS_PER_MODEL_UNIT: f64 = 2.0;
pub const MAX_PIXELS_PER_MODEL_UNIT: f64 = 2_000.0;
pub const FIT_MARGIN_PIXELS: f64 = 64.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CanvasCamera {
    screen_size: [f64; 2],
    model_center: [f64; 2],
    pixels_per_model_unit: f64,
}

impl Default for CanvasCamera {
    fn default() -> Self {
        Self {
            screen_size: SCREEN_SIZE,
            model_center: [0.0, 0.0],
            pixels_per_model_unit: DEFAULT_PIXELS_PER_MODEL_UNIT,
        }
    }
}

impl CanvasCamera {
    /// Constructs one finite camera inside the canonical scale interval.
    pub fn new(model_center: [f64; 2], pixels_per_model_unit: f64) -> Option<Self> {
        (model_center.into_iter().all(f64::is_finite)
            && pixels_per_model_unit.is_finite()
            && (MIN_PIXELS_PER_MODEL_UNIT..=MAX_PIXELS_PER_MODEL_UNIT)
                .contains(&pixels_per_model_unit))
        .then_some(Self {
            screen_size: SCREEN_SIZE,
            model_center,
            pixels_per_model_unit,
        })
    }

    pub const fn model_center(self) -> [f64; 2] {
        self.model_center
    }

    pub const fn pixels_per_model_unit(self) -> f64 {
        self.pixels_per_model_unit
    }

    /// Returns the validated viewport represented by this camera.
    ///
    /// # Panics
    ///
    /// Panics only if a private camera mutation violates the finite center,
    /// positive extent or positive finite scale invariants.
    pub fn viewport(self) -> Viewport {
        Viewport::new(
            self.screen_size,
            self.model_center,
            self.pixels_per_model_unit,
        )
        .expect("CanvasCamera construction and mutation preserve viewport invariants")
    }

    pub fn reset(&mut self) {
        self.model_center = [0.0, 0.0];
        self.pixels_per_model_unit = DEFAULT_PIXELS_PER_MODEL_UNIT;
    }

    /// Changes the CSS-pixel workplane extent while preserving its center and scale.
    /// Invalid or unchanged extents leave the camera untouched.
    pub fn resize(&mut self, screen_size: [f64; 2]) -> bool {
        if screen_size.map(f64::to_bits) == self.screen_size.map(f64::to_bits)
            || !screen_size
                .into_iter()
                .all(|extent| extent.is_finite() && extent > 0.0)
        {
            return false;
        }
        self.screen_size = screen_size;
        true
    }

    fn fit_margins(self) -> [f64; 2] {
        self.screen_size
            .map(|extent| FIT_MARGIN_PIXELS.min(extent * 0.25))
    }

    fn minimum_scale(self) -> f64 {
        // Preserve the canonical maximum visible model span when a smaller
        // workplane has fewer pixels available inside its fit margins. Static
        // exports retain their exact 1000 × 700 scale interval.
        let margins = self.fit_margins();
        let ratio = (0..2).fold(1.0_f64, |ratio, axis| {
            ratio.min(
                (self.screen_size[axis] - 2.0 * margins[axis])
                    / (SCREEN_SIZE[axis] - 2.0 * FIT_MARGIN_PIXELS),
            )
        });
        (MIN_PIXELS_PER_MODEL_UNIT * ratio).max(f64::MIN_POSITIVE)
    }

    pub fn center_origin(&mut self) -> bool {
        if self.model_center == [0.0, 0.0] {
            return false;
        }
        self.model_center = [0.0, 0.0];
        true
    }

    pub fn zoom_about(&mut self, anchor: ScreenPoint, factor: f64) -> bool {
        if !anchor.x.is_finite() || !anchor.y.is_finite() || !factor.is_finite() || factor <= 0.0 {
            return false;
        }
        let before = self.viewport().screen_to_model(anchor);
        // Resizing preserves zoom even when enlarging the host raises its floor.
        // A subsequent zoom-out must never jump inward to that new floor.
        let minimum = self.minimum_scale().min(self.pixels_per_model_unit);
        let next_scale =
            (self.pixels_per_model_unit * factor).clamp(minimum, MAX_PIXELS_PER_MODEL_UNIT);
        if (next_scale - self.pixels_per_model_unit).abs()
            <= f64::EPSILON * self.pixels_per_model_unit.max(1.0)
        {
            return false;
        }
        let mut candidate = *self;
        candidate.pixels_per_model_unit = next_scale;
        let after = candidate.viewport().screen_to_model(anchor);
        candidate.model_center = [
            candidate.model_center[0] + before[0] - after[0],
            candidate.model_center[1] + before[1] - after[1],
        ];
        if !candidate.model_center.into_iter().all(f64::is_finite) {
            return false;
        }
        *self = candidate;
        true
    }

    pub fn pan_from(
        &mut self,
        origin_center: [f64; 2],
        origin: ScreenPoint,
        current: ScreenPoint,
    ) -> bool {
        if !origin_center.into_iter().all(f64::is_finite)
            || !origin.x.is_finite()
            || !origin.y.is_finite()
            || !current.x.is_finite()
            || !current.y.is_finite()
        {
            return false;
        }
        let model_center = [
            origin_center[0] - (current.x - origin.x) / self.pixels_per_model_unit,
            origin_center[1] + (current.y - origin.y) / self.pixels_per_model_unit,
        ];
        if !model_center.into_iter().all(f64::is_finite) {
            return false;
        }
        self.model_center = model_center;
        true
    }

    pub fn fit_scene(&mut self, scene: &EditorScene) -> bool {
        self.fit_model_bounds(scene.model_bounds())
    }

    /// Fits explicit finite model bounds into this camera's canvas.
    ///
    /// Empty, reversed, non-finite, or uncontainable bounds are rejected
    /// without changing the camera. A successful fit guarantees that the
    /// complete bounds map finitely inside a 64 px margin (reduced to a quarter
    /// of each extent for small canvases). The canonical 2–2000 px/model-unit
    /// interval lowers its minimum proportionally for smaller usable extents.
    pub fn fit_model_bounds(&mut self, bounds: Option<([f64; 2], [f64; 2])>) -> bool {
        let Some((minimum, maximum)) = bounds else {
            return false;
        };
        if !minimum.into_iter().all(f64::is_finite)
            || !maximum.into_iter().all(f64::is_finite)
            || minimum
                .into_iter()
                .zip(maximum)
                .any(|(minimum, maximum)| minimum > maximum)
        {
            return false;
        }

        let spans = [maximum[0] - minimum[0], maximum[1] - minimum[1]];
        if !spans.into_iter().all(f64::is_finite) {
            return false;
        }
        let margins = self.fit_margins();
        let available = [
            self.screen_size[0] - 2.0 * margins[0],
            self.screen_size[1] - 2.0 * margins[1],
        ];
        let model_center = [minimum[0] + spans[0] * 0.5, minimum[1] + spans[1] * 0.5];
        let mut pixels_per_model_unit = MAX_PIXELS_PER_MODEL_UNIT;
        for (span, available) in spans.into_iter().zip(available) {
            if span > 0.0 {
                pixels_per_model_unit = pixels_per_model_unit.min(available / span);
            }
        }
        if !model_center.into_iter().all(f64::is_finite)
            || !pixels_per_model_unit.is_finite()
            || pixels_per_model_unit < self.minimum_scale()
        {
            return false;
        }
        pixels_per_model_unit = pixels_per_model_unit.min(MAX_PIXELS_PER_MODEL_UNIT);

        let candidate = Self {
            screen_size: self.screen_size,
            model_center,
            pixels_per_model_unit,
        };
        if !bounds_fit_with_margin(candidate, minimum, maximum) {
            return false;
        }

        *self = candidate;
        true
    }

    /// Fits finite native geometry, or returns an empty workplane to the canonical Origin view.
    pub fn fit_scene_or_reset(&mut self, scene: Option<&EditorScene>) -> bool {
        if scene.is_some_and(|scene| self.fit_scene(scene)) {
            return true;
        }
        self.reset();
        false
    }
}

fn bounds_fit_with_margin(camera: CanvasCamera, minimum: [f64; 2], maximum: [f64; 2]) -> bool {
    const CONTAINMENT_EPSILON_PIXELS: f64 = 1.0e-7;

    let viewport = camera.viewport();
    let margins = camera.fit_margins();
    [
        minimum,
        [minimum[0], maximum[1]],
        [maximum[0], minimum[1]],
        maximum,
    ]
    .into_iter()
    .map(|point| viewport.model_to_screen(point))
    .all(|point| {
        point.x.is_finite()
            && point.y.is_finite()
            && point.x >= margins[0] - CONTAINMENT_EPSILON_PIXELS
            && point.x <= viewport.screen_size[0] - margins[0] + CONTAINMENT_EPSILON_PIXELS
            && point.y >= margins[1] - CONTAINMENT_EPSILON_PIXELS
            && point.y <= viewport.screen_size[1] - margins[1] + CONTAINMENT_EPSILON_PIXELS
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn camera_fit_uses_wide_tall_and_small_live_extents() {
        for extent in [[1800.0, 400.0], [400.0, 1200.0], [120.0, 90.0]] {
            let mut camera = CanvasCamera::default();
            assert!(camera.resize(extent));
            assert!(camera.fit_model_bounds(Some(([-5.0, -3.0], [5.0, 3.0]))));
            assert_eq!(
                camera.viewport().screen_size.map(f64::to_bits),
                extent.map(f64::to_bits)
            );
            assert!(bounds_fit_with_margin(camera, [-5.0, -3.0], [5.0, 3.0]));
        }
    }

    #[test]
    fn camera_fits_scale_samples_in_narrow_canvas_and_zoom_never_reverses_direction() {
        for extent in [[771.109_375, 1120.0], [320.0, 700.0], [120.0, 90.0]] {
            let mut camera = CanvasCamera::default();
            assert!(camera.resize(extent));
            // Public fixture field/backplane extents; fitting must remain possible
            // when their CSS-pixel scale falls below the canonical camera floor.
            assert!(camera.fit_model_bounds(Some(([-180.0, -110.0], [180.0, 110.0]))));
            assert!(bounds_fit_with_margin(
                camera,
                [-180.0, -110.0],
                [180.0, 110.0]
            ));
            let fit_scale = camera.pixels_per_model_unit();
            assert!(fit_scale < MIN_PIXELS_PER_MODEL_UNIT);
            let anchor = ScreenPoint {
                x: extent[0] * 0.5,
                y: extent[1] * 0.5,
            };
            assert!(camera.zoom_about(anchor, 0.95));
            assert!(camera.pixels_per_model_unit() < fit_scale);
            let before_expand = camera.pixels_per_model_unit();
            assert!(camera.resize(SCREEN_SIZE));
            let _ = camera.zoom_about(anchor, 0.95);
            assert!(camera.pixels_per_model_unit() <= before_expand);
            assert!(camera.zoom_about(anchor, 1.05));
            assert!(camera.pixels_per_model_unit() > before_expand);
        }
    }

    #[test]
    fn camera_fit_rejects_empty_and_invalid_bounds_without_mutation() {
        let original = CanvasCamera::new([4.0, -3.0], 75.0).expect("valid camera");
        let mut camera = original;

        assert!(!camera.fit_model_bounds(None));
        assert_eq!(camera, original);
        assert!(!camera.fit_model_bounds(Some(([2.0, 0.0], [1.0, 1.0]))));
        assert_eq!(camera, original);
        assert!(!camera.fit_model_bounds(Some(([f64::NAN, 0.0], [1.0, 1.0]))));
        assert_eq!(camera, original);
        assert!(!camera.fit_model_bounds(Some(([0.0, 0.0], [f64::INFINITY, 1.0]))));
        assert_eq!(camera, original);
    }

    #[test]
    fn camera_pan_rejects_finite_overflow_without_mutation() {
        let mut camera = CanvasCamera::default();
        let original = camera;

        assert!(!camera.pan_from(
            [f64::MAX, 0.0],
            ScreenPoint {
                x: -f64::MAX,
                y: 0.0,
            },
            ScreenPoint {
                x: f64::MAX,
                y: 0.0,
            },
        ));
        assert_eq!(camera, original);
    }

    #[test]
    fn camera_fit_handles_points_degenerate_axes_and_off_origin_bounds() {
        let mut camera = CanvasCamera::default();

        assert!(camera.fit_model_bounds(Some(([12.0, -8.0], [12.0, -8.0]))));
        assert_eq!(
            camera.model_center.map(f64::to_bits),
            [12.0_f64.to_bits(), (-8.0_f64).to_bits()]
        );
        assert_eq!(
            camera.pixels_per_model_unit.to_bits(),
            MAX_PIXELS_PER_MODEL_UNIT.to_bits()
        );

        assert!(camera.fit_model_bounds(Some(([100.0, -40.0], [100.0 + 1.0e-12, -38.0],))));
        assert_eq!(camera.model_center[1].to_bits(), (-39.0_f64).to_bits());
        assert!((camera.pixels_per_model_unit - 286.0).abs() <= f64::EPSILON);

        assert!(camera.fit_model_bounds(Some(([100.0, -40.0], [104.0, -30.0]))));
        assert_eq!(
            camera.model_center.map(f64::to_bits),
            [102.0_f64.to_bits(), (-35.0_f64).to_bits()]
        );
        assert!((camera.pixels_per_model_unit - 57.2).abs() <= 1.0e-12);
    }

    #[test]
    fn camera_fit_rejects_uncontainable_and_overflowing_finite_bounds() {
        let mut camera = CanvasCamera::default();
        let original = camera;

        assert!(!camera.fit_model_bounds(Some(([-1_000.0, 0.0], [1_000.0, 0.0]))));
        assert_eq!(camera, original);
        assert!(!camera.fit_model_bounds(Some(([-f64::MAX, -f64::MAX], [f64::MAX, f64::MAX],))));
        assert_eq!(camera, original);

        assert!(camera.fit_model_bounds(Some(([f64::MAX, f64::MAX], [f64::MAX, f64::MAX],))));
        assert_eq!(
            camera.model_center.map(f64::to_bits),
            [f64::MAX.to_bits(), f64::MAX.to_bits()]
        );
        assert_eq!(
            camera.pixels_per_model_unit.to_bits(),
            MAX_PIXELS_PER_MODEL_UNIT.to_bits()
        );
    }
}
