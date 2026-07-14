use geosolve_core::CoreError;
use geosolve_geometry::Point2;
use slotmap::{Key, SlotMap, new_key_type};
use thiserror::Error;

new_key_type! {
    /// Stable identity of a sketch point.
    pub struct PointId;
    /// Stable identity of a line segment.
    pub struct SegmentId;
    /// Stable identity of a geometric sketch constraint.
    pub struct SketchConstraintId;
    /// Stable identity of a driving or reference dimension.
    pub struct SketchDimensionId;
}

/// Errors produced while editing, compiling, or solving a sketch.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum SketchError {
    #[error("model scale must be positive and finite, got {0}")]
    InvalidModelScale(f64),
    #[error("{context} must contain only finite coordinates, got ({x}, {y})")]
    NonFinitePoint {
        context: &'static str,
        x: f64,
        y: f64,
    },
    #[error("{context} must be finite, got {value}")]
    NonFiniteValue { context: &'static str, value: f64 },
    #[error("dimension value must be positive and finite, got {0}")]
    InvalidDimensionValue(f64),
    #[error("{0} label must not be empty")]
    EmptyLabel(&'static str),
    #[error("unknown or stale point ID {0:?}")]
    UnknownPoint(PointId),
    #[error("unknown or stale segment ID {0:?}")]
    UnknownSegment(SegmentId),
    #[error("unknown or stale sketch constraint ID {0:?}")]
    UnknownConstraint(SketchConstraintId),
    #[error("unknown or stale sketch dimension ID {0:?}")]
    UnknownDimension(SketchDimensionId),
    #[error("point {0:?} is still referenced by sketch geometry")]
    PointInUse(PointId),
    #[error("segment {0:?} is still referenced by a constraint or dimension")]
    SegmentInUse(SegmentId),
    #[error("a line segment requires two different, noncoincident points")]
    DegenerateSegment,
    #[error("a point-pair constraint or dimension requires two different points")]
    RepeatedPoint,
    #[error("a driving distance dimension requires noncoincident point geometry")]
    DegenerateDistance,
    #[error(transparent)]
    Core(#[from] CoreError),
}

#[derive(Clone, Debug)]
pub(crate) struct StableStore<K: Key, V> {
    values: SlotMap<K, V>,
    insertion_order: Vec<K>,
}

impl<K: Key, V> StableStore<K, V> {
    fn new() -> Self {
        Self {
            values: SlotMap::with_key(),
            insertion_order: Vec::new(),
        }
    }

    fn insert(&mut self, value: V) -> K {
        let id = self.values.insert(value);
        self.insertion_order.push(id);
        id
    }

    pub(crate) fn get(&self, id: K) -> Option<&V> {
        self.values.get(id)
    }

    fn get_mut(&mut self, id: K) -> Option<&mut V> {
        self.values.get_mut(id)
    }

    fn remove(&mut self, id: K) -> Option<V> {
        self.values.remove(id)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.insertion_order
            .iter()
            .filter_map(|&id| self.values.get(id).map(|value| (id, value)))
    }

    fn next_ordinal(&self) -> usize {
        self.insertion_order.len() + 1
    }
}

/// A sketch point with a deterministic human-readable label.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchPoint {
    position: Point2<f64>,
    label: String,
}

impl SketchPoint {
    #[must_use]
    pub fn position(&self) -> Point2<f64> {
        self.position
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Explicit directed branch retained by a line segment across solves.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentBranch {
    reference_direction: [f64; 2],
}

impl SegmentBranch {
    pub(crate) fn from_points(start: Point2<f64>, end: Point2<f64>) -> Result<Self, SketchError> {
        let direction = end - start;
        let norm = direction.x.hypot(direction.y);
        if !norm.is_finite() || norm == 0.0 {
            return Err(SketchError::DegenerateSegment);
        }
        Ok(Self {
            reference_direction: [direction.x / norm, direction.y / norm],
        })
    }

    /// Unit direction selected when this branch was explicitly established.
    #[must_use]
    pub const fn reference_direction(self) -> [f64; 2] {
        self.reference_direction
    }

    /// Whether a directed start/end pair remains in the selected half-plane.
    #[must_use]
    pub fn is_preserved(self, start: Point2<f64>, end: Point2<f64>) -> bool {
        let direction = end - start;
        let projection =
            direction.x * self.reference_direction[0] + direction.y * self.reference_direction[1];
        projection.is_finite() && projection > 0.0
    }
}

/// A directed line segment and its explicit branch state.
#[derive(Clone, Debug, PartialEq)]
pub struct LineSegment {
    start: PointId,
    end: PointId,
    branch: SegmentBranch,
    label: String,
}

impl LineSegment {
    #[must_use]
    pub const fn start(&self) -> PointId {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> PointId {
        self.end
    }

    #[must_use]
    pub const fn branch(&self) -> SegmentBranch {
        self.branch
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }
}

/// Cartesian coordinate selected by a fixed-coordinate constraint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoordinateAxis {
    X,
    Y,
}

/// Supported first-slice geometric constraints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SketchConstraintKind {
    FixedPoint {
        point: PointId,
        target: Point2<f64>,
    },
    FixedCoordinate {
        point: PointId,
        axis: CoordinateAxis,
        target: f64,
    },
    Coincident {
        first: PointId,
        second: PointId,
    },
    Horizontal {
        segment: SegmentId,
    },
    Vertical {
        segment: SegmentId,
    },
}

/// One stable high-level geometric constraint.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchConstraint {
    kind: SketchConstraintKind,
    ordinal: usize,
}

impl SketchConstraint {
    #[must_use]
    pub const fn kind(&self) -> SketchConstraintKind {
        self.kind
    }

    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
}

/// Whether a dimension contributes a hard equation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DimensionMode {
    Driving,
    Reference,
}

