// SPDX-License-Identifier: GPL-3.0-or-later
//! Source-owned selection translation; detached presentation grants no edit authority.
use crate::{EditableSession, EngineError, ToolOperationOperand};
use geosolve_constraint_editor::{
    IntentNativeBinding, ProjectionalEditorSession, SceneCurveOrigin, SelectionItem,
    SelectionPresentationState, Viewport,
};
use geosolve_sketch::{CurveSpan, DocumentFilletTrimEndpoint};
use geosolve_sketch_intent::IntentKey;
use serde::{Deserialize, Serialize};

fn error(value: impl std::fmt::Display) -> EngineError {
    EngineError::Admission(value.to_string())
}
fn viewport() -> Viewport {
    Viewport {
        screen_size: [800.0, 600.0],
        model_center: [0.0, 0.0],
        pixels_per_model_unit: 1.0,
    }
}

/// Stable ownership of one native binding, independent of document allocators.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolSelectionBinding {
    pub symbol: IntentKey,
    pub binding: usize,
}
/// Exact selected visible occurrence, including a computed discarded interval.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ToolCurveOccurrence {
    Native,
    FilletDiscarded {
        source: ToolSelectionBinding,
        segment: u32,
        interval: [f64; 2],
        feature: ToolSelectionBinding,
        corner: ToolSelectionBinding,
        endpoint: DocumentFilletTrimEndpoint,
        base_interval: [f64; 2],
    },
}
fn binding_for(
    editor: &ProjectionalEditorSession,
    target: IntentNativeBinding,
) -> Result<ToolSelectionBinding, EngineError> {
    let bindings = editor
        .presentation_bindings()
        .ok_or_else(|| error("missing native source bindings"))?;
    let found = bindings
        .nodes
        .iter()
        .flat_map(|(symbol, values)| {
            values.iter().enumerate().filter_map(|(binding, value)| {
                (*value == target).then_some(ToolSelectionBinding {
                    symbol: symbol.clone(),
                    binding,
                })
            })
        })
        .collect::<Vec<_>>();
    if let [binding] = found.as_slice() {
        Ok(binding.clone())
    } else {
        Err(error("selected occurrence requires one exact source owner"))
    }
}
fn occurrence_for(
    editor: &ProjectionalEditorSession,
    origin: SceneCurveOrigin,
) -> Result<ToolCurveOccurrence, EngineError> {
    Ok(match origin {
        SceneCurveOrigin::Native => ToolCurveOccurrence::Native,
        SceneCurveOrigin::FilletDiscarded {
            source,
            interval,
            provenance,
            ..
        } => ToolCurveOccurrence::FilletDiscarded {
            source: binding_for(editor, IntentNativeBinding::Curve(source.span.curve))?,
            segment: source.span.segment,
            interval: [interval.start, interval.end],
            feature: binding_for(
                editor,
                IntentNativeBinding::ComputedFeature(provenance.owner.feature),
            )?,
            corner: binding_for(
                editor,
                IntentNativeBinding::ComputedFeatureCorner(provenance.owner.corner),
            )?,
            endpoint: provenance.endpoint,
            base_interval: [provenance.base_interval.start, provenance.base_interval.end],
        },
    })
}
pub(super) fn validate_occurrence(
    editor: &ProjectionalEditorSession,
    span: CurveSpan,
    parameter: Option<f64>,
    occurrence: &ToolCurveOccurrence,
) -> Result<(), EngineError> {
    let parameter = parameter
        .filter(|value| value.is_finite())
        .ok_or_else(|| error("selected occurrence requires a finite picked parameter"))?;
    let scene = editor.scene(viewport(), 0.25).map_err(error)?;
    let mut matches = Vec::new();
    for curve in &scene.curves {
        if curve.span == span
            && occurrence_for(editor, curve.origin).is_ok_and(|candidate| candidate == *occurrence)
        {
            let selection = SelectionPresentationState {
                items: vec![SelectionItem::Curve(span)],
                curve_picks: vec![geosolve_constraint_editor::CurvePickContext {
                    span,
                    parameter,
                    origin: curve.origin,
                }],
            };
            if selection.validate(&scene).is_ok() {
                matches.push(curve.origin);
            }
        }
    }
    if matches.len() != 1 {
        return Err(error(
            "selected native occurrence is absent, changed or ambiguous",
        ));
    }
    Ok(())
}
impl EditableSession {
    /// Exact accepted scene and source bindings for personal selection translation.
    /// # Errors
    /// Rejects an invalid viewport or unavailable accepted presentation.
    pub fn tool_operation_presentation_json(
        &self,
        viewport: Viewport,
    ) -> Result<String, EngineError> {
        let editor = &self.accepted().0.materialized.editor;
        let scene = editor
            .scene(viewport, 0.25)
            .map_err(error)?
            .to_detached_json()
            .map_err(error)?;
        let bindings = editor
            .presentation_bindings()
            .ok_or_else(|| error("missing accepted source bindings"))?;
        serde_json::to_string(&serde_json::json!({"scene":scene,"bindings":bindings}))
            .map_err(error)
    }
    /// Resolve accepted personal selection to stable semantic operands.
    ///
    /// # Errors
    /// Rejects foreign scenes, invalid occurrences or ambiguous native ownership.
    pub fn tool_operation_view_operands(
        &self,
        viewport: Viewport,
        view: geosolve_constraint_editor::DetachedSelectionView,
    ) -> Result<Vec<ToolOperationOperand>, EngineError> {
        let editor = &self.accepted().0.materialized.editor;
        let scene = editor.scene(viewport, 0.25).map_err(error)?;
        let selection = view
            .map_to(&scene, editor.presentation_bindings().as_ref())
            .map_err(error)?;
        self.tool_operation_operands(&selection)
    }

