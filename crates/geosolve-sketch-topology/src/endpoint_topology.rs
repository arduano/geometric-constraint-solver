// SPDX-License-Identifier: GPL-3.0-or-later

//! Exact accepted-input-stamped native endpoint connectivity.
//!
//! This query intentionally does not run visual-profile arrangement. Endpoint
//! connectivity is owned only by shared persistent points or active native
//! endpoint constraints, so unrelated intersections cannot invalidate it.

use geosolve_sketch::{
    CurveSpan, PreparedSketchInput, RetainedSketchDocumentSession, SketchAcceptedStateIdentity,
};
use thiserror::Error;

use crate::TopologySnapshot;
use crate::offset_operands::{
    EndpointTopologyBuildError, EndpointTopologyBuildLimits, OffsetEndpointAdjacency,
    OffsetEndpointCandidate, OffsetEndpointRef, build_endpoint_topology_catalog,
};

/// Deterministic fail-closed output limits for exact endpoint topology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[allow(
    clippy::struct_field_names,
    reason = "public limit names stay explicit at construction sites"
)]
pub struct EndpointTopologyRequest {
    /// Maximum active persistent spans admitted to one complete index.
    pub max_spans: usize,
    /// Maximum unique pairwise endpoint adjacencies admitted to one complete index.
    pub max_adjacencies: usize,
    /// Maximum shared-point or active-constraint link attempts before failing closed.
    pub max_connection_attempts: usize,
}

impl Default for EndpointTopologyRequest {
    fn default() -> Self {
        Self {
            max_spans: 100_000,
            max_adjacencies: 100_000,
            max_connection_attempts: 1_000_000,
        }
    }
}

impl EndpointTopologyRequest {
    fn validate(self) -> Result<(), EndpointTopologyError> {
        for (field, value) in [
            ("max_spans", self.max_spans),
            ("max_adjacencies", self.max_adjacencies),
            ("max_connection_attempts", self.max_connection_attempts),
        ] {
            if value == 0 {
                return Err(EndpointTopologyError::InvalidRequest {
                    field,
                    message: "must be positive",
                });
            }
        }
        Ok(())
    }
}

/// Exact native endpoint evidence for one active persistent curve span.
///
/// Lines, Polyline spans and circular arcs expose bounded endpoints. A Circle
/// is marked periodic and has no bounded endpoints; unsupported curve families
/// remain enumerable with no endpoint claims.
#[derive(Clone, Debug, PartialEq)]
pub struct EndpointTopologySpan {
    /// Stable complete native span.
    pub span: CurveSpan,
    /// Whether this span is a supported full periodic Circle.
    pub periodic: bool,
    /// Exact supported bounded endpoints and their owned adjacency degree.
    pub endpoints: Vec<OffsetEndpointCandidate>,
}

/// Exact endpoint connectivity for one independently accepted prepared input.
#[derive(Clone, Debug, PartialEq)]
pub struct EndpointTopologyIndex {
    input: PreparedSketchInput,
    accepted: SketchAcceptedStateIdentity,
    spans: Vec<EndpointTopologySpan>,
    adjacencies: Vec<OffsetEndpointAdjacency>,
}

impl EndpointTopologyIndex {
    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.input
    }

    #[must_use]
    pub const fn accepted_state_identity(&self) -> SketchAcceptedStateIdentity {
        self.accepted
    }

    #[must_use]
    pub fn spans(&self) -> &[EndpointTopologySpan] {
        &self.spans
    }

    #[must_use]
    pub fn adjacencies(&self) -> &[OffsetEndpointAdjacency] {
        &self.adjacencies
    }

    #[must_use]
    pub fn span(&self, span: CurveSpan) -> Option<&EndpointTopologySpan> {
        self.spans
            .binary_search_by_key(&span, |candidate| candidate.span)
            .ok()
            .map(|index| &self.spans[index])
    }

    /// Returns exact semantic neighbours of one endpoint in canonical order.
    pub fn adjacent_endpoints(
        &self,
        endpoint: OffsetEndpointRef,
    ) -> impl Iterator<Item = OffsetEndpointRef> + '_ {
        self.adjacencies.iter().filter_map(move |adjacency| {
            if adjacency.endpoints[0] == endpoint {
                Some(adjacency.endpoints[1])
            } else if adjacency.endpoints[1] == endpoint {
                Some(adjacency.endpoints[0])
            } else {
                None
            }
        })
    }

    /// Revalidates exact live-session provenance before native consumption.
    ///
    /// # Errors
    ///
    /// Rejects any newer design, attempt, accepted state, parameter, activation
    /// or external-snapshot input.
    pub fn validate_current(
        &self,
        session: &RetainedSketchDocumentSession,
    ) -> Result<(), EndpointTopologyConsumptionError> {
        let accepted = session
            .accepted_state_for_current_input()
            .ok_or(EndpointTopologyConsumptionError::Stale)?;
        if session.prepared_input() != self.input
            || accepted.identity() != self.accepted
            || accepted.design_identity() != session.design_identity()
        {
            return Err(EndpointTopologyConsumptionError::Stale);
        }
        Ok(())
    }
}

