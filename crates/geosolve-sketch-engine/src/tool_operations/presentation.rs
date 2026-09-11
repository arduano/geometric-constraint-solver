// SPDX-License-Identifier: GPL-3.0-or-later
//! Paint-only state from the native collectors; carries no publication capability.
use super::{SelectionItem, ToolOperationPrediction, ToolOperationTool};
use geosolve_constraint_editor::{
    OffsetAuthoringOperand, OffsetAuthoringTarget, OffsetAuthoringTargetAvailability,
};
use geosolve_sketch_topology::{OffsetEndpointRole, OffsetTraversal};
use serde_json::{Value, json};

impl ToolOperationPrediction {
    pub(super) fn presentation(&self) -> Value {
        let mut pending = self
            .authoring
            .pending()
            .iter()
            .map(|operand| operand.item)
            .collect::<Vec<_>>();
        pending.extend(self.fillet.pending_items());
        let hover = self.editor.editor().hover_state();
        let offset =
            (self.command.tool == ToolOperationTool::Offset).then(|| self.offset_presentation());
        json!({
            "pending": pending,
            "provisional": self.editor.offset_authoring_provisional_items(),
            "hover": hover.target.map(geosolve_constraint_editor::EditorHoverTarget::item),
            "context_owner": hover.context_owner,
            "offset": offset,
        })
    }
    fn offset_presentation(&self) -> Value {
        fn push_target(items: &mut Vec<SelectionItem>, target: &OffsetAuthoringTarget) {
            match target {
                OffsetAuthoringTarget::Face(key) => items.extend(
                    key.outer
                        .spans
                        .iter()
                        .chain(key.holes.iter().flat_map(|hole| &hole.spans))
                        .map(|directed| SelectionItem::Curve(directed.span)),
                ),
                OffsetAuthoringTarget::Span(span) => items.push(SelectionItem::Curve(*span)),
            }
        }
        let mut pending = Vec::new();
        let mut unavailable = Vec::new();
        let mut unavailable_message = None;
        match self.offset.operand() {
            Some(OffsetAuthoringOperand::Face { key, .. }) => {
                push_target(&mut pending, &OffsetAuthoringTarget::Face(key.clone()));
            }
            Some(OffsetAuthoringOperand::OpenChain { spans, .. }) => pending.extend(
                spans
                    .iter()
                    .map(|directed| SelectionItem::Curve(directed.span)),
            ),
            None => {}
        }
        if let Some(hover) = self.offset.hover() {
            match &hover.availability {
                OffsetAuthoringTargetAvailability::Available => {
                    push_target(&mut pending, &hover.target);
                }
                OffsetAuthoringTargetAvailability::Unavailable { message, .. } => {
                    push_target(&mut unavailable, &hover.target);
                    unavailable_message = Some(message);
                }
            }
        }
        pending.sort_unstable();
        pending.dedup();
        unavailable.sort_unstable();
        unavailable.dedup();
        let chain = self.offset.chain_presentation().map(|chain| {
            let terminal = |terminal: geosolve_constraint_editor::OffsetAuthoringChainTerminal| json!({
                "span": terminal.endpoint.span,
                "endpoint": match terminal.endpoint.endpoint { OffsetEndpointRole::Start => "start", OffsetEndpointRole::End => "end" },
                "model_position": terminal.model_position,
            });
            json!({
                "spans": chain.spans.iter().map(|directed| json!({
                    "span": directed.span,
                    "traversal": match directed.traversal { OffsetTraversal::Forward => "forward", OffsetTraversal::Reverse => "reverse" },
                })).collect::<Vec<_>>(),
                "start": terminal(chain.start), "end": terminal(chain.end),
            })
        });
        json!({"pending":pending,"unavailable":unavailable,"unavailable_message":unavailable_message,"chain":chain})
    }
}