/// Supported first-slice dimensions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DimensionKind {
    PointDistance {
        first: PointId,
        second: PointId,
        target: f64,
    },
    SegmentLength {
        segment: SegmentId,
        target: f64,
    },
}

/// One stable driving/reference dimension.
#[derive(Clone, Debug, PartialEq)]
pub struct SketchDimension {
    kind: DimensionKind,
    mode: DimensionMode,
    ordinal: usize,
}

impl SketchDimension {
    #[must_use]
    pub const fn kind(&self) -> DimensionKind {
        self.kind
    }

    #[must_use]
    pub const fn mode(&self) -> DimensionMode {
        self.mode
    }

    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PersistentSource {
    Constraint(SketchConstraintId),
    Dimension(SketchDimensionId),
}

/// Ordered point/segment sketch domain with guarded edits.
#[derive(Clone, Debug)]
pub struct Sketch {
    pub(crate) model_scale: f64,
    pub(crate) points: StableStore<PointId, SketchPoint>,
    pub(crate) segments: StableStore<SegmentId, LineSegment>,
    pub(crate) constraints: StableStore<SketchConstraintId, SketchConstraint>,
    pub(crate) dimensions: StableStore<SketchDimensionId, SketchDimension>,
    pub(crate) source_order: Vec<PersistentSource>,
}

impl Sketch {
    /// Creates an empty sketch with one positive finite characteristic scale.
    ///
    /// # Errors
    ///
    /// Returns [`SketchError::InvalidModelScale`] for a nonpositive or non-finite scale.
    pub fn new(model_scale: f64) -> Result<Self, SketchError> {
        validate_model_scale(model_scale)?;
        Ok(Self {
            model_scale,
            points: StableStore::new(),
            segments: StableStore::new(),
            constraints: StableStore::new(),
            dimensions: StableStore::new(),
            source_order: Vec::new(),
        })
    }

    #[must_use]
    pub const fn model_scale(&self) -> f64 {
        self.model_scale
    }

    /// Replaces the characteristic model scale.
    ///
    /// # Errors
    ///
    /// Returns [`SketchError::InvalidModelScale`] for a nonpositive or non-finite scale.
    pub fn set_model_scale(&mut self, model_scale: f64) -> Result<(), SketchError> {
        validate_model_scale(model_scale)?;
        self.model_scale = model_scale;
        Ok(())
    }

