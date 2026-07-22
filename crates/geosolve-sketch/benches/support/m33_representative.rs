// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::{AuditEvaluationStatus, HardValidity, SolverConfig};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DesignPointId, DesignScalarId, DocumentBSplineForm,
    DocumentCommand, DocumentConstraintDefinition, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentEdit, DocumentId, DocumentSolveRequest, DocumentSourceId,
    PersistentId, ScalarDomain, ScalarUnit, SketchDocument, SketchDocumentSession,
    SketchSolveRequest, VisualProfileAnalysis, VisualProfileOptions, VisualProfileStatus,
};

pub const WORKLOADS: [WorkloadKind; 6] = [
    WorkloadKind::Connected,
    WorkloadKind::Disconnected,
    WorkloadKind::SplineHeavy,
    WorkloadKind::ParameterHeavyCurrentV4WorkloadShapeProxy,
    WorkloadKind::ExternalReferenceCurrentV4WorkloadShapeProxy,
    WorkloadKind::ActivationHeavyCurrentV4WorkloadShapeProxy,
];

const REPRESENTATIVE_CONNECTED_SEGMENTS: usize = 64;
const REPRESENTATIVE_DISCONNECTED_RECTANGLES: usize = 32;
const REPRESENTATIVE_SPLINE_CURVES: usize = 16;
const REPRESENTATIVE_SPLINE_CONTROLS: usize = 8;
const REPRESENTATIVE_PARAMETER_CELLS: usize = 64;
const REPRESENTATIVE_EXTERNAL_REFERENCE_CELLS: usize = 32;
const REPRESENTATIVE_ACTIVATION_RECTANGLES: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadKind {
    Connected,
    Disconnected,
    SplineHeavy,
    ParameterHeavyCurrentV4WorkloadShapeProxy,
    ExternalReferenceCurrentV4WorkloadShapeProxy,
    ActivationHeavyCurrentV4WorkloadShapeProxy,
}

