// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared source/native terminal consistency and bounded diagnostic projection.
//! No source compiler, browser state or solver equations live here.

use crate::{
    CodeInteractionOverlay, CodePointEdit, CodeProject, CodeRectangleCorner, CodeWritableAddress,
    ExpandedCodeProject, ExpandedPort, ExpandedSemanticTarget, ExpandedWritablePoint,
    KeyedReconcileState, MaterializedCodeProject, SemanticSymbol,
    materialize_code_project_incremental_with_overlay_and_accepted_continuation_audited,
    stage_point_drags, transport_code_point_terminal_branches,
};
use geosolve_constraint_editor::{
    ComputedEdgeGeometry, ComputedEdgeProvenance, ComputedFeatureDefinition,
    ComputedFeatureDocument, ComputedFeatureEvaluation, ComputedFeatureEvaluationState,
    ComputedFeatureSnapshot, IntentNativeBinding, NativeCurveSpanSource, ProjectionalEditorSession,
};
use geosolve_sketch::{
    CurveSpan, DocumentObjectId, DocumentObjectRelabel, OperationControl, OperationOutcome,
    RetainedSketchDocumentSession, SketchHardValidity,
};
use geosolve_sketch_features::{
    ComputedEvaluationAllocator, ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
};
use geosolve_sketch_intent::{
    GeometryRecipeKind, IntentKey, IntentNodeKind, IntentPortKind, NodeId,
};
use std::collections::{BTreeMap, BTreeSet};

use std::fmt::Write as _;
/// Optional diagnostic sink; it has no effect on numerical acceptance.
pub type TerminalTraceSink<'a> = &'a mut dyn FnMut(&str, &str);

/// Independently validated native preview and its semantic point owner.
#[derive(Clone, Copy, Debug)]
pub struct TerminalPointPreview<'a> {
    editor: &'a ProjectionalEditorSession,
    session: &'a RetainedSketchDocumentSession,
}