    /// Adds a point with a deterministic generated label.
    ///
    /// # Errors
    ///
    /// Returns an error when either coordinate is non-finite.
    pub fn add_point(&mut self, position: Point2<f64>) -> Result<PointId, SketchError> {
        let label = format!("P{}", self.points.next_ordinal());
        self.add_named_point(label, position)
    }

    /// Adds a point with an explicit audit label.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty label or non-finite coordinate.
    pub fn add_named_point(
        &mut self,
        label: impl Into<String>,
        position: Point2<f64>,
    ) -> Result<PointId, SketchError> {
        validate_point(position, "point position")?;
        let label = nonempty_label(label, "point")?;
        Ok(self.points.insert(SketchPoint { position, label }))
    }

    #[must_use]
    pub fn point(&self, point: PointId) -> Option<&SketchPoint> {
        self.points.get(point)
    }

    pub fn points(&self) -> impl Iterator<Item = (PointId, &SketchPoint)> {
        self.points.iter()
    }

    /// Replaces a point's warm-start position without changing segment branch state.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale ID or non-finite coordinate.
    pub fn set_point_position(
        &mut self,
        point: PointId,
        position: Point2<f64>,
    ) -> Result<(), SketchError> {
        validate_point(position, "point position")?;
        self.points
            .get_mut(point)
            .ok_or(SketchError::UnknownPoint(point))?
            .position = position;
        Ok(())
    }

    /// Removes an unreferenced point.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale ID or a point still referenced by geometry.
    pub fn remove_point(&mut self, point: PointId) -> Result<SketchPoint, SketchError> {
        if self
            .segments
            .iter()
            .any(|(_, segment)| segment.start == point || segment.end == point)
            || self
                .constraints
                .iter()
                .any(|(_, constraint)| constraint_references_point(constraint.kind, point, self))
            || self
                .dimensions
                .iter()
                .any(|(_, dimension)| dimension_references_point(dimension.kind, point, self))
        {
            return Err(SketchError::PointInUse(point));
        }
        self.points
            .remove(point)
            .ok_or(SketchError::UnknownPoint(point))
    }

    /// Adds a directed segment with a generated label and current-direction branch.
    ///
    /// # Errors
    ///
    /// Returns an error for stale, repeated, or coincident endpoint geometry.
    pub fn add_segment(&mut self, start: PointId, end: PointId) -> Result<SegmentId, SketchError> {
        let label = format!("S{}", self.segments.next_ordinal());
        self.add_named_segment(label, start, end)
    }

    /// Adds a named directed segment and records its current-direction branch.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty label or invalid endpoint geometry.
    pub fn add_named_segment(
        &mut self,
        label: impl Into<String>,
        start: PointId,
        end: PointId,
    ) -> Result<SegmentId, SketchError> {
        if start == end {
            return Err(SketchError::DegenerateSegment);
        }
        let start_position = self.point_position(start)?;
        let end_position = self.point_position(end)?;
        let branch = SegmentBranch::from_points(start_position, end_position)?;
        let label = nonempty_label(label, "segment")?;
        Ok(self.segments.insert(LineSegment {
            start,
            end,
            branch,
            label,
        }))
    }

    #[must_use]
    pub fn segment(&self, segment: SegmentId) -> Option<&LineSegment> {
        self.segments.get(segment)
    }

    pub fn segments(&self) -> impl Iterator<Item = (SegmentId, &LineSegment)> {
        self.segments.iter()
    }

    /// Reports whether the current geometry is on a segment's explicit branch.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment or endpoint ID.
    pub fn segment_branch_is_preserved(&self, segment: SegmentId) -> Result<bool, SketchError> {
        let segment = self
            .segments
            .get(segment)
            .ok_or(SketchError::UnknownSegment(segment))?;
        Ok(segment.branch.is_preserved(
            self.point_position(segment.start)?,
            self.point_position(segment.end)?,
        ))
    }