impl WorkloadKind {
    pub const fn key(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Disconnected => "disconnected",
            Self::SplineHeavy => "spline_heavy",
            Self::ParameterHeavyCurrentV4WorkloadShapeProxy => "parameter_heavy",
            Self::ExternalReferenceCurrentV4WorkloadShapeProxy => "external_reference",
            Self::ActivationHeavyCurrentV4WorkloadShapeProxy => "activation_heavy",
        }
    }

    pub const fn shape_name(self) -> &'static str {
        match self {
            Self::Connected => "connected current-v4 sketch",
            Self::Disconnected => "disconnected current-v4 sketch",
            Self::SplineHeavy => "spline-heavy current-v4 sketch",
            Self::ParameterHeavyCurrentV4WorkloadShapeProxy => {
                "current-v4 parameter-heavy workload-shape proxy (not an API)"
            }
            Self::ExternalReferenceCurrentV4WorkloadShapeProxy => {
                "current-v4 external-reference workload-shape proxy (not an API)"
            }
            Self::ActivationHeavyCurrentV4WorkloadShapeProxy => {
                "current-v4 activation-heavy workload-shape proxy (not an API)"
            }
        }
    }

    const fn namespace(self) -> u128 {
        match self {
            Self::Connected => 0x3301_0000,
            Self::Disconnected => 0x3302_0000,
            Self::SplineHeavy => 0x3303_0000,
            Self::ParameterHeavyCurrentV4WorkloadShapeProxy => 0x3304_0000,
            Self::ExternalReferenceCurrentV4WorkloadShapeProxy => 0x3305_0000,
            Self::ActivationHeavyCurrentV4WorkloadShapeProxy => 0x3306_0000,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadSize {
    Representative,
    Smoke,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentSignature {
    pub points: usize,
    pub scalars: usize,
    pub curves: usize,
    pub contacts: usize,
    pub constraints: usize,
    pub dimensions: usize,
    pub trim_views: usize,
    pub active_sources: usize,
    pub suppressed_sources: usize,
    pub canonical_bytes: usize,
    pub canonical_fnv1a64: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SolveSignature {
    pub tangent_dimensions: usize,
    pub active_hard_rows: usize,
    pub components: usize,
    pub numerical_rank: usize,
    pub right_nullity: usize,
    pub audit_sources: usize,
    pub audit_rows: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileSignature {
    pub status: VisualProfileStatus,
    pub families: usize,
    pub faces: usize,
    pub intersections: usize,
    pub candidate_pairs: usize,
    pub fragments: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadSignature {
    pub document: DocumentSignature,
    pub solve: SolveSignature,
    pub profile: ProfileSignature,
}

#[allow(clippy::too_many_lines)]
pub const fn expected_representative_signature(kind: WorkloadKind) -> WorkloadSignature {
    match kind {
        WorkloadKind::Connected => WorkloadSignature {
            document: DocumentSignature {
                points: 65,
                scalars: 64,
                curves: 64,
                contacts: 0,
                constraints: 0,
                dimensions: 64,
                trim_views: 0,
                active_sources: 64,
                suppressed_sources: 0,
                canonical_bytes: 57_080,
                canonical_fnv1a64: 474_325_552_708_855_630,
            },
            solve: SolveSignature {
                tangent_dimensions: 130,
                active_hard_rows: 64,
                components: 1,
                numerical_rank: 64,
                right_nullity: 66,
                audit_sources: 64,
                audit_rows: 64,
            },
            profile: ProfileSignature {
                status: VisualProfileStatus::Complete,
                families: 1,
                faces: 0,
                intersections: 0,
                candidate_pairs: 2_016,
                fragments: 64,
            },
        },
        WorkloadKind::Disconnected => WorkloadSignature {
            document: DocumentSignature {
                points: 128,
                scalars: 64,
                curves: 128,
                contacts: 0,
                constraints: 160,
                dimensions: 64,
                trim_views: 0,
                active_sources: 224,
                suppressed_sources: 0,
                canonical_bytes: 124_903,
                canonical_fnv1a64: 3_663_696_500_421_568_382,
            },
            solve: SolveSignature {
                tangent_dimensions: 256,
                active_hard_rows: 192,
                components: 64,
                numerical_rank: 192,
                right_nullity: 0,
                audit_sources: 352,
                audit_rows: 512,
            },
            profile: ProfileSignature {
                status: VisualProfileStatus::Complete,
                families: 1,
                faces: 32,
                intersections: 0,
                candidate_pairs: 8_128,
                fragments: 128,
            },
        },
        WorkloadKind::SplineHeavy => WorkloadSignature {
            document: DocumentSignature {
                points: 128,
                scalars: 128,
                curves: 16,
                contacts: 0,
                constraints: 16,
                dimensions: 0,
                trim_views: 0,
                active_sources: 16,
                suppressed_sources: 0,
                canonical_bytes: 54_942,
                canonical_fnv1a64: 15_540_598_803_879_734_887,
            },
            solve: SolveSignature {
                tangent_dimensions: 368,
                active_hard_rows: 0,
                components: 240,
                numerical_rank: 0,
                right_nullity: 336,
                audit_sources: 16,
                audit_rows: 32,
            },
            profile: ProfileSignature {
                status: VisualProfileStatus::Complete,
                families: 1,
                faces: 0,
                intersections: 0,
                candidate_pairs: 3_240,
                fragments: 80,
            },
        },
        WorkloadKind::ParameterHeavyCurrentV4WorkloadShapeProxy => WorkloadSignature {
            document: DocumentSignature {
                points: 128,
                scalars: 64,
                curves: 64,
                contacts: 0,
                constraints: 128,
                dimensions: 64,
                trim_views: 0,
                active_sources: 192,
                suppressed_sources: 0,
                canonical_bytes: 101_294,
                canonical_fnv1a64: 860_480_882_882_553_057,
            },
            solve: SolveSignature {
                tangent_dimensions: 256,
                active_hard_rows: 128,
                components: 128,
                numerical_rank: 128,
                right_nullity: 0,
                audit_sources: 320,
                audit_rows: 512,
            },
            profile: ProfileSignature {
                status: VisualProfileStatus::Complete,
                families: 1,
                faces: 0,
                intersections: 0,
                candidate_pairs: 2_016,
                fragments: 64,
            },
        },
        WorkloadKind::ExternalReferenceCurrentV4WorkloadShapeProxy => WorkloadSignature {
            document: DocumentSignature {
                points: 96,
                scalars: 64,
                curves: 32,
                contacts: 0,
                constraints: 64,
                dimensions: 64,
                trim_views: 0,
                active_sources: 128,
                suppressed_sources: 0,
                canonical_bytes: 72_037,
                canonical_fnv1a64: 9_169_199_859_917_444_177,
            },
            solve: SolveSignature {
                tangent_dimensions: 192,
                active_hard_rows: 64,
                components: 96,
                numerical_rank: 64,
                right_nullity: 0,
                audit_sources: 224,
                audit_rows: 384,
            },
            profile: ProfileSignature {
                status: VisualProfileStatus::Complete,
                families: 1,
                faces: 0,
                intersections: 0,
                candidate_pairs: 496,
                fragments: 32,
            },
        },
        WorkloadKind::ActivationHeavyCurrentV4WorkloadShapeProxy => WorkloadSignature {
            document: DocumentSignature {
                points: 128,
                scalars: 64,
                curves: 128,
                contacts: 0,
                constraints: 160,
                dimensions: 64,
                trim_views: 0,
                active_sources: 128,
                suppressed_sources: 96,
                canonical_bytes: 132_961,
                canonical_fnv1a64: 15_026_922_256_822_046_457,
            },
            solve: SolveSignature {
                tangent_dimensions: 256,
                active_hard_rows: 96,
                components: 128,
                numerical_rank: 96,
                right_nullity: 96,
                audit_sources: 256,
                audit_rows: 416,
            },
            profile: ProfileSignature {
                status: VisualProfileStatus::Complete,
                families: 1,
                faces: 32,
                intersections: 0,
                candidate_pairs: 8_128,
                fragments: 128,
            },
        },
    }
}

#[derive(Clone, Copy, Debug)]
enum WorkloadEdit {
    Scalar {
        scalar: DesignScalarId,
        value: f64,
    },
    Point {
        point: DesignPointId,
        position: [f64; 2],
    },
    Unsuppress {
        source: DocumentSourceId,
    },
}

impl WorkloadEdit {
    fn command(self, expected_revision: u64) -> DocumentCommand {
        let edit = match self {
            Self::Scalar { scalar, value } => DocumentEdit::SetScalarValue { scalar, value },
            Self::Point { point, position } => DocumentEdit::SetPointPosition { point, position },
            Self::Unsuppress { source } => DocumentEdit::SetSourceSuppressed {
                source,
                suppressed: false,
            },
        };
        DocumentCommand::new(expected_revision, edit)
    }
}

#[derive(Clone, Debug)]
pub struct RepresentativeWorkload {
    pub kind: WorkloadKind,
    pub document: SketchDocument,
    pub request: DocumentSolveRequest,
    edit: WorkloadEdit,
}

impl RepresentativeWorkload {
    pub fn edit_command(&self, expected_revision: u64) -> DocumentCommand {
        self.edit.command(expected_revision)
    }

    pub fn document_signature(&self) -> DocumentSignature {
        document_signature(&self.document)
    }

    pub fn runtime_request(&self) -> SketchSolveRequest {
        if self.request.previous_state_preferences {
            SketchSolveRequest::default()
        } else {
            SketchSolveRequest::default().without_previous_state_preferences()
        }
    }
}

#[derive(Clone, Debug)]
pub struct PreparedWorkload {
    pub definition: RepresentativeWorkload,
    pub accepted: SketchDocumentSession,
    pub profile_options: VisualProfileOptions,
    pub signature: WorkloadSignature,
}

impl PreparedWorkload {
    pub fn prepare(definition: RepresentativeWorkload) -> Self {
        let accepted = SketchDocumentSession::new(
            definition.document.clone(),
            definition.request,
            SolverConfig::default(),
        )
        .expect("representative workload must solve");
        validate_session(&accepted);

        let mut edited = accepted.clone();
        let outcome = edited
            .apply(definition.edit_command(edited.revision()))
            .expect("representative edit must execute");
        assert!(
            outcome.accepted(),
            "{} edit rejected: {:#?}",
            definition.kind.key(),
            outcome.result.solve()
        );
        validate_session(&edited);

        let lowered = definition
            .document
            .lower()
            .expect("representative document must lower");
        let compiled = lowered
            .sketch()
            .compile(definition.runtime_request())
            .expect("representative sketch must compile");
        assert_eq!(
            compiled
                .problem()
                .packed_layout()
                .unwrap()
                .tangent_dimension(),
            accepted
                .accepted_result()
                .accepted_view()
                .core_report
                .structural
                .tangent_dimensions
        );

        let profile_options = VisualProfileOptions::default();
        let profile = accepted.document().analyze_visual_profiles(profile_options);
        validate_profile(&profile, profile_options);

        let signature = WorkloadSignature {
            document: definition.document_signature(),
            solve: solve_signature(&accepted),
            profile: profile_signature(&profile),
        };
        Self {
            definition,
            accepted,
            profile_options,
            signature,
        }
    }
}

pub fn workloads(size: WorkloadSize) -> Vec<RepresentativeWorkload> {
    WORKLOADS
        .into_iter()
        .map(|kind| build_workload(kind, size))
        .collect()
}

pub fn build_workload(kind: WorkloadKind, size: WorkloadSize) -> RepresentativeWorkload {
    let mut document = SketchDocument::with_id(
        10.0,
        DocumentId(PersistentId::from_u128(
            kind.namespace() + u128::from(size == WorkloadSize::Smoke),
        )),
    )
    .expect("M33 workload namespace and scale are valid");
    let edit = match kind {
        WorkloadKind::Connected => {
            let count = select_size(size, REPRESENTATIVE_CONNECTED_SEGMENTS, 8);
            add_connected(&mut document, count)
        }
        WorkloadKind::Disconnected => {
            let count = select_size(size, REPRESENTATIVE_DISCONNECTED_RECTANGLES, 3);
            add_disconnected(&mut document, count)
        }
        WorkloadKind::SplineHeavy => {
            let curves = select_size(size, REPRESENTATIVE_SPLINE_CURVES, 2);
            let controls = select_size(size, REPRESENTATIVE_SPLINE_CONTROLS, 6);
            add_spline_heavy(&mut document, curves, controls)
        }
        WorkloadKind::ParameterHeavyCurrentV4WorkloadShapeProxy => {
            let count = select_size(size, REPRESENTATIVE_PARAMETER_CELLS, 4);
            add_parameter_heavy_current_v4_workload_shape_proxy(&mut document, count)
        }
        WorkloadKind::ExternalReferenceCurrentV4WorkloadShapeProxy => {
            let count = select_size(size, REPRESENTATIVE_EXTERNAL_REFERENCE_CELLS, 3);
            add_external_reference_current_v4_workload_shape_proxy(&mut document, count)
        }
        WorkloadKind::ActivationHeavyCurrentV4WorkloadShapeProxy => {
            let count = select_size(size, REPRESENTATIVE_ACTIVATION_RECTANGLES, 3);
            add_activation_heavy_current_v4_workload_shape_proxy(&mut document, count)
        }
    };
    document.validate().expect("M33 workload must be valid");
    let request = match kind {
        WorkloadKind::Connected | WorkloadKind::SplineHeavy => {
            DocumentSolveRequest::default().without_previous_state_preferences()
        }
        WorkloadKind::Disconnected
        | WorkloadKind::ParameterHeavyCurrentV4WorkloadShapeProxy
        | WorkloadKind::ExternalReferenceCurrentV4WorkloadShapeProxy
        | WorkloadKind::ActivationHeavyCurrentV4WorkloadShapeProxy => {
            DocumentSolveRequest::default()
        }
    };
    RepresentativeWorkload {
        kind,
        document,
        request,
        edit,
    }
}

pub fn validate_session(session: &SketchDocumentSession) {
    session.document().validate().unwrap();
    let result = session.accepted_result();
    let accepted = result.accepted_view();
    let report = &accepted.core_report;
    assert!(accepted.accepted(), "{:#?}", accepted.rejection);
    assert_eq!(report.hard_validity, HardValidity::Valid);
    assert!(report.hard_residuals_validated);
    assert!(report.hard_residual_max.is_finite());
    assert!(report.hard_residual_max <= 1.0e-9);
    assert!(report.rank_is_valid);
    assert!(
        accepted
            .acceptance_hard_residual_max
            .is_some_and(|value| value.is_finite() && value <= 1.0e-9)
    );
    for source in &accepted.display_audit.sources {
        assert!(!source.source_label.is_empty());
        for row in &source.rows {
            assert_eq!(row.evaluation_status, AuditEvaluationStatus::Evaluated);
            assert!(!row.template.is_empty());
            assert!(row.scale.is_finite() && row.scale > 0.0);
            assert!(row.raw_residual.is_finite());
            assert!(row.normalized_residual.is_finite());
        }
    }
}

pub fn validate_profile(analysis: &VisualProfileAnalysis, options: VisualProfileOptions) {
    assert_eq!(
        analysis.status,
        VisualProfileStatus::Complete,
        "{analysis:#?}"
    );
    assert!(analysis.issues.is_empty(), "{analysis:#?}");
    let counters = [
        (
            analysis.budgets.candidate_pairs,
            options.max_candidate_pairs,
        ),
        (
            analysis.budgets.intersection_subdivisions,
            options.max_intersection_subdivisions,
        ),
        (
            analysis.budgets.intersection_roots,
            options.max_intersection_roots,
        ),
        (analysis.budgets.fragments, options.max_fragments),
        (
            analysis.budgets.integration_subdivisions,
            options.max_integration_subdivisions,
        ),
        (
            analysis.budgets.containment_tests,
            options.max_containment_tests,
        ),
        (analysis.budgets.faces, options.max_faces),
    ];
    for (counter, limit) in counters {
        assert_eq!(counter.limit, limit);
        assert!(counter.consumed <= counter.limit);
    }
    for intersection in &analysis.intersections {
        assert!(
            intersection
                .first_parameter_enclosure
                .into_iter()
                .chain(intersection.second_parameter_enclosure)
                .chain(intersection.position_enclosure.into_iter().flatten())
                .all(f64::is_finite)
        );
    }
    for face in &analysis.faces {
        assert!(face.visual_area.is_finite());
        assert!(face.area_uncertainty.is_finite());
        for contour in &face.contours {
            assert!(contour.signed_area.is_finite());
            assert!(contour.area_uncertainty.is_finite());
            for edge in &contour.edges {
                assert!(
                    edge.start
                        .into_iter()
                        .chain(edge.end)
                        .chain(edge.source_parameters)
                        .chain(edge.source_parameter_enclosures.into_iter().flatten())
                        .all(f64::is_finite)
                );
            }
        }
    }
}

fn add_connected(document: &mut SketchDocument, segments: usize) -> WorkloadEdit {
    let points = (0..=segments)
        .map(|index| {
            let index_f64 = f64_from_usize(index);
            document
                .add_point(
                    format!("M33 connected chain point {index:03}"),
                    [1.25 * index_f64, 0.4 * (0.31 * index_f64).sin()],
                )
                .unwrap()
        })
        .collect::<Vec<_>>();
    let mut edit = None;
    for index in 0..segments {
        let curve = add_line(
            document,
            format!("M33 connected chain segment {index:03}"),
            points[index],
            points[index + 1],
        );
        let first = document.point(points[index]).unwrap().position;
        let second = document.point(points[index + 1]).unwrap().position;
        let length = (second[0] - first[0]).hypot(second[1] - first[1]);
        let target = add_length_dimension(
            document,
            format!("M33 connected chain length {index:03}"),
            curve,
            length,
        );
        if index == segments / 2 {
            edit = Some(WorkloadEdit::Scalar {
                scalar: target,
                value: length * 1.01,
            });
        }
    }
    edit.expect("connected workload has a middle segment")
}

fn add_disconnected(document: &mut SketchDocument, rectangles: usize) -> WorkloadEdit {
    let mut edit = None;
    for index in 0..rectangles {
        let column = index % 8;
        let row = index / 8;
        let width = 2.0 + 0.05 * f64_from_usize(index % 5);
        let height = 1.25 + 0.05 * f64_from_usize(index % 3);
        let ids = document
            .add_rectangle(
                &format!("M33 disconnected rectangle {index:03}"),
                [4.0 * f64_from_usize(column), 3.0 * f64_from_usize(row)],
                width,
                height,
            )
            .unwrap();
        if index == 0 {
            edit = Some(WorkloadEdit::Scalar {
                scalar: ids.targets[0],
                value: width * 1.02,
            });
        }
    }
    edit.expect("disconnected workload has a rectangle")
}

fn add_spline_heavy(
    document: &mut SketchDocument,
    curves: usize,
    controls_per_curve: usize,
) -> WorkloadEdit {
    assert!(controls_per_curve >= 6);
    let degree = 3_u32;
    let span_count = controls_per_curve - usize::try_from(degree).unwrap();
    let mut edit = None;
    for curve_index in 0..curves {
        let controls = (0..controls_per_curve)
            .map(|control_index| {
                let x = 1.1 * f64_from_usize(control_index);
                let phase = 0.53 * f64_from_usize(control_index + curve_index);
                let y = 4.0 * f64_from_usize(curve_index) + 0.7 * phase.sin();
                document
                    .add_point(
                        format!(
                            "M33 spline-heavy curve {curve_index:03} control {control_index:02}"
                        ),
                        [x, y],
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let weights = (0..controls_per_curve)
            .map(|control_index| {
                let weight = if control_index == 0 {
                    1.0
                } else {
                    0.85 + 0.05 * f64_from_usize((curve_index + control_index) % 7)
                };
                document
                    .add_scalar(
                        format!(
                            "M33 spline-heavy curve {curve_index:03} weight {control_index:02}"
                        ),
                        weight,
                        ScalarUnit::Parameter,
                        ScalarDomain::Positive,
                    )
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let mut knots = vec![0.0; usize::try_from(degree).unwrap() + 1];
        for knot in 1..span_count {
            knots.push(f64_from_usize(knot));
        }
        knots.extend(std::iter::repeat_n(
            f64_from_usize(span_count),
            usize::try_from(degree).unwrap() + 1,
        ));
        let first_span = u32::try_from(curve_index * 100).unwrap();
        let span_ids = (0..span_count)
            .map(|span| first_span + u32::try_from(span).unwrap())
            .collect::<Vec<_>>();
        let gauge_weight = weights[0];
        document
            .add_curve(
                format!("M33 spline-heavy NURBS {curve_index:03}"),
                CurveDefinition::Nurbs {
                    form: DocumentBSplineForm::Clamped,
                    degree,
                    controls: controls.clone(),
                    weights,
                    gauge_weight,
                    knots,
                    span_ids,
                    next_span_id: first_span + u32::try_from(span_count).unwrap(),
                },
            )
            .unwrap();
        let fixed = document.point(controls[0]).unwrap().position;
        document
            .add_constraint(
                format!("M33 spline-heavy anchor {curve_index:03}"),
                DocumentConstraintDefinition::FixedPoint {
                    point: controls[0],
                    target: fixed,
                },
            )
            .unwrap();
        if curve_index == 0 {
            let point = controls[controls_per_curve / 2];
            let mut position = document.point(point).unwrap().position;
            position[1] += 0.2;
            edit = Some(WorkloadEdit::Point { point, position });
        }
    }
    edit.expect("spline-heavy workload has a curve")
}

fn add_parameter_heavy_current_v4_workload_shape_proxy(
    document: &mut SketchDocument,
    cells: usize,
) -> WorkloadEdit {
    let mut edit = None;
    for index in 0..cells {
        let y = 1.5 * f64_from_usize(index);
        let length = 2.0 + 0.02 * f64_from_usize(index % 11);
        let start = document
            .add_point(
                format!("M33 current-v4 parameter proxy start {index:03}"),
                [0.0, y],
            )
            .unwrap();
        let end = document
            .add_point(
                format!("M33 current-v4 parameter proxy end {index:03}"),
                [length, y],
            )
            .unwrap();
        let line = add_line(
            document,
            format!("M33 current-v4 parameter proxy line {index:03}"),
            start,
            end,
        );
        document
            .add_constraint(
                format!("M33 current-v4 parameter proxy anchor {index:03}"),
                DocumentConstraintDefinition::FixedPoint {
                    point: start,
                    target: [0.0, y],
                },
            )
            .unwrap();
        document
            .add_constraint(
                format!("M33 current-v4 parameter proxy horizontal {index:03}"),
                DocumentConstraintDefinition::Horizontal {
                    line: CurveSpan::line(line),
                },
            )
            .unwrap();
        let scalar = add_length_dimension(
            document,
            format!("M33 current-v4 parameter-fed target proxy {index:03}"),
            line,
            length,
        );
        if index == cells / 2 {
            edit = Some(WorkloadEdit::Scalar {
                scalar,
                value: length * 1.01,
            });
        }
    }
    edit.expect("parameter-heavy proxy has a middle cell")
}

fn add_external_reference_current_v4_workload_shape_proxy(
    document: &mut SketchDocument,
    cells: usize,
) -> WorkloadEdit {
    let mut edit = None;
    for index in 0..cells {
        let column = index % 8;
        let row = index / 8;
        let origin = [5.0 * f64_from_usize(column), 4.0 * f64_from_usize(row)];
        let first = document
            .add_point(
                format!("M33 current-v4 external snapshot proxy A {index:03}"),
                origin,
            )
            .unwrap();
        let second_position = [origin[0] + 2.0, origin[1]];
        let second = document
            .add_point(
                format!("M33 current-v4 external snapshot proxy B {index:03}"),
                second_position,
            )
            .unwrap();
        let local_position = [origin[0] + 1.0, origin[1] + 1.5];
        let local = document
            .add_point(
                format!("M33 local dependent point {index:03}"),
                local_position,
            )
            .unwrap();
        add_line(
            document,
            format!("M33 current-v4 external support proxy {index:03}"),
            first,
            second,
        );
        for (suffix, point, target) in [("A", first, origin), ("B", second, second_position)] {
            document
                .add_constraint(
                    format!("M33 current-v4 external proxy {suffix} fixed {index:03}"),
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .unwrap();
        }
        let distance = 1.0_f64.hypot(1.5);
        let first_target = add_point_distance_dimension(
            document,
            format!("M33 external proxy A to local {index:03}"),
            first,
            local,
            distance,
        );
        add_point_distance_dimension(
            document,
            format!("M33 external proxy B to local {index:03}"),
            second,
            local,
            distance,
        );
        if index == 0 {
            edit = Some(WorkloadEdit::Scalar {
                scalar: first_target,
                value: distance * 1.01,
            });
        }
    }
    edit.expect("external-reference proxy has a cell")
}

fn add_activation_heavy_current_v4_workload_shape_proxy(
    document: &mut SketchDocument,
    rectangles: usize,
) -> WorkloadEdit {
    let mut edit = None;
    for index in 0..rectangles {
        let column = index % 8;
        let row = index / 8;
        let ids = document
            .add_rectangle(
                &format!("M33 current-v4 activation proxy rectangle {index:03}"),
                [4.0 * f64_from_usize(column), 3.0 * f64_from_usize(row)],
                2.0,
                1.25,
            )
            .unwrap();
        let suppressed = [
            document.constraint(ids.constraints[2]).unwrap().source_id,
            document.constraint(ids.constraints[3]).unwrap().source_id,
            document.dimension(ids.dimensions[1]).unwrap().source_id,
        ];
        for source in suppressed {
            document.set_source_suppressed(source, true).unwrap();
        }
        if index == 0 {
            edit = Some(WorkloadEdit::Unsuppress {
                source: suppressed[0],
            });
        }
    }
    edit.expect("activation-heavy proxy has a rectangle")
}

fn add_line(
    document: &mut SketchDocument,
    label: impl Into<String>,
    start: DesignPointId,
    end: DesignPointId,
) -> geosolve_sketch::CurveId {
    let first = document.point(start).unwrap().position;
    let second = document.point(end).unwrap().position;
    let delta = [second[0] - first[0], second[1] - first[1]];
    let norm = delta[0].hypot(delta[1]);
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [delta[0] / norm, delta[1] / norm],
            },
        )
        .unwrap()
}

fn add_length_dimension(
    document: &mut SketchDocument,
    label: impl Into<String>,
    curve: geosolve_sketch::CurveId,
    value: f64,
) -> DesignScalarId {
    let label = label.into();
    let target = document
        .add_scalar(
            format!("{label} target"),
            value,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_dimension(
            label,
            DocumentDimensionDefinition::CurveLength {
                curve: CurveSpan::line(curve),
                target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    target
}

fn add_point_distance_dimension(
    document: &mut SketchDocument,
    label: impl Into<String>,
    first: DesignPointId,
    second: DesignPointId,
    value: f64,
) -> DesignScalarId {
    let label = label.into();
    let target = document
        .add_scalar(
            format!("{label} target"),
            value,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_dimension(
            label,
            DocumentDimensionDefinition::PointDistance {
                first,
                second,
                target,
            },
            DocumentDimensionMode::Driving,
        )
        .unwrap();
    target
}

fn document_signature(document: &SketchDocument) -> DocumentSignature {
    let canonical = document.to_canonical_json().unwrap();
    let suppressed_sources = document
        .sources()
        .filter(|source| source.suppressed)
        .count();
    DocumentSignature {
        points: document.points().len(),
        scalars: document.scalars().len(),
        curves: document.curves().len(),
        contacts: document.contacts().len(),
        constraints: document.constraints().len(),
        dimensions: document.dimensions().len(),
        trim_views: document.trim_views().len(),
        active_sources: document.source_order().len() - suppressed_sources,
        suppressed_sources,
        canonical_bytes: canonical.len(),
        canonical_fnv1a64: fnv1a64(canonical.as_bytes()),
    }
}

fn solve_signature(session: &SketchDocumentSession) -> SolveSignature {
    let result = session.accepted_result();
    let accepted = result.accepted_view();
    let report = &accepted.core_report;
    SolveSignature {
        tangent_dimensions: report.structural.tangent_dimensions,
        active_hard_rows: report.structural.active_hard_rows,
        components: report.structural.components,
        numerical_rank: report.rank,
        right_nullity: report.right_nullity,
        audit_sources: accepted.display_audit.sources.len(),
        audit_rows: accepted
            .display_audit
            .sources
            .iter()
            .map(|source| source.rows.len())
            .sum(),
    }
}

fn profile_signature(analysis: &VisualProfileAnalysis) -> ProfileSignature {
    ProfileSignature {
        status: analysis.status,
        families: analysis.families.len(),
        faces: analysis.faces.len(),
        intersections: analysis.intersections.len(),
        candidate_pairs: analysis.candidate_pairs,
        fragments: analysis.fragment_count,
    }
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn select_size(size: WorkloadSize, representative: usize, smoke: usize) -> usize {
    match size {
        WorkloadSize::Representative => representative,
        WorkloadSize::Smoke => smoke,
    }
}

fn f64_from_usize(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("M33 workload index fits u32"))
}