impl<'a> TerminalPointPreview<'a> {
    /// Creates a read-only view after independent native preview validation.
    /// # Errors
    /// Rejects missing, stale, nonfinite or independently invalid accepted geometry.
    pub fn new(
        editor: &'a ProjectionalEditorSession,
        session: &'a RetainedSketchDocumentSession,
    ) -> Result<Self, String> {
        validate_terminal_preview_session(session)?;
        Ok(Self { editor, session })
    }
    /// Native semantic owner; this view cannot publish into it.
    #[must_use]
    pub fn editor(self) -> &'a ProjectionalEditorSession {
        self.editor
    }
    /// Independently validated native preview session.
    #[must_use]
    pub fn session(self) -> &'a RetainedSketchDocumentSession {
        self.session
    }

    /// Uses the coordinator's independently accepted native authority.
    /// # Errors
    /// Rejects missing or invalid accepted geometry.
    pub fn from_accepted(editor: &'a ProjectionalEditorSession) -> Result<Self, String> {
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

    /// Finite accepted position of one source-owned native point.
    #[must_use]
    pub fn position(self, handle: &ExpandedPort) -> Option<[f64; 2]> {
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
pub struct RectangleTerminalProjection {
    anchors: [ExpandedPort; 2],
    redundant_aliases: [ExpandedPort; 2],
}

#[derive(Clone, Debug)]
pub struct CanonicalTerminalPointBundle {
    pub placements: Vec<(ExpandedWritablePoint, [f64; 2])>,
    pub rectangle_projections: Vec<RectangleTerminalProjection>,
}

// This is an arithmetic-depth budget applied to a semantic coordinate scale,
// not a fixed ULP-distance gate. Rectangle aliases are independently solved
// projections of two canonical seeds, so their last-bit drift is relative to
// the authenticated rectangle's own seeds and aliases rather than unrelated
// document geometry or one (possibly near-zero) coordinate.
const TERMINAL_SEED_ROUNDOFF_ULPS: u64 = 8;
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

/// Whether independent residual and feature validation permits publication.
#[must_use]
pub fn accepted_validation_is_publishable(
    validation: &geosolve_constraint_editor::IntentValidationEvidence,
) -> bool {
    validation.hard_residuals_validated
        && validation.all_active_features_current
        && validation
            .maximum_normalized_hard_residual
            .is_none_or(|residual| residual.is_finite() && residual <= 1.0e-9)
}

/// Validates exact current preview authority and independent residual evidence.
///
/// # Errors
/// Rejects missing, stale or independently invalid native geometry.
pub fn validate_terminal_preview_session(
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

/// Resolves a semantic point port through authenticated native ownership.
#[must_use]
pub fn expanded_port_point(
    editor: &ProjectionalEditorSession,
    handle: &ExpandedPort,
) -> Option<geosolve_sketch::DesignPointId> {
    if handle.kind != IntentPortKind::Point {
        return None;
    }
    let intent = editor.coordinator().intent();
    let node = intent.graph().node_by_symbol(&handle.alias)?;
    let port = node.port_by_selector(handle.selector)?;
    if port.kind != handle.kind {
        return None;
    }
    match editor
        .coordinator()
        .accepted_materialization()?
        .ownership
        .port(port.as_ref(node.id))?
    {
        IntentNativeBinding::Point(point) => Some(point),
        _ => None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedDeclarationLabelProjection {
    pub node: NodeId,
    pub terminal_symbol: IntentKey,
    pub declaration: SemanticSymbol,
}

/// Authenticates compiled source against native terminal construction.
///
/// # Errors
/// Rejects any changed geometry, explicit branch, provenance or identity.
pub fn validate_construction_parity(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    projections: &[PreparedDeclarationLabelProjection],
) -> Result<(), String> {
    validate_terminal_native_parity_with_trace(terminal, staged, expansion, &[], projections, None)
}

/// Captures exact native-to-source declaration label correspondence.
///
/// # Errors
/// Rejects duplicate, foreign or missing created declarations.
pub fn canvas_declaration_label_projections(
    candidate_editor: &ProjectionalEditorSession,
    declarations: &[crate::EditorBootstrapDeclaration],
) -> Result<Vec<PreparedDeclarationLabelProjection>, String> {
    let graph = candidate_editor.coordinator().intent().graph();
    let mut nodes = BTreeSet::new();
    let mut symbols = BTreeSet::new();
    declarations
        .iter()
        .map(|declaration| {
            if !nodes.insert(declaration.node) || !symbols.insert(declaration.symbol.clone()) {
                return Err("canvas declaration label witness repeats a node or symbol".into());
            }
            let node = graph.node(declaration.node).ok_or_else(|| {
                format!(
                    "canvas declaration label witness lost candidate node {}",
                    declaration.node
                )
            })?;
            Ok(PreparedDeclarationLabelProjection {
                node: declaration.node,
                terminal_symbol: node.symbol.clone(),
                declaration: declaration.symbol.clone(),
            })
        })
        .collect()
}

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

#[allow(
    clippy::too_many_lines,
    reason = "one fail-closed diagnostic walker mirrors the exact persistent feature/corner/parent hierarchy"
)]
fn first_terminal_feature_document_mismatch(
    terminal: &ComputedFeatureDocument,
    staged: &ComputedFeatureDocument,
    policy: &TerminalComputedParityPolicy,
) -> String {
    let terminal_identity = terminal.identity();
    let staged_identity = staged.identity();
    if terminal_identity.document != staged_identity.document {
        return format!(
            "feature.document terminal={:?} staged={:?}",
            terminal_identity.document, staged_identity.document
        );
    }
    if terminal_identity.sketch_document != staged_identity.sketch_document {
        return format!(
            "feature.sketch_document terminal={:?} staged={:?}",
            terminal_identity.sketch_document, staged_identity.sketch_document
        );
    }
    if terminal.allocator_high_water() != staged.allocator_high_water() {
        return format!(
            "feature.allocator terminal={:?} staged={:?}",
            terminal.allocator_high_water(),
            staged.allocator_high_water()
        );
    }
    if terminal.features().len() != staged.features().len() {
        return format!(
            "feature.count terminal={} staged={}",
            terminal.features().len(),
            staged.features().len()
        );
    }
    for (index, (terminal_feature, staged_feature)) in terminal
        .features()
        .iter()
        .zip(staged.features())
        .enumerate()
    {
        if terminal_feature.id != staged_feature.id {
            return format!(
                "feature[{index}].id terminal={:?} staged={:?}",
                terminal_feature.id, staged_feature.id
            );
        }
        if terminal_feature.label != staged_feature.label {
            return format!(
                "feature[{index}].label terminal={:?} staged={:?}",
                terminal_feature.label, staged_feature.label
            );
        }
        if terminal_feature.suppressed != staged_feature.suppressed {
            return format!(
                "feature[{index}].suppressed terminal={} staged={}",
                terminal_feature.suppressed, staged_feature.suppressed
            );
        }
        let (
            ComputedFeatureDefinition::FilletSet(terminal_fillet),
            ComputedFeatureDefinition::FilletSet(staged_fillet),
        ) = (&terminal_feature.definition, &staged_feature.definition);
        if terminal_fillet.radius.to_bits() != staged_fillet.radius.to_bits() {
            return format!(
                "feature[{index}].radius terminal={:.17e} staged={:.17e}",
                terminal_fillet.radius, staged_fillet.radius
            );
        }
        if terminal_fillet.corners.len() != staged_fillet.corners.len() {
            return format!(
                "feature[{index}].corner_count terminal={} staged={}",
                terminal_fillet.corners.len(),
                staged_fillet.corners.len()
            );
        }
        for (corner_index, (terminal_corner, staged_corner)) in terminal_fillet
            .corners
            .iter()
            .zip(&staged_fillet.corners)
            .enumerate()
        {
            if terminal_corner.id != staged_corner.id {
                return format!(
                    "feature[{index}].corner[{corner_index}].id terminal={:?} staged={:?}",
                    terminal_corner.id, staged_corner.id
                );
            }
            if terminal_corner.endpoint_order != staged_corner.endpoint_order {
                return format!(
                    "feature[{index}].corner[{corner_index}].endpoint_order terminal={:?} staged={:?}",
                    terminal_corner.endpoint_order, staged_corner.endpoint_order
                );
            }
            if terminal_corner.sweep != staged_corner.sweep {
                return format!(
                    "feature[{index}].corner[{corner_index}].sweep terminal={:?} staged={:?}",
                    terminal_corner.sweep, staged_corner.sweep
                );
            }
            for (parent_name, terminal_parent, staged_parent) in [
                ("first", terminal_corner.first, staged_corner.first),
                ("second", terminal_corner.second, staged_corner.second),
            ] {
                if terminal_parent.source != staged_parent.source {
                    return format!(
                        "feature[{index}].corner[{corner_index}].{parent_name}.source terminal={:?} staged={:?}",
                        terminal_parent.source, staged_parent.source
                    );
                }
                let parameter_matches = match policy {
                    TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales }
                        if !terminal_feature.suppressed
                            && source_scales.contains_key(&terminal_parent.source)
                            && source_scales.contains_key(&staged_parent.source) =>
                    {
                        terminal_derived_scalar_matches(
                            terminal_parent.picked_parameter,
                            staged_parent.picked_parameter,
                            1.0,
                        )
                    }
                    TerminalComputedParityPolicy::Exact
                    | TerminalComputedParityPolicy::RectangleAliasRoundoff { .. } => {
                        terminal_parent.picked_parameter.to_bits()
                            == staged_parent.picked_parameter.to_bits()
                    }
                };
                if !parameter_matches {
                    return format!(
                        "feature[{index}].corner[{corner_index}].{parent_name}.parameter terminal={:.17e} staged={:.17e}",
                        terminal_parent.picked_parameter, staged_parent.picked_parameter
                    );
                }
                if terminal_parent.winding != staged_parent.winding
                    || terminal_parent.neighborhood != staged_parent.neighborhood
                    || terminal_parent.normal_side != staged_parent.normal_side
                    || terminal_parent.retained_endpoint != staged_parent.retained_endpoint
                    || terminal_parent.periodic_anchor != staged_parent.periodic_anchor
                {
                    return format!(
                        "feature[{index}].corner[{corner_index}].{parent_name}.branch terminal={terminal_parent:?} staged={staged_parent:?}"
                    );
                }
            }
        }
    }
    "unknown feature-document mismatch".into()
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum TerminalScalarParityPolicy {
    Exact,
    Roundoff { coordinate_scale: f64 },
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

fn terminal_computed_periodic_angle_evidence(
    terminal: &ComputedFeatureSnapshot,
    staged: &ComputedFeatureSnapshot,
    policy: &TerminalComputedParityPolicy,
) -> Vec<String> {
    let TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } = policy else {
        return Vec::new();
    };
    terminal
        .edges()
        .iter()
        .zip(staged.edges())
        .enumerate()
        .flat_map(|(index, (terminal_edge, staged_edge))| {
            if terminal_computed_edge_roundoff_scale(terminal_edge, staged_edge, source_scales)
                .is_none()
                || !terminal_computed_edge_matches(terminal_edge, staged_edge, source_scales)
            {
                return Vec::new();
            }
            let (
                ComputedEdgeGeometry::CircularArc(terminal_arc),
                ComputedEdgeGeometry::CircularArc(staged_arc),
            ) = (&terminal_edge.geometry, &staged_edge.geometry)
            else {
                return Vec::new();
            };
            [
                (
                    "start_angle",
                    terminal_arc.start_angle,
                    staged_arc.start_angle,
                ),
                (
                    "end_angle",
                    terminal_arc.end_angle,
                    staged_arc.end_angle,
                ),
            ]
            .into_iter()
            .filter_map(|(field, terminal, staged)| {
                if terminal_derived_scalar_matches(terminal, staged, 1.0) {
                    return None;
                }
                let unwrapped = terminal_periodic_angle_unwrap(terminal, staged)?;
                terminal_derived_scalar_matches(terminal, unwrapped.staged, 1.0).then(|| {
                    format!(
                        "path=edge[{index}].arc.{field} periodic_turn={} unwrapped_staged={:.17e} unwrapped_residual={:.17e}",
                        unwrapped.turns, unwrapped.staged, unwrapped.residual,
                    )
                })
            })
            .collect::<Vec<_>>()
        })
        .collect()
}

fn terminal_scalar_mismatch_trace(
    path: &str,
    terminal: f64,
    staged: f64,
    policy: TerminalScalarParityPolicy,
    coordinate_domain: bool,
) -> String {
    let (coordinate_scale, tolerance) = match policy {
        TerminalScalarParityPolicy::Exact => (None, None),
        TerminalScalarParityPolicy::Roundoff { coordinate_scale } => {
            let scale = if coordinate_domain {
                coordinate_scale
            } else {
                1.0
            };
            (
                Some(scale),
                semantic_roundoff_tolerance(terminal, staged, scale),
            )
        }
    };
    format!(
        "path={path} terminal={:.17e}/0x{:016x} staged={:.17e}/0x{:016x} ulp_diff={} epsilon_budget={} coordinate_scale={coordinate_scale:?} tolerance={tolerance:?} zero_cell={} terminal_finite={} staged_finite={}",
        terminal,
        terminal.to_bits(),
        staged,
        staged.to_bits(),
        terminal.to_bits().abs_diff(staged.to_bits()),
        TERMINAL_SEED_ROUNDOFF_ULPS,
        TERMINAL_SEED_ZERO_ROUNDOFF,
        terminal.is_finite(),
        staged.is_finite(),
    )
}

fn terminal_periodic_angle_mismatch_trace(
    path: &str,
    terminal: f64,
    staged: f64,
    policy: TerminalScalarParityPolicy,
) -> String {
    let scalar = terminal_scalar_mismatch_trace(path, terminal, staged, policy, false);
    match policy {
        TerminalScalarParityPolicy::Exact => {
            format!("{scalar} periodic_policy=exact periodic_turn=None")
        }
        TerminalScalarParityPolicy::Roundoff { .. } => {
            let unwrapped = terminal_periodic_angle_unwrap(terminal, staged);
            let turn = unwrapped.map(|unwrapped| unwrapped.turns);
            let staged = unwrapped.map(|unwrapped| unwrapped.staged);
            let residual = unwrapped.map(|unwrapped| unwrapped.residual);
            let tolerance = unwrapped
                .and_then(|unwrapped| semantic_roundoff_tolerance(terminal, unwrapped.staged, 1.0));
            format!(
                "{scalar} periodic_policy=one-turn periodic_turn={turn:?} unwrapped_staged={staged:?} unwrapped_residual={residual:?} unwrapped_tolerance={tolerance:?}"
            )
        }
    }
}

fn terminal_pair_mismatch_trace(
    path: &str,
    terminal: [f64; 2],
    staged: [f64; 2],
    policy: TerminalScalarParityPolicy,
) -> Option<String> {
    ["x", "y"]
        .into_iter()
        .zip(terminal.into_iter().zip(staged))
        .find_map(|(axis, (terminal, staged))| {
            (!terminal_scalar_matches_trace_policy(terminal, staged, policy, true)).then(|| {
                terminal_scalar_mismatch_trace(
                    &format!("{path}.{axis}"),
                    terminal,
                    staged,
                    policy,
                    true,
                )
            })
        })
}

fn terminal_scalar_matches_trace_policy(
    terminal: f64,
    staged: f64,
    policy: TerminalScalarParityPolicy,
    coordinate_domain: bool,
) -> bool {
    match policy {
        TerminalScalarParityPolicy::Exact => terminal.to_bits() == staged.to_bits(),
        TerminalScalarParityPolicy::Roundoff { coordinate_scale } => {
            terminal_derived_scalar_matches(
                terminal,
                staged,
                if coordinate_domain {
                    coordinate_scale
                } else {
                    1.0
                },
            )
        }
    }
}

fn terminal_periodic_angle_matches_trace_policy(
    terminal: f64,
    staged: f64,
    policy: TerminalScalarParityPolicy,
) -> bool {
    match policy {
        TerminalScalarParityPolicy::Exact => terminal.to_bits() == staged.to_bits(),
        TerminalScalarParityPolicy::Roundoff { .. } => {
            terminal_derived_periodic_angle_matches(terminal, staged)
        }
    }
}

fn first_terminal_computed_provenance_mismatch(
    prefix: &str,
    terminal: &ComputedEdgeProvenance,
    staged: &ComputedEdgeProvenance,
    policy: TerminalScalarParityPolicy,
) -> String {
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
            for (path, terminal, staged) in [
                (
                    "source",
                    format!("{terminal_source:?}"),
                    format!("{staged_source:?}"),
                ),
                (
                    "start_claim",
                    format!("{terminal_start:?}"),
                    format!("{staged_start:?}"),
                ),
                (
                    "end_claim",
                    format!("{terminal_end:?}"),
                    format!("{staged_end:?}"),
                ),
            ] {
                if terminal != staged {
                    return format!(
                        "path={prefix}.provenance.{path} terminal={terminal} staged={staged}"
                    );
                }
            }
            for (path, terminal, staged) in [
                (
                    "interval.start",
                    terminal_interval.start,
                    staged_interval.start,
                ),
                ("interval.end", terminal_interval.end, staged_interval.end),
            ] {
                if !terminal_scalar_matches_trace_policy(terminal, staged, policy, false) {
                    return terminal_scalar_mismatch_trace(
                        &format!("{prefix}.provenance.{path}"),
                        terminal,
                        staged,
                        policy,
                        false,
                    );
                }
            }
            format!("path={prefix}.provenance.source_fragment unknown_mismatch")
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
        ) => {
            if terminal_owner != staged_owner {
                return format!(
                    "path={prefix}.provenance.owner terminal={terminal_owner:?} staged={staged_owner:?}"
                );
            }
            if terminal_sources != staged_sources {
                return format!(
                    "path={prefix}.provenance.sources terminal={terminal_sources:?} staged={staged_sources:?}"
                );
            }
            format!("path={prefix}.provenance.fillet_arc unknown_mismatch")
        }
        _ => format!("path={prefix}.provenance.variant terminal={terminal:?} staged={staged:?}"),
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the diagnostic enumerates every exact computed-edge field in causal comparison order"
)]
fn first_terminal_computed_edge_mismatch(
    index: usize,
    terminal: &geosolve_constraint_editor::ComputedEdge,
    staged: &geosolve_constraint_editor::ComputedEdge,
    policy: &TerminalComputedParityPolicy,
) -> String {
    let prefix = format!("edge[{index}]");
    if terminal.id.ordinal != staged.id.ordinal {
        return format!(
            "path={prefix}.id.ordinal terminal={} staged={}",
            terminal.id.ordinal, staged.id.ordinal
        );
    }
    if terminal.role != staged.role {
        return format!(
            "path={prefix}.role terminal={:?} staged={:?}",
            terminal.role, staged.role
        );
    }
    let scalar_policy = match policy {
        TerminalComputedParityPolicy::Exact => TerminalScalarParityPolicy::Exact,
        TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
            terminal_computed_edge_roundoff_scale(terminal, staged, source_scales)
                .map_or(TerminalScalarParityPolicy::Exact, |coordinate_scale| {
                    TerminalScalarParityPolicy::Roundoff { coordinate_scale }
                })
        }
    };
    let provenance_matches = match scalar_policy {
        TerminalScalarParityPolicy::Exact => terminal.provenance == staged.provenance,
        TerminalScalarParityPolicy::Roundoff { .. } => {
            terminal_computed_provenance_matches(&terminal.provenance, &staged.provenance)
        }
    };
    if !provenance_matches {
        return first_terminal_computed_provenance_mismatch(
            &prefix,
            &terminal.provenance,
            &staged.provenance,
            scalar_policy,
        );
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
            if terminal_source != staged_source {
                return format!(
                    "path={prefix}.native_source terminal={terminal_source:?} staged={staged_source:?}"
                );
            }
            for (field, terminal, staged) in [
                (
                    "interval.start",
                    terminal_interval.start,
                    staged_interval.start,
                ),
                ("interval.end", terminal_interval.end, staged_interval.end),
            ] {
                if !terminal_scalar_matches_trace_policy(terminal, staged, scalar_policy, false) {
                    return terminal_scalar_mismatch_trace(
                        &format!("{prefix}.{field}"),
                        terminal,
                        staged,
                        scalar_policy,
                        false,
                    );
                }
            }
            format!("path={prefix}.native_fragment unknown_mismatch")
        }
        (
            ComputedEdgeGeometry::CircularArc(terminal),
            ComputedEdgeGeometry::CircularArc(staged),
        ) => {
            if let Some(mismatch) = terminal_pair_mismatch_trace(
                &format!("{prefix}.arc.center"),
                terminal.center,
                staged.center,
                scalar_policy,
            ) {
                return mismatch;
            }
            if terminal.radius.to_bits() != staged.radius.to_bits() {
                return terminal_scalar_mismatch_trace(
                    &format!("{prefix}.arc.radius"),
                    terminal.radius,
                    staged.radius,
                    TerminalScalarParityPolicy::Exact,
                    true,
                );
            }
            for (field, terminal, staged) in [
                ("start_angle", terminal.start_angle, staged.start_angle),
                ("end_angle", terminal.end_angle, staged.end_angle),
            ] {
                if !terminal_periodic_angle_matches_trace_policy(terminal, staged, scalar_policy) {
                    return terminal_periodic_angle_mismatch_trace(
                        &format!("{prefix}.arc.{field}"),
                        terminal,
                        staged,
                        scalar_policy,
                    );
                }
            }
            if terminal.sweep != staged.sweep {
                return format!(
                    "path={prefix}.arc.sweep terminal={:?} staged={:?}",
                    terminal.sweep, staged.sweep
                );
            }
            if terminal.tangent_orientations != staged.tangent_orientations {
                return format!(
                    "path={prefix}.arc.tangent_orientations terminal={:?} staged={:?}",
                    terminal.tangent_orientations, staged.tangent_orientations
                );
            }
            if terminal.contacts.len() != staged.contacts.len() {
                return format!(
                    "path={prefix}.arc.contacts.length terminal={} staged={}",
                    terminal.contacts.len(),
                    staged.contacts.len()
                );
            }
            for (contact_index, (terminal, staged)) in
                terminal.contacts.iter().zip(&staged.contacts).enumerate()
            {
                let contact = format!("{prefix}.arc.contacts[{contact_index}]");
                if terminal.source != staged.source {
                    return format!(
                        "path={contact}.source terminal={:?} staged={:?}",
                        terminal.source, staged.source
                    );
                }
                if terminal.winding != staged.winding {
                    return format!(
                        "path={contact}.winding terminal={} staged={}",
                        terminal.winding, staged.winding
                    );
                }
                for (field, terminal, staged) in [
                    ("parameter", terminal.parameter, staged.parameter),
                    (
                        "total_parameter",
                        terminal.total_parameter,
                        staged.total_parameter,
                    ),
                ] {
                    if !terminal_scalar_matches_trace_policy(terminal, staged, scalar_policy, false)
                    {
                        return terminal_scalar_mismatch_trace(
                            &format!("{contact}.{field}"),
                            terminal,
                            staged,
                            scalar_policy,
                            false,
                        );
                    }
                }
                if let Some(mismatch) = terminal_pair_mismatch_trace(
                    &format!("{contact}.position"),
                    terminal.position,
                    staged.position,
                    scalar_policy,
                ) {
                    return mismatch;
                }
            }
            format!("path={prefix}.arc unknown_mismatch")
        }
        _ => format!(
            "path={prefix}.geometry.variant terminal={:?} staged={:?}",
            terminal.geometry, staged.geometry
        ),
    }
}