    /// Reports whether this segment currently has a discrete axis/length root.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment ID.
    pub fn segment_has_enforced_branch(&self, segment: SegmentId) -> Result<bool, SketchError> {
        self.segments
            .get(segment)
            .ok_or(SketchError::UnknownSegment(segment))?;
        Ok(self.segment_branch_is_enforced(segment))
    }

    pub(crate) fn segment_branch_is_enforced(&self, segment: SegmentId) -> bool {
        let has_axis_constraint = self.constraints.iter().any(|(_, constraint)| {
            matches!(
                constraint.kind,
                SketchConstraintKind::Horizontal { segment: id }
                    | SketchConstraintKind::Vertical { segment: id }
                    if id == segment
            )
        });
        let has_driving_length = self.dimensions.iter().any(|(_, dimension)| {
            dimension.mode == DimensionMode::Driving
                && matches!(
                    dimension.kind,
                    DimensionKind::SegmentLength { segment: id, .. } if id == segment
                )
        });
        has_axis_constraint && has_driving_length
    }

    /// Explicitly selects the segment branch represented by its current direction.
    ///
    /// # Errors
    ///
    /// Returns an error for stale IDs or coincident endpoints.
    pub fn reselect_segment_branch(&mut self, segment: SegmentId) -> Result<(), SketchError> {
        let (start, end) = self.segment_endpoints(segment)?;
        let branch =
            SegmentBranch::from_points(self.point_position(start)?, self.point_position(end)?)?;
        self.segments
            .get_mut(segment)
            .ok_or(SketchError::UnknownSegment(segment))?
            .branch = branch;
        Ok(())
    }

    /// Removes a segment not referenced by a constraint or dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale or referenced segment.
    pub fn remove_segment(&mut self, segment: SegmentId) -> Result<LineSegment, SketchError> {
        if self.constraints.iter().any(|(_, constraint)| {
            matches!(
                constraint.kind,
                SketchConstraintKind::Horizontal { segment: id }
                    | SketchConstraintKind::Vertical { segment: id }
                    if id == segment
            )
        }) || self.dimensions.iter().any(|(_, dimension)| {
            matches!(
                dimension.kind,
                DimensionKind::SegmentLength { segment: id, .. } if id == segment
            )
        }) {
            return Err(SketchError::SegmentInUse(segment));
        }
        self.segments
            .remove(segment)
            .ok_or(SketchError::UnknownSegment(segment))
    }

    /// Fixes a point at its current position using trusted core elimination.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale point ID.
    pub fn add_fixed_point(&mut self, point: PointId) -> Result<SketchConstraintId, SketchError> {
        let target = self.point_position(point)?;
        Ok(self.insert_constraint(SketchConstraintKind::FixedPoint { point, target }))
    }

    /// Fixes a point at an explicit finite target using trusted core elimination.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale point ID or non-finite target.
    pub fn add_fixed_point_at(
        &mut self,
        point: PointId,
        target: Point2<f64>,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point_position(point)?;
        validate_point(target, "fixed-point target")?;
        Ok(self.insert_constraint(SketchConstraintKind::FixedPoint { point, target }))
    }

    /// Adds a scalar fixed-coordinate equation.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale point ID or non-finite target.
    pub fn add_fixed_coordinate(
        &mut self,
        point: PointId,
        axis: CoordinateAxis,
        target: f64,
    ) -> Result<SketchConstraintId, SketchError> {
        self.point_position(point)?;
        validate_finite(target, "fixed-coordinate target")?;
        Ok(
            self.insert_constraint(SketchConstraintKind::FixedCoordinate {
                point,
                axis,
                target,
            }),
        )
    }

    /// Adds a two-row point coincidence constraint.
    ///
    /// # Errors
    ///
    /// Returns an error for stale or repeated point IDs.
    pub fn add_coincident(
        &mut self,
        first: PointId,
        second: PointId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_point_pair(first, second)?;
        Ok(self.insert_constraint(SketchConstraintKind::Coincident { first, second }))
    }

    /// Constrains a segment to be horizontal.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment ID.
    pub fn add_horizontal(
        &mut self,
        segment: SegmentId,
    ) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(segment)?;
        Ok(self.insert_constraint(SketchConstraintKind::Horizontal { segment }))
    }

