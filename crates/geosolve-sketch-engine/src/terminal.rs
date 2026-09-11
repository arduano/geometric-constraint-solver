// SPDX-License-Identifier: GPL-3.0-or-later
//! Canonical native terminal parity extracted from the workbench's existing code-owner path.
//! No source compiler, browser state or solver equations live here.

use super::point_gesture::expanded_port_point;
use geosolve_constraint_editor::{
    ComputedEdgeGeometry, ComputedEdgeProvenance, ComputedFeatureDefinition,
    ComputedFeatureDocument, ComputedFeatureEvaluation, ComputedFeatureEvaluationState,
    ComputedFeatureSnapshot, IntentNativeBinding, NativeCurveSpanSource, ProjectionalEditorSession,
};
use geosolve_sketch::{
    CurveSpan, DocumentObjectId, DocumentObjectRelabel, OperationControl, OperationOutcome,
    RetainedSketchDocumentSession, SketchHardValidity,
};
use geosolve_sketch_code::{
    CodeInteractionOverlay, CodePointEdit, CodeProject, CodeRectangleCorner, CodeWritableAddress,
    ExpandedCodeProject, ExpandedPort, ExpandedSemanticTarget, ExpandedWritablePoint,
    KeyedReconcileState, MaterializedCodeProject, SemanticSymbol,
    materialize_code_project_incremental_with_overlay_and_accepted_continuation_audited,
    stage_point_drags,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
};
use geosolve_sketch_intent::{GeometryRecipeKind, IntentKey, IntentNodeKind, NodeId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
struct TerminalPointPreview<'a> {
    editor: &'a ProjectionalEditorSession,
    session: &'a RetainedSketchDocumentSession,
}

impl<'a> TerminalPointPreview<'a> {
    fn from_accepted(editor: &'a ProjectionalEditorSession) -> Result<Self, String> {
        let session = &editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| "terminal code drag has no accepted native authority".to_owned())?
            .session;
        validate_terminal_preview_session(session)?;
        Ok(Self { editor, session })
    }

    fn point(self, handle: &ExpandedPort) -> Option<geosolve_sketch::DesignPointId> {
        expanded_port_point(self.editor, handle)
    }

    fn position(self, handle: &ExpandedPort) -> Option<[f64; 2]> {
        let point = self.point(handle)?;
        let position = self
            .session
            .accepted_state_for_current_input()?
            .document()
            .point(point)?
            .position;
        position.into_iter().all(f64::is_finite).then_some(position)
    }
}

#[derive(Clone, Debug)]
struct RectangleTerminalProjection {
    anchors: [ExpandedPort; 2],
    redundant_aliases: [ExpandedPort; 2],
}

#[derive(Clone, Debug)]
struct CanonicalTerminalPointBundle {
    placements: Vec<(ExpandedWritablePoint, [f64; 2])>,
    rectangle_projections: Vec<RectangleTerminalProjection>,
}

// This is an arithmetic-depth budget applied to a semantic coordinate scale,
// not a fixed ULP-distance gate. Rectangle aliases are independently solved
// projections of two canonical seeds, so their last-bit drift is relative to
// the authenticated rectangle's own seeds and aliases rather than unrelated
// document geometry or one (possibly near-zero) coordinate.
const TERMINAL_SEED_ROUNDOFF_FACTOR: f64 = 8.0;
const TERMINAL_SEED_ZERO_ROUNDOFF: f64 = 32.0 * f64::EPSILON;
const F64_SIGN_MASK: u64 = 0x8000_0000_0000_0000;

fn rectangle_corner(edit: &CodePointEdit) -> Option<CodeRectangleCorner> {
    match edit {
        CodePointEdit::RectangleCorner { corner, .. } => Some(*corner),
        CodePointEdit::Point { .. } => None,
    }
}

const fn opposite_rectangle_corner(corner: CodeRectangleCorner) -> CodeRectangleCorner {
    match corner {
        CodeRectangleCorner::LowerLeft => CodeRectangleCorner::UpperRight,
        CodeRectangleCorner::LowerRight => CodeRectangleCorner::UpperLeft,
        CodeRectangleCorner::UpperRight => CodeRectangleCorner::LowerLeft,
        CodeRectangleCorner::UpperLeft => CodeRectangleCorner::LowerRight,
    }
}

struct RectangleLensIndex<'a> {
    groups: BTreeMap<
        (CodeWritableAddress, CodeWritableAddress),
        BTreeMap<CodeRectangleCorner, &'a ExpandedWritablePoint>,
    >,
}

impl<'a> RectangleLensIndex<'a> {
    fn new(expansion: &'a ExpandedCodeProject) -> Result<Self, String> {
        let mut groups = BTreeMap::<_, BTreeMap<_, _>>::new();
        let mut effective = BTreeMap::new();
        let mut ordinary = BTreeSet::new();
        let mut seed_owners = BTreeMap::<CodeWritableAddress, _>::new();

        for point in &expansion.writable_points {
            match &point.edit {
                CodePointEdit::Point { address } => {
                    ordinary.insert(address.clone());
                }
                CodePointEdit::RectangleCorner {
                    lower_left,
                    upper_right,
                    corner,
                    effective_lower_left,
                    effective_upper_right,
                } => {
                    if lower_left == upper_right {
                        return Err(
                            "rectangle semantic seed codec aliases both seed addresses".into()
                        );
                    }
                    let key = (lower_left.clone(), upper_right.clone());
                    for address in [lower_left, upper_right] {
                        if seed_owners
                            .insert(address.clone(), key.clone())
                            .is_some_and(|owner| owner != key)
                        {
                            return Err("rectangle semantic seed groups partially overlap".into());
                        }
                    }
                    let seed_bits = (
                        pair_bits(*effective_lower_left),
                        pair_bits(*effective_upper_right),
                    );
                    if effective
                        .insert(key.clone(), seed_bits)
                        .is_some_and(|expected| expected != seed_bits)
                    {
                        return Err(
                            "rectangle semantic corner codecs disagree on effective seeds".into(),
                        );
                    }
                    if groups
                        .entry(key)
                        .or_default()
                        .insert(*corner, point)
                        .is_some()
                    {
                        return Err(
                            "rectangle semantic seed group has a duplicate corner codec".into()
                        );
                    }
                }
            }
        }

        if ordinary
            .iter()
            .any(|address| seed_owners.contains_key(address))
        {
            return Err(
                "rectangle semantic seed address collides with an ordinary point codec".into(),
            );
        }
        if groups.values().any(|lenses| lenses.len() != 4) {
            return Err("rectangle semantic seed group has no complete corner codec".into());
        }
        Ok(Self { groups })
    }

    fn get(
        &self,
        key: &(CodeWritableAddress, CodeWritableAddress),
    ) -> Result<&BTreeMap<CodeRectangleCorner, &'a ExpandedWritablePoint>, String> {
        self.groups
            .get(key)
            .ok_or_else(|| "rectangle semantic seed group has no complete corner codec".into())
    }
}

fn canonical_rectangle_seeds(
    first_corner: CodeRectangleCorner,
    first_target: [f64; 2],
    second_corner: CodeRectangleCorner,
    second_target: [f64; 2],
) -> Result<([f64; 2], [f64; 2]), String> {
    let mut lower = [None, None];
    let mut upper = [None, None];
    for (corner, target) in [(first_corner, first_target), (second_corner, second_target)] {
        match corner {
            CodeRectangleCorner::LowerLeft => lower = target.map(Some),
            CodeRectangleCorner::LowerRight => {
                upper[0] = Some(target[0]);
                lower[1] = Some(target[1]);
            }
            CodeRectangleCorner::UpperRight => upper = target.map(Some),
            CodeRectangleCorner::UpperLeft => {
                lower[0] = Some(target[0]);
                upper[1] = Some(target[1]);
            }
        }
    }
    let complete = |seed: [Option<f64>; 2]| {
        Some([seed[0]?, seed[1]?]).filter(|seed| seed.iter().all(|value| value.is_finite()))
    };
    let lower = complete(lower)
        .ok_or_else(|| "canonical rectangle lenses do not cover the lower seed".to_owned())?;
    let upper = complete(upper)
        .ok_or_else(|| "canonical rectangle lenses do not cover the upper seed".to_owned())?;
    Ok((lower, upper))
}

const fn rectangle_corner_position(
    lower: [f64; 2],
    upper: [f64; 2],
    corner: CodeRectangleCorner,
) -> [f64; 2] {
    match corner {
        CodeRectangleCorner::LowerLeft => lower,
        CodeRectangleCorner::LowerRight => [upper[0], lower[1]],
        CodeRectangleCorner::UpperRight => upper,
        CodeRectangleCorner::UpperLeft => [lower[0], upper[1]],
    }
}

fn point_seed_roundoff_compatible(left: [f64; 2], right: [f64; 2], model_scale: f64) -> bool {
    left.into_iter()
        .zip(right)
        .all(|(left, right)| scalar_seed_roundoff_compatible(left, right, model_scale))
}

fn semantic_roundoff_tolerance(left: f64, right: f64, model_scale: f64) -> Option<f64> {
    if !left.is_finite() || !right.is_finite() || !model_scale.is_finite() || model_scale <= 0.0 {
        return None;
    }
    Some(
        left.abs().max(right.abs()).max(model_scale) * f64::EPSILON * TERMINAL_SEED_ROUNDOFF_FACTOR,
    )
}

