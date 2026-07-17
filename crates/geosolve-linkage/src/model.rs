use geosolve_core::{ContinuationError, CoreError};
use geosolve_geometry::{PlaneFrame, Point2, Pose2, Vector2};
use slotmap::{Key, SlotMap, new_key_type};
use thiserror::Error;

const FRAME_TOLERANCE: f64 = 1.0e-12;

new_key_type! {
    /// Stable identity of a rigid body.
    pub struct BodyId;
    /// Stable identity of a body-local point feature.
    pub struct PointFeatureId;
    /// Stable identity of a body-local directed axis feature.
    pub struct AxisFeatureId;
    /// Stable identity of a joint.
    pub struct JointId;
    /// Stable identity of a driver.
    pub struct DriverId;
    /// Stable identity of a scenario/domain branch monitor.
    pub struct BranchMonitorId;
}

/// Errors produced while constructing, compiling, solving, or differentiating a linkage.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum LinkageError {
    #[error("model scale must be positive and finite, got {0}")]
    InvalidModelScale(f64),
    #[error("invalid plane frame: {0}")]
    InvalidPlaneFrame(&'static str),
    #[error("{context} pose must be finite")]
    NonFinitePose { context: &'static str },
    #[error("{context} point must contain only finite coordinates")]
    NonFinitePoint { context: &'static str },
    #[error("{context} axis must be finite and nonzero")]
    InvalidAxis { context: &'static str },
    #[error("{context} must be finite, got {value}")]
    NonFiniteValue { context: &'static str, value: f64 },
    #[error("maximum continuation step must be positive and finite, got {0}")]
    InvalidContinuationStep(f64),
    #[error("pseudo-arclength distance must be positive and finite, got {0}")]
    InvalidContinuationDistance(f64),
    #[error("requested continuation requires too many deterministic samples")]
    ContinuationSampleOverflow,
    #[error("{0} label must not be empty")]
    EmptyLabel(&'static str),
    #[error("unknown or stale rigid body ID {0:?}")]
    UnknownBody(BodyId),
    #[error("unknown or stale point feature ID {0:?}")]
    UnknownPointFeature(PointFeatureId),
    #[error("unknown or stale axis feature ID {0:?}")]
    UnknownAxisFeature(AxisFeatureId),
    #[error("unknown or stale joint ID {0:?}")]
    UnknownJoint(JointId),
    #[error("unknown or stale driver ID {0:?}")]
    UnknownDriver(DriverId),
    #[error("unknown or stale branch monitor ID {0:?}")]
    UnknownBranchMonitor(BranchMonitorId),
    #[error("rigid body {0:?} is still referenced by linkage geometry")]
    BodyInUse(BodyId),
    #[error("point feature {0:?} is still referenced")]
    PointFeatureInUse(PointFeatureId),
    #[error("axis feature {0:?} is still referenced")]
    AxisFeatureInUse(AxisFeatureId),
    #[error("a joint or relative driver requires two different rigid bodies")]
    RepeatedBody,
    #[error(
        "a prismatic joint's point and axis features must belong to the same respective bodies"
    )]
    PrismaticFeatureBodyMismatch,
    #[error("a linear driver guide axis and origin point must belong to the same body")]
    LinearDriverFeatureBodyMismatch,
    #[error("a solve was requested at geometry that is not an accepted hard-constraint state: {0}")]
    PositionNotAccepted(String),
    #[error("velocity solve failed: {0}")]
    VelocityFailure(&'static str),
    #[error(transparent)]
    Continuation(#[from] ContinuationError),
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

    pub(crate) fn get_mut(&mut self, id: K) -> Option<&mut V> {
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

/// One accepted planar pose and its fixed/free role in the mechanism.
#[derive(Clone, Debug, PartialEq)]
pub struct RigidBody {
    label: String,
    pose: Pose2,
    grounded: bool,
}

impl RigidBody {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn pose(&self) -> Pose2 {
        self.pose
    }

    #[must_use]
    pub const fn grounded(&self) -> bool {
        self.grounded
    }
}

/// A finite point expressed in its owning body's coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct PointFeature {
    label: String,
    body: BodyId,
    local_point: Point2<f64>,
}

impl PointFeature {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }

    #[must_use]
    pub const fn local_point(&self) -> Point2<f64> {
        self.local_point
    }
}

/// A normalized directed axis expressed in its owning body's coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct AxisFeature {
    label: String,
    body: BodyId,
    local_axis: Vector2<f64>,
}

impl AxisFeature {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> BodyId {
        self.body
    }

    #[must_use]
    pub const fn local_axis(&self) -> Vector2<f64> {
        self.local_axis
    }
}

/// Explicit relative direction selected for the two axes of a prismatic joint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AxisDirectionBranch {
    Same,
    Opposite,
}

impl AxisDirectionBranch {
    pub(crate) const fn multiplier(self) -> f64 {
        match self {
            Self::Same => 1.0,
            Self::Opposite => -1.0,
        }
    }
}

/// Supported planar joint equations.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JointKind {
    Revolute {
        first: PointFeatureId,
        second: PointFeatureId,
    },
    Prismatic {
        first_anchor: PointFeatureId,
        first_axis: AxisFeatureId,
        second_anchor: PointFeatureId,
        second_axis: AxisFeatureId,
        axis_branch: AxisDirectionBranch,
    },
    Weld {
        first: PointFeatureId,
        second: PointFeatureId,
        relative_angle: f64,
    },
}

/// One stable high-level linkage joint.
#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    label: String,
    kind: JointKind,
    ordinal: usize,
}

impl Joint {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> JointKind {
        self.kind
    }

    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
}

/// Physical unit exposed by a driver target and rate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriverUnit {
    Radian,
    ModelUnit,
}