    /// Constrains a segment to be vertical.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment ID.
    pub fn add_vertical(&mut self, segment: SegmentId) -> Result<SketchConstraintId, SketchError> {
        self.validate_segment_geometry(segment)?;
        Ok(self.insert_constraint(SketchConstraintKind::Vertical { segment }))
    }

    #[must_use]
    pub fn constraint(&self, constraint: SketchConstraintId) -> Option<&SketchConstraint> {
        self.constraints.get(constraint)
    }

    pub fn constraints(&self) -> impl Iterator<Item = (SketchConstraintId, &SketchConstraint)> {
        self.constraints.iter()
    }

    /// Removes a constraint while leaving its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale constraint ID.
    pub fn remove_constraint(
        &mut self,
        constraint: SketchConstraintId,
    ) -> Result<SketchConstraint, SketchError> {
        self.constraints
            .remove(constraint)
            .ok_or(SketchError::UnknownConstraint(constraint))
    }

    /// Adds a point-distance driving or reference dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/repeated points or an invalid target.
    pub fn add_point_distance(
        &mut self,
        first: PointId,
        second: PointId,
        target: f64,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        self.validate_point_pair(first, second)?;
        validate_dimension_value(target)?;
        if mode == DimensionMode::Driving
            && self.point_position(first)? == self.point_position(second)?
        {
            return Err(SketchError::DegenerateDistance);
        }
        Ok(self.insert_dimension(
            DimensionKind::PointDistance {
                first,
                second,
                target,
            },
            mode,
        ))
    }

    /// Adds a segment-length driving or reference dimension.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale segment or invalid target.
    pub fn add_segment_length(
        &mut self,
        segment: SegmentId,
        target: f64,
        mode: DimensionMode,
    ) -> Result<SketchDimensionId, SketchError> {
        if mode == DimensionMode::Driving {
            self.validate_segment_geometry(segment)?;
        } else {
            self.segment_endpoints(segment)?;
        }
        validate_dimension_value(target)?;
        Ok(self.insert_dimension(DimensionKind::SegmentLength { segment, target }, mode))
    }

    #[must_use]
    pub fn dimension(&self, dimension: SketchDimensionId) -> Option<&SketchDimension> {
        self.dimensions.get(dimension)
    }

    pub fn dimensions(&self) -> impl Iterator<Item = (SketchDimensionId, &SketchDimension)> {
        self.dimensions.iter()
    }

    /// Toggles a dimension without changing its stable ID or source position.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale dimension ID.
    pub fn set_dimension_mode(
        &mut self,
        dimension: SketchDimensionId,
        mode: DimensionMode,
    ) -> Result<(), SketchError> {
        self.dimensions
            .get_mut(dimension)
            .ok_or(SketchError::UnknownDimension(dimension))?
            .mode = mode;
        Ok(())
    }

    /// Edits a finite positive dimension target without changing its stable ID.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid target or stale dimension ID.
    pub fn set_dimension_target(
        &mut self,
        dimension: SketchDimensionId,
        target: f64,
    ) -> Result<(), SketchError> {
        validate_dimension_value(target)?;
        let kind = &mut self
            .dimensions
            .get_mut(dimension)
            .ok_or(SketchError::UnknownDimension(dimension))?
            .kind;
        match kind {
            DimensionKind::PointDistance {
                target: current, ..
            }
            | DimensionKind::SegmentLength {
                target: current, ..
            } => *current = target,
        }
        Ok(())
    }

    /// Removes a dimension while leaving its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale dimension ID.
    pub fn remove_dimension(
        &mut self,
        dimension: SketchDimensionId,
    ) -> Result<SketchDimension, SketchError> {
        self.dimensions
            .remove(dimension)
            .ok_or(SketchError::UnknownDimension(dimension))
    }

    pub(crate) fn point_position(&self, point: PointId) -> Result<Point2<f64>, SketchError> {
        self.points
            .get(point)
            .map(SketchPoint::position)
            .ok_or(SketchError::UnknownPoint(point))
    }

