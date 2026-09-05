// SPDX-License-Identifier: GPL-3.0-or-later

//! Internal M92 evidence exporter; the public headless report format is unchanged.

use std::collections::BTreeSet;
use std::error::Error;

use geosolve_constraint_editor::{IntentNativeBinding, Viewport};
use geosolve_sketch::{CurveDefinition, DocumentId, PersistentId};
use geosolve_sketch_code::{
    CodeProject, KeyedReconcileState, bundled_sample, materialize_code_project_cold,
    required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;
use serde_json::json;

fn read_project() -> Result<CodeProject, Box<dyn Error>> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    Ok(match arguments.as_slice() {
        [flag, key] if flag == "--sample" => bundled_sample(key)
            .ok_or("unknown bundled sample")?
            .project(),
        [flag, path] if flag == "--project" => {
            CodeProject::from_json(&std::fs::read_to_string(path)?)?
        }
        _ => return Err("usage: m92_sample_audit (--sample KEY | --project FILE)".into()),
    })
}

fn main() -> Result<(), Box<dyn Error>> {
    let project = read_project()?;
    let generated = KeyedReconcileState::empty()
        .plan(required_generated_members(&project)?, &BTreeSet::new())?
        .into_staged();
    let materialized = materialize_code_project_cold(
        &project,
        &generated,
        IntentSessionId::from_raw(0x92_0500),
        DocumentId(PersistentId::from_u128(0x92_0501)),
        1.0,
    )?;
    let coordinator = materialized.editor.coordinator();
    let accepted = coordinator
        .accepted_materialization()
        .ok_or("missing accepted materialization")?;
    if !accepted.validation.hard_residuals_validated
        || !accepted.validation.all_active_features_current
        || accepted
            .validation
            .maximum_normalized_hard_residual
            .is_some_and(|value| !value.is_finite() || value > 1.0e-9)
    {
        return Err("independent accepted validation failed".into());
    }
    let state = accepted
        .session
        .accepted_state_for_current_input()
        .ok_or("missing current accepted state")?;
    let document = state.document();
    let mut curves = Vec::new();
    for curve in document.curves() {
        let owner = accepted
            .ownership
            .exact_owner(IntentNativeBinding::Curve(curve.id));
        let declaration = owner.and_then(|owner| {
            materialized
                .expansion
                .declaration_provenance
                .iter()
                .find_map(|(alias, declaration)| {
                    coordinator
                        .intent()
                        .graph()
                        .node_by_symbol(alias)
                        .filter(|node| node.id == owner)
                        .map(|_| declaration.0.as_str())
                })
        });
        let periodic = matches!(
            curve.definition,
            CurveDefinition::Circle { .. } | CurveDefinition::Ellipse { .. }
        );
        let mut spans = Vec::new();
        for span in document.curve_spans(curve.id)? {
            let mut positions = Vec::new();
            for step in 0..=32 {
                let parameter =
                    f64::from(step) / 32.0 * if periodic { std::f64::consts::TAU } else { 1.0 };
                let point = document.evaluate_curve_jet(span, parameter)?.position;
                positions.push([point.x, point.y]);
            }
            spans.push(json!({"span": span, "positions": positions}));
        }
        curves.push(json!({
            "id": curve.id, "label": curve.label, "declaration": declaration,
            "definition": curve.definition, "role": document.geometry_role(curve.id),
            "spans": spans,
        }));
    }
    let scene = materialized
        .editor
        .scene(Viewport::new([1000.0, 700.0], [0.0, 0.0], 1.0)?, 0.25)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "source_digest": project.managed.source_digest,
            "expansion_digest": materialized.expansion.digest,
            "validation": accepted.validation,
            "document": serde_json::from_str::<serde_json::Value>(&document.to_draft_v5_json()?)?,
            "points": document.points(), "scalars": document.scalars(), "curves": curves,
            "visible_native_curves": scene.curves.len(),
            "visible_computed_curves": scene.computed_curves.len(),
            "computed_curves": scene.computed_curves.iter().map(|curve| json!({
                "feature": curve.owner.feature, "corner": curve.owner.corner,
                "center": curve.center, "radius": curve.radius,
                "start_angle": curve.start_angle, "end_angle": curve.end_angle,
                "contacts": curve.contacts.map(|contact| json!({
                    "parameter": contact.parameter, "winding": contact.winding,
                    "total_parameter": contact.total_parameter, "position": contact.position,
                })), "sweep": curve.sweep,
            })).collect::<Vec<_>>(),
        }))?
    );
    Ok(())
}
