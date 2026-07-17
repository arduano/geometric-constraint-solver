use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;

use geosolve_core::{
    AuditBinding, AuditEvaluationStatus, AuditSnapshot, CoreError, HardValidity, Problem,
    ResidualBlock, ResidualCategory, ResidualId, ResidualRowAudit, SessionError, SolveReport,
    SolveSession, SolverConfig, SourceConstraint, SourceConstraintId, VariableBlock, VariableId,
    VariableKind, VariableValue,
};
use geosolve_geometry::{Frame3, GeometryError, Point3, Pose3};
use thiserror::Error;

use crate::spatial_residuals::{
    SpatialBallResidual, SpatialFixedFrameResidual, SpatialRevoluteResidual,
};

const ORIENTATION_BRANCH_MARGIN: f64 = 1.0e-3;
const SPATIAL_ACCEPTANCE_TOLERANCE: f64 = 1.0e-9;

macro_rules! spatial_id {
    ($name:ident, $description:literal) => {
        #[doc = $description]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn as_u64(self) -> u64 {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

spatial_id!(SpatialBodyId, "Opaque spatial rigid-body identity.");
spatial_id!(
    SpatialPointFeatureId,
    "Opaque body-local spatial point-feature identity."
);
spatial_id!(
    SpatialFrameFeatureId,
    "Opaque body-local spatial frame-feature identity."
);
spatial_id!(SpatialSourceId, "Opaque spatial physical-source identity.");

/// Construction, compilation, gauge, solve, or independent-validation failure.
#[derive(Debug, Error)]
pub enum SpatialAssemblyError {
    #[error("spatial model scale must be positive and finite, got {value}")]
    InvalidModelScale { value: f64 },
    #[error("spatial {field} label must not be empty")]
    InvalidLabel { field: &'static str },
    #[error("invalid spatial assembly field {field}: {message}")]
    InvalidField {
        field: &'static str,
        message: String,
    },
    #[error("unknown spatial body reference {0}")]
    UnknownBody(SpatialBodyId),
    #[error("unknown spatial point-feature reference {0}")]
    UnknownPointFeature(SpatialPointFeatureId),
    #[error("unknown spatial frame-feature reference {0}")]
    UnknownFrameFeature(SpatialFrameFeatureId),
    #[error("unknown spatial source reference {0}")]
    UnknownSource(SpatialSourceId),
    #[error("spatial body {0} is physically grounded more than once")]
    DuplicateGround(SpatialBodyId),
    #[error("spatial joint endpoints must belong to different bodies, got {0}")]
    SameBodyJointEndpoints(SpatialBodyId),
    #[error("spatial ID space is exhausted")]
    IdExhausted,
    #[error("invalid spatial gauge policy: {0}")]
    InvalidGaugePolicy(String),
    #[error("spatial gauge certification failed: {0}")]
    GaugeCertification(String),
    #[error("stale spatial assembly revision: expected {expected}, actual {actual}")]
    StaleRevision { expected: u64, actual: u64 },
    #[error("spatial assembly revision is exhausted")]
    RevisionExhausted,
    #[error("spatial independent validation failed: {0}")]
    IndependentValidation(String),
    #[error("initial spatial assembly was rejected: {0}")]
    InitialRejected(String),
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error(transparent)]
    Session(#[from] SessionError),
    #[error(transparent)]
    Geometry(#[from] GeometryError),
}

/// Explicit directed-axis relationship retained by a spatial revolute joint.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SpatialAxisParity {
    Aligned,
    Opposed,
}

impl SpatialAxisParity {
    #[must_use]
    pub const fn multiplier(self) -> f64 {
        match self {
            Self::Aligned => 1.0,
            Self::Opposed => -1.0,
        }
    }
}

/// Numerical coordinate policy, separate from physical grounding.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum SpatialGaugePolicy {
    /// Fix the lowest body ID in each certified floating component privately.
    #[default]
    LowestPersistentBody,
    /// Select exactly one reference for every floating component.
    ExplicitReferences { bodies: Vec<SpatialBodyId> },
}

/// One spatial rigid body and its current accepted or staged pose guess.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialBody {
    id: SpatialBodyId,
    label: String,
    pose_guess: Pose3,
}

impl SpatialBody {
    #[must_use]
    pub const fn id(&self) -> SpatialBodyId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn pose_guess(&self) -> Pose3 {
        self.pose_guess
    }
}

/// One body-local point feature.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialPointFeature {
    id: SpatialPointFeatureId,
    label: String,
    body: SpatialBodyId,
    local_point: Point3<f64>,
}

impl SpatialPointFeature {
    #[must_use]
    pub const fn id(&self) -> SpatialPointFeatureId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> SpatialBodyId {
        self.body
    }

    #[must_use]
    pub const fn local_point(&self) -> Point3<f64> {
        self.local_point
    }
}

/// One validated body-local coordinate frame.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialFrameFeature {
    id: SpatialFrameFeatureId,
    label: String,
    body: SpatialBodyId,
    local_frame: Frame3,
}

impl SpatialFrameFeature {
    #[must_use]
    pub const fn id(&self) -> SpatialFrameFeatureId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn body(&self) -> SpatialBodyId {
        self.body
    }

    #[must_use]
    pub const fn local_frame(&self) -> Frame3 {
        self.local_frame
    }
}

/// One physical spatial equation source.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialSourceKind {
    PhysicalGround {
        body: SpatialBodyId,
        target_pose: Pose3,
    },
    BallJoint {
        first: SpatialPointFeatureId,
        second: SpatialPointFeatureId,
    },
    FixedFrame {
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
    },
    RevoluteJoint {
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
        parity: SpatialAxisParity,
    },
}

/// One physical source in deterministic insertion order.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialSource {
    id: SpatialSourceId,
    label: String,
    kind: SpatialSourceKind,
}

impl SpatialSource {
    #[must_use]
    pub const fn id(&self) -> SpatialSourceId {
        self.id
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn kind(&self) -> SpatialSourceKind {
        self.kind
    }

    #[must_use]
    pub const fn definition(&self) -> SpatialSourceKind {
        self.kind
    }
}

/// Minimal in-memory spatial assembly definition and accepted pose state.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialAssembly {
    model_scale: f64,
    revision: u64,
    next_id: u64,
    gauge_policy: SpatialGaugePolicy,
    bodies: Vec<SpatialBody>,
    point_features: Vec<SpatialPointFeature>,
    frame_features: Vec<SpatialFrameFeature>,
    sources: Vec<SpatialSource>,
}

impl SpatialAssembly {
    /// Creates an empty spatial assembly.
    ///
    /// # Errors
    ///
    /// Returns an error unless `model_scale` is positive and finite.
    pub fn new(model_scale: f64) -> Result<Self, SpatialAssemblyError> {
        validate_model_scale(model_scale)?;
        Ok(Self {
            model_scale,
            revision: 0,
            next_id: 1,
            gauge_policy: SpatialGaugePolicy::default(),
            bodies: Vec::new(),
            point_features: Vec::new(),
            frame_features: Vec::new(),
            sources: Vec::new(),
        })
    }

