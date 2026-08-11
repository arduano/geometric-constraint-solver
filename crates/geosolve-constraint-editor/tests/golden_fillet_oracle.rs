// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(not(target_arch = "wasm32"))]
#![allow(
    clippy::too_many_lines,
    reason = "the four reviewed Fillet permutations remain one process-isolated golden family"
)]

use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use geosolve_constraint_editor::{
    FeatureAuthoringOutcome, FeatureAuthoringStage, FeatureAuthoringState, FeatureAuthoringTool,
    RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveSpan, DesignPointId, DocumentArcSweep,
    DocumentConstraintDefinition, DocumentCurveNormalSide, DocumentFilletEndpointOrder,
    DocumentFilletTrimEndpoint, DocumentId, DocumentSolveRequest, DocumentTrimParameter,
    OperationControl, OperationOutcome, PersistentId, RetainedSketchDocumentSession, ScalarDomain,
    ScalarUnit, SketchDocument, SketchHardValidity, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedEdgeGeometry, ComputedEvaluationAllocator, ComputedFeatureAuthoringSnapshot,
    ComputedFeatureDocument, ComputedFeatureDocumentId, ComputedFeatureEvaluationPolicy,
    ComputedFeatureEvaluationSnapshot, ComputedFeatureEvaluationState, ComputedFeatureFailure,
    ComputedFilletContactReseedRequest, ComputedFilletParent, ComputedFilletParentIndex,
    NativeCurveSpanSource, NewComputedFilletCorner,
};

const FAMILY: &str = "feature.fillet";
const TSV_HEADER: &str = "case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint";
const CASE_IDS: [&str; 4] = [
    "feature.fillet.authoring.coincident-closure.curve-pair",
    "feature.fillet.authoring.coincident-closure.point",
    "feature.fillet.evaluation.line-circle.same-cell-lower",
    "feature.fillet.evaluation.line-circle.same-cell-seam",
];

#[derive(Clone, Debug)]
struct SemanticDefect {
    class: &'static str,
    message: String,
}

type OracleResult = Result<(), SemanticDefect>;

fn defect(class: &'static str, message: impl Into<String>) -> SemanticDefect {
    SemanticDefect {
        class,
        message: message.into(),
    }
}

#[derive(Clone, Debug)]
struct Observation {
    input_fingerprint: String,
    outcome: OracleResult,
}

#[derive(Clone, Debug)]
struct SurveyRow {
    case_id: String,
    status: &'static str,
    failure_class: String,
    fingerprint: String,
}

impl SurveyRow {
    fn pass(case_id: &str, input_fingerprint: String) -> Self {
        Self {
            case_id: case_id.into(),
            status: "PASS",
            failure_class: "-".into(),
            fingerprint: input_fingerprint,
        }
    }

    fn failed(case_id: &str, observation: &Observation, failure: &SemanticDefect) -> Self {
        let detail = format!(
            "input={}; {}",
            observation.input_fingerprint,
            sanitize_tsv(&failure.message)
        );
        Self {
            case_id: case_id.into(),
            status: "DEFECT",
            failure_class: failure.class.into(),
            fingerprint: format!("{:016x}:{detail}", fnv1a64(detail.as_bytes())),
        }
    }

    fn panicked(case_id: &str, message: &str) -> Self {
        let detail = sanitize_tsv(message);
        Self {
            case_id: case_id.into(),
            status: "PANIC",
            failure_class: "test-panic".into(),
            fingerprint: format!("{:016x}:{detail}", fnv1a64(detail.as_bytes())),
        }
    }

    fn write_to(&self, output: &mut impl Write) -> std::io::Result<()> {
        writeln!(
            output,
            "{}\t{FAMILY}\t{}\t-\t{}\t{}",
            self.case_id, self.status, self.failure_class, self.fingerprint
        )
    }
}