fn semantic_local_coordinate_scale(
    model_scales: impl IntoIterator<Item = f64>,
    positions: impl IntoIterator<Item = [f64; 2]>,
) -> Option<f64> {
    let mut scale = 1.0_f64;
    for model_scale in model_scales {
        if !model_scale.is_finite() || model_scale <= 0.0 {
            return None;
        }
        scale = scale.max(model_scale);
    }
    for coordinate in positions.into_iter().flatten() {
        if !coordinate.is_finite() {
            return None;
        }
        scale = scale.max(coordinate.abs());
    }
    Some(scale)
}

fn semantic_point_coordinate_scale(
    documents: [&geosolve_sketch::SketchDocument; 2],
    points: &BTreeSet<geosolve_sketch::DesignPointId>,
) -> Option<f64> {
    if points.is_empty() {
        return None;
    }
    let model_scales = documents.map(geosolve_sketch::SketchDocument::model_scale);
    let positions = documents.into_iter().flat_map(|document| {
        points
            .iter()
            .map(|point| document.point(*point).map(|point| point.position))
    });
    let positions = positions.collect::<Option<Vec<_>>>()?;
    semantic_local_coordinate_scale(model_scales, positions)
}

fn accepted_validation_is_publishable(
    validation: &geosolve_constraint_editor::IntentValidationEvidence,
) -> bool {
    validation.hard_residuals_validated
        && validation.all_active_features_current
        && validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
}

fn validate_terminal_preview_session(
    session: &RetainedSketchDocumentSession,
) -> Result<(), String> {
    let accepted = session
        .accepted_state_for_current_input()
        .ok_or_else(|| "terminal point preview has no current accepted native state".to_owned())?;
    let solve = accepted
        .diagnostics()
        .solve
        .ok_or_else(|| "terminal point preview has no independent solve evidence".to_owned())?;
    if !solve.accepted
        || solve.hard_validity != SketchHardValidity::Valid
        || !solve.hard_residuals_validated
        || solve
            .maximum_normalized_hard_residual
            .is_some_and(|residual| !residual.is_finite() || residual > 1.0e-9)
    {
        return Err(
            "terminal point preview failed independent native hard-residual validation".into(),
        );
    }
    Ok(())
}

fn scalar_seed_roundoff_compatible(left: f64, right: f64, model_scale: f64) -> bool {
    let Some(tolerance) = semantic_roundoff_tolerance(left, right, model_scale) else {
        return false;
    };
    let left = left.to_bits();
    let right = right.to_bits();
    if left == right {
        return true;
    }
    if left & !F64_SIGN_MASK == 0 && right & !F64_SIGN_MASK == 0 {
        return false;
    }
    (left & F64_SIGN_MASK == right & F64_SIGN_MASK
        && (f64::from_bits(left) - f64::from_bits(right)).abs() <= tolerance)
        || (f64::from_bits(left).abs() <= TERMINAL_SEED_ZERO_ROUNDOFF
            && f64::from_bits(right).abs() <= TERMINAL_SEED_ZERO_ROUNDOFF)
}

#[allow(
    clippy::too_many_lines,
    reason = "preserved explicit feature hierarchy parity audit"
)]
fn feature_documents_match_for_terminal_parity(
    terminal: &ComputedFeatureDocument,
    staged: &ComputedFeatureDocument,
    policy: &TerminalComputedParityPolicy,
) -> bool {
    let terminal_identity = terminal.identity();
    let staged_identity = staged.identity();
    terminal_identity.document == staged_identity.document
        && terminal_identity.sketch_document == staged_identity.sketch_document
        && terminal.allocator_high_water() == staged.allocator_high_water()
        && terminal.features().len() == staged.features().len()
        && terminal
            .features()
            .iter()
            .zip(staged.features())
            .all(|(terminal, staged)| {
                if terminal.id != staged.id
                    || terminal.label != staged.label
                    || terminal.suppressed != staged.suppressed
                {
                    return false;
                }
                let admits_reanchoring = !terminal.suppressed;
                let (
                    ComputedFeatureDefinition::FilletSet(terminal),
                    ComputedFeatureDefinition::FilletSet(staged),
                ) = (&terminal.definition, &staged.definition);
                terminal.radius.to_bits() == staged.radius.to_bits()
                    && terminal.corners.len() == staged.corners.len()
                    && terminal
                        .corners
                        .iter()
                        .zip(&staged.corners)
                        .all(|(terminal, staged)| {
                            terminal.id == staged.id
                                && terminal.endpoint_order == staged.endpoint_order
                                && terminal.sweep == staged.sweep
                                && [
                                    (terminal.first, staged.first),
                                    (terminal.second, staged.second),
                                ]
                                .into_iter()
                                .all(|(terminal, staged)| {
                                    let picked_parameter_matches = match policy {
                                        TerminalComputedParityPolicy::RectangleAliasRoundoff {
                                            source_scales,
                                        } if admits_reanchoring
                                            && source_scales.contains_key(&terminal.source)
                                            && source_scales.contains_key(&staged.source) =>
                                        {
                                            terminal_derived_scalar_matches(
                                                terminal.picked_parameter,
                                                staged.picked_parameter,
                                                1.0,
                                            )
                                        }
                                        TerminalComputedParityPolicy::Exact
                                        | TerminalComputedParityPolicy::RectangleAliasRoundoff {
                                            ..
                                        } => {
                                            terminal.picked_parameter.to_bits()
                                                == staged.picked_parameter.to_bits()
                                        }
                                    };
                                    // `picked_parameter` is a recomputable
                                    // scalar only in the authenticated causal
                                    // curve closure. Every durable owner,
                                    // winding, neighborhood, normal, endpoint
                                    // and periodic anchor stays exact.
                                    terminal.source == staged.source
                                        && terminal.winding == staged.winding
                                        && terminal.neighborhood == staged.neighborhood
                                        && terminal.normal_side == staged.normal_side
                                        && terminal.retained_endpoint == staged.retained_endpoint
                                        && terminal.periodic_anchor == staged.periodic_anchor
                                        && picked_parameter_matches
                                })
                        })
            })
}

