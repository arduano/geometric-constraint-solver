// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    ArcId, CircleId, DimensionKind, DimensionMode, PointId, ProfileOffsetId, SegmentId, Sketch,
    SketchDimensionId, SketchError,
};

/// Direction in which one source or target support is traversed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OffsetTraversal {
    Forward,
    Reverse,
}

impl OffsetTraversal {
    pub(crate) const fn sign(self) -> f64 {
        match self {
            Self::Forward => 1.0,
            Self::Reverse => -1.0,
        }
    }
}

/// Exact analytic curve families accepted by the constraint-friendly offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileOffsetCurve {
    Line(SegmentId),
    CircularArc(ArcId),
    Circle(CircleId),
}

/// One exact curve support plus its explicit profile traversal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectedProfileOffsetCurve {
    pub curve: ProfileOffsetCurve,
    pub traversal: OffsetTraversal,
}

/// One source support and its existing, same-family target support.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileOffsetEdgePair {
    pub source: DirectedProfileOffsetCurve,
    pub target: DirectedProfileOffsetCurve,
}

/// Retained non-tangent turn at one ordered source/target junction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileOffsetTurn {
    Left,
    Right,
}

impl ProfileOffsetTurn {
    pub(crate) const fn sign(self) -> f64 {
        match self {
            Self::Left => 1.0,
            Self::Right => -1.0,
        }
    }
}

/// Explicit branch retained at one ordered edge junction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileOffsetJunctionBranch {
    Miter { turn: ProfileOffsetTurn },
    Tangent,
}

/// One closed material-left loop. Junction `i` joins edge `i` to edge `i + 1`
/// modulo the edge count.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileOffsetLoop {
    pub edges: Vec<ProfileOffsetEdgePair>,
    pub junctions: Vec<ProfileOffsetJunctionBranch>,
}

/// One open directed chain. Junction `i` joins edge `i` to edge `i + 1`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileOffsetChain {
    pub edges: Vec<ProfileOffsetEdgePair>,
    pub junctions: Vec<ProfileOffsetJunctionBranch>,
    pub start_terminal: ProfileOffsetTerminalPolicy,
    pub end_terminal: ProfileOffsetTerminalPolicy,
}

/// Explicit endpoint policy for one open offset chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileOffsetTerminalPolicy {
    NormalTranslation,
}

/// Material-side direction for a closed-face offset.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaceOffsetDirection {
    Outward,
    Inward,
}

impl FaceOffsetDirection {
    /// Signed displacement along the material-left normal.
    pub(crate) const fn left_normal_sign(self) -> f64 {
        match self {
            Self::Outward => -1.0,
            Self::Inward => 1.0,
        }
    }
}

/// Exact source/target topology retained by one grouped profile offset.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProfileOffsetOperand {
    Face {
        direction: FaceOffsetDirection,
        outer: ProfileOffsetLoop,
        holes: Vec<ProfileOffsetLoop>,
    },
    OpenChain {
        side: crate::LineSide,
        chain: ProfileOffsetChain,
    },
}

/// Runtime state owned by one driving profile-offset dimension.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileOffsetAssociation {
    pub operand: ProfileOffsetOperand,
}

impl ProfileOffsetAssociation {
    pub(crate) fn edge_pairs(&self) -> impl Iterator<Item = &ProfileOffsetEdgePair> {
        let (first, rest): (&[ProfileOffsetEdgePair], Vec<&[ProfileOffsetEdgePair]>) =
            match &self.operand {
                ProfileOffsetOperand::Face { outer, holes, .. } => (
                    &outer.edges,
                    holes.iter().map(|value| value.edges.as_slice()).collect(),
                ),
                ProfileOffsetOperand::OpenChain { chain, .. } => (&chain.edges, Vec::new()),
            };
        first.iter().chain(rest.into_iter().flatten())
    }

    pub(crate) fn references_curve(&self, curve: ProfileOffsetCurve) -> bool {
        self.edge_pairs()
            .any(|pair| pair.source.curve == curve || pair.target.curve == curve)
    }

    pub(crate) fn references_point(&self, sketch: &Sketch, point: PointId) -> bool {
        self.edge_pairs().any(|pair| {
            [pair.source.curve, pair.target.curve]
                .into_iter()
                .any(|curve| match curve {
                    ProfileOffsetCurve::Line(segment) => sketch
                        .segment_endpoints(segment)
                        .is_ok_and(|(start, end)| start == point || end == point),
                    ProfileOffsetCurve::CircularArc(arc) => sketch
                        .arcs
                        .get(arc)
                        .is_some_and(|value| value.center() == point),
                    ProfileOffsetCurve::Circle(circle) => sketch
                        .circles
                        .get(circle)
                        .is_some_and(|value| value.center() == point),
                })
        })
    }
}

impl Sketch {
    /// Adds one grouped, driving profile-offset association atomically.
    ///
    /// # Errors
    /// Returns an error for stale, repeated, unsupported, or structurally invalid
    /// curve topology, or for a non-positive distance.
    pub fn add_profile_offset(
        &mut self,
        association: ProfileOffsetAssociation,
        target: f64,
    ) -> Result<(ProfileOffsetId, SketchDimensionId), SketchError> {
        crate::model::validate_dimension_value(target)?;
        self.validate_profile_offset_association(&association)?;
        let profile = self.profile_offsets.insert(association);
        let dimension = self.insert_dimension(
            DimensionKind::ProfileOffset { profile, target },
            DimensionMode::Driving,
        );
        Ok((profile, dimension))
    }