#[test]
fn golden_fillet_oracle_inventory_and_tsv_schema_are_exhaustive() {
    assert_eq!(CASE_IDS.len(), 4);
    assert_eq!(TSV_HEADER.split('\t').count(), 6);
    assert_eq!(
        CASE_IDS.into_iter().collect::<BTreeSet<_>>().len(),
        CASE_IDS.len()
    );
    assert!(CASE_IDS.into_iter().all(|case| case.starts_with(FAMILY)));
}

#[test]
fn golden_fillet_oracle_survey() {
    let selected = env::var("GEOSOLVE_GOLDEN_ORACLE_CASE");
    let output = env::var("GEOSOLVE_GOLDEN_ORACLE_OUTPUT");
    if selected.is_err() && output.is_err() {
        return;
    }
    let selected = selected.expect("GEOSOLVE_GOLDEN_ORACLE_CASE must accompany oracle output");
    let output = output.expect("GEOSOLVE_GOLDEN_ORACLE_OUTPUT must accompany oracle case");
    assert!(
        CASE_IDS.contains(&selected.as_str()),
        "unknown Fillet oracle case"
    );

    let row = match catch_unwind(AssertUnwindSafe(|| observe(&selected))) {
        Ok(observation) => match observation.outcome.clone() {
            Ok(()) => SurveyRow::pass(&selected, observation.input_fingerprint),
            Err(failure) => SurveyRow::failed(&selected, &observation, &failure),
        },
        Err(payload) => SurveyRow::panicked(&selected, &panic_payload(&payload)),
    };
    let file = File::create(&output)
        .unwrap_or_else(|error| panic!("cannot create Fillet oracle TSV {output}: {error}"));
    let mut output = BufWriter::new(file);
    writeln!(output, "{TSV_HEADER}").expect("write Fillet oracle header");
    row.write_to(&mut output).expect("write Fillet oracle row");
    output.flush().expect("flush Fillet oracle row");
}

fn observe(case_id: &str) -> Observation {
    match case_id {
        "feature.fillet.authoring.coincident-closure.point" => {
            observe_coincident_closure(ClosureRoute::Point)
        }
        "feature.fillet.authoring.coincident-closure.curve-pair" => {
            observe_coincident_closure(ClosureRoute::CurvePair)
        }
        "feature.fillet.evaluation.line-circle.same-cell-lower" => {
            observe_line_circle(LineCircleRow::LOWER)
        }
        "feature.fillet.evaluation.line-circle.same-cell-seam" => {
            observe_line_circle(LineCircleRow::SEAM)
        }
        _ => unreachable!("inventory checked the selected case"),
    }
}

#[derive(Clone, Copy)]
enum ClosureRoute {
    Point,
    CurvePair,
}

struct ClosedTriangleFixture {
    coordinator: RetainedEditorCoordinator,
    points: [DesignPointId; 4],
    spans: [CurveSpan; 3],
    input_fingerprint: String,
}

fn closed_triangle_fixture(route: ClosureRoute) -> ClosedTriangleFixture {
    let mut document = SketchDocument::with_id(
        10.0,
        DocumentId(PersistentId::from_u128(
            0x676f_6c64_656e_5f66_696c_6c65_7400_0001,
        )),
    )
    .expect("golden triangle document");
    let points = [
        document.add_point("first", [0.0, 0.0]).expect("first"),
        document.add_point("second", [6.0, 0.0]).expect("second"),
        document.add_point("third", [3.0, 5.0]).expect("third"),
        document
            .add_point("last coincident", [0.25, -0.15])
            .expect("last"),
    ];
    let curve = document
        .add_curve(
            "coincident-closed open triangle",
            CurveDefinition::Polyline {
                points: points.to_vec(),
                closed: false,
                branch_directions: vec![
                    [1.0, 0.0],
                    [-0.514_495_755_427_526_5, 0.857_492_925_712_544_1],
                    [-0.514_495_755_427_526_5, -0.857_492_925_712_544_1],
                ],
            },
        )
        .expect("triangle polyline");
    document
        .add_constraint(
            "close endpoints",
            DocumentConstraintDefinition::Coincident {
                first: points[0],
                second: points[3],
            },
        )
        .expect("closure constraint");
    let design_json = document.to_canonical_json().expect("triangle design JSON");
    let session = retained(document);
    assert_current_accepted(&session);
    let accepted_json = session
        .accepted_state_for_current_input()
        .expect("accepted triangle")
        .document()
        .to_canonical_json()
        .expect("accepted triangle JSON");
    let route_tag = match route {
        ClosureRoute::Point => "point",
        ClosureRoute::CurvePair => "curve-pair",
    };
    let input_fingerprint = input_fingerprint(&[&design_json, &accepted_json, route_tag]);
    ClosedTriangleFixture {
        coordinator: RetainedEditorCoordinator::new(session).expect("triangle coordinator"),
        points,
        spans: [0, 1, 2].map(|segment| CurveSpan { curve, segment }),
        input_fingerprint,
    }
}