fn terminal_derived_scalar_matches(first: f64, second: f64, coordinate_scale: f64) -> bool {
    // The staged overlay and accepted pointer terminal start from the same
    // authenticated rectangle seeds, but redundant rectangle aliases can
    // differ within the explicitly admitted terminal seed cell. Re-evaluated
    // Fillet coordinates may therefore inherit only that bounded ULP/near-zero
    // noise. Keep every discrete owner, branch, winding and topology field
    // exact; this predicate applies only to recomputable finite scalars.
    if !first.is_finite() || !second.is_finite() {
        return false;
    }
    let first_bits = first.to_bits();
    let second_bits = second.to_bits();
    if first_bits == second_bits
        || (first_bits & !F64_SIGN_MASK == 0 && second_bits & !F64_SIGN_MASK == 0)
    {
        return true;
    }
    let Some(tolerance) = semantic_roundoff_tolerance(first, second, coordinate_scale) else {
        return false;
    };
    let zero_tolerance = TERMINAL_SEED_ZERO_ROUNDOFF * coordinate_scale.max(1.0);
    (first_bits & F64_SIGN_MASK == second_bits & F64_SIGN_MASK
        && (first - second).abs() <= tolerance)
        || (first.abs() <= zero_tolerance && second.abs() <= zero_tolerance)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct TerminalPeriodicAngleUnwrap {
    turns: i8,
    staged: f64,
    residual: f64,
}

fn terminal_periodic_angle_unwrap(
    terminal: f64,
    staged: f64,
) -> Option<TerminalPeriodicAngleUnwrap> {
    if !terminal.is_finite() || !staged.is_finite() {
        return None;
    }
    let delta = terminal - staged;
    if !delta.is_finite() {
        return None;
    }
    let rounded_turns = (delta / std::f64::consts::TAU).round();
    if !rounded_turns.is_finite() || rounded_turns.abs().to_bits() != 1.0_f64.to_bits() {
        return None;
    }
    let turns = if rounded_turns.is_sign_negative() {
        -1
    } else {
        1
    };
    let staged = staged + f64::from(turns) * std::f64::consts::TAU;
    let residual = terminal - staged;
    (staged.is_finite() && residual.is_finite()).then_some(TerminalPeriodicAngleUnwrap {
        turns,
        staged,
        residual,
    })
}

fn terminal_derived_periodic_angle_matches(first: f64, second: f64) -> bool {
    if terminal_derived_scalar_matches(first, second, 1.0) {
        return true;
    }
    terminal_periodic_angle_unwrap(first, second)
        .is_some_and(|unwrapped| terminal_derived_scalar_matches(first, unwrapped.staged, 1.0))
}

fn terminal_derived_pair_matches(first: [f64; 2], second: [f64; 2], coordinate_scale: f64) -> bool {
    first
        .into_iter()
        .zip(second)
        .all(|(first, second)| terminal_derived_scalar_matches(first, second, coordinate_scale))
}

fn terminal_computed_edge_roundoff_scale(
    terminal: &geosolve_constraint_editor::ComputedEdge,
    staged: &geosolve_constraint_editor::ComputedEdge,
    source_scales: &TerminalRoundoffSourceScales,
) -> Option<f64> {
    let mut maximum = None::<u64>;
    let mut include_source = |source| {
        if let Some(scale) = source_scales.get(&source).copied()
            && maximum.is_none_or(|current| {
                f64::from_bits(scale)
                    .total_cmp(&f64::from_bits(current))
                    .is_gt()
            })
        {
            maximum = Some(scale);
        }
    };
    match (
        &terminal.geometry,
        &terminal.provenance,
        &staged.geometry,
        &staged.provenance,
    ) {
        (
            ComputedEdgeGeometry::NativeSourceFragment {
                source: terminal_geometry,
                ..
            },
            ComputedEdgeProvenance::SourceFragment {
                source: terminal_provenance,
                start_claim: terminal_start,
                end_claim: terminal_end,
                ..
            },
            ComputedEdgeGeometry::NativeSourceFragment {
                source: staged_geometry,
                ..
            },
            ComputedEdgeProvenance::SourceFragment {
                source: staged_provenance,
                start_claim: staged_start,
                end_claim: staged_end,
                ..
            },
        ) if terminal_geometry == terminal_provenance
            && staged_geometry == staged_provenance
            && terminal_geometry == staged_geometry
            && terminal_start == staged_start
            && terminal_end == staged_end =>
        {
            include_source(*terminal_geometry);
        }
        (
            ComputedEdgeGeometry::CircularArc(terminal_geometry),
            ComputedEdgeProvenance::FilletArc {
                owner: terminal_owner,
                sources: terminal_sources,
            },
            ComputedEdgeGeometry::CircularArc(staged_geometry),
            ComputedEdgeProvenance::FilletArc {
                owner: staged_owner,
                sources: staged_sources,
            },
        ) if terminal_owner == staged_owner
            && terminal_sources == staged_sources
            && terminal_geometry
                .contacts
                .iter()
                .map(|contact| contact.source)
                .eq(terminal_sources.iter().copied())
            && staged_geometry
                .contacts
                .iter()
                .map(|contact| contact.source)
                .eq(staged_sources.iter().copied()) =>
        {
            for source in terminal_sources {
                include_source(*source);
            }
        }
        _ => return None,
    }
    maximum
        .map(f64::from_bits)
        .filter(|scale| scale.is_finite() && *scale > 0.0)
}

fn terminal_computed_edge_matches(
    terminal: &geosolve_constraint_editor::ComputedEdge,
    staged: &geosolve_constraint_editor::ComputedEdge,
    source_scales: &TerminalRoundoffSourceScales,
) -> bool {
    if terminal.id.ordinal != staged.id.ordinal || terminal.role != staged.role {
        return false;
    }
    let Some(coordinate_scale) =
        terminal_computed_edge_roundoff_scale(terminal, staged, source_scales)
    else {
        return terminal.geometry == staged.geometry && terminal.provenance == staged.provenance;
    };
    if !terminal_computed_provenance_matches(&terminal.provenance, &staged.provenance) {
        return false;
    }
    match (&terminal.geometry, &staged.geometry) {
        (
            ComputedEdgeGeometry::NativeSourceFragment {
                source: terminal_source,
                interval: terminal_interval,
            },
            ComputedEdgeGeometry::NativeSourceFragment {
                source: staged_source,
                interval: staged_interval,
            },
        ) => {
            terminal_source == staged_source
                && terminal_derived_scalar_matches(
                    terminal_interval.start,
                    staged_interval.start,
                    1.0,
                )
                && terminal_derived_scalar_matches(terminal_interval.end, staged_interval.end, 1.0)
        }
        (
            ComputedEdgeGeometry::CircularArc(terminal),
            ComputedEdgeGeometry::CircularArc(staged),
        ) => {
            terminal_derived_pair_matches(terminal.center, staged.center, coordinate_scale)
                && terminal.radius.to_bits() == staged.radius.to_bits()
                && terminal_derived_periodic_angle_matches(terminal.start_angle, staged.start_angle)
                && terminal_derived_periodic_angle_matches(terminal.end_angle, staged.end_angle)
                && terminal.sweep == staged.sweep
                && terminal.tangent_orientations == staged.tangent_orientations
                && terminal.contacts.len() == staged.contacts.len()
                && terminal
                    .contacts
                    .iter()
                    .zip(staged.contacts)
                    .all(|(terminal, staged)| {
                        terminal.source == staged.source
                            && terminal.winding == staged.winding
                            && terminal_derived_scalar_matches(
                                terminal.parameter,
                                staged.parameter,
                                1.0,
                            )
                            && terminal_derived_scalar_matches(
                                terminal.total_parameter,
                                staged.total_parameter,
                                1.0,
                            )
                            && terminal_derived_pair_matches(
                                terminal.position,
                                staged.position,
                                coordinate_scale,
                            )
                    })
        }
        _ => false,
    }
}

fn terminal_computed_provenance_matches(
    terminal: &ComputedEdgeProvenance,
    staged: &ComputedEdgeProvenance,
) -> bool {
    match (terminal, staged) {
        (
            ComputedEdgeProvenance::SourceFragment {
                source: terminal_source,
                interval: terminal_interval,
                start_claim: terminal_start,
                end_claim: terminal_end,
            },
            ComputedEdgeProvenance::SourceFragment {
                source: staged_source,
                interval: staged_interval,
                start_claim: staged_start,
                end_claim: staged_end,
            },
        ) => {
            terminal_source == staged_source
                && terminal_start == staged_start
                && terminal_end == staged_end
                && terminal_derived_scalar_matches(
                    terminal_interval.start,
                    staged_interval.start,
                    1.0,
                )
                && terminal_derived_scalar_matches(terminal_interval.end, staged_interval.end, 1.0)
        }
        (
            ComputedEdgeProvenance::FilletArc {
                owner: terminal_owner,
                sources: terminal_sources,
            },
            ComputedEdgeProvenance::FilletArc {
                owner: staged_owner,
                sources: staged_sources,
            },
        ) => terminal_owner == staged_owner && terminal_sources == staged_sources,
        _ => false,
    }
}

fn terminal_computed_fragment_matches(
    terminal: &geosolve_constraint_editor::ComputedConstructionFragment,
    staged: &geosolve_constraint_editor::ComputedConstructionFragment,
    source_scales: &TerminalRoundoffSourceScales,
) -> bool {
    if !source_scales.contains_key(&terminal.source) || !source_scales.contains_key(&staged.source)
    {
        return terminal_computed_fragment_exact_matches(terminal, staged);
    }
    terminal.id.ordinal == staged.id.ordinal
        && terminal.source == staged.source
        && terminal.source_role == staged.source_role
        && terminal.provenance.owner == staged.provenance.owner
        && terminal.provenance.endpoint == staged.provenance.endpoint
        && terminal_derived_scalar_matches(terminal.interval.start, staged.interval.start, 1.0)
        && terminal_derived_scalar_matches(terminal.interval.end, staged.interval.end, 1.0)
        && terminal_derived_scalar_matches(
            terminal.provenance.base_interval.start,
            staged.provenance.base_interval.start,
            1.0,
        )
        && terminal_derived_scalar_matches(
            terminal.provenance.base_interval.end,
            staged.provenance.base_interval.end,
            1.0,
        )
}

fn terminal_computed_fragment_exact_matches(
    terminal: &geosolve_constraint_editor::ComputedConstructionFragment,
    staged: &geosolve_constraint_editor::ComputedConstructionFragment,
) -> bool {
    terminal.id.ordinal == staged.id.ordinal
        && terminal.source == staged.source
        && terminal.interval == staged.interval
        && terminal.source_role == staged.source_role
        && terminal.provenance == staged.provenance
}

fn terminal_feature_evaluation_matches(
    terminal: &ComputedFeatureEvaluation,
    staged: &ComputedFeatureEvaluation,
) -> bool {
    if terminal.feature != staged.feature {
        return false;
    }
    match (&terminal.state, &staged.state) {
        (
            ComputedFeatureEvaluationState::Current {
                corner_edges: terminal,
            },
            ComputedFeatureEvaluationState::Current {
                corner_edges: staged,
            },
        ) => {
            terminal.len() == staged.len()
                && terminal.iter().zip(staged).all(
                    |((terminal_corner, terminal_edge), (staged_corner, staged_edge))| {
                        terminal_corner == staged_corner
                            && terminal_edge.ordinal == staged_edge.ordinal
                    },
                )
        }
        (
            ComputedFeatureEvaluationState::Failed { failure: terminal },
            ComputedFeatureEvaluationState::Failed { failure: staged },
        ) => terminal == staged,
        (
            ComputedFeatureEvaluationState::Suppressed,
            ComputedFeatureEvaluationState::Suppressed,
        ) => true,
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq)]
enum TerminalComputedParityPolicy {
    Exact,
    RectangleAliasRoundoff {
        source_scales: TerminalRoundoffSourceScales,
    },
}

fn computed_snapshots_match_for_terminal_parity(
    terminal: &ComputedFeatureSnapshot,
    staged: &ComputedFeatureSnapshot,
    policy: &TerminalComputedParityPolicy,
) -> bool {
    terminal.edges().len() == staged.edges().len()
        && terminal
            .edges()
            .iter()
            .zip(staged.edges())
            .all(|(terminal, staged)| match policy {
                TerminalComputedParityPolicy::Exact => {
                    terminal.id.ordinal == staged.id.ordinal
                        && terminal.role == staged.role
                        && terminal.geometry == staged.geometry
                        && terminal.provenance == staged.provenance
                }
                TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
                    terminal_computed_edge_matches(terminal, staged, source_scales)
                }
            })
        && terminal.construction_fragments().len() == staged.construction_fragments().len()
        && terminal
            .construction_fragments()
            .iter()
            .zip(staged.construction_fragments())
            .all(|(terminal, staged)| match policy {
                TerminalComputedParityPolicy::Exact => {
                    terminal_computed_fragment_exact_matches(terminal, staged)
                }
                TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
                    terminal_computed_fragment_matches(terminal, staged, source_scales)
                }
            })
        && terminal.replaced_sources() == staged.replaced_sources()
        && terminal.feature_evaluations().len() == staged.feature_evaluations().len()
        && terminal
            .feature_evaluations()
            .iter()
            .zip(staged.feature_evaluations())
            .all(|(terminal, staged)| terminal_feature_evaluation_matches(terminal, staged))
}

