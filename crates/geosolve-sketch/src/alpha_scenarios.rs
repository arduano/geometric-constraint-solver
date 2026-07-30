use std::f64::consts::PI;

use crate::{
    ContactId, ContactNeighborhood, CurveDefinition, CurveId, CurveSpan, DesignPointId,
    DesignScalarId, DocumentArcSweep, DocumentArcTangencySide, DocumentBSplineForm,
    DocumentConstraintDefinition, DocumentConstraintId, DocumentCoordinateAxis,
    DocumentCurveContinuity, DocumentCurveNormalSide, DocumentDimensionDefinition,
    DocumentDimensionId, DocumentDimensionMode, DocumentError, DocumentFilletEndpointOrder,
    DocumentFilletTrimEndpoint, DocumentHyperbolaBranch, DocumentId, DocumentLineOffsetOrientation,
    DocumentLineSide, DocumentSolveRequest, DocumentTrimParameter, FeatureEndpoint,
    LineLineFilletIds, LineLineFilletRequest, MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
    MIN_REPRESENTABLE_RADIUS, MirroredCurveIds, PersistentId, RectangleIds, ScalarDomain,
    ScalarUnit, SketchDocument, TangentOrientation, VisualProfileOptions, VisualProfileStatus,
};
use crate::{CurveCurveFilletIds, CurveCurveFilletRequest, CurveFilletParentRequest};

/// Canonical playground-alpha scenarios shared by native tests and browser examples.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaScenarioKind {
    A1,
    A2,
    A3,
    A4,
    A5,
    A8,
    Corpus,
    StressCompass,
    StressBridge,
    MotionCam,
    MotionOrbit,
    MotionTrammel,
    MotionScotchYoke,
    MotionRotatingSquare,
    MotionScissor,
    MotionScissorTower,
    MotionPeaucellier,
    MotionFourBarCoupler,
    MotionPantograph,
    MotionDrawingArm,
    BranchLockedElbow,
    BranchFourBar,
    DiagnosticRankDrop,
    DiagnosticEndpointBound,
    DiagnosticRedundancy,
    ConicGallery,
    ConicTangency,
    ConicCircleLimit,
    M28TrimmedFillet,
    SupportingOffset,
    ExactTranslatedOffset,
    EntityMirror,
    DirectedAngle,
    M27ReferenceFillet,
    FilletLineCircle,
    FilletLineBezier,
    FilletNurbsLine,
    NurbsQuarterCircle,
    NurbsLocalSupport,
    NurbsPeriodic,
    NurbsDifferential,
    ProfileAllFamilies,
    ProfileCurvedTopology,
    ProfileFilletTrim,
    ProfileNurbsSelfIntersection,
    ProfileIncomplete,
    ProfileBudget,
}

/// Deterministic workload sizes used by the M14 interaction budgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AlphaPerformanceSize {
    Small,
    Medium,
}

/// Concise interaction contract shown by the browser for one focused UAT lab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlphaScenarioUat {
    pub title: &'static str,
    pub instructions: &'static str,
    pub expected_equality_dof: usize,
    pub expected_bounded_dof: usize,
    pub primary_drag: &'static str,
}

impl AlphaScenarioKind {
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::A1 => "a1",
            Self::A2 => "a2",
            Self::A3 => "a3",
            Self::A4 => "a4",
            Self::A5 => "a5",
            Self::A8 => "a8",
            Self::Corpus => "corpus",
            Self::StressCompass => "stress-compass",
            Self::StressBridge => "stress-bridge",
            Self::MotionCam => "motion-cam",
            Self::MotionOrbit => "motion-orbit",
            Self::MotionTrammel => "motion-trammel",
            Self::MotionScotchYoke => "motion-scotch-yoke",
            Self::MotionRotatingSquare => "motion-rotating-square",
            Self::MotionScissor => "motion-scissor",
            Self::MotionScissorTower => "motion-scissor-tower",
            Self::MotionPeaucellier => "motion-peaucellier",
            Self::MotionFourBarCoupler => "motion-four-bar-coupler",
            Self::MotionPantograph => "motion-pantograph",
            Self::MotionDrawingArm => "motion-drawing-arm",
            Self::BranchLockedElbow => "branch-locked-elbow",
            Self::BranchFourBar => "branch-four-bar",
            Self::DiagnosticRankDrop => "diagnostic-rank-drop",
            Self::DiagnosticEndpointBound => "diagnostic-endpoint-bound",
            Self::DiagnosticRedundancy => "diagnostic-redundancy",
            Self::ConicGallery => "conic-gallery",
            Self::ConicTangency => "conic-tangency",
            Self::ConicCircleLimit => "conic-circle-limit",
            Self::M28TrimmedFillet => "m28-trimmed-fillet",
            Self::SupportingOffset => "construction-supporting-offset",
            Self::ExactTranslatedOffset => "construction-exact-offset",
            Self::EntityMirror => "construction-entity-mirror",
            Self::DirectedAngle => "construction-directed-angle",
            Self::M27ReferenceFillet => "fillet-line-line-reference",
            Self::FilletLineCircle => "fillet-line-circle",
            Self::FilletLineBezier => "fillet-line-bezier",
            Self::FilletNurbsLine => "fillet-nurbs-line",
            Self::NurbsQuarterCircle => "nurbs-quarter-circle",
            Self::NurbsLocalSupport => "nurbs-local-support",
            Self::NurbsPeriodic => "nurbs-periodic",
            Self::NurbsDifferential => "nurbs-differential",
            Self::ProfileAllFamilies => "profile-all-families",
            Self::ProfileCurvedTopology => "profile-curved-topology",
            Self::ProfileFilletTrim => "profile-fillet-trim",
            Self::ProfileNurbsSelfIntersection => "profile-nurbs-self-intersection",
            Self::ProfileIncomplete => "profile-incomplete",
            Self::ProfileBudget => "profile-budget",
        }
    }

    /// Returns the focused UAT contract for interactive M25-M28 and NURBS labs.
    #[must_use]
    pub const fn uat(self) -> Option<AlphaScenarioUat> {
        let uat = match self {
            Self::SupportingOffset => AlphaScenarioUat {
                title: "Supporting-line offset",
                instructions: "Drag either target endpoint. The target stays parallel and two units left of the fixed source while axial position and length remain free.",
                expected_equality_dof: 2,
                expected_bounded_dof: 2,
                primary_drag: "Supporting offset draggable target end",
            },
            Self::ExactTranslatedOffset => AlphaScenarioUat {
                title: "Exact translated-segment offset",
                instructions: "Drag the source end around its anchor. The associated target must rotate with identical endpoint correspondence and offset.",
                expected_equality_dof: 1,
                expected_bounded_dof: 1,
                primary_drag: "Exact offset draggable source end",
            },
            Self::EntityMirror => AlphaScenarioUat {
                title: "Entity mirror",
                instructions: "Drag the source or reflected endpoint. Ordinary symmetry rows project the counterpart across the fixed axis.",
                expected_equality_dof: 1,
                expected_bounded_dof: 1,
                primary_drag: "Mirror source draggable end",
            },
            Self::DirectedAngle => AlphaScenarioUat {
                title: "Directed-angle branch cut",
                instructions: "Drag the moving tip through the negative-X cut while the angle is reference, then select the angle dimension and switch it to driving or edit target/orientation.",
                expected_equality_dof: 1,
                expected_bounded_dof: 1,
                primary_drag: "Directed angle draggable branch-cut tip",
            },
            Self::M27ReferenceFillet => AlphaScenarioUat {
                title: "M27 untrimmed line-line fillet",
                instructions: "Drag the fillet center to change its reference radius and contacts. Both complete parent lines remain visibly untrimmed.",
                expected_equality_dof: 1,
                expected_bounded_dof: 1,
                primary_drag: "M27 reference-radius untrimmed fillet.center",
            },
            Self::FilletLineCircle => AlphaScenarioUat {
                title: "Generic line-circle fillet",
                instructions: "Drag the fillet center. The reference radius, output arc, both contacts, and visible parent boundaries move together.",
                expected_equality_dof: 1,
                expected_bounded_dof: 1,
                primary_drag: "Interactive line-circle fillet.center",
            },
            Self::FilletLineBezier => AlphaScenarioUat {
                title: "Generic line-Bezier fillet",
                instructions: "Drag the fillet center along the regular family and inspect both moving trim markers and the associated output arc.",
                expected_equality_dof: 1,
                expected_bounded_dof: 1,
                primary_drag: "Interactive line-Bezier fillet.center",
            },
            Self::FilletNurbsLine => AlphaScenarioUat {
                title: "Generic NURBS-line fillet",
                instructions: "Drag the output center or edit a non-gauge NURBS weight. Contacts, output, and NURBS/line trim state stay associated.",
                expected_equality_dof: 3,
                expected_bounded_dof: 3,
                primary_drag: "Interactive NURBS-line fillet.center",
            },
            Self::NurbsQuarterCircle => AlphaScenarioUat {
                title: "NURBS quarter-circle and weight",
                instructions: "Drag the middle control or select the curve and edit its non-gauge middle weight. Re-gauging must preserve the curve exactly.",
                expected_equality_dof: 4,
                expected_bounded_dof: 4,
                primary_drag: "NURBS quarter-circle weight lab control 2",
            },
            Self::NurbsLocalSupport => AlphaScenarioUat {
                title: "NURBS local support and insertion",
                instructions: "Drag a middle control to see local span support, then select the curve and insert a knot; geometry must remain unchanged while topology gains one control/weight pair.",
                expected_equality_dof: 13,
                expected_bounded_dof: 13,
                primary_drag: "Local-support NURBS control 3",
            },
            Self::NurbsPeriodic => AlphaScenarioUat {
                title: "Periodic NURBS span and winding",
                instructions: "Drag a periodic control, then select the seam contact and use Next/Previous span. The world point stays fixed while semantic span and winding change explicitly.",
                expected_equality_dof: 13,
                expected_bounded_dof: 12,
                primary_drag: "Periodic NURBS control 4",
            },
            Self::NurbsDifferential => AlphaScenarioUat {
                title: "NURBS differential and C2 continuity",
                instructions: "Drag the shared seam or an adjacent handle. The rate-explicit parametric C2 source keeps position, first derivative, and second derivative associated.",
                expected_equality_dof: 10,
                expected_bounded_dof: 10,
                primary_drag: "NURBS C2 draggable seam",
            },
            _ => return None,
        };
        Some(uat)
    }

    /// Returns the visual-profile inspection contract for M31 profile scenes.
    #[must_use]
    pub const fn profile_uat(self) -> Option<AlphaProfileScenarioUat> {
        let uat = match self {
            Self::ProfileAllFamilies => AlphaProfileScenarioUat {
                title: "All-family profile gallery",
                instructions: "Inspect all 15 curve-family roles, their closed finite faces, and the certified splitter intersections.",
                expected_status: VisualProfileStatus::Complete,
                expected_family_count: 15,
                expected_minimum_face_count: 15,
                options: DEFAULT_PROFILE_OPTIONS,
            },
            Self::ProfileCurvedTopology => AlphaProfileScenarioUat {
                title: "Curved intersections and holes",
                instructions: "Inspect transverse circle/ellipse roots and the separate nested curved face with an inner hole contour.",
                expected_status: VisualProfileStatus::Complete,
                expected_family_count: 2,
                expected_minimum_face_count: 5,
                options: DEFAULT_PROFILE_OPTIONS,
            },
            Self::ProfileFilletTrim => AlphaProfileScenarioUat {
                title: "Fillet-owned visible trim joins",
                instructions: "Inspect the closed contour whose traversal welds the trimmed parent circle and line to the associated output arc through explicit M28 ownership joins.",
                expected_status: VisualProfileStatus::Complete,
                expected_family_count: 3,
                expected_minimum_face_count: 1,
                options: DEFAULT_PROFILE_OPTIONS,
            },
            Self::ProfileNurbsSelfIntersection => AlphaProfileScenarioUat {
                title: "Editable NURBS self-intersection",
                instructions: "Select the NURBS, edit exact control coordinates or a non-gauge weight, and inspect whether the native analyzer certifies a transverse self-root and bounded lobe or reports a typed incomplete result.",
                expected_status: VisualProfileStatus::Complete,
                expected_family_count: 1,
                expected_minimum_face_count: 1,
                options: DEFAULT_PROFILE_OPTIONS,
            },
            Self::ProfileIncomplete => AlphaProfileScenarioUat {
                title: "Retained face beside tangency",
                instructions: "Inspect the clean standalone curved face retained beside a typed unresolved tangent pair.",
                expected_status: VisualProfileStatus::Truncated,
                expected_family_count: 1,
                expected_minimum_face_count: 1,
                options: DEFAULT_PROFILE_OPTIONS,
            },
            Self::ProfileBudget => AlphaProfileScenarioUat {
                title: "Deterministic profile budget stop",
                instructions: "Inspect the valid curved document with its intersection-root budget reduced to zero; no face may be published.",
                expected_status: VisualProfileStatus::Skipped,
                expected_family_count: 2,
                expected_minimum_face_count: 0,
                options: ROOT_BUDGET_PROFILE_OPTIONS,
            },
            _ => return None,
        };
        Some(uat)
    }
}

const DEFAULT_PROFILE_OPTIONS: VisualProfileOptions = VisualProfileOptions {
    max_candidate_pairs: 100_000,
    max_intersection_subdivisions: 500_000,
    max_intersection_depth: 64,
    max_intersection_roots: 100_000,
    max_fragments: 100_000,
    max_integration_subdivisions: 500_000,
    max_containment_tests: 100_000,
    max_faces: 10_000,
};

const ROOT_BUDGET_PROFILE_OPTIONS: VisualProfileOptions = VisualProfileOptions {
    max_intersection_roots: 0,
    ..DEFAULT_PROFILE_OPTIONS
};

/// Concise analysis contract shown by consumers for one visual-profile UAT scene.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AlphaProfileScenarioUat {
    pub title: &'static str,
    pub instructions: &'static str,
    pub expected_status: VisualProfileStatus,
    pub expected_family_count: usize,
    pub expected_minimum_face_count: usize,
    pub options: VisualProfileOptions,
}

/// Persistent roles in A1.
#[derive(Clone, Debug, PartialEq)]
pub struct A1ScenarioIds {
    pub rectangle: RectangleIds,
    pub diagonal_target: DesignScalarId,
    pub diagonal: DocumentDimensionId,
}

/// Persistent roles in A2.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct A2ScenarioIds {
    pub a: DesignPointId,
    pub b: DesignPointId,
    pub c: DesignPointId,
    pub ab: CurveId,
    pub distance_ac: DocumentDimensionId,
}

/// Persistent roles in A3.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct A3ScenarioIds {
    pub line: CurveId,
    pub guide: DesignPointId,
    pub center: DesignPointId,
    pub circle: CurveId,
    pub line_contact: ContactId,
    pub circle_contact: ContactId,
    pub tangency: DocumentConstraintId,
}

/// Persistent roles in A4.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct A4ScenarioIds {
    pub arc_center: DesignPointId,
    pub circle_center: DesignPointId,
    pub arc: CurveId,
    pub circle: CurveId,
    pub circle_radius: DesignScalarId,
    pub circle_contact: ContactId,
    pub arc_contact: ContactId,
    pub tangency: DocumentConstraintId,
}

/// Persistent roles in A5.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct A5ScenarioIds {
    pub controls: [DesignPointId; 4],
    pub a: DesignPointId,
    pub b: DesignPointId,
    pub line: CurveId,
    pub bezier: CurveId,
    pub bezier_contact: ContactId,
    pub tangency: DocumentConstraintId,
}

/// Persistent roles in the combined A8 round-trip document.
#[derive(Clone, Debug, PartialEq)]
pub struct A8ScenarioIds {
    pub a1: A1ScenarioIds,
    pub a3: A3ScenarioIds,
    pub a4: A4ScenarioIds,
    pub a5: A5ScenarioIds,
}

/// Persistent roles in the symmetric drafting-compass stress example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StressCompassIds {
    pub origin: DesignPointId,
    pub first_tip: DesignPointId,
    pub second_tip: DesignPointId,
    pub first_arm: CurveId,
    pub second_arm: CurveId,
    pub angle: DocumentDimensionId,
}

/// Persistent roles in the cubic Bezier C1-bridge stress example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StressBridgeIds {
    pub left_seam: DesignPointId,
    pub right_seam: DesignPointId,
    pub left_curve: CurveId,
    pub right_curve: CurveId,
    pub tangency: DocumentConstraintId,
    pub equal_handles: DocumentConstraintId,
}

/// Persistent roles in the twin-roller Bezier-cam motion example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionCamIds {
    pub controls: [DesignPointId; 3],
    pub left_center: DesignPointId,
    pub right_center: DesignPointId,
    pub cam: CurveId,
    pub left_circle: CurveId,
    pub right_circle: CurveId,
}

/// Persistent roles in the branch-preserving tangent-orbit motion example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionOrbitIds {
    pub fixed_center: DesignPointId,
    pub moving_center: DesignPointId,
    pub fixed_circle: CurveId,
    pub moving_circle: CurveId,
    pub tangency: DocumentConstraintId,
}

/// Persistent roles in the elliptic-trammel motion example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionTrammelIds {
    pub horizontal_slider: DesignPointId,
    pub vertical_slider: DesignPointId,
    pub tracer: DesignPointId,
    pub bar: CurveId,
    pub horizontal_contact: ContactId,
    pub vertical_contact: ContactId,
}

/// Persistent roles in the offset Scotch-yoke motion example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionScotchYokeIds {
    pub crank_center: DesignPointId,
    pub crank_pin: DesignPointId,
    pub slider: DesignPointId,
    pub crank: CurveId,
    pub slot: CurveId,
}

/// Persistent roles in the constraint-built rotating-square example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionRotatingSquareIds {
    pub corners: [DesignPointId; 4],
    pub edges: [CurveId; 4],
}

/// Persistent roles in the symmetric scissor-jack motion example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionScissorIds {
    pub anchor: DesignPointId,
    pub slider: DesignPointId,
    pub upper_joint: DesignPointId,
    pub lower_joint: DesignPointId,
    pub axis: CurveId,
}

/// Persistent roles in the synchronized five-stage scissor-tower example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionScissorTowerIds {
    pub left_levels: [DesignPointId; 6],
    pub right_levels: [DesignPointId; 6],
    pub platforms: [CurveId; 6],
    pub diagonal_bars: [CurveId; 10],
}

/// Persistent roles in the Peaucellier-Lipkin straight-line example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionPeaucellierIds {
    pub origin: DesignPointId,
    pub input_center: DesignPointId,
    pub input: DesignPointId,
    pub output: DesignPointId,
    pub shoulders: [DesignPointId; 2],
    pub bars: [CurveId; 7],
}

/// Persistent roles in the four-bar coupler-path example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionFourBarCouplerIds {
    pub grounds: [DesignPointId; 2],
    pub joints: [DesignPointId; 2],
    pub tracer: DesignPointId,
    pub bars: [CurveId; 3],
}

/// Persistent roles in the two-DOF pantograph example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionPantographIds {
    pub anchor: DesignPointId,
    pub input: DesignPointId,
    pub guide: DesignPointId,
    pub output: DesignPointId,
    pub center: DesignPointId,
    pub bars: [CurveId; 4],
}

