// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use geosolve_constraint_editor::{IntentNativeBinding, IntentValidationEvidence};
mod authoring;
mod construction;
mod construction_names;
mod point_gesture;
mod profiles;
mod replay;
pub use replay::{ConstructionAllocation, ConstructionReplayWitness, PointReplayWitness};
mod session;
mod terminal;
pub use authoring::{
    AuthoringPrediction, AuthoringValueInverse, AuthoringValueWrite, PreparedAuthoringMutation,
    PreparedAuthoringSource, PreparedAuthoringValues,
};
pub use construction::{
    ConstructionCommand, ConstructionEvent, ConstructionFrame, ConstructionGuide,
    ConstructionPrediction, ConstructionSample, ConstructionTerminal, ConstructionTool,
    MAX_CONSTRUCTION_SAMPLES, PreparedConstruction, PreparedConstructionCommit,
};
pub use point_gesture::{
    MAX_POINT_GESTURE_SAMPLES, PointGestureCommand, PointGestureFrame, PointGestureHandle,
    PointGestureSample, PointGestureTarget, PointGestureTerminal, RetainedPointGesture,
};
pub use session::{
    EditableDesign, EditableSession, EditableSessionState, PreparedPointGestureCommit,
};

use geosolve_sketch::{
    CurveSpan, DesignCurve, DesignPoint, DesignPointId, DesignScalar, DocumentArcSweep, DocumentId,
    GeometryRole, PersistentId,
};
use geosolve_sketch_code::{
    CodeProject, ExpandedSemanticOutput, ExpandedSemanticTarget, GeneratedApplication,
    GeneratedGroup, GeneratedParameter, GeneratedSketchArtifact, GeneratedValue,
    KeyedReconcileState, ManagedDocumentPresentation, MaterializedCodeProject,
    materialize_code_project_cold, materialize_generated_sketch_cold, required_generated_members,
};
use geosolve_sketch_features::{ComputedEdgeGeometry, ComputedEdgeProvenance};
use geosolve_sketch_intent::{IntentSessionId, intent_content_digest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const ENGINE_RESULT_FORMAT: &str = "geosolve-engine-result-v1";

/// Complete compiler result and pinned local files, before native materialization.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ManagedProjectInput {
    pub project: String,
    pub compiled: geosolve_sketch_code::CompiledManagedSource,
    pub custom_files: BTreeMap<String, geosolve_sketch_code::CodeProjectFile>,
    pub artifacts: BTreeMap<String, serde_json::Value>,
    pub lock: serde_json::Value,
}