type TerminalRectangleAliasScales = BTreeMap<geosolve_sketch::DesignPointId, u64>;
type TerminalRoundoffSourceScales = BTreeMap<NativeCurveSpanSource, u64>;

#[derive(Clone, Debug, Eq, PartialEq)]
enum TerminalDocumentParity {
    Exact,
    NormalizedRedundantRectangleAliases(TerminalRectangleAliasScales),
    Mismatch,
}

impl TerminalDocumentParity {
    const fn matches(&self) -> bool {
        !matches!(self, Self::Mismatch)
    }

    fn normalized_redundant_rectangle_aliases(&self) -> Option<&TerminalRectangleAliasScales> {
        match self {
            Self::NormalizedRedundantRectangleAliases(points) => Some(points),
            Self::Exact | Self::Mismatch => None,
        }
    }
}

fn terminal_computed_roundoff_points<'a>(
    design: &'a TerminalDocumentParity,
    accepted: &'a TerminalDocumentParity,
) -> Option<&'a TerminalRectangleAliasScales> {
    match (
        design.normalized_redundant_rectangle_aliases(),
        accepted.normalized_redundant_rectangle_aliases(),
    ) {
        (Some(design), Some(accepted))
            if design.keys().eq(accepted.keys()) && !accepted.is_empty() =>
        {
            Some(accepted)
        }
        _ => None,
    }
}

fn terminal_document_normalizations_match(
    design: &TerminalDocumentParity,
    accepted: &TerminalDocumentParity,
) -> bool {
    match (design, accepted) {
        (TerminalDocumentParity::Exact, TerminalDocumentParity::Exact) => true,
        (
            TerminalDocumentParity::NormalizedRedundantRectangleAliases(design),
            TerminalDocumentParity::Exact,
        ) => !design.is_empty(),
        (
            TerminalDocumentParity::NormalizedRedundantRectangleAliases(design),
            TerminalDocumentParity::NormalizedRedundantRectangleAliases(accepted),
        ) => design.keys().eq(accepted.keys()) && !design.is_empty(),
        _ => false,
    }
}

fn maximum_point_roundoff_scale(
    point_scales: &TerminalRectangleAliasScales,
    points: impl IntoIterator<Item = geosolve_sketch::DesignPointId>,
) -> Option<u64> {
    let mut maximum = None::<u64>;
    for point in points {
        if let Some(scale) = point_scales.get(&point).copied()
            && maximum.is_none_or(|current| {
                f64::from_bits(scale)
                    .total_cmp(&f64::from_bits(current))
                    .is_gt()
            })
        {
            maximum = Some(scale);
        }
    }
    maximum
}

fn curve_definition_roundoff_scale(
    definition: &geosolve_sketch::CurveDefinition,
    point_scales: &TerminalRectangleAliasScales,
) -> Option<u64> {
    use geosolve_sketch::CurveDefinition;

    match definition {
        CurveDefinition::Line { start, end, .. }
        | CurveDefinition::RationalQuadraticConic { start, end, .. } => {
            maximum_point_roundoff_scale(point_scales, [*start, *end])
        }
        CurveDefinition::Polyline {
            points: controls, ..
        } => maximum_point_roundoff_scale(point_scales, controls.iter().copied()),
        CurveDefinition::BSpline { .. } | CurveDefinition::Nurbs { .. } => None,
        CurveDefinition::Circle { center, .. } | CurveDefinition::CircularArc { center, .. } => {
            maximum_point_roundoff_scale(point_scales, [*center])
        }
        CurveDefinition::QuadraticBezier { controls } => {
            maximum_point_roundoff_scale(point_scales, controls.iter().copied())
        }
        CurveDefinition::CubicBezier { controls } => {
            maximum_point_roundoff_scale(point_scales, controls.iter().copied())
        }
        CurveDefinition::Ellipse {
            center,
            major_axis_point,
            ..
        }
        | CurveDefinition::EllipticalArc {
            center,
            major_axis_point,
            ..
        } => maximum_point_roundoff_scale(point_scales, [*center, *major_axis_point]),
        CurveDefinition::ParabolaSegment { vertex, focus, .. } => {
            maximum_point_roundoff_scale(point_scales, [*vertex, *focus])
        }
        CurveDefinition::HyperbolaSegment {
            center,
            transverse_axis_point,
            ..
        } => maximum_point_roundoff_scale(point_scales, [*center, *transverse_axis_point]),
    }
}

fn insert_terminal_roundoff_source(
    sources: &mut TerminalRoundoffSourceScales,
    source: NativeCurveSpanSource,
    scale: u64,
) -> Result<(), String> {
    if sources.insert(source, scale).is_some() {
        return Err("terminal roundoff policy repeats one native source span".into());
    }
    Ok(())
}

fn insert_terminal_spline_roundoff_sources(
    sources: &mut TerminalRoundoffSourceScales,
    curve: geosolve_sketch::CurveId,
    definition: &geosolve_sketch::CurveDefinition,
    point_scales: &TerminalRectangleAliasScales,
) -> Result<(), String> {
    let (geosolve_sketch::CurveDefinition::BSpline {
        form,
        degree,
        controls,
        knots,
        span_ids,
        ..
    }
    | geosolve_sketch::CurveDefinition::Nurbs {
        form,
        degree,
        controls,
        knots,
        span_ids,
        ..
    }) = definition
    else {
        return Err("terminal roundoff spline helper received a non-spline curve".into());
    };
    if !controls
        .iter()
        .any(|control| point_scales.contains_key(control))
    {
        return Ok(());
    }
    let basis = match form {
        geosolve_sketch::DocumentBSplineForm::Clamped => {
            geosolve_sketch::BSplineBasis::try_clamped(*degree, controls.len(), knots.clone())
        }
        geosolve_sketch::DocumentBSplineForm::Periodic => {
            geosolve_sketch::BSplineBasis::try_periodic(*degree, controls.len(), knots.clone())
        }
    }
    .map_err(|error| {
        format!("terminal roundoff policy encountered an invalid spline basis: {error}")
    })?;
    if span_ids.len() != basis.spans().len() {
        return Err(
            "terminal roundoff policy encountered inconsistent spline span identity".into(),
        );
    }
    for (segment, span) in span_ids.iter().zip(basis.spans()) {
        let support = span
            .support()
            .iter()
            .map(|index| controls.get(*index).copied())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                "terminal roundoff policy encountered invalid spline support".to_owned()
            })?;
        let Some(scale) = maximum_point_roundoff_scale(point_scales, support) else {
            continue;
        };
        insert_terminal_roundoff_source(
            sources,
            NativeCurveSpanSource {
                span: CurveSpan {
                    curve,
                    segment: *segment,
                },
            },
            scale,
        )?;
    }
    Ok(())
}

fn terminal_roundoff_sources(
    document: &geosolve_sketch::SketchDocument,
    point_scales: &TerminalRectangleAliasScales,
) -> Result<TerminalRoundoffSourceScales, String> {
    use geosolve_sketch::CurveDefinition;

    let mut sources = TerminalRoundoffSourceScales::new();
    for curve in document.curves() {
        match &curve.definition {
            CurveDefinition::Polyline { points, closed, .. } => {
                let count = points.len().saturating_sub(1) + usize::from(*closed);
                for index in 0..count {
                    let end = if index + 1 == points.len() {
                        0
                    } else {
                        index + 1
                    };
                    let start_point = points.get(index).copied().ok_or_else(|| {
                        "terminal roundoff policy encountered an invalid polyline span".to_owned()
                    })?;
                    let end_point = points.get(end).copied().ok_or_else(|| {
                        "terminal roundoff policy encountered an invalid polyline span".to_owned()
                    })?;
                    let Some(scale) =
                        maximum_point_roundoff_scale(point_scales, [start_point, end_point])
                    else {
                        continue;
                    };
                    let segment = u32::try_from(index).map_err(|_| {
                        "terminal roundoff policy has an unrepresentable polyline span".to_owned()
                    })?;
                    insert_terminal_roundoff_source(
                        &mut sources,
                        NativeCurveSpanSource {
                            span: CurveSpan {
                                curve: curve.id,
                                segment,
                            },
                        },
                        scale,
                    )?;
                }
            }
            CurveDefinition::BSpline { .. } | CurveDefinition::Nurbs { .. } => {
                insert_terminal_spline_roundoff_sources(
                    &mut sources,
                    curve.id,
                    &curve.definition,
                    point_scales,
                )?;
            }
            _ => {
                let Some(scale) = curve_definition_roundoff_scale(&curve.definition, point_scales)
                else {
                    continue;
                };
                insert_terminal_roundoff_source(
                    &mut sources,
                    NativeCurveSpanSource {
                        span: CurveSpan {
                            curve: curve.id,
                            segment: 0,
                        },
                    },
                    scale,
                )?;
            }
        }
    }
    Ok(sources)
}

fn point_positions_match_bits(
    left: &geosolve_sketch::SketchDocument,
    right: &geosolve_sketch::SketchDocument,
) -> bool {
    left.points().len() == right.points().len()
        && left
            .points()
            .iter()
            .zip(right.points())
            .all(|(left, right)| {
                left.id == right.id && pair_bits(left.position) == pair_bits(right.position)
            })
}