/// Persistent roles in the three-DOF articulated drawing-arm example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MotionDrawingArmIds {
    pub anchor: DesignPointId,
    pub joints: [DesignPointId; 3],
    pub links: [CurveId; 3],
}

/// Persistent roles in the explicit two-root locked-elbow branch example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BranchLockedElbowIds {
    pub base: DesignPointId,
    pub elbow: DesignPointId,
    pub end: DesignPointId,
    pub links: [CurveId; 2],
}

/// Persistent roles in the structural-versus-numerical rank diagnostic example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticRankDropIds {
    pub point: DesignPointId,
    pub first_distance: DocumentDimensionId,
    pub second_distance: DocumentDimensionId,
}

/// Persistent roles in the fixed-endpoint versus active-radius diagnostic example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticEndpointBoundIds {
    pub line: CurveId,
    pub point: DesignPointId,
    pub contact: ContactId,
    pub point_on_curve: DocumentConstraintId,
    pub circle: CurveId,
    pub radius: DesignScalarId,
}

/// Persistent roles in the redundant-source diagnostic example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DiagnosticRedundancyIds {
    pub line: CurveId,
    pub primary_length: DocumentDimensionId,
    pub duplicate_length: DocumentDimensionId,
}

/// Persistent curve roles in the five-family conic gallery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConicGalleryIds {
    /// Ellipse, elliptical arc, rational quadratic, parabola, and hyperbola.
    pub curves: [CurveId; 5],
}

/// Persistent roles in the generic conic-contact and tangency example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConicTangencyIds {
    pub curves: [CurveId; 2],
    pub contact_points: [DesignPointId; 2],
    pub point_contacts: [ContactId; 2],
    pub tangency_contacts: [ContactId; 2],
    pub tangency: DocumentConstraintId,
}

/// Persistent roles in the full-ellipse and directed-arc circle-limit example.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConicCircleLimitIds {
    /// Full ellipse followed by directed elliptical arc.
    pub curves: [CurveId; 2],
    pub full_ellipse_contacts: [ContactId; 2],
    pub arc_endpoint_contacts: [ContactId; 2],
}

/// Persistent roles in the M28 line-circle fillet with explicit parent trim views.
#[derive(Clone, Debug, PartialEq)]
pub struct M28TrimmedFilletIds {
    pub circle: CurveId,
    pub line: CurveId,
    pub fillet: CurveCurveFilletIds,
}

/// Persistent curve roles sufficient to inspect one visual-profile UAT scene.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileScenarioIds {
    pub curves: Vec<CurveId>,
}

/// Persistent roles in the supporting-line offset interaction lab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupportingOffsetIds {
    pub source: CurveId,
    pub target: CurveId,
    pub target_points: [DesignPointId; 2],
    pub dimension: DocumentDimensionId,
}

/// Persistent roles in the exact translated-segment offset interaction lab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactTranslatedOffsetIds {
    pub source: CurveId,
    pub source_end: DesignPointId,
    pub target: CurveId,
    pub target_points: [DesignPointId; 2],
    pub dimension: DocumentDimensionId,
}

/// Persistent roles in the point-defined entity-mirror interaction lab.
#[derive(Clone, Debug, PartialEq)]
pub struct EntityMirrorIds {
    pub axis: CurveId,
    pub source_end: DesignPointId,
    pub mirrored_end: DesignPointId,
    pub mirror: MirroredCurveIds,
}

/// Persistent roles in the directed-angle branch-cut interaction lab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectedAngleIds {
    pub first: CurveId,
    pub second: CurveId,
    pub moving_tip: DesignPointId,
    pub dimension: DocumentDimensionId,
    pub target: DesignScalarId,
}

/// Persistent roles in the M27 visibly-untrimmed reference-radius fillet lab.
#[derive(Clone, Debug, PartialEq)]
pub struct M27ReferenceFilletIds {
    pub parents: [CurveId; 2],
    pub fillet: LineLineFilletIds,
}

/// Persistent roles shared by the M28 generic fillet interaction labs.
#[derive(Clone, Debug, PartialEq)]
pub struct GenericFilletLabIds {
    pub parents: [CurveId; 2],
    pub fillet: CurveCurveFilletIds,
}

/// Persistent roles shared by the NURBS object-inspector labs.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsLabIds {
    pub curve: CurveId,
    pub controls: Vec<DesignPointId>,
    pub weights: Vec<DesignScalarId>,
    pub primary_control: DesignPointId,
    pub contact: Option<ContactId>,
}

/// Persistent roles in the NURBS differential/continuity lab.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsDifferentialIds {
    pub curves: [CurveId; 2],
    pub seam: DesignPointId,
    pub controls: Vec<DesignPointId>,
    pub weights: Vec<DesignScalarId>,
    pub contacts: [ContactId; 2],
    pub continuity: DocumentConstraintId,
}

/// Scenario-specific persistent roles.
#[derive(Clone, Debug, PartialEq)]
pub enum AlphaScenarioIds {
    A1(A1ScenarioIds),
    A2(A2ScenarioIds),
    A3(A3ScenarioIds),
    A4(A4ScenarioIds),
    A5(A5ScenarioIds),
    A8(Box<A8ScenarioIds>),
    Corpus,
    StressCompass(StressCompassIds),
    StressBridge(StressBridgeIds),
    MotionCam(MotionCamIds),
    MotionOrbit(MotionOrbitIds),
    MotionTrammel(MotionTrammelIds),
    MotionScotchYoke(MotionScotchYokeIds),
    MotionRotatingSquare(MotionRotatingSquareIds),
    MotionScissor(MotionScissorIds),
    MotionScissorTower(MotionScissorTowerIds),
    MotionPeaucellier(MotionPeaucellierIds),
    MotionFourBarCoupler(MotionFourBarCouplerIds),
    MotionPantograph(MotionPantographIds),
    MotionDrawingArm(MotionDrawingArmIds),
    BranchLockedElbow(BranchLockedElbowIds),
    BranchFourBar(MotionFourBarCouplerIds),
    DiagnosticRankDrop(DiagnosticRankDropIds),
    DiagnosticEndpointBound(DiagnosticEndpointBoundIds),
    DiagnosticRedundancy(DiagnosticRedundancyIds),
    ConicGallery(ConicGalleryIds),
    ConicTangency(ConicTangencyIds),
    ConicCircleLimit(ConicCircleLimitIds),
    M28TrimmedFillet(M28TrimmedFilletIds),
    SupportingOffset(SupportingOffsetIds),
    ExactTranslatedOffset(ExactTranslatedOffsetIds),
    EntityMirror(EntityMirrorIds),
    DirectedAngle(DirectedAngleIds),
    M27ReferenceFillet(M27ReferenceFilletIds),
    FilletLineCircle(GenericFilletLabIds),
    FilletLineBezier(GenericFilletLabIds),
    FilletNurbsLine(GenericFilletLabIds),
    NurbsQuarterCircle(NurbsLabIds),
    NurbsLocalSupport(NurbsLabIds),
    NurbsPeriodic(NurbsLabIds),
    NurbsDifferential(NurbsDifferentialIds),
    ProfileAllFamilies(ProfileScenarioIds),
    ProfileCurvedTopology(ProfileScenarioIds),
    ProfileFilletTrim(M28TrimmedFilletIds),
    ProfileNurbsSelfIntersection(NurbsLabIds),
    ProfileIncomplete(ProfileScenarioIds),
    ProfileBudget(ProfileScenarioIds),
}

/// One deterministic document and initial solve request for an alpha scenario.
#[derive(Clone, Debug, PartialEq)]
pub struct AlphaScenarioFixture {
    pub document: SketchDocument,
    pub request: DocumentSolveRequest,
    pub ids: AlphaScenarioIds,
}