    /// Validate exact native selection and return stable semantic operands.
    /// # Errors
    /// Rejects stale namespaces, ambiguous owners and invalid picked occurrences.
    pub fn tool_operation_operands(
        &self,
        selection: &SelectionPresentationState,
    ) -> Result<Vec<ToolOperationOperand>, EngineError> {
        if selection.items.len() > 1024 {
            return Err(error("tool selection exceeds 1024 operands"));
        }
        let editor = &self.accepted().0.materialized.editor;
        let scene = editor.scene(viewport(), 0.25).map_err(error)?;
        selection.validate(&scene).map_err(error)?;
        selection
            .items
            .iter()
            .map(|item| {
                let picked = selection
                    .curve_picks
                    .iter()
                    .find(|pick| *item == SelectionItem::Curve(pick.span));
                let mut operand =
                    self.tool_operation_operand(*item, picked.map(|pick| pick.parameter))?;
                if let ToolOperationOperand::Binding { occurrence, .. } = &mut operand {
                    *occurrence = picked
                        .map(|pick| occurrence_for(editor, pick.origin))
                        .transpose()?;
                }
                Ok(operand)
            })
            .collect()
    }
}

pub(super) fn required_declarations(
    session: &EditableSession,
    command: &crate::ToolOperationCommand,
) -> Result<Vec<geosolve_sketch_code::SemanticSymbol>, EngineError> {
    let mut symbols = std::collections::BTreeSet::new();
    let mut inspect = |operand: &ToolOperationOperand| {
        if let ToolOperationOperand::Binding {
            symbol, occurrence, ..
        } = operand
        {
            symbols.insert(symbol.clone());
            if let Some(ToolCurveOccurrence::FilletDiscarded {
                source,
                feature,
                corner,
                ..
            }) = occurrence
            {
                for owner in [source, feature, corner] {
                    symbols.insert(owner.symbol.clone());
                }
            }
        }
    };
    for operand in &command.selection {
        inspect(operand);
    }
    for sample in &command.samples {
        match &sample.input {
            crate::ToolOperationEvent::Pick { operand } => inspect(operand),
            crate::ToolOperationEvent::PickSelection { operands } => {
                for operand in operands {
                    inspect(operand);
                }
            }
            _ => {}
        }
    }
    let expansion = &session.accepted().0.materialized.expansion;
    symbols
        .into_iter()
        .map(|symbol| {
            expansion
                .declaration_provenance
                .get(&symbol)
                .cloned()
                .ok_or_else(|| error("tool selection has no exact source lifetime owner"))
        })
        .collect()
}