impl DriverUnit {
    #[must_use]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::Radian => "rad",
            Self::ModelUnit => "model-unit",
        }
    }
}

/// Supported hard position drivers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DriverKind {
    Angular {
        reference: BodyId,
        driven: BodyId,
    },
    Linear {
        origin: PointFeatureId,
        measured: PointFeatureId,
        guide_axis: AxisFeatureId,
    },
}

/// One stable hard driver with a bounded continuation policy.
#[derive(Clone, Debug, PartialEq)]
pub struct Driver {
    label: String,
    kind: DriverKind,
    target: f64,
    max_continuation_step: f64,
    ordinal: usize,
}

impl Driver {
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> DriverKind {
        self.kind
    }

    #[must_use]
    pub const fn target(&self) -> f64 {
        self.target
    }

    #[must_use]
    pub const fn max_continuation_step(&self) -> f64 {
        self.max_continuation_step
    }

    #[must_use]
    pub const fn unit(&self) -> DriverUnit {
        match self.kind {
            DriverKind::Angular { .. } => DriverUnit::Radian,
            DriverKind::Linear { .. } => DriverUnit::ModelUnit,
        }
    }

    #[must_use]
    pub const fn ordinal(&self) -> usize {
        self.ordinal
    }
}

/// Sign retained by an explicit branch monitor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchSign {
    Positive,
    Negative,
}

impl BranchSign {
    pub(crate) const fn multiplier(self) -> f64 {
        match self {
            Self::Positive => 1.0,
            Self::Negative => -1.0,
        }
    }

    pub(crate) fn from_nonzero(value: f64) -> Result<Self, LinkageError> {
        if !value.is_finite() || value == 0.0 {
            Err(LinkageError::NonFiniteValue {
                context: "branch orientation",
                value,
            })
        } else if value > 0.0 {
            Ok(Self::Positive)
        } else {
            Ok(Self::Negative)
        }
    }
}

/// Explicit non-equation branch state checked after every attempted solve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BranchMonitor {
    Orientation {
        line_start: PointFeatureId,
        line_end: PointFeatureId,
        observed: PointFeatureId,
        sign: BranchSign,
    },
    DirectedDisplacement {
        origin: PointFeatureId,
        measured: PointFeatureId,
        axis: AxisFeatureId,
        sign: BranchSign,
    },
}

/// The exact discrete branch check that rejected a candidate state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BranchViolation {
    PrismaticAxis(JointId),
    Monitor(BranchMonitorId),
}

/// Ordered linkage domain model. Stored poses and driver targets are accepted state.
#[derive(Clone, Debug)]
pub struct Linkage {
    pub(crate) model_scale: f64,
    pub(crate) plane_frame: PlaneFrame,
    pub(crate) bodies: StableStore<BodyId, RigidBody>,
    pub(crate) point_features: StableStore<PointFeatureId, PointFeature>,
    pub(crate) axis_features: StableStore<AxisFeatureId, AxisFeature>,
    pub(crate) joints: StableStore<JointId, Joint>,
    pub(crate) drivers: StableStore<DriverId, Driver>,
    pub(crate) branch_monitors: StableStore<BranchMonitorId, BranchMonitor>,
}