/// Builds one canonical alpha scenario at a uniform model scale.
///
/// Persistent identities are invariant across scales for the same scenario.
///
/// # Errors
///
/// Returns a document validation error for a nonpositive/non-finite scale or an
/// unexpected construction failure.
#[allow(clippy::too_many_lines)]
pub fn alpha_scenario(
    kind: AlphaScenarioKind,
    scale: f64,
) -> Result<AlphaScenarioFixture, DocumentError> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(DocumentError::InvalidField {
            field: "alpha scenario scale",
            message: "must be positive and finite".into(),
        });
    }
    let namespace = match kind {
        AlphaScenarioKind::A1 => 0xa1_0000,
        AlphaScenarioKind::A2 => 0xa2_0000,
        AlphaScenarioKind::A3 => 0xa3_0000,
        AlphaScenarioKind::A4 => 0xa4_0000,
        AlphaScenarioKind::A5 => 0xa5_0000,
        AlphaScenarioKind::A8 => 0xa8_0000,
        AlphaScenarioKind::Corpus => 0xac_0000,
        AlphaScenarioKind::StressCompass => 0xc1_0000,
        AlphaScenarioKind::StressBridge => 0xc2_0000,
        AlphaScenarioKind::MotionCam => 0xc3_0000,
        AlphaScenarioKind::MotionOrbit => 0xc4_0000,
        AlphaScenarioKind::MotionTrammel => 0xc5_0000,
        AlphaScenarioKind::MotionScotchYoke => 0xc6_0000,
        AlphaScenarioKind::MotionRotatingSquare => 0xc7_0000,
        AlphaScenarioKind::MotionScissor => 0xc8_0000,
        AlphaScenarioKind::MotionScissorTower => 0xc9_0000,
        AlphaScenarioKind::MotionPeaucellier => 0xca_0000,
        AlphaScenarioKind::MotionFourBarCoupler => 0xcb_0000,
        AlphaScenarioKind::MotionPantograph => 0xcc_0000,
        AlphaScenarioKind::MotionDrawingArm => 0xcd_0000,
        AlphaScenarioKind::BranchLockedElbow => 0xce_0000,
        AlphaScenarioKind::BranchFourBar => 0xcf_0000,
        AlphaScenarioKind::DiagnosticRankDrop => 0xd1_0000,
        AlphaScenarioKind::DiagnosticEndpointBound => 0xd2_0000,
        AlphaScenarioKind::DiagnosticRedundancy => 0xd3_0000,
        AlphaScenarioKind::ConicGallery => 0xe1_0000,
        AlphaScenarioKind::ConicTangency => 0xe2_0000,
        AlphaScenarioKind::ConicCircleLimit => 0xe3_0000,
        AlphaScenarioKind::M28TrimmedFillet => 0xf1_0000,
        AlphaScenarioKind::SupportingOffset => 0xf2_0000,
        AlphaScenarioKind::ExactTranslatedOffset => 0xf3_0000,
        AlphaScenarioKind::EntityMirror => 0xf4_0000,
        AlphaScenarioKind::DirectedAngle => 0xf5_0000,
        AlphaScenarioKind::M27ReferenceFillet => 0xf6_0000,
        AlphaScenarioKind::FilletLineCircle => 0xf7_0000,
        AlphaScenarioKind::FilletLineBezier => 0xf8_0000,
        AlphaScenarioKind::FilletNurbsLine => 0xf9_0000,
        AlphaScenarioKind::NurbsQuarterCircle => 0xfa_0000,
        AlphaScenarioKind::NurbsLocalSupport => 0xfb_0000,
        AlphaScenarioKind::NurbsPeriodic => 0xfc_0000,
        AlphaScenarioKind::NurbsDifferential => 0xfd_0000,
        AlphaScenarioKind::ProfileAllFamilies => 0xfe_0000,
        AlphaScenarioKind::ProfileCurvedTopology => 0xff_0000,
        AlphaScenarioKind::ProfileFilletTrim => 0x100_0000,
        AlphaScenarioKind::ProfileNurbsSelfIntersection => 0x103_0000,
        AlphaScenarioKind::ProfileIncomplete => 0x101_0000,
        AlphaScenarioKind::ProfileBudget => 0x102_0000,
    };
    let mut document =
        SketchDocument::with_id(10.0 * scale, DocumentId(PersistentId::from_u128(namespace)))?;
    let (ids, request) = match kind {
        AlphaScenarioKind::A1 => (
            AlphaScenarioIds::A1(add_a1(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::A2 => (
            AlphaScenarioIds::A2(add_a2(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::A3 => (
            AlphaScenarioIds::A3(add_a3(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::A4 => (
            AlphaScenarioIds::A4(add_a4(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::A5 => (
            AlphaScenarioIds::A5(add_a5(&mut document, scale, [0.0, 0.0])?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::A8 => {
            let a1 = add_a1(&mut document, scale, [0.0, 0.0])?;
            let a3 = add_a3(&mut document, scale, [20.0 * scale, 0.0])?;
            let a4 = add_a4(&mut document, scale, [40.0 * scale, 0.0])?;
            let a5 = add_a5(&mut document, scale, [60.0 * scale, 0.0])?;
            (
                AlphaScenarioIds::A8(Box::new(A8ScenarioIds { a1, a3, a4, a5 })),
                DocumentSolveRequest::default(),
            )
        }
        AlphaScenarioKind::Corpus => {
            add_alpha_corpus(&mut document, scale)?;
            (AlphaScenarioIds::Corpus, DocumentSolveRequest::default())
        }
        AlphaScenarioKind::StressCompass => (
            AlphaScenarioIds::StressCompass(add_stress_compass(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::StressBridge => (
            AlphaScenarioIds::StressBridge(add_stress_bridge(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::MotionCam => (
            AlphaScenarioIds::MotionCam(add_motion_cam(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::MotionOrbit => (
            AlphaScenarioIds::MotionOrbit(add_motion_orbit(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::MotionTrammel => (
            AlphaScenarioIds::MotionTrammel(add_motion_trammel(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::MotionScotchYoke => (
            AlphaScenarioIds::MotionScotchYoke(add_motion_scotch_yoke(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::MotionRotatingSquare => (
            AlphaScenarioIds::MotionRotatingSquare(add_motion_rotating_square(
                &mut document,
                scale,
            )?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::MotionScissor => (
            AlphaScenarioIds::MotionScissor(add_motion_scissor(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::MotionScissorTower => (
            AlphaScenarioIds::MotionScissorTower(add_motion_scissor_tower(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::MotionPeaucellier => (
            AlphaScenarioIds::MotionPeaucellier(add_motion_peaucellier(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::MotionFourBarCoupler => (
            AlphaScenarioIds::MotionFourBarCoupler(add_motion_four_bar_coupler(
                &mut document,
                scale,
            )?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::MotionPantograph => (
            AlphaScenarioIds::MotionPantograph(add_motion_pantograph(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::MotionDrawingArm => (
            AlphaScenarioIds::MotionDrawingArm(add_motion_drawing_arm(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::BranchLockedElbow => (
            AlphaScenarioIds::BranchLockedElbow(add_branch_locked_elbow(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::BranchFourBar => (
            AlphaScenarioIds::BranchFourBar(add_branch_four_bar(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::DiagnosticRankDrop => (
            AlphaScenarioIds::DiagnosticRankDrop(add_diagnostic_rank_drop(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::DiagnosticEndpointBound => (
            AlphaScenarioIds::DiagnosticEndpointBound(add_diagnostic_endpoint_bound(
                &mut document,
                scale,
            )?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::DiagnosticRedundancy => (
            AlphaScenarioIds::DiagnosticRedundancy(add_diagnostic_redundancy(
                &mut document,
                scale,
            )?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::ConicGallery => (
            AlphaScenarioIds::ConicGallery(add_conic_gallery(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::ConicTangency => (
            AlphaScenarioIds::ConicTangency(add_conic_tangency(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::ConicCircleLimit => (
            AlphaScenarioIds::ConicCircleLimit(add_conic_circle_limit(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::M28TrimmedFillet => (
            AlphaScenarioIds::M28TrimmedFillet(add_m28_trimmed_fillet(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::SupportingOffset => (
            AlphaScenarioIds::SupportingOffset(add_supporting_offset(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::ExactTranslatedOffset => (
            AlphaScenarioIds::ExactTranslatedOffset(add_exact_translated_offset(
                &mut document,
                scale,
            )?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::EntityMirror => (
            AlphaScenarioIds::EntityMirror(add_entity_mirror(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::DirectedAngle => (
            AlphaScenarioIds::DirectedAngle(add_directed_angle(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::M27ReferenceFillet => (
            AlphaScenarioIds::M27ReferenceFillet(add_m27_reference_fillet(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::FilletLineCircle => (
            AlphaScenarioIds::FilletLineCircle(add_line_circle_fillet(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::FilletLineBezier => (
            AlphaScenarioIds::FilletLineBezier(add_line_bezier_fillet(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::FilletNurbsLine => (
            AlphaScenarioIds::FilletNurbsLine(add_nurbs_line_fillet(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::NurbsQuarterCircle => (
            AlphaScenarioIds::NurbsQuarterCircle(add_nurbs_quarter_circle(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::NurbsLocalSupport => (
            AlphaScenarioIds::NurbsLocalSupport(add_nurbs_local_support(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::NurbsPeriodic => (
            AlphaScenarioIds::NurbsPeriodic(add_periodic_nurbs(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::NurbsDifferential => (
            AlphaScenarioIds::NurbsDifferential(add_nurbs_differential(&mut document, scale)?),
            DocumentSolveRequest::default().without_previous_state_preferences(),
        ),
        AlphaScenarioKind::ProfileAllFamilies => (
            AlphaScenarioIds::ProfileAllFamilies(add_profile_all_families(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::ProfileCurvedTopology => (
            AlphaScenarioIds::ProfileCurvedTopology(add_profile_curved_topology(
                &mut document,
                scale,
            )?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::ProfileFilletTrim => (
            AlphaScenarioIds::ProfileFilletTrim(add_profile_fillet_trim(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::ProfileNurbsSelfIntersection => (
            AlphaScenarioIds::ProfileNurbsSelfIntersection(add_profile_nurbs_self_intersection(
                &mut document,
                scale,
            )?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::ProfileIncomplete => (
            AlphaScenarioIds::ProfileIncomplete(add_profile_incomplete(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
        AlphaScenarioKind::ProfileBudget => (
            AlphaScenarioIds::ProfileBudget(add_profile_curved_topology(&mut document, scale)?),
            DocumentSolveRequest::default(),
        ),
    };
    Ok(AlphaScenarioFixture {
        document,
        request,
        ids,
    })
}

/// Builds a deterministic disconnected mixed-alpha performance document.
///
/// Small contains one A1/A3/A4/A5 tile. Medium contains eight translated tiles.
///
/// # Errors
///
/// Returns an unexpected document construction or validation error.
pub fn alpha_performance_document(
    size: AlphaPerformanceSize,
) -> Result<SketchDocument, DocumentError> {
    let count = match size {
        AlphaPerformanceSize::Small => 1,
        AlphaPerformanceSize::Medium => 8,
    };
    let namespace = match size {
        AlphaPerformanceSize::Small => 0xb1_0000,
        AlphaPerformanceSize::Medium => 0xb8_0000,
    };
    let mut document =
        SketchDocument::with_id(10.0, DocumentId(PersistentId::from_u128(namespace)))?;
    for index in 0..count {
        let origin = [f64::from(index) * 80.0, 0.0];
        add_a1(&mut document, 1.0, origin)?;
        add_a3(&mut document, 1.0, [origin[0] + 20.0, 0.0])?;
        add_a4(&mut document, 1.0, [origin[0] + 40.0, 0.0])?;
        add_a5(&mut document, 1.0, [origin[0] + 60.0, 0.0])?;
    }
    Ok(document)
}

fn translated(origin: [f64; 2], point: [f64; 2], scale: f64) -> [f64; 2] {
    [origin[0] + point[0] * scale, origin[1] + point[1] * scale]
}

fn add_line(
    document: &mut SketchDocument,
    label: &str,
    start: DesignPointId,
    end: DesignPointId,
) -> Result<CurveId, DocumentError> {
    let first = document
        .point(start)
        .ok_or(DocumentError::UnknownId {
            kind: "point",
            id: start.0,
        })?
        .position;
    let second = document
        .point(end)
        .ok_or(DocumentError::UnknownId {
            kind: "point",
            id: end.0,
        })?
        .position;
    let delta = [second[0] - first[0], second[1] - first[1]];
    let norm = delta[0].hypot(delta[1]);
    document.add_curve(
        label,
        CurveDefinition::Line {
            start,
            end,
            branch_direction: [delta[0] / norm, delta[1] / norm],
        },
    )
}

fn fix_point(
    document: &mut SketchDocument,
    label: &str,
    point: DesignPointId,
) -> Result<DocumentConstraintId, DocumentError> {
    let target = document
        .point(point)
        .ok_or(DocumentError::UnknownId {
            kind: "point",
            id: point.0,
        })?
        .position;
    document.add_constraint(
        label,
        DocumentConstraintDefinition::FixedPoint { point, target },
    )
}

fn add_length_dimension(
    document: &mut SketchDocument,
    label: &str,
    curve: CurveSpan,
    value: f64,
    mode: DocumentDimensionMode,
) -> Result<DocumentDimensionId, DocumentError> {
    let target = document.add_scalar(
        format!("{label} target"),
        value,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        label,
        DocumentDimensionDefinition::CurveLength { curve, target },
        mode,
    )
}

fn add_radius_dimension(
    document: &mut SketchDocument,
    label: &str,
    curve: CurveId,
    value: f64,
) -> Result<DocumentDimensionId, DocumentError> {
    let target = document.add_scalar(
        format!("{label} target"),
        value,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        label,
        DocumentDimensionDefinition::Radius { curve, target },
        DocumentDimensionMode::Driving,
    )
}

fn add_a1(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A1ScenarioIds, DocumentError> {
    let rectangle = document.add_rectangle("A1 rectangle", origin, 4.0 * scale, 3.0 * scale)?;
    let width_source = document
        .dimension(rectangle.dimensions[0])
        .expect("new rectangle width dimension")
        .source_id;
    document.set_source_label(width_source, "width-4")?;
    let diagonal_target = document.add_scalar(
        "A1 diagonal target",
        5.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let diagonal = document.add_dimension(
        "A1 diagonal reference",
        DocumentDimensionDefinition::PointDistance {
            first: rectangle.points[0],
            second: rectangle.points[2],
            target: diagonal_target,
        },
        DocumentDimensionMode::Reference,
    )?;
    Ok(A1ScenarioIds {
        rectangle,
        diagonal_target,
        diagonal,
    })
}

fn add_a2(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A2ScenarioIds, DocumentError> {
    let a = document.add_point("A2 A", translated(origin, [0.0, 0.0], scale))?;
    let b = document.add_point("A2 B", translated(origin, [4.0, 0.0], scale))?;
    let c = document.add_point("A2 C", translated(origin, [2.2, 2.0], scale))?;
    let ab = add_line(document, "A2 AB", a, b)?;
    fix_point(document, "A2 A fixed", a)?;
    document.add_constraint(
        "A2 AB horizontal",
        DocumentConstraintDefinition::Horizontal {
            line: CurveSpan::line(ab),
        },
    )?;
    add_length_dimension(
        document,
        "A2 AB length",
        CurveSpan::line(ab),
        4.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    let distance_target = document.add_scalar(
        "A2 AC target",
        3.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let distance_ac = document.add_dimension(
        "A2 AC distance",
        DocumentDimensionDefinition::PointDistance {
            first: a,
            second: c,
            target: distance_target,
        },
        DocumentDimensionMode::Driving,
    )?;
    Ok(A2ScenarioIds {
        a,
        b,
        c,
        ab,
        distance_ac,
    })
}

fn add_a3(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A3ScenarioIds, DocumentError> {
    let start = document.add_point("A3 line start", translated(origin, [-5.0, 0.0], scale))?;
    let end = document.add_point("A3 line end", translated(origin, [5.0, 0.0], scale))?;
    let guide = document.add_point("A3 guide G", translated(origin, [1.0, 0.0], scale))?;
    let center = document.add_point("A3 circle center O", translated(origin, [1.0, 3.0], scale))?;
    let line = add_line(document, "A3 fixed line", start, end)?;
    let guide_line = add_line(document, "A3 vertical guide", guide, center)?;
    let radius = document.add_scalar(
        "A3 circle radius",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let circle = document.add_curve("A3 circle", CurveDefinition::Circle { center, radius })?;
    fix_point(document, "A3 line start fixed", start)?;
    fix_point(document, "A3 line end fixed", end)?;
    fix_point(document, "A3 guide fixed", guide)?;
    document.add_constraint(
        "A3 center guide vertical",
        DocumentConstraintDefinition::Vertical {
            line: CurveSpan::line(guide_line),
        },
    )?;
    add_radius_dimension(document, "A3 radius 2", circle, 2.0 * scale)?;
    let line_contact = document.add_curve_contact(
        "A3 line contact",
        CurveSpan::line(line),
        0.6,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    let circle_contact = document.add_curve_contact(
        "A3 circle contact",
        CurveSpan::line(circle),
        1.5 * PI,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    let tangency = document.add_constraint(
        "A3 line-circle tangency",
        DocumentConstraintDefinition::LineCircleTangency {
            line_contact,
            circle_contact,
            side: DocumentLineSide::Left,
        },
    )?;
    Ok(A3ScenarioIds {
        line,
        guide,
        center,
        circle,
        line_contact,
        circle_contact,
        tangency,
    })
}

fn add_a4(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A4ScenarioIds, DocumentError> {
    let arc_center = document.add_point("A4 arc center", origin)?;
    let circle_center =
        document.add_point("A4 circle center", translated(origin, [8.0, 0.0], scale))?;
    let arc_radius = document.add_scalar(
        "A4 arc radius",
        5.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let start_angle = document.add_scalar(
        "A4 arc start",
        -5.0 * PI / 6.0,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    let end_angle = document.add_scalar(
        "A4 arc end",
        5.0 * PI / 6.0,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    let arc = document.add_curve(
        "A4 300 degree arc",
        CurveDefinition::CircularArc {
            center: arc_center,
            radius: arc_radius,
            start_angle,
            end_angle,
            sweep: DocumentArcSweep::CounterClockwise,
        },
    )?;
    let circle_radius = document.add_scalar(
        "A4 free circle radius",
        3.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let circle = document.add_curve(
        "A4 free-radius circle",
        CurveDefinition::Circle {
            center: circle_center,
            radius: circle_radius,
        },
    )?;
    fix_point(document, "A4 arc center fixed", arc_center)?;
    add_radius_dimension(document, "A4 arc radius 5", arc, 5.0 * scale)?;
    let circle_contact = document.add_curve_contact(
        "A4 circle contact",
        CurveSpan::line(circle),
        PI,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let arc_contact = document.add_curve_contact(
        "A4 arc contact",
        CurveSpan::line(arc),
        0.5,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let tangency = document.add_constraint(
        "A4 circle-arc tangency",
        DocumentConstraintDefinition::CircleArcTangency {
            circle_contact,
            arc_contact,
            side: DocumentArcTangencySide::OutsideArc,
        },
    )?;
    Ok(A4ScenarioIds {
        arc_center,
        circle_center,
        arc,
        circle,
        circle_radius,
        circle_contact,
        arc_contact,
        tangency,
    })
}

fn add_a5(
    document: &mut SketchDocument,
    scale: f64,
    origin: [f64; 2],
) -> Result<A5ScenarioIds, DocumentError> {
    let controls = [
        document.add_point("A5 P0", translated(origin, [0.0, 0.0], scale))?,
        document.add_point("A5 P1", translated(origin, [1.0, 0.0], scale))?,
        document.add_point("A5 P2", translated(origin, [2.0, 1.0], scale))?,
        document.add_point("A5 P3", translated(origin, [3.0, 1.0], scale))?,
    ];
    let a = document.add_point("A5 line A", translated(origin, [0.0, 0.0], scale))?;
    let b = document.add_point("A5 line B", translated(origin, [2.0, 0.0], scale))?;
    let line = add_line(document, "A5 tangent line", a, b)?;
    let bezier =
        document.add_curve("A5 cubic Bezier", CurveDefinition::CubicBezier { controls })?;
    fix_point(document, "A5 A fixed", a)?;
    add_length_dimension(
        document,
        "A5 line length 2",
        CurveSpan::line(line),
        2.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    let bezier_contact = document.add_curve_contact(
        "A5 Bezier start contact",
        CurveSpan::line(bezier),
        0.0,
        0,
        ContactNeighborhood::Start,
        Some(TangentOrientation::Aligned),
    )?;
    let tangency = document.add_constraint(
        "A5 line-Bezier tangency",
        DocumentConstraintDefinition::LineCurveTangency {
            line: CurveSpan::line(line),
            endpoint: FeatureEndpoint::Start,
            curve_contact: bezier_contact,
        },
    )?;
    Ok(A5ScenarioIds {
        controls,
        a,
        b,
        line,
        bezier,
        bezier_contact,
        tangency,
    })
}

fn add_m28_trimmed_fillet(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<M28TrimmedFilletIds, DocumentError> {
    let circle_center = document.add_point("M28 circle center", [0.0, 0.0])?;
    let line_start = document.add_point("M28 hidden line endpoint", [0.0, scale])?;
    let line_end = document.add_point("M28 visible line endpoint", [6.0 * scale, scale])?;
    let circle_radius = document.add_scalar(
        "M28 parent circle radius",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let circle = document.add_curve(
        "M28 trimmed circle parent",
        CurveDefinition::Circle {
            center: circle_center,
            radius: circle_radius,
        },
    )?;
    let line = add_line(document, "M28 trimmed line parent", line_start, line_end)?;
    fix_point(document, "M28 circle center fixed", circle_center)?;
    fix_point(document, "M28 line start fixed", line_start)?;
    fix_point(document, "M28 line end fixed", line_end)?;
    add_radius_dimension(
        document,
        "M28 parent circle radius fixed",
        circle,
        2.0 * scale,
    )?;
    let fillet = document.add_curve_curve_fillet(
        "M28 trimmed line-circle fillet",
        CurveCurveFilletRequest {
            first: CurveFilletParentRequest {
                curve: CurveSpan::line(circle),
                parameter: 0.0,
                winding: 0,
                neighborhood: ContactNeighborhood::Local {
                    lower: -0.4,
                    upper: 0.4,
                },
                side: DocumentCurveNormalSide::Right,
                trim_endpoint: DocumentFilletTrimEndpoint::End,
                periodic_anchor: Some(DocumentTrimParameter {
                    parameter: PI,
                    winding: -1,
                }),
            },
            second: CurveFilletParentRequest {
                curve: CurveSpan::line(line),
                parameter: 0.5,
                winding: 0,
                neighborhood: ContactNeighborhood::Interior,
                side: DocumentCurveNormalSide::Right,
                trim_endpoint: DocumentFilletTrimEndpoint::Start,
                periodic_anchor: None,
            },
            endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
            sweep: DocumentArcSweep::CounterClockwise,
            radius: scale,
            radius_mode: DocumentDimensionMode::Driving,
        },
    )?;
    Ok(M28TrimmedFilletIds {
        circle,
        line,
        fillet,
    })
}

fn add_profile_fillet_trim(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<M28TrimmedFilletIds, DocumentError> {
    let ids = add_m28_trimmed_fillet(document, scale)?;
    let CurveDefinition::Line { end: line_end, .. } = document
        .curve(ids.line)
        .ok_or(DocumentError::UnknownId {
            kind: "curve",
            id: ids.line.0,
        })?
        .definition
    else {
        return Err(DocumentError::InvalidField {
            field: "profile fillet closure",
            message: "trimmed line parent must remain ordinary line geometry".into(),
        });
    };
    let diagonal = std::f64::consts::SQRT_2 * scale;
    let circle_start =
        document.add_point("Profile fillet circle closure", [diagonal, -diagonal])?;
    add_line(
        document,
        "Profile fillet ordinary closure",
        line_end,
        circle_start,
    )?;
    let contact = document.add_curve_contact(
        "Profile fillet circle closure contact",
        CurveSpan::line(ids.circle),
        1.75 * PI,
        -1,
        ContactNeighborhood::Local {
            lower: -0.25 * PI - 0.1,
            upper: -0.25 * PI + 0.1,
        },
        None,
    )?;
    document.add_constraint(
        "Profile fillet circle closure join",
        DocumentConstraintDefinition::PointOnCurve {
            point: circle_start,
            contact,
        },
    )?;
    Ok(ids)
}

fn add_profile_nurbs_self_intersection(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<NurbsLabIds, DocumentError> {
    let controls = [[0.0, 0.0], [2.0, 3.0], [-2.0, 3.0], [84.0 / 79.0, 0.0]]
        .into_iter()
        .enumerate()
        .map(|(index, point)| {
            document.add_point(
                format!("Profile self-intersecting NURBS control {}", index + 1),
                [point[0] * scale, point[1] * scale],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let weights = add_nurbs_weights(
        document,
        "Profile self-intersecting NURBS",
        &[1.0, 0.9, 1.1, 0.95],
    )?;
    let curve = document.add_curve(
        "Profile self-intersecting NURBS",
        CurveDefinition::Nurbs {
            form: DocumentBSplineForm::Clamped,
            degree: 3,
            controls: controls.clone(),
            weights: weights.clone(),
            gauge_weight: weights[0],
            knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            span_ids: vec![103],
            next_span_id: 104,
        },
    )?;
    Ok(NurbsLabIds {
        curve,
        primary_control: controls[1],
        controls,
        weights,
        contact: None,
    })
}

fn add_profile_circle(
    document: &mut SketchDocument,
    label: &str,
    center_position: [f64; 2],
    radius_value: f64,
) -> Result<CurveId, DocumentError> {
    let center = document.add_point(format!("{label} center"), center_position)?;
    let radius = document.add_scalar(
        format!("{label} radius"),
        radius_value,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_curve(label, CurveDefinition::Circle { center, radius })
}

fn add_profile_ellipse(
    document: &mut SketchDocument,
    label: &str,
    center_position: [f64; 2],
    axis_position: [f64; 2],
    ratio_value: f64,
) -> Result<CurveId, DocumentError> {
    let center = document.add_point(format!("{label} center"), center_position)?;
    let major_axis_point = document.add_point(format!("{label} axis"), axis_position)?;
    let minor_axis_ratio = document.add_scalar(
        format!("{label} ratio"),
        ratio_value,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    )?;
    document.add_curve(
        label,
        CurveDefinition::Ellipse {
            center,
            major_axis_point,
            minor_axis_ratio,
        },
    )
}

fn join_profile_endpoint(
    document: &mut SketchDocument,
    label: &str,
    point: DesignPointId,
    curve: CurveId,
    parameter: f64,
    neighborhood: ContactNeighborhood,
) -> Result<(), DocumentError> {
    let contact = document.add_curve_contact(
        format!("{label} contact"),
        CurveSpan::line(curve),
        parameter,
        0,
        neighborhood,
        None,
    )?;
    document.add_constraint(
        format!("{label} join"),
        DocumentConstraintDefinition::PointOnCurve { point, contact },
    )?;
    Ok(())
}

fn close_profile_curve(
    document: &mut SketchDocument,
    label: &str,
    curve: CurveId,
) -> Result<CurveId, DocumentError> {
    let span = CurveSpan::line(curve);
    let endpoint = |parameter| {
        document
            .evaluate_curve_jet(span, parameter)
            .map(|jet| [jet.position.x, jet.position.y])
            .map_err(|error| DocumentError::InvalidField {
                field: "profile curve closure",
                message: error.to_string(),
            })
    };
    let first_position = endpoint(0.0)?;
    let second_position = endpoint(1.0)?;
    let first = document.add_point(format!("{label} first endpoint"), first_position)?;
    let second = document.add_point(format!("{label} second endpoint"), second_position)?;
    let closure = add_line(document, &format!("{label} closure"), second, first)?;
    join_profile_endpoint(
        document,
        &format!("{label} first"),
        first,
        curve,
        0.0,
        ContactNeighborhood::Start,
    )?;
    join_profile_endpoint(
        document,
        &format!("{label} second"),
        second,
        curve,
        1.0,
        ContactNeighborhood::End,
    )?;
    Ok(closure)
}

#[allow(clippy::too_many_lines)]
fn add_profile_all_families(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<ProfileScenarioIds, DocumentError> {
    let point = |x: f64, y: f64| [x * scale, y * scale];

    let polyline_points = [point(0.0, 0.0), point(2.0, 0.0), point(1.0, 2.0)]
        .map(|position| document.add_point("Profile polyline point", position))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let diagonal = 5.0_f64.sqrt();
    let polyline = document.add_curve(
        "Profile closed polyline",
        CurveDefinition::Polyline {
            points: polyline_points,
            closed: true,
            branch_directions: vec![
                [1.0, 0.0],
                [-1.0 / diagonal, 2.0 / diagonal],
                [-1.0 / diagonal, -2.0 / diagonal],
            ],
        },
    )?;

    let line_points = [
        point(10.0, 0.0),
        point(12.0, 0.0),
        point(12.0, 2.0),
        point(10.0, 2.0),
    ]
    .map(|position| document.add_point("Profile line point", position))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let mut square_lines = Vec::with_capacity(4);
    for index in 0..4 {
        square_lines.push(add_line(
            document,
            "Profile line square edge",
            line_points[index],
            line_points[(index + 1) % 4],
        )?);
    }
    let line_family = square_lines[0];

    let circle = add_profile_circle(document, "Profile circle", point(21.0, 1.0), scale)?;
    let ellipse = add_profile_ellipse(
        document,
        "Profile ellipse",
        point(31.0, 1.0),
        point(33.0, 1.0),
        0.5,
    )?;

    let quadratic_controls = [point(40.0, 0.0), point(41.0, 3.0), point(42.0, 0.0)]
        .map(|position| document.add_point("Profile quadratic control", position))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
    let quadratic_controls: [DesignPointId; 3] = quadratic_controls
        .try_into()
        .expect("three quadratic controls");
    let quadratic = document.add_curve(
        "Profile quadratic Bezier",
        CurveDefinition::QuadraticBezier {
            controls: quadratic_controls,
        },
    )?;
    add_line(
        document,
        "Profile quadratic closure",
        quadratic_controls[2],
        quadratic_controls[0],
    )?;

    let cubic_controls = [
        point(50.0, 0.0),
        point(50.5, 3.0),
        point(51.5, 3.0),
        point(52.0, 0.0),
    ]
    .map(|position| document.add_point("Profile cubic control", position))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let cubic_controls: [DesignPointId; 4] =
        cubic_controls.try_into().expect("four cubic controls");
    let cubic = document.add_curve(
        "Profile cubic Bezier",
        CurveDefinition::CubicBezier {
            controls: cubic_controls,
        },
    )?;
    add_line(
        document,
        "Profile cubic closure",
        cubic_controls[3],
        cubic_controls[0],
    )?;

    let rational_start = document.add_point("Profile rational start", point(60.0, 0.0))?;
    let rational_end = document.add_point("Profile rational end", point(62.0, 0.0))?;
    let rational_weight = document.add_scalar(
        "Profile rational weight",
        0.75,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
            upper: f64::MAX,
        },
    )?;
    let rational = document.add_curve(
        "Profile rational quadratic conic",
        CurveDefinition::RationalQuadraticConic {
            start: rational_start,
            weighted_middle: point(45.75, 2.25),
            middle_weight: rational_weight,
            end: rational_end,
        },
    )?;
    add_line(
        document,
        "Profile rational closure",
        rational_end,
        rational_start,
    )?;

    let circular_center = document.add_point("Profile circular arc center", point(71.0, 0.0))?;
    let circular_radius = document.add_scalar(
        "Profile circular arc radius",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let circular_start = document.add_scalar(
        "Profile circular arc start",
        0.0,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    let circular_end = document.add_scalar(
        "Profile circular arc end",
        PI,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    let circular_arc = document.add_curve(
        "Profile circular arc",
        CurveDefinition::CircularArc {
            center: circular_center,
            radius: circular_radius,
            start_angle: circular_start,
            end_angle: circular_end,
            sweep: DocumentArcSweep::CounterClockwise,
        },
    )?;
    close_profile_curve(document, "Profile circular arc", circular_arc)?;

    let elliptical_center =
        document.add_point("Profile elliptical arc center", point(81.0, 0.0))?;
    let elliptical_axis = document.add_point("Profile elliptical arc axis", point(83.0, 0.0))?;
    let elliptical_ratio = document.add_scalar(
        "Profile elliptical arc ratio",
        0.5,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    )?;
    let elliptical_start = document.add_scalar(
        "Profile elliptical arc start",
        0.0,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    let elliptical_end = document.add_scalar(
        "Profile elliptical arc end",
        PI,
        ScalarUnit::Angle,
        ScalarDomain::Finite,
    )?;
    let elliptical_arc = document.add_curve(
        "Profile elliptical arc",
        CurveDefinition::EllipticalArc {
            center: elliptical_center,
            major_axis_point: elliptical_axis,
            minor_axis_ratio: elliptical_ratio,
            start_angle: elliptical_start,
            end_angle: elliptical_end,
            sweep: DocumentArcSweep::CounterClockwise,
        },
    )?;
    close_profile_curve(document, "Profile elliptical arc", elliptical_arc)?;

    let parabola_vertex = document.add_point("Profile parabola vertex", point(91.0, 0.0))?;
    let parabola_focus = document.add_point("Profile parabola focus", point(91.0, 0.5))?;
    let parabola_start = document.add_scalar(
        "Profile parabola start",
        -2.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    )?;
    let parabola_end = document.add_scalar(
        "Profile parabola end",
        2.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    )?;
    let parabola = document.add_curve(
        "Profile parabola",
        CurveDefinition::ParabolaSegment {
            vertex: parabola_vertex,
            focus: parabola_focus,
            trim_start: parabola_start,
            trim_end: parabola_end,
        },
    )?;
    close_profile_curve(document, "Profile parabola", parabola)?;

    let hyperbola_center = document.add_point("Profile hyperbola center", point(101.0, 0.0))?;
    let hyperbola_axis = document.add_point("Profile hyperbola axis", point(102.0, 0.0))?;
    let hyperbola_conjugate = document.add_scalar(
        "Profile hyperbola conjugate",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let hyperbola_start = document.add_scalar(
        "Profile hyperbola start",
        -1.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    )?;
    let hyperbola_end = document.add_scalar(
        "Profile hyperbola end",
        1.0,
        ScalarUnit::Parameter,
        ScalarDomain::Finite,
    )?;
    let hyperbola = document.add_curve(
        "Profile hyperbola",
        CurveDefinition::HyperbolaSegment {
            center: hyperbola_center,
            transverse_axis_point: hyperbola_axis,
            semi_conjugate: hyperbola_conjugate,
            branch: DocumentHyperbolaBranch::Positive,
            trim_start: hyperbola_start,
            trim_end: hyperbola_end,
        },
    )?;
    close_profile_curve(document, "Profile hyperbola", hyperbola)?;

    let mut spline_curves = Vec::with_capacity(4);
    for (offset, form, rational_spline) in [
        (110.0, DocumentBSplineForm::Clamped, false),
        (120.0, DocumentBSplineForm::Periodic, false),
        (130.0, DocumentBSplineForm::Clamped, true),
        (140.0, DocumentBSplineForm::Periodic, true),
    ] {
        let coordinates = if form == DocumentBSplineForm::Clamped {
            vec![
                point(offset, 0.0),
                point(offset + 1.0, 3.0),
                point(offset + 2.0, 0.0),
            ]
        } else {
            vec![
                point(offset, 0.0),
                point(offset + 1.0, -1.0),
                point(offset + 2.0, 0.0),
                point(offset + 1.5, 2.0),
                point(offset + 0.5, 2.0),
            ]
        };
        let controls = coordinates
            .into_iter()
            .map(|position| document.add_point("Profile spline control", position))
            .collect::<Result<Vec<_>, _>>()?;
        let (knots, span_ids) = if form == DocumentBSplineForm::Clamped {
            (vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], vec![0])
        } else {
            (vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0], vec![0, 1, 2, 3, 4])
        };
        let next_span_id = u32::try_from(span_ids.len()).expect("bounded profile span count");
        let curve = if rational_spline {
            let weights = controls
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    document.add_scalar(
                        "Profile spline weight",
                        if form == DocumentBSplineForm::Clamped && index == 1 {
                            0.8
                        } else {
                            1.0
                        },
                        ScalarUnit::Parameter,
                        ScalarDomain::Positive,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            document.add_curve(
                "Profile NURBS",
                CurveDefinition::Nurbs {
                    form,
                    degree: 2,
                    controls: controls.clone(),
                    gauge_weight: weights[0],
                    weights,
                    knots,
                    span_ids,
                    next_span_id,
                },
            )?
        } else {
            document.add_curve(
                "Profile B-spline",
                CurveDefinition::BSpline {
                    form,
                    degree: 2,
                    controls: controls.clone(),
                    knots,
                    span_ids,
                    next_span_id,
                },
            )?
        };
        if form == DocumentBSplineForm::Clamped {
            add_line(document, "Profile spline closure", controls[2], controls[0])?;
        }
        spline_curves.push(curve);
    }

    for x in [
        1.0, 11.0, 21.0, 31.0, 41.0, 51.0, 61.0, 71.0, 81.0, 91.0, 111.0, 121.2, 131.0, 141.2,
    ] {
        let start = document.add_point("Profile splitter start", point(x, -4.0))?;
        let end = document.add_point("Profile splitter end", point(x, 4.0))?;
        add_line(document, "Profile vertical splitter", start, end)?;
    }
    let hyperbola_splitter_start =
        document.add_point("Profile hyperbola splitter start", point(99.0, 0.0))?;
    let hyperbola_splitter_end =
        document.add_point("Profile hyperbola splitter end", point(105.0, 0.0))?;
    add_line(
        document,
        "Profile hyperbola horizontal splitter",
        hyperbola_splitter_start,
        hyperbola_splitter_end,
    )?;

    Ok(ProfileScenarioIds {
        curves: vec![
            line_family,
            polyline,
            circle,
            circular_arc,
            ellipse,
            elliptical_arc,
            rational,
            parabola,
            hyperbola,
            quadratic,
            cubic,
            spline_curves[0],
            spline_curves[1],
            spline_curves[2],
            spline_curves[3],
        ],
    })
}

fn add_profile_curved_topology(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<ProfileScenarioIds, DocumentError> {
    let crossing_circle = add_profile_circle(
        document,
        "Profile transverse circle",
        [0.0, 0.0],
        2.0 * scale,
    )?;
    let crossing_ellipse = add_profile_ellipse(
        document,
        "Profile transverse ellipse",
        [0.0, 0.0],
        [3.0 * scale, 0.0],
        0.5,
    )?;
    let nested_circle = add_profile_circle(
        document,
        "Profile nested outer circle",
        [8.0 * scale, 0.0],
        3.0 * scale,
    )?;
    let nested_ellipse = add_profile_ellipse(
        document,
        "Profile nested inner ellipse",
        [8.0 * scale, 0.0],
        [9.5 * scale, 0.0],
        0.5,
    )?;
    Ok(ProfileScenarioIds {
        curves: vec![
            crossing_circle,
            crossing_ellipse,
            nested_circle,
            nested_ellipse,
        ],
    })
}

fn add_profile_incomplete(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<ProfileScenarioIds, DocumentError> {
    let clean = add_profile_circle(
        document,
        "Profile retained clean circle",
        [10.0 * scale, 0.0],
        scale,
    )?;
    let first_tangent =
        add_profile_circle(document, "Profile tangent circle A", [0.0, 0.0], scale)?;
    let second_tangent = add_profile_circle(
        document,
        "Profile tangent circle B",
        [2.0 * scale, 0.0],
        scale,
    )?;
    Ok(ProfileScenarioIds {
        curves: vec![clean, first_tangent, second_tangent],
    })
}

fn add_supporting_offset(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<SupportingOffsetIds, DocumentError> {
    let source_points = [
        document.add_point(
            "Supporting offset fixed source start",
            [-4.0 * scale, -2.0 * scale],
        )?,
        document.add_point(
            "Supporting offset fixed source end",
            [4.0 * scale, -2.0 * scale],
        )?,
    ];
    let target_points = [
        document.add_point(
            "Supporting offset draggable target start",
            [-3.0 * scale, 0.0],
        )?,
        document.add_point("Supporting offset draggable target end", [2.0 * scale, 0.0])?,
    ];
    let source = add_line(
        document,
        "Supporting offset source",
        source_points[0],
        source_points[1],
    )?;
    let target = add_line(
        document,
        "Supporting offset target",
        target_points[0],
        target_points[1],
    )?;
    for (index, point) in source_points.into_iter().enumerate() {
        fix_point(
            document,
            &format!("Supporting offset source fixed {index}"),
            point,
        )?;
    }
    let offset = document.add_scalar(
        "Supporting offset distance",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let dimension = document.add_dimension(
        "Supporting-line offset / 2 DOF",
        DocumentDimensionDefinition::SupportingLineOffset {
            source: CurveSpan::line(source),
            target_segment: CurveSpan::line(target),
            target: offset,
            side: DocumentLineSide::Left,
            orientation: DocumentLineOffsetOrientation::Same,
        },
        DocumentDimensionMode::Driving,
    )?;
    Ok(SupportingOffsetIds {
        source,
        target,
        target_points,
        dimension,
    })
}

fn add_exact_translated_offset(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<ExactTranslatedOffsetIds, DocumentError> {
    let source_start =
        document.add_point("Exact offset anchored source start", [-3.0 * scale, -scale])?;
    let source_end = document.add_point("Exact offset draggable source end", [scale, -scale])?;
    let target_points = [
        document.add_point(
            "Exact offset associated target start",
            [-3.0 * scale, scale],
        )?,
        document.add_point("Exact offset associated target end", [scale, scale])?,
    ];
    let source = add_line(
        document,
        "Exact offset rotating source",
        source_start,
        source_end,
    )?;
    let target = add_line(
        document,
        "Exact offset translated target",
        target_points[0],
        target_points[1],
    )?;
    fix_point(document, "Exact offset source anchor fixed", source_start)?;
    add_length_dimension(
        document,
        "Exact offset source length 4",
        CurveSpan::line(source),
        4.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    let offset = document.add_scalar(
        "Exact translated offset distance",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let dimension = document.add_dimension(
        "Exact translated-segment offset / 1 rotational DOF",
        DocumentDimensionDefinition::ExactTranslatedSegmentOffset {
            source: CurveSpan::line(source),
            target_segment: CurveSpan::line(target),
            target: offset,
            side: DocumentLineSide::Left,
            orientation: DocumentLineOffsetOrientation::Same,
        },
        DocumentDimensionMode::Driving,
    )?;
    Ok(ExactTranslatedOffsetIds {
        source,
        source_end,
        target,
        target_points,
        dimension,
    })
}

fn add_entity_mirror(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<EntityMirrorIds, DocumentError> {
    let axis_points = [
        document.add_point("Mirror axis fixed start", [-5.0 * scale, 0.0])?,
        document.add_point("Mirror axis fixed end", [5.0 * scale, 0.0])?,
    ];
    let axis = add_line(
        document,
        "Mirror construction axis",
        axis_points[0],
        axis_points[1],
    )?;
    for (index, point) in axis_points.into_iter().enumerate() {
        fix_point(document, &format!("Mirror axis fixed {index}"), point)?;
    }
    let source_start = document.add_point("Mirror source anchored start", [-2.0 * scale, scale])?;
    let source_end = document.add_point("Mirror source draggable end", [scale, 2.0 * scale])?;
    let source = add_line(document, "Mirror source line", source_start, source_end)?;
    fix_point(document, "Mirror source start fixed", source_start)?;
    add_length_dimension(
        document,
        "Mirror source length sqrt(10)",
        CurveSpan::line(source),
        10.0_f64.sqrt() * scale,
        DocumentDimensionMode::Driving,
    )?;
    let mirror =
        document.add_mirrored_curve("Associative entity mirror", source, CurveSpan::line(axis))?;
    let mirrored_end = mirror
        .point_pairs
        .iter()
        .find_map(|(source, mirrored)| (*source == source_end).then_some(*mirrored))
        .expect("mirrored source endpoint");
    Ok(EntityMirrorIds {
        axis,
        source_end,
        mirrored_end,
        mirror,
    })
}

fn add_directed_angle(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<DirectedAngleIds, DocumentError> {
    let first_angle = 175.0_f64.to_radians();
    let second_angle = -175.0_f64.to_radians();
    let origin = document.add_point("Directed angle fixed vertex", [0.0, 0.0])?;
    let first_tip = document.add_point(
        "Directed angle fixed reference tip",
        [
            4.0 * scale * first_angle.cos(),
            4.0 * scale * first_angle.sin(),
        ],
    )?;
    let moving_tip = document.add_point(
        "Directed angle draggable branch-cut tip",
        [
            3.0 * scale * second_angle.cos(),
            3.0 * scale * second_angle.sin(),
        ],
    )?;
    let first = add_line(document, "Directed angle reference ray", origin, first_tip)?;
    let second = add_line(document, "Directed angle moving ray", origin, moving_tip)?;
    fix_point(document, "Directed angle vertex fixed", origin)?;
    fix_point(document, "Directed angle reference tip fixed", first_tip)?;
    add_length_dimension(
        document,
        "Directed angle moving radius 3",
        CurveSpan::line(second),
        3.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    let target = document.add_scalar(
        "Directed angle editable target",
        10.0_f64.to_radians(),
        ScalarUnit::Angle,
        ScalarDomain::Positive,
    )?;
    let dimension = document.add_dimension(
        "Directed angle reference / branch cut",
        DocumentDimensionDefinition::OrientedAngle {
            first: CurveSpan::line(first),
            second: CurveSpan::line(second),
            target,
            orientation: crate::DocumentAngleOrientation::CounterClockwise,
        },
        DocumentDimensionMode::Reference,
    )?;
    Ok(DirectedAngleIds {
        first,
        second,
        moving_tip,
        dimension,
        target,
    })
}

fn add_fixed_crossing_lines(
    document: &mut SketchDocument,
    scale: f64,
    prefix: &str,
) -> Result<[CurveId; 2], DocumentError> {
    let points = [
        document.add_point(format!("{prefix} horizontal start"), [-4.0 * scale, 0.0])?,
        document.add_point(format!("{prefix} horizontal end"), [4.0 * scale, 0.0])?,
        document.add_point(format!("{prefix} vertical start"), [0.0, -4.0 * scale])?,
        document.add_point(format!("{prefix} vertical end"), [0.0, 4.0 * scale])?,
    ];
    let curves = [
        add_line(
            document,
            &format!("{prefix} horizontal parent"),
            points[0],
            points[1],
        )?,
        add_line(
            document,
            &format!("{prefix} vertical parent"),
            points[2],
            points[3],
        )?,
    ];
    for (index, point) in points.into_iter().enumerate() {
        fix_point(
            document,
            &format!("{prefix} parent point {index} fixed"),
            point,
        )?;
    }
    Ok(curves)
}

fn add_m27_reference_fillet(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<M27ReferenceFilletIds, DocumentError> {
    let parents = add_fixed_crossing_lines(document, scale, "M27 untrimmed")?;
    let fillet = document.add_line_line_fillet(
        "M27 reference-radius untrimmed fillet",
        LineLineFilletRequest {
            first: CurveSpan::line(parents[0]),
            first_side: DocumentCurveNormalSide::Left,
            second: CurveSpan::line(parents[1]),
            second_side: DocumentCurveNormalSide::Left,
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            sweep: DocumentArcSweep::CounterClockwise,
            radius: scale,
            radius_mode: DocumentDimensionMode::Reference,
        },
    )?;
    Ok(M27ReferenceFilletIds { parents, fillet })
}

fn generic_fillet_parent(
    curve: CurveSpan,
    parameter: f64,
    side: DocumentCurveNormalSide,
    trim_endpoint: DocumentFilletTrimEndpoint,
    periodic_anchor: Option<DocumentTrimParameter>,
) -> CurveFilletParentRequest {
    CurveFilletParentRequest {
        curve,
        parameter,
        winding: 0,
        neighborhood: ContactNeighborhood::Local {
            lower: parameter - 0.3,
            upper: parameter + 0.3,
        },
        side,
        trim_endpoint,
        periodic_anchor,
    }
}

fn add_line_circle_fillet(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<GenericFilletLabIds, DocumentError> {
    let circle_center = document.add_point("Line-circle parent center fixed", [0.0, 0.0])?;
    let line_points = [
        document.add_point("Line-circle parent line start fixed", [0.0, scale])?,
        document.add_point("Line-circle parent line end fixed", [6.0 * scale, scale])?,
    ];
    let radius = document.add_scalar(
        "Line-circle parent radius",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let circle = document.add_curve(
        "Interactive fillet circle parent",
        CurveDefinition::Circle {
            center: circle_center,
            radius,
        },
    )?;
    let line = add_line(
        document,
        "Interactive fillet line parent",
        line_points[0],
        line_points[1],
    )?;
    for (label, point) in [
        ("Line-circle center fixed", circle_center),
        ("Line-circle line start fixed", line_points[0]),
        ("Line-circle line end fixed", line_points[1]),
    ] {
        fix_point(document, label, point)?;
    }
    add_radius_dimension(document, "Line-circle parent radius 2", circle, 2.0 * scale)?;
    let fillet = document.add_curve_curve_fillet(
        "Interactive line-circle fillet",
        CurveCurveFilletRequest {
            first: generic_fillet_parent(
                CurveSpan::line(circle),
                0.0,
                DocumentCurveNormalSide::Right,
                DocumentFilletTrimEndpoint::End,
                Some(DocumentTrimParameter {
                    parameter: PI,
                    winding: -1,
                }),
            ),
            second: generic_fillet_parent(
                CurveSpan::line(line),
                0.5,
                DocumentCurveNormalSide::Right,
                DocumentFilletTrimEndpoint::Start,
                None,
            ),
            endpoint_order: DocumentFilletEndpointOrder::SecondThenFirst,
            sweep: DocumentArcSweep::CounterClockwise,
            radius: scale,
            radius_mode: DocumentDimensionMode::Reference,
        },
    )?;
    Ok(GenericFilletLabIds {
        parents: [line, circle],
        fillet,
    })
}

fn add_line_bezier_fillet(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<GenericFilletLabIds, DocumentError> {
    let line_points = [
        document.add_point("Line-Bezier line start fixed", [-2.0 * scale, -scale])?,
        document.add_point("Line-Bezier line end fixed", [2.0 * scale, -scale])?,
    ];
    let line = add_line(
        document,
        "Interactive line-Bezier line parent",
        line_points[0],
        line_points[1],
    )?;
    let controls = [
        [2.5 * scale, -1.5 * scale],
        [0.5 * scale, -0.5 * scale],
        [0.5 * scale, 0.5 * scale],
        [2.5 * scale, 1.5 * scale],
    ]
    .map(|position| document.add_point("Line-Bezier cubic control fixed", position))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let controls: [DesignPointId; 4] = controls.try_into().expect("four cubic controls");
    let bezier = document.add_curve(
        "Interactive line-Bezier cubic parent",
        CurveDefinition::CubicBezier { controls },
    )?;
    for (index, point) in line_points.into_iter().chain(controls).enumerate() {
        fix_point(
            document,
            &format!("Line-Bezier support {index} fixed"),
            point,
        )?;
    }
    let fillet = document.add_curve_curve_fillet(
        "Interactive line-Bezier fillet",
        CurveCurveFilletRequest {
            first: generic_fillet_parent(
                CurveSpan::line(line),
                0.5,
                DocumentCurveNormalSide::Left,
                DocumentFilletTrimEndpoint::End,
                None,
            ),
            second: generic_fillet_parent(
                CurveSpan::line(bezier),
                0.5,
                DocumentCurveNormalSide::Left,
                DocumentFilletTrimEndpoint::Start,
                None,
            ),
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            sweep: DocumentArcSweep::CounterClockwise,
            radius: scale,
            radius_mode: DocumentDimensionMode::Reference,
        },
    )?;
    Ok(GenericFilletLabIds {
        parents: [line, bezier],
        fillet,
    })
}

fn add_nurbs_weights(
    document: &mut SketchDocument,
    label: &str,
    values: &[f64],
) -> Result<Vec<DesignScalarId>, DocumentError> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            document.add_scalar(
                format!("{label} weight {}", index + 1),
                *value,
                ScalarUnit::Parameter,
                ScalarDomain::Positive,
            )
        })
        .collect()
}

fn add_quarter_circle_nurbs(
    document: &mut SketchDocument,
    label: &str,
    scale: f64,
    span_id: u32,
) -> Result<(CurveId, Vec<DesignPointId>, Vec<DesignScalarId>), DocumentError> {
    let controls = [[2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]
        .into_iter()
        .enumerate()
        .map(|(index, point)| {
            document.add_point(
                format!("{label} control {}", index + 1),
                [point[0] * scale, point[1] * scale],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let weights = add_nurbs_weights(
        document,
        label,
        &[1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0],
    )?;
    let curve = document.add_curve(
        label,
        CurveDefinition::Nurbs {
            form: DocumentBSplineForm::Clamped,
            degree: 2,
            controls: controls.clone(),
            weights: weights.clone(),
            gauge_weight: weights[0],
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            span_ids: vec![span_id],
            next_span_id: span_id + 1,
        },
    )?;
    Ok((curve, controls, weights))
}

fn add_nurbs_line_fillet(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<GenericFilletLabIds, DocumentError> {
    let (nurbs, controls, _) =
        add_quarter_circle_nurbs(document, "Interactive NURBS fillet parent", scale, 71)?;
    let span = CurveSpan {
        curve: nurbs,
        segment: 71,
    };
    let jet =
        document
            .evaluate_curve_jet(span, 0.5)
            .map_err(|error| DocumentError::InvalidField {
                field: "NURBS fillet fixture",
                message: error.to_string(),
            })?;
    let differential = jet
        .differential()
        .map_err(|error| DocumentError::InvalidField {
            field: "NURBS fillet fixture",
            message: error.to_string(),
        })?;
    let tangent = differential.unit_tangent;
    let line_tangent = differential.left_normal;
    let center = jet.position + differential.left_normal * scale;
    let line_normal = geosolve_geometry::Vector2::new(-line_tangent.y, line_tangent.x);
    let line_contact = center - line_normal * scale;
    let line_points = [
        line_contact - line_tangent * (2.0 * scale),
        line_contact + line_tangent * (2.0 * scale),
    ]
    .map(|point| document.add_point("NURBS-line parent line point fixed", [point.x, point.y]))
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    let line = document.add_curve(
        "Interactive NURBS-line line parent",
        CurveDefinition::Line {
            start: line_points[0],
            end: line_points[1],
            branch_direction: [line_tangent.x, line_tangent.y],
        },
    )?;
    for (index, point) in controls.into_iter().chain(line_points).enumerate() {
        fix_point(
            document,
            &format!("NURBS-line support {index} fixed"),
            point,
        )?;
    }
    let fillet = document.add_curve_curve_fillet(
        "Interactive NURBS-line fillet",
        CurveCurveFilletRequest {
            first: generic_fillet_parent(
                span,
                0.5,
                DocumentCurveNormalSide::Left,
                DocumentFilletTrimEndpoint::End,
                None,
            ),
            second: generic_fillet_parent(
                CurveSpan::line(line),
                0.5,
                DocumentCurveNormalSide::Left,
                DocumentFilletTrimEndpoint::Start,
                None,
            ),
            endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
            sweep: DocumentArcSweep::CounterClockwise,
            radius: scale,
            radius_mode: DocumentDimensionMode::Reference,
        },
    )?;
    let _ = tangent;
    Ok(GenericFilletLabIds {
        parents: [nurbs, line],
        fillet,
    })
}

fn add_nurbs_quarter_circle(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<NurbsLabIds, DocumentError> {
    let (curve, controls, weights) =
        add_quarter_circle_nurbs(document, "NURBS quarter-circle weight lab", scale, 7)?;
    fix_point(document, "Quarter-circle start fixed", controls[0])?;
    fix_point(document, "Quarter-circle end fixed", controls[2])?;
    Ok(NurbsLabIds {
        curve,
        primary_control: controls[1],
        controls,
        weights,
        contact: None,
    })
}

fn add_nurbs_local_support(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<NurbsLabIds, DocumentError> {
    let controls = [
        [-4.0, 0.0],
        [-3.0, 2.0],
        [-1.5, -1.0],
        [0.5, 1.5],
        [2.5, -1.0],
        [4.0, 0.5],
    ]
    .into_iter()
    .enumerate()
    .map(|(index, point)| {
        document.add_point(
            format!("Local-support NURBS control {}", index + 1),
            [point[0] * scale, point[1] * scale],
        )
    })
    .collect::<Result<Vec<_>, _>>()?;
    let weights = add_nurbs_weights(
        document,
        "Local-support NURBS",
        &[0.8, 1.0, 1.3, 0.7, 1.15, 0.9],
    )?;
    let curve = document.add_curve(
        "NURBS local-support and knot-insertion lab",
        CurveDefinition::Nurbs {
            form: DocumentBSplineForm::Clamped,
            degree: 3,
            controls: controls.clone(),
            weights: weights.clone(),
            gauge_weight: weights[1],
            knots: vec![0.0, 0.0, 0.0, 0.0, 0.34, 0.67, 1.0, 1.0, 1.0, 1.0],
            span_ids: vec![41, 73, 89],
            next_span_id: 90,
        },
    )?;
    fix_point(document, "Local-support first endpoint fixed", controls[0])?;
    fix_point(document, "Local-support last endpoint fixed", controls[5])?;
    Ok(NurbsLabIds {
        curve,
        primary_control: controls[2],
        controls,
        weights,
        contact: None,
    })
}

fn add_periodic_nurbs(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<NurbsLabIds, DocumentError> {
    let controls = [[0.0, 0.0], [1.5, -0.2], [2.0, 1.4], [0.5, 2.2], [-0.8, 1.0]]
        .into_iter()
        .enumerate()
        .map(|(index, point)| {
            document.add_point(
                format!("Periodic NURBS control {}", index + 1),
                [point[0] * scale, point[1] * scale],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let weights = add_nurbs_weights(document, "Periodic NURBS", &[0.75, 1.0, 1.4, 0.9, 1.2])?;
    let curve = document.add_curve(
        "Periodic NURBS span and winding lab",
        CurveDefinition::Nurbs {
            form: DocumentBSplineForm::Periodic,
            degree: 2,
            controls: controls.clone(),
            weights: weights.clone(),
            gauge_weight: weights[1],
            knots: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            span_ids: vec![11, 17, 23, 29, 31],
            next_span_id: 32,
        },
    )?;
    fix_point(document, "Periodic NURBS anchor fixed", controls[0])?;
    let contact = document.add_curve_contact(
        "Periodic NURBS explicit seam contact",
        CurveSpan { curve, segment: 31 },
        1.0,
        2,
        ContactNeighborhood::End,
        None,
    )?;
    let seam =
        document
            .evaluate_contact_jet(contact)
            .map_err(|error| DocumentError::InvalidField {
                field: "periodic NURBS seam",
                message: error.to_string(),
            })?;
    let witness = document.add_point(
        "Periodic NURBS seam witness",
        [seam.position.x, seam.position.y],
    )?;
    document.add_constraint(
        "Periodic NURBS witness on explicit span",
        DocumentConstraintDefinition::PointOnCurve {
            point: witness,
            contact,
        },
    )?;
    Ok(NurbsLabIds {
        curve,
        primary_control: controls[3],
        controls,
        weights,
        contact: Some(contact),
    })
}

fn add_nurbs_differential(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<NurbsDifferentialIds, DocumentError> {
    let seam = document.add_point("NURBS C2 draggable seam", [0.0, 0.0])?;
    let first_controls = vec![
        document.add_point("NURBS C2 incoming outer", [-scale, scale])?,
        document.add_point("NURBS C2 incoming handle", [-0.5 * scale, 0.0])?,
        seam,
    ];
    let second_controls = vec![
        seam,
        document.add_point("NURBS C2 outgoing handle", [scale, 0.0])?,
        document.add_point("NURBS C2 outgoing outer", [2.0 * scale, 4.0 * scale])?,
    ];
    let first_weights = add_nurbs_weights(document, "NURBS C2 incoming", &[1.0, 1.0, 1.0])?;
    let second_weights = add_nurbs_weights(document, "NURBS C2 outgoing", &[1.0, 1.0, 1.0])?;
    let curves = [
        document.add_curve(
            "NURBS C2 incoming curve",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: first_controls.clone(),
                weights: first_weights.clone(),
                gauge_weight: first_weights[0],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![10],
                next_span_id: 11,
            },
        )?,
        document.add_curve(
            "NURBS C2 outgoing curve",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: second_controls.clone(),
                weights: second_weights.clone(),
                gauge_weight: second_weights[0],
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                span_ids: vec![20],
                next_span_id: 21,
            },
        )?,
    ];
    let contacts = [
        document.add_curve_contact(
            "NURBS C2 incoming end",
            CurveSpan {
                curve: curves[0],
                segment: 10,
            },
            1.0,
            0,
            ContactNeighborhood::End,
            None,
        )?,
        document.add_curve_contact(
            "NURBS C2 outgoing start",
            CurveSpan {
                curve: curves[1],
                segment: 20,
            },
            0.0,
            0,
            ContactNeighborhood::Start,
            None,
        )?,
    ];
    let continuity = document.add_constraint(
        "NURBS rate-explicit parametric C2 continuity",
        DocumentConstraintDefinition::EndpointContinuity {
            first_contact: contacts[0],
            second_contact: contacts[1],
            continuity: DocumentCurveContinuity::ParametricC2 {
                first_rate: 2.0,
                second_rate: 1.0,
            },
        },
    )?;
    let controls = first_controls
        .into_iter()
        .chain(second_controls.into_iter().skip(1))
        .collect();
    let weights = first_weights.into_iter().chain(second_weights).collect();
    Ok(NurbsDifferentialIds {
        curves,
        seam,
        controls,
        weights,
        contacts,
        continuity,
    })
}

fn conic_ratio(
    document: &mut SketchDocument,
    label: &str,
    value: f64,
) -> Result<DesignScalarId, DocumentError> {
    document.add_scalar(
        label,
        value,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: f64::from_bits(1),
            upper: 1.0,
        },
    )
}

fn conic_parameter(
    document: &mut SketchDocument,
    label: &str,
    value: f64,
    unit: ScalarUnit,
) -> Result<DesignScalarId, DocumentError> {
    document.add_scalar(label, value, unit, ScalarDomain::Finite)
}

#[allow(clippy::too_many_lines)]
fn add_conic_gallery(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<ConicGalleryIds, DocumentError> {
    let ellipse_center =
        document.add_point("Gallery ellipse center", [-8.0 * scale, 4.0 * scale])?;
    let ellipse_axis = document.add_point(
        "Gallery ellipse major-axis handle",
        [-5.5 * scale, 4.0 * scale],
    )?;
    let ellipse_ratio = conic_ratio(document, "Gallery ellipse minor-axis ratio", 0.55)?;
    let ellipse = document.add_curve(
        "Ellipse - full periodic conic",
        CurveDefinition::Ellipse {
            center: ellipse_center,
            major_axis_point: ellipse_axis,
            minor_axis_ratio: ellipse_ratio,
        },
    )?;

    let arc_center = document.add_point("Gallery arc center", [0.0, 4.0 * scale])?;
    let arc_axis =
        document.add_point("Gallery arc major-axis handle", [2.7 * scale, 4.4 * scale])?;
    let arc_ratio = conic_ratio(document, "Gallery arc minor-axis ratio", 0.62)?;
    let arc_start = conic_parameter(document, "Gallery arc start angle", 2.25, ScalarUnit::Angle)?;
    let arc_end = conic_parameter(document, "Gallery arc end angle", -0.35, ScalarUnit::Angle)?;
    let arc = document.add_curve(
        "Elliptical arc - clockwise directed trim",
        CurveDefinition::EllipticalArc {
            center: arc_center,
            major_axis_point: arc_axis,
            minor_axis_ratio: arc_ratio,
            start_angle: arc_start,
            end_angle: arc_end,
            sweep: DocumentArcSweep::Clockwise,
        },
    )?;

    let rational_start =
        document.add_point("Gallery rational start", [6.0 * scale, 2.5 * scale])?;
    let rational_end = document.add_point("Gallery rational end", [10.0 * scale, 2.2 * scale])?;
    let rational_weight = document.add_scalar(
        "Gallery rational middle weight",
        0.65,
        ScalarUnit::Parameter,
        ScalarDomain::Bounded {
            lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
            upper: f64::MAX,
        },
    )?;
    let rational = document.add_curve(
        "Rational quadratic - homogeneous control",
        CurveDefinition::RationalQuadraticConic {
            start: rational_start,
            weighted_middle: [5.2 * scale, 3.25 * scale],
            middle_weight: rational_weight,
            end: rational_end,
        },
    )?;

    let parabola_vertex =
        document.add_point("Gallery parabola vertex", [-5.0 * scale, -4.0 * scale])?;
    let parabola_focus =
        document.add_point("Gallery parabola focus", [-4.0 * scale, -3.6 * scale])?;
    let parabola_start = conic_parameter(
        document,
        "Gallery parabola reversed trim start",
        1.2,
        ScalarUnit::Parameter,
    )?;
    let parabola_end = conic_parameter(
        document,
        "Gallery parabola reversed trim end",
        -1.1,
        ScalarUnit::Parameter,
    )?;
    let parabola = document.add_curve(
        "Parabola - reversed directed trim",
        CurveDefinition::ParabolaSegment {
            vertex: parabola_vertex,
            focus: parabola_focus,
            trim_start: parabola_start,
            trim_end: parabola_end,
        },
    )?;

    let hyperbola_center =
        document.add_point("Gallery hyperbola center", [4.0 * scale, -4.0 * scale])?;
    let hyperbola_axis = document.add_point(
        "Gallery hyperbola transverse-axis handle",
        [6.2 * scale, -3.5 * scale],
    )?;
    let hyperbola_semi_conjugate = document.add_scalar(
        "Gallery hyperbola semi-conjugate length",
        1.4 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let hyperbola_start = conic_parameter(
        document,
        "Gallery hyperbola reversed trim start",
        0.9,
        ScalarUnit::Parameter,
    )?;
    let hyperbola_end = conic_parameter(
        document,
        "Gallery hyperbola reversed trim end",
        -0.7,
        ScalarUnit::Parameter,
    )?;
    let hyperbola = document.add_curve(
        "Hyperbola - negative branch reversed trim",
        CurveDefinition::HyperbolaSegment {
            center: hyperbola_center,
            transverse_axis_point: hyperbola_axis,
            semi_conjugate: hyperbola_semi_conjugate,
            branch: DocumentHyperbolaBranch::Negative,
            trim_start: hyperbola_start,
            trim_end: hyperbola_end,
        },
    )?;

    Ok(ConicGalleryIds {
        curves: [ellipse, arc, rational, parabola, hyperbola],
    })
}

fn add_fixed_curve_point(
    document: &mut SketchDocument,
    label: &str,
    curve: CurveId,
    parameter: f64,
    winding: i32,
    neighborhood: ContactNeighborhood,
) -> Result<(DesignPointId, ContactId), DocumentError> {
    let position = document
        .evaluate_curve_jet(CurveSpan::line(curve), parameter)
        .map_err(|error| DocumentError::InvalidField {
            field: "conic example contact",
            message: error.to_string(),
        })?
        .position;
    let point = document.add_point(format!("{label} witness"), [position.x, position.y])?;
    fix_point(document, &format!("{label} witness fixed"), point)?;
    let contact = document.add_curve_contact(
        format!("{label} contact"),
        CurveSpan::line(curve),
        parameter,
        winding,
        neighborhood,
        None,
    )?;
    document.add_constraint(
        format!("{label} point on conic"),
        DocumentConstraintDefinition::PointOnCurve { point, contact },
    )?;
    Ok((point, contact))
}

fn add_conic_tangency(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<ConicTangencyIds, DocumentError> {
    let first_center = document.add_point("Tangency left ellipse center", [-2.0 * scale, 0.0])?;
    let first_axis = document.add_point("Tangency left ellipse axis", [0.0, 0.0])?;
    let second_center = document.add_point("Tangency right ellipse center", [2.0 * scale, 0.0])?;
    let second_axis = document.add_point("Tangency right ellipse reversed axis", [0.0, 0.0])?;
    let first_ratio = conic_ratio(document, "Tangency left ellipse ratio", 0.6)?;
    let second_ratio = conic_ratio(document, "Tangency right ellipse ratio", 0.6)?;
    let first = document.add_curve(
        "Left ellipse",
        CurveDefinition::Ellipse {
            center: first_center,
            major_axis_point: first_axis,
            minor_axis_ratio: first_ratio,
        },
    )?;
    let second = document.add_curve(
        "Right ellipse",
        CurveDefinition::Ellipse {
            center: second_center,
            major_axis_point: second_axis,
            minor_axis_ratio: second_ratio,
        },
    )?;
    for (label, point) in [
        ("Tangency left center fixed", first_center),
        ("Tangency left axis fixed", first_axis),
        ("Tangency right center fixed", second_center),
        ("Tangency right axis fixed", second_axis),
    ] {
        fix_point(document, label, point)?;
    }

    let first_tangent = document.add_curve_contact(
        "Tangency left ellipse contact",
        CurveSpan::line(first),
        0.0,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let second_tangent = document.add_curve_contact(
        "Tangency right ellipse contact",
        CurveSpan::line(second),
        0.0,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let tangency = document.add_constraint(
        "Generic ellipse-ellipse external tangency",
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: first_tangent,
            second_contact: second_tangent,
        },
    )?;

    let (first_point, first_point_contact) = add_fixed_curve_point(
        document,
        "Tangency left upper",
        first,
        PI / 2.0,
        0,
        ContactNeighborhood::Interior,
    )?;
    let (second_point, second_point_contact) = add_fixed_curve_point(
        document,
        "Tangency right lower",
        second,
        PI / 2.0,
        0,
        ContactNeighborhood::Interior,
    )?;
    Ok(ConicTangencyIds {
        curves: [first, second],
        contact_points: [first_point, second_point],
        point_contacts: [first_point_contact, second_point_contact],
        tangency_contacts: [first_tangent, second_tangent],
        tangency,
    })
}

fn add_axis_length_constraint(
    document: &mut SketchDocument,
    label: &str,
    center: DesignPointId,
    axis: DesignPointId,
    length: f64,
) -> Result<DocumentDimensionId, DocumentError> {
    let target = document.add_scalar(
        format!("{label} target"),
        length,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        label,
        DocumentDimensionDefinition::PointDistance {
            first: center,
            second: axis,
            target,
        },
        DocumentDimensionMode::Driving,
    )
}

#[allow(clippy::too_many_lines)]
fn add_conic_circle_limit(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<ConicCircleLimitIds, DocumentError> {
    let ellipse_center =
        document.add_point("Circle-limit full ellipse center", [-4.0 * scale, 0.0])?;
    let ellipse_axis =
        document.add_point("Circle-limit full ellipse axis handle", [-2.0 * scale, 0.0])?;
    let ellipse_ratio = conic_ratio(document, "Circle-limit full ellipse ratio = 1", 1.0)?;
    let ellipse = document.add_curve(
        "Circle-limit full ellipse - orientation unobservable",
        CurveDefinition::Ellipse {
            center: ellipse_center,
            major_axis_point: ellipse_axis,
            minor_axis_ratio: ellipse_ratio,
        },
    )?;
    fix_point(
        document,
        "Circle-limit full ellipse center fixed",
        ellipse_center,
    )?;
    add_axis_length_constraint(
        document,
        "Circle-limit full ellipse radius",
        ellipse_center,
        ellipse_axis,
        2.0 * scale,
    )?;
    let (_, full_first) = add_fixed_curve_point(
        document,
        "Circle-limit full ellipse first",
        ellipse,
        0.4,
        1,
        ContactNeighborhood::Interior,
    )?;
    let (_, full_second) = add_fixed_curve_point(
        document,
        "Circle-limit full ellipse second",
        ellipse,
        1.6,
        0,
        ContactNeighborhood::Interior,
    )?;

    let arc_center = document.add_point("Circle-limit directed arc center", [4.0 * scale, 0.0])?;
    let arc_axis =
        document.add_point("Circle-limit directed arc axis handle", [6.0 * scale, 0.0])?;
    let arc_ratio = conic_ratio(document, "Circle-limit directed arc ratio = 1", 1.0)?;
    let arc_start = conic_parameter(
        document,
        "Circle-limit directed arc start",
        -0.4,
        ScalarUnit::Angle,
    )?;
    let arc_end = conic_parameter(
        document,
        "Circle-limit directed arc end",
        1.4,
        ScalarUnit::Angle,
    )?;
    let arc = document.add_curve(
        "Circle-limit elliptical arc - directed orientation observable",
        CurveDefinition::EllipticalArc {
            center: arc_center,
            major_axis_point: arc_axis,
            minor_axis_ratio: arc_ratio,
            start_angle: arc_start,
            end_angle: arc_end,
            sweep: DocumentArcSweep::CounterClockwise,
        },
    )?;
    fix_point(
        document,
        "Circle-limit directed arc center fixed",
        arc_center,
    )?;
    add_axis_length_constraint(
        document,
        "Circle-limit directed arc radius",
        arc_center,
        arc_axis,
        2.0 * scale,
    )?;
    let (_, arc_start_contact) = add_fixed_curve_point(
        document,
        "Circle-limit directed arc start",
        arc,
        0.0,
        0,
        ContactNeighborhood::Start,
    )?;
    let (_, arc_end_contact) = add_fixed_curve_point(
        document,
        "Circle-limit directed arc end",
        arc,
        1.0,
        0,
        ContactNeighborhood::End,
    )?;

    Ok(ConicCircleLimitIds {
        curves: [ellipse, arc],
        full_ellipse_contacts: [full_first, full_second],
        arc_endpoint_contacts: [arc_start_contact, arc_end_contact],
    })
}

fn add_diagnostic_rank_drop(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<DiagnosticRankDropIds, DocumentError> {
    let first_center = document.add_point("Rank lens center A", [-2.0 * scale, 0.0])?;
    let second_center = document.add_point("Rank lens center B", [2.0 * scale, 0.0])?;
    let point = document.add_point("Rank-dependent point P", [0.0, 0.0])?;
    let first_radius = document.add_scalar(
        "Rank lens radius A",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let second_radius = document.add_scalar(
        "Rank lens radius B",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let first_circle = document.add_curve(
        "Rank lens circle A",
        CurveDefinition::Circle {
            center: first_center,
            radius: first_radius,
        },
    )?;
    let second_circle = document.add_curve(
        "Rank lens circle B",
        CurveDefinition::Circle {
            center: second_center,
            radius: second_radius,
        },
    )?;
    fix_point(document, "Rank lens center A fixed", first_center)?;
    fix_point(document, "Rank lens center B fixed", second_center)?;
    add_radius_dimension(
        document,
        "Rank lens circle A radius 2",
        first_circle,
        2.0 * scale,
    )?;
    add_radius_dimension(
        document,
        "Rank lens circle B radius 2",
        second_circle,
        2.0 * scale,
    )?;
    let first_target = document.add_scalar(
        "Rank distance A-P target",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let first_distance = document.add_dimension(
        "Rank distance A-P = 2",
        DocumentDimensionDefinition::PointDistance {
            first: first_center,
            second: point,
            target: first_target,
        },
        DocumentDimensionMode::Driving,
    )?;
    let second_target = document.add_scalar(
        "Rank distance B-P target",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let second_distance = document.add_dimension(
        "Rank distance B-P = 2",
        DocumentDimensionDefinition::PointDistance {
            first: second_center,
            second: point,
            target: second_target,
        },
        DocumentDimensionMode::Driving,
    )?;
    Ok(DiagnosticRankDropIds {
        point,
        first_distance,
        second_distance,
    })
}

fn add_diagnostic_endpoint_bound(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<DiagnosticEndpointBoundIds, DocumentError> {
    let start = document.add_point("Endpoint rail start", [-4.0 * scale, 0.0])?;
    let end = document.add_point("Endpoint rail end", [4.0 * scale, 0.0])?;
    let point = document.add_point("Endpoint-fixed follower", [4.0 * scale, 0.0])?;
    let line = add_line(document, "Endpoint-bounded rail", start, end)?;
    fix_point(document, "Endpoint rail start fixed", start)?;
    fix_point(document, "Endpoint rail end fixed", end)?;
    let contact = document.add_curve_contact(
        "Endpoint-fixed contact t=1",
        CurveSpan::line(line),
        1.0,
        0,
        ContactNeighborhood::End,
        None,
    )?;
    let point_on_curve = document.add_constraint(
        "Endpoint follower on bounded rail",
        DocumentConstraintDefinition::PointOnCurve { point, contact },
    )?;
    let center = document.add_point("Positive-radius center", [0.0, 2.0 * scale])?;
    let radius = document.add_scalar(
        "Positive radius at lower domain",
        MIN_REPRESENTABLE_RADIUS,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let circle = document.add_curve(
        "Positive-radius bound",
        CurveDefinition::Circle { center, radius },
    )?;
    fix_point(document, "Positive-radius center fixed", center)?;
    Ok(DiagnosticEndpointBoundIds {
        line,
        point,
        contact,
        point_on_curve,
        circle,
        radius,
    })
}

fn add_diagnostic_redundancy(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<DiagnosticRedundancyIds, DocumentError> {
    let origin = document.add_point("Redundancy arm origin", [0.0, 0.0])?;
    let tip = document.add_point("Redundancy arm tip", [4.0 * scale, 0.0])?;
    let line = add_line(document, "Redundancy test arm", origin, tip)?;
    fix_point(document, "Redundancy arm origin fixed", origin)?;
    document.add_constraint(
        "Redundancy arm horizontal",
        DocumentConstraintDefinition::Horizontal {
            line: CurveSpan::line(line),
        },
    )?;
    let primary_length = add_length_dimension(
        document,
        "Primary arm length 4",
        CurveSpan::line(line),
        4.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    let duplicate_length = add_length_dimension(
        document,
        "Duplicate arm length 4",
        CurveSpan::line(line),
        4.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    Ok(DiagnosticRedundancyIds {
        line,
        primary_length,
        duplicate_length,
    })
}

fn add_stress_compass(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<StressCompassIds, DocumentError> {
    let sqrt_three = 3.0_f64.sqrt();
    let origin = document.add_point("Compass pivot O", [0.0, 0.0])?;
    let axis_tip = document.add_point(
        "Compass fixed bisector K",
        [2.5 * sqrt_three * scale, 2.5 * scale],
    )?;
    let first_tip = document.add_point("Compass tip A", [4.0 * scale, 0.0])?;
    let second_tip =
        document.add_point("Compass tip B", [2.0 * scale, 2.0 * sqrt_three * scale])?;
    let axis = add_line(document, "Compass symmetry axis", origin, axis_tip)?;
    let first_arm = add_line(document, "Compass arm OA", origin, first_tip)?;
    let second_arm = add_line(document, "Compass arm OB", origin, second_tip)?;
    let chord = add_line(document, "Compass chord AB", first_tip, second_tip)?;
    fix_point(document, "Compass pivot fixed", origin)?;
    fix_point(document, "Compass bisector fixed", axis_tip)?;
    document.add_constraint(
        "Compass symmetric tips",
        DocumentConstraintDefinition::SymmetricAboutLine {
            first: first_tip,
            second: second_tip,
            line: CurveSpan::line(axis),
        },
    )?;
    document.add_constraint(
        "Compass equal arms",
        DocumentConstraintDefinition::EqualLength {
            first: CurveSpan::line(first_arm),
            second: CurveSpan::line(second_arm),
        },
    )?;
    add_length_dimension(
        document,
        "Compass arm length 4",
        CurveSpan::line(first_arm),
        4.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    add_length_dimension(
        document,
        "Compass second arm reference",
        CurveSpan::line(second_arm),
        4.0 * scale,
        DocumentDimensionMode::Reference,
    )?;
    add_length_dimension(
        document,
        "Compass chord reference",
        CurveSpan::line(chord),
        4.0 * scale,
        DocumentDimensionMode::Reference,
    )?;
    let angle_target = document.add_scalar(
        "Compass opening angle target",
        PI / 3.0,
        ScalarUnit::Angle,
        ScalarDomain::Positive,
    )?;
    let angle = document.add_dimension(
        "Compass opening angle 60 deg",
        DocumentDimensionDefinition::OrientedAngle {
            first: CurveSpan::line(first_arm),
            second: CurveSpan::line(second_arm),
            target: angle_target,
            orientation: crate::DocumentAngleOrientation::CounterClockwise,
        },
        DocumentDimensionMode::Reference,
    )?;
    Ok(StressCompassIds {
        origin,
        first_tip,
        second_tip,
        first_arm,
        second_arm,
        angle,
    })
}

fn add_stress_bridge(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<StressBridgeIds, DocumentError> {
    let left = [
        document.add_point("Bridge left P0", [-4.0 * scale, 0.0])?,
        document.add_point("Bridge left P1", [-3.0 * scale, 2.0 * scale])?,
        document.add_point("Bridge left P2", [-scale, 2.0 * scale])?,
        document.add_point("Bridge left seam", [0.0, 0.0])?,
    ];
    let right = [
        document.add_point("Bridge right seam", [0.0, 0.0])?,
        document.add_point("Bridge right P1", [1.0 * scale, -2.0 * scale])?,
        document.add_point("Bridge right P2", [3.0 * scale, -2.0 * scale])?,
        document.add_point("Bridge right P3", [4.0 * scale, 0.0])?,
    ];
    let left_curve = document.add_curve(
        "Bridge left cubic Bezier",
        CurveDefinition::CubicBezier { controls: left },
    )?;
    let right_curve = document.add_curve(
        "Bridge right cubic Bezier",
        CurveDefinition::CubicBezier { controls: right },
    )?;
    let handle_direction = [1.0 / 5.0_f64.sqrt(), -2.0 / 5.0_f64.sqrt()];
    let left_handle = document.add_curve(
        "Bridge left seam handle",
        CurveDefinition::Line {
            start: left[2],
            end: left[3],
            branch_direction: handle_direction,
        },
    )?;
    let right_handle = document.add_curve(
        "Bridge right seam handle",
        CurveDefinition::Line {
            start: right[0],
            end: right[1],
            branch_direction: handle_direction,
        },
    )?;
    for (index, point) in [left[0], left[1], left[2], right[1], right[2], right[3]]
        .into_iter()
        .enumerate()
    {
        fix_point(
            document,
            &format!("Bridge outer control {} fixed", index + 1),
            point,
        )?;
    }
    let left_contact = document.add_curve_contact(
        "Bridge left endpoint contact",
        CurveSpan::line(left_curve),
        1.0,
        0,
        ContactNeighborhood::End,
        Some(TangentOrientation::Aligned),
    )?;
    let right_contact = document.add_curve_contact(
        "Bridge right endpoint contact",
        CurveSpan::line(right_curve),
        0.0,
        0,
        ContactNeighborhood::Start,
        Some(TangentOrientation::Aligned),
    )?;
    let tangency = document.add_constraint(
        "Bridge C1 endpoint tangency",
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: left_contact,
            second_contact: right_contact,
        },
    )?;
    let equal_handles = document.add_constraint(
        "Bridge equal seam handles",
        DocumentConstraintDefinition::EqualLength {
            first: CurveSpan::line(left_handle),
            second: CurveSpan::line(right_handle),
        },
    )?;
    suppress_constraint(document, equal_handles)?;
    add_length_dimension(
        document,
        "Bridge left handle reference",
        CurveSpan::line(left_handle),
        5.0_f64.sqrt() * scale,
        DocumentDimensionMode::Reference,
    )?;
    add_length_dimension(
        document,
        "Bridge right handle reference",
        CurveSpan::line(right_handle),
        5.0_f64.sqrt() * scale,
        DocumentDimensionMode::Reference,
    )?;
    Ok(StressBridgeIds {
        left_seam: left[3],
        right_seam: right[0],
        left_curve,
        right_curve,
        tangency,
        equal_handles,
    })
}

fn add_motion_cam(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionCamIds, DocumentError> {
    let controls = [
        document.add_point("Cam Q0", [-4.0 * scale, 0.0])?,
        document.add_point("Cam Q1", [0.0, 4.0 * scale])?,
        document.add_point("Cam Q2", [4.0 * scale, 0.0])?,
    ];
    let cam = document.add_curve(
        "Quadratic Bezier cam",
        CurveDefinition::QuadraticBezier { controls },
    )?;
    for (index, point) in controls.into_iter().enumerate() {
        fix_point(document, &format!("Cam Q{index} fixed"), point)?;
    }
    let inverse_sqrt_five = 1.0 / 5.0_f64.sqrt();
    let left_center = document.add_point(
        "Left roller center",
        [
            (-2.0 - inverse_sqrt_five) * scale,
            (1.5 + 2.0 * inverse_sqrt_five) * scale,
        ],
    )?;
    let right_center = document.add_point(
        "Right roller center",
        [
            (2.0 + inverse_sqrt_five) * scale,
            (1.5 + 2.0 * inverse_sqrt_five) * scale,
        ],
    )?;
    let left_radius = document.add_scalar(
        "Left roller radius",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let right_radius = document.add_scalar(
        "Right roller radius",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let left_circle = document.add_curve(
        "Left cam roller",
        CurveDefinition::Circle {
            center: left_center,
            radius: left_radius,
        },
    )?;
    let right_circle = document.add_curve(
        "Right cam roller",
        CurveDefinition::Circle {
            center: right_center,
            radius: right_radius,
        },
    )?;
    add_radius_dimension(document, "Cam roller radius 1", left_circle, scale)?;
    document.add_constraint(
        "Cam rollers equal radius",
        DocumentConstraintDefinition::EqualRadius {
            first: left_circle,
            second: right_circle,
        },
    )?;
    add_motion_cam_tangency(
        document,
        "Left roller",
        cam,
        0.25,
        left_circle,
        2.0 * PI - 2.0_f64.atan(),
    )?;
    add_motion_cam_tangency(
        document,
        "Right roller",
        cam,
        0.75,
        right_circle,
        PI + 2.0_f64.atan(),
    )?;
    let diameter_target = document.add_scalar(
        "Right roller diameter target",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "Right roller diameter reference",
        DocumentDimensionDefinition::Diameter {
            curve: right_circle,
            target: diameter_target,
        },
        DocumentDimensionMode::Reference,
    )?;
    Ok(MotionCamIds {
        controls,
        left_center,
        right_center,
        cam,
        left_circle,
        right_circle,
    })
}

fn add_motion_orbit(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionOrbitIds, DocumentError> {
    let fixed_center = document.add_point("Orbit fixed center", [0.0, 0.0])?;
    let moving_center = document.add_point("Orbit satellite center", [4.0 * scale, 0.0])?;
    let fixed_radius = document.add_scalar(
        "Orbit fixed radius",
        3.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let moving_radius = document.add_scalar(
        "Orbit satellite radius",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let fixed_circle = document.add_curve(
        "Orbit fixed circle",
        CurveDefinition::Circle {
            center: fixed_center,
            radius: fixed_radius,
        },
    )?;
    let moving_circle = document.add_curve(
        "Orbit tangent satellite",
        CurveDefinition::Circle {
            center: moving_center,
            radius: moving_radius,
        },
    )?;
    fix_point(document, "Orbit center fixed", fixed_center)?;
    add_radius_dimension(document, "Orbit fixed radius 3", fixed_circle, 3.0 * scale)?;
    add_radius_dimension(document, "Orbit satellite radius 1", moving_circle, scale)?;
    let fixed_contact = document.add_curve_contact(
        "Orbit fixed-circle contact",
        CurveSpan::line(fixed_circle),
        0.0,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let moving_contact = document.add_curve_contact(
        "Orbit satellite contact",
        CurveSpan::line(moving_circle),
        PI,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Opposed),
    )?;
    let tangency = document.add_constraint(
        "Orbit external tangency",
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: fixed_contact,
            second_contact: moving_contact,
        },
    )?;
    let distance_target = document.add_scalar(
        "Orbit center distance target",
        4.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "Orbit center distance reference",
        DocumentDimensionDefinition::PointDistance {
            first: fixed_center,
            second: moving_center,
            target: distance_target,
        },
        DocumentDimensionMode::Reference,
    )?;
    Ok(MotionOrbitIds {
        fixed_center,
        moving_center,
        fixed_circle,
        moving_circle,
        tangency,
    })
}

#[allow(clippy::too_many_lines)]
fn add_motion_trammel(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionTrammelIds, DocumentError> {
    let horizontal_start =
        document.add_point("Trammel horizontal rail start", [-6.0 * scale, 0.0])?;
    let horizontal_end = document.add_point("Trammel horizontal rail end", [6.0 * scale, 0.0])?;
    let vertical_start = document.add_point("Trammel vertical rail start", [0.0, -6.0 * scale])?;
    let vertical_end = document.add_point("Trammel vertical rail end", [0.0, 6.0 * scale])?;
    let horizontal_rail = add_line(
        document,
        "Trammel horizontal rail",
        horizontal_start,
        horizontal_end,
    )?;
    let vertical_rail = add_line(
        document,
        "Trammel vertical rail",
        vertical_start,
        vertical_end,
    )?;
    for (label, point) in [
        ("Trammel horizontal rail start fixed", horizontal_start),
        ("Trammel horizontal rail end fixed", horizontal_end),
        ("Trammel vertical rail start fixed", vertical_start),
        ("Trammel vertical rail end fixed", vertical_end),
    ] {
        fix_point(document, label, point)?;
    }

    let horizontal_slider =
        document.add_point("Trammel horizontal slider A", [4.0 * scale, 0.0])?;
    let vertical_slider = document.add_point("Trammel vertical slider B", [0.0, 3.0 * scale])?;
    let midpoint = document.add_point("Trammel bar midpoint M", [2.0 * scale, 1.5 * scale])?;
    let tracer = document.add_point("Trammel elliptic tracer T", [3.0 * scale, 0.75 * scale])?;
    let bar = document.add_curve(
        "Trammel fixed-length bar AB",
        CurveDefinition::Line {
            start: horizontal_slider,
            end: vertical_slider,
            branch_direction: [-0.8, 0.6],
        },
    )?;
    let quarter_arm = document.add_curve(
        "Trammel quarter arm AM",
        CurveDefinition::Line {
            start: horizontal_slider,
            end: midpoint,
            branch_direction: [-0.8, 0.6],
        },
    )?;
    let horizontal_contact = document.add_curve_contact(
        "Trammel horizontal slider contact",
        CurveSpan::line(horizontal_rail),
        5.0 / 6.0,
        0,
        ContactNeighborhood::Interior,
        None,
    )?;
    let vertical_contact = document.add_curve_contact(
        "Trammel vertical slider contact",
        CurveSpan::line(vertical_rail),
        0.75,
        0,
        ContactNeighborhood::Interior,
        None,
    )?;
    document.add_constraint(
        "Trammel A slides horizontally",
        DocumentConstraintDefinition::PointOnCurve {
            point: horizontal_slider,
            contact: horizontal_contact,
        },
    )?;
    document.add_constraint(
        "Trammel B slides vertically",
        DocumentConstraintDefinition::PointOnCurve {
            point: vertical_slider,
            contact: vertical_contact,
        },
    )?;
    add_length_dimension(
        document,
        "Trammel bar length 5",
        CurveSpan::line(bar),
        5.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    document.add_constraint(
        "Trammel M bisects AB",
        DocumentConstraintDefinition::Midpoint {
            point: midpoint,
            line: CurveSpan::line(bar),
        },
    )?;
    document.add_constraint(
        "Trammel T bisects AM",
        DocumentConstraintDefinition::Midpoint {
            point: tracer,
            line: CurveSpan::line(quarter_arm),
        },
    )?;
    Ok(MotionTrammelIds {
        horizontal_slider,
        vertical_slider,
        tracer,
        bar,
        horizontal_contact,
        vertical_contact,
    })
}

fn add_motion_scotch_yoke(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionScotchYokeIds, DocumentError> {
    let crank_center = document.add_point("Yoke crank center O", [0.0, 0.0])?;
    let crank_pin = document.add_point("Yoke crank pin P", [3.0 * scale, 4.0 * scale])?;
    let slider = document.add_point("Yoke horizontal slider S", [3.0 * scale, -6.0 * scale])?;
    let crank = document.add_curve(
        "Yoke crank OP",
        CurveDefinition::Line {
            start: crank_center,
            end: crank_pin,
            branch_direction: [0.6, 0.8],
        },
    )?;
    let slot = add_line(document, "Yoke vertical slot SP", slider, crank_pin)?;
    fix_point(document, "Yoke crank center fixed", crank_center)?;
    document.add_constraint(
        "Yoke slider on horizontal guide",
        DocumentConstraintDefinition::FixedCoordinate {
            point: slider,
            axis: DocumentCoordinateAxis::Y,
            target: -6.0 * scale,
        },
    )?;
    document.add_constraint(
        "Yoke slot remains vertical",
        DocumentConstraintDefinition::Vertical {
            line: CurveSpan::line(slot),
        },
    )?;
    add_length_dimension(
        document,
        "Yoke crank radius 5",
        CurveSpan::line(crank),
        5.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    Ok(MotionScotchYokeIds {
        crank_center,
        crank_pin,
        slider,
        crank,
        slot,
    })
}

fn add_motion_rotating_square(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionRotatingSquareIds, DocumentError> {
    let corners = [
        document.add_point("Rotating square anchor A", [0.0, 0.0])?,
        document.add_point("Rotating square corner B", [3.0 * scale, 0.0])?,
        document.add_point("Rotating square corner C", [3.0 * scale, 3.0 * scale])?,
        document.add_point("Rotating square corner D", [0.0, 3.0 * scale])?,
    ];
    let edges = [
        add_line(document, "Rotating square edge AB", corners[0], corners[1])?,
        add_line(document, "Rotating square edge BC", corners[1], corners[2])?,
        add_line(document, "Rotating square edge CD", corners[2], corners[3])?,
        add_line(document, "Rotating square edge DA", corners[3], corners[0])?,
    ];
    fix_point(document, "Rotating square anchor fixed", corners[0])?;
    add_length_dimension(
        document,
        "Rotating square side length 3",
        CurveSpan::line(edges[0]),
        3.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    document.add_constraint(
        "Rotating square adjacent edges perpendicular",
        DocumentConstraintDefinition::Perpendicular {
            first: CurveSpan::line(edges[0]),
            second: CurveSpan::line(edges[1]),
        },
    )?;
    document.add_constraint(
        "Rotating square adjacent edges equal",
        DocumentConstraintDefinition::EqualLength {
            first: CurveSpan::line(edges[0]),
            second: CurveSpan::line(edges[1]),
        },
    )?;
    document.add_constraint(
        "Rotating square opposite edges AB CD parallel",
        DocumentConstraintDefinition::Parallel {
            first: CurveSpan::line(edges[0]),
            second: CurveSpan::line(edges[2]),
        },
    )?;
    document.add_constraint(
        "Rotating square opposite edges BC DA parallel",
        DocumentConstraintDefinition::Parallel {
            first: CurveSpan::line(edges[1]),
            second: CurveSpan::line(edges[3]),
        },
    )?;
    Ok(MotionRotatingSquareIds { corners, edges })
}

fn add_motion_scissor(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionScissorIds, DocumentError> {
    let anchor = document.add_point("Scissor fixed anchor A", [-4.0 * scale, 0.0])?;
    let slider = document.add_point("Scissor base slider B", [4.0 * scale, 0.0])?;
    let upper_joint = document.add_point("Scissor upper joint U", [0.0, 3.0 * scale])?;
    let lower_joint = document.add_point("Scissor lower joint L", [0.0, -3.0 * scale])?;
    let axis = add_line(document, "Scissor sliding base AB", anchor, slider)?;
    let upper_left = document.add_curve(
        "Scissor upper-left arm AU",
        CurveDefinition::Line {
            start: anchor,
            end: upper_joint,
            branch_direction: [0.8, 0.6],
        },
    )?;
    let upper_right = document.add_curve(
        "Scissor upper-right arm UB",
        CurveDefinition::Line {
            start: upper_joint,
            end: slider,
            branch_direction: [0.8, -0.6],
        },
    )?;
    document.add_curve(
        "Scissor lower-left arm AL",
        CurveDefinition::Line {
            start: anchor,
            end: lower_joint,
            branch_direction: [0.8, -0.6],
        },
    )?;
    document.add_curve(
        "Scissor lower-right arm LB",
        CurveDefinition::Line {
            start: lower_joint,
            end: slider,
            branch_direction: [0.8, 0.6],
        },
    )?;
    fix_point(document, "Scissor anchor fixed", anchor)?;
    document.add_constraint(
        "Scissor B slides horizontally",
        DocumentConstraintDefinition::FixedCoordinate {
            point: slider,
            axis: DocumentCoordinateAxis::Y,
            target: 0.0,
        },
    )?;
    add_length_dimension(
        document,
        "Scissor arm length 5",
        CurveSpan::line(upper_left),
        5.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    document.add_constraint(
        "Scissor upper arms equal",
        DocumentConstraintDefinition::EqualLength {
            first: CurveSpan::line(upper_left),
            second: CurveSpan::line(upper_right),
        },
    )?;
    document.add_constraint(
        "Scissor joints mirror across base",
        DocumentConstraintDefinition::SymmetricAboutLine {
            first: upper_joint,
            second: lower_joint,
            line: CurveSpan::line(axis),
        },
    )?;
    Ok(MotionScissorIds {
        anchor,
        slider,
        upper_joint,
        lower_joint,
        axis,
    })
}

#[allow(clippy::too_many_lines)]
fn add_motion_scissor_tower(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionScissorTowerIds, DocumentError> {
    let mut left_levels = Vec::with_capacity(6);
    let mut right_levels = Vec::with_capacity(6);
    for level in 0..=5 {
        let height = 6.0 * f64::from(level) * scale;
        left_levels
            .push(document.add_point(format!("Tower level {level} left"), [-4.0 * scale, height])?);
        right_levels
            .push(document.add_point(format!("Tower level {level} right"), [4.0 * scale, height])?);
    }
    let left_levels: [DesignPointId; 6] = left_levels.try_into().expect("six left tower levels");
    let right_levels: [DesignPointId; 6] = right_levels.try_into().expect("six right tower levels");

    let mut platforms = Vec::with_capacity(6);
    for level in 0..=5 {
        platforms.push(add_line(
            document,
            &format!("Tower platform level {level}"),
            left_levels[level],
            right_levels[level],
        )?);
    }
    let platforms: [CurveId; 6] = platforms.try_into().expect("six tower platforms");

    let mut diagonal_bars = Vec::with_capacity(10);
    for stage in 0..5 {
        diagonal_bars.push(document.add_curve(
            format!("Tower stage {} rising-right bar", stage + 1),
            CurveDefinition::Line {
                start: left_levels[stage],
                end: right_levels[stage + 1],
                branch_direction: [0.8, 0.6],
            },
        )?);
        diagonal_bars.push(document.add_curve(
            format!("Tower stage {} rising-left bar", stage + 1),
            CurveDefinition::Line {
                start: right_levels[stage],
                end: left_levels[stage + 1],
                branch_direction: [-0.8, 0.6],
            },
        )?);
    }
    let diagonal_bars: [CurveId; 10] = diagonal_bars.try_into().expect("ten tower diagonal bars");

    fix_point(document, "Tower base left fixed", left_levels[0])?;
    document.add_constraint(
        "Tower base right slides horizontally",
        DocumentConstraintDefinition::FixedCoordinate {
            point: right_levels[0],
            axis: DocumentCoordinateAxis::Y,
            target: 0.0,
        },
    )?;
    for (level, platform) in platforms.iter().copied().enumerate().skip(1) {
        document.add_constraint(
            format!("Tower platform {level} remains horizontal"),
            DocumentConstraintDefinition::Horizontal {
                line: CurveSpan::line(platform),
            },
        )?;
        document.add_constraint(
            format!("Tower platform {level} matches base width"),
            DocumentConstraintDefinition::EqualLength {
                first: CurveSpan::line(platforms[0]),
                second: CurveSpan::line(platform),
            },
        )?;
    }
    add_length_dimension(
        document,
        "Tower master diagonal length 10",
        CurveSpan::line(diagonal_bars[0]),
        10.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    for (index, bar) in diagonal_bars.iter().copied().enumerate().skip(1) {
        document.add_constraint(
            format!("Tower diagonal {} matches master", index + 1),
            DocumentConstraintDefinition::EqualLength {
                first: CurveSpan::line(diagonal_bars[0]),
                second: CurveSpan::line(bar),
            },
        )?;
    }
    Ok(MotionScissorTowerIds {
        left_levels,
        right_levels,
        platforms,
        diagonal_bars,
    })
}

#[allow(clippy::too_many_lines)]
fn add_motion_peaucellier(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionPeaucellierIds, DocumentError> {
    let shoulder_offset = 3.5_f64.sqrt();
    let origin = document.add_point("Peaucellier fixed origin O", [0.0, 0.0])?;
    let input_center = document.add_point("Peaucellier input center S", [4.0 * scale, 0.0])?;
    let input = document.add_point("Peaucellier circular input P", [4.0 * scale, 4.0 * scale])?;
    let output = document.add_point(
        "Peaucellier straight-line output Q",
        [2.0 * scale, 2.0 * scale],
    )?;
    let shoulders = [
        document.add_point(
            "Peaucellier shoulder B",
            [
                (3.0 - shoulder_offset) * scale,
                (3.0 + shoulder_offset) * scale,
            ],
        )?,
        document.add_point(
            "Peaucellier shoulder D",
            [
                (3.0 + shoulder_offset) * scale,
                (3.0 - shoulder_offset) * scale,
            ],
        )?,
    ];
    let bars = [
        document.add_curve(
            "Peaucellier long bar OB",
            CurveDefinition::Line {
                start: origin,
                end: shoulders[0],
                branch_direction: [(3.0 - shoulder_offset) / 5.0, (3.0 + shoulder_offset) / 5.0],
            },
        )?,
        document.add_curve(
            "Peaucellier long bar OD",
            CurveDefinition::Line {
                start: origin,
                end: shoulders[1],
                branch_direction: [(3.0 + shoulder_offset) / 5.0, (3.0 - shoulder_offset) / 5.0],
            },
        )?,
        document.add_curve(
            "Peaucellier rhombus bar BP",
            CurveDefinition::Line {
                start: shoulders[0],
                end: input,
                branch_direction: [(1.0 + shoulder_offset) / 3.0, (1.0 - shoulder_offset) / 3.0],
            },
        )?,
        document.add_curve(
            "Peaucellier rhombus bar PD",
            CurveDefinition::Line {
                start: input,
                end: shoulders[1],
                branch_direction: [
                    (shoulder_offset - 1.0) / 3.0,
                    (-1.0 - shoulder_offset) / 3.0,
                ],
            },
        )?,
        document.add_curve(
            "Peaucellier rhombus bar DQ",
            CurveDefinition::Line {
                start: shoulders[1],
                end: output,
                branch_direction: [
                    (-1.0 - shoulder_offset) / 3.0,
                    (shoulder_offset - 1.0) / 3.0,
                ],
            },
        )?,
        document.add_curve(
            "Peaucellier rhombus bar QB",
            CurveDefinition::Line {
                start: output,
                end: shoulders[0],
                branch_direction: [(1.0 - shoulder_offset) / 3.0, (1.0 + shoulder_offset) / 3.0],
            },
        )?,
        document.add_curve(
            "Peaucellier circular driver SP",
            CurveDefinition::Line {
                start: input_center,
                end: input,
                branch_direction: [0.0, 1.0],
            },
        )?,
    ];
    fix_point(document, "Peaucellier origin fixed", origin)?;
    fix_point(document, "Peaucellier input center fixed", input_center)?;
    add_length_dimension(
        document,
        "Peaucellier long radius 5",
        CurveSpan::line(bars[0]),
        5.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    document.add_constraint(
        "Peaucellier long bars equal",
        DocumentConstraintDefinition::EqualLength {
            first: CurveSpan::line(bars[0]),
            second: CurveSpan::line(bars[1]),
        },
    )?;
    add_length_dimension(
        document,
        "Peaucellier rhombus side 3",
        CurveSpan::line(bars[2]),
        3.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    for (index, bar) in bars[3..=5].iter().copied().enumerate() {
        document.add_constraint(
            format!("Peaucellier rhombus side {} equal", index + 2),
            DocumentConstraintDefinition::EqualLength {
                first: CurveSpan::line(bars[2]),
                second: CurveSpan::line(bar),
            },
        )?;
    }
    add_length_dimension(
        document,
        "Peaucellier input circle radius 4",
        CurveSpan::line(bars[6]),
        4.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    Ok(MotionPeaucellierIds {
        origin,
        input_center,
        input,
        output,
        shoulders,
        bars,
    })
}

fn add_motion_four_bar_coupler(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionFourBarCouplerIds, DocumentError> {
    let grounds = [
        document.add_point("Four-bar ground O2", [0.0, 0.0])?,
        document.add_point("Four-bar ground O4", [8.0 * scale, 0.0])?,
    ];
    let joints = [
        document.add_point("Four-bar input joint A", [3.0 * scale, 4.0 * scale])?,
        document.add_point("Four-bar output joint B", [7.0 * scale, 4.0 * scale])?,
    ];
    let tracer = document.add_point("Four-bar coupler tracer C", [5.0 * scale, 4.0 * scale])?;
    let inverse_sqrt_17 = 1.0 / 17.0_f64.sqrt();
    let bars = [
        add_line_with_direction(
            document,
            "Four-bar input crank O2-A",
            grounds[0],
            joints[0],
            [0.6, 0.8],
        )?,
        add_line_with_direction(
            document,
            "Four-bar coupler A-B",
            joints[0],
            joints[1],
            [1.0, 0.0],
        )?,
        add_line_with_direction(
            document,
            "Four-bar output rocker B-O4",
            joints[1],
            grounds[1],
            [inverse_sqrt_17, -4.0 * inverse_sqrt_17],
        )?,
    ];
    fix_point(document, "Four-bar ground O2 fixed", grounds[0])?;
    fix_point(document, "Four-bar ground O4 fixed", grounds[1])?;
    add_length_dimension(
        document,
        "Four-bar input crank length 5",
        CurveSpan::line(bars[0]),
        5.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    add_length_dimension(
        document,
        "Four-bar coupler length 4",
        CurveSpan::line(bars[1]),
        4.0 * scale,
        DocumentDimensionMode::Driving,
    )?;
    add_length_dimension(
        document,
        "Four-bar output rocker length sqrt 17",
        CurveSpan::line(bars[2]),
        17.0_f64.sqrt() * scale,
        DocumentDimensionMode::Driving,
    )?;
    document.add_constraint(
        "Four-bar tracer bisects coupler",
        DocumentConstraintDefinition::Midpoint {
            point: tracer,
            line: CurveSpan::line(bars[1]),
        },
    )?;
    Ok(MotionFourBarCouplerIds {
        grounds,
        joints,
        tracer,
        bars,
    })
}

fn add_branch_four_bar(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionFourBarCouplerIds, DocumentError> {
    let ids = add_motion_four_bar_coupler(document, scale)?;
    fix_point(
        document,
        "Four-bar input crank locked for branch comparison",
        ids.joints[0],
    )?;
    Ok(ids)
}

fn add_motion_pantograph(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionPantographIds, DocumentError> {
    let anchor = document.add_point("Pantograph fixed anchor O", [0.0, 0.0])?;
    let input = document.add_point("Pantograph input A", [4.0 * scale, 1.0 * scale])?;
    let guide = document.add_point("Pantograph guide B", [1.0 * scale, 3.0 * scale])?;
    let output = document.add_point("Pantograph output C", [5.0 * scale, 4.0 * scale])?;
    let center = document.add_point("Pantograph center M", [2.5 * scale, 2.0 * scale])?;
    let inverse_sqrt_17 = 1.0 / 17.0_f64.sqrt();
    let inverse_sqrt_10 = 1.0 / 10.0_f64.sqrt();
    let bars = [
        add_line_with_direction(
            document,
            "Pantograph input arm O-A",
            anchor,
            input,
            [4.0 * inverse_sqrt_17, inverse_sqrt_17],
        )?,
        add_line_with_direction(
            document,
            "Pantograph guide arm O-B",
            anchor,
            guide,
            [inverse_sqrt_10, 3.0 * inverse_sqrt_10],
        )?,
        add_line_with_direction(
            document,
            "Pantograph translated guide A-C",
            input,
            output,
            [inverse_sqrt_10, 3.0 * inverse_sqrt_10],
        )?,
        add_line_with_direction(
            document,
            "Pantograph translated input B-C",
            guide,
            output,
            [4.0 * inverse_sqrt_17, inverse_sqrt_17],
        )?,
    ];
    let inverse_sqrt_41 = 1.0 / 41.0_f64.sqrt();
    let diagonal = add_line_with_direction(
        document,
        "Pantograph diagonal O-C",
        anchor,
        output,
        [5.0 * inverse_sqrt_41, 4.0 * inverse_sqrt_41],
    )?;
    fix_point(document, "Pantograph anchor fixed", anchor)?;
    document.add_constraint(
        "Pantograph input sides parallel",
        DocumentConstraintDefinition::Parallel {
            first: CurveSpan::line(bars[0]),
            second: CurveSpan::line(bars[3]),
        },
    )?;
    document.add_constraint(
        "Pantograph guide sides parallel",
        DocumentConstraintDefinition::Parallel {
            first: CurveSpan::line(bars[1]),
            second: CurveSpan::line(bars[2]),
        },
    )?;
    add_length_dimension(
        document,
        "Pantograph input arm length sqrt 17",
        CurveSpan::line(bars[0]),
        17.0_f64.sqrt() * scale,
        DocumentDimensionMode::Driving,
    )?;
    add_length_dimension(
        document,
        "Pantograph guide arm length sqrt 10",
        CurveSpan::line(bars[1]),
        10.0_f64.sqrt() * scale,
        DocumentDimensionMode::Driving,
    )?;
    document.add_constraint(
        "Pantograph center bisects diagonal",
        DocumentConstraintDefinition::Midpoint {
            point: center,
            line: CurveSpan::line(diagonal),
        },
    )?;
    Ok(MotionPantographIds {
        anchor,
        input,
        guide,
        output,
        center,
        bars,
    })
}

fn add_motion_drawing_arm(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<MotionDrawingArmIds, DocumentError> {
    let anchor = document.add_point("Drawing arm fixed anchor O", [0.0, 0.0])?;
    let joints = [
        document.add_point("Drawing arm shoulder A", [3.0 * scale, 0.0])?,
        document.add_point("Drawing arm elbow B", [5.0 * scale, 2.0 * scale])?,
        document.add_point("Drawing arm pen C", [7.0 * scale, 1.0 * scale])?,
    ];
    let inverse_sqrt_5 = 1.0 / 5.0_f64.sqrt();
    let links = [
        add_line_with_direction(
            document,
            "Drawing arm link O-A",
            anchor,
            joints[0],
            [1.0, 0.0],
        )?,
        add_line_with_direction(
            document,
            "Drawing arm link A-B",
            joints[0],
            joints[1],
            [
                std::f64::consts::FRAC_1_SQRT_2,
                std::f64::consts::FRAC_1_SQRT_2,
            ],
        )?,
        add_line_with_direction(
            document,
            "Drawing arm link B-C",
            joints[1],
            joints[2],
            [2.0 * inverse_sqrt_5, -inverse_sqrt_5],
        )?,
    ];
    fix_point(document, "Drawing arm anchor fixed", anchor)?;
    for (index, (link, target)) in [
        (links[0], 3.0),
        (links[1], 8.0_f64.sqrt()),
        (links[2], 5.0_f64.sqrt()),
    ]
    .into_iter()
    .enumerate()
    {
        add_length_dimension(
            document,
            &format!("Drawing arm link {} length", index + 1),
            CurveSpan::line(link),
            target * scale,
            DocumentDimensionMode::Driving,
        )?;
    }
    Ok(MotionDrawingArmIds {
        anchor,
        joints,
        links,
    })
}

fn add_branch_locked_elbow(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<BranchLockedElbowIds, DocumentError> {
    let base = document.add_point("Locked elbow base A", [-2.0 * scale, 0.0])?;
    let elbow = document.add_point("Locked elbow joint B", [0.0, 1.5 * scale])?;
    let end = document.add_point("Locked elbow base C", [2.0 * scale, 0.0])?;
    let links = [
        add_line_with_direction(document, "Locked elbow link AB", base, elbow, [0.8, 0.6])?,
        add_line_with_direction(document, "Locked elbow link BC", elbow, end, [0.8, -0.6])?,
    ];
    fix_point(document, "Locked elbow base A fixed", base)?;
    fix_point(document, "Locked elbow base C fixed", end)?;
    for (index, link) in links.into_iter().enumerate() {
        add_length_dimension(
            document,
            &format!("Locked elbow link {} length 2.5", index + 1),
            CurveSpan::line(link),
            2.5 * scale,
            DocumentDimensionMode::Driving,
        )?;
    }
    Ok(BranchLockedElbowIds {
        base,
        elbow,
        end,
        links,
    })
}

fn add_line_with_direction(
    document: &mut SketchDocument,
    label: &str,
    start: DesignPointId,
    end: DesignPointId,
    branch_direction: [f64; 2],
) -> Result<CurveId, DocumentError> {
    document.add_curve(
        label,
        CurveDefinition::Line {
            start,
            end,
            branch_direction,
        },
    )
}

#[allow(clippy::too_many_lines)]
fn add_alpha_corpus(document: &mut SketchDocument, scale: f64) -> Result<(), DocumentError> {
    add_a1(document, scale, [0.0, 0.0])?;
    add_a3(document, scale, [20.0 * scale, 0.0])?;
    add_a4(document, scale, [40.0 * scale, 0.0])?;
    add_corpus_bezier_tangency(document, scale)?;

    document.add_point("corpus point", [80.0 * scale, 0.0])?;
    let polyline_points = [
        document.add_point("corpus polyline A", [82.0 * scale, 0.0])?,
        document.add_point("corpus polyline B", [83.0 * scale, scale])?,
        document.add_point("corpus polyline C", [84.0 * scale, 0.0])?,
    ];
    document.add_curve(
        "corpus polyline",
        CurveDefinition::Polyline {
            points: polyline_points.to_vec(),
            closed: false,
            branch_directions: [
                [
                    std::f64::consts::FRAC_1_SQRT_2,
                    std::f64::consts::FRAC_1_SQRT_2,
                ],
                [
                    std::f64::consts::FRAC_1_SQRT_2,
                    -std::f64::consts::FRAC_1_SQRT_2,
                ],
            ]
            .to_vec(),
        },
    )?;
    let quadratic_controls = [
        document.add_point("corpus quadratic A", [86.0 * scale, 0.0])?,
        document.add_point("corpus quadratic B", [87.0 * scale, scale])?,
        document.add_point("corpus quadratic C", [88.0 * scale, 0.0])?,
    ];
    document.add_curve(
        "corpus quadratic",
        CurveDefinition::QuadraticBezier {
            controls: quadratic_controls,
        },
    )?;

    let coincident_a = document.add_point("corpus coincident A", [100.0 * scale, 0.0])?;
    let coincident_b = document.add_point("corpus coincident B", [100.0 * scale, 0.0])?;
    document.add_constraint(
        "corpus coincident",
        DocumentConstraintDefinition::Coincident {
            first: coincident_a,
            second: coincident_b,
        },
    )?;

    let on_line_a = document.add_point("corpus on-line A", [109.0 * scale, 0.0])?;
    let on_line_b = document.add_point("corpus on-line B", [111.0 * scale, 0.0])?;
    let on_line_point = document.add_point("corpus on-line point", [110.0 * scale, 0.0])?;
    let on_line = add_line(document, "corpus point line", on_line_a, on_line_b)?;
    let point_contact = document.add_curve_contact(
        "corpus point contact",
        CurveSpan::line(on_line),
        0.5,
        0,
        ContactNeighborhood::Interior,
        None,
    )?;
    document.add_constraint(
        "corpus point on curve",
        DocumentConstraintDefinition::PointOnCurve {
            point: on_line_point,
            contact: point_contact,
        },
    )?;

    let parallel_first = corpus_line(
        document,
        "corpus parallel first",
        scale,
        120.0,
        0.0,
        2.0,
        0.0,
    )?;
    let parallel_second = corpus_line(
        document,
        "corpus parallel second",
        scale,
        120.0,
        1.0,
        2.0,
        0.0,
    )?;
    document.add_constraint(
        "corpus parallel",
        DocumentConstraintDefinition::Parallel {
            first: CurveSpan::line(parallel_first),
            second: CurveSpan::line(parallel_second),
        },
    )?;

    let perpendicular_first = corpus_line(
        document,
        "corpus perpendicular first",
        scale,
        130.0,
        0.0,
        2.0,
        0.0,
    )?;
    let perpendicular_second = corpus_line(
        document,
        "corpus perpendicular second",
        scale,
        131.0,
        -1.0,
        0.0,
        2.0,
    )?;
    document.add_constraint(
        "corpus perpendicular",
        DocumentConstraintDefinition::Perpendicular {
            first: CurveSpan::line(perpendicular_first),
            second: CurveSpan::line(perpendicular_second),
        },
    )?;

    let equal_length_first = corpus_line(
        document,
        "corpus equal length first",
        scale,
        140.0,
        0.0,
        2.0,
        0.0,
    )?;
    let equal_length_second = corpus_line(
        document,
        "corpus equal length second",
        scale,
        140.0,
        1.0,
        0.0,
        2.0,
    )?;
    document.add_constraint(
        "corpus equal length",
        DocumentConstraintDefinition::EqualLength {
            first: CurveSpan::line(equal_length_first),
            second: CurveSpan::line(equal_length_second),
        },
    )?;

    let equal_center_a = document.add_point("corpus radius center A", [150.0 * scale, 0.0])?;
    let equal_center_b = document.add_point("corpus radius center B", [153.0 * scale, 0.0])?;
    let equal_radius_a = document.add_scalar(
        "corpus radius A",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let equal_radius_b = document.add_scalar(
        "corpus radius B",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let equal_circle_a = document.add_curve(
        "corpus radius circle A",
        CurveDefinition::Circle {
            center: equal_center_a,
            radius: equal_radius_a,
        },
    )?;
    let equal_circle_b = document.add_curve(
        "corpus radius circle B",
        CurveDefinition::Circle {
            center: equal_center_b,
            radius: equal_radius_b,
        },
    )?;
    document.add_constraint(
        "corpus equal radius",
        DocumentConstraintDefinition::EqualRadius {
            first: equal_circle_a,
            second: equal_circle_b,
        },
    )?;

    let midpoint_line = corpus_line(
        document,
        "corpus midpoint line",
        scale,
        159.0,
        0.0,
        2.0,
        0.0,
    )?;
    let midpoint = document.add_point("corpus midpoint", [160.0 * scale, 0.0])?;
    document.add_constraint(
        "corpus midpoint",
        DocumentConstraintDefinition::Midpoint {
            point: midpoint,
            line: CurveSpan::line(midpoint_line),
        },
    )?;

    let symmetry_axis = corpus_line(
        document,
        "corpus symmetry axis",
        scale,
        168.0,
        0.0,
        4.0,
        0.0,
    )?;
    let symmetry_a = document.add_point("corpus symmetry A", [170.0 * scale, scale])?;
    let symmetry_b = document.add_point("corpus symmetry B", [170.0 * scale, -scale])?;
    document.add_constraint(
        "corpus symmetry",
        DocumentConstraintDefinition::SymmetricAboutLine {
            first: symmetry_a,
            second: symmetry_b,
            line: CurveSpan::line(symmetry_axis),
        },
    )?;

    let contact_first = corpus_line(
        document,
        "corpus contact first",
        scale,
        179.0,
        0.0,
        2.0,
        0.0,
    )?;
    let contact_second = corpus_line(
        document,
        "corpus contact second",
        scale,
        180.0,
        -1.0,
        0.0,
        2.0,
    )?;
    let first_contact = document.add_curve_contact(
        "corpus first curve contact",
        CurveSpan::line(contact_first),
        0.5,
        0,
        ContactNeighborhood::Interior,
        None,
    )?;
    let second_contact = document.add_curve_contact(
        "corpus second curve contact",
        CurveSpan::line(contact_second),
        0.5,
        0,
        ContactNeighborhood::Interior,
        None,
    )?;
    document.add_constraint(
        "corpus curve contact",
        DocumentConstraintDefinition::CurveCurveContact {
            first_contact,
            second_contact,
        },
    )?;

    let tangent_first = corpus_line(
        document,
        "corpus tangent first",
        scale,
        189.0,
        0.0,
        2.0,
        0.0,
    )?;
    let tangent_second = corpus_line(
        document,
        "corpus tangent second",
        scale,
        189.0,
        0.0,
        2.0,
        0.0,
    )?;
    let first_tangent = document.add_curve_contact(
        "corpus first tangent contact",
        CurveSpan::line(tangent_first),
        0.5,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    let second_tangent = document.add_curve_contact(
        "corpus second tangent contact",
        CurveSpan::line(tangent_second),
        0.5,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    document.add_constraint(
        "corpus curve tangency",
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: first_tangent,
            second_contact: second_tangent,
        },
    )?;

    let distance_a = document.add_point("corpus distance A", [200.0 * scale, 0.0])?;
    let distance_b = document.add_point("corpus distance B", [202.0 * scale, 0.0])?;
    let distance_target = document.add_scalar(
        "corpus distance target",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "corpus distance",
        DocumentDimensionDefinition::PointDistance {
            first: distance_a,
            second: distance_b,
            target: distance_target,
        },
        DocumentDimensionMode::Driving,
    )?;

    let diameter_center = document.add_point("corpus diameter center", [210.0 * scale, 0.0])?;
    let diameter_radius = document.add_scalar(
        "corpus diameter radius",
        scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    let diameter_circle = document.add_curve(
        "corpus diameter circle",
        CurveDefinition::Circle {
            center: diameter_center,
            radius: diameter_radius,
        },
    )?;
    let diameter_target = document.add_scalar(
        "corpus diameter target",
        2.0 * scale,
        ScalarUnit::Length,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "corpus diameter",
        DocumentDimensionDefinition::Diameter {
            curve: diameter_circle,
            target: diameter_target,
        },
        DocumentDimensionMode::Driving,
    )?;

    let angle_first = corpus_line(document, "corpus angle first", scale, 220.0, 0.0, 2.0, 0.0)?;
    let angle_second = corpus_line(document, "corpus angle second", scale, 220.0, 0.0, 0.0, 2.0)?;
    let angle_target = document.add_scalar(
        "corpus angle target",
        std::f64::consts::FRAC_PI_2,
        ScalarUnit::Angle,
        ScalarDomain::Positive,
    )?;
    document.add_dimension(
        "corpus angle",
        DocumentDimensionDefinition::OrientedAngle {
            first: CurveSpan::line(angle_first),
            second: CurveSpan::line(angle_second),
            target: angle_target,
            orientation: crate::DocumentAngleOrientation::CounterClockwise,
        },
        DocumentDimensionMode::Driving,
    )?;
    Ok(())
}

fn corpus_line(
    document: &mut SketchDocument,
    label: &str,
    scale: f64,
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
) -> Result<CurveId, DocumentError> {
    let start = document.add_point(format!("{label} A"), [x * scale, y * scale])?;
    let end = document.add_point(format!("{label} B"), [(x + dx) * scale, (y + dy) * scale])?;
    add_line(document, label, start, end)
}

fn suppress_constraint(
    document: &mut SketchDocument,
    constraint: DocumentConstraintId,
) -> Result<(), DocumentError> {
    let source = document
        .constraint(constraint)
        .expect("new scenario constraint")
        .source_id;
    document.set_source_suppressed(source, true)
}

fn add_motion_cam_tangency(
    document: &mut SketchDocument,
    roller_label: &str,
    cam: CurveId,
    cam_parameter: f64,
    circle: CurveId,
    circle_parameter: f64,
) -> Result<DocumentConstraintId, DocumentError> {
    let cam_contact = document.add_curve_contact(
        format!("{roller_label} cam contact"),
        CurveSpan::line(cam),
        cam_parameter,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    let circle_contact = document.add_curve_contact(
        format!("{roller_label} circle contact"),
        CurveSpan::line(circle),
        circle_parameter,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    document.add_constraint(
        format!("{roller_label} tangent to cam"),
        DocumentConstraintDefinition::CurveCurveTangency {
            first_contact: cam_contact,
            second_contact: circle_contact,
        },
    )
}

fn add_corpus_bezier_tangency(
    document: &mut SketchDocument,
    scale: f64,
) -> Result<(), DocumentError> {
    let controls = [
        document.add_point("corpus cubic A", [60.0 * scale, 0.0])?,
        document.add_point("corpus cubic B", [61.0 * scale, 0.0])?,
        document.add_point("corpus cubic C", [62.0 * scale, 0.0])?,
        document.add_point("corpus cubic D", [63.0 * scale, 0.0])?,
    ];
    let bezier = document.add_curve(
        "corpus cubic Bezier",
        CurveDefinition::CubicBezier { controls },
    )?;
    let line_start = document.add_point("corpus Bezier line A", [61.5 * scale, 0.0])?;
    let line_end = document.add_point("corpus Bezier line B", [63.5 * scale, 0.0])?;
    let line = add_line(document, "corpus Bezier tangent line", line_start, line_end)?;
    let contact = document.add_curve_contact(
        "corpus Bezier tangent contact",
        CurveSpan::line(bezier),
        0.5,
        0,
        ContactNeighborhood::Interior,
        Some(TangentOrientation::Aligned),
    )?;
    document.add_constraint(
        "corpus line-Bezier tangency",
        DocumentConstraintDefinition::LineCurveTangency {
            line: CurveSpan::line(line),
            endpoint: FeatureEndpoint::Start,
            curve_contact: contact,
        },
    )?;
    Ok(())
}