    #[must_use]
    pub const fn model_scale(&self) -> f64 {
        self.model_scale
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    #[must_use]
    pub const fn gauge_policy(&self) -> &SpatialGaugePolicy {
        &self.gauge_policy
    }

    #[must_use]
    pub fn bodies(&self) -> &[SpatialBody] {
        &self.bodies
    }

    #[must_use]
    pub fn point_features(&self) -> &[SpatialPointFeature] {
        &self.point_features
    }

    #[must_use]
    pub fn frame_features(&self) -> &[SpatialFrameFeature] {
        &self.frame_features
    }

    #[must_use]
    pub fn sources(&self) -> &[SpatialSource] {
        &self.sources
    }

    #[must_use]
    pub fn body(&self, id: SpatialBodyId) -> Option<&SpatialBody> {
        self.bodies.iter().find(|body| body.id == id)
    }

    #[must_use]
    pub fn point_feature(&self, id: SpatialPointFeatureId) -> Option<&SpatialPointFeature> {
        self.point_features.iter().find(|feature| feature.id == id)
    }

    #[must_use]
    pub fn frame_feature(&self, id: SpatialFrameFeatureId) -> Option<&SpatialFrameFeature> {
        self.frame_features.iter().find(|feature| feature.id == id)
    }

    #[must_use]
    pub fn source(&self, id: SpatialSourceId) -> Option<&SpatialSource> {
        self.sources.iter().find(|source| source.id == id)
    }

    /// Adds one body with a finite manifold pose guess.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, invalid pose, or exhausted ID space.
    pub fn add_body(
        &mut self,
        label: impl Into<String>,
        pose_guess: Pose3,
    ) -> Result<SpatialBodyId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "body")?;
        validate_pose(pose_guess)?;
        let id = SpatialBodyId(self.allocate_id()?);
        self.bodies.push(SpatialBody {
            id,
            label,
            pose_guess,
        });
        Ok(id)
    }

    /// Adds one finite body-local point feature.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale body, non-finite point, or exhausted ID space.
    pub fn add_point_feature(
        &mut self,
        label: impl Into<String>,
        body: SpatialBodyId,
        local_point: Point3<f64>,
    ) -> Result<SpatialPointFeatureId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "point feature")?;
        self.require_body(body)?;
        validate_point(local_point, "point_feature.local_point")?;
        let id = SpatialPointFeatureId(self.allocate_id()?);
        self.point_features.push(SpatialPointFeature {
            id,
            label,
            body,
            local_point,
        });
        Ok(id)
    }

    /// Adds one validated body-local frame feature.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale body, invalid frame, or exhausted ID space.
    pub fn add_frame_feature(
        &mut self,
        label: impl Into<String>,
        body: SpatialBodyId,
        local_frame: Frame3,
    ) -> Result<SpatialFrameFeatureId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "frame feature")?;
        self.require_body(body)?;
        let local_frame = revalidate_frame(local_frame)?;
        let id = SpatialFrameFeatureId(self.allocate_id()?);
        self.frame_features.push(SpatialFrameFeature {
            id,
            label,
            body,
            local_frame,
        });
        Ok(id)
    }

    /// Adds a physical fixed-pose source whose target is captured immediately.
    ///
    /// # Errors
    ///
    /// Rejects an empty label, stale body, duplicate physical ground, or exhausted ID space.
    pub fn add_physical_ground(
        &mut self,
        label: impl Into<String>,
        body: SpatialBodyId,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "physical ground")?;
        let target_pose = self.require_body(body)?.pose_guess;
        if self.sources.iter().any(|source| {
            matches!(source.kind, SpatialSourceKind::PhysicalGround { body: existing, .. } if existing == body)
        }) {
            return Err(SpatialAssemblyError::DuplicateGround(body));
        }
        self.add_source_record(
            label,
            SpatialSourceKind::PhysicalGround { body, target_pose },
        )
    }

    /// Adds a coincident-point ball joint.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_ball_joint(
        &mut self,
        label: impl Into<String>,
        first: SpatialPointFeatureId,
        second: SpatialPointFeatureId,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "ball joint")?;
        let first_body = self.require_point_feature(first)?.body;
        let second_body = self.require_point_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(label, SpatialSourceKind::BallJoint { first, second })
    }

    /// Adds a coincident, identically oriented fixed-frame relationship.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_fixed_frame(
        &mut self,
        label: impl Into<String>,
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "fixed frame")?;
        let first_body = self.require_frame_feature(first)?.body;
        let second_body = self.require_frame_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(label, SpatialSourceKind::FixedFrame { first, second })
    }

    /// Adds a revolute joint about the local frame z axes with explicit parity.
    ///
    /// # Errors
    ///
    /// Rejects empty labels, stale features, same-body endpoints, or exhausted ID space.
    pub fn add_revolute_joint(
        &mut self,
        label: impl Into<String>,
        first: SpatialFrameFeatureId,
        second: SpatialFrameFeatureId,
        parity: SpatialAxisParity,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let label = label.into();
        validate_label(&label, "revolute joint")?;
        let first_body = self.require_frame_feature(first)?.body;
        let second_body = self.require_frame_feature(second)?.body;
        require_distinct_bodies(first_body, second_body)?;
        self.add_source_record(
            label,
            SpatialSourceKind::RevoluteJoint {
                first,
                second,
                parity,
            },
        )
    }

    /// Compiles only physical equations in deterministic insertion order.
    ///
    /// # Errors
    ///
    /// Rejects invalid assembly state or any core declaration failure.
    pub fn compile(&self) -> Result<CompiledSpatialAssembly, SpatialAssemblyError> {
        self.validate_structure()?;
        self.compile_validated()
    }

    #[allow(clippy::too_many_lines)]
    fn compile_validated(&self) -> Result<CompiledSpatialAssembly, SpatialAssemblyError> {
        let mut problem = Problem::new();
        let mut body_variables = Vec::with_capacity(self.bodies.len());
        let mut variables = HashMap::with_capacity(self.bodies.len());
        let pose_scales = [
            self.model_scale,
            self.model_scale,
            self.model_scale,
            1.0,
            1.0,
            1.0,
        ];
        for body in &self.bodies {
            let variable_id = problem.add_variable(VariableBlock::pose3(
                body.pose_guess.ambient(),
                pose_scales,
            )?);
            body_variables.push(SpatialBodyVariableMapping {
                body_id: body.id,
                variable_id,
            });
            variables.insert(body.id, variable_id);
        }

        let mut source_mappings = Vec::with_capacity(self.sources.len());
        for source in &self.sources {
            let core_source_id = problem.add_source(SourceConstraint::new(&source.label)?);
            let residual = match source.kind {
                SpatialSourceKind::PhysicalGround { body, target_pose } => {
                    let variable = variable_for_body(&variables, body)?;
                    let residual = ResidualBlock::fixed_variable(
                        core_source_id,
                        variable,
                        VariableValue::Pose3(target_pose.ambient()),
                        pose_scales.to_vec(),
                        ground_audit_rows(body, target_pose),
                    )?;
                    let residual_id = problem.add_residual(residual)?;
                    problem.declare_fixed_variable(
                        variable,
                        VariableValue::Pose3(target_pose.ambient()),
                        residual_id,
                    )?;
                    residual_id
                }
                SpatialSourceKind::BallJoint { first, second } => {
                    let first_feature = self.require_point_feature(first)?;
                    let second_feature = self.require_point_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        3,
                        vec![self.model_scale; 3],
                        point_joint_audit_rows("ball joint", first_feature, second_feature),
                        SpatialBallResidual {
                            first_local: point_array(first_feature.local_point),
                            second_local: point_array(second_feature.local_point),
                        },
                    )?)?
                }
                SpatialSourceKind::FixedFrame { first, second } => {
                    let first_feature = self.require_frame_feature(first)?;
                    let second_feature = self.require_frame_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        6,
                        vec![
                            self.model_scale,
                            self.model_scale,
                            self.model_scale,
                            1.0,
                            1.0,
                            1.0,
                        ],
                        frame_joint_audit_rows(
                            "fixed frame",
                            first_feature,
                            second_feature,
                            None,
                            &[
                                "world origin x difference",
                                "world origin y difference",
                                "world origin z difference",
                                "first y dot second x",
                                "first z dot second x",
                                "first z dot second y",
                            ],
                        ),
                        SpatialFixedFrameResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                        },
                    )?)?
                }
                SpatialSourceKind::RevoluteJoint {
                    first,
                    second,
                    parity,
                } => {
                    let first_feature = self.require_frame_feature(first)?;
                    let second_feature = self.require_frame_feature(second)?;
                    problem.add_residual(ResidualBlock::new(
                        core_source_id,
                        ResidualCategory::Hard,
                        vec![
                            variable_for_body(&variables, first_feature.body)?,
                            variable_for_body(&variables, second_feature.body)?,
                        ],
                        5,
                        vec![
                            self.model_scale,
                            self.model_scale,
                            self.model_scale,
                            1.0,
                            1.0,
                        ],
                        frame_joint_audit_rows(
                            "revolute joint",
                            first_feature,
                            second_feature,
                            Some(parity),
                            &[
                                "world origin x difference",
                                "world origin y difference",
                                "world origin z difference",
                                "first x dot parity-adjusted second z",
                                "first y dot parity-adjusted second z",
                            ],
                        ),
                        SpatialRevoluteResidual {
                            first_local: first_feature.local_frame,
                            second_local: second_feature.local_frame,
                            parity_multiplier: parity.multiplier(),
                        },
                    )?)?
                }
            };
            source_mappings.push(SpatialSourceMapping {
                source: source.id,
                source_label: source.label.clone(),
                core_source_id,
                residual_ids: vec![residual],
            });
        }

        Ok(CompiledSpatialAssembly {
            problem,
            body_variables,
            source_mappings,
            point_features: self.point_features.clone(),
            frame_features: self.frame_features.clone(),
        })
    }

    fn validate_structure(&self) -> Result<(), SpatialAssemblyError> {
        validate_model_scale(self.model_scale)?;
        let mut raw_ids = BTreeSet::new();
        for body in &self.bodies {
            validate_label(&body.label, "body")?;
            validate_pose(body.pose_guess)?;
            require_unique_raw_id(&mut raw_ids, body.id.0)?;
        }
        for feature in &self.point_features {
            validate_label(&feature.label, "point feature")?;
            self.require_body(feature.body)?;
            validate_point(feature.local_point, "point_feature.local_point")?;
            require_unique_raw_id(&mut raw_ids, feature.id.0)?;
        }
        for feature in &self.frame_features {
            validate_label(&feature.label, "frame feature")?;
            self.require_body(feature.body)?;
            revalidate_frame(feature.local_frame)?;
            require_unique_raw_id(&mut raw_ids, feature.id.0)?;
        }
        let mut grounded = BTreeSet::new();
        for source in &self.sources {
            validate_label(&source.label, "source")?;
            require_unique_raw_id(&mut raw_ids, source.id.0)?;
            match source.kind {
                SpatialSourceKind::PhysicalGround { body, target_pose } => {
                    self.require_body(body)?;
                    validate_pose(target_pose)?;
                    if !grounded.insert(body) {
                        return Err(SpatialAssemblyError::DuplicateGround(body));
                    }
                }
                SpatialSourceKind::BallJoint { first, second } => {
                    let first = self.require_point_feature(first)?;
                    let second = self.require_point_feature(second)?;
                    require_distinct_bodies(first.body, second.body)?;
                }
                SpatialSourceKind::FixedFrame { first, second }
                | SpatialSourceKind::RevoluteJoint { first, second, .. } => {
                    let first = self.require_frame_feature(first)?;
                    let second = self.require_frame_feature(second)?;
                    require_distinct_bodies(first.body, second.body)?;
                }
            }
        }
        if raw_ids
            .iter()
            .next_back()
            .is_some_and(|maximum| self.next_id <= *maximum)
        {
            return invalid_field("next_id", "must exceed every allocated ID");
        }
        let components = certified_components(self)?;
        resolve_gauge_references(&self.gauge_policy, &components)?;
        Ok(())
    }

    fn allocate_id(&mut self) -> Result<u64, SpatialAssemblyError> {
        let id = self.next_id;
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(SpatialAssemblyError::IdExhausted)?;
        Ok(id)
    }

    fn add_source_record(
        &mut self,
        label: String,
        kind: SpatialSourceKind,
    ) -> Result<SpatialSourceId, SpatialAssemblyError> {
        let id = SpatialSourceId(self.allocate_id()?);
        self.sources.push(SpatialSource { id, label, kind });
        Ok(id)
    }

    fn require_body(&self, id: SpatialBodyId) -> Result<&SpatialBody, SpatialAssemblyError> {
        self.body(id).ok_or(SpatialAssemblyError::UnknownBody(id))
    }

    fn require_point_feature(
        &self,
        id: SpatialPointFeatureId,
    ) -> Result<&SpatialPointFeature, SpatialAssemblyError> {
        self.point_feature(id)
            .ok_or(SpatialAssemblyError::UnknownPointFeature(id))
    }

    fn require_frame_feature(
        &self,
        id: SpatialFrameFeatureId,
    ) -> Result<&SpatialFrameFeature, SpatialAssemblyError> {
        self.frame_feature(id)
            .ok_or(SpatialAssemblyError::UnknownFrameFeature(id))
    }
}

