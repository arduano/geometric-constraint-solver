// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg(not(target_arch = "wasm32"))]

use std::env;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::panic::{AssertUnwindSafe, catch_unwind};

use geosolve_constraint_editor::{
    OffsetAuthoringApplyEffect, OffsetAuthoringOutcome, OffsetAuthoringRoute, OffsetAuthoringState,
    OffsetAuthoringTarget, RetainedEditorCoordinator, SelectionItem,
};
use geosolve_sketch::{
    CurveDefinition, CurveOffsetGeometry, CurveSpan, DocumentId, DocumentSolveRequest,
    PersistentId, RetainedSketchDocumentSession, SketchDocument, SketchHardValidity, SolverConfig,
};
use geosolve_sketch_features::{
    ComputedEdgeGeometry, ComputedEdgeProvenance, ComputedFeatureEvaluationState,
};

const FAMILY: &str = "feature.curve-offset";
const CASE_ID: &str = "feature.curve-offset.authoring.general-open-chain";
const TSV_HEADER: &str = "case_id\tfamily\tstatus\tfinding_id\tfailure_class\tfingerprint";

#[derive(Clone, Debug)]
struct SemanticDefect {
    class: &'static str,
    message: String,
}

#[derive(Clone, Debug)]
struct Observation {
    input_fingerprint: String,
    outcome: Result<(), SemanticDefect>,
}

fn defect(class: &'static str, message: impl Into<String>) -> SemanticDefect {
    SemanticDefect {
        class,
        message: message.into(),
    }
}

#[test]
fn golden_curve_offset_oracle_inventory_and_tsv_schema_are_exhaustive() {
    assert!(CASE_ID.starts_with(FAMILY));
    assert_eq!(TSV_HEADER.split('\t').count(), 6);
}

#[test]
fn golden_curve_offset_oracle_survey() {
    let selected = env::var("GEOSOLVE_GOLDEN_ORACLE_CASE");
    let output = env::var("GEOSOLVE_GOLDEN_ORACLE_OUTPUT");
    if selected.is_err() && output.is_err() {
        return;
    }
    let selected = selected.expect("GEOSOLVE_GOLDEN_ORACLE_CASE must accompany oracle output");
    let output = output.expect("GEOSOLVE_GOLDEN_ORACLE_OUTPUT must accompany oracle case");
    assert_eq!(selected, CASE_ID, "unknown Curve Offset oracle case");

    let row = match catch_unwind(AssertUnwindSafe(observe)) {
        Ok(observation) => match &observation.outcome {
            Ok(()) => format!(
                "{CASE_ID}\t{FAMILY}\tPASS\t-\t-\t{}",
                observation.input_fingerprint
            ),
            Err(failure) => {
                let detail = format!(
                    "input={}; {}",
                    observation.input_fingerprint,
                    sanitize_tsv(&failure.message)
                );
                format!(
                    "{CASE_ID}\t{FAMILY}\tDEFECT\t-\t{}\t{:016x}:{detail}",
                    failure.class,
                    fnv1a64(detail.as_bytes())
                )
            }
        },
        Err(payload) => {
            let detail = sanitize_tsv(&panic_payload(&payload));
            format!(
                "{CASE_ID}\t{FAMILY}\tPANIC\t-\ttest-panic\t{:016x}:{detail}",
                fnv1a64(detail.as_bytes())
            )
        }
    };

    let file = File::create(&output)
        .unwrap_or_else(|error| panic!("cannot create Curve Offset oracle TSV {output}: {error}"));
    let mut output = BufWriter::new(file);
    writeln!(output, "{TSV_HEADER}").expect("write Curve Offset oracle header");
    writeln!(output, "{row}").expect("write Curve Offset oracle row");
    output.flush().expect("flush Curve Offset oracle row");
}