impl Linkage {
    /// Creates an empty linkage with validated model scale and work plane.
    ///
    /// # Errors
    ///
    /// Rejects nonpositive/non-finite scale and non-finite, non-unit, or
    /// non-orthogonal plane axes.
    pub fn new(model_scale: f64, plane_frame: PlaneFrame) -> Result<Self, LinkageError> {
        validate_model_scale(model_scale)?;
        validate_plane_frame(plane_frame)?;
        Ok(Self {
            model_scale,
            plane_frame,
            bodies: StableStore::new(),
            point_features: StableStore::new(),
            axis_features: StableStore::new(),
            joints: StableStore::new(),
            drivers: StableStore::new(),
            branch_monitors: StableStore::new(),
        })
    }

    #[must_use]
    pub const fn model_scale(&self) -> f64 {
        self.model_scale
    }

    #[must_use]
    pub const fn plane_frame(&self) -> PlaneFrame {
        self.plane_frame
    }

    /// Replaces the model scale after validation.
    ///
    /// # Errors
    ///
    /// Returns [`LinkageError::InvalidModelScale`] for invalid input.
    pub fn set_model_scale(&mut self, model_scale: f64) -> Result<(), LinkageError> {
        validate_model_scale(model_scale)?;
        self.model_scale = model_scale;
        Ok(())
    }

    /// Replaces the work plane after strict orthonormal validation.
    ///
    /// # Errors
    ///
    /// Returns [`LinkageError::InvalidPlaneFrame`] for an invalid frame.
    pub fn set_plane_frame(&mut self, plane_frame: PlaneFrame) -> Result<(), LinkageError> {
        validate_plane_frame(plane_frame)?;
        self.plane_frame = plane_frame;
        Ok(())
    }

    /// Adds a rigid body with a finite accepted pose.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty label or non-finite pose.
    pub fn add_body(
        &mut self,
        label: impl Into<String>,
        pose: Pose2,
        grounded: bool,
    ) -> Result<BodyId, LinkageError> {
        validate_pose(pose, "rigid body")?;
        let label = nonempty_label(label, "rigid body")?;
        Ok(self.bodies.insert(RigidBody {
            label,
            pose,
            grounded,
        }))
    }

    #[must_use]
    pub fn body(&self, body: BodyId) -> Option<&RigidBody> {
        self.bodies.get(body)
    }

    pub fn bodies(&self) -> impl Iterator<Item = (BodyId, &RigidBody)> {
        self.bodies.iter()
    }

    /// Replaces an accepted warm-start pose without wrapping its angle.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale body or non-finite pose.
    pub fn set_body_pose(&mut self, body: BodyId, pose: Pose2) -> Result<(), LinkageError> {
        validate_pose(pose, "rigid body")?;
        self.bodies
            .get_mut(body)
            .ok_or(LinkageError::UnknownBody(body))?
            .pose = pose;
        Ok(())
    }

    /// Removes an unreferenced body, leaving its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale or referenced body.
    pub fn remove_body(&mut self, body: BodyId) -> Result<RigidBody, LinkageError> {
        if self
            .point_features
            .iter()
            .any(|(_, feature)| feature.body == body)
            || self
                .axis_features
                .iter()
                .any(|(_, feature)| feature.body == body)
            || self.drivers.iter().any(|(_, driver)| {
                matches!(
                    driver.kind,
                    DriverKind::Angular { reference, driven }
                        if reference == body || driven == body
                )
            })
        {
            return Err(LinkageError::BodyInUse(body));
        }
        self.bodies
            .remove(body)
            .ok_or(LinkageError::UnknownBody(body))
    }

    /// Adds a finite body-local point feature.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale body, empty label, or non-finite point.
    pub fn add_point_feature(
        &mut self,
        label: impl Into<String>,
        body: BodyId,
        local_point: Point2<f64>,
    ) -> Result<PointFeatureId, LinkageError> {
        self.require_body(body)?;
        validate_point(local_point, "point feature")?;
        let label = nonempty_label(label, "point feature")?;
        Ok(self.point_features.insert(PointFeature {
            label,
            body,
            local_point,
        }))
    }

    #[must_use]
    pub fn point_feature(&self, feature: PointFeatureId) -> Option<&PointFeature> {
        self.point_features.get(feature)
    }