/// Exact mapping from one physical spatial source to core identity and rows.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialSourceMapping {
    pub source: SpatialSourceId,
    pub source_label: String,
    pub core_source_id: SourceConstraintId,
    pub residual_ids: Vec<ResidualId>,
}

/// Exact spatial body-to-Pose3-variable relationship in body insertion order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpatialBodyVariableMapping {
    pub body_id: SpatialBodyId,
    pub variable_id: VariableId,
}

/// Read-only compiled physical spatial problem and exact domain mappings.
#[derive(Clone, Debug)]
pub struct CompiledSpatialAssembly {
    problem: Problem,
    body_variables: Vec<SpatialBodyVariableMapping>,
    source_mappings: Vec<SpatialSourceMapping>,
    point_features: Vec<SpatialPointFeature>,
    frame_features: Vec<SpatialFrameFeature>,
}

impl CompiledSpatialAssembly {
    #[must_use]
    pub fn body_variables(&self) -> &[SpatialBodyVariableMapping] {
        &self.body_variables
    }

    #[must_use]
    pub fn source_mappings(&self) -> &[SpatialSourceMapping] {
        &self.source_mappings
    }

    #[must_use]
    pub fn variable_for_body(&self, body: SpatialBodyId) -> Option<VariableId> {
        self.body_variables
            .iter()
            .find_map(|mapping| (mapping.body_id == body).then_some(mapping.variable_id))
    }

    #[must_use]
    pub fn source_mapping(&self, source: SpatialSourceId) -> Option<&SpatialSourceMapping> {
        self.source_mappings
            .iter()
            .find(|mapping| mapping.source == source)
    }

    /// Checks every physical residual Jacobian against central right-retraction differences.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid finite-difference step or failed residual evaluation.
    pub fn check_jacobians(
        &self,
        normalized_step: f64,
    ) -> Result<geosolve_core::JacobianCheckReport, SpatialAssemblyError> {
        Ok(self.problem.check_jacobians(normalized_step)?)
    }

    pub(crate) fn add_numerical_pose_gauge(
        &mut self,
        body: SpatialBodyId,
        target: Pose3,
        model_scale: f64,
    ) -> Result<(), SpatialAssemblyError> {
        validate_pose(target)?;
        validate_model_scale(model_scale)?;
        let variable = self
            .variable_for_body(body)
            .ok_or(SpatialAssemblyError::UnknownBody(body))?;
        let source = self.problem.add_source(SourceConstraint::new(format!(
            "private spatial numerical gauge for body {body}"
        ))?);
        let value = VariableValue::Pose3(target.ambient());
        let residual = self.problem.add_residual(ResidualBlock::fixed_variable(
            source,
            variable,
            value,
            vec![model_scale, model_scale, model_scale, 1.0, 1.0, 1.0],
            private_gauge_audit_rows(body),
        )?)?;
        self.problem
            .declare_fixed_variable(variable, value, residual)?;
        Ok(())
    }
}

