// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(not(target_arch = "wasm32"))]
#![allow(
    clippy::too_many_lines,
    reason = "the five reviewed Fillet permutations remain one process-isolated golden family"
)]

use std::collections::BTreeSet;
use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use geosolve_constraint_editor::{
    FeatureAuthoringOptions, FeatureAuthoringOutcome, FeatureAuthoringStage, FeatureAuthoringState,
    FeatureAuthoringTool, RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    ContactNeighborhood, CurveDefinition, CurveSpan, DesignPointId, DocumentArcSweep,
    DocumentConstraintDefinition, DocumentCurveNormalSide, DocumentDimensionDefinition,
    DocumentDimensionMode, DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentId,
    DocumentSolveRequest, DocumentTrimParameter, GeometryRole, OperationControl, OperationOutcome,
    PersistentId, RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
    SketchHardValidity, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedCornerRef, ComputedEdgeGeometry, ComputedEvaluationAllocator,
    ComputedFeatureAuthoringSnapshot, ComputedFeatureDefinition, ComputedFeatureDocument,
    ComputedFeatureDocumentId, ComputedFeatureEvaluationPolicy, ComputedFeatureEvaluationSnapshot,
    ComputedFeatureEvaluationState, ComputedFeatureFailure, ComputedFilletContactReseedRequest,
    ComputedFilletParent, ComputedFilletParentIndex, NativeCurveSpanSource,
    NewComputedFilletCorner,
};

const FAMILY: &str = "feature.fillet";
const TSV_HEADER: &str = "case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint";
const CASE_IDS: [&str; 6] = [
    "feature.fillet.authoring.coincident-closure.curve-pair",
    "feature.fillet.authoring.coincident-closure.point",
    "feature.fillet.authoring.native-profile.line-line",
    "feature.fillet.evaluation.line-circle.same-cell-lower",
    "feature.fillet.evaluation.line-circle.same-cell-seam",
    "feature.fillet.evaluation.line-circle.source-rotation.retained-start",
];

const SOURCE_ROTATION_SKETCH_JSON: &str = concat!(
    r#"{"version":4,"id":"7653a0003fed873aee16ee394279fe5e","next_id":"7653a0003fed873aee16ee394279fe65","model_scale":10.0,"points":["#,
    r#"{"id":"7653a0003fed873aee16ee394279fe5f","label":"draft point","position":[0.16002449354493023,1.9065418176251467]},"#,
    r#"{"id":"7653a0003fed873aee16ee394279fe62","label":"draft point","position":[-2.6404041434913528,2.0437056692350866]},"#,
    r#"{"id":"7653a0003fed873aee16ee394279fe63","label":"draft point","position":[1.371638516099403,4.855564627238864]}],"#,
    r#""scalars":[{"id":"7653a0003fed873aee16ee394279fe60","label":"radius","value":2.201783656372145,"unit":"length","domain":{"kind":"positive"}}],"#,
    r#""curves":[{"id":"7653a0003fed873aee16ee394279fe61","label":"circle","definition":{"kind":"circle","center":"7653a0003fed873aee16ee394279fe5f","radius":"7653a0003fed873aee16ee394279fe60"}},"#,
    r#"{"id":"7653a0003fed873aee16ee394279fe64","label":"line","definition":{"kind":"line","start":"7653a0003fed873aee16ee394279fe62","end":"7653a0003fed873aee16ee394279fe63","branch_direction":[0.9748804436785523,0.22272880490208083]}}],"#,
    r#""contacts":[],"trim_views":[],"constraints":[],"dimensions":[],"source_order":[]}"#,
);