#[allow(
    clippy::too_many_lines,
    reason = "one compact golden row keeps the public Curve Offset route and exact current authority reviewable together"
)]
fn observe() -> Observation {
    let mut document = SketchDocument::with_id(
        10.0,
        DocumentId(PersistentId::from_u128(
            0x676f_6c64_656e_5f63_7572_7665_0000_0001,
        )),
    )
    .expect("golden Curve Offset document");
    let controls = [
        document.add_point("start", [0.0, 0.0]).expect("start"),
        document.add_point("control", [2.0, 1.0]).expect("control"),
        document.add_point("end", [4.0, 0.0]).expect("end"),
    ];
    let span = CurveSpan::line(
        document
            .add_curve(
                "quadratic source",
                CurveDefinition::QuadraticBezier { controls },
            )
            .expect("quadratic source"),
    );
    let design_json = document
        .to_canonical_json()
        .expect("Curve Offset design JSON");
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("golden Curve Offset accepted session");
    assert_current_accepted(&session);
    let accepted_json = session
        .export_accepted_json()
        .expect("export Curve Offset accepted JSON")
        .expect("Curve Offset accepted JSON");
    let input_fingerprint = input_fingerprint(&[
        &design_json,
        &accepted_json,
        "quadratic-open-chain:left:0.2",
    ]);
    let mut coordinator =
        RetainedEditorCoordinator::new(session).expect("Curve Offset coordinator");
    let base_features = coordinator
        .feature_document()
        .to_json()
        .expect("empty feature JSON");
    let base_history = (coordinator.history_len(), coordinator.history_cursor());

    let mut state = OffsetAuthoringState::default();
    if !matches!(
        coordinator.activate_offset_authoring(&mut state),
        Ok(OffsetAuthoringOutcome::ModeEntered(_))
    ) || !matches!(
        state.pick_target(OffsetAuthoringTarget::Span(span)),
        OffsetAuthoringOutcome::OperandChanged { .. }
    ) || !matches!(
        state.set_distance(0.2),
        OffsetAuthoringOutcome::DistanceChanged { .. }
    ) {
        return Observation {
            input_fingerprint,
            outcome: Err(defect(
                "curve-offset.authoring.collection",
                "ordinary Offset authoring did not collect the general open chain",
            )),
        };
    }
    let Some(candidate) = state.candidate() else {
        return Observation {
            input_fingerprint,
            outcome: Err(defect(
                "curve-offset.authoring.route",
                "complete general open chain did not produce a candidate",
            )),
        };
    };
    if candidate.route != OffsetAuthoringRoute::ComputedCurve
        || candidate.input != coordinator.session().prepared_input()
    {
        return Observation {
            input_fingerprint,
            outcome: Err(defect(
                "curve-offset.authoring.route",
                "general open chain did not select the authenticated computed route",
            )),
        };
    }

    let preview = match coordinator.prepare_offset_authoring_preview(&state, "Curve Offset") {
        Ok(preview) => preview,
        Err(error) => {
            return Observation {
                input_fingerprint,
                outcome: Err(defect(
                    "curve-offset.authoring.preview",
                    format!("computed preview was unavailable: {error}"),
                )),
            };
        }
    };
    let Some(metadata) = preview.computed_curve() else {
        return Observation {
            input_fingerprint,
            outcome: Err(defect(
                "curve-offset.authoring.route",
                "computed candidate published native preview metadata",
            )),
        };
    };
    let feature = metadata.feature;
    let preview_edges = metadata.generated_edges.clone();
    if metadata.source_spans != [span]
        || preview_edges.is_empty()
        || coordinator.feature_document().to_json().ok().as_deref() != Some(&base_features)
        || (coordinator.history_len(), coordinator.history_cursor()) != base_history
    {
        return Observation {
            input_fingerprint,
            outcome: Err(defect(
                "curve-offset.authoring.preview-authority",
                "preview did not remain provisional with complete source/output authority",
            )),
        };
    }
    if let Err(failure) = validate_current_output(&coordinator, feature, &preview_edges) {
        return Observation {
            input_fingerprint,
            outcome: Err(failure),
        };
    }

    let applied = match coordinator.apply_offset_authoring_preview(&mut state) {
        Ok(applied) => applied,
        Err(error) => {
            return Observation {
                input_fingerprint,
                outcome: Err(defect(
                    "curve-offset.authoring.publication",
                    format!("exact preview did not publish: {error}"),
                )),
            };
        }
    };
    if applied.value != OffsetAuthoringApplyEffect::ComputedCurve(feature)
        || (coordinator.history_len(), coordinator.history_cursor())
            != (base_history.0 + 1, base_history.1 + 1)
        || coordinator.transcript().len() != 1
        || coordinator.session().export_design_json().ok().as_deref() != Some(&design_json)
        || coordinator
            .session()
            .export_accepted_json()
            .ok()
            .flatten()
            .as_deref()
            != Some(accepted_json.as_str())
        || !coordinator
            .feature_document()
            .to_json()
            .is_ok_and(|json| json.contains("\"version\":2"))
    {
        return Observation {
            input_fingerprint,
            outcome: Err(defect(
                "curve-offset.authoring.publication",
                "Apply did not publish exactly one v2 computed feature history step over unchanged native geometry",
            )),
        };
    }
    let outcome = validate_current_output(&coordinator, feature, &preview_edges);
    Observation {
        input_fingerprint,
        outcome,
    }
}