/// One solved spatial body pose in deterministic body order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialSolvedBody {
    pub body_id: SpatialBodyId,
    pub pose: Pose3,
}

/// One transformed spatial point feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialTransformedPointFeature {
    pub feature_id: SpatialPointFeatureId,
    pub body_id: SpatialBodyId,
    pub world: Point3<f64>,
}

/// One transformed spatial frame feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialTransformedFrameFeature {
    pub feature_id: SpatialFrameFeatureId,
    pub body_id: SpatialBodyId,
    pub world: Frame3,
}

/// Accepted finite body and transformed feature geometry.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialGeometry {
    pub bodies: Vec<SpatialSolvedBody>,
    pub points: Vec<SpatialTransformedPointFeature>,
    pub frames: Vec<SpatialTransformedFrameFeature>,
}

impl SpatialGeometry {
    #[must_use]
    pub fn body_pose(&self, body: SpatialBodyId) -> Option<Pose3> {
        self.bodies
            .iter()
            .find_map(|item| (item.body_id == body).then_some(item.pose))
    }

    #[must_use]
    pub fn world_point(&self, feature: SpatialPointFeatureId) -> Option<Point3<f64>> {
        self.points
            .iter()
            .find_map(|item| (item.feature_id == feature).then_some(item.world))
    }

    #[must_use]
    pub fn point(&self, feature: SpatialPointFeatureId) -> Option<Point3<f64>> {
        self.world_point(feature)
    }

    #[must_use]
    pub fn world_frame(&self, feature: SpatialFrameFeatureId) -> Option<Frame3> {
        self.frames
            .iter()
            .find_map(|item| (item.feature_id == feature).then_some(item.world))
    }

    #[must_use]
    pub fn frame(&self, feature: SpatialFrameFeatureId) -> Option<Frame3> {
        self.world_frame(feature)
    }
}

/// Certification of the common-left world action for one domain component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpatialWorldActionCertification {
    FloatingSe3,
    PhysicallyGrounded,
}

/// Private numerical reference selected for one floating component.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpatialGaugeReference {
    pub body: SpatialBodyId,
    pub target_pose: Pose3,
}

/// Gauge and equality mobility for one deterministic spatial domain component.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialComponentGaugeReport {
    pub component_index: usize,
    pub bodies: Vec<SpatialBodyId>,
    pub sources: Vec<SpatialSourceId>,
    pub core_component_indices: Vec<usize>,
    pub numerical_equality_right_nullity: usize,
    pub gauge_dof: usize,
    pub internal_mobility: usize,
    pub world_action: SpatialWorldActionCertification,
    pub physical_ground_sources: Vec<SpatialSourceId>,
    pub numerical_reference: Option<SpatialGaugeReference>,
}

/// Domain-certified split of physical equality mobility into gauge and internal motion.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialGaugeReport {
    pub numerical_equality_right_nullity: usize,
    pub gauge_dof: usize,
    pub internal_mobility: usize,
    pub components: Vec<SpatialComponentGaugeReport>,
}

/// Independently accepted spatial solve state and its physical core evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialSolveResult {
    pub geometry: SpatialGeometry,
    pub display_audit: AuditSnapshot,
    pub source_mappings: Vec<SpatialSourceMapping>,
    pub core_report: SolveReport,
    pub acceptance_hard_residual_max: f64,
}

/// One revision-checked spatial assembly edit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpatialPatch {
    BodyPoseGuess {
        body: SpatialBodyId,
        pose: Pose3,
    },
    PointLocal {
        feature: SpatialPointFeatureId,
        local_point: Point3<f64>,
    },
    FrameLocal {
        feature: SpatialFrameFeatureId,
        local_frame: Frame3,
    },
}

/// Accepted spatial assembly plus its authoritative ungauged physical core session.
#[derive(Clone, Debug)]
pub struct SpatialAssemblySession {
    assembly: SpatialAssembly,
    core_session: SolveSession,
    body_variables: Vec<SpatialBodyVariableMapping>,
    source_mappings: Vec<SpatialSourceMapping>,
    accepted_result: SpatialSolveResult,
    gauge_report: SpatialGaugeReport,
    config: SolverConfig,
}

impl SpatialAssemblySession {
    /// Solves through a private-gauge scratch problem, then publishes a separately
    /// solved and independently validated ungauged physical problem.
    ///
    /// # Errors
    ///
    /// Rejects invalid assembly/gauge data, unsuccessful core solves, invalid
    /// rank, non-finite geometry/audit, excessive residuals, or branch failures.
    pub fn new(
        mut assembly: SpatialAssembly,
        config: SolverConfig,
    ) -> Result<Self, SpatialAssemblyError> {
        assembly.validate_structure()?;
        let components = certified_components(&assembly)?;
        let references = resolve_gauge_references(&assembly.gauge_policy, &components)?;

        let mut scratch = assembly.compile_validated()?;
        for body in references.iter().flatten() {
            let target = assembly.require_body(*body)?.pose_guess;
            scratch.add_numerical_pose_gauge(*body, target, assembly.model_scale)?;
        }
        let CompiledSpatialAssembly {
            problem,
            body_variables,
            source_mappings,
            point_features,
            frame_features,
        } = scratch;
        let scratch_session = accepted_session(problem, config, "private-gauge scratch solve")?;
        let scratch_geometry = solved_geometry_from_problem(
            scratch_session.problem(),
            &body_variables,
            &point_features,
            &frame_features,
        )?;
        validate_physical_candidate(
            &assembly,
            &scratch_geometry,
            &scratch_session,
            &source_mappings,
            config,
        )?;
        project_geometry(&mut assembly, &scratch_geometry)?;

        let physical = assembly.compile_validated()?;
        let CompiledSpatialAssembly {
            problem,
            body_variables,
            source_mappings,
            point_features,
            frame_features,
        } = physical;
        let core_session = accepted_session(problem, config, "ungauged physical solve")?;
        let geometry = solved_geometry_from_problem(
            core_session.problem(),
            &body_variables,
            &point_features,
            &frame_features,
        )?;
        let acceptance_hard_residual_max = validate_physical_candidate(
            &assembly,
            &geometry,
            &core_session,
            &source_mappings,
            config,
        )?;
        project_geometry(&mut assembly, &geometry)?;
        let gauge_report = build_gauge_report(
            &assembly,
            &components,
            &references,
            &body_variables,
            &source_mappings,
            &core_session,
        )?;
        let core_report = core_session.report().clone();
        let accepted_result = SpatialSolveResult {
            geometry,
            display_audit: core_report.audit.clone(),
            source_mappings: source_mappings.clone(),
            core_report,
            acceptance_hard_residual_max,
        };
        Ok(Self {
            assembly,
            core_session,
            body_variables,
            source_mappings,
            accepted_result,
            gauge_report,
            config,
        })
    }

    #[must_use]
    pub const fn assembly(&self) -> &SpatialAssembly {
        &self.assembly
    }

    #[must_use]
    pub const fn revision(&self) -> u64 {
        self.assembly.revision
    }

    #[must_use]
    pub const fn core_session(&self) -> &SolveSession {
        &self.core_session
    }

    #[must_use]
    pub const fn accepted_result(&self) -> &SpatialSolveResult {
        &self.accepted_result
    }

    #[must_use]
    pub const fn gauge_report(&self) -> &SpatialGaugeReport {
        &self.gauge_report
    }

    #[must_use]
    pub fn body_variables(&self) -> &[SpatialBodyVariableMapping] {
        &self.body_variables
    }

    #[must_use]
    pub fn source_mappings(&self) -> &[SpatialSourceMapping] {
        &self.source_mappings
    }