fn observe_coincident_closure(route: ClosureRoute) -> Observation {
    let mut fixture = closed_triangle_fixture(route);
    let snapshot = fixture
        .coordinator
        .feature_authoring_snapshot()
        .expect("triangle authoring snapshot");
    let mut state = FeatureAuthoringState::default();
    assert!(matches!(
        state.activate(
            &snapshot,
            snapshot.sketch_document(),
            FeatureAuthoringTool::Fillet,
            &[],
        ),
        FeatureAuthoringOutcome::ModeEntered(_)
    ));

    let outcome = match route {
        ClosureRoute::Point => fixture
            .coordinator
            .transact_feature_authoring_pick_items(
                &mut state,
                &[(SelectionItem::Point(fixture.points[0]), None)],
                "golden coincident closure point",
            )
            .expect("typed point transaction"),
        ClosureRoute::CurvePair => {
            let first = fixture
                .coordinator
                .transact_feature_authoring_pick_items(
                    &mut state,
                    &[(SelectionItem::Curve(fixture.spans[2]), Some(0.75))],
                    "golden closing span",
                )
                .expect("first curve transaction");
            if !matches!(
                first.outcome,
                FeatureAuthoringOutcome::Collecting {
                    ref pending,
                    ref guidance,
                } if pending.len() == 1
                    && guidance.stage == FeatureAuthoringStage::PickSecondFilletCurve
            ) {
                return Observation {
                    input_fingerprint: fixture.input_fingerprint,
                    outcome: Err(defect(
                        "fillet.authoring.topology",
                        format!(
                            "first closure span did not enter collection: {:?}",
                            first.outcome
                        ),
                    )),
                };
            }
            fixture
                .coordinator
                .transact_feature_authoring_pick_items(
                    &mut state,
                    &[(SelectionItem::Curve(fixture.spans[0]), Some(0.25))],
                    "golden first span",
                )
                .expect("second curve transaction")
        }
    };

    let candidate = match &outcome.outcome {
        FeatureAuthoringOutcome::PreviewRequested { candidate, .. }
            if candidate.corners().len() == 1 =>
        {
            candidate.clone()
        }
        other => {
            return Observation {
                input_fingerprint: fixture.input_fingerprint,
                outcome: Err(defect(
                    "fillet.authoring.topology",
                    format!("Coincident-equivalent closure was not authorable: {other:?}"),
                )),
            };
        }
    };
    let expected_sources = [fixture.spans[2], fixture.spans[0]]
        .into_iter()
        .collect::<BTreeSet<_>>();
    let authored_sources = [
        candidate.corners()[0].corner.first.source.span,
        candidate.corners()[0].corner.second.source.span,
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    if authored_sources != expected_sources {
        return Observation {
            input_fingerprint: fixture.input_fingerprint,
            outcome: Err(defect(
                "fillet.authoring.topology",
                "closure candidate resolved to spans other than the two incident closure spans",
            )),
        };
    }
    let Some(preview) = outcome.preview else {
        return Observation {
            input_fingerprint: fixture.input_fingerprint,
            outcome: Err(defect(
                "fillet.authoring.publication",
                "complete closure candidate had no exact computed preview",
            )),
        };
    };
    if let Err(error) = fixture
        .coordinator
        .apply_feature_authoring_preview(preview.token, &candidate)
    {
        return Observation {
            input_fingerprint: fixture.input_fingerprint,
            outcome: Err(defect(
                "fillet.authoring.publication",
                format!("closure preview did not publish: {error}"),
            )),
        };
    }
    let current = fixture
        .coordinator
        .computed_snapshot()
        .expect("published closure snapshot");
    let current_corners = current
        .feature_evaluations()
        .iter()
        .filter(|evaluation| {
            matches!(
                evaluation.state,
                ComputedFeatureEvaluationState::Current { .. }
            )
        })
        .count();
    let arcs = current
        .edges()
        .iter()
        .filter_map(|edge| match &edge.geometry {
            ComputedEdgeGeometry::CircularArc(arc) => Some(arc),
            _ => None,
        })
        .collect::<Vec<_>>();
    let published_sources = arcs.first().map(|arc| {
        arc.contacts
            .iter()
            .map(|contact| contact.source.span)
            .collect::<BTreeSet<_>>()
    });
    Observation {
        input_fingerprint: fixture.input_fingerprint,
        outcome: if current_corners == 1
            && arcs.len() == 1
            && published_sources.as_ref() == Some(&expected_sources)
        {
            Ok(())
        } else {
            Err(defect(
                "fillet.authoring.publication",
                "published closure did not contain one Current Fillet arc on the two closure spans",
            ))
        },
    }
}

#[derive(Clone, Copy)]
struct LineCircleRow {
    line_start: [f64; 2],
    line_end: [f64; 2],
    viable_circle_parameter: f64,
    viable_circle_winding: i32,
}

impl LineCircleRow {
    const LOWER: Self = Self {
        line_start: [-5.020_101_821_235_499_5, 0.079_969_938_399_629_43],
        line_end: [4.232_404_345_772_32, 0.079_969_938_399_628_96],
        viable_circle_parameter: 5.551_739_581_930_468,
        viable_circle_winding: 0,
    };
    const SEAM: Self = Self {
        line_start: [-5.020_101_821_235_499_5, 2.043_335_287_688_456],
        line_end: [4.596_861_386_658_269_5, 2.043_335_287_688_455],
        viable_circle_parameter: 6.517_367_674_350_06,
        viable_circle_winding: 1,
    };
}

struct LineCircleFixture {
    session: RetainedSketchDocumentSession,
    features: ComputedFeatureDocument,
    feature: geosolve_sketch_features::ComputedFeatureId,
    corner: NewComputedFilletCorner,
    input_fingerprint: String,
}

fn line_circle_fixture(row: LineCircleRow) -> LineCircleFixture {
    let mut document = SketchDocument::with_id(
        10.0,
        DocumentId(PersistentId::from_u128(
            0x9455_3000_3fee_983a_59bf_6060_4279_fed7,
        )),
    )
    .expect("line-circle document");
    let center = document
        .add_point(
            "draft point",
            [-0.964_047_656_537_027_3, 2.537_115_794_695_225],
        )
        .expect("circle center");
    let radius = document
        .add_scalar(
            "radius",
            1.181_531_590_369_537_4,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .expect("circle radius");
    let circle = CurveSpan::line(
        document
            .add_curve("circle", CurveDefinition::Circle { center, radius })
            .expect("circle"),
    );
    let start = document
        .add_point("draft point", row.line_start)
        .expect("line start");
    let end = document
        .add_point("draft point", row.line_end)
        .expect("line end");
    let line = CurveSpan::line(
        document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line"),
    );
    document
        .add_constraint(
            "auto horizontal",
            DocumentConstraintDefinition::Horizontal { line },
        )
        .expect("horizontal line");
    let session = retained(document);
    assert_current_accepted(&session);
    let corner = NewComputedFilletCorner {
        first: ComputedFilletParent {
            source: NativeCurveSpanSource { span: circle },
            picked_parameter: 6.010_678_569_256_539,
            winding: 0,
            neighborhood: ContactNeighborhood::Local {
                lower: 4.712_388_980_384_694,
                upper: 7.853_981_633_974_479,
            },
            normal_side: DocumentCurveNormalSide::Right,
            retained_endpoint: DocumentFilletTrimEndpoint::End,
            periodic_anchor: Some(DocumentTrimParameter {
                parameter: 2.869_085_915_666_746,
                winding: 0,
            }),
        },
        second: ComputedFilletParent {
            source: NativeCurveSpanSource { span: line },
            picked_parameter: 0.634_799_522_276_009_7,
            winding: 0,
            neighborhood: ContactNeighborhood::Interior,
            normal_side: DocumentCurveNormalSide::Left,
            retained_endpoint: DocumentFilletTrimEndpoint::End,
            periodic_anchor: None,
        },
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
    };
    let mut features = ComputedFeatureDocument::with_id(
        session.design_document().id(),
        ComputedFeatureDocumentId::from_raw(0xf330_5f73_5082_ee5a_3fda_0114_370b_9ba4),
    );
    let feature = features
        .create_fillet_set("Fillet 1", 1.0, vec![corner])
        .expect("persisted Fillet");
    let sketch_json = session
        .accepted_state_for_current_input()
        .expect("accepted line-circle")
        .document()
        .to_canonical_json()
        .expect("line-circle JSON");
    let feature_json = features.to_json().expect("feature JSON");
    let input_fingerprint = input_fingerprint(&[&sketch_json, &feature_json]);
    LineCircleFixture {
        session,
        features,
        feature,
        corner,
        input_fingerprint,
    }
}

fn observe_line_circle(row: LineCircleRow) -> Observation {
    let fixture = line_circle_fixture(row);
    let snapshot = evaluate(&fixture.session, &fixture.features);
    let evaluation = snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == fixture.feature)
        .expect("line-circle feature evaluation");
    if matches!(
        evaluation.state,
        ComputedFeatureEvaluationState::Current { .. }
    ) {
        let arc = snapshot
            .edges()
            .iter()
            .find_map(|edge| match &edge.geometry {
                ComputedEdgeGeometry::CircularArc(arc) => Some(arc),
                _ => None,
            })
            .expect("Current line-circle Fillet arc");
        let outcome = validate_same_branch_arc(
            fixture
                .session
                .accepted_state_for_current_input()
                .expect("accepted line-circle")
                .document(),
            fixture.corner,
            arc,
            row,
        );
        return Observation {
            input_fingerprint: fixture.input_fingerprint,
            outcome,
        };
    }

    let failure = match &evaluation.state {
        ComputedFeatureEvaluationState::Failed {
            failure: ComputedFeatureFailure::NoLocalRoot { .. },
        } => {
            let authoring = ComputedFeatureAuthoringSnapshot::capture(&fixture.session)
                .expect("line-circle authoring snapshot");
            let reanchored = complete(
                authoring
                    .reseed_fillet_contact(
                        ComputedFilletContactReseedRequest {
                            prior: fixture.corner,
                            parent: ComputedFilletParentIndex::First,
                            parameter: row
                                .viable_circle_parameter
                                .rem_euclid(std::f64::consts::TAU),
                        },
                        1.0,
                        ComputedFeatureEvaluationPolicy::default(),
                        OperationControl::unlimited(),
                    )
                    .expect("same-cell public reseed"),
            );
            if let Err(failure) = validate_reseeded_branch(fixture.corner, &reanchored.corner, row)
            {
                return Observation {
                    input_fingerprint: fixture.input_fingerprint,
                    outcome: Err(failure),
                };
            }
            if let Err(failure) = validate_same_branch_arc(
                fixture
                    .session
                    .accepted_state_for_current_input()
                    .expect("accepted line-circle")
                    .document(),
                reanchored.corner,
                &reanchored.arc,
                row,
            ) {
                return Observation {
                    input_fingerprint: fixture.input_fingerprint,
                    outcome: Err(failure),
                };
            }
            defect(
                "fillet.evaluation.branch-locality",
                format!(
                    "persisted evaluation returned NoLocalRoot although circle parameter {:.15} with winding {} is valid inside the unchanged Local cell",
                    row.viable_circle_parameter, row.viable_circle_winding
                ),
            )
        }
        other => defect(
            "fillet.evaluation.branch-locality",
            format!("line-circle same-cell evaluation was not Current: {other:?}"),
        ),
    };
    Observation {
        input_fingerprint: fixture.input_fingerprint,
        outcome: Err(failure),
    }
}

fn validate_reseeded_branch(
    persisted: NewComputedFilletCorner,
    actual: &NewComputedFilletCorner,
    row: LineCircleRow,
) -> OracleResult {
    if actual.first.source != persisted.first.source
        || actual.second.source != persisted.second.source
        || actual.first.normal_side != persisted.first.normal_side
        || actual.second.normal_side != persisted.second.normal_side
        || actual.first.retained_endpoint != persisted.first.retained_endpoint
        || actual.second.retained_endpoint != persisted.second.retained_endpoint
        || actual.endpoint_order != persisted.endpoint_order
        || actual.sweep != persisted.sweep
        || actual.first.winding != row.viable_circle_winding
        || actual.second.winding != persisted.second.winding
        || actual.second.periodic_anchor.is_some()
    {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "viable root changed explicit source, cell, side, retention, order, sweep or winding",
        ));
    }
    let ContactNeighborhood::Local { lower, upper } = actual.first.neighborhood else {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "circle contact lost its Local cell",
        ));
    };
    let ContactNeighborhood::Local {
        lower: persisted_lower,
        upper: persisted_upper,
    } = persisted.first.neighborhood
    else {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "persisted circle contact has no Local cell",
        ));
    };
    if (lower - persisted_lower).abs() > 2.0e-14
        || (upper - persisted_upper).abs() > 2.0e-14
        || actual.second.neighborhood != ContactNeighborhood::Interior
    {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "viable root changed the persisted contact neighborhoods",
        ));
    }
    if !(row.viable_circle_parameter > lower && row.viable_circle_parameter < upper) {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "viable circle parameter escaped the persisted Local cell",
        ));
    }
    let expected_principal = row
        .viable_circle_parameter
        .rem_euclid(std::f64::consts::TAU);
    if (actual.first.picked_parameter - expected_principal).abs() > 2.0e-10 {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "viable root parameter and winding do not encode the expected circle contact",
        ));
    }
    let (Some(actual_anchor), Some(persisted_anchor)) = (
        actual.first.periodic_anchor,
        persisted.first.periodic_anchor,
    ) else {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "viable root lost the persisted periodic anchor",
        ));
    };
    let actual_total =
        actual.first.picked_parameter + f64::from(actual.first.winding) * std::f64::consts::TAU;
    let persisted_total = persisted.first.picked_parameter
        + f64::from(persisted.first.winding) * std::f64::consts::TAU;
    let actual_anchor_total =
        actual_anchor.parameter + f64::from(actual_anchor.winding) * std::f64::consts::TAU;
    let persisted_anchor_total =
        persisted_anchor.parameter + f64::from(persisted_anchor.winding) * std::f64::consts::TAU;
    if ((actual_anchor_total - persisted_anchor_total) - (actual_total - persisted_total)).abs()
        > 2.0e-12
    {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "viable root moved its periodic anchor incoherently with the contact parameter",
        ));
    }
    Ok(())
}

