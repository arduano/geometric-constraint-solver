// SPDX-License-Identifier: GPL-3.0-or-later

//! Deterministic, topology-preserving native geometry construction for grouped offsets.

use crate::{
    ContactNeighborhood, CurveDefinition, CurveSpan, DesignPointId, DocumentArcSweep,
    DocumentConstraintDefinition, DocumentCurveContinuity, DocumentDirectedProfileOffsetCurve,
    DocumentError, DocumentFaceOffsetDirection, DocumentLineSide, DocumentOffsetTraversal,
    DocumentProfileOffsetChain, DocumentProfileOffsetEdgePair, DocumentProfileOffsetIds,
    DocumentProfileOffsetJunction, DocumentProfileOffsetJunctionBranch,
    DocumentProfileOffsetJunctionOwner, DocumentProfileOffsetLoop, DocumentProfileOffsetOperand,
    DocumentProfileOffsetTerminalPolicy, DocumentProfileOffsetTurn, GeometryRole, ScalarDomain,
    ScalarUnit, SketchDocument,
};

const MAX_PROFILE_OFFSET_EDGES: usize = 256;
const GEOMETRY_EPSILON: f64 = 1.0e-10;

/// Source-only ordered path authenticated by the topology owner before construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DocumentProfileOffsetCreationPath {
    pub edges: Vec<DocumentDirectedProfileOffsetCurve>,
    pub junctions: Vec<DocumentProfileOffsetCreationJunction>,
}

/// Source-owned join and retained local branch used while constructing target topology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentProfileOffsetCreationJunction {
    pub source_owner: DocumentProfileOffsetJunctionOwner,
    pub branch: DocumentProfileOffsetJunctionBranch,
}

/// One exact source operand for atomic target construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DocumentProfileOffsetCreationOperand {
    Face {
        direction: DocumentFaceOffsetDirection,
        outer: DocumentProfileOffsetCreationPath,
        holes: Vec<DocumentProfileOffsetCreationPath>,
    },
    OpenChain {
        side: DocumentLineSide,
        chain: DocumentProfileOffsetCreationPath,
    },
}

/// Complete deterministic native Offset construction request.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentProfileOffsetCreationRequest {
    pub label: String,
    pub distance: f64,
    pub operand: DocumentProfileOffsetCreationOperand,
}

/// Opaque, accepted-geometry-derived construction plan for one native Profile Offset.
///
/// The plan contains only provisional target geometry and the exact retained source intent needed
/// to authenticate it again at application time. It is deliberately not persistent sketch state:
/// successful application stores ordinary target geometry plus the grouped association, while a
/// rejected or stale plan changes nothing.
#[derive(Clone, Debug, PartialEq)]
pub struct DocumentPreparedProfileOffsetGeometry {
    label: String,
    distance: f64,
    operand: PreparedCreationOperand,
}

