use std::collections::BTreeSet;

use super::{
    ContactDomain, ContactId, ContactNeighborhood, CurveDefinition, CurveSpan, DesignPointId,
    DocumentConstraintDefinition, DocumentDirectedProfileOffsetCurve, DocumentError,
    DocumentOffsetTraversal, DocumentProfileOffsetEdgePair, DocumentProfileOffsetJunction,
    DocumentProfileOffsetJunctionOwner, DocumentProfileOffsetOperand, FeatureEndpoint,
    GeometryRole, SketchDocument, constraint_contacts, invalid, unknown,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProfileOffsetCurveFamily {
    Line,
    CircularArc,
    Circle,
}

const fn profile_offset_endpoint_parameter(traversal: DocumentOffsetTraversal, start: bool) -> f64 {
    match (traversal, start) {
        (DocumentOffsetTraversal::Forward, true) | (DocumentOffsetTraversal::Reverse, false) => 0.0,
        (DocumentOffsetTraversal::Forward, false) | (DocumentOffsetTraversal::Reverse, true) => 1.0,
    }
}

impl SketchDocument {
    pub(super) fn validate_profile_offset_operand(
        &self,
        operand: &DocumentProfileOffsetOperand,
    ) -> Result<(), DocumentError> {
        let mut used = BTreeSet::new();
        match operand {
            DocumentProfileOffsetOperand::Face { outer, holes, .. } => {
                self.validate_profile_offset_path(&outer.edges, &outer.junctions, true, &mut used)?;
                for hole in holes {
                    self.validate_profile_offset_path(
                        &hole.edges,
                        &hole.junctions,
                        true,
                        &mut used,
                    )?;
                }
            }
            DocumentProfileOffsetOperand::OpenChain { chain, .. } => {
                self.validate_profile_offset_path(
                    &chain.edges,
                    &chain.junctions,
                    false,
                    &mut used,
                )?;
            }
        }
        Ok(())
    }

    fn validate_profile_offset_path(
        &self,
        edges: &[DocumentProfileOffsetEdgePair],
        junctions: &[DocumentProfileOffsetJunction],
        closed: bool,
        used: &mut BTreeSet<CurveSpan>,
    ) -> Result<(), DocumentError> {
        if edges.is_empty() {
            return invalid(
                "profile offset topology",
                "an operand path must contain at least one edge",
            );
        }
        let first_family = self.profile_offset_curve_family(edges[0].source.curve)?;
        let periodic_circle = edges.len() == 1 && first_family == ProfileOffsetCurveFamily::Circle;
        if periodic_circle && !closed {
            return invalid(
                "profile offset topology",
                "a full circle is available only as a closed face operand",
            );
        }
        let expected_junctions = if periodic_circle {
            0
        } else if closed {
            edges.len()
        } else {
            edges.len() - 1
        };
        if junctions.len() != expected_junctions {
            return invalid(
                "profile offset topology",
                "junction count does not match the ordered path",
            );
        }
        if !periodic_circle && edges.len() == 1 && closed {
            return invalid(
                "profile offset topology",
                "a one-edge closed loop must be a full circle",
            );
        }
        for edge in edges {
            let source_family = self.profile_offset_curve_family(edge.source.curve)?;
            let target_family = self.profile_offset_curve_family(edge.target.curve)?;
            if self.geometry_role(edge.source.curve.curve) != Some(GeometryRole::Profile)
                || self.geometry_role(edge.target.curve.curve) != Some(GeometryRole::Profile)
            {
                return invalid(
                    "profile offset geometry role",
                    "source and target supports must remain Profile geometry",
                );
            }
            if source_family != target_family {
                return invalid(
                    "profile offset edge pair",
                    "source and target supports must use the same exact curve family",
                );
            }
            if edge.source.curve == edge.target.curve
                || !used.insert(edge.source.curve)
                || !used.insert(edge.target.curve)
            {
                return invalid(
                    "profile offset edge pair",
                    "every source and target support must be distinct and occur exactly once",
                );
            }
            if source_family == ProfileOffsetCurveFamily::Circle && !periodic_circle {
                return invalid(
                    "profile offset topology",
                    "a full circle must be the only edge in its periodic path",
                );
            }
        }
        for (index, junction) in junctions.iter().enumerate() {
            let incoming = edges[index];
            let outgoing = edges[(index + 1) % edges.len()];
            self.validate_profile_offset_junction_owner(
                junction.source_owner,
                incoming.source,
                outgoing.source,
            )?;
            self.validate_profile_offset_junction_owner(
                junction.target_owner,
                incoming.target,
                outgoing.target,
            )?;
        }
        Ok(())
    }

    fn profile_offset_curve_family(
        &self,
        span: CurveSpan,
    ) -> Result<ProfileOffsetCurveFamily, DocumentError> {
        let curve = self.validate_span(span)?;
        Ok(match curve.definition {
            CurveDefinition::Line { .. } | CurveDefinition::Polyline { .. } => {
                ProfileOffsetCurveFamily::Line
            }
            CurveDefinition::CircularArc { .. } => ProfileOffsetCurveFamily::CircularArc,
            CurveDefinition::Circle { .. } => ProfileOffsetCurveFamily::Circle,
            _ => {
                return invalid(
                    "profile offset support",
                    "only lines, circular arcs, and full circles are supported",
                );
            }
        })
    }

    fn validate_profile_offset_junction_owner(
        &self,
        owner: DocumentProfileOffsetJunctionOwner,
        incoming: DocumentDirectedProfileOffsetCurve,
        outgoing: DocumentDirectedProfileOffsetCurve,
    ) -> Result<(), DocumentError> {
        match owner {
            DocumentProfileOffsetJunctionOwner::SharedPoint(point) => {
                self.require_point(point)?;
                let incoming_end = self.profile_offset_line_endpoint(incoming, false)?;
                let outgoing_start = self.profile_offset_line_endpoint(outgoing, true)?;
                if incoming_end != point || outgoing_start != point {
                    return invalid(
                        "profile offset junction owner",
                        "the retained shared point must own both directed line endpoints",
                    );
                }
            }
            DocumentProfileOffsetJunctionOwner::Constraint(owner) => {
                let constraint = self
                    .constraint(owner)
                    .ok_or_else(|| unknown("profile offset junction constraint", owner.0))?;
                let expected = [
                    (
                        incoming,
                        false,
                        profile_offset_endpoint_parameter(incoming.traversal, false),
                    ),
                    (
                        outgoing,
                        true,
                        profile_offset_endpoint_parameter(outgoing.traversal, true),
                    ),
                ];
                let both_contact_owned = expected.iter().all(|(directed, _, parameter)| {
                    constraint_contacts(&constraint.definition)
                        .into_iter()
                        .any(|contact| {
                            self.profile_offset_contact_matches(contact, *directed, *parameter)
                        })
                });
                let owns_pair = match constraint.definition {
                    DocumentConstraintDefinition::Coincident { first, second } => self
                        .profile_offset_line_endpoint(incoming, false)
                        .ok()
                        .zip(self.profile_offset_line_endpoint(outgoing, true).ok())
                        .is_some_and(|(incoming, outgoing)| {
                            (incoming == first && outgoing == second)
                                || (incoming == second && outgoing == first)
                        }),
                    DocumentConstraintDefinition::PointOnCurve { point, contact } => {
                        [(0, 1), (1, 0)]
                            .into_iter()
                            .any(|(curve_index, point_index)| {
                                let (curve, _, parameter) = expected[curve_index];
                                let (point_curve, directed_start, _) = expected[point_index];
                                self.profile_offset_contact_matches(contact, curve, parameter)
                                    && self
                                        .profile_offset_line_endpoint(point_curve, directed_start)
                                        .is_ok_and(|endpoint| endpoint == point)
                            })
                    }
                    DocumentConstraintDefinition::LineCurveTangency {
                        line,
                        endpoint,
                        curve_contact,
                    } => [(0, 1), (1, 0)]
                        .into_iter()
                        .any(|(line_index, curve_index)| {
                            let (expected_line, _, line_parameter) = expected[line_index];
                            let (expected_curve, _, curve_parameter) = expected[curve_index];
                            let native_line_parameter = match endpoint {
                                FeatureEndpoint::Start => 0.0,
                                FeatureEndpoint::End => 1.0,
                            };
                            expected_line.curve == line
                                && (line_parameter - native_line_parameter).abs()
                                    <= 64.0 * f64::EPSILON
                                && self.profile_offset_contact_matches(
                                    curve_contact,
                                    expected_curve,
                                    curve_parameter,
                                )
                        }),
                    DocumentConstraintDefinition::LineCircleTangency { .. }
                    | DocumentConstraintDefinition::CircleArcTangency { .. }
                    | DocumentConstraintDefinition::CurveCurveContact { .. }
                    | DocumentConstraintDefinition::CurveCurveTangency { .. }
                    | DocumentConstraintDefinition::EndpointContinuity { .. } => both_contact_owned,
                    _ => false,
                };
                if !owns_pair {
                    return invalid(
                        "profile offset junction owner",
                        "the retained constraint must own both exact directed endpoints",
                    );
                }
            }
        }
        Ok(())
    }

    fn profile_offset_contact_matches(
        &self,
        contact: ContactId,
        directed: DocumentDirectedProfileOffsetCurve,
        parameter: f64,
    ) -> bool {
        self.contact(contact).is_some_and(|contact| {
            let neighborhood_matches = if parameter.to_bits() == 0.0_f64.to_bits() {
                matches!(contact.neighborhood, ContactNeighborhood::Start)
            } else if parameter.to_bits() == 1.0_f64.to_bits() {
                matches!(contact.neighborhood, ContactNeighborhood::End)
            } else {
                false
            };
            contact.curve == directed.curve
                && contact.domain
                    == (ContactDomain::Bounded {
                        lower: 0.0,
                        upper: 1.0,
                    })
                && contact.winding == 0
                && neighborhood_matches
                && self
                    .scalar(contact.parameter)
                    .is_some_and(|scalar| scalar.value.to_bits() == parameter.to_bits())
        })
    }

    fn profile_offset_line_endpoint(
        &self,
        curve: DocumentDirectedProfileOffsetCurve,
        start: bool,
    ) -> Result<DesignPointId, DocumentError> {
        let (native_start, native_end) = self.line_span_endpoint_ids(curve.curve)?;
        Ok(match (curve.traversal, start) {
            (DocumentOffsetTraversal::Forward, true)
            | (DocumentOffsetTraversal::Reverse, false) => native_start,
            (DocumentOffsetTraversal::Forward, false)
            | (DocumentOffsetTraversal::Reverse, true) => native_end,
        })
    }
}

pub(super) fn document_profile_offset_edges(
    operand: &DocumentProfileOffsetOperand,
) -> impl Iterator<Item = &DocumentProfileOffsetEdgePair> {
    let (first, rest): (
        &[DocumentProfileOffsetEdgePair],
        Vec<&[DocumentProfileOffsetEdgePair]>,
    ) = match operand {
        DocumentProfileOffsetOperand::Face { outer, holes, .. } => (
            &outer.edges,
            holes.iter().map(|value| value.edges.as_slice()).collect(),
        ),
        DocumentProfileOffsetOperand::OpenChain { chain, .. } => (&chain.edges, Vec::new()),
    };
    first.iter().chain(rest.into_iter().flatten())
}

pub(super) fn document_profile_offset_junctions(
    operand: &DocumentProfileOffsetOperand,
) -> impl Iterator<Item = &DocumentProfileOffsetJunction> {
    let (first, rest): (
        &[DocumentProfileOffsetJunction],
        Vec<&[DocumentProfileOffsetJunction]>,
    ) = match operand {
        DocumentProfileOffsetOperand::Face { outer, holes, .. } => (
            &outer.junctions,
            holes
                .iter()
                .map(|value| value.junctions.as_slice())
                .collect(),
        ),
        DocumentProfileOffsetOperand::OpenChain { chain, .. } => (&chain.junctions, Vec::new()),
    };
    first.iter().chain(rest.into_iter().flatten())
}