fn validate_same_branch_arc(
    document: &SketchDocument,
    corner: NewComputedFilletCorner,
    arc: &geosolve_sketch_features::ComputedCircularArc,
    row: LineCircleRow,
) -> OracleResult {
    let ContactNeighborhood::Local { lower, upper } = corner.first.neighborhood else {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "persisted circle parent lost its explicit Local cell",
        ));
    };
    if corner.first.normal_side != DocumentCurveNormalSide::Right
        || corner.second.normal_side != DocumentCurveNormalSide::Left
        || corner.first.retained_endpoint != DocumentFilletTrimEndpoint::End
        || corner.second.retained_endpoint != DocumentFilletTrimEndpoint::End
        || corner.second.neighborhood != ContactNeighborhood::Interior
        || corner.first.periodic_anchor
            != Some(DocumentTrimParameter {
                parameter: 2.869_085_915_666_746,
                winding: 0,
            })
        || corner.second.periodic_anchor.is_some()
        || corner.endpoint_order != DocumentFilletEndpointOrder::FirstThenSecond
        || corner.sweep != DocumentArcSweep::CounterClockwise
    {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "persisted line-circle source, side, retention, neighborhood, anchor, order or sweep changed",
        ));
    }
    if !arc.center.into_iter().all(f64::is_finite)
        || !arc.radius.is_finite()
        || arc.contacts.iter().any(|contact| {
            !contact.parameter.is_finite()
                || !contact.total_parameter.is_finite()
                || !contact.position.into_iter().all(f64::is_finite)
        })
    {
        return Err(defect(
            "fillet.evaluation.invalid-geometry",
            "line-circle Fillet published non-finite geometry",
        ));
    }
    if (arc.radius - 1.0).abs() > 1.0e-12 || arc.sweep != DocumentArcSweep::CounterClockwise {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "line-circle Fillet changed its radius or explicit sweep",
        ));
    }
    if (arc.contacts[0].total_parameter - row.viable_circle_parameter).abs() > 2.0e-9
        || arc.contacts[0].winding != row.viable_circle_winding
        || !(lower < arc.contacts[0].total_parameter && arc.contacts[0].total_parameter < upper)
    {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "line-circle Fillet selected a different circle root, winding or Local cell",
        ));
    }
    for (index, (parent, contact)) in [corner.first, corner.second]
        .into_iter()
        .zip(arc.contacts)
        .enumerate()
    {
        if contact.source != parent.source {
            return Err(defect(
                "fillet.evaluation.branch-state",
                "published contact changed its native source",
            ));
        }
        let represented_total = if index == 0 {
            contact.parameter + f64::from(contact.winding) * std::f64::consts::TAU
        } else {
            if contact.winding != 0 {
                return Err(defect(
                    "fillet.evaluation.branch-state",
                    "bounded line contact acquired a winding",
                ));
            }
            contact.parameter
        };
        if (represented_total - contact.total_parameter).abs() > 2.0e-12 {
            return Err(defect(
                "fillet.evaluation.branch-state",
                "contact parameter and winding do not represent the published total parameter",
            ));
        }
        let jet = document
            .evaluate_curve_jet(parent.source.span, contact.total_parameter)
            .map_err(|error| {
                defect(
                    "fillet.evaluation.invalid-geometry",
                    format!("source contact jet failed: {error}"),
                )
            })?;
        let position_error =
            (jet.position.x - contact.position[0]).hypot(jet.position.y - contact.position[1]);
        let radial = [
            arc.center[0] - contact.position[0],
            arc.center[1] - contact.position[1],
        ];
        let radial_length = radial[0].hypot(radial[1]);
        let tangent_length = jet.first_derivative.x.hypot(jet.first_derivative.y);
        let normalized_tangency =
            (jet.first_derivative.x * radial[0] + jet.first_derivative.y * radial[1]).abs()
                / (tangent_length * radial_length);
        let signed_offset = radial[0] * (-jet.first_derivative.y / tangent_length)
            + radial[1] * (jet.first_derivative.x / tangent_length);
        let expected_offset = match parent.normal_side {
            DocumentCurveNormalSide::Left => arc.radius,
            DocumentCurveNormalSide::Right => -arc.radius,
        };
        if position_error > 1.0e-9
            || (radial_length - arc.radius).abs() > 1.0e-9
            || normalized_tangency > 1.0e-9
            || (signed_offset - expected_offset).abs() > 1.0e-9
        {
            return Err(defect(
                "fillet.evaluation.invalid-geometry",
                "line-circle Fillet failed incidence, radius, tangency or signed-side validation",
            ));
        }
    }
    let (start, end) = match corner.endpoint_order {
        DocumentFilletEndpointOrder::FirstThenSecond => (arc.contacts[0], arc.contacts[1]),
        DocumentFilletEndpointOrder::SecondThenFirst => (arc.contacts[1], arc.contacts[0]),
    };
    for (angle, contact) in [(arc.start_angle, start), (arc.end_angle, end)] {
        let expected =
            (contact.position[1] - arc.center[1]).atan2(contact.position[0] - arc.center[0]);
        let delta = angle - expected;
        if delta.sin().atan2(delta.cos()).abs() > 1.0e-9 {
            return Err(defect(
                "fillet.evaluation.branch-state",
                "line-circle Fillet endpoint angles changed the explicit endpoint order",
            ));
        }
    }
    Ok(())
}