fn first_terminal_feature_evaluation_mismatch(
    index: usize,
    terminal: &ComputedFeatureEvaluation,
    staged: &ComputedFeatureEvaluation,
) -> String {
    let prefix = format!("feature_evaluations[{index}]");
    if terminal.feature != staged.feature {
        return format!(
            "path={prefix}.feature terminal={:?} staged={:?}",
            terminal.feature, staged.feature
        );
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
            if terminal.len() != staged.len() {
                return format!(
                    "path={prefix}.current.corner_edges.length terminal={} staged={}",
                    terminal.len(),
                    staged.len()
                );
            }
            for (edge_index, ((terminal_corner, terminal_edge), (staged_corner, staged_edge))) in
                terminal.iter().zip(staged).enumerate()
            {
                let edge = format!("{prefix}.current.corner_edges[{edge_index}]");
                if terminal_corner != staged_corner {
                    return format!(
                        "path={edge}.corner terminal={terminal_corner:?} staged={staged_corner:?}"
                    );
                }
                if terminal_edge.ordinal != staged_edge.ordinal {
                    return format!(
                        "path={edge}.edge.ordinal terminal={} staged={}",
                        terminal_edge.ordinal, staged_edge.ordinal
                    );
                }
            }
            format!("path={prefix}.current unknown_mismatch")
        }
        (
            ComputedFeatureEvaluationState::Failed { failure: terminal },
            ComputedFeatureEvaluationState::Failed { failure: staged },
        ) => format!("path={prefix}.failed terminal={terminal:?} staged={staged:?}"),
        (terminal, staged) => {
            format!("path={prefix}.state terminal={terminal:?} staged={staged:?}")
        }
    }
}