    /// Applies one edit by fully rebuilding a candidate and swapping only on acceptance.
    ///
    /// # Errors
    ///
    /// Rejects stale revisions, stale object IDs, invalid geometry, revision
    /// exhaustion, or any failed solve/validation while retaining all accepted views.
    pub fn apply_patch(
        &mut self,
        expected_revision: u64,
        patch: SpatialPatch,
    ) -> Result<&SpatialSolveResult, SpatialAssemblyError> {
        self.require_revision(expected_revision)?;
        let mut candidate = self.assembly.clone();
        match patch {
            SpatialPatch::BodyPoseGuess { body, pose } => {
                validate_pose(pose)?;
                candidate
                    .bodies
                    .iter_mut()
                    .find(|candidate| candidate.id == body)
                    .ok_or(SpatialAssemblyError::UnknownBody(body))?
                    .pose_guess = pose;
            }
            SpatialPatch::PointLocal {
                feature,
                local_point,
            } => {
                validate_point(local_point, "patch.point_local")?;
                candidate
                    .point_features
                    .iter_mut()
                    .find(|candidate| candidate.id == feature)
                    .ok_or(SpatialAssemblyError::UnknownPointFeature(feature))?
                    .local_point = local_point;
            }
            SpatialPatch::FrameLocal {
                feature,
                local_frame,
            } => {
                let local_frame = revalidate_frame(local_frame)?;
                candidate
                    .frame_features
                    .iter_mut()
                    .find(|candidate| candidate.id == feature)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(feature))?
                    .local_frame = local_frame;
            }
        }
        candidate.revision = expected_revision
            .checked_add(1)
            .ok_or(SpatialAssemblyError::RevisionExhausted)?;
        let replacement = Self::new(candidate, self.config)?;
        *self = replacement;
        Ok(&self.accepted_result)
    }

    /// Replaces only numerical gauge metadata through the same atomic rebuild path.
    ///
    /// # Errors
    ///
    /// Rejects stale revisions; duplicate, missing, stale, or grounded references;
    /// revision exhaustion; or any failed solve/validation without changing this session.
    pub fn set_gauge_policy(
        &mut self,
        expected_revision: u64,
        policy: SpatialGaugePolicy,
    ) -> Result<&SpatialSolveResult, SpatialAssemblyError> {
        self.require_revision(expected_revision)?;
        let mut candidate = self.assembly.clone();
        candidate.gauge_policy = policy;
        candidate.revision = expected_revision
            .checked_add(1)
            .ok_or(SpatialAssemblyError::RevisionExhausted)?;
        let replacement = Self::new(candidate, self.config)?;
        *self = replacement;
        Ok(&self.accepted_result)
    }

    fn require_revision(&self, expected: u64) -> Result<(), SpatialAssemblyError> {
        let actual = self.assembly.revision;
        if expected == actual {
            Ok(())
        } else {
            Err(SpatialAssemblyError::StaleRevision { expected, actual })
        }
    }
}

#[derive(Clone, Debug)]
struct CertifiedSpatialComponent {
    bodies: Vec<SpatialBodyId>,
    sources: Vec<SpatialSourceId>,
    physical_ground_sources: Vec<SpatialSourceId>,
}

fn certified_components(
    assembly: &SpatialAssembly,
) -> Result<Vec<CertifiedSpatialComponent>, SpatialAssemblyError> {
    let bodies = assembly
        .bodies
        .iter()
        .map(|body| body.id)
        .collect::<Vec<_>>();
    let body_indices = bodies
        .iter()
        .enumerate()
        .map(|(index, body)| (*body, index))
        .collect::<HashMap<_, _>>();
    let mut parents = (0..bodies.len()).collect::<Vec<_>>();
    for source in &assembly.sources {
        let incident = source_bodies(assembly, source)?;
        if let Some((&first, rest)) = incident.split_first() {
            let first = *body_indices
                .get(&first)
                .ok_or(SpatialAssemblyError::UnknownBody(first))?;
            for body in rest {
                let next = *body_indices
                    .get(body)
                    .ok_or(SpatialAssemblyError::UnknownBody(*body))?;
                union_roots(&mut parents, first, next);
            }
        }
    }

    let mut groups = BTreeMap::<SpatialBodyId, Vec<SpatialBodyId>>::new();
    for (index, body) in bodies.iter().copied().enumerate() {
        let root = find_root(&mut parents, index);
        groups.entry(bodies[root]).or_default().push(body);
    }
    let mut components = groups
        .into_values()
        .map(|mut component_bodies| {
            component_bodies.sort_unstable();
            CertifiedSpatialComponent {
                bodies: component_bodies,
                sources: Vec::new(),
                physical_ground_sources: Vec::new(),
            }
        })
        .collect::<Vec<_>>();
    components.sort_by_key(|component| component.bodies[0]);
    let component_for_body = components
        .iter()
        .enumerate()
        .flat_map(|(index, component)| component.bodies.iter().map(move |body| (*body, index)))
        .collect::<HashMap<_, _>>();
    for source in &assembly.sources {
        let incident = source_bodies(assembly, source)?;
        let body = incident.first().copied().ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "source {} has no incident body",
                source.id
            ))
        })?;
        let component_index = *component_for_body
            .get(&body)
            .ok_or(SpatialAssemblyError::UnknownBody(body))?;
        components[component_index].sources.push(source.id);
        if matches!(source.kind, SpatialSourceKind::PhysicalGround { .. }) {
            components[component_index]
                .physical_ground_sources
                .push(source.id);
        }
    }
    Ok(components)
}

fn source_bodies(
    assembly: &SpatialAssembly,
    source: &SpatialSource,
) -> Result<Vec<SpatialBodyId>, SpatialAssemblyError> {
    let mut bodies = match source.kind {
        SpatialSourceKind::PhysicalGround { body, .. } => vec![body],
        SpatialSourceKind::BallJoint { first, second } => vec![
            assembly.require_point_feature(first)?.body,
            assembly.require_point_feature(second)?.body,
        ],
        SpatialSourceKind::FixedFrame { first, second }
        | SpatialSourceKind::RevoluteJoint { first, second, .. } => vec![
            assembly.require_frame_feature(first)?.body,
            assembly.require_frame_feature(second)?.body,
        ],
    };
    bodies.sort_unstable();
    bodies.dedup();
    Ok(bodies)
}

fn resolve_gauge_references(
    policy: &SpatialGaugePolicy,
    components: &[CertifiedSpatialComponent],
) -> Result<Vec<Option<SpatialBodyId>>, SpatialAssemblyError> {
    match policy {
        SpatialGaugePolicy::LowestPersistentBody => Ok(components
            .iter()
            .map(|component| {
                component
                    .physical_ground_sources
                    .is_empty()
                    .then_some(component.bodies[0])
            })
            .collect()),
        SpatialGaugePolicy::ExplicitReferences { bodies } => {
            if bodies.iter().copied().collect::<BTreeSet<_>>().len() != bodies.len() {
                return Err(SpatialAssemblyError::InvalidGaugePolicy(
                    "explicit references must be unique".to_owned(),
                ));
            }
            let all_bodies = components
                .iter()
                .flat_map(|component| component.bodies.iter().copied())
                .collect::<BTreeSet<_>>();
            if let Some(body) = bodies.iter().find(|body| !all_bodies.contains(body)) {
                return Err(SpatialAssemblyError::InvalidGaugePolicy(format!(
                    "unknown explicit body reference {body}"
                )));
            }
            components
                .iter()
                .map(|component| {
                    let selected = bodies
                        .iter()
                        .copied()
                        .filter(|body| component.bodies.contains(body))
                        .collect::<Vec<_>>();
                    if component.physical_ground_sources.is_empty() {
                        if selected.len() == 1 {
                            Ok(Some(selected[0]))
                        } else {
                            Err(SpatialAssemblyError::InvalidGaugePolicy(format!(
                                "floating component beginning at {} requires exactly one reference",
                                component.bodies[0]
                            )))
                        }
                    } else if selected.is_empty() {
                        Ok(None)
                    } else {
                        Err(SpatialAssemblyError::InvalidGaugePolicy(format!(
                            "physically grounded component beginning at {} cannot have a numerical reference",
                            component.bodies[0]
                        )))
                    }
                })
                .collect()
        }
    }
}