const SOURCE_ROTATION_FEATURE_JSON: &str = concat!(
    r#"{"version":1,"document_id":"1136cf735081f15888738f4d370b9b2d","sketch_document":"7653a0003fed873aee16ee394279fe5e","revision":7,"next_feature_id":"0000000000000002","next_corner_id":"0000000000000002","features":["#,
    r#"{"id":"0000000000000001","label":"Fillet 1","suppressed":false,"definition":{"kind":"fillet_set","radius":1.0,"corners":["#,
    r#"{"id":"0000000000000001","first":{"source":{"span":{"curve":"7653a0003fed873aee16ee394279fe61","segment":0}},"picked_parameter":0.01630131737160223,"winding":1,"neighborhood":{"local":{"lower":4.959571177211237,"upper":7.857323073392596}},"normal_side":"right","retained_endpoint":"end","periodic_anchor":{"parameter":3.1578939709613953,"winding":0}},"#,
    r#""second":{"source":{"span":{"curve":"7653a0003fed873aee16ee394279fe64","segment":0}},"picked_parameter":0.6995120213306758,"winding":0,"neighborhood":"interior","normal_side":"left","retained_endpoint":"start","periodic_anchor":null},"#,
    r#""endpoint_order":"first_then_second","sweep":"counter_clockwise"}]}}],"digest":"df8408ece03aa63593d91056ed1d09592f4f1f2654cb2616f205be04cb217081"}"#,
);

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
    assert_eq!(CASE_IDS.len(), 6);
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
        "feature.fillet.authoring.native-profile.line-line" => observe_native_profile_line_line(),
        "feature.fillet.evaluation.line-circle.same-cell-lower" => {
            observe_line_circle(LineCircleRow::LOWER)
        }
        "feature.fillet.evaluation.line-circle.same-cell-seam" => {
            observe_line_circle(LineCircleRow::SEAM)
        }
        "feature.fillet.evaluation.line-circle.source-rotation.retained-start" => {
            observe_source_rotation()
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

struct NativeProfileVariant {
    document: SketchDocument,
    corner: DesignPointId,
    spans: [CurveSpan; 2],
    tag: &'static str,
}

fn transform_native_profile_point(
    point: [f64; 2],
    scale: f64,
    rotation: f64,
    translation: [f64; 2],
) -> [f64; 2] {
    let (sin, cos) = rotation.sin_cos();
    [
        translation[0] + scale * (cos * point[0] - sin * point[1]),
        translation[1] + scale * (sin * point[0] + cos * point[1]),
    ]
}

fn transform_native_profile_direction(direction: [f64; 2], rotation: f64) -> [f64; 2] {
    let (sin, cos) = rotation.sin_cos();
    [
        cos * direction[0] - sin * direction[1],
        sin * direction[0] + cos * direction[1],
    ]
}

fn add_native_profile_line(
    document: &mut SketchDocument,
    label: &'static str,
    start: DesignPointId,
    end: DesignPointId,
    branch_direction: [f64; 2],
) -> CurveSpan {
    CurveSpan::line(
        document
            .add_curve(
                label,
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction,
                },
            )
            .expect("golden native Profile line"),
    )
}

fn native_profile_variant(
    ordinal: u128,
    tag: &'static str,
    scale: f64,
    rotation: f64,
    translation: [f64; 2],
    reverse_creation: bool,
) -> NativeProfileVariant {
    let mut document = SketchDocument::with_id(
        4.0 * scale,
        DocumentId(PersistentId::from_u128(
            0x6e61_7469_7665_5f66_696c_6c65_7400_0100 + ordinal,
        )),
    )
    .expect("golden native Profile document");
    let first_outer = document
        .add_point(
            "first outer",
            transform_native_profile_point([-3.0, 0.0], scale, rotation, translation),
        )
        .expect("first outer");
    let corner = document
        .add_point(
            "sharp corner",
            transform_native_profile_point([0.0, 0.0], scale, rotation, translation),
        )
        .expect("sharp corner");
    let second_outer = document
        .add_point(
            "second outer",
            transform_native_profile_point([0.0, 3.0], scale, rotation, translation),
        )
        .expect("second outer");
    let first_direction = transform_native_profile_direction([1.0, 0.0], rotation);
    let second_direction = transform_native_profile_direction([0.0, 1.0], rotation);
    let (first, second) = if reverse_creation {
        let second = add_native_profile_line(
            &mut document,
            "second line",
            corner,
            second_outer,
            second_direction,
        );
        let first = add_native_profile_line(
            &mut document,
            "first line",
            first_outer,
            corner,
            first_direction,
        );
        (first, second)
    } else {
        let first = add_native_profile_line(
            &mut document,
            "first line",
            first_outer,
            corner,
            first_direction,
        );
        let second = add_native_profile_line(
            &mut document,
            "second line",
            corner,
            second_outer,
            second_direction,
        );
        (first, second)
    };
    NativeProfileVariant {
        document,
        corner,
        spans: [first, second],
        tag,
    }
}