    pub fn point_features(&self) -> impl Iterator<Item = (PointFeatureId, &PointFeature)> {
        self.point_features.iter()
    }

    /// Removes an unreferenced point feature.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale or referenced feature.
    pub fn remove_point_feature(
        &mut self,
        feature: PointFeatureId,
    ) -> Result<PointFeature, LinkageError> {
        if self
            .joints
            .iter()
            .any(|(_, joint)| joint_references_point(joint.kind, feature))
            || self
                .drivers
                .iter()
                .any(|(_, driver)| driver_references_point(driver.kind, feature))
            || self
                .branch_monitors
                .iter()
                .any(|(_, monitor)| monitor_references_point(*monitor, feature))
        {
            return Err(LinkageError::PointFeatureInUse(feature));
        }
        self.point_features
            .remove(feature)
            .ok_or(LinkageError::UnknownPointFeature(feature))
    }

    /// Adds a directed feature, normalizing any finite nonzero input vector.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale body, empty label, or invalid axis.
    pub fn add_axis_feature(
        &mut self,
        label: impl Into<String>,
        body: BodyId,
        local_axis: Vector2<f64>,
    ) -> Result<AxisFeatureId, LinkageError> {
        self.require_body(body)?;
        let local_axis = normalized_axis(local_axis, "axis feature")?;
        let label = nonempty_label(label, "axis feature")?;
        Ok(self.axis_features.insert(AxisFeature {
            label,
            body,
            local_axis,
        }))
    }

    #[must_use]
    pub fn axis_feature(&self, feature: AxisFeatureId) -> Option<&AxisFeature> {
        self.axis_features.get(feature)
    }

    pub fn axis_features(&self) -> impl Iterator<Item = (AxisFeatureId, &AxisFeature)> {
        self.axis_features.iter()
    }

    /// Removes an unreferenced axis feature.
    ///
    /// # Errors
    ///
    /// Returns an error for a stale or referenced feature.
    pub fn remove_axis_feature(
        &mut self,
        feature: AxisFeatureId,
    ) -> Result<AxisFeature, LinkageError> {
        if self
            .joints
            .iter()
            .any(|(_, joint)| joint_references_axis(joint.kind, feature))
            || self
                .drivers
                .iter()
                .any(|(_, driver)| driver_references_axis(driver.kind, feature))
            || self
                .branch_monitors
                .iter()
                .any(|(_, monitor)| monitor_references_axis(*monitor, feature))
        {
            return Err(LinkageError::AxisFeatureInUse(feature));
        }
        self.axis_features
            .remove(feature)
            .ok_or(LinkageError::UnknownAxisFeature(feature))
    }

    /// Adds a two-row transformed-anchor coincidence joint.
    ///
    /// # Errors
    ///
    /// Returns an error for stale features, equal owning bodies, or an empty label.
    pub fn add_revolute_joint(
        &mut self,
        label: impl Into<String>,
        first: PointFeatureId,
        second: PointFeatureId,
    ) -> Result<JointId, LinkageError> {
        self.require_distinct_point_bodies(first, second)?;
        self.insert_joint(label, JointKind::Revolute { first, second })
    }

    #[allow(clippy::too_many_arguments)]
    /// Adds a transverse-displacement/alignment prismatic joint.
    ///
    /// # Errors
    ///
    /// Returns an error for stale features, invalid ownership, equal bodies, or an empty label.
    pub fn add_prismatic_joint(
        &mut self,
        label: impl Into<String>,
        first_anchor: PointFeatureId,
        first_axis: AxisFeatureId,
        second_anchor: PointFeatureId,
        second_axis: AxisFeatureId,
        axis_branch: AxisDirectionBranch,
    ) -> Result<JointId, LinkageError> {
        let first_point_body = self.require_point_feature(first_anchor)?.body;
        let second_point_body = self.require_point_feature(second_anchor)?.body;
        let first_axis_body = self.require_axis_feature(first_axis)?.body;
        let second_axis_body = self.require_axis_feature(second_axis)?.body;
        if first_point_body != first_axis_body || second_point_body != second_axis_body {
            return Err(LinkageError::PrismaticFeatureBodyMismatch);
        }
        if first_point_body == second_point_body {
            return Err(LinkageError::RepeatedBody);
        }
        self.insert_joint(
            label,
            JointKind::Prismatic {
                first_anchor,
                first_axis,
                second_anchor,
                second_axis,
                axis_branch,
            },
        )
    }