    pub(crate) fn segment_endpoints(
        &self,
        segment: SegmentId,
    ) -> Result<(PointId, PointId), SketchError> {
        self.segments
            .get(segment)
            .map(|segment| (segment.start, segment.end))
            .ok_or(SketchError::UnknownSegment(segment))
    }

    fn validate_segment_geometry(&self, segment: SegmentId) -> Result<(), SketchError> {
        let (start, end) = self.segment_endpoints(segment)?;
        SegmentBranch::from_points(self.point_position(start)?, self.point_position(end)?)?;
        Ok(())
    }

    pub(crate) fn dimension_value(&self, dimension: &SketchDimension) -> Result<f64, SketchError> {
        let (first, second) = match dimension.kind {
            DimensionKind::PointDistance { first, second, .. } => (first, second),
            DimensionKind::SegmentLength { segment, .. } => self.segment_endpoints(segment)?,
        };
        let first = self.point_position(first)?;
        let second = self.point_position(second)?;
        let value = (second - first).norm();
        validate_finite(value, "reference dimension value")?;
        Ok(value)
    }

    fn validate_point_pair(&self, first: PointId, second: PointId) -> Result<(), SketchError> {
        self.point_position(first)?;
        self.point_position(second)?;
        if first == second {
            Err(SketchError::RepeatedPoint)
        } else {
            Ok(())
        }
    }

    fn insert_constraint(&mut self, kind: SketchConstraintKind) -> SketchConstraintId {
        let constraint = self.constraints.insert(SketchConstraint {
            kind,
            ordinal: self.constraints.next_ordinal(),
        });
        self.source_order
            .push(PersistentSource::Constraint(constraint));
        constraint
    }

    fn insert_dimension(&mut self, kind: DimensionKind, mode: DimensionMode) -> SketchDimensionId {
        let dimension = self.dimensions.insert(SketchDimension {
            kind,
            mode,
            ordinal: self.dimensions.next_ordinal(),
        });
        self.source_order
            .push(PersistentSource::Dimension(dimension));
        dimension
    }
}

fn constraint_references_point(
    kind: SketchConstraintKind,
    point: PointId,
    sketch: &Sketch,
) -> bool {
    match kind {
        SketchConstraintKind::FixedPoint { point: id, .. }
        | SketchConstraintKind::FixedCoordinate { point: id, .. } => id == point,
        SketchConstraintKind::Coincident { first, second } => first == point || second == point,
        SketchConstraintKind::Horizontal { segment }
        | SketchConstraintKind::Vertical { segment } => sketch
            .segment_endpoints(segment)
            .is_ok_and(|(first, second)| first == point || second == point),
    }
}

fn dimension_references_point(kind: DimensionKind, point: PointId, sketch: &Sketch) -> bool {
    match kind {
        DimensionKind::PointDistance { first, second, .. } => first == point || second == point,
        DimensionKind::SegmentLength { segment, .. } => sketch
            .segment_endpoints(segment)
            .is_ok_and(|(first, second)| first == point || second == point),
    }
}

pub(crate) fn validate_model_scale(model_scale: f64) -> Result<(), SketchError> {
    if model_scale.is_finite() && model_scale > 0.0 {
        Ok(())
    } else {
        Err(SketchError::InvalidModelScale(model_scale))
    }
}

pub(crate) fn validate_point(point: Point2<f64>, context: &'static str) -> Result<(), SketchError> {
    if point.x.is_finite() && point.y.is_finite() {
        Ok(())
    } else {
        Err(SketchError::NonFinitePoint {
            context,
            x: point.x,
            y: point.y,
        })
    }
}

pub(crate) fn validate_finite(value: f64, context: &'static str) -> Result<(), SketchError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(SketchError::NonFiniteValue { context, value })
    }
}

fn validate_dimension_value(value: f64) -> Result<(), SketchError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(SketchError::InvalidDimensionValue(value))
    }
}

fn nonempty_label(label: impl Into<String>, kind: &'static str) -> Result<String, SketchError> {
    let label = label.into();
    if label.trim().is_empty() {
        Err(SketchError::EmptyLabel(kind))
    } else {
        Ok(label)
    }
}