fn observe_native_profile_line_line() -> Observation {
    let variants = vec![
        native_profile_variant(0, "base", 1.0, 0.0, [0.0, 0.0], false),
        native_profile_variant(
            1,
            "tiny-translated-rotated",
            1.0e-6,
            0.47,
            [2.0e-6, -1.0e-6],
            false,
        ),
        native_profile_variant(
            2,
            "large-translated-rotated-reversed",
            1.0e6,
            -1.13,
            [-2.0e6, 1.5e6],
            true,
        ),
    ];
    let mut input_parts = Vec::with_capacity(variants.len() * 2);
    for variant in &variants {
        input_parts.push(variant.tag.to_owned());
        input_parts.push(
            variant
                .document
                .to_canonical_json()
                .expect("native Profile golden input JSON"),
        );
    }
    let input_refs = input_parts.iter().map(String::as_str).collect::<Vec<_>>();
    let input_fingerprint = input_fingerprint(&input_refs);
    for variant in variants {
        if let Err(failure) = validate_native_profile_variant(variant) {
            return Observation {
                input_fingerprint,
                outcome: Err(failure),
            };
        }
    }
    Observation {
        input_fingerprint,
        outcome: Ok(()),
    }
}

fn validate_native_profile_variant(variant: NativeProfileVariant) -> OracleResult {
    let model_scale = variant.document.model_scale();
    let session = retained(variant.document);
    assert_current_accepted(&session);
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("native coordinator");
    let snapshot = coordinator
        .feature_authoring_snapshot()
        .expect("native authoring snapshot");
    let mut state = FeatureAuthoringState::default();
    if !matches!(
        state.activate(
            &snapshot,
            snapshot.sketch_document(),
            FeatureAuthoringTool::Fillet,
            &[],
        ),
        FeatureAuthoringOutcome::ModeEntered(_)
    ) {
        return Err(defect(
            "fillet.native.authoring",
            format!("{}: Fillet mode did not activate", variant.tag),
        ));
    }
    if matches!(
        state.set_options(
            &snapshot,
            FeatureAuthoringOptions {
                fillet_radius: Some(0.25 * model_scale),
                ..FeatureAuthoringOptions::default()
            },
        ),
        FeatureAuthoringOutcome::Warning(_)
    ) {
        return Err(defect(
            "fillet.native.authoring",
            format!("{}: transformed radius was rejected", variant.tag),
        ));
    }
    let transaction = coordinator
        .transact_feature_authoring_pick_items(
            &mut state,
            &[(SelectionItem::Point(variant.corner), None)],
            format!("{} native Profile", variant.tag),
        )
        .map_err(|error| {
            defect(
                "fillet.native.authoring",
                format!("{}: corner collection failed: {error}", variant.tag),
            )
        })?;
    let FeatureAuthoringOutcome::PreviewRequested { candidate, .. } = transaction.outcome else {
        return Err(defect(
            "fillet.native.authoring",
            format!("{}: corner did not produce one preview", variant.tag),
        ));
    };
    let Some(preview) = transaction.preview else {
        return Err(defect(
            "fillet.native.authority",
            format!("{}: candidate had no exact held preview", variant.tag),
        ));
    };
    let [corner_preview] = candidate.corners() else {
        return Err(defect(
            "fillet.native.authoring",
            format!("{}: native candidate was not one corner", variant.tag),
        ));
    };
    coordinator
        .native_feature_authoring_availability(preview.token, &candidate)
        .map_err(|error| {
            defect(
                "fillet.native.eligibility",
                format!(
                    "{}: eligible transformed corner was unavailable: {error}",
                    variant.tag
                ),
            )
        })?;
    let expected_orientations = corner_preview.arc.tangent_orientations;
    let history_before = coordinator.history_len();
    let mutation = coordinator
        .apply_feature_authoring_native_profile(preview.token, &candidate)
        .map_err(|error| {
            defect(
                "fillet.native.publication",
                format!("{}: exact native publication failed: {error}", variant.tag),
            )
        })?;
    let ids = mutation.value;
    assert_current_accepted(coordinator.session());
    let document = coordinator.session().design_document();
    let expected_sources = variant
        .spans
        .into_iter()
        .map(|span| span.curve)
        .collect::<BTreeSet<_>>();
    let published_sources = ids.source_lines.into_iter().collect::<BTreeSet<_>>();
    let source_lines_exist = ids.source_lines.iter().all(|line| {
        matches!(
            document.curve(*line).map(|curve| &curve.definition),
            Some(CurveDefinition::Line { .. })
        )
    });
    let arc_is_native = matches!(
        document.curve(ids.arc).map(|curve| &curve.definition),
        Some(CurveDefinition::CircularArc { .. })
    ) && document.geometry_role(ids.arc) == Some(GeometryRole::Profile);
    let tangencies_match = ids
        .tangencies
        .iter()
        .enumerate()
        .all(|(index, constraint)| {
            matches!(
                document.constraint(*constraint).map(|value| &value.definition),
                Some(DocumentConstraintDefinition::LineCurveTangency { curve_contact, .. })
                    if *curve_contact == ids.contacts[index]
            ) && document
                .contact(ids.contacts[index])
                .is_some_and(|contact| {
                    contact.tangent_orientation == Some(expected_orientations[index])
                })
        });
    let radius_matches = document
        .dimension(ids.radius_dimension)
        .is_some_and(|dimension| {
            dimension.mode == DocumentDimensionMode::Driving
                && matches!(
                    dimension.definition,
                    DocumentDimensionDefinition::Radius { curve, target }
                        if curve == ids.arc && target == ids.radius_target
                )
        });
    if expected_sources != published_sources
        || !source_lines_exist
        || document.point(variant.corner).is_some()
        || !arc_is_native
        || !tangencies_match
        || !radius_matches
        || !coordinator.feature_document().features().is_empty()
        || coordinator.history_len() != history_before + 1
    {
        return Err(defect(
            "fillet.native.publication",
            format!(
                "{}: publication lost native lines/arc/tangencies/Radius, branch state, or one-step history",
                variant.tag
            ),
        ));
    }
    coordinator.undo().map_err(|error| {
        defect(
            "fillet.native.lifecycle",
            format!("{}: Undo failed: {error}", variant.tag),
        )
    })?;
    if coordinator
        .session()
        .design_document()
        .curve(ids.arc)
        .is_some()
        || coordinator
            .session()
            .design_document()
            .point(variant.corner)
            .is_none()
    {
        return Err(defect(
            "fillet.native.lifecycle",
            format!("{}: Undo did not restore the sharp corner", variant.tag),
        ));
    }
    coordinator.redo().map_err(|error| {
        defect(
            "fillet.native.lifecycle",
            format!("{}: Redo failed: {error}", variant.tag),
        )
    })?;
    assert_current_accepted(coordinator.session());
    if coordinator
        .session()
        .design_document()
        .curve(ids.arc)
        .is_none()
        || coordinator
            .session()
            .design_document()
            .point(variant.corner)
            .is_some()
    {
        return Err(defect(
            "fillet.native.lifecycle",
            format!("{}: Redo did not restore native identities", variant.tag),
        ));
    }
    Ok(())
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

fn observe_source_rotation() -> Observation {
    let document = SketchDocument::from_json(SOURCE_ROTATION_SKETCH_JSON)
        .expect("source-rotation accepted sketch JSON");
    assert_eq!(
        document.to_canonical_json().expect("canonical sketch JSON"),
        SOURCE_ROTATION_SKETCH_JSON,
        "source-rotation sketch fixture must remain canonical"
    );
    let session = retained(document);
    assert_current_accepted(&session);
    let accepted = session
        .accepted_state_for_current_input()
        .expect("source-rotation accepted state");
    let diagnostics = accepted.diagnostics();
    let rank = diagnostics.rank.expect("source-rotation rank diagnostics");
    let mobility = diagnostics
        .mobility
        .expect("source-rotation mobility diagnostics");
    assert_eq!(rank.numerical_rank, Some(0));
    assert_eq!(mobility.equality_degrees_of_freedom, Some(7));
    assert_eq!(mobility.bidirectional_bounded_degrees_of_freedom, Some(7));

    let features = ComputedFeatureDocument::from_json(SOURCE_ROTATION_FEATURE_JSON)
        .expect("source-rotation feature JSON");
    assert_eq!(
        features.to_json().expect("canonical feature JSON"),
        SOURCE_ROTATION_FEATURE_JSON,
        "source-rotation feature fixture must retain its exact digest and revision"
    );
    assert_eq!(features.sketch_document(), accepted.document().id());
    assert_eq!(features.features().len(), 1);
    let feature = &features.features()[0];
    let ComputedFeatureDefinition::FilletSet(fillet) = &feature.definition;
    assert_eq!(fillet.corners.len(), 1);
    let persisted_corner = fillet.corners[0].without_id();
    let corner = fillet.corners[0].id;
    let feature = feature.id;

    let input_fingerprint =
        input_fingerprint(&[SOURCE_ROTATION_SKETCH_JSON, SOURCE_ROTATION_FEATURE_JSON]);
    assert_eq!(input_fingerprint, "input-04658a77db2dc779");

    let design_json = session
        .design_document()
        .to_canonical_json()
        .expect("source-rotation retained design JSON");
    let accepted_identity = accepted.identity();
    let accepted_json = accepted
        .document()
        .to_canonical_json()
        .expect("source-rotation accepted JSON");
    let prepared_input = session.prepared_input();
    let feature_identity = features.identity();
    let feature_json = features.to_json().expect("source-rotation feature JSON");

    let snapshot = evaluate(&session, &features);
    let outcome = validate_source_rotation_snapshot(
        accepted.document(),
        &snapshot,
        feature,
        corner,
        persisted_corner,
    );

    let accepted_after = session
        .accepted_state_for_current_input()
        .expect("evaluation must retain source-rotation accepted state");
    assert_eq!(
        session
            .design_document()
            .to_canonical_json()
            .expect("retained design JSON after evaluation"),
        design_json
    );
    assert_eq!(accepted_after.identity(), accepted_identity);
    assert_eq!(
        accepted_after
            .document()
            .to_canonical_json()
            .expect("accepted JSON after evaluation"),
        accepted_json
    );
    assert_eq!(session.prepared_input(), prepared_input);
    assert_eq!(features.identity(), feature_identity);
    assert_eq!(
        features.to_json().expect("feature JSON after evaluation"),
        feature_json
    );

    Observation {
        input_fingerprint,
        outcome,
    }
}

fn validate_source_rotation_snapshot(
    document: &SketchDocument,
    snapshot: &geosolve_sketch_features::ComputedFeatureSnapshot,
    feature: geosolve_sketch_features::ComputedFeatureId,
    corner: geosolve_sketch_features::ComputedFeatureCornerId,
    persisted: NewComputedFilletCorner,
) -> OracleResult {
    let evaluation = snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == feature)
        .expect("source-rotation feature evaluation");
    let ComputedFeatureEvaluationState::Current { corner_edges } = &evaluation.state else {
        let message = match &evaluation.state {
            ComputedFeatureEvaluationState::Failed {
                failure: ComputedFeatureFailure::NoLocalRoot { .. },
            } => concat!(
                "source rotation left a regular intended line-circle root just beyond the stale ",
                "persisted Local-cell edge, but evaluation returned NoLocalRoot"
            )
            .to_owned(),
            other => format!("source-rotation Fillet evaluation was not Current: {other:?}"),
        };
        return Err(defect("fillet.evaluation.certificate-transport", message));
    };
    if corner_edges.len() != 1 || corner_edges[0].0 != corner {
        return Err(defect(
            "fillet.evaluation.publication",
            "source-rotation Fillet did not publish exactly its one persistent corner",
        ));
    }
    if snapshot
        .edges()
        .iter()
        .filter(|edge| matches!(edge.geometry, ComputedEdgeGeometry::CircularArc(_)))
        .count()
        != 1
    {
        return Err(defect(
            "fillet.evaluation.publication",
            "source-rotation Fillet did not publish exactly one circular arc",
        ));
    }
    let edge = snapshot
        .fillet_arc_edge(ComputedCornerRef { feature, corner })
        .ok_or_else(|| {
            defect(
                "fillet.evaluation.publication",
                "source-rotation Current state had no arc for its stable corner owner",
            )
        })?;
    if edge.id != corner_edges[0].1 {
        return Err(defect(
            "fillet.evaluation.publication",
            "source-rotation corner publication referenced a different generated edge",
        ));
    }
    let ComputedEdgeGeometry::CircularArc(arc) = &edge.geometry else {
        return Err(defect(
            "fillet.evaluation.publication",
            "source-rotation corner edge was not a circular arc",
        ));
    };
    validate_source_rotation_arc(document, persisted, arc)
}