fn scalar_values_match_bits(
    left: &geosolve_sketch::SketchDocument,
    right: &geosolve_sketch::SketchDocument,
) -> bool {
    left.scalars().len() == right.scalars().len()
        && left
            .scalars()
            .iter()
            .zip(right.scalars())
            .all(|(left, right)| {
                left.id == right.id && left.value.to_bits() == right.value.to_bits()
            })
}

#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed parity normalizer keeps per-rectangle anchor, alias, local-scale, and exact-document checks adjacent"
)]
fn documents_match_for_terminal_parity(
    terminal_editor: &ProjectionalEditorSession,
    staged_editor: &ProjectionalEditorSession,
    terminal: &geosolve_sketch::SketchDocument,
    staged: &geosolve_sketch::SketchDocument,
    rectangle_projections: &[RectangleTerminalProjection],
    recomputable_line_branches: &BTreeSet<geosolve_sketch::CurveId>,
    object_relabels: &[DocumentObjectRelabel],
) -> Result<TerminalDocumentParity, String> {
    if terminal.exact_except_recomputable_line_branches_and_object_relabels(
        staged,
        recomputable_line_branches,
        object_relabels,
    ) && point_positions_match_bits(terminal, staged)
        && scalar_values_match_bits(terminal, staged)
    {
        return Ok(TerminalDocumentParity::Exact);
    }
    let resolve = |editor: &ProjectionalEditorSession, handle: &ExpandedPort| {
        expanded_port_point(editor, handle)
            .ok_or_else(|| "rectangle parity lens has no accepted native point binding".to_owned())
    };
    let mut anchors = BTreeSet::new();
    let mut redundant = BTreeSet::new();
    let mut redundant_scales = BTreeMap::new();
    for projection in rectangle_projections {
        let mut local_points = BTreeSet::new();
        for handle in &projection.anchors {
            let terminal_point = resolve(terminal_editor, handle)?;
            if terminal_point != resolve(staged_editor, handle)? {
                return Ok(TerminalDocumentParity::Mismatch);
            }
            if !anchors.insert(terminal_point) {
                return Err("rectangle parity repeats one anchor point".into());
            }
            local_points.insert(terminal_point);
        }
        for handle in &projection.redundant_aliases {
            let terminal_point = resolve(terminal_editor, handle)?;
            if terminal_point != resolve(staged_editor, handle)? {
                return Ok(TerminalDocumentParity::Mismatch);
            }
            if !redundant.insert(terminal_point) {
                return Err("rectangle parity repeats one redundant point".into());
            }
            local_points.insert(terminal_point);
        }
        let local_scale = semantic_point_coordinate_scale([terminal, staged], &local_points)
            .ok_or_else(|| {
                "rectangle parity has no finite local semantic coordinate scale".to_owned()
            })?;
        for handle in &projection.redundant_aliases {
            let point = resolve(terminal_editor, handle)?;
            if redundant_scales.insert(point, local_scale).is_some() {
                return Err("rectangle parity repeats one redundant point scale".into());
            }
        }
    }
    if !anchors.is_disjoint(&redundant) {
        return Err("rectangle parity anchor aliases a redundant point".into());
    }
    let mut normalized = terminal.clone();
    let mut normalized_redundant_rectangle_aliases = TerminalRectangleAliasScales::new();
    for point in anchors {
        let terminal_position = terminal
            .point(point)
            .ok_or_else(|| "terminal rectangle anchor disappeared".to_owned())?
            .position;
        let staged_position = staged
            .point(point)
            .ok_or_else(|| "staged rectangle anchor disappeared".to_owned())?
            .position;
        if pair_bits(terminal_position) != pair_bits(staged_position) {
            return Ok(TerminalDocumentParity::Mismatch);
        }
    }
    for point in redundant {
        let terminal_position = terminal
            .point(point)
            .ok_or_else(|| "terminal redundant rectangle point disappeared".to_owned())?
            .position;
        let staged_position = staged
            .point(point)
            .ok_or_else(|| "staged redundant rectangle point disappeared".to_owned())?
            .position;
        if pair_bits(terminal_position) == pair_bits(staged_position) {
            continue;
        }
        if !terminal_position.into_iter().all(f64::is_finite)
            || !staged_position.into_iter().all(f64::is_finite)
        {
            return Ok(TerminalDocumentParity::Mismatch);
        }
        let coordinate_scale = redundant_scales
            .get(&point)
            .copied()
            .ok_or_else(|| "rectangle parity redundant point has no local scale".to_owned())?;
        if !point_seed_roundoff_compatible(terminal_position, staged_position, coordinate_scale) {
            return Ok(TerminalDocumentParity::Mismatch);
        }
        normalized
            .set_point_position(point, staged_position)
            .map_err(|error| format!("rectangle parity normalization failed: {error}"))?;
        if normalized_redundant_rectangle_aliases
            .insert(point, coordinate_scale.to_bits())
            .is_some()
        {
            return Err("rectangle parity repeats one normalized alias scale".into());
        }
    }
    if !normalized.exact_except_recomputable_line_branches_and_object_relabels(
        staged,
        recomputable_line_branches,
        object_relabels,
    ) || !point_positions_match_bits(&normalized, staged)
        || !scalar_values_match_bits(&normalized, staged)
    {
        return Ok(TerminalDocumentParity::Mismatch);
    }
    Ok(if normalized_redundant_rectangle_aliases.is_empty() {
        TerminalDocumentParity::Exact
    } else {
        TerminalDocumentParity::NormalizedRedundantRectangleAliases(
            normalized_redundant_rectangle_aliases,
        )
    })
}

/// Reconstructs the exact source-seeded design witness beneath a delegated
/// native drag. Retained preview sessions keep their origin design inputs and
/// place the pointer target in the accepted solve; the semantic owner instead
/// persists only its authenticated source seeds. Rectangle aliases are added
/// as derived parity witnesses, never as extra drafts.
fn terminal_seeded_design_document(
    preview: TerminalPointPreview<'_>,
    placements: &[(ExpandedWritablePoint, [f64; 2])],
    rectangle_projections: &[RectangleTerminalProjection],
) -> Result<geosolve_sketch::SketchDocument, String> {
    let mut targets = BTreeMap::<geosolve_sketch::DesignPointId, [f64; 2]>::new();
    let mut insert = |handle: &ExpandedPort, target: [f64; 2]| -> Result<(), String> {
        let point = preview
            .point(handle)
            .ok_or_else(|| "terminal design seed has no native point binding".to_owned())?;
        if let Some(previous) = targets.insert(point, target)
            && pair_bits(previous) != pair_bits(target)
        {
            return Err("terminal design seed aliases conflicting native positions".into());
        }
        Ok(())
    };
    for (point, target) in placements {
        insert(&point.handle, *target)?;
    }
    for projection in rectangle_projections {
        for handle in projection
            .anchors
            .iter()
            .chain(&projection.redundant_aliases)
        {
            let target = preview.position(handle).ok_or_else(|| {
                "terminal rectangle design witness has no accepted position".to_owned()
            })?;
            insert(handle, target)?;
        }
    }
    let mut design = preview.session.design_document().clone();
    for (point, target) in targets {
        design
            .set_point_position(point, target)
            .map_err(|error| format!("terminal design seed normalization failed: {error}"))?;
    }
    Ok(design)
}

fn terminal_semantic_inputs_match(
    origin: &RetainedSketchDocumentSession,
    staged: &RetainedSketchDocumentSession,
) -> bool {
    let Some(origin_input) = origin
        .accepted_prepared_input()
        .map(geosolve_sketch::PreparedSketchInput::attempt_input)
    else {
        return false;
    };
    let Some(staged_input) = staged
        .accepted_prepared_input()
        .map(geosolve_sketch::PreparedSketchInput::attempt_input)
    else {
        return false;
    };
    origin_input.publication_request() == staged_input.publication_request()
        && origin_input.solver_config() == staged_input.solver_config()
        && origin_input.effective_activation_revision()
            == staged_input.effective_activation_revision()
        && origin_input.activation_digest() == staged_input.activation_digest()
        && origin_input.parameter_revision() == staged_input.parameter_revision()
        && origin_input.parameter_digest() == staged_input.parameter_digest()
        && origin_input.external_snapshot_set_revision()
            == staged_input.external_snapshot_set_revision()
        && origin_input.external_snapshot_set_digest()
            == staged_input.external_snapshot_set_digest()
        && origin.request() == staged.request()
        && origin.parameter_batch() == staged.parameter_batch()
        && origin.external_snapshot_set() == staged.external_snapshot_set()
        && origin.accepted_parameter_batch() == staged.accepted_parameter_batch()
        && origin.accepted_external_snapshot_set() == staged.accepted_external_snapshot_set()
}