    /// Adds a weld preserving the bodies' current unwrapped relative angle.
    ///
    /// # Errors
    ///
    /// Returns an error for stale features, equal bodies, or an empty label.
    pub fn add_weld_joint(
        &mut self,
        label: impl Into<String>,
        first: PointFeatureId,
        second: PointFeatureId,
    ) -> Result<JointId, LinkageError> {
        let (first_body, second_body) = self.require_distinct_point_bodies(first, second)?;
        let relative_angle =
            self.require_body(second_body)?.pose.angle - self.require_body(first_body)?.pose.angle;
        self.add_weld_joint_with_angle(label, first, second, relative_angle)
    }

    /// Adds a weld with an explicit finite unwrapped relative angle.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid features, angle, ownership, or label.
    pub fn add_weld_joint_with_angle(
        &mut self,
        label: impl Into<String>,
        first: PointFeatureId,
        second: PointFeatureId,
        relative_angle: f64,
    ) -> Result<JointId, LinkageError> {
        self.require_distinct_point_bodies(first, second)?;
        validate_finite(relative_angle, "weld relative angle")?;
        self.insert_joint(
            label,
            JointKind::Weld {
                first,
                second,
                relative_angle,
            },
        )
    }

    #[must_use]
    pub fn joint(&self, joint: JointId) -> Option<&Joint> {
        self.joints.get(joint)
    }

    pub fn joints(&self) -> impl Iterator<Item = (JointId, &Joint)> {
        self.joints.iter()
    }

    /// Removes a joint, leaving its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale ID.
    pub fn remove_joint(&mut self, joint: JointId) -> Result<Joint, LinkageError> {
        self.joints
            .remove(joint)
            .ok_or(LinkageError::UnknownJoint(joint))
    }

    /// Adds a hard unwrapped relative-angle driver.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/equal bodies or invalid target, step, or label.
    pub fn add_angular_driver(
        &mut self,
        label: impl Into<String>,
        reference: BodyId,
        driven: BodyId,
        target: f64,
        max_continuation_step: f64,
    ) -> Result<DriverId, LinkageError> {
        self.require_body(reference)?;
        self.require_body(driven)?;
        if reference == driven {
            return Err(LinkageError::RepeatedBody);
        }
        self.insert_driver(
            label,
            DriverKind::Angular { reference, driven },
            target,
            max_continuation_step,
        )
    }

    /// Adds a hard directed displacement driver along a body-local guide axis.
    ///
    /// # Errors
    ///
    /// Returns an error for stale/mismatched features or invalid target, step, or label.
    pub fn add_linear_driver(
        &mut self,
        label: impl Into<String>,
        origin: PointFeatureId,
        measured: PointFeatureId,
        guide_axis: AxisFeatureId,
        target: f64,
        max_continuation_step: f64,
    ) -> Result<DriverId, LinkageError> {
        let origin_body = self.require_point_feature(origin)?.body;
        let measured_body = self.require_point_feature(measured)?.body;
        let guide_body = self.require_axis_feature(guide_axis)?.body;
        if origin_body != guide_body {
            return Err(LinkageError::LinearDriverFeatureBodyMismatch);
        }
        if origin_body == measured_body {
            return Err(LinkageError::RepeatedBody);
        }
        self.insert_driver(
            label,
            DriverKind::Linear {
                origin,
                measured,
                guide_axis,
            },
            target,
            max_continuation_step,
        )
    }

    #[must_use]
    pub fn driver(&self, driver: DriverId) -> Option<&Driver> {
        self.drivers.get(driver)
    }

    pub fn drivers(&self) -> impl Iterator<Item = (DriverId, &Driver)> {
        self.drivers.iter()
    }

    /// Removes a driver, leaving its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale ID.
    pub fn remove_driver(&mut self, driver: DriverId) -> Result<Driver, LinkageError> {
        self.drivers
            .remove(driver)
            .ok_or(LinkageError::UnknownDriver(driver))
    }

    /// Adds a signed side-of-directed-line assembly monitor.
    ///
    /// # Errors
    ///
    /// Returns an error for any stale point feature.
    pub fn add_orientation_branch_monitor(
        &mut self,
        line_start: PointFeatureId,
        line_end: PointFeatureId,
        observed: PointFeatureId,
        sign: BranchSign,
    ) -> Result<BranchMonitorId, LinkageError> {
        self.require_point_feature(line_start)?;
        self.require_point_feature(line_end)?;
        self.require_point_feature(observed)?;
        Ok(self.branch_monitors.insert(BranchMonitor::Orientation {
            line_start,
            line_end,
            observed,
            sign,
        }))
    }