fn first_terminal_computed_snapshot_mismatch(
    terminal: &ComputedFeatureSnapshot,
    staged: &ComputedFeatureSnapshot,
    policy: &TerminalComputedParityPolicy,
) -> String {
    if terminal.edges().len() != staged.edges().len() {
        return format!(
            "path=edges.length terminal={} staged={}",
            terminal.edges().len(),
            staged.edges().len()
        );
    }
    for (index, (terminal, staged)) in terminal.edges().iter().zip(staged.edges()).enumerate() {
        let matches = match policy {
            TerminalComputedParityPolicy::Exact => {
                terminal.id.ordinal == staged.id.ordinal
                    && terminal.role == staged.role
                    && terminal.geometry == staged.geometry
                    && terminal.provenance == staged.provenance
            }
            TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
                terminal_computed_edge_matches(terminal, staged, source_scales)
            }
        };
        if !matches {
            return first_terminal_computed_edge_mismatch(index, terminal, staged, policy);
        }
    }
    if terminal.construction_fragments().len() != staged.construction_fragments().len() {
        return format!(
            "path=construction_fragments.length terminal={} staged={}",
            terminal.construction_fragments().len(),
            staged.construction_fragments().len()
        );
    }
    for (index, (terminal, staged)) in terminal
        .construction_fragments()
        .iter()
        .zip(staged.construction_fragments())
        .enumerate()
    {
        let matches = match policy {
            TerminalComputedParityPolicy::Exact => {
                terminal_computed_fragment_exact_matches(terminal, staged)
            }
            TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales } => {
                terminal_computed_fragment_matches(terminal, staged, source_scales)
            }
        };
        if !matches {
            return format!(
                "path=construction_fragments[{index}] terminal={terminal:?} staged={staged:?}"
            );
        }
    }
    if terminal.replaced_sources() != staged.replaced_sources() {
        return format!(
            "path=replaced_sources terminal={:?} staged={:?}",
            terminal.replaced_sources(),
            staged.replaced_sources()
        );
    }
    if terminal.feature_evaluations().len() != staged.feature_evaluations().len() {
        return format!(
            "path=feature_evaluations.length terminal={} staged={}",
            terminal.feature_evaluations().len(),
            staged.feature_evaluations().len()
        );
    }
    for (index, (terminal, staged)) in terminal
        .feature_evaluations()
        .iter()
        .zip(staged.feature_evaluations())
        .enumerate()
    {
        if !terminal_feature_evaluation_matches(terminal, staged) {
            return first_terminal_feature_evaluation_mismatch(index, terminal, staged);
        }
    }
    "path=computed_snapshot unknown_mismatch".into()
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
/// Reconstructs source-seeded design intent beneath an accepted native preview.
///
/// # Errors
/// Rejects missing or contradictory writable point bindings.
pub fn terminal_seeded_design_document(
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
    reason = "one exhaustive ownership audit authenticates all label relabels together"
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

/// Validates complete native parity without an interaction trace.
/// # Errors
/// Rejects any terminal geometry, provenance, branch or accepted-state mismatch.
#[cfg(test)]
pub fn validate_terminal_native_parity(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    rectangle_projections: &[RectangleTerminalProjection],
) -> Result<(), String> {
    validate_terminal_native_parity_with_trace(
        terminal,
        staged,
        expansion,
        rectangle_projections,
        &[],
        None,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one parity gate records every accepted-authority domain before issuing a single rejection"
)]
/// Checks terminal and compiled-source native parity with optional mismatch diagnostics.
///
/// # Errors
/// Rejects changed geometry, branch state, provenance or accepted validation.
pub fn validate_terminal_native_parity_with_trace(
    terminal: &ProjectionalEditorSession,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    rectangle_projections: &[RectangleTerminalProjection],
    declaration_label_projections: &[PreparedDeclarationLabelProjection],
    trace: Option<TerminalTraceSink<'_>>,
) -> Result<(), String> {
    validate_terminal_preview_native_parity_with_trace(
        TerminalPointPreview::from_accepted(terminal)?,
        staged,
        expansion,
        &[],
        rectangle_projections,
        declaration_label_projections,
        trace,
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one parity gate records every accepted-authority domain before issuing a single rejection"
)]
/// Checks source/native parity against an independently validated retained preview.
///
/// # Errors
/// Rejects any mismatch outside authenticated bounded rectangle roundoff.
pub fn validate_terminal_preview_native_parity_with_trace(
    terminal: TerminalPointPreview<'_>,
    staged: &ProjectionalEditorSession,
    expansion: &ExpandedCodeProject,
    terminal_seed_placements: &[(ExpandedWritablePoint, [f64; 2])],
    rectangle_projections: &[RectangleTerminalProjection],
    declaration_label_projections: &[PreparedDeclarationLabelProjection],
    mut trace: Option<TerminalTraceSink<'_>>,
) -> Result<(), String> {
    let terminal_authority = terminal
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "terminal code drag has no accepted native authority".to_owned())?;
    let staged_authority = staged
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| "staged code drag has no accepted native authority".to_owned())?;
    if !accepted_validation_is_publishable(&terminal_authority.validation)
        || !accepted_validation_is_publishable(&staged_authority.validation)
    {
        return Err(
            "terminal parity requires two independently validated current native authorities"
                .into(),
        );
    }
    let terminal_input = terminal
        .session
        .accepted_prepared_input()
        .ok_or_else(|| "terminal code drag has no accepted prepared input".to_owned())?;
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
        .map_err(|error| format!("terminal computed preview capture failed: {error}"))?
        .prepare(&mut allocator)
        .map_err(|error| format!("terminal computed preview preparation failed: {error}"))?
        .execute(OperationControl::unlimited())
        .map_err(|error| format!("terminal computed preview evaluation failed: {error}"))?;
        let OperationOutcome::Completed {
            value: computed, ..
        } = outcome
        else {
            return Err("terminal computed preview evaluation did not complete".into());
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
        .ok_or_else(|| "terminal code drag has no current accepted document".to_owned())?
        .document();
    let staged_accepted = staged_authority
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| "staged code drag has no current accepted document".to_owned())?
        .document();
    let terminal_design =
        terminal_seeded_design_document(terminal, terminal_seed_placements, rectangle_projections)?;
    let transported = if terminal_seed_placements.is_empty() {
        None
    } else {
        transport_code_point_terminal_branches(
            terminal.editor,
            expansion,
            terminal_accepted,
            &terminal_design,
        )?
    };
    let (terminal_accepted, terminal_design) = transported.as_ref().map_or(
        (terminal_accepted, &terminal_design),
        |(accepted, design)| (accepted, design),
    );
    let declaration_object_relabels = authenticated_declaration_object_relabels(
        terminal.editor,
        staged,
        expansion,
        declaration_label_projections,
        terminal_design,
        staged_authority.session.design_document(),
    )?;
    let design_document_parity = documents_match_for_terminal_parity(
        terminal.editor,
        staged,
        terminal_design,
        staged_authority.session.design_document(),
        rectangle_projections,
        &recomputable,
        &declaration_object_relabels,
    )?;
    let accepted_document_parity = documents_match_for_terminal_parity(
        terminal.editor,
        staged,
        terminal_accepted,
        staged_accepted,
        rectangle_projections,
        &recomputable,
        &declaration_object_relabels,
    )?;
    let same_documents = design_document_parity.matches()
        && accepted_document_parity.matches()
        && terminal_document_normalizations_match(
            &design_document_parity,
            &accepted_document_parity,
        );
    let computed_policy =
        terminal_computed_roundoff_points(&design_document_parity, &accepted_document_parity)
            .map(|point_scales| {
                terminal_roundoff_sources(staged_accepted, point_scales).map(|source_scales| {
                    TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales }
                })
            })
            .transpose()?
            .unwrap_or(TerminalComputedParityPolicy::Exact);
    let same_feature_documents = feature_documents_match_for_terminal_parity(
        &terminal_authority.features,
        &staged_authority.features,
        &computed_policy,
    );
    let same_computed_snapshots = computed_snapshots_match_for_terminal_parity(
        terminal_computed,
        &staged_authority.computed,
        &computed_policy,
    );
    let same_features = same_feature_documents && same_computed_snapshots;
    let same_semantic_inputs =
        terminal_semantic_inputs_match(&terminal_authority.session, &staged_authority.session);
    if let Some(trace) = trace.as_deref_mut() {
        record_trace(
            trace,
            "parity.documents",
            format!(
                "design={design_document_parity:?} accepted={accepted_document_parity:?} recomputable_line_branches={recomputable:?} authenticated_object_relabels={declaration_object_relabels:?}"
            ),
        );
        record_trace(
            trace,
            "parity.computed.policy",
            format!("{computed_policy:?}"),
        );
        let periodic_angles = terminal_computed_periodic_angle_evidence(
            terminal_computed,
            &staged_authority.computed,
            &computed_policy,
        );
        if !periodic_angles.is_empty() {
            record_trace(
                trace,
                "parity.computed.periodic-angle",
                periodic_angles.join(" | "),
            );
        }
        record_trace(
            trace,
            "parity.features",
            format!(
                "feature_documents={same_feature_documents} computed_snapshot={same_computed_snapshots} terminal_features={:?} staged_features={:?}",
                terminal_authority.features.identity(),
                staged_authority.features.identity(),
            ),
        );
        record_trace(
            trace,
            "parity.semantic-inputs",
            format!("durable_input_payloads={same_semantic_inputs}"),
        );
        if !same_computed_snapshots {
            record_trace(
                trace,
                "parity.computed.first-mismatch",
                first_terminal_computed_snapshot_mismatch(
                    terminal_computed,
                    &staged_authority.computed,
                    &computed_policy,
                ),
            );
        }
    }
    // Revision/digest stamps and Fillet pick seeds can refresh when staged
    // source is canonically rematerialized. Discrete topology, durable branch
    // cells, ownership rows and persistent IDs stay exact. Recomputed finite
    // feature scalars receive the same bounded cell only after redundant
    // rectangle aliases demonstrably needed that normalization above.
    let same_ownership = terminal_authority.ownership.nodes == staged_authority.ownership.nodes
        && terminal_authority.ownership.ports == staged_authority.ownership.ports
        && terminal_authority.ownership.reservations == staged_authority.ownership.reservations
        && terminal_authority.ownership.writable_leaves
            == staged_authority.ownership.writable_leaves
        && terminal_authority.ownership.aggregates == staged_authority.ownership.aggregates;
    if let Some(trace) = trace.as_deref_mut() {
        record_trace(
            trace,
            "parity.ownership",
            format!(
                "nodes={} ports={} reservations={} writable_leaves={} aggregates={}",
                terminal_authority.ownership.nodes == staged_authority.ownership.nodes,
                terminal_authority.ownership.ports == staged_authority.ownership.ports,
                terminal_authority.ownership.reservations
                    == staged_authority.ownership.reservations,
                terminal_authority.ownership.writable_leaves
                    == staged_authority.ownership.writable_leaves,
                terminal_authority.ownership.aggregates == staged_authority.ownership.aggregates,
            ),
        );
        record_trace(
            trace,
            "parity.allocators",
            format!(
                "feature_terminal={:?} feature_staged={:?} sketch_terminal={:?} sketch_staged={:?}",
                terminal_authority.feature_lifecycle_high_water.allocator,
                staged_authority.feature_lifecycle_high_water.allocator,
                terminal.session.persistent_identity_high_water(),
                staged_authority.session.persistent_identity_high_water(),
            ),
        );
    }
    let mut differences = Vec::new();
    if !same_documents {
        differences.push("sketch documents");
    }
    if !same_features {
        differences.push("computed features");
    }
    if !same_ownership {
        differences.push("native ownership");
    }
    if !same_semantic_inputs {
        differences.push("native semantic inputs");
    }
    if terminal_authority.feature_lifecycle_high_water.allocator
        != staged_authority.feature_lifecycle_high_water.allocator
    {
        differences.push("feature allocator");
    }
    if terminal.session.persistent_identity_high_water()
        != staged_authority.session.persistent_identity_high_water()
    {
        differences.push("sketch allocator");
    }
    if !differences.is_empty() {
        let mut error = format!(
            "terminal code drag differs from its independently staged native authority in {}",
            differences.join(", ")
        );
        if !same_feature_documents {
            let detail = first_terminal_feature_document_mismatch(
                &terminal_authority.features,
                &staged_authority.features,
                &computed_policy,
            );
            let _ = write!(error, "; {detail}");
        } else if !same_computed_snapshots {
            let detail = first_terminal_computed_snapshot_mismatch(
                terminal_computed,
                &staged_authority.computed,
                &computed_policy,
            );
            let _ = write!(error, "; {detail}");
        }
        if let Some(trace) = trace.as_deref_mut() {
            record_trace(trace, "parity.reject", &error);
        }
        return Err(error);
    }
    if let Some(trace) = trace {
        record_trace(
            trace,
            "parity.accept",
            "all terminal authority domains match",
        );
    }
    Ok(())
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