#[allow(
    clippy::too_many_lines,
    reason = "one atomic exact semantic codec classifier"
)]
fn canonical_terminal_point_bundle(
    expansion: &ExpandedCodeProject,
    overlay: &CodeInteractionOverlay,
    authenticated: &ExpandedWritablePoint,
    placements: Vec<(ExpandedWritablePoint, [f64; 2])>,
    preview: TerminalPointPreview<'_>,
) -> Result<CanonicalTerminalPointBundle, String> {
    if !placements.iter().any(|(point, _)| point == authenticated) {
        return Err(
            "terminal semantic placement bundle does not contain its authenticated point lens"
                .into(),
        );
    }
    let mut rectangle_groups = BTreeMap::<
        (CodeWritableAddress, CodeWritableAddress),
        Vec<(ExpandedWritablePoint, [f64; 2])>,
    >::new();
    let mut point_groups =
        BTreeMap::<CodeWritableAddress, Vec<(ExpandedWritablePoint, [f64; 2])>>::new();
    for placement in placements {
        match &placement.0.edit {
            CodePointEdit::Point { address } => {
                point_groups
                    .entry(address.clone())
                    .or_default()
                    .push(placement);
            }
            CodePointEdit::RectangleCorner {
                lower_left,
                upper_right,
                ..
            } => {
                rectangle_groups
                    .entry((lower_left.clone(), upper_right.clone()))
                    .or_default()
                    .push(placement);
            }
        }
    }

    let mut canonical = Vec::new();
    for (_address, group) in point_groups {
        let selected = group
            .iter()
            .find(|(point, _)| point == authenticated)
            .unwrap_or(&group[0]);
        if group
            .iter()
            .any(|(_, target)| pair_bits(*target) != pair_bits(selected.1))
        {
            return Err("conflicting semantic point aliases require distinct exact drafts".into());
        }
        canonical.push(selected.clone());
    }

    let mut rectangle_projections = Vec::new();
    let candidate_document = preview
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "terminal rectangle has no current accepted document".to_owned())?
        .document();
    let rectangle_lenses = RectangleLensIndex::new(expansion)?;
    for (key, group) in rectangle_groups {
        let lenses = rectangle_lenses.get(&key)?;
        let authenticated_corner = group.iter().find_map(|(point, _)| {
            (point == authenticated)
                .then(|| rectangle_corner(&point.edit))
                .flatten()
        });
        let (first_corner, second_corner) = authenticated_corner.map_or(
            (
                CodeRectangleCorner::LowerLeft,
                CodeRectangleCorner::UpperRight,
            ),
            |corner| (corner, opposite_rectangle_corner(corner)),
        );
        let first = if authenticated_corner == Some(first_corner) {
            authenticated.clone()
        } else {
            (*lenses
                .get(&first_corner)
                .ok_or_else(|| "canonical rectangle first lens disappeared".to_owned())?)
            .clone()
        };
        let second = (*lenses
            .get(&second_corner)
            .ok_or_else(|| "canonical rectangle second lens disappeared".to_owned())?)
        .clone();
        let first_target = preview
            .position(&first.handle)
            .ok_or_else(|| "canonical rectangle lens has no accepted native position".to_owned())?;
        let second_target = preview
            .position(&second.handle)
            .ok_or_else(|| "canonical rectangle lens has no accepted native position".to_owned())?;
        let (lower_left, upper_right) =
            canonical_rectangle_seeds(first_corner, first_target, second_corner, second_target)?;
        let lens_positions = lenses
            .values()
            .map(|point| {
                preview.position(&point.handle).ok_or_else(|| {
                    "rectangle parity lens has no accepted native position".to_owned()
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let coordinate_scale = semantic_local_coordinate_scale(
            [candidate_document.model_scale()],
            [lower_left, upper_right].into_iter().chain(lens_positions),
        )
        .ok_or_else(|| {
            "terminal rectangle has no finite local semantic coordinate scale".to_owned()
        })?;

        // Authenticate every one of the exact four GUI lenses against the
        // candidate's independently accepted native document. The two
        // diagonal anchors above are the only source seeds; adjacent
        // corners are solver-derived projections and may differ by
        // scale-relative machine roundoff, but not materially.
        for (corner, point) in lenses {
            let instance_position = preview.position(&point.handle).ok_or_else(|| {
                "rectangle parity lens has no accepted native position".to_owned()
            })?;
            let native_point = preview.point(&point.handle).ok_or_else(|| {
                "rectangle parity lens has no accepted native point binding".to_owned()
            })?;
            let native_position = candidate_document
                .point(native_point)
                .ok_or_else(|| "rectangle parity native point disappeared".to_owned())?
                .position;
            if pair_bits(instance_position) != pair_bits(native_position) {
                return Err(format!(
                    "rectangle `{}` terminal lens is not authenticated by accepted native authority",
                    key.0.display_path(),
                ));
            }
            let expected = rectangle_corner_position(lower_left, upper_right, *corner);
            if !point_seed_roundoff_compatible(native_position, expected, coordinate_scale) {
                return Err(format!(
                    "conflicting rectangle `{}` aliases disagree beyond semantic roundoff",
                    key.0.display_path(),
                ));
            }
        }
        for (point, target) in &group {
            let corner = rectangle_corner(&point.edit)
                .ok_or_else(|| "rectangle group contains a non-rectangle lens".to_owned())?;
            if lenses.get(&corner).copied() != Some(point) {
                return Err(format!(
                    "rectangle `{}` terminal contains an unauthenticated corner lens",
                    key.0.display_path(),
                ));
            }
            let accepted = preview
                .position(&point.handle)
                .ok_or_else(|| "rectangle terminal lens has no accepted position".to_owned())?;
            if pair_bits(*target) != pair_bits(accepted) {
                return Err(format!(
                    "rectangle `{}` placement is not authenticated by its accepted lens",
                    key.0.display_path(),
                ));
            }
        }
        let redundant_aliases = [
            CodeRectangleCorner::LowerLeft,
            CodeRectangleCorner::LowerRight,
            CodeRectangleCorner::UpperRight,
            CodeRectangleCorner::UpperLeft,
        ]
        .into_iter()
        .filter(|corner| *corner != first_corner && *corner != second_corner)
        .map(|corner| {
            lenses
                .get(&corner)
                .map(|point| point.handle.clone())
                .ok_or_else(|| "redundant rectangle parity lens disappeared".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "rectangle parity requires exactly two redundant aliases".to_owned())?;
        rectangle_projections.push(RectangleTerminalProjection {
            anchors: [first.handle.clone(), second.handle.clone()],
            redundant_aliases,
        });
        canonical.push((first, first_target));
        canonical.push((second, second_target));
    }
    canonical.sort_by_key(|(point, _)| (point != authenticated, point.handle.clone()));
    stage_point_drags(
        overlay,
        canonical.iter().map(|(point, target)| (point, *target)),
    )
    .map_err(|error| error.to_string())?;
    Ok(CanonicalTerminalPointBundle {
        placements: canonical,
        rectangle_projections,
    })
}
fn recomputable_code_line_branches(
    editor: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    declaration_label_projections: &[PreparedDeclarationLabelProjection],
) -> Result<BTreeSet<geosolve_sketch::CurveId>, String> {
    let intent = editor.coordinator().intent();
    let accepted = editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "code project has no accepted native authority".to_owned())?;
    let expansion_owned_segments = expansion
        .patch
        .operations()
        .iter()
        .filter_map(|operation| match operation {
            geosolve_sketch_intent::IntentPatchOperation::CreateNode { draft, .. }
                if matches!(
                    draft.kind,
                    IntentNodeKind::Geometry {
                        recipe: GeometryRecipeKind::Segment
                    }
                ) =>
            {
                Some(draft.symbol.clone())
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut curves = BTreeSet::new();
    for node in intent.graph().nodes().values() {
        let IntentNodeKind::Geometry { recipe } = node.kind else {
            continue;
        };
        // M83 Segment branches remain explicit. Only the exact current code
        // expansion proves that a Segment came from a managed/artifact
        // declaration whose branch is source-derived. An ordinary GUI Segment
        // living beside a code project must still compare bit-for-bit.
        let source_derived_segment = recipe == GeometryRecipeKind::Segment
            && expansion_owned_segments.contains(&node.symbol);
        if !source_derived_segment
            && !matches!(
                recipe,
                GeometryRecipeKind::Polyline
                    | GeometryRecipeKind::TwoPointAlignedRectangle
                    | GeometryRecipeKind::ThreePointCornerRectangle
                    | GeometryRecipeKind::CenterRectangle
                    | GeometryRecipeKind::ThreePointCenterRectangle
            )
        {
            continue;
        }
        if let Some(ownership) = accepted.ownership.node(node.id) {
            curves.extend(ownership.owned.iter().filter_map(|binding| match binding {
                IntentNativeBinding::Curve(curve) => Some(*curve),
                _ => None,
            }));
        }
    }
    for (address, provenance) in &expansion.generated_provenance {
        if address.template != ["polyline", "segment"] {
            continue;
        }
        let ExpandedSemanticTarget::Port { port } = &provenance.target else {
            continue;
        };
        // During canvas-to-source publication, the terminal checkpoint still
        // carries the GUI-authored symbol while the independently staged
        // source expansion carries `port.alias`. The Rust-only projection
        // binds both spellings to the same persistent node and declaration;
        // use it only for that authenticated transition. Ordinary code drags
        // have no projection and continue to require the exact expansion
        // alias.
        let node = intent.graph().node_by_symbol(&port.alias).or_else(|| {
            declaration_label_projections
                .iter()
                .find(|projection| projection.declaration == provenance.declaration)
                .and_then(|projection| {
                    let node = intent.graph().node(projection.node)?;
                    (node.symbol == projection.terminal_symbol
                        || expansion.declaration_for_alias(&node.symbol)
                            == Some(&projection.declaration))
                    .then_some(node)
                })
        });
        let node = node
            .ok_or_else(|| format!("generated segment `{}` disappeared", address.display_path()))?;
        let output = node.port_by_selector(port.selector).ok_or_else(|| {
            format!(
                "generated segment `{}` lost its output",
                address.display_path()
            )
        })?;
        match accepted.ownership.port(output.as_ref(node.id)) {
            Some(IntentNativeBinding::CurveSpan(span)) => {
                curves.insert(span.curve);
            }
            Some(IntentNativeBinding::Curve(curve)) => {
                curves.insert(curve);
            }
            _ => {
                return Err(format!(
                    "generated segment `{}` has no native curve",
                    address.display_path()
                ));
            }
        }
    }
    Ok(curves)
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    [value[0].to_bits(), value[1].to_bits()]
}

/// Stages the complete semantic native terminal, preserving every writable moved companion.
/// Existing referenced followers stay references; only the explicitly selected consumer
/// may detach. The shared codec authenticates generated allocations and rectangle seeds.
pub(super) fn materialize_terminal(
    previous: &MaterializedCodeProject,
    project: &CodeProject,
    generated: &KeyedReconcileState,
    overlay: &CodeInteractionOverlay,
    authenticated: &ExpandedWritablePoint,
    editor: &ProjectionalEditorSession,
    session: &RetainedSketchDocumentSession,
) -> Result<(MaterializedCodeProject, CodeInteractionOverlay), String> {
    validate_terminal_preview_session(session)?;
    let preview = TerminalPointPreview { editor, session };
    let target = preview
        .position(&authenticated.handle)
        .ok_or_else(|| "terminal lost its authenticated point lens".to_owned())?;
    let mut placements = vec![(authenticated.clone(), target)];
    for lens in &previous.expansion.writable_points {
        if lens == authenticated || lens.source.is_reference() {
            continue;
        }
        let point = expanded_port_point(editor, &lens.handle)
            .ok_or_else(|| "terminal companion has no native point owner".to_owned())?;
        let origin = editor
            .coordinator()
            .accepted_materialization()
            .and_then(|accepted| accepted.session.accepted_state_for_current_input())
            .and_then(|accepted| accepted.document().point(point))
            .ok_or_else(|| "terminal companion origin disappeared".to_owned())?
            .position;
        let terminal = preview.position(&lens.handle).ok_or_else(|| {
            "terminal companion has no independently accepted position".to_owned()
        })?;
        if pair_bits(origin) != pair_bits(terminal) {
            placements.push((lens.clone(), terminal));
        }
    }
    let mut bundle = canonical_terminal_point_bundle(
        &previous.expansion,
        overlay,
        authenticated,
        placements,
        preview,
    )?;
    append_named_rectangle_projections(&mut bundle, &previous.expansion, editor)?;
    let overlay = stage_point_drags(
        overlay,
        bundle
            .placements
            .iter()
            .map(|(lens, target)| (lens, *target)),
    )
    .map_err(|error| error.to_string())?;
    let continuation = session
        .accepted_state_for_current_input()
        .ok_or_else(|| "terminal has no accepted continuation".to_owned())?
        .document();
    let materialized =
        materialize_code_project_incremental_with_overlay_and_accepted_continuation_audited(
            previous,
            project,
            generated,
            &overlay,
            continuation,
        )
        .outcome
        .map_err(|error| error.to_string())?;
    let staged = TerminalPointPreview::from_accepted(&materialized.editor)?;
    for (lens, position) in &bundle.placements {
        if staged.position(&lens.handle).map(pair_bits) != Some(pair_bits(*position)) {
            return Err("semantic point terminal failed exact staged/native seed parity".into());
        }
    }
    validate_terminal_parity(
        preview,
        &materialized.editor,
        &materialized.expansion,
        &bundle,
        &[],
    )?;
    Ok((materialized, overlay))
}

#[allow(
    clippy::too_many_lines,
    reason = "one transactional cross-domain terminal parity proof"
)]
fn validate_terminal_parity(
    terminal: TerminalPointPreview<'_>,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    bundle: &CanonicalTerminalPointBundle,
    declaration_label_projections: &[PreparedDeclarationLabelProjection],
) -> Result<(), String> {
    let terminal_authority = terminal
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "missing terminal native authority".to_owned())?;
    let staged_authority = staged
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "missing staged native authority".to_owned())?;
    if !accepted_validation_is_publishable(&terminal_authority.validation)
        || !accepted_validation_is_publishable(&staged_authority.validation)
    {
        return Err(
            "terminal parity requires independently validated current native authorities".into(),
        );
    }
    let terminal_input = terminal
        .session
        .accepted_prepared_input()
        .ok_or_else(|| "missing terminal accepted input".to_owned())?;
    let mut transient_computed = None;
    let terminal_computed = if terminal_authority.computed.input().sketch == terminal_input {
        &terminal_authority.computed
    } else {
        let mut allocator = ComputedEvaluationAllocator::from_high_water(
            terminal_authority.computed_evaluation_high_water,
        );
        let outcome = ComputedFeatureEvaluationSnapshot::capture(
            terminal.session,
            &terminal_authority.features,
            ComputedFeatureEvaluationPolicy::default(),
        )
        .map_err(|error| error.to_string())?
        .prepare(&mut allocator)
        .map_err(|error| error.to_string())?
        .execute(OperationControl::unlimited())
        .map_err(|error| error.to_string())?;
        let OperationOutcome::Completed {
            value: computed, ..
        } = outcome
        else {
            return Err("terminal computed evaluation did not complete".into());
        };
        transient_computed.insert(computed)
    };
    if terminal_computed
        .feature_evaluations()
        .iter()
        .any(|evaluation| {
            !matches!(
                evaluation.state,
                ComputedFeatureEvaluationState::Current { .. }
            )
        })
    {
        return Err("terminal computed preview contains a non-current feature".into());
    }
    let mut recomputable =
        recomputable_code_line_branches(terminal.editor, expansion, declaration_label_projections)?;
    recomputable.extend(recomputable_code_line_branches(
        staged,
        expansion,
        declaration_label_projections,
    )?);
    let terminal_accepted = terminal
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "missing terminal document".to_owned())?
        .document();
    let staged_accepted = staged_authority
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "missing staged document".to_owned())?
        .document();
    let terminal_design = terminal_seeded_design_document(
        terminal,
        &bundle.placements,
        &bundle.rectangle_projections,
    )?;
    let declaration_object_relabels = authenticated_declaration_object_relabels(
        terminal.editor,
        staged,
        expansion,
        declaration_label_projections,
        &terminal_design,
        staged_authority.session.design_document(),
    )?;
    let design = documents_match_for_terminal_parity(
        terminal.editor,
        staged,
        &terminal_design,
        staged_authority.session.design_document(),
        &bundle.rectangle_projections,
        &recomputable,
        &declaration_object_relabels,
    )?;
    let accepted = documents_match_for_terminal_parity(
        terminal.editor,
        staged,
        terminal_accepted,
        staged_accepted,
        &bundle.rectangle_projections,
        &recomputable,
        &declaration_object_relabels,
    )?;
    let same_documents = design.matches()
        && accepted.matches()
        && terminal_document_normalizations_match(&design, &accepted);
    let policy = terminal_computed_roundoff_points(&design, &accepted)
        .map(|scales| {
            terminal_roundoff_sources(staged_accepted, scales).map(|source_scales| {
                TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales }
            })
        })
        .transpose()?
        .unwrap_or(TerminalComputedParityPolicy::Exact);
    let same_features = feature_documents_match_for_terminal_parity(
        &terminal_authority.features,
        &staged_authority.features,
        &policy,
    ) && computed_snapshots_match_for_terminal_parity(
        terminal_computed,
        &staged_authority.computed,
        &policy,
    );
    let same_ownership = terminal_authority.ownership.nodes == staged_authority.ownership.nodes
        && terminal_authority.ownership.ports == staged_authority.ownership.ports
        && terminal_authority.ownership.reservations == staged_authority.ownership.reservations
        && terminal_authority.ownership.writable_leaves
            == staged_authority.ownership.writable_leaves
        && terminal_authority.ownership.aggregates == staged_authority.ownership.aggregates;
    let mut differences = Vec::new();
    if !same_documents {
        differences.push(format!(
            "sketch documents (design={design:?}, accepted={accepted:?})"
        ));
    }
    if !same_features {
        differences.push("computed features".into());
    }
    if !same_ownership {
        differences.push("native ownership".into());
    }
    if !terminal_semantic_inputs_match(&terminal_authority.session, &staged_authority.session) {
        differences.push("native semantic inputs".into());
    }
    if terminal_authority.feature_lifecycle_high_water.allocator
        != staged_authority.feature_lifecycle_high_water.allocator
    {
        differences.push("feature allocator".into());
    }
    if terminal.session.persistent_identity_high_water()
        != staged_authority.session.persistent_identity_high_water()
    {
        differences.push("sketch allocator".into());
    }
    if !differences.is_empty() {
        return Err(format!(
            "terminal differs from independently staged authority in {}",
            differences.join(", ")
        ));
    }
    Ok(())
}