fn retained(document: SketchDocument) -> RetainedSketchDocumentSession {
    RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("golden accepted session")
}

fn assert_current_accepted(session: &RetainedSketchDocumentSession) {
    let accepted = session
        .accepted_state_for_current_input()
        .expect("golden input must be current and accepted");
    assert!(
        accepted
            .document()
            .points()
            .iter()
            .all(|point| point.position.into_iter().all(f64::is_finite))
    );
    assert!(
        accepted
            .document()
            .scalars()
            .iter()
            .all(|scalar| scalar.value.is_finite())
    );
    let solve = accepted.diagnostics().solve.expect("solve diagnostics");
    assert_eq!(solve.hard_validity, SketchHardValidity::Valid);
    assert!(solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_some_and(|residual| residual <= 1.0e-9)
    );
}

fn evaluate(
    session: &RetainedSketchDocumentSession,
    features: &ComputedFeatureDocument,
) -> geosolve_sketch_features::ComputedFeatureSnapshot {
    let snapshot = ComputedFeatureEvaluationSnapshot::capture(
        session,
        features,
        ComputedFeatureEvaluationPolicy::default(),
    )
    .expect("computed evaluation snapshot");
    complete(
        snapshot
            .prepare(&mut ComputedEvaluationAllocator::default())
            .expect("prepared computed evaluation")
            .execute(OperationControl::unlimited())
            .expect("computed evaluation"),
    )
}

fn complete<T: std::fmt::Debug>(outcome: OperationOutcome<T>) -> T {
    match outcome {
        OperationOutcome::Completed { value, .. } => value,
        other => panic!("expected completed golden operation, got {other:?}"),
    }
}

fn input_fingerprint(parts: &[&str]) -> String {
    let mut bytes = Vec::new();
    for part in parts {
        bytes.extend_from_slice(part.as_bytes());
        bytes.push(0);
    }
    format!("input-{:016x}", fnv1a64(&bytes))
}

fn sanitize_tsv(value: &str) -> String {
    value
        .chars()
        .map(|value| match value {
            '\t' | '\n' | '\r' => ' ',
            other => other,
        })
        .collect()
}

const fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    let mut index = 0;
    while index < bytes.len() {
        hash ^= bytes[index] as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        index += 1;
    }
    hash
}

fn panic_payload(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".into()
    }
}
