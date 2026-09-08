// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{OperationControl, OperationOutcome};
use geosolve_sketch_topology::{TopologyRequest, TopologySelfIntersectionPolicy, TopologySnapshot};

use super::WorkbenchBridge;

impl WorkbenchBridge {
    pub(crate) fn bake_profile_json(&self, max_chord_error_mm: f64) -> Result<String, String> {
        if self.pending_managed_mutation.is_some()
            || self.captured_pointer.is_some()
            || self.last_error.is_some()
        {
            return Err("bake requires settled accepted source and no active gesture/error".into());
        }
        // Authenticate complete current code authority; never export a retained failed draft.
        super::super::code_projects::canonical_code_project_files(self.code_project.as_ref())
            .map_err(|error| error.to_string())?;
        let coordinator = self
            .authority
            .projectional_ref()
            .ok_or("bake requires an accepted managed sketch")?
            .coordinator();
        let accepted = coordinator
            .accepted_materialization()
            .ok_or("bake has no accepted geometry")?;
        if accepted.validation.semantic != coordinator.intent().semantic_identity()
            || !accepted.validation.hard_residuals_validated
            || !accepted.validation.all_active_features_current
        {
            return Err("bake refuses stale or unvalidated accepted geometry".into());
        }
        if !accepted.features.features().is_empty() {
            return Err("bake v1 does not support computed-feature geometry; refusing an incomplete native-only export".into());
        }
        let mut request = TopologyRequest::default();
        request.policy.self_intersections = TopologySelfIntersectionPolicy::Reject;
        let outcome = TopologySnapshot::capture(&accepted.session)
            .map_err(|error| error.to_string())?
            .prepare(request)
            .execute(OperationControl::default())
            .map_err(|error| error.to_string())?;
        let OperationOutcome::Completed { value, .. } = outcome else {
            return Err("bake topology query did not complete".into());
        };
        let profile = value.production_profile.ok_or_else(|| {
            format!(
                "bake requires complete production topology: {:?}: {:?}",
                value.completeness, value.issues
            )
        })?;
        let regions = profile
            .sample_polygons(&accepted.session, max_chord_error_mm)
            .map_err(|error| error.to_string())?;
        let regions = regions
            .into_iter()
            .map(|region| {
                serde_json::json!({
                    "id": region.id, "outer": region.outer, "holes": region.holes,
                })
            })
            .collect::<Vec<_>>();
        // File provenance is added by the CLI after checking the exact accepted disk bytes.
        serde_json::to_string(&serde_json::json!({
            "format": "geosolve-baked-profile-v1",
            "units": "mm",
            "plane": {"origin": [0, 0, 0], "x_axis": [1, 0, 0], "y_axis": [0, 1, 0]},
            "sampling": {"max_chord_error_mm": max_chord_error_mm},
            "regions": regions,
        }))
        .map_err(|error| error.to_string())
    }
}