    /// Adds a signed displacement monitor along an origin body's axis.
    ///
    /// # Errors
    ///
    /// Returns an error for stale features or mismatched origin/axis ownership.
    pub fn add_directed_displacement_branch_monitor(
        &mut self,
        origin: PointFeatureId,
        measured: PointFeatureId,
        axis: AxisFeatureId,
        sign: BranchSign,
    ) -> Result<BranchMonitorId, LinkageError> {
        let origin_body = self.require_point_feature(origin)?.body;
        self.require_point_feature(measured)?;
        if self.require_axis_feature(axis)?.body != origin_body {
            return Err(LinkageError::LinearDriverFeatureBodyMismatch);
        }
        Ok(self
            .branch_monitors
            .insert(BranchMonitor::DirectedDisplacement {
                origin,
                measured,
                axis,
                sign,
            }))
    }

    #[must_use]
    pub fn branch_monitor(&self, monitor: BranchMonitorId) -> Option<&BranchMonitor> {
        self.branch_monitors.get(monitor)
    }

    pub fn branch_monitors(&self) -> impl Iterator<Item = (BranchMonitorId, &BranchMonitor)> {
        self.branch_monitors.iter()
    }

    /// Removes a branch monitor, leaving its stable ID stale.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown or stale ID.
    pub fn remove_branch_monitor(
        &mut self,
        monitor: BranchMonitorId,
    ) -> Result<BranchMonitor, LinkageError> {
        self.branch_monitors
            .remove(monitor)
            .ok_or(LinkageError::UnknownBranchMonitor(monitor))
    }

    pub(crate) fn require_body(&self, body: BodyId) -> Result<&RigidBody, LinkageError> {
        self.bodies.get(body).ok_or(LinkageError::UnknownBody(body))
    }

    pub(crate) fn require_point_feature(
        &self,
        feature: PointFeatureId,
    ) -> Result<&PointFeature, LinkageError> {
        self.point_features
            .get(feature)
            .ok_or(LinkageError::UnknownPointFeature(feature))
    }

    pub(crate) fn require_axis_feature(
        &self,
        feature: AxisFeatureId,
    ) -> Result<&AxisFeature, LinkageError> {
        self.axis_features
            .get(feature)
            .ok_or(LinkageError::UnknownAxisFeature(feature))
    }

    pub(crate) fn set_driver_target_accepted(
        &mut self,
        driver: DriverId,
        target: f64,
    ) -> Result<(), LinkageError> {
        validate_finite(target, "driver target")?;
        self.drivers
            .get_mut(driver)
            .ok_or(LinkageError::UnknownDriver(driver))?
            .target = target;
        Ok(())
    }

    fn require_distinct_point_bodies(
        &self,
        first: PointFeatureId,
        second: PointFeatureId,
    ) -> Result<(BodyId, BodyId), LinkageError> {
        let first_body = self.require_point_feature(first)?.body;
        let second_body = self.require_point_feature(second)?.body;
        if first_body == second_body {
            Err(LinkageError::RepeatedBody)
        } else {
            Ok((first_body, second_body))
        }
    }

    fn insert_joint(
        &mut self,
        label: impl Into<String>,
        kind: JointKind,
    ) -> Result<JointId, LinkageError> {
        let label = nonempty_label(label, "joint")?;
        let ordinal = self.joints.next_ordinal();
        Ok(self.joints.insert(Joint {
            label,
            kind,
            ordinal,
        }))
    }

    fn insert_driver(
        &mut self,
        label: impl Into<String>,
        kind: DriverKind,
        target: f64,
        max_continuation_step: f64,
    ) -> Result<DriverId, LinkageError> {
        validate_finite(target, "driver target")?;
        if !max_continuation_step.is_finite() || max_continuation_step <= 0.0 {
            return Err(LinkageError::InvalidContinuationStep(max_continuation_step));
        }
        let label = nonempty_label(label, "driver")?;
        let ordinal = self.drivers.next_ordinal();
        Ok(self.drivers.insert(Driver {
            label,
            kind,
            target,
            max_continuation_step,
            ordinal,
        }))
    }
}

