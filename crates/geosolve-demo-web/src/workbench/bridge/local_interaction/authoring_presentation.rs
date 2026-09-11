// SPDX-License-Identifier: GPL-3.0-or-later
//! Typed paint-only operation state in the prediction scene's native namespace.
use geosolve_constraint_editor::{
    EditorHoverState, EditorHoverTarget, OffsetAuthoringChainPresentation,
    OffsetAuthoringChainTerminal, SelectionItem,
};
use geosolve_sketch::CurveSpan;
use geosolve_sketch_topology::{
    OffsetDirectedSpan, OffsetEndpointRef, OffsetEndpointRole, OffsetTraversal,
};
use serde::Deserialize;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct OperationPresentation {
    pub pending: Vec<SelectionItem>,
    pub provisional: Vec<SelectionItem>,
    hover: Option<SelectionItem>,
    context_owner: Option<SelectionItem>,
    offset: Option<OffsetPresentation>,
}
impl OperationPresentation {
    pub fn hover(&self) -> EditorHoverState {
        EditorHoverState {
            target: self.hover.map(EditorHoverTarget::Geometry),
            context_owner: self.context_owner,
        }
    }
    pub fn offset(&self) -> Option<geosolve_sketch_render::OffsetCanvasPresentation> {
        self.offset
            .as_ref()
            .map(|offset| geosolve_sketch_render::OffsetCanvasPresentation {
                pending: offset.pending.clone(),
                unavailable: offset.unavailable.clone(),
                unavailable_message: offset.unavailable_message.clone(),
                chain: offset.chain.as_ref().map(Chain::native),
            })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OffsetPresentation {
    pending: Vec<SelectionItem>,
    unavailable: Vec<SelectionItem>,
    unavailable_message: Option<String>,
    chain: Option<Chain>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Chain {
    spans: Vec<DirectedSpan>,
    start: Terminal,
    end: Terminal,
}
impl Chain {
    fn native(&self) -> OffsetAuthoringChainPresentation {
        OffsetAuthoringChainPresentation {
            spans: self
                .spans
                .iter()
                .map(|span| OffsetDirectedSpan {
                    span: span.span,
                    traversal: match span.traversal {
                        Traversal::Forward => OffsetTraversal::Forward,
                        Traversal::Reverse => OffsetTraversal::Reverse,
                    },
                })
                .collect(),
            start: self.start.native(),
            end: self.end.native(),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DirectedSpan {
    span: CurveSpan,
    traversal: Traversal,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Traversal {
    Forward,
    Reverse,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Terminal {
    span: CurveSpan,
    endpoint: Endpoint,
    model_position: [f64; 2],
}
impl Terminal {
    fn native(&self) -> OffsetAuthoringChainTerminal {
        OffsetAuthoringChainTerminal {
            endpoint: OffsetEndpointRef {
                span: self.span,
                endpoint: match self.endpoint {
                    Endpoint::Start => OffsetEndpointRole::Start,
                    Endpoint::End => OffsetEndpointRole::End,
                },
            },
            model_position: self.model_position,
        }
    }
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Endpoint {
    Start,
    End,
}