/// An exact endpoint index is stale after any accepted-input transition.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum EndpointTopologyConsumptionError {
    #[error("endpoint topology is stale for the current sketch input")]
    Stale,
}

/// Fail-closed endpoint-topology request or output-bound failure.
#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum EndpointTopologyError {
    #[error("invalid endpoint topology {field}: {message}")]
    InvalidRequest {
        field: &'static str,
        message: &'static str,
    },
    #[error("endpoint topology {resource} requires {actual} entries but the limit is {limit}")]
    ResourceLimit {
        resource: &'static str,
        actual: usize,
        limit: usize,
    },
}

/// Worker-movable immutable exact endpoint-topology query.
#[derive(Debug)]
pub struct PreparedEndpointTopologyQuery {
    snapshot: TopologySnapshot,
    request: EndpointTopologyRequest,
}

impl PreparedEndpointTopologyQuery {
    /// Captures one current independently accepted endpoint-topology input.
    ///
    /// # Errors
    ///
    /// Rejects absent, historical or input-mismatched accepted state.
    pub fn capture(
        session: &RetainedSketchDocumentSession,
        request: EndpointTopologyRequest,
    ) -> Result<Self, crate::TopologySnapshotError> {
        Ok(TopologySnapshot::capture(session)?.prepare_endpoint_topology(request))
    }

    #[must_use]
    pub const fn input(&self) -> PreparedSketchInput {
        self.snapshot.input
    }

    #[must_use]
    pub const fn request(&self) -> EndpointTopologyRequest {
        self.request
    }

    /// Builds exact endpoint ownership without visual-profile arrangement.
    ///
    /// # Errors
    ///
    /// Rejects invalid zero limits and any span, adjacency or connection-work
    /// count that would exceed the declared fail-closed request.
    pub fn execute(self) -> Result<EndpointTopologyIndex, EndpointTopologyError> {
        self.request.validate()?;
        let catalog = build_endpoint_topology_catalog(
            &self.snapshot.document,
            Some(EndpointTopologyBuildLimits {
                max_spans: self.request.max_spans,
                max_adjacencies: self.request.max_adjacencies,
                max_connection_attempts: self.request.max_connection_attempts,
            }),
        )
        .map_err(|error| match error {
            EndpointTopologyBuildError::ResourceLimit {
                resource,
                actual,
                limit,
            } => EndpointTopologyError::ResourceLimit {
                resource,
                actual,
                limit,
            },
        })?;
        Ok(EndpointTopologyIndex {
            input: self.snapshot.input,
            accepted: self.snapshot.accepted,
            spans: catalog
                .spans
                .into_iter()
                .map(|span| EndpointTopologySpan {
                    span: span.span,
                    periodic: span.periodic,
                    endpoints: span.endpoints,
                })
                .collect(),
            adjacencies: catalog.adjacencies,
        })
    }
}

impl TopologySnapshot {
    /// Prepares exact accepted native endpoint connectivity from this snapshot.
    #[must_use]
    pub fn prepare_endpoint_topology(
        self,
        request: EndpointTopologyRequest,
    ) -> PreparedEndpointTopologyQuery {
        PreparedEndpointTopologyQuery {
            snapshot: self,
            request,
        }
    }
}