#[allow(clippy::too_many_lines)]
fn build_gauge_report(
    assembly: &SpatialAssembly,
    certified: &[CertifiedSpatialComponent],
    references: &[Option<SpatialBodyId>],
    body_variables: &[SpatialBodyVariableMapping],
    source_mappings: &[SpatialSourceMapping],
    session: &SolveSession,
) -> Result<SpatialGaugeReport, SpatialAssemblyError> {
    let component_for_body = certified
        .iter()
        .enumerate()
        .flat_map(|(index, component)| component.bodies.iter().map(move |body| (*body, index)))
        .collect::<HashMap<_, _>>();
    let mut variable_components = HashMap::new();
    for mapping in body_variables {
        let component = *component_for_body.get(&mapping.body_id).ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "body {} has no certified component",
                mapping.body_id
            ))
        })?;
        variable_components.insert(mapping.variable_id, component);
    }
    let source_components = certified
        .iter()
        .enumerate()
        .flat_map(|(index, component)| component.sources.iter().map(move |source| (*source, index)))
        .collect::<HashMap<_, _>>();
    let mut residual_components = HashMap::new();
    for mapping in source_mappings {
        let component = *source_components.get(&mapping.source).ok_or_else(|| {
            SpatialAssemblyError::GaugeCertification(format!(
                "physical source {} has no certified component",
                mapping.source
            ))
        })?;
        for residual in &mapping.residual_ids {
            residual_components.insert(*residual, component);
        }
    }
    let mut core_components = vec![Vec::new(); certified.len()];
    let mut right_nullities = vec![0_usize; certified.len()];
    for summary in &session.report().structural.component_summaries {
        let mut domain_components = summary
            .variable_ids
            .iter()
            .map(|variable| {
                variable_components.get(variable).copied().ok_or_else(|| {
                    SpatialAssemblyError::GaugeCertification(format!(
                        "core variable {variable:?} is not a spatial body"
                    ))
                })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        for residual in &summary.residual_ids {
            domain_components.insert(*residual_components.get(residual).ok_or_else(|| {
                SpatialAssemblyError::GaugeCertification(format!(
                    "core residual {residual:?} is not mapped to a spatial source"
                ))
            })?);
        }
        if domain_components.len() != 1 {
            return Err(SpatialAssemblyError::GaugeCertification(format!(
                "core component {} does not map to exactly one spatial component",
                summary.component_index
            )));
        }
        let domain_component = *domain_components.iter().next().expect("length checked");
        let solve = session
            .report()
            .component_solves
            .iter()
            .find(|solve| solve.component_index == summary.component_index)
            .ok_or_else(|| {
                SpatialAssemblyError::GaugeCertification(format!(
                    "core component {} has no numerical report",
                    summary.component_index
                ))
            })?;
        core_components[domain_component].push(summary.component_index);
        right_nullities[domain_component] = right_nullities[domain_component]
            .checked_add(solve.right_nullity)
            .ok_or_else(|| {
                SpatialAssemblyError::GaugeCertification(
                    "component right nullity overflowed".to_owned(),
                )
            })?;
    }

    let mut components = Vec::with_capacity(certified.len());
    for (index, component) in certified.iter().enumerate() {
        let (world_action, gauge_dof, numerical_reference) = if let Some(body) = references[index] {
            let target_pose = assembly.require_body(body)?.pose_guess;
            (
                SpatialWorldActionCertification::FloatingSe3,
                6,
                Some(SpatialGaugeReference { body, target_pose }),
            )
        } else {
            (SpatialWorldActionCertification::PhysicallyGrounded, 0, None)
        };
        if right_nullities[index] < gauge_dof {
            return Err(SpatialAssemblyError::GaugeCertification(format!(
                "component {index} has right nullity {} below certified gauge DOF {gauge_dof}",
                right_nullities[index]
            )));
        }
        components.push(SpatialComponentGaugeReport {
            component_index: index,
            bodies: component.bodies.clone(),
            sources: component.sources.clone(),
            core_component_indices: core_components[index].clone(),
            numerical_equality_right_nullity: right_nullities[index],
            gauge_dof,
            internal_mobility: right_nullities[index] - gauge_dof,
            world_action,
            physical_ground_sources: component.physical_ground_sources.clone(),
            numerical_reference,
        });
    }
    let numerical_equality_right_nullity = checked_sum(
        right_nullities.iter().copied(),
        "total equality right nullity overflowed",
    )?;
    if numerical_equality_right_nullity != session.report().right_nullity {
        return Err(SpatialAssemblyError::GaugeCertification(format!(
            "mapped right nullity {numerical_equality_right_nullity} does not match core {}",
            session.report().right_nullity
        )));
    }
    let gauge_dof = checked_sum(
        components.iter().map(|component| component.gauge_dof),
        "total gauge DOF overflowed",
    )?;
    let internal_mobility = checked_sum(
        components
            .iter()
            .map(|component| component.internal_mobility),
        "total internal mobility overflowed",
    )?;
    Ok(SpatialGaugeReport {
        numerical_equality_right_nullity,
        gauge_dof,
        internal_mobility,
        components,
    })
}

fn accepted_session(
    problem: Problem,
    config: SolverConfig,
    stage: &'static str,
) -> Result<SolveSession, SpatialAssemblyError> {
    SolveSession::new(problem, config)
        .map_err(|error| SpatialAssemblyError::InitialRejected(format!("{stage}: {error}")))
}

fn solved_geometry_from_problem(
    problem: &Problem,
    body_variables: &[SpatialBodyVariableMapping],
    point_features: &[SpatialPointFeature],
    frame_features: &[SpatialFrameFeature],
) -> Result<SpatialGeometry, SpatialAssemblyError> {
    let mut bodies = Vec::with_capacity(body_variables.len());
    let mut poses = HashMap::with_capacity(body_variables.len());
    for mapping in body_variables {
        let variable = problem
            .variable(mapping.variable_id)
            .ok_or(CoreError::UnknownVariable(mapping.variable_id))?;
        let VariableValue::Pose3(ambient) = variable.value() else {
            return Err(CoreError::VariableKindMismatch {
                expected: VariableKind::Pose3,
                actual: variable.kind(),
            }
            .into());
        };
        let pose = Pose3::from_ambient(ambient)?;
        bodies.push(SpatialSolvedBody {
            body_id: mapping.body_id,
            pose,
        });
        poses.insert(mapping.body_id, pose);
    }
    let points = point_features
        .iter()
        .map(|feature| {
            let pose = poses
                .get(&feature.body)
                .copied()
                .ok_or(SpatialAssemblyError::UnknownBody(feature.body))?;
            Ok(SpatialTransformedPointFeature {
                feature_id: feature.id,
                body_id: feature.body,
                world: pose.try_transform_point(feature.local_point)?,
            })
        })
        .collect::<Result<Vec<_>, SpatialAssemblyError>>()?;
    let frames = frame_features
        .iter()
        .map(|feature| {
            let pose = poses
                .get(&feature.body)
                .copied()
                .ok_or(SpatialAssemblyError::UnknownBody(feature.body))?;
            Ok(SpatialTransformedFrameFeature {
                feature_id: feature.id,
                body_id: feature.body,
                world: transform_frame(pose, feature.local_frame)?,
            })
        })
        .collect::<Result<Vec<_>, SpatialAssemblyError>>()?;
    Ok(SpatialGeometry {
        bodies,
        points,
        frames,
    })
}

fn validate_physical_candidate(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
    session: &SolveSession,
    mappings: &[SpatialSourceMapping],
    config: SolverConfig,
) -> Result<f64, SpatialAssemblyError> {
    let tolerance = spatial_acceptance_tolerance(config);
    validate_core_acceptance(session.report(), tolerance)?;
    let core_max = physical_audit_max(session, mappings, tolerance)?;
    let domain_max = physical_domain_residual_max(assembly, geometry)?;
    let maximum = core_max.max(domain_max);
    if !maximum.is_finite() {
        return independent("combined physical residual maximum is non-finite");
    }
    if maximum > tolerance {
        return independent(format!(
            "physical residual {maximum:e} exceeds {tolerance:e}"
        ));
    }
    Ok(maximum)
}

fn validate_core_acceptance(
    report: &SolveReport,
    tolerance: f64,
) -> Result<(), SpatialAssemblyError> {
    if report.hard_validity != HardValidity::Valid {
        return independent(format!("core hard validity is {:?}", report.hard_validity));
    }
    if !report.hard_residuals_validated {
        return independent("core hard rows were not independently validated");
    }
    if !report.rank_is_valid {
        return independent("core numerical rank is invalid");
    }
    if !report.hard_residual_max.is_finite() || report.hard_residual_max > tolerance {
        return independent(format!(
            "core hard maximum {} is non-finite or exceeds {:e}",
            report.hard_residual_max, tolerance
        ));
    }
    for component in &report.component_solves {
        if component.hard_validity != HardValidity::Valid
            || !component.hard_residuals_validated
            || !component.rank_is_valid
            || !component.hard_residual_max.is_finite()
        {
            return independent(format!(
                "core component {} lacks finite hard/rank validity",
                component.component_index
            ));
        }
    }
    Ok(())
}

fn physical_audit_max(
    session: &SolveSession,
    mappings: &[SpatialSourceMapping],
    tolerance: f64,
) -> Result<f64, SpatialAssemblyError> {
    let mut maximum = 0.0_f64;
    for mapping in mappings {
        let source = session
            .report()
            .audit
            .sources
            .iter()
            .find(|source| source.source_id == mapping.core_source_id)
            .ok_or_else(|| {
                SpatialAssemblyError::IndependentValidation(format!(
                    "physical source {} is absent from accepted audit",
                    mapping.source
                ))
            })?;
        if source.source_label != mapping.source_label {
            return independent(format!(
                "physical source {} audit label does not match its mapping",
                mapping.source
            ));
        }
        let expected_rows = mapping.residual_ids.iter().try_fold(0_usize, |sum, id| {
            let rows = session
                .problem()
                .residual(*id)
                .ok_or(CoreError::UnknownResidual(*id))?
                .output_dimension();
            sum.checked_add(rows).ok_or(CoreError::DimensionOverflow {
                context: "spatial physical audit rows",
            })
        })?;
        if source.rows.len() != expected_rows {
            return independent(format!(
                "physical source {} has {} audit rows, expected {expected_rows}",
                mapping.source,
                source.rows.len()
            ));
        }
        for row in &source.rows {
            if !mapping.residual_ids.contains(&row.residual_id) {
                return independent(format!(
                    "physical source {} contains an unmapped residual",
                    mapping.source
                ));
            }
            if row.category != ResidualCategory::Hard
                || row.evaluation_status != AuditEvaluationStatus::Evaluated
            {
                return independent(format!(
                    "physical source {} contains a non-evaluated hard row",
                    mapping.source
                ));
            }
            if !row.raw_residual.is_finite()
                || !row.normalized_residual.is_finite()
                || !row.scale.is_finite()
                || row.scale <= 0.0
            {
                return independent(format!(
                    "physical source {} contains non-finite audit data",
                    mapping.source
                ));
            }
            maximum = maximum.max(row.normalized_residual.abs());
        }
    }
    if maximum > tolerance {
        return independent(format!(
            "physical core audit maximum {maximum:e} exceeds {tolerance:e}"
        ));
    }
    Ok(maximum)
}

#[allow(clippy::too_many_lines)]
fn physical_domain_residual_max(
    assembly: &SpatialAssembly,
    geometry: &SpatialGeometry,
) -> Result<f64, SpatialAssemblyError> {
    let mut maximum = 0.0_f64;
    for source in &assembly.sources {
        match source.kind {
            SpatialSourceKind::PhysicalGround { body, target_pose } => {
                let pose = geometry
                    .body_pose(body)
                    .ok_or(SpatialAssemblyError::UnknownBody(body))?;
                let difference = target_pose.local_difference(&pose)?;
                include_normalized(
                    &mut maximum,
                    &[
                        difference[0] / assembly.model_scale,
                        difference[1] / assembly.model_scale,
                        difference[2] / assembly.model_scale,
                        difference[3],
                        difference[4],
                        difference[5],
                    ],
                    "physical ground",
                )?;
            }
            SpatialSourceKind::BallJoint { first, second } => {
                let first = geometry
                    .world_point(first)
                    .ok_or(SpatialAssemblyError::UnknownPointFeature(first))?;
                let second = geometry
                    .world_point(second)
                    .ok_or(SpatialAssemblyError::UnknownPointFeature(second))?;
                let difference = second - first;
                include_normalized(
                    &mut maximum,
                    &[
                        difference.x / assembly.model_scale,
                        difference.y / assembly.model_scale,
                        difference.z / assembly.model_scale,
                    ],
                    "ball joint",
                )?;
            }
            SpatialSourceKind::FixedFrame { first, second } => {
                let first = geometry
                    .world_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(first))?;
                let second = geometry
                    .world_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(second))?;
                let difference = second.origin() - first.origin();
                let diagonal = [
                    first.x_axis().dot(&second.x_axis()),
                    first.y_axis().dot(&second.y_axis()),
                    first.z_axis().dot(&second.z_axis()),
                ];
                if !diagonal.iter().all(|value| value.is_finite())
                    || diagonal
                        .iter()
                        .any(|value| *value <= ORIENTATION_BRANCH_MARGIN)
                {
                    return independent(format!(
                        "fixed-frame source {} reached a false half-turn orientation root",
                        source.id
                    ));
                }
                include_normalized(
                    &mut maximum,
                    &[
                        difference.x / assembly.model_scale,
                        difference.y / assembly.model_scale,
                        difference.z / assembly.model_scale,
                        first.y_axis().dot(&second.x_axis()),
                        first.z_axis().dot(&second.x_axis()),
                        first.z_axis().dot(&second.y_axis()),
                    ],
                    "fixed frame",
                )?;
            }
            SpatialSourceKind::RevoluteJoint {
                first,
                second,
                parity,
            } => {
                let first = geometry
                    .world_frame(first)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(first))?;
                let second = geometry
                    .world_frame(second)
                    .ok_or(SpatialAssemblyError::UnknownFrameFeature(second))?;
                let difference = second.origin() - first.origin();
                let second_axis = second.z_axis() * parity.multiplier();
                let parity_dot = first.z_axis().dot(&second_axis);
                if !parity_dot.is_finite() || parity_dot <= ORIENTATION_BRANCH_MARGIN {
                    return independent(format!(
                        "revolute source {} violated {:?} axis parity",
                        source.id, parity
                    ));
                }
                include_normalized(
                    &mut maximum,
                    &[
                        difference.x / assembly.model_scale,
                        difference.y / assembly.model_scale,
                        difference.z / assembly.model_scale,
                        first.x_axis().dot(&second_axis),
                        first.y_axis().dot(&second_axis),
                    ],
                    "revolute joint",
                )?;
            }
        }
    }
    Ok(maximum)
}

