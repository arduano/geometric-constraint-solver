// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{IntentNativeBinding, ProjectionalEditorSession};
use geosolve_sketch::{CurveDefinition, CurveSpan, SketchDocument};
use geosolve_sketch_intent::{GeometryRecipeKind, IntentNodeKind, IntentPatchOperation};

use crate::ExpandedCodeProject;

/// Transports dormant, source-derived Polyline reference directions at an
/// authenticated point terminal. Returns the accepted and seeded design documents,
/// respectively, or `None` when no reference needs transport. Positions, scalars
/// and all other durable fields stay exact.
///
/// The caller supplies the gesture-origin editor and its authenticated code
/// expansion, never a client-provided document or expansion as authority. Explicit
/// source branch fields are excluded. The sketch owner authenticates the origin
/// branch bits and permits only unenforced references to follow actual endpoint
/// rotation. Ordinary branch-cell comparison and independent residual validation
/// still apply when the caller materializes and audits the source candidate.
///
/// # Errors
/// Rejects missing/mismatched ownership, foreign terminal structure, forged branch
/// metadata, invalid span geometry, or a branch-enforced reference transition.
#[allow(
    clippy::too_many_lines,
    reason = "one source ownership audit selects only changed crossing spans before domain transport"
)]
pub fn transport_code_point_terminal_branches(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    accepted_terminal: &SketchDocument,
    seeded_design: &SketchDocument,
) -> Result<Option<(SketchDocument, SketchDocument)>, String> {
    let authority = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "point branch transport has no accepted origin".to_owned())?;
    let origin = authority
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "point branch transport has no current origin".to_owned())?
        .document();
    let graph = editor.coordinator().intent().graph();
    let seeded_points = seeded_design
        .points()
        .iter()
        .map(|point| (point.id, point.position))
        .collect::<BTreeMap<_, _>>();
    let changed_points = origin
        .points()
        .iter()
        .filter(|point| {
            seeded_points
                .get(&point.id)
                .map(|value| value.map(f64::to_bits))
                != Some(point.position.map(f64::to_bits))
        })
        .map(|point| point.id)
        .collect::<BTreeSet<_>>();
    if changed_points.is_empty() {
        return Ok(None);
    }
    let mut spans = BTreeSet::new();
    for operation in expansion.patch.operations() {
        let IntentPatchOperation::CreateNode { draft, .. } = operation else {
            continue;
        };
        if !matches!(
            draft.kind,
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::Polyline
            }
        ) || draft
            .fields
            .keys()
            .any(|key| key.0.as_str().starts_with("branch_direction_"))
        {
            continue;
        }
        let node = graph
            .node_by_symbol(&draft.symbol)
            .ok_or_else(|| "source-derived Polyline lost its origin node".to_owned())?;
        if node.suppressed {
            continue;
        }
        if node.kind != draft.kind || node.fields != draft.fields {
            return Err("source-derived Polyline differs from its origin fields".into());
        }
        let ownership = authority
            .ownership
            .node(node.id)
            .ok_or_else(|| "source-derived Polyline lost its origin ownership".to_owned())?;
        for binding in &ownership.owned {
            let IntentNativeBinding::Curve(curve) = binding else {
                continue;
            };
            let Some(CurveDefinition::Polyline {
                points,
                branch_directions,
                ..
            }) = origin.curve(*curve).map(|curve| &curve.definition)
            else {
                return Err("source-derived Polyline has a foreign native curve".into());
            };
            for (index, branch) in branch_directions.iter().enumerate() {
                let start = points[index];
                let end = points[(index + 1) % points.len()];
                if !changed_points.contains(&start) && !changed_points.contains(&end) {
                    continue;
                }
                let [Some(start), Some(end)] = [seeded_points.get(&start), seeded_points.get(&end)]
                else {
                    return Err("source-derived Polyline lost a terminal endpoint".into());
                };
                let delta = [end[0] - start[0], end[1] - start[1]];
                let length = delta[0].hypot(delta[1]);
                if !length.is_finite() || length <= 0.0 {
                    return Err(
                        "source-derived Polyline terminal span is not finite and nonzero".into(),
                    );
                }
                // This is only a candidate filter. The sketch owner independently
                // derives the reference again and authenticates all branch metadata.
                let dot = branch[0] * (delta[0] / length) + branch[1] * (delta[1] / length);
                if dot.is_finite() && dot > 0.0 {
                    continue;
                }
                spans.insert(CurveSpan {
                    curve: *curve,
                    segment: u32::try_from(index)
                        .map_err(|_| "source-derived Polyline span index overflow".to_owned())?,
                });
            }
        }
    }
    if spans.is_empty() {
        return Ok(None);
    }
    let accepted = accepted_terminal
        .transport_unenforced_source_line_branches(origin, seeded_design, &spans)
        .map_err(|error| error.to_string())?;
    let design = seeded_design
        .transport_unenforced_source_line_branches(origin, seeded_design, &spans)
        .map_err(|error| error.to_string())?;
    Ok(Some((accepted, design)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CodeProject, CompiledManagedSource, KeyedReconcileState, ProjectKey,
        materialize_code_project_cold, required_generated_members,
    };
    use geosolve_sketch::{DocumentId, PersistentId};
    use geosolve_sketch_intent::IntentSessionId;

    #[test]
    fn ordinary_point_terminals_do_not_allocate_branch_transport_documents() {
        let compiled = CompiledManagedSource::from_json(include_str!(
            "../../geosolve-sketch-engine/tests/fixtures/tool-operation-feature.json"
        ))
        .unwrap();
        let project =
            CodeProject::managed(ProjectKey("branch-transport".into()), compiled).unwrap();
        let generated = KeyedReconcileState::empty()
            .plan(
                required_generated_members(&project).unwrap(),
                &BTreeSet::new(),
            )
            .unwrap()
            .into_staged();
        let materialized = materialize_code_project_cold(
            &project,
            &generated,
            IntentSessionId::from_raw(98_041),
            DocumentId(PersistentId::from_u128(98_041)),
            1.0,
        )
        .unwrap();
        let origin = materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .session
            .accepted_state_for_current_input()
            .unwrap()
            .document();
        let corner = origin
            .points()
            .iter()
            .find(|point| point.position.map(f64::to_bits) == [220.0_f64, 0.0].map(f64::to_bits))
            .unwrap()
            .id;
        for destination in [[220.0, 0.0], [216.0, 4.0]] {
            let mut terminal = origin.clone();
            terminal.set_point_position(corner, destination).unwrap();
            assert!(
                transport_code_point_terminal_branches(
                    &materialized.editor,
                    &materialized.expansion,
                    &terminal,
                    &terminal,
                )
                .unwrap()
                .is_none(),
                "no full document transport for an ordinary point release"
            );
        }
        let mut terminal = origin.clone();
        terminal.set_point_position(corner, [215.0, 21.0]).unwrap();
        let (accepted, design) = transport_code_point_terminal_branches(
            &materialized.editor,
            &materialized.expansion,
            &terminal,
            &terminal,
        )
        .unwrap()
        .expect("crossing reference requires explicit transport");
        assert_eq!(accepted.points(), terminal.points());
        assert_eq!(design, accepted);
        let curve = &accepted.curves()[0];
        let direction = accepted
            .curve_branch_direction(CurveSpan {
                curve: curve.id,
                segment: 1,
            })
            .unwrap();
        assert!(direction[0] > 0.0 && direction[1] < 0.0);
    }
}