    #[must_use]
    pub fn profile_offset(&self, profile: ProfileOffsetId) -> Option<&ProfileOffsetAssociation> {
        self.profile_offsets.get(profile)
    }

    pub fn profile_offsets(
        &self,
    ) -> impl Iterator<Item = (ProfileOffsetId, &ProfileOffsetAssociation)> {
        self.profile_offsets.iter()
    }

    /// Atomically replaces the retained topology/branch state of one association.
    ///
    /// # Errors
    /// Returns an error for a stale association or invalid replacement topology.
    pub fn set_profile_offset_association(
        &mut self,
        profile: ProfileOffsetId,
        association: ProfileOffsetAssociation,
    ) -> Result<(), SketchError> {
        if self.profile_offsets.get(profile).is_none() {
            return Err(SketchError::UnknownProfileOffset(profile));
        }
        self.validate_profile_offset_association(&association)?;
        *self
            .profile_offsets
            .get_mut(profile)
            .ok_or(SketchError::UnknownProfileOffset(profile))? = association;
        Ok(())
    }

    fn validate_profile_offset_association(
        &self,
        association: &ProfileOffsetAssociation,
    ) -> Result<(), SketchError> {
        let mut curves = Vec::new();
        match &association.operand {
            ProfileOffsetOperand::Face { outer, holes, .. } => {
                validate_loop_shape(outer)?;
                for hole in holes {
                    validate_loop_shape(hole)?;
                }
            }
            ProfileOffsetOperand::OpenChain { chain, .. } => validate_chain_shape(chain)?,
        }
        for pair in association.edge_pairs() {
            validate_same_family(pair)?;
            if pair.source.curve == pair.target.curve {
                return Err(SketchError::InvalidProfileOffset(
                    "source and target supports must be distinct",
                ));
            }
            for directed in [pair.source, pair.target] {
                self.validate_profile_offset_curve(directed.curve)?;
                if curves.contains(&directed.curve) {
                    return Err(SketchError::InvalidProfileOffset(
                        "every source and target support must occur exactly once",
                    ));
                }
                curves.push(directed.curve);
            }
        }
        Ok(())
    }

    fn validate_profile_offset_curve(&self, curve: ProfileOffsetCurve) -> Result<(), SketchError> {
        match curve {
            ProfileOffsetCurve::Line(segment) => self.validate_segment_geometry(segment),
            ProfileOffsetCurve::CircularArc(arc) => self.arc_value(arc).map(|_| ()),
            ProfileOffsetCurve::Circle(circle) => self.circle_value(circle).map(|_| ()),
        }
    }
}

fn validate_same_family(pair: &ProfileOffsetEdgePair) -> Result<(), SketchError> {
    if matches!(
        (pair.source.curve, pair.target.curve),
        (ProfileOffsetCurve::Line(_), ProfileOffsetCurve::Line(_))
            | (
                ProfileOffsetCurve::CircularArc(_),
                ProfileOffsetCurve::CircularArc(_)
            )
            | (ProfileOffsetCurve::Circle(_), ProfileOffsetCurve::Circle(_))
    ) {
        Ok(())
    } else {
        Err(SketchError::InvalidProfileOffset(
            "every source/target pair must use the same exact curve family",
        ))
    }
}

fn validate_loop_shape(loop_value: &ProfileOffsetLoop) -> Result<(), SketchError> {
    if loop_value.edges.is_empty() {
        return Err(SketchError::InvalidProfileOffset(
            "a face loop must contain at least one edge",
        ));
    }
    let single_circle = loop_value.edges.len() == 1
        && matches!(
            loop_value.edges[0].source.curve,
            ProfileOffsetCurve::Circle(_)
        );
    if single_circle {
        if loop_value.junctions.is_empty() {
            return Ok(());
        }
    } else if loop_value.edges.len() >= 2
        && loop_value.junctions.len() == loop_value.edges.len()
        && loop_value.edges.iter().all(|edge| {
            !matches!(edge.source.curve, ProfileOffsetCurve::Circle(_))
                && !matches!(edge.target.curve, ProfileOffsetCurve::Circle(_))
        })
    {
        return Ok(());
    }
    Err(SketchError::InvalidProfileOffset(
        "closed loops require one junction per non-circular edge, while a full circle has none",
    ))
}

fn validate_chain_shape(chain: &ProfileOffsetChain) -> Result<(), SketchError> {
    if chain.edges.is_empty() {
        return Err(SketchError::InvalidProfileOffset(
            "an open-chain operand must contain at least one edge",
        ));
    }
    if chain.junctions.len() + 1 == chain.edges.len()
        && chain
            .edges
            .iter()
            .all(|edge| !matches!(edge.source.curve, ProfileOffsetCurve::Circle(_)))
    {
        Ok(())
    } else {
        Err(SketchError::InvalidProfileOffset(
            "an open chain requires one junction between each edge and cannot contain a full circle",
        ))
    }
}