fn project_geometry(
    assembly: &mut SpatialAssembly,
    geometry: &SpatialGeometry,
) -> Result<(), SpatialAssemblyError> {
    for solved in &geometry.bodies {
        validate_pose(solved.pose)?;
        assembly
            .bodies
            .iter_mut()
            .find(|body| body.id == solved.body_id)
            .ok_or(SpatialAssemblyError::UnknownBody(solved.body_id))?
            .pose_guess = solved.pose;
    }
    Ok(())
}

fn ground_audit_rows(body: SpatialBodyId, target: Pose3) -> Vec<ResidualRowAudit> {
    let coordinates = ["vx", "vy", "vz", "wx", "wy", "wz"];
    coordinates
        .iter()
        .enumerate()
        .map(|(index, coordinate)| {
            ResidualRowAudit::new(
                format!("physical ground target local difference {coordinate}"),
                vec![
                    AuditBinding::new("body", body.to_string()),
                    AuditBinding::new("target_pose", format!("{:?}", target.ambient())),
                ],
                if index < 3 { "model-unit" } else { "rad" },
            )
        })
        .collect()
}

fn private_gauge_audit_rows(body: SpatialBodyId) -> Vec<ResidualRowAudit> {
    let coordinates = ["vx", "vy", "vz", "wx", "wy", "wz"];
    coordinates
        .iter()
        .enumerate()
        .map(|(index, coordinate)| {
            ResidualRowAudit::new(
                format!("private spatial numerical gauge local {coordinate}"),
                vec![AuditBinding::new("body", body.to_string())],
                if index < 3 { "model-unit" } else { "rad" },
            )
        })
        .collect()
}