/// Canonicalizes a complete authenticated point terminal and derived rectangle aliases.
///
/// # Errors
/// Rejects incompatible lenses, noncanonical rectangles or invalid source point updates.
#[allow(
    clippy::too_many_lines,
    reason = "one transactional classification retains the complete rectangle alias proof"
)]
pub fn canonical_terminal_point_bundle(
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

/// Cold-validates source placement from one independently accepted native terminal.
///
/// # Errors
/// Rejects stale writable lenses, changed branches or source/native parity failures.
pub fn materialize_terminal(
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
    let seeded_design = terminal_seeded_design_document(
        preview,
        &bundle.placements,
        &bundle.rectangle_projections,
    )?;
    let transported = transport_code_point_terminal_branches(
        editor,
        &previous.expansion,
        continuation,
        &seeded_design,
    )?;
    let continuation = transported
        .as_ref()
        .map_or(continuation, |(accepted, _)| accepted);
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
    validate_terminal_preview_native_parity_with_trace(
        preview,
        &materialized.editor,
        &materialized.expansion,
        &bundle.placements,
        &bundle.rectangle_projections,
        &[],
        None,
    )?;
    Ok((materialized, overlay))
}

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

fn record_trace(trace: &mut dyn FnMut(&str, &str), stage: &str, detail: impl AsRef<str>) {
    trace(stage, detail.as_ref());
}

fn pair_bits(value: [f64; 2]) -> [u64; 2] {
    [value[0].to_bits(), value[1].to_bits()]
}

#[cfg(test)]
mod tests {
    use super::*;
    fn open_boxed(key: &str) -> (ExpandedCodeProject, Box<ProjectionalEditorSession>) {
        let project = crate::managed_regression_projects::managed_regression_project(key)
            .expect("exact historical parity fixture");
        let generated = KeyedReconcileState::empty()
            .plan(
                crate::required_generated_members(&project).expect("generated members"),
                &BTreeSet::new(),
            )
            .expect("generated reconciliation")
            .into_staged();
        let materialized = crate::materialize_code_project_cold(
            &project,
            &generated,
            geosolve_sketch_intent::IntentSessionId::from_raw(0x9900_0100),
            geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(0x9900_0100)),
            1.0,
        )
        .expect("independently materialized parity fixture");
        (materialized.expansion, Box::new(materialized.editor))
    }
    #[test]
    fn rectangle_terminal_roundoff_contract_is_tight_and_signed_zero_exact() {
        let seed = 1.0_f64;
        let within = f64::from_bits(seed.to_bits() + TERMINAL_SEED_ROUNDOFF_ULPS);
        let outside = f64::from_bits(seed.to_bits() + TERMINAL_SEED_ROUNDOFF_ULPS + 1);
        assert!(scalar_seed_roundoff_compatible(seed, seed, 1.0));
        assert!(scalar_seed_roundoff_compatible(seed, within, 1.0));
        assert!(!scalar_seed_roundoff_compatible(seed, outside, 1.0));
        assert!(scalar_seed_roundoff_compatible(
            3.0 * f64::EPSILON,
            -2.0 * f64::EPSILON,
            1.0,
        ));
        assert!(!scalar_seed_roundoff_compatible(
            2.0 * TERMINAL_SEED_ZERO_ROUNDOFF,
            0.0,
            1.0,
        ));
        assert!(!scalar_seed_roundoff_compatible(0.0, -0.0, 1.0));
        assert!(!scalar_seed_roundoff_compatible(f64::NAN, f64::NAN, 1.0));
        assert!(!scalar_seed_roundoff_compatible(
            f64::INFINITY,
            f64::INFINITY,
            1.0,
        ));

        assert!(terminal_derived_scalar_matches(seed, within, 1.0));
        assert!(!terminal_derived_scalar_matches(seed, outside, 1.0));
        assert!(terminal_derived_scalar_matches(0.0, -0.0, 1.0));
        assert!(terminal_derived_scalar_matches(
            3.0 * f64::EPSILON,
            -2.0 * f64::EPSILON,
            1.0,
        ));
        assert!(!terminal_derived_scalar_matches(
            2.0 * TERMINAL_SEED_ZERO_ROUNDOFF,
            0.0,
            1.0,
        ));
        assert!(!terminal_derived_scalar_matches(f64::NAN, f64::NAN, 1.0));
        assert!(!terminal_derived_scalar_matches(
            f64::INFINITY,
            f64::INFINITY,
            1.0,
        ));

        let browser_terminal = 1.743_119_266_055_046_5_f64;
        let projected_alias = f64::from_bits(browser_terminal.to_bits() + 14);
        assert!(scalar_seed_roundoff_compatible(
            browser_terminal,
            projected_alias,
            80.0,
        ));
        assert!(terminal_derived_scalar_matches(
            browser_terminal,
            projected_alias,
            80.0,
        ));
        assert!(!scalar_seed_roundoff_compatible(
            browser_terminal,
            browser_terminal + 1.0e-10,
            80.0,
        ));
    }

    fn typed_panel_fillet_edge_and_sources() -> (
        geosolve_constraint_editor::ComputedEdge,
        TerminalRoundoffSourceScales,
    ) {
        let (_workbench, editor) = open_boxed("typed-panel");
        let edge = editor
            .coordinator()
            .accepted_materialization()
            .expect("Typed Panel accepted materialization")
            .computed
            .edges()
            .iter()
            .find(|edge| matches!(edge.geometry, ComputedEdgeGeometry::CircularArc(_)))
            .expect("Typed Panel computed Fillet arc")
            .clone();
        let ComputedEdgeGeometry::CircularArc(reference_arc) = &edge.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        let source_scales = reference_arc
            .contacts
            .iter()
            .map(|contact| (contact.source, 1.0_f64.to_bits()))
            .collect();
        (edge, source_scales)
    }

    #[test]
    fn rectangle_terminal_periodic_arc_angle_roundoff_is_causal_and_bounded() {
        let (mut terminal, source_scales) = typed_panel_fillet_edge_and_sources();
        let mut staged = terminal.clone();
        let ComputedEdgeGeometry::CircularArc(terminal_arc) = &mut terminal.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        let ComputedEdgeGeometry::CircularArc(staged_arc) = &mut staged.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        terminal_arc.start_angle = -std::f64::consts::PI;
        staged_arc.start_angle = std::f64::consts::PI;

        assert!(
            terminal_computed_edge_matches(&terminal, &staged, &source_scales),
            "the authenticated computed arc must treat -PI/+PI as one periodic direction",
        );
        assert!(
            !terminal_computed_edge_matches(
                &terminal,
                &staged,
                &TerminalRoundoffSourceScales::new(),
            ),
            "an empty or unrelated policy must keep the same arc angles bit-exact",
        );
        assert!(!terminal_periodic_angle_matches_trace_policy(
            -std::f64::consts::PI,
            std::f64::consts::PI,
            TerminalScalarParityPolicy::Exact,
        ));
        assert!(terminal_periodic_angle_matches_trace_policy(
            -std::f64::consts::PI,
            std::f64::consts::PI,
            TerminalScalarParityPolicy::Roundoff {
                coordinate_scale: 1.0,
            },
        ));

        let ComputedEdgeGeometry::CircularArc(staged_arc) = &mut staged.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        staged_arc.start_angle = std::f64::consts::PI - 1.0e-6;
        assert!(
            !terminal_computed_edge_matches(&terminal, &staged, &source_scales),
            "a genuinely different direction must not enter the periodic seam cell",
        );
        let policy = TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales };
        let edge_index = usize::try_from(terminal.id.ordinal).expect("bounded computed edge");
        let mismatch =
            first_terminal_computed_edge_mismatch(edge_index, &terminal, &staged, &policy);
        assert!(mismatch.starts_with(&format!("path=edge[{edge_index}].arc.start_angle ")));
        assert!(mismatch.contains("periodic_policy=one-turn"));
        assert!(mismatch.contains("periodic_turn=Some(-1)"));
        assert!(mismatch.contains("unwrapped_staged=Some("));
        assert!(mismatch.contains("unwrapped_residual=Some("));
        assert!(mismatch.contains("unwrapped_tolerance=Some("));
        assert!(!mismatch.contains("unknown_mismatch"));
    }

    #[test]
    fn rectangle_roundoff_keeps_the_encoded_fillet_radius_bit_exact() {
        let (edge, source_scales) = typed_panel_fillet_edge_and_sources();
        let mut changed = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut changed.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.radius = f64::from_bits(arc.radius.to_bits() + 1);
        assert!(
            !terminal_computed_edge_matches(&edge, &changed, &source_scales),
            "the feature-owned radius is copied input, not causal rectangle roundoff",
        );
        let policy = TerminalComputedParityPolicy::RectangleAliasRoundoff { source_scales };
        let mismatch = first_terminal_computed_edge_mismatch(
            usize::try_from(edge.id.ordinal).expect("bounded computed edge"),
            &edge,
            &changed,
            &policy,
        );
        assert!(mismatch.contains(".arc.radius "));
        assert!(mismatch.contains("coordinate_scale=None"));
    }

    #[test]
    fn rectangle_roundoff_requires_internal_geometry_provenance_source_parity() {
        let (terminal, source_scales) = typed_panel_fillet_edge_and_sources();
        let mut inconsistent_terminal = terminal.clone();
        let mut inconsistent_staged = terminal.clone();
        for edge in [&mut inconsistent_terminal, &mut inconsistent_staged] {
            let ComputedEdgeProvenance::FilletArc { sources, .. } = &mut edge.provenance else {
                panic!("selected edge must retain Fillet provenance")
            };
            sources.swap(0, 1);
        }
        let ComputedEdgeGeometry::CircularArc(arc) = &mut inconsistent_staged.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.center[0] = f64::from_bits(arc.center[0].to_bits() + 1);
        assert_eq!(
            terminal_computed_edge_roundoff_scale(
                &inconsistent_terminal,
                &inconsistent_staged,
                &source_scales,
            ),
            None,
        );
        assert!(!terminal_computed_edge_matches(
            &inconsistent_terminal,
            &inconsistent_staged,
            &source_scales,
        ));
    }

    #[test]
    fn unrelated_large_rectangle_scale_does_not_inflate_a_computed_edge() {
        let (edge, mut source_scales) = typed_panel_fillet_edge_and_sources();
        let (_workbench, editor) = open_boxed("typed-panel");
        let unrelated_curve = editor
            .coordinator()
            .accepted_materialization()
            .expect("Typed Panel accepted materialization")
            .session
            .design_document()
            .curves()
            .iter()
            .map(|curve| NativeCurveSpanSource {
                span: CurveSpan {
                    curve: curve.id,
                    segment: 0,
                },
            })
            .find(|source| !source_scales.contains_key(source))
            .expect("Typed Panel has an unrelated rectangle source curve");
        source_scales.insert(unrelated_curve, 1.0e12_f64.to_bits());

        let mut material_difference = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut material_difference.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.center[0] += 1.0e-8;
        assert_eq!(
            terminal_computed_edge_roundoff_scale(&edge, &material_difference, &source_scales),
            Some(1.0),
        );
        assert!(
            !terminal_computed_edge_matches(&edge, &material_difference, &source_scales),
            "an unrelated large rectangle must not loosen this edge's local roundoff cell",
        );
    }

    #[test]
    fn rectangle_roundoff_marks_only_incident_polyline_spans() {
        let mut document = geosolve_sketch::SketchDocument::new(1.0).unwrap();
        let points = [[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [3.0, 0.0]]
            .map(|position| document.add_point("polyline control", position).unwrap());
        let curve = document
            .add_curve(
                "polyline",
                geosolve_sketch::CurveDefinition::Polyline {
                    points: points.to_vec(),
                    closed: false,
                    branch_directions: vec![[1.0, 0.0]; 3],
                },
            )
            .unwrap();
        let sources =
            terminal_roundoff_sources(&document, &BTreeMap::from([(points[0], 1.0_f64.to_bits())]))
                .expect("valid polyline roundoff sources");
        let source = |segment| NativeCurveSpanSource {
            span: CurveSpan { curve, segment },
        };
        assert_eq!(
            sources.keys().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([source(0)]),
        );
        assert!(!sources.contains_key(&source(1)));
        assert!(!sources.contains_key(&source(2)));
    }

    #[test]
    fn rectangle_roundoff_marks_only_locally_supported_spline_spans() {
        let mut document = geosolve_sketch::SketchDocument::new(1.0).unwrap();
        let clamped_controls = (0..7)
            .map(|index| {
                document
                    .add_point("clamped control", [f64::from(index), 0.0])
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let clamped_spans = [101, 103, 107, 109];
        let clamped = document
            .add_curve(
                "clamped cubic",
                geosolve_sketch::CurveDefinition::BSpline {
                    form: geosolve_sketch::DocumentBSplineForm::Clamped,
                    degree: 3,
                    controls: clamped_controls.clone(),
                    knots: vec![0.0, 0.0, 0.0, 0.0, 0.2, 0.55, 0.8, 1.0, 1.0, 1.0, 1.0],
                    span_ids: clamped_spans.to_vec(),
                    next_span_id: 110,
                },
            )
            .unwrap();

        let periodic_controls = (0..5)
            .map(|index| {
                document
                    .add_point("periodic control", [f64::from(index), 10.0])
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let periodic_spans = [11, 17, 23, 29, 31];
        let periodic = document
            .add_curve(
                "periodic quadratic",
                geosolve_sketch::CurveDefinition::BSpline {
                    form: geosolve_sketch::DocumentBSplineForm::Periodic,
                    degree: 2,
                    controls: periodic_controls.clone(),
                    knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
                    span_ids: periodic_spans.to_vec(),
                    next_span_id: 32,
                },
            )
            .unwrap();
        let sources = terminal_roundoff_sources(
            &document,
            &BTreeMap::from([
                (clamped_controls[0], 1.0_f64.to_bits()),
                (periodic_controls[0], 2.0_f64.to_bits()),
            ]),
        )
        .expect("valid local spline roundoff sources");
        let source = |curve, segment| NativeCurveSpanSource {
            span: CurveSpan { curve, segment },
        };

        assert!(sources.contains_key(&source(clamped, clamped_spans[0])));
        for segment in &clamped_spans[1..] {
            assert!(
                !sources.contains_key(&source(clamped, *segment)),
                "a distant clamped span must remain exact",
            );
        }
        for segment in [periodic_spans[0], periodic_spans[3], periodic_spans[4]] {
            assert!(
                sources.contains_key(&source(periodic, segment)),
                "periodic wraparound support must remain causal",
            );
        }
        for segment in [periodic_spans[1], periodic_spans[2]] {
            assert!(
                !sources.contains_key(&source(periodic, segment)),
                "a distant periodic span must remain exact",
            );
        }
    }

    #[test]
    fn terminal_evaluation_mismatch_trace_ignores_refreshable_edge_revisions() {
        let (_workbench, editor) = open_boxed("typed-panel");
        let evaluation = editor
            .coordinator()
            .accepted_materialization()
            .expect("Typed Panel accepted materialization")
            .computed
            .feature_evaluations()
            .first()
            .expect("Typed Panel computed feature evaluation")
            .clone();
        let mut refreshed = evaluation.clone();
        {
            let ComputedFeatureEvaluationState::Current { corner_edges } = &mut refreshed.state
            else {
                panic!("Typed Panel evaluation must remain Current")
            };
            let (_, edge) = corner_edges
                .first_mut()
                .expect("Typed Panel Current evaluation edge");
            edge.evaluation = geosolve_constraint_editor::ComputedEvaluationRevision::from_raw(
                edge.evaluation.raw().saturating_add(1),
            );
        }
        assert_ne!(evaluation, refreshed);
        assert!(
            terminal_feature_evaluation_matches(&evaluation, &refreshed),
            "evaluation-local edge revisions are deliberately outside terminal parity",
        );

        let ComputedFeatureEvaluationState::Current { corner_edges } = &mut refreshed.state else {
            panic!("Typed Panel evaluation must remain Current")
        };
        let (_, edge) = corner_edges
            .first_mut()
            .expect("Typed Panel Current evaluation edge");
        edge.ordinal = edge.ordinal.saturating_add(1);
        assert!(!terminal_feature_evaluation_matches(
            &evaluation,
            &refreshed,
        ));
        assert!(
            first_terminal_feature_evaluation_mismatch(0, &evaluation, &refreshed)
                .contains(".edge.ordinal "),
        );
    }

    #[test]
    fn validate_terminal_native_parity_trace_rejects_a_computed_scalar_mismatch() {
        let (workbench, mut terminal) = open_boxed("typed-panel");
        let expansion = workbench;
        let staged = terminal
            .fork_accepted_authority()
            .expect("independent staged Typed Panel authority");
        let (feature, radius) = {
            let accepted = terminal
                .coordinator()
                .accepted_materialization()
                .expect("Typed Panel accepted materialization");
            let feature = accepted
                .features
                .features()
                .first()
                .expect("Typed Panel computed Fillet feature");
            let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
            (feature.id, fillet.radius)
        };
        let changed_radius = f64::from_bits(radius.to_bits() + TERMINAL_SEED_ROUNDOFF_ULPS + 1);
        terminal
            .edit_computed_fillet_radius(feature, changed_radius)
            .expect("finite nearby radius remains an accepted native authority");

        let mut events = Vec::new();
        let mut trace = |stage: &str, detail: &str| events.push(format!("\t{stage}\t{detail}"));
        let error = validate_terminal_native_parity_with_trace(
            &terminal,
            &staged,
            &expansion,
            &[],
            &[],
            Some(&mut trace),
        )
        .expect_err("changed computed radius must reject terminal/native parity");
        assert!(error.contains("computed features"));

        let exported = events.join("\n");
        let mismatch_row = exported
            .lines()
            .find(|line| line.contains("\tparity.computed.first-mismatch\t"))
            .unwrap_or_else(|| panic!("trace must name its first computed mismatch: {exported}"));
        assert!(mismatch_row.contains("path=edge["));
        assert!(mismatch_row.contains(".arc."));
        assert!(!mismatch_row.contains("unknown_mismatch"));
        assert!(mismatch_row.contains("ulp_diff="));
        assert!(mismatch_row.contains(&format!("epsilon_budget={TERMINAL_SEED_ROUNDOFF_ULPS}")));
        assert!(
            exported
                .find("\tparity.computed.first-mismatch\t")
                .expect("first-mismatch stage")
                < exported
                    .find("\tparity.reject\t")
                    .expect("terminal parity rejection stage"),
        );
    }

    #[test]
    fn rectangle_terminal_derived_roundoff_keeps_public_fillet_branch_state_exact() {
        let (edge, source_scales) = typed_panel_fillet_edge_and_sources();
        let mut changed_branch = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut changed_branch.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.sweep = match arc.sweep {
            geosolve_sketch::DocumentArcSweep::Clockwise => {
                geosolve_sketch::DocumentArcSweep::CounterClockwise
            }
            geosolve_sketch::DocumentArcSweep::CounterClockwise => {
                geosolve_sketch::DocumentArcSweep::Clockwise
            }
        };
        assert!(!terminal_computed_edge_matches(
            &edge,
            &changed_branch,
            &source_scales,
        ));

        let mut changed_tangent = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut changed_tangent.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.tangent_orientations[0] = match arc.tangent_orientations[0] {
            geosolve_sketch::TangentOrientation::Aligned => {
                geosolve_sketch::TangentOrientation::Opposed
            }
            geosolve_sketch::TangentOrientation::Opposed => {
                geosolve_sketch::TangentOrientation::Aligned
            }
        };
        assert!(!terminal_computed_edge_matches(
            &edge,
            &changed_tangent,
            &source_scales,
        ));

        let mut changed_winding = edge.clone();
        let ComputedEdgeGeometry::CircularArc(arc) = &mut changed_winding.geometry else {
            panic!("selected edge must remain a circular arc")
        };
        arc.contacts[0].winding = arc.contacts[0]
            .winding
            .checked_add(1)
            .expect("bounded fixture winding");
        assert!(!terminal_computed_edge_matches(
            &edge,
            &changed_winding,
            &source_scales,
        ));

        let mut changed_provenance = edge.clone();
        let ComputedEdgeProvenance::FilletArc { sources, .. } = &mut changed_provenance.provenance
        else {
            panic!("selected edge must retain Fillet provenance")
        };
        sources.swap(0, 1);
        assert!(!terminal_computed_edge_matches(
            &edge,
            &changed_provenance,
            &source_scales,
        ));
    }

    #[test]
    fn computed_roundoff_requires_matching_design_and_accepted_alias_normalization() {
        let (_workbench, editor) = open_boxed("typed-panel");
        let points = editor
            .coordinator()
            .accepted_materialization()
            .expect("Typed Panel accepted materialization")
            .session
            .design_document()
            .points();
        let first = points.first().expect("Typed Panel point").id;
        let second = points.get(1).expect("second Typed Panel point").id;
        let normalized = TerminalDocumentParity::NormalizedRedundantRectangleAliases(
            BTreeMap::from([(first, 1.0_f64.to_bits())]),
        );
        let accepted_scale = TerminalDocumentParity::NormalizedRedundantRectangleAliases(
            BTreeMap::from([(first, 2.0_f64.to_bits())]),
        );
        let other =
            TerminalDocumentParity::NormalizedRedundantRectangleAliases(BTreeMap::from([(
                second,
                1.0_f64.to_bits(),
            )]));
        assert_eq!(
            terminal_computed_roundoff_points(&normalized, &normalized),
            Some(&BTreeMap::from([(first, 1.0_f64.to_bits())])),
        );
        assert!(terminal_document_normalizations_match(
            &normalized,
            &normalized,
        ));
        assert_eq!(
            terminal_computed_roundoff_points(&normalized, &accepted_scale),
            accepted_scale.normalized_redundant_rectangle_aliases(),
            "computed roundoff uses the accepted domain's scale metadata",
        );
        assert!(terminal_document_normalizations_match(
            &normalized,
            &accepted_scale,
        ));
        assert!(terminal_document_normalizations_match(
            &TerminalDocumentParity::Exact,
            &TerminalDocumentParity::Exact,
        ));
        assert!(
            terminal_computed_roundoff_points(&TerminalDocumentParity::Exact, &normalized)
                .is_none()
        );
        assert!(
            terminal_computed_roundoff_points(&normalized, &TerminalDocumentParity::Exact)
                .is_none()
        );
        assert!(terminal_computed_roundoff_points(&normalized, &other).is_none());
        assert!(!terminal_document_normalizations_match(
            &TerminalDocumentParity::Exact,
            &normalized,
        ));
        assert!(terminal_document_normalizations_match(
            &normalized,
            &TerminalDocumentParity::Exact,
        ));
        assert!(!terminal_document_normalizations_match(&normalized, &other,));
    }
}