/// Named aligned rectangles use two ordinary authored Point lenses. Recover their
/// two derived native corner witnesses from the exact recipe schema/ownership, so
/// terminal design normalization preserves the same four-corner proof as the compound codec.
fn append_named_rectangle_projections(
    bundle: &mut CanonicalTerminalPointBundle,
    expansion: &ExpandedCodeProject,
    editor: &ProjectionalEditorSession,
) -> Result<(), String> {
    let aliases = bundle
        .placements
        .iter()
        .map(|(lens, _)| lens.handle.alias.clone())
        .collect::<BTreeSet<_>>();
    for alias in aliases {
        let node = editor
            .coordinator()
            .intent()
            .graph()
            .node_by_symbol(&alias)
            .ok_or_else(|| "terminal geometry alias disappeared".to_owned())?;
        if !matches!(
            node.kind,
            IntentNodeKind::Geometry {
                recipe: GeometryRecipeKind::TwoPointAlignedRectangle
            }
        ) {
            continue;
        }
        let anchors = expansion
            .writable_points
            .iter()
            .filter(|lens| lens.handle.alias == alias)
            .map(|lens| lens.handle.clone())
            .collect::<Vec<_>>();
        let [first, second] = anchors.as_slice() else {
            return Err("named rectangle lacks two exact authored point lenses".into());
        };
        let anchor_points = anchors
            .iter()
            .map(|handle| {
                expanded_port_point(editor, handle)
                    .ok_or_else(|| "rectangle anchor lost native owner".to_owned())
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        if anchor_points.len() != 2 {
            return Err("named rectangle aliases its authored anchors".into());
        }
        let mut redundant = BTreeMap::new();
        for port in node
            .ports
            .values()
            .filter(|port| port.kind == geosolve_sketch_intent::IntentPortKind::Point)
        {
            let handle = ExpandedPort {
                alias: alias.clone(),
                selector: port.selector,
                kind: port.kind,
            };
            let native = expanded_port_point(editor, &handle)
                .ok_or_else(|| "rectangle corner lost native owner".to_owned())?;
            if !anchor_points.contains(&native) {
                redundant.insert(native, handle);
            }
        }
        let redundant = redundant.into_values().collect::<Vec<_>>();
        let [third, fourth] = redundant.as_slice() else {
            return Err("named rectangle lacks two exact derived corner witnesses".into());
        };
        bundle
            .rectangle_projections
            .push(RectangleTerminalProjection {
                anchors: [first.clone(), second.clone()],
                redundant_aliases: [third.clone(), fourth.clone()],
            });
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PreparedDeclarationLabelProjection {
    pub node: NodeId,
    pub terminal_symbol: IntentKey,
    pub declaration: SemanticSymbol,
}

pub(super) fn validate_construction_parity(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    projections: &[PreparedDeclarationLabelProjection],
) -> Result<(), String> {
    validate_terminal_parity(
        TerminalPointPreview::from_accepted(terminal)?,
        staged,
        expansion,
        &CanonicalTerminalPointBundle {
            placements: Vec::new(),
            rectangle_projections: Vec::new(),
        },
        projections,
    )
}

fn document_object_for_native_binding(binding: IntentNativeBinding) -> Option<DocumentObjectId> {
    match binding {
        IntentNativeBinding::Point(id) => Some(DocumentObjectId::Point(id)),
        IntentNativeBinding::Scalar(id) => Some(DocumentObjectId::Scalar(id)),
        IntentNativeBinding::Curve(id) => Some(DocumentObjectId::Curve(id)),
        IntentNativeBinding::Contact(id) => Some(DocumentObjectId::Contact(id)),
        IntentNativeBinding::Constraint(id) => Some(DocumentObjectId::Constraint(id)),
        IntentNativeBinding::Dimension(id) => Some(DocumentObjectId::Dimension(id)),
        IntentNativeBinding::Parameter(id) => Some(DocumentObjectId::Parameter(id)),
        IntentNativeBinding::ExternalBinding(id) => Some(DocumentObjectId::ExternalBinding(id)),
        IntentNativeBinding::CurveSpan(_)
        | IntentNativeBinding::Source(_)
        | IntentNativeBinding::ComputedFeature(_)
        | IntentNativeBinding::ComputedFeatureCorner(_)
        | IntentNativeBinding::Logical(_) => None,
    }
}

fn document_object_label(
    document: &geosolve_sketch::SketchDocument,
    object: DocumentObjectId,
) -> Option<&str> {
    match object {
        DocumentObjectId::Point(id) => document.point(id).map(|value| value.label.as_str()),
        DocumentObjectId::Scalar(id) => document.scalar(id).map(|value| value.label.as_str()),
        DocumentObjectId::Curve(id) => document.curve(id).map(|value| value.label.as_str()),
        DocumentObjectId::Contact(id) => document.contact(id).map(|value| value.label.as_str()),
        DocumentObjectId::Constraint(id) => {
            document.constraint(id).map(|value| value.label.as_str())
        }
        DocumentObjectId::Dimension(id) => document.dimension(id).map(|value| value.label.as_str()),
        DocumentObjectId::Parameter(id) => document.parameter(id).map(|value| value.label.as_str()),
        DocumentObjectId::ExternalBinding(id) => document
            .external_binding(id)
            .map(|value| value.label.as_str()),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one authenticated witness checks source identity, native ownership and exact label projection together"
)]
fn authenticated_declaration_object_relabels(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    projections: &[PreparedDeclarationLabelProjection],
    terminal_document: &geosolve_sketch::SketchDocument,
    staged_document: &geosolve_sketch::SketchDocument,
) -> Result<Vec<DocumentObjectRelabel>, String> {
    if projections.is_empty() {
        return Ok(Vec::new());
    }
    let terminal_authority = terminal
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "declaration relabel witness has no terminal native authority".to_owned())?;
    let staged_authority = staged
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "declaration relabel witness has no staged native authority".to_owned())?;
    let terminal_graph = terminal.coordinator().intent().graph();
    let staged_graph = staged.coordinator().intent().graph();
    let mut witnessed_terminal_nodes = BTreeSet::new();
    let mut witnessed_staged_nodes = BTreeSet::new();
    let mut witnessed_objects = BTreeSet::new();
    let mut relabels = Vec::new();
    for projection in projections {
        if !witnessed_terminal_nodes.insert(projection.node) {
            return Err("declaration relabel witness repeats a terminal persistent node".into());
        }
        let terminal_node = terminal_graph.node(projection.node).ok_or_else(|| {
            format!(
                "declaration relabel witness lost terminal node {}",
                projection.node
            )
        })?;
        if terminal_node.symbol != projection.terminal_symbol {
            return Err("declaration relabel witness terminal symbol is stale".into());
        }
        let mut staged_matches = staged_graph.nodes().values().filter(|node| {
            expansion.declaration_for_alias(&node.symbol) == Some(&projection.declaration)
        });
        let staged_node = staged_matches.next().ok_or_else(|| {
            format!(
                "declaration relabel witness lost staged declaration `{}`",
                projection.declaration.0
            )
        })?;
        if staged_matches.next().is_some() {
            return Err(format!(
                "declaration relabel witness staged declaration `{}` is not unique",
                projection.declaration.0
            ));
        }
        if !witnessed_staged_nodes.insert(staged_node.id) {
            return Err("declaration relabel witness repeats a staged persistent node".into());
        }
        // Logical aggregate helpers own typed references but no native object labels.
        // Admit only an exact equation-free helper; every native owner below remains exact.
        if terminal_authority.ownership.node(projection.node).is_none()
            && staged_authority.ownership.node(staged_node.id).is_none()
            && matches!(terminal_node.kind, IntentNodeKind::Aggregate { .. })
            && terminal_node.kind == staged_node.kind
            && terminal_node.fields == staged_node.fields
            && terminal_node.inputs == staged_node.inputs
            && terminal_node.reservations.is_empty()
            && staged_node.reservations.is_empty()
            && terminal_node.operation_outputs.is_empty()
            && staged_node.operation_outputs.is_empty()
            && terminal_node.children.is_empty()
            && staged_node.children.is_empty()
        {
            continue;
        }
        let terminal_owner = terminal_authority
            .ownership
            .node(projection.node)
            .ok_or_else(|| "declaration relabel witness has no terminal native owner".to_owned())?;
        let staged_owner = staged_authority
            .ownership
            .node(staged_node.id)
            .ok_or_else(|| "declaration relabel witness has no staged native owner".to_owned())?;
        if terminal_owner.owned != staged_owner.owned {
            return Err(format!(
                "declaration relabel witness changed native ownership for `{}`: terminal={:?} staged={:?}",
                projection.declaration.0, terminal_owner.owned, staged_owner.owned
            ));
        }
        let terminal_prefix = projection.terminal_symbol.as_str();
        let staged_prefix = staged_node.symbol.as_str();
        for object in terminal_owner
            .owned
            .iter()
            .filter_map(|binding| document_object_for_native_binding(*binding))
        {
            if !witnessed_objects.insert(object) {
                return Err("declaration relabel witness repeats one native object".into());
            }
            let current = document_object_label(terminal_document, object).ok_or_else(|| {
                "declaration relabel witness terminal object disappeared".to_owned()
            })?;
            let replacement = document_object_label(staged_document, object).ok_or_else(|| {
                "declaration relabel witness staged object disappeared".to_owned()
            })?;
            let suffix = current.strip_prefix(terminal_prefix).ok_or_else(|| {
                "declaration-owned native label does not carry the exact terminal symbol prefix"
                    .to_owned()
            })?;
            let offset_distance = suffix == " distance"
                && matches!(
                    terminal_node.kind,
                    IntentNodeKind::Operation {
                        operation: geosolve_sketch_intent::OperationKind::ProfileOffset
                    }
                )
                && terminal_node.kind == staged_node.kind
                && matches!(object, DocumentObjectId::Scalar(_));
            if !suffix.is_empty() && !suffix.starts_with('.') && !offset_distance {
                return Err(format!(
                    "declaration-owned native label has a non-canonical symbol suffix: declaration={} object={object:?} current={current:?} terminal={terminal_prefix:?} replacement={replacement:?}",
                    projection.declaration.0
                ));
            }
            let expected = format!("{staged_prefix}{suffix}");
            if replacement != expected {
                return Err(
                    "declaration-owned native label is not the exact staged alias projection"
                        .into(),
                );
            }
            if current != replacement {
                relabels.push(DocumentObjectRelabel::new(object, current, replacement));
            }
        }
    }
    Ok(relabels)
}