fn point_joint_audit_rows(
    joint: &str,
    first: &SpatialPointFeature,
    second: &SpatialPointFeature,
) -> Vec<ResidualRowAudit> {
    ["x", "y", "z"]
        .into_iter()
        .map(|coordinate| {
            ResidualRowAudit::new(
                format!("{joint} second world point {coordinate} - first world point {coordinate}"),
                point_bindings(first, second),
                "model-unit",
            )
        })
        .collect()
}

fn point_bindings(first: &SpatialPointFeature, second: &SpatialPointFeature) -> Vec<AuditBinding> {
    vec![
        AuditBinding::new("first_body", first.body.to_string()),
        AuditBinding::new("first_point_feature", first.id.to_string()),
        AuditBinding::new("second_body", second.body.to_string()),
        AuditBinding::new("second_point_feature", second.id.to_string()),
    ]
}

fn frame_joint_audit_rows(
    joint: &str,
    first: &SpatialFrameFeature,
    second: &SpatialFrameFeature,
    parity: Option<SpatialAxisParity>,
    templates: &[&str],
) -> Vec<ResidualRowAudit> {
    templates
        .iter()
        .enumerate()
        .map(|(index, template)| {
            ResidualRowAudit::new(
                format!("{joint} {template}"),
                frame_bindings(first, second, parity),
                if index < 3 {
                    "model-unit"
                } else {
                    "dimensionless"
                },
            )
        })
        .collect()
}

fn frame_bindings(
    first: &SpatialFrameFeature,
    second: &SpatialFrameFeature,
    parity: Option<SpatialAxisParity>,
) -> Vec<AuditBinding> {
    let mut bindings = vec![
        AuditBinding::new("first_body", first.body.to_string()),
        AuditBinding::new("first_frame_feature", first.id.to_string()),
        AuditBinding::new("second_body", second.body.to_string()),
        AuditBinding::new("second_frame_feature", second.id.to_string()),
    ];
    if let Some(parity) = parity {
        bindings.push(AuditBinding::new("axis_parity", format!("{parity:?}")));
    }
    bindings
}

fn transform_frame(pose: Pose3, local: Frame3) -> Result<Frame3, SpatialAssemblyError> {
    let local = revalidate_frame(local)?;
    Ok(Frame3::try_new(
        pose.try_transform_point(local.origin())?,
        pose.try_transform_vector(local.x_axis())?,
        pose.try_transform_vector(local.y_axis())?,
        pose.try_transform_vector(local.z_axis())?,
    )?)
}

fn revalidate_frame(frame: Frame3) -> Result<Frame3, SpatialAssemblyError> {
    Ok(Frame3::try_new(
        frame.origin(),
        frame.x_axis(),
        frame.y_axis(),
        frame.z_axis(),
    )?)
}

fn validate_pose(pose: Pose3) -> Result<(), SpatialAssemblyError> {
    Pose3::from_ambient(pose.ambient())?;
    Ok(())
}

fn validate_point(point: Point3<f64>, field: &'static str) -> Result<(), SpatialAssemblyError> {
    if point.coords.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        invalid_field(field, "point coordinates must be finite")
    }
}

fn validate_model_scale(model_scale: f64) -> Result<(), SpatialAssemblyError> {
    if model_scale.is_finite() && model_scale > 0.0 {
        Ok(())
    } else {
        Err(SpatialAssemblyError::InvalidModelScale { value: model_scale })
    }
}

fn spatial_acceptance_tolerance(config: SolverConfig) -> f64 {
    config
        .normalized_residual_tolerance
        .min(SPATIAL_ACCEPTANCE_TOLERANCE)
}

fn validate_label(label: &str, field: &'static str) -> Result<(), SpatialAssemblyError> {
    if label.trim().is_empty() {
        Err(SpatialAssemblyError::InvalidLabel { field })
    } else {
        Ok(())
    }
}

fn require_distinct_bodies(
    first: SpatialBodyId,
    second: SpatialBodyId,
) -> Result<(), SpatialAssemblyError> {
    if first == second {
        Err(SpatialAssemblyError::SameBodyJointEndpoints(first))
    } else {
        Ok(())
    }
}

fn require_unique_raw_id(ids: &mut BTreeSet<u64>, id: u64) -> Result<(), SpatialAssemblyError> {
    if id == 0 || !ids.insert(id) {
        invalid_field("id", format!("ID {id} is zero or duplicated"))
    } else {
        Ok(())
    }
}

fn variable_for_body(
    variables: &HashMap<SpatialBodyId, VariableId>,
    body: SpatialBodyId,
) -> Result<VariableId, SpatialAssemblyError> {
    variables
        .get(&body)
        .copied()
        .ok_or(SpatialAssemblyError::UnknownBody(body))
}

fn point_array(point: Point3<f64>) -> [f64; 3] {
    [point.x, point.y, point.z]
}

fn include_normalized(
    maximum: &mut f64,
    values: &[f64],
    context: &str,
) -> Result<(), SpatialAssemblyError> {
    if !values.iter().all(|value| value.is_finite()) {
        return independent(format!("{context} independent residual is non-finite"));
    }
    for value in values {
        *maximum = maximum.max(value.abs());
    }
    Ok(())
}

fn checked_sum(
    values: impl IntoIterator<Item = usize>,
    message: &'static str,
) -> Result<usize, SpatialAssemblyError> {
    values.into_iter().try_fold(0_usize, |sum, value| {
        sum.checked_add(value)
            .ok_or_else(|| SpatialAssemblyError::GaugeCertification(message.to_owned()))
    })
}

fn find_root(parents: &mut [usize], index: usize) -> usize {
    if parents[index] != index {
        parents[index] = find_root(parents, parents[index]);
    }
    parents[index]
}

fn union_roots(parents: &mut [usize], first: usize, second: usize) {
    let first_root = find_root(parents, first);
    let second_root = find_root(parents, second);
    if first_root != second_root {
        let (lower, higher) = if first_root < second_root {
            (first_root, second_root)
        } else {
            (second_root, first_root)
        };
        parents[higher] = lower;
    }
}

fn invalid_field<T>(
    field: &'static str,
    message: impl Into<String>,
) -> Result<T, SpatialAssemblyError> {
    Err(SpatialAssemblyError::InvalidField {
        field,
        message: message.into(),
    })
}

fn independent<T>(message: impl Into<String>) -> Result<T, SpatialAssemblyError> {
    Err(SpatialAssemblyError::IndependentValidation(message.into()))
}