fn joint_references_point(kind: JointKind, point: PointFeatureId) -> bool {
    match kind {
        JointKind::Revolute { first, second } | JointKind::Weld { first, second, .. } => {
            first == point || second == point
        }
        JointKind::Prismatic {
            first_anchor,
            second_anchor,
            ..
        } => first_anchor == point || second_anchor == point,
    }
}

fn joint_references_axis(kind: JointKind, axis: AxisFeatureId) -> bool {
    matches!(
        kind,
        JointKind::Prismatic {
            first_axis,
            second_axis,
            ..
        } if first_axis == axis || second_axis == axis
    )
}

fn driver_references_point(kind: DriverKind, point: PointFeatureId) -> bool {
    matches!(
        kind,
        DriverKind::Linear {
            origin,
            measured,
            ..
        } if origin == point || measured == point
    )
}

fn driver_references_axis(kind: DriverKind, axis: AxisFeatureId) -> bool {
    matches!(kind, DriverKind::Linear { guide_axis, .. } if guide_axis == axis)
}

fn monitor_references_point(monitor: BranchMonitor, point: PointFeatureId) -> bool {
    match monitor {
        BranchMonitor::Orientation {
            line_start,
            line_end,
            observed,
            ..
        } => line_start == point || line_end == point || observed == point,
        BranchMonitor::DirectedDisplacement {
            origin, measured, ..
        } => origin == point || measured == point,
    }
}

fn monitor_references_axis(monitor: BranchMonitor, axis: AxisFeatureId) -> bool {
    matches!(monitor, BranchMonitor::DirectedDisplacement { axis: id, .. } if id == axis)
}

pub(crate) fn validate_model_scale(model_scale: f64) -> Result<(), LinkageError> {
    if model_scale.is_finite() && model_scale > 0.0 {
        Ok(())
    } else {
        Err(LinkageError::InvalidModelScale(model_scale))
    }
}

pub(crate) fn validate_plane_frame(frame: PlaneFrame) -> Result<(), LinkageError> {
    let origin = frame.origin();
    let u = frame.u();
    let v = frame.v();
    if !origin.coords.iter().all(|value| value.is_finite())
        || !u.iter().all(|value| value.is_finite())
        || !v.iter().all(|value| value.is_finite())
    {
        return Err(LinkageError::InvalidPlaneFrame(
            "origin and axes must be finite",
        ));
    }
    let u_norm = u.x.hypot(u.y).hypot(u.z);
    let v_norm = v.x.hypot(v.y).hypot(v.z);
    if (u_norm - 1.0).abs() > FRAME_TOLERANCE || (v_norm - 1.0).abs() > FRAME_TOLERANCE {
        return Err(LinkageError::InvalidPlaneFrame("axes must be unit length"));
    }
    if u.dot(&v).abs() > FRAME_TOLERANCE {
        return Err(LinkageError::InvalidPlaneFrame("axes must be orthogonal"));
    }
    frame.validate().map_err(|_| {
        LinkageError::InvalidPlaneFrame("workplane must be finite with orthonormal axes")
    })
}

pub(crate) fn validate_pose(pose: Pose2, context: &'static str) -> Result<(), LinkageError> {
    pose.validate()
        .map_err(|_| LinkageError::NonFinitePose { context })
}

pub(crate) fn validate_point(
    point: Point2<f64>,
    context: &'static str,
) -> Result<(), LinkageError> {
    if point.x.is_finite() && point.y.is_finite() {
        Ok(())
    } else {
        Err(LinkageError::NonFinitePoint { context })
    }
}

pub(crate) fn validate_finite(value: f64, context: &'static str) -> Result<(), LinkageError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(LinkageError::NonFiniteValue { context, value })
    }
}

fn normalized_axis(
    axis: Vector2<f64>,
    context: &'static str,
) -> Result<Vector2<f64>, LinkageError> {
    let norm = axis.x.hypot(axis.y);
    if axis.x.is_finite() && axis.y.is_finite() && norm.is_finite() && norm > 0.0 {
        Ok(axis / norm)
    } else {
        Err(LinkageError::InvalidAxis { context })
    }
}

fn nonempty_label(label: impl Into<String>, kind: &'static str) -> Result<String, LinkageError> {
    let label = label.into();
    if label.trim().is_empty() {
        Err(LinkageError::EmptyLabel(kind))
    } else {
        Ok(label)
    }
}