fn validate_current_output(
    coordinator: &RetainedEditorCoordinator,
    feature: geosolve_sketch_features::ComputedFeatureId,
    expected_edges: &[geosolve_sketch_features::ComputedEdgeId],
) -> Result<(), SemanticDefect> {
    let snapshot = coordinator.computed_snapshot().ok_or_else(|| {
        defect(
            "curve-offset.authoring.current-authority",
            "current computed snapshot is absent",
        )
    })?;
    let evaluation = snapshot
        .feature_evaluations()
        .iter()
        .find(|evaluation| evaluation.feature == feature)
        .ok_or_else(|| {
            defect(
                "curve-offset.authoring.current-authority",
                "feature-local evaluation is absent",
            )
        })?;
    let ComputedFeatureEvaluationState::Current {
        corner_edges,
        generated_edges,
    } = &evaluation.state
    else {
        return Err(defect(
            "curve-offset.authoring.current-authority",
            format!("feature was not Current: {:?}", evaluation.state),
        ));
    };
    if !corner_edges.is_empty() || generated_edges != expected_edges {
        return Err(defect(
            "curve-offset.authoring.current-authority",
            "Current output did not retain the exact generated-edge set",
        ));
    }
    for edge_id in generated_edges {
        let edge = snapshot.edge(*edge_id).ok_or_else(|| {
            defect(
                "curve-offset.authoring.current-authority",
                "generated edge identity did not resolve in its owning snapshot",
            )
        })?;
        let ComputedEdgeProvenance::CurveOffset { owner, .. } = edge.provenance else {
            return Err(defect(
                "curve-offset.authoring.provenance",
                "generated edge did not carry Curve Offset provenance",
            ));
        };
        let ComputedEdgeGeometry::CurveOffset(CurveOffsetGeometry::CubicPatches(patches)) =
            &edge.geometry
        else {
            return Err(defect(
                "curve-offset.authoring.geometry",
                "general source did not publish certified cubic output",
            ));
        };
        if owner != feature
            || patches.is_empty()
            || !patches.iter().all(|patch| {
                patch
                    .source_parameters
                    .iter()
                    .chain(patch.controls.iter().flatten())
                    .all(|value| value.is_finite())
            })
            || coordinator.selection_for_computed_edge(*edge_id)
                != Some(SelectionItem::Feature(feature))
        {
            return Err(defect(
                "curve-offset.authoring.geometry",
                "generated cubic output was non-finite or lost stable feature ownership",
            ));
        }
    }
    Ok(())
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
    let solve = accepted.diagnostics().solve.expect("solve diagnostics");
    assert_eq!(solve.hard_validity, SketchHardValidity::Valid);
    assert!(solve.hard_residuals_validated);
    assert!(
        solve
            .maximum_normalized_hard_residual
            .is_some_and(|residual| residual <= 1.0e-9)
    );
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