/// Constructs the existing strict offline project without fabricating source projection.
///
/// # Errors
/// Rejects oversized, invalid compiler authority, stale source/artifact pins or invalid paths.
pub fn compile_project_json(json: &str) -> Result<String, EngineError> {
    if json.len() > geosolve_sketch_code::CODE_PROJECT_LIMIT {
        return Err(EngineError::Admission(
            "project input byte limit exceeded".into(),
        ));
    }
    let input: ManagedProjectInput =
        serde_json::from_str(json).map_err(|error| EngineError::Admission(error.to_string()))?;
    let managed = input
        .compiled
        .into_managed_document()
        .map_err(|error| EngineError::Admission(error.to_string()))?;
    let project = CodeProject {
        project: geosolve_sketch_code::ProjectKey(input.project),
        managed,
        custom_files: input.custom_files,
        artifacts: input.artifacts,
        lock: input.lock,
    };
    project
        .to_canonical_json()
        .map_err(|error| EngineError::Admission(error.to_string()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineAuthoringMode {
    Editable,
    Generator,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineCapabilities {
    pub managed_source_edits: bool,
    pub reverse_geometry_edits: bool,
    /// The export operation is available; topology and numerical validation may
    /// still reject this particular geometry or requested chord tolerance.
    pub profile_export: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EngineComputedGeometry {
    NativeFragment {
        ordinal: u32,
        span: CurveSpan,
        interval: [f64; 2],
        role: GeometryRole,
    },
    CircularArc {
        ordinal: u32,
        center: [f64; 2],
        radius: f64,
        start_angle: f64,
        end_angle: f64,
        sweep: DocumentArcSweep,
        role: GeometryRole,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineCurve {
    pub curve: DesignCurve,
    pub role: GeometryRole,
    pub visible_intervals: Vec<EngineVisibleInterval>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineVisibleInterval {
    pub span: CurveSpan,
    pub interval: [f64; 2],
}

/// Model-space data only. Rendering is entirely host-owned.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineGeometry {
    pub points: Vec<DesignPoint>,
    pub scalars: Vec<DesignScalar>,
    pub curves: Vec<EngineCurve>,
    pub computed_edges: Vec<EngineComputedGeometry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineGeneratedMetadata {
    pub parameters: Vec<GeneratedParameter>,
    pub applications: Vec<GeneratedApplication>,
    pub groups: Vec<GeneratedGroup>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineOutputGeometry {
    pub points: Vec<DesignPointId>,
    pub spans: Vec<CurveSpan>,
    pub computed_edges: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EngineAcceptedResult {
    pub format: String,
    pub status: String,
    pub result_id: String,
    pub input_digest: String,
    pub mode: EngineAuthoringMode,
    pub capabilities: EngineCapabilities,
    pub document: ManagedDocumentPresentation,
    pub validation: IntentValidationEvidence,
    pub geometry: EngineGeometry,
    pub named_outputs: BTreeMap<String, ExpandedSemanticOutput>,
    pub named_geometry: BTreeMap<String, EngineOutputGeometry>,
    pub generated_output: Option<GeneratedValue>,
    pub generated_metadata: Option<EngineGeneratedMetadata>,
}

#[derive(Debug)]
struct EvaluationAuthority {
    report: EngineAcceptedResult,
    materialized: MaterializedCodeProject,
}

/// An immutable, independently accepted result. Cloning retains the same exact
/// native authority without copying the document, history or geometry.
#[derive(Clone, Debug)]
pub struct AcceptedEvaluation(Rc<EvaluationAuthority>);

impl AcceptedEvaluation {
    pub fn result(&self) -> &EngineAcceptedResult {
        &self.0.report
    }

    /// Exports independently validated production regions with model-space chord bounds.
    ///
    /// # Errors
    /// Rejects invalid tolerance, unsupported geometry, incomplete production
    /// topology, or invalid sampled loops.
    pub fn export_profiles(
        &self,
        max_chord_error_mm: f64,
    ) -> Result<serde_json::Value, EngineError> {
        self.export_selected_profiles(max_chord_error_mm, None)
    }

    /// Exports complete regions containing at least one outer-boundary fragment
    /// owned by the named output. Holes are retained. Full topology is validated
    /// before selection; an individual edge may therefore select its whole region.
    ///
    /// # Errors
    /// Rejects unknown names, incomplete geometry, invalid sampling or no selected region.
    pub fn export_profiles_for_output(
        &self,
        max_chord_error_mm: f64,
        output: &str,
    ) -> Result<serde_json::Value, EngineError> {
        let selection = self
            .result()
            .named_geometry
            .get(output)
            .ok_or_else(|| EngineError::Export(format!("unknown output {output}")))?;
        self.export_selected_profiles(max_chord_error_mm, Some(selection))
    }

    fn export_selected_profiles(
        &self,
        max_chord_error_mm: f64,
        selection: Option<&EngineOutputGeometry>,
    ) -> Result<serde_json::Value, EngineError> {
        let accepted = self
            .0
            .materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| EngineError::Validation("missing accepted authority".into()))?;
        let regions = profiles::export(accepted, max_chord_error_mm, selection)?.into_iter()
            .map(|region| serde_json::json!({ "id": region.id, "outer": region.outer, "holes": region.holes })).collect::<Vec<_>>();
        Ok(serde_json::json!({
            "format": "geosolve-baked-profile-v1", "units": "mm",
            "plane": { "origin": [0, 0, 0], "x_axis": [1, 0, 0], "y_axis": [0, 1, 0] },
            "sampling": { "max_chord_error_mm": max_chord_error_mm }, "regions": regions,
            "evaluation": { "result_id": self.result().result_id, "input_digest": self.result().input_digest },
        }))
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EngineError {
    #[error("engine admission: {0}")]
    Admission(String),
    #[error("engine materialization: {0}")]
    Materialization(String),
    #[error("engine independent validation: {0}")]
    Validation(String),
    #[error("engine profile export: {0}")]
    Export(String),
}

/// Synchronous headless evaluation. Hosts use a terminable worker when execution
/// cancellation is required; rejected calls never replace the accepted handle.
#[derive(Debug, Default)]
pub struct SketchEngine {
    accepted: Option<AcceptedEvaluation>,
}

impl SketchEngine {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn last_accepted(&self) -> Option<&AcceptedEvaluation> {
        self.accepted.as_ref()
    }

    /// Evaluates a bounded generator artifact with no managed source authority.
    ///
    /// # Errors
    /// Returns an admission, reference, solve, feature or independent-validation error.
    pub fn evaluate_generated_json(
        &mut self,
        json: &str,
    ) -> Result<AcceptedEvaluation, EngineError> {
        let generated = GeneratedSketchArtifact::from_json(json)
            .map_err(|error| EngineError::Admission(error.to_string()))?;
        let (intent, document) = deterministic_ids(generated.digest().as_bytes());
        let materialized = materialize_generated_sketch_cold(&generated, intent, document, 1.0)
            .map_err(|error| EngineError::Materialization(error.to_string()))?;
        self.publish(
            materialized,
            generated.digest().into(),
            EngineAuthoringMode::Generator,
            generated.artifact().document.clone(),
            Some(generated.artifact().output.clone()),
            Some(EngineGeneratedMetadata {
                parameters: generated.artifact().parameters.clone(),
                applications: generated.artifact().applications.clone(),
                groups: generated.artifact().groups.clone(),
            }),
        )
    }

    /// Evaluates the existing independently authenticated editable project wire.
    ///
    /// # Errors
    /// Returns an admission, solve, feature or independent-validation error.
    pub fn evaluate_managed_json(&mut self, json: &str) -> Result<AcceptedEvaluation, EngineError> {
        let project = CodeProject::from_json(json)
            .map_err(|error| EngineError::Admission(error.to_string()))?;
        let canonical = project
            .to_canonical_json()
            .map_err(|error| EngineError::Admission(error.to_string()))?;
        let input_digest = intent_content_digest(canonical.as_bytes()).to_string();
        let (intent, document) = deterministic_ids(input_digest.as_bytes());
        let mut reconciliation = KeyedReconcileState::default();
        let members = required_generated_members(&project)
            .map_err(|error| EngineError::Admission(error.to_string()))?;
        let plan = reconciliation
            .plan(members, &BTreeSet::new())
            .map_err(|error| EngineError::Admission(error.to_string()))?;
        reconciliation
            .commit(plan)
            .map_err(|error| EngineError::Admission(error.to_string()))?;
        let presentation = project
            .managed
            .compiled
            .as_deref()
            .map(geosolve_sketch_code::CompiledManagedSource::document_presentation)
            .transpose()
            .map_err(|error| EngineError::Admission(error.to_string()))?
            .unwrap_or_default();
        let materialized =
            materialize_code_project_cold(&project, &reconciliation, intent, document, 1.0)
                .map_err(|error| EngineError::Materialization(error.to_string()))?;
        self.publish(
            materialized,
            input_digest,
            EngineAuthoringMode::Editable,
            presentation,
            None,
            None,
        )
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one candidate publication validates all model-space output before replacing authority"
    )]
    fn publish(
        &mut self,
        materialized: MaterializedCodeProject,
        input_digest: String,
        mode: EngineAuthoringMode,
        document: ManagedDocumentPresentation,
        generated_output: Option<GeneratedValue>,
        generated_metadata: Option<EngineGeneratedMetadata>,
    ) -> Result<AcceptedEvaluation, EngineError> {
        let accepted = materialized
            .editor
            .coordinator()
            .accepted_materialization()
            .ok_or_else(|| EngineError::Validation("no accepted materialization".into()))?;
        let validation = &accepted.validation;
        if !validation.hard_residuals_validated
            || !validation.all_active_features_current
            || validation
                .maximum_normalized_hard_residual
                .is_some_and(|value| !value.is_finite() || value > 1e-9)
        {
            return Err(EngineError::Validation(
                "invalid independent residual/feature evidence".into(),
            ));
        }
        let state = accepted
            .session
            .accepted_state_for_current_input()
            .ok_or_else(|| EngineError::Validation("no current accepted state".into()))?;
        let native = state.document();
        if native
            .points()
            .iter()
            .flat_map(|point| point.position)
            .any(|value| !value.is_finite())
            || native
                .scalars()
                .iter()
                .any(|scalar| !scalar.value.is_finite())
        {
            return Err(EngineError::Validation("nonfinite native geometry".into()));
        }
        let computed_edges = accepted
            .computed
            .edges()
            .iter()
            .map(|edge| match &edge.geometry {
                ComputedEdgeGeometry::NativeSourceFragment { source, interval } => {
                    Ok(EngineComputedGeometry::NativeFragment {
                        ordinal: edge.id.ordinal,
                        span: source.span,
                        interval: [interval.start, interval.end],
                        role: edge.role,
                    })
                }
                ComputedEdgeGeometry::CircularArc(arc) => Ok(EngineComputedGeometry::CircularArc {
                    ordinal: edge.id.ordinal,
                    center: arc.center,
                    radius: arc.radius,
                    start_angle: arc.start_angle,
                    end_angle: arc.end_angle,
                    sweep: arc.sweep,
                    role: edge.role,
                }),
                _ => Err(EngineError::Validation(
                    "unsupported computed geometry projection".into(),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let curves = native
            .curves()
            .iter()
            .map(|curve| {
                let visible_intervals = if state.effective_activity().is_active(curve.id) {
                    native
                        .visible_curve_intervals(curve.id)
                        .map_err(|error| EngineError::Validation(error.to_string()))?
                        .into_iter()
                        .filter(|interval| {
                            !accepted
                                .computed
                                .replaced_sources()
                                .iter()
                                .any(|source| source.span == interval.support)
                        })
                        .map(|interval| EngineVisibleInterval {
                            span: interval.support,
                            interval: [interval.start, interval.end],
                        })
                        .collect()
                } else {
                    Vec::new()
                };
                Ok(EngineCurve {
                    curve: curve.clone(),
                    role: native
                        .geometry_role(curve.id)
                        .unwrap_or(GeometryRole::Profile),
                    visible_intervals,
                })
            })
            .collect::<Result<Vec<_>, EngineError>>()?;
        let report = EngineAcceptedResult {
            format: ENGINE_RESULT_FORMAT.into(),
            status: "accepted".into(),
            result_id: format!("evaluation.{input_digest}"),
            input_digest,
            mode,
            capabilities: EngineCapabilities {
                managed_source_edits: false,
                reverse_geometry_edits: false,
                profile_export: true,
            },
            document,
            validation: validation.clone(),
            geometry: EngineGeometry {
                points: native.points().to_vec(),
                scalars: native.scalars().to_vec(),
                curves,
                computed_edges,
            },
            named_outputs: materialized.expansion.semantic_outputs.clone(),
            named_geometry: materialized
                .expansion
                .semantic_outputs
                .iter()
                .map(|(name, output)| Ok((name.clone(), output_geometry(&materialized, output)?)))
                .collect::<Result<_, EngineError>>()?,
            generated_output,
            generated_metadata,
        };
        let result = AcceptedEvaluation(Rc::new(EvaluationAuthority {
            report,
            materialized,
        }));
        self.accepted = Some(result.clone());
        Ok(result)
    }
}

fn deterministic_ids(bytes: &[u8]) -> (IntentSessionId, DocumentId) {
    let digest = intent_content_digest(bytes);
    let bytes = digest.bytes();
    let mut left = [0_u8; 16];
    let mut right = [0_u8; 16];
    left.copy_from_slice(&bytes[..16]);
    right.copy_from_slice(&bytes[16..]);
    (
        IntentSessionId::from_raw(u128::from_be_bytes(left).max(1)),
        DocumentId(PersistentId::from_u128(u128::from_be_bytes(right).max(1))),
    )
}

#[allow(
    clippy::too_many_lines,
    reason = "one projection resolves every semantic output kind against exact native ownership"
)]
fn output_geometry(
    materialized: &MaterializedCodeProject,
    output: &ExpandedSemanticOutput,
) -> Result<EngineOutputGeometry, EngineError> {
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .ok_or_else(|| EngineError::Validation("missing output authority".into()))?;
    let native = accepted
        .session
        .accepted_state_for_current_input()
        .ok_or_else(|| EngineError::Validation("missing output native state".into()))?
        .document();
    let mut bindings = BTreeSet::new();
    let mut explicit_edges = BTreeSet::new();
    let mut pending = vec![&output.target];
    while let Some(target) = pending.pop() {
        match target {
            ExpandedSemanticTarget::Port { port } => {
                if let Some(port) = materialized
                    .base_outcome
                    .aliases
                    .port(&port.alias, port.selector)
                {
                    if let Some(binding) = accepted.ownership.port(port) {
                        bindings.insert(binding);
                    }
                    if let Some(spans) = accepted.ownership.aggregate_spans(port) {
                        bindings.extend(spans.iter().copied().map(IntentNativeBinding::CurveSpan));
                    }
                }
            }
            ExpandedSemanticTarget::Declaration { alias, .. } => {
                let mut aliases = vec![alias];
                if materialized.base_outcome.aliases.node(alias).is_none() {
                    aliases = materialized
                        .expansion
                        .declaration_provenance
                        .iter()
                        .filter_map(|(alias, owner)| {
                            (owner == &output.reference.declaration).then_some(alias)
                        })
                        .collect();
                }
                for alias in aliases {
                    if let Some(node) = materialized.base_outcome.aliases.node(alias) {
                        if let Some(owned) = accepted.ownership.node(node) {
                            bindings.extend(&owned.owned);
                        }
                        for aggregate in &accepted.ownership.aggregates {
                            if aggregate.port.node == node {
                                bindings.extend(
                                    aggregate
                                        .spans
                                        .iter()
                                        .copied()
                                        .map(IntentNativeBinding::CurveSpan),
                                );
                            }
                        }
                    }
                }
            }
            ExpandedSemanticTarget::Collection { members } => {
                pending.extend(members.values().map(AsRef::as_ref));
            }
            ExpandedSemanticTarget::FeatureCorner { corner } => {
                for port in [&corner.point, &corner.incoming, &corner.outgoing] {
                    if let Some(binding) = materialized
                        .base_outcome
                        .aliases
                        .port(&port.alias, port.selector)
                        .and_then(|port| accepted.ownership.port(port))
                    {
                        bindings.insert(binding);
                    }
                }
            }
            ExpandedSemanticTarget::HostOutput { address, .. } => {
                if let Some(outputs) = materialized.host_outputs.get(address) {
                    for edge in accepted.computed.edges() {
                        if let ComputedEdgeProvenance::FilletArc { owner, .. } = &edge.provenance
                            && outputs.iter().any(|output| output.owner == *owner)
                        {
                            explicit_edges.insert(edge.id.ordinal);
                        }
                    }
                }
            }
        }
    }
    let mut points = BTreeSet::new();
    let mut spans = BTreeSet::new();
    for binding in bindings {
        match binding {
            IntentNativeBinding::Point(point) => {
                if native.point(point).is_some() {
                    points.insert(point);
                }
            }
            IntentNativeBinding::CurveSpan(span) => {
                if native.curve(span.curve).is_some() {
                    spans.insert(span);
                }
            }
            IntentNativeBinding::Curve(curve) => {
                if native.curve(curve).is_none() {
                    continue;
                }
                spans.extend(
                    native
                        .curve_spans(curve)
                        .map_err(|error| EngineError::Validation(error.to_string()))?,
                );
            }
            IntentNativeBinding::ComputedFeature(feature) => {
                for edge in accepted.computed.edges() {
                    if let ComputedEdgeProvenance::FilletArc { owner, .. } = edge.provenance
                        && owner.feature == feature
                    {
                        explicit_edges.insert(edge.id.ordinal);
                    }
                }
            }
            IntentNativeBinding::ComputedFeatureCorner(corner) => {
                for edge in accepted.computed.edges() {
                    if let ComputedEdgeProvenance::FilletArc { owner, .. } = edge.provenance
                        && owner.corner == corner
                    {
                        explicit_edges.insert(edge.id.ordinal);
                    }
                }
            }
            _ => {}
        }
    }
    for edge in accepted.computed.edges() {
        let selected = match &edge.provenance {
            ComputedEdgeProvenance::SourceFragment { source, .. } => spans.contains(&source.span),
            ComputedEdgeProvenance::FilletArc { sources, .. } => {
                sources.iter().all(|source| spans.contains(&source.span))
            }
            _ => false,
        };
        if selected {
            explicit_edges.insert(edge.id.ordinal);
        }
    }
    Ok(EngineOutputGeometry {
        points: points.into_iter().collect(),
        spans: spans.into_iter().collect(),
        computed_edges: explicit_edges.into_iter().collect(),
    })
}