#[derive(Clone, Debug, PartialEq)]
enum PreparedCreationOperand {
    Face {
        direction: DocumentFaceOffsetDirection,
        outer: PreparedPath,
        holes: Vec<PreparedPath>,
    },
    OpenChain {
        side: DocumentLineSide,
        chain: PreparedPath,
    },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum OffsetSupport {
    Line { point: [f64; 2], tangent: [f64; 2] },
    Circle { center: [f64; 2], radius: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum TargetFamily {
    Line,
    CircularArc {
        center: [f64; 2],
        radius: f64,
        sweep: DocumentArcSweep,
    },
    Circle {
        center: [f64; 2],
        radius: f64,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct PreparedEdge {
    source: DocumentDirectedProfileOffsetCurve,
    source_definition: CurveDefinition,
    source_role: GeometryRole,
    support: OffsetSupport,
    family: TargetFamily,
    provisional_start: [f64; 2],
    provisional_end: [f64; 2],
    source_start_tangent: [f64; 2],
    source_end_tangent: [f64; 2],
}

#[derive(Clone, Debug, PartialEq)]
struct PreparedPath {
    edges: Vec<PreparedEdge>,
    junctions: Vec<DocumentProfileOffsetCreationJunction>,
    boundaries: Vec<[f64; 2]>,
    closed: bool,
}

#[derive(Clone, Copy, Debug)]
struct TargetEndpoint {
    position: [f64; 2],
    point: Option<DesignPointId>,
}

impl TargetEndpoint {
    const fn new(position: [f64; 2], point: Option<DesignPointId>) -> Self {
        Self { position, point }
    }
}

impl SketchDocument {
    /// Prepares deterministic target seeds from this document's finite geometry without mutating
    /// it. Callers that own retained design/accepted separation should invoke this method on the
    /// independently accepted document, then apply the opaque result to the exact stamped design.
    ///
    /// # Errors
    ///
    /// Rejects unsupported/stale source spans, invalid retained junction branches, collapsed or
    /// non-intersecting supports, and non-finite geometry.
    pub fn prepare_profile_offset_geometry(
        &self,
        request: DocumentProfileOffsetCreationRequest,
    ) -> Result<DocumentPreparedProfileOffsetGeometry, DocumentError> {
        if !request.distance.is_finite() || request.distance <= 0.0 {
            return construction_error("profile offset distance", "must be finite and positive");
        }
        let operand = match &request.operand {
            DocumentProfileOffsetCreationOperand::Face {
                direction,
                outer,
                holes,
            } => {
                let sign = match direction {
                    DocumentFaceOffsetDirection::Outward => -1.0,
                    DocumentFaceOffsetDirection::Inward => 1.0,
                };
                PreparedCreationOperand::Face {
                    direction: *direction,
                    outer: self.prepare_profile_offset_path(outer, sign, request.distance, true)?,
                    holes: holes
                        .iter()
                        .map(|hole| {
                            self.prepare_profile_offset_path(hole, sign, request.distance, true)
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                }
            }
            DocumentProfileOffsetCreationOperand::OpenChain { side, chain } => {
                let sign = match side {
                    DocumentLineSide::Left => 1.0,
                    DocumentLineSide::Right => -1.0,
                };
                PreparedCreationOperand::OpenChain {
                    side: *side,
                    chain: self.prepare_profile_offset_path(
                        chain,
                        sign,
                        request.distance,
                        false,
                    )?,
                }
            }
        };
        Ok(DocumentPreparedProfileOffsetGeometry {
            label: request.label,
            distance: request.distance,
            operand,
        })
    }

    /// Creates every native target curve, ordinary endpoint topology, positive scalar, and one
    /// grouped driving dimension in one atomic document mutation.
    ///
    /// # Errors
    ///
    /// Rejects unsupported/stale source spans, invalid retained junction branches, collapsed or
    /// non-intersecting supports, non-finite geometry, and any document validation failure.
    pub fn create_profile_offset_geometry(
        &mut self,
        request: DocumentProfileOffsetCreationRequest,
    ) -> Result<DocumentProfileOffsetIds, DocumentError> {
        let prepared = self.prepare_profile_offset_geometry(request)?;
        self.create_prepared_profile_offset_geometry(prepared)
    }

    /// Applies an accepted-geometry-derived Profile Offset plan to a compatible retained design.
    /// Source identities, definitions, and profile roles are reauthenticated before allocation;
    /// the accepted seed never replaces retained source coordinates or scalar values.
    ///
    /// # Errors
    ///
    /// Rejects a stale/incompatible source, any allocation or document validation failure, and
    /// leaves this document unchanged.
    pub fn create_prepared_profile_offset_geometry(
        &mut self,
        prepared: DocumentPreparedProfileOffsetGeometry,
    ) -> Result<DocumentProfileOffsetIds, DocumentError> {
        let DocumentPreparedProfileOffsetGeometry {
            label,
            distance,
            operand: prepared_operand,
        } = prepared;
        let mut candidate = self.clone();
        let operand = match &prepared_operand {
            PreparedCreationOperand::Face {
                direction,
                outer,
                holes,
            } => {
                let outer =
                    candidate.construct_profile_offset_path(&format!("{label}.outer"), outer)?;
                let holes = holes
                    .iter()
                    .enumerate()
                    .map(|(index, hole)| {
                        candidate.construct_profile_offset_path(
                            &format!("{label}.hole_{}", index + 1),
                            hole,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                DocumentProfileOffsetOperand::Face {
                    direction: *direction,
                    outer: DocumentProfileOffsetLoop {
                        edges: outer.edges,
                        junctions: outer.junctions,
                    },
                    holes: holes
                        .into_iter()
                        .map(|path| DocumentProfileOffsetLoop {
                            edges: path.edges,
                            junctions: path.junctions,
                        })
                        .collect(),
                }
            }
            PreparedCreationOperand::OpenChain { side, chain } => {
                let chain =
                    candidate.construct_profile_offset_path(&format!("{label}.chain"), chain)?;
                DocumentProfileOffsetOperand::OpenChain {
                    side: *side,
                    chain: DocumentProfileOffsetChain {
                        edges: chain.edges,
                        junctions: chain.junctions,
                        start_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                        end_terminal: DocumentProfileOffsetTerminalPolicy::NormalTranslation,
                    },
                }
            }
        };
        let ids = candidate.add_profile_offset(label, distance, operand)?;
        candidate.validate()?;
        *self = candidate;
        Ok(ids)
    }

    fn prepare_profile_offset_path(
        &self,
        source: &DocumentProfileOffsetCreationPath,
        left_normal_sign: f64,
        distance: f64,
        closed: bool,
    ) -> Result<PreparedPath, DocumentError> {
        if source.edges.is_empty() || source.edges.len() > MAX_PROFILE_OFFSET_EDGES {
            return construction_error(
                "profile offset source path",
                "must contain 1..=256 source spans",
            );
        }
        let edges = source
            .edges
            .iter()
            .copied()
            .map(|edge| self.prepare_profile_offset_edge(edge, left_normal_sign, distance))
            .collect::<Result<Vec<_>, _>>()?;
        let periodic_circle =
            edges.len() == 1 && matches!(edges[0].family, TargetFamily::Circle { .. });
        if periodic_circle {
            if !closed || !source.junctions.is_empty() {
                return construction_error(
                    "profile offset source path",
                    "a full circle is a junction-free closed face operand",
                );
            }
            return Ok(PreparedPath {
                edges,
                junctions: Vec::new(),
                boundaries: Vec::new(),
                closed,
            });
        }
        if edges
            .iter()
            .any(|edge| matches!(edge.family, TargetFamily::Circle { .. }))
        {
            return construction_error(
                "profile offset source path",
                "a full circle cannot participate in a multi-edge or open path",
            );
        }
        let expected_junctions = if closed { edges.len() } else { edges.len() - 1 };
        if source.junctions.len() != expected_junctions {
            return construction_error(
                "profile offset source junctions",
                "junction count does not match the source path",
            );
        }
        let mut boundaries = vec![[0.0, 0.0]; edges.len() + usize::from(!closed)];
        if !closed {
            boundaries[0] = edges[0].provisional_start;
            boundaries[edges.len()] = edges[edges.len() - 1].provisional_end;
        }
        for (index, junction) in source.junctions.iter().enumerate() {
            let outgoing = (index + 1) % edges.len();
            let position = retained_junction_position(&edges[index], &edges[outgoing], junction)?;
            let boundary = if closed { outgoing } else { index + 1 };
            boundaries[boundary] = position;
        }
        if boundaries
            .iter()
            .flatten()
            .any(|coordinate| !coordinate.is_finite())
        {
            return construction_error(
                "profile offset target topology",
                "constructed junctions must remain finite",
            );
        }
        Ok(PreparedPath {
            edges,
            junctions: source.junctions.clone(),
            boundaries,
            closed,
        })
    }

    #[allow(
        clippy::too_many_lines,
        reason = "one atomic path constructor keeps seed, topology-owner, and curve allocation order explicit"
    )]
    fn construct_profile_offset_path(
        &mut self,
        label: &str,
        source: &PreparedPath,
    ) -> Result<ConstructedPath, DocumentError> {
        for edge in &source.edges {
            let retained = self.curve(edge.source.curve.curve).ok_or_else(|| {
                construction_error_value("profile offset source", "source curve is missing")
            })?;
            if retained.definition != edge.source_definition
                || self.geometry_role(edge.source.curve.curve) != Some(edge.source_role)
            {
                return construction_error(
                    "profile offset prepared source",
                    "source definition or profile role changed after preparation",
                );
            }
        }
        let periodic_circle = source.edges.len() == 1
            && matches!(source.edges[0].family, TargetFamily::Circle { .. });
        if periodic_circle {
            if !source.closed || !source.junctions.is_empty() || !source.boundaries.is_empty() {
                return construction_error(
                    "profile offset source path",
                    "a full circle is a junction-free closed face operand",
                );
            }
            let target = self.create_profile_offset_target_curve(
                &format!("{label}.edge_1"),
                &source.edges[0],
                None,
                None,
            )?;
            return Ok(ConstructedPath {
                edges: vec![DocumentProfileOffsetEdgePair {
                    source: source.edges[0].source,
                    target,
                }],
                junctions: Vec::new(),
            });
        }
        let expected_junctions = if source.closed {
            source.edges.len()
        } else {
            source.edges.len() - 1
        };
        if source.junctions.len() != expected_junctions {
            return construction_error(
                "profile offset source junctions",
                "junction count does not match the source path",
            );
        }

        if source.boundaries.len() != source.edges.len() + usize::from(!source.closed)
            || source
                .boundaries
                .iter()
                .flatten()
                .any(|coordinate| !coordinate.is_finite())
        {
            return construction_error(
                "profile offset target topology",
                "constructed junctions must remain finite",
            );
        }

        let mut boundary_needs_point = vec![false; source.boundaries.len()];
        for (index, edge) in source.edges.iter().enumerate() {
            if matches!(edge.family, TargetFamily::Line) {
                boundary_needs_point[index] = true;
                let end = if source.closed {
                    (index + 1) % source.boundaries.len()
                } else {
                    index + 1
                };
                boundary_needs_point[end] = true;
            }
        }
        let boundary_points = source
            .boundaries
            .iter()
            .zip(boundary_needs_point)
            .enumerate()
            .map(|(index, (position, needed))| {
                needed
                    .then(|| self.add_point(format!("{label}.junction_{}", index + 1), *position))
                    .transpose()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut target_curves = Vec::with_capacity(source.edges.len());
        for (index, edge) in source.edges.iter().enumerate() {
            let end_index = if source.closed {
                (index + 1) % boundary_points.len()
            } else {
                index + 1
            };
            target_curves.push(self.create_profile_offset_target_curve(
                &format!("{label}.edge_{}", index + 1),
                edge,
                Some(TargetEndpoint::new(
                    source.boundaries[index],
                    boundary_points[index],
                )),
                Some(TargetEndpoint::new(
                    source.boundaries[end_index],
                    boundary_points[end_index],
                )),
            )?);
        }

        let mut junctions = Vec::with_capacity(source.junctions.len());
        for (index, source_junction) in source.junctions.iter().copied().enumerate() {
            let outgoing = (index + 1) % source.edges.len();
            let target_owner = if matches!(source.edges[index].family, TargetFamily::Line)
                && matches!(source.edges[outgoing].family, TargetFamily::Line)
            {
                let boundary = if source.closed { outgoing } else { index + 1 };
                DocumentProfileOffsetJunctionOwner::SharedPoint(
                    boundary_points[boundary].ok_or_else(|| {
                        construction_error_value(
                            "profile offset target junction",
                            "shared line junction point is missing",
                        )
                    })?,
                )
            } else {
                self.create_profile_offset_target_junction(
                    &format!("{label}.junction_{}", index + 1),
                    target_curves[index],
                    target_curves[outgoing],
                    source_junction.branch,
                )?
            };
            junctions.push(DocumentProfileOffsetJunction {
                source_owner: source_junction.source_owner,
                target_owner,
                branch: source_junction.branch,
            });
        }

        Ok(ConstructedPath {
            edges: source
                .edges
                .iter()
                .zip(target_curves)
                .map(|(source, target)| DocumentProfileOffsetEdgePair {
                    source: source.source,
                    target,
                })
                .collect(),
            junctions,
        })
    }

    fn create_profile_offset_target_junction(
        &mut self,
        label: &str,
        incoming: DocumentDirectedProfileOffsetCurve,
        outgoing: DocumentDirectedProfileOffsetCurve,
        branch: DocumentProfileOffsetJunctionBranch,
    ) -> Result<DocumentProfileOffsetJunctionOwner, DocumentError> {
        let incoming_parameter = endpoint_parameter(incoming.traversal, false);
        let outgoing_parameter = endpoint_parameter(outgoing.traversal, true);
        let incoming_contact = self.add_curve_contact(
            format!("{label}.incoming_contact"),
            incoming.curve,
            incoming_parameter,
            0,
            endpoint_neighborhood(incoming_parameter),
            None,
        )?;
        let outgoing_contact = self.add_curve_contact(
            format!("{label}.outgoing_contact"),
            outgoing.curve,
            outgoing_parameter,
            0,
            endpoint_neighborhood(outgoing_parameter),
            None,
        )?;
        let continuity = match branch {
            DocumentProfileOffsetJunctionBranch::Miter { .. } => DocumentCurveContinuity::G0,
            DocumentProfileOffsetJunctionBranch::Tangent => DocumentCurveContinuity::G1,
        };
        let owner = self.add_constraint(
            format!("{label}.continuity"),
            DocumentConstraintDefinition::EndpointContinuity {
                first_contact: incoming_contact,
                second_contact: outgoing_contact,
                continuity,
            },
        )?;
        Ok(DocumentProfileOffsetJunctionOwner::Constraint(owner))
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed supported-family dispatcher keeps exact line, arc, and circle seeding together"
    )]
    fn prepare_profile_offset_edge(
        &self,
        source: DocumentDirectedProfileOffsetCurve,
        left_normal_sign: f64,
        distance: f64,
    ) -> Result<PreparedEdge, DocumentError> {
        let curve = self.curve(source.curve.curve).ok_or_else(|| {
            construction_error_value("profile offset source", "source curve is missing")
        })?;
        let source_definition = curve.definition.clone();
        let source_role = self.geometry_role(source.curve.curve).ok_or_else(|| {
            construction_error_value("profile offset source", "source curve role is missing")
        })?;
        match &curve.definition {
            CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => {
                let [native_start, native_end] = line_span_positions(self, source.curve)?;
                let (start, end) = match source.traversal {
                    DocumentOffsetTraversal::Forward => (native_start, native_end),
                    DocumentOffsetTraversal::Reverse => (native_end, native_start),
                };
                let tangent = direction(start, end)?;
                let displacement = scale(left_normal(tangent), left_normal_sign * distance);
                let provisional_start = add(start, displacement);
                let provisional_end = add(end, displacement);
                Ok(PreparedEdge {
                    source,
                    source_definition,
                    source_role,
                    support: OffsetSupport::Line {
                        point: provisional_start,
                        tangent,
                    },
                    family: TargetFamily::Line,
                    provisional_start,
                    provisional_end,
                    source_start_tangent: tangent,
                    source_end_tangent: tangent,
                })
            }
            CurveDefinition::CircularArc {
                center,
                radius,
                start_angle,
                end_angle,
                sweep,
            } => {
                let center = self
                    .point(*center)
                    .ok_or_else(|| {
                        construction_error_value(
                            "profile offset source arc",
                            "center point is missing",
                        )
                    })?
                    .position;
                let radius = scalar_value(self, *radius, "profile offset source arc radius")?;
                let native_start =
                    scalar_value(self, *start_angle, "profile offset source arc start")?;
                let native_end = scalar_value(self, *end_angle, "profile offset source arc end")?;
                let native_turn = match sweep {
                    DocumentArcSweep::CounterClockwise => 1.0,
                    DocumentArcSweep::Clockwise => -1.0,
                };
                let traversal = traversal_sign(source.traversal);
                let turn = native_turn * traversal;
                let target_radius = radius - left_normal_sign * turn * distance;
                if !target_radius.is_finite()
                    || target_radius <= GEOMETRY_EPSILON * self.model_scale().abs().max(1.0)
                {
                    return construction_error(
                        "profile offset target arc",
                        "offset would collapse or invert the radius",
                    );
                }
                let (start_angle, end_angle) = match source.traversal {
                    DocumentOffsetTraversal::Forward => (native_start, native_end),
                    DocumentOffsetTraversal::Reverse => (native_end, native_start),
                };
                let provisional_start = radial_point(center, target_radius, start_angle);
                let provisional_end = radial_point(center, target_radius, end_angle);
                Ok(PreparedEdge {
                    source,
                    source_definition,
                    source_role,
                    support: OffsetSupport::Circle {
                        center,
                        radius: target_radius,
                    },
                    family: TargetFamily::CircularArc {
                        center,
                        radius: target_radius,
                        sweep: *sweep,
                    },
                    provisional_start,
                    provisional_end,
                    source_start_tangent: radial_tangent(start_angle, turn),
                    source_end_tangent: radial_tangent(end_angle, turn),
                })
            }
            CurveDefinition::Circle { center, radius } => {
                let center = self
                    .point(*center)
                    .ok_or_else(|| {
                        construction_error_value(
                            "profile offset source circle",
                            "center point is missing",
                        )
                    })?
                    .position;
                let radius = scalar_value(self, *radius, "profile offset source circle radius")?;
                let turn = traversal_sign(source.traversal);
                let target_radius = radius - left_normal_sign * turn * distance;
                if !target_radius.is_finite()
                    || target_radius <= GEOMETRY_EPSILON * self.model_scale().abs().max(1.0)
                {
                    return construction_error(
                        "profile offset target circle",
                        "offset would collapse or invert the radius",
                    );
                }
                Ok(PreparedEdge {
                    source,
                    source_definition,
                    source_role,
                    support: OffsetSupport::Circle {
                        center,
                        radius: target_radius,
                    },
                    family: TargetFamily::Circle {
                        center,
                        radius: target_radius,
                    },
                    provisional_start: radial_point(center, target_radius, 0.0),
                    provisional_end: radial_point(center, target_radius, 0.0),
                    source_start_tangent: [0.0, turn],
                    source_end_tangent: [0.0, turn],
                })
            }
            CurveDefinition::Ellipse { .. }
            | CurveDefinition::EllipticalArc { .. }
            | CurveDefinition::RationalQuadraticConic { .. }
            | CurveDefinition::ParabolaSegment { .. }
            | CurveDefinition::HyperbolaSegment { .. }
            | CurveDefinition::QuadraticBezier { .. }
            | CurveDefinition::CubicBezier { .. }
            | CurveDefinition::BSpline { .. }
            | CurveDefinition::Nurbs { .. } => construction_error(
                "profile offset source",
                "curve family does not have an exact native offset",
            ),
        }
    }

    #[allow(
        clippy::too_many_lines,
        reason = "the closed supported-family allocator preserves deterministic identity order"
    )]
    fn create_profile_offset_target_curve(
        &mut self,
        label: &str,
        edge: &PreparedEdge,
        directed_start: Option<TargetEndpoint>,
        directed_end: Option<TargetEndpoint>,
    ) -> Result<DocumentDirectedProfileOffsetCurve, DocumentError> {
        let curve = match edge.family {
            TargetFamily::Line => {
                let directed_start = directed_start
                    .and_then(|endpoint| endpoint.point)
                    .ok_or_else(|| {
                        construction_error_value(
                            "profile offset target line",
                            "line start point is missing",
                        )
                    })?;
                let directed_end = directed_end
                    .and_then(|endpoint| endpoint.point)
                    .ok_or_else(|| {
                        construction_error_value(
                            "profile offset target line",
                            "line end point is missing",
                        )
                    })?;
                let (start, end) = match edge.source.traversal {
                    DocumentOffsetTraversal::Forward => (directed_start, directed_end),
                    DocumentOffsetTraversal::Reverse => (directed_end, directed_start),
                };
                let branch_direction = direction(
                    self.point(start).expect("new target start").position,
                    self.point(end).expect("new target end").position,
                )?;
                self.add_curve(
                    format!("{label}.line"),
                    CurveDefinition::Line {
                        start,
                        end,
                        branch_direction,
                    },
                )?
            }
            TargetFamily::CircularArc {
                center,
                radius,
                sweep,
            } => {
                let directed_start = directed_start
                    .ok_or_else(|| {
                        construction_error_value(
                            "profile offset target arc",
                            "arc start point is missing",
                        )
                    })?
                    .position;
                let directed_end = directed_end
                    .ok_or_else(|| {
                        construction_error_value(
                            "profile offset target arc",
                            "arc end point is missing",
                        )
                    })?
                    .position;
                let directed_angles = [
                    (directed_start[1] - center[1]).atan2(directed_start[0] - center[0]),
                    (directed_end[1] - center[1]).atan2(directed_end[0] - center[0]),
                ];
                let native_angles = match edge.source.traversal {
                    DocumentOffsetTraversal::Forward => directed_angles,
                    DocumentOffsetTraversal::Reverse => [directed_angles[1], directed_angles[0]],
                };
                let center = self.add_point(format!("{label}.center"), center)?;
                let radius = self.add_scalar(
                    format!("{label}.radius"),
                    radius,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )?;
                let start_angle = self.add_scalar(
                    format!("{label}.start_angle"),
                    native_angles[0],
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                let end_angle = self.add_scalar(
                    format!("{label}.end_angle"),
                    native_angles[1],
                    ScalarUnit::Angle,
                    ScalarDomain::Finite,
                )?;
                self.add_curve(
                    format!("{label}.arc"),
                    CurveDefinition::CircularArc {
                        center,
                        radius,
                        start_angle,
                        end_angle,
                        sweep,
                    },
                )?
            }
            TargetFamily::Circle { center, radius } => {
                let center = self.add_point(format!("{label}.center"), center)?;
                let radius = self.add_scalar(
                    format!("{label}.radius"),
                    radius,
                    ScalarUnit::Length,
                    ScalarDomain::Positive,
                )?;
                self.add_curve(
                    format!("{label}.circle"),
                    CurveDefinition::Circle { center, radius },
                )?
            }
        };
        Ok(DocumentDirectedProfileOffsetCurve {
            curve: CurveSpan::line(curve),
            traversal: edge.source.traversal,
        })
    }
}

#[derive(Clone, Debug)]
struct ConstructedPath {
    edges: Vec<DocumentProfileOffsetEdgePair>,
    junctions: Vec<DocumentProfileOffsetJunction>,
}

fn retained_junction_position(
    incoming: &PreparedEdge,
    outgoing: &PreparedEdge,
    junction: &DocumentProfileOffsetCreationJunction,
) -> Result<[f64; 2], DocumentError> {
    let cross_value = cross(incoming.source_end_tangent, outgoing.source_start_tangent);
    let alignment = dot(incoming.source_end_tangent, outgoing.source_start_tangent);
    let scale = incoming
        .provisional_end
        .into_iter()
        .chain(outgoing.provisional_start)
        .fold(1.0_f64, |value, coordinate| value.max(coordinate.abs()));
    let angular_tolerance = 1024.0 * f64::EPSILON * scale;
    match junction.branch {
        DocumentProfileOffsetJunctionBranch::Tangent => {
            if alignment <= 0.0 || cross_value.abs() > angular_tolerance.max(GEOMETRY_EPSILON) {
                return construction_error(
                    "profile offset tangent junction",
                    "source tangents left the retained aligned branch",
                );
            }
            let point = midpoint(incoming.provisional_end, outgoing.provisional_start);
            if support_distance(incoming.support, point) > GEOMETRY_EPSILON * scale
                || support_distance(outgoing.support, point) > GEOMETRY_EPSILON * scale
            {
                return construction_error(
                    "profile offset tangent junction",
                    "offset supports no longer share the retained tangent contact",
                );
            }
            Ok(point)
        }
        DocumentProfileOffsetJunctionBranch::Miter { turn } => {
            let expected_sign = match turn {
                DocumentProfileOffsetTurn::Left => 1.0,
                DocumentProfileOffsetTurn::Right => -1.0,
            };
            if cross_value * expected_sign <= GEOMETRY_EPSILON {
                return construction_error(
                    "profile offset miter junction",
                    "source tangents crossed the retained turn branch",
                );
            }
            let expected = midpoint(incoming.provisional_end, outgoing.provisional_start);
            support_intersections(incoming.support, outgoing.support)
                .into_iter()
                .min_by(|first, second| {
                    squared_distance(*first, expected)
                        .total_cmp(&squared_distance(*second, expected))
                })
                .ok_or_else(|| {
                    construction_error_value(
                        "profile offset miter junction",
                        "offset supports do not intersect in the retained topology cell",
                    )
                })
        }
    }
}

fn support_intersections(first: OffsetSupport, second: OffsetSupport) -> Vec<[f64; 2]> {
    match (first, second) {
        (
            OffsetSupport::Line {
                point: first_point,
                tangent: first_tangent,
            },
            OffsetSupport::Line {
                point: second_point,
                tangent: second_tangent,
            },
        ) => {
            let denominator = cross(first_tangent, second_tangent);
            if denominator.abs() <= GEOMETRY_EPSILON {
                return Vec::new();
            }
            let parameter = cross(sub(second_point, first_point), second_tangent) / denominator;
            vec![add(first_point, scale(first_tangent, parameter))]
        }
        (OffsetSupport::Line { point, tangent }, OffsetSupport::Circle { center, radius })
        | (OffsetSupport::Circle { center, radius }, OffsetSupport::Line { point, tangent }) => {
            let relative = sub(point, center);
            let projection = dot(relative, tangent);
            let constant = dot(relative, relative) - radius * radius;
            let discriminant = projection.mul_add(projection, -constant);
            if discriminant < -GEOMETRY_EPSILON {
                return Vec::new();
            }
            let root = discriminant.max(0.0).sqrt();
            let mut points = vec![add(point, scale(tangent, -projection + root))];
            if root > GEOMETRY_EPSILON {
                points.push(add(point, scale(tangent, -projection - root)));
            }
            points
        }
        (
            OffsetSupport::Circle {
                center: first_center,
                radius: first_radius,
            },
            OffsetSupport::Circle {
                center: second_center,
                radius: second_radius,
            },
        ) => {
            let centers = sub(second_center, first_center);
            let distance = centers[0].hypot(centers[1]);
            if distance <= GEOMETRY_EPSILON
                || distance > first_radius + second_radius + GEOMETRY_EPSILON
                || distance < (first_radius - second_radius).abs() - GEOMETRY_EPSILON
            {
                return Vec::new();
            }
            let axis = [centers[0] / distance, centers[1] / distance];
            let axial = (first_radius * first_radius - second_radius * second_radius
                + distance * distance)
                / (2.0 * distance);
            let height_squared = first_radius * first_radius - axial * axial;
            if height_squared < -GEOMETRY_EPSILON {
                return Vec::new();
            }
            let base = add(first_center, scale(axis, axial));
            let height = height_squared.max(0.0).sqrt();
            let normal = left_normal(axis);
            let mut points = vec![add(base, scale(normal, height))];
            if height > GEOMETRY_EPSILON {
                points.push(add(base, scale(normal, -height)));
            }
            points
        }
    }
}

fn support_distance(support: OffsetSupport, point: [f64; 2]) -> f64 {
    match support {
        OffsetSupport::Line {
            point: origin,
            tangent,
        } => cross(tangent, sub(point, origin)).abs(),
        OffsetSupport::Circle { center, radius } => {
            (sub(point, center)[0].hypot(sub(point, center)[1]) - radius).abs()
        }
    }
}

fn line_span_positions(
    document: &SketchDocument,
    span: CurveSpan,
) -> Result<[[f64; 2]; 2], DocumentError> {
    let curve = document.curve(span.curve).ok_or_else(|| {
        construction_error_value("profile offset line source", "curve is missing")
    })?;
    let [start, end] = match &curve.definition {
        CurveDefinition::Line { start, end, .. } if span.segment == 0 => [*start, *end],
        CurveDefinition::Polyline { points, closed, .. } => {
            let index = usize::try_from(span.segment).map_err(|_| {
                construction_error_value(
                    "profile offset polyline span",
                    "segment index is out of range",
                )
            })?;
            let next = index + 1;
            if index >= points.len()
                || (next >= points.len() && !(*closed && index + 1 == points.len()))
            {
                return construction_error(
                    "profile offset polyline span",
                    "segment index is out of range",
                );
            }
            [points[index], points[next % points.len()]]
        }
        _ => {
            return construction_error(
                "profile offset line source",
                "span is not a complete line or polyline segment",
            );
        }
    };
    Ok([
        document
            .point(start)
            .ok_or_else(|| {
                construction_error_value("profile offset line source", "start point is missing")
            })?
            .position,
        document
            .point(end)
            .ok_or_else(|| {
                construction_error_value("profile offset line source", "end point is missing")
            })?
            .position,
    ])
}

fn scalar_value(
    document: &SketchDocument,
    scalar: crate::DesignScalarId,
    field: &'static str,
) -> Result<f64, DocumentError> {
    let value = document
        .scalar(scalar)
        .ok_or_else(|| construction_error_value(field, "scalar is missing"))?
        .value;
    if value.is_finite() {
        Ok(value)
    } else {
        construction_error(field, "scalar must be finite")
    }
}

fn direction(start: [f64; 2], end: [f64; 2]) -> Result<[f64; 2], DocumentError> {
    let value = sub(end, start);
    let length = value[0].hypot(value[1]);
    if !length.is_finite() || length <= GEOMETRY_EPSILON {
        construction_error("profile offset support", "source or target span collapsed")
    } else {
        Ok([value[0] / length, value[1] / length])
    }
}

const fn traversal_sign(traversal: DocumentOffsetTraversal) -> f64 {
    match traversal {
        DocumentOffsetTraversal::Forward => 1.0,
        DocumentOffsetTraversal::Reverse => -1.0,
    }
}

const fn endpoint_parameter(traversal: DocumentOffsetTraversal, directed_start: bool) -> f64 {
    match (traversal, directed_start) {
        (DocumentOffsetTraversal::Forward, true) | (DocumentOffsetTraversal::Reverse, false) => 0.0,
        (DocumentOffsetTraversal::Forward, false) | (DocumentOffsetTraversal::Reverse, true) => 1.0,
    }
}

const fn endpoint_neighborhood(parameter: f64) -> ContactNeighborhood {
    if parameter == 0.0 {
        ContactNeighborhood::Start
    } else {
        ContactNeighborhood::End
    }
}

fn radial_point(center: [f64; 2], radius: f64, angle: f64) -> [f64; 2] {
    [
        center[0] + radius * angle.cos(),
        center[1] + radius * angle.sin(),
    ]
}

fn radial_tangent(angle: f64, turn: f64) -> [f64; 2] {
    [-angle.sin() * turn, angle.cos() * turn]
}

const fn left_normal(value: [f64; 2]) -> [f64; 2] {
    [-value[1], value[0]]
}

const fn add(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] + second[0], first[1] + second[1]]
}

const fn sub(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [first[0] - second[0], first[1] - second[1]]
}

const fn scale(value: [f64; 2], factor: f64) -> [f64; 2] {
    [value[0] * factor, value[1] * factor]
}

fn midpoint(first: [f64; 2], second: [f64; 2]) -> [f64; 2] {
    [(first[0] + second[0]) * 0.5, (first[1] + second[1]) * 0.5]
}

fn dot(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[0], first[1] * second[1])
}

fn cross(first: [f64; 2], second: [f64; 2]) -> f64 {
    first[0].mul_add(second[1], -first[1] * second[0])
}

fn squared_distance(first: [f64; 2], second: [f64; 2]) -> f64 {
    let delta = sub(first, second);
    delta[0].mul_add(delta[0], delta[1] * delta[1])
}

fn construction_error<T>(field: &'static str, message: &'static str) -> Result<T, DocumentError> {
    Err(construction_error_value(field, message))
}

fn construction_error_value(field: &'static str, message: &'static str) -> DocumentError {
    DocumentError::InvalidField {
        field,
        message: message.into(),
    }
}