#[allow(
    clippy::too_many_lines,
    reason = "the source-rotation row independently validates branch transport and published geometry"
)]
fn validate_source_rotation_arc(
    document: &SketchDocument,
    persisted: NewComputedFilletCorner,
    arc: &geosolve_sketch_features::ComputedCircularArc,
) -> OracleResult {
    const CIRCLE_TOTAL: f64 = 7.909_322_804_062_922;
    const CIRCLE_PRINCIPAL: f64 = 1.626_137_496_883_336;
    const LINE_PARAMETER: f64 = 0.796_915_905_159_832;
    const EXPECTED_CENTER: [f64; 2] = [-0.017_075_528_971_715, 5.103_423_761_681_947];
    const EXPECTED_TRANSVERSALITY: f64 = -0.527_757_423_204_954_1;

    let ContactNeighborhood::Local {
        lower: stored_lower,
        upper: stored_upper,
    } = persisted.first.neighborhood
    else {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "source-rotation circle parent lost its persisted Local witness",
        ));
    };
    let Some(anchor) = persisted.first.periodic_anchor else {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "source-rotation circle parent lost its periodic anchor",
        ));
    };
    if persisted.first.normal_side != DocumentCurveNormalSide::Right
        || persisted.first.retained_endpoint != DocumentFilletTrimEndpoint::End
        || persisted.first.winding != 1
        || persisted.second.normal_side != DocumentCurveNormalSide::Left
        || persisted.second.retained_endpoint != DocumentFilletTrimEndpoint::Start
        || persisted.second.winding != 0
        || persisted.second.neighborhood != ContactNeighborhood::Interior
        || persisted.second.periodic_anchor.is_some()
        || persisted.endpoint_order != DocumentFilletEndpointOrder::FirstThenSecond
        || persisted.sweep != DocumentArcSweep::CounterClockwise
    {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "source-rotation persisted side, retention, winding, neighborhood, order or sweep changed",
        ));
    }
    let persisted_seed = persisted.first.picked_parameter
        + f64::from(persisted.first.winding) * std::f64::consts::TAU;
    if !(stored_lower < persisted_seed
        && persisted_seed < stored_upper
        && CIRCLE_TOTAL > stored_upper)
    {
        return Err(defect(
            "fillet.evaluation.certificate-transport",
            "source-rotation fixture no longer crosses only the stale persisted cell edge",
        ));
    }

    let support_lower = anchor.parameter + f64::from(anchor.winding) * std::f64::consts::TAU;
    let support_upper = support_lower + std::f64::consts::TAU;
    let seed_cell = document
        .certify_line_curve_fillet_branch_cell(
            persisted.second.source.span,
            persisted.first.source.span,
            persisted_seed,
            support_lower,
            support_upper,
        )
        .map_err(|error| {
            defect(
                "fillet.evaluation.certificate-transport",
                format!("current source geometry could not certify its seed cell: {error}"),
            )
        })?;
    let candidate_cell = document
        .certify_line_curve_fillet_branch_cell(
            persisted.second.source.span,
            persisted.first.source.span,
            CIRCLE_TOTAL,
            support_lower,
            support_upper,
        )
        .map_err(|error| {
            defect(
                "fillet.evaluation.certificate-transport",
                format!(
                    "current source geometry could not certify its regular branch cell: {error}"
                ),
            )
        })?;
    let ContactNeighborhood::Local {
        lower: seed_lower,
        upper: seed_upper,
    } = seed_cell
    else {
        return Err(defect(
            "fillet.evaluation.certificate-transport",
            "current source geometry returned no Local seed certificate",
        ));
    };
    let ContactNeighborhood::Local {
        lower: candidate_lower,
        upper: candidate_upper,
    } = candidate_cell
    else {
        return Err(defect(
            "fillet.evaluation.certificate-transport",
            "current source geometry returned no Local candidate certificate",
        ));
    };
    if !(seed_lower < persisted_seed
        && persisted_seed < seed_upper
        && candidate_lower < CIRCLE_TOTAL
        && CIRCLE_TOTAL < candidate_upper
        && stored_lower.max(seed_lower) < stored_upper.min(seed_upper)
        && seed_lower.max(candidate_lower) < seed_upper.min(candidate_upper))
    {
        return Err(defect(
            "fillet.evaluation.certificate-transport",
            format!(
                "persisted seed {persisted_seed:.15} and intended root {CIRCLE_TOTAL:.15} are not connected by stored [{stored_lower:.15}, {stored_upper:.15}], seed [{seed_lower:.15}, {seed_upper:.15}] and candidate [{candidate_lower:.15}, {candidate_upper:.15}] regular cells"
            ),
        ));
    }

    if !arc.center.into_iter().all(f64::is_finite)
        || !arc.radius.is_finite()
        || !arc.start_angle.is_finite()
        || !arc.end_angle.is_finite()
        || arc.contacts.iter().any(|contact| {
            !contact.parameter.is_finite()
                || !contact.total_parameter.is_finite()
                || !contact.position.into_iter().all(f64::is_finite)
        })
    {
        return Err(defect(
            "fillet.evaluation.invalid-geometry",
            "source-rotation Fillet published non-finite geometry",
        ));
    }
    if (arc.radius - 1.0).abs() > 1.0e-12
        || arc.sweep != DocumentArcSweep::CounterClockwise
        || (arc.center[0] - EXPECTED_CENTER[0]).abs() > 2.0e-9
        || (arc.center[1] - EXPECTED_CENTER[1]).abs() > 2.0e-9
    {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "source-rotation Fillet changed radius, sweep or intended local root center",
        ));
    }
    if arc.contacts[0].source != persisted.first.source
        || arc.contacts[1].source != persisted.second.source
        || (arc.contacts[0].total_parameter - CIRCLE_TOTAL).abs() > 2.0e-9
        || (arc.contacts[0].parameter - CIRCLE_PRINCIPAL).abs() > 2.0e-9
        || arc.contacts[0].winding != 1
        || (arc.contacts[1].total_parameter - LINE_PARAMETER).abs() > 2.0e-9
        || (arc.contacts[1].parameter - LINE_PARAMETER).abs() > 2.0e-9
        || arc.contacts[1].winding != 0
    {
        return Err(defect(
            "fillet.evaluation.branch-state",
            "source-rotation Fillet selected a different source, contact root or winding",
        ));
    }

    let mut unit_tangents = [[0.0; 2]; 2];
    for (index, (parent, contact)) in [persisted.first, persisted.second]
        .into_iter()
        .zip(arc.contacts)
        .enumerate()
    {
        let jet = document
            .evaluate_curve_jet(parent.source.span, contact.total_parameter)
            .map_err(|error| {
                defect(
                    "fillet.evaluation.invalid-geometry",
                    format!("source-rotation contact jet failed: {error}"),
                )
            })?;
        let tangent_length = jet.first_derivative.x.hypot(jet.first_derivative.y);
        if !tangent_length.is_finite() || tangent_length <= 0.0 {
            return Err(defect(
                "fillet.evaluation.invalid-geometry",
                "source-rotation contact has no finite regular tangent",
            ));
        }
        unit_tangents[index] = [
            jet.first_derivative.x / tangent_length,
            jet.first_derivative.y / tangent_length,
        ];
        let position_error =
            (jet.position.x - contact.position[0]).hypot(jet.position.y - contact.position[1]);
        let radial = [
            arc.center[0] - contact.position[0],
            arc.center[1] - contact.position[1],
        ];
        let radial_length = radial[0].hypot(radial[1]);
        let normalized_tangency =
            (unit_tangents[index][0] * radial[0] + unit_tangents[index][1] * radial[1]).abs()
                / radial_length;
        let signed_offset =
            radial[0] * -unit_tangents[index][1] + radial[1] * unit_tangents[index][0];
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
                "source-rotation Fillet failed incidence, radius, tangency or signed-side validation",
            ));
        }
    }
    let transversality =
        unit_tangents[0][0] * unit_tangents[1][1] - unit_tangents[0][1] * unit_tangents[1][0];
    if transversality >= -0.5 || (transversality - EXPECTED_TRANSVERSALITY).abs() > 2.0e-9 {
        return Err(defect(
            "fillet.evaluation.certificate-transport",
            "source-rotation intended root is no longer regular on its retained orientation branch",
        ));
    }

    let (start, end) = match persisted.endpoint_order {
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
                "source-rotation Fillet endpoint angles changed explicit endpoint order",
            ));
        }
    }
    Ok(())
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
