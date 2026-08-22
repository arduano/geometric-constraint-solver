// SPDX-License-Identifier: GPL-3.0-or-later

//! Inventory-driven lineage lifecycle proof for the complete M83-W2/W3
//! semantic surface.
//!
//! Owning-domain geometry and equation behavior remains covered by the sketch
//! and editor suites. This test deliberately targets the presentation-free
//! lineage contract: every frozen family must survive the same typed action
//! lifecycle without losing its semantic payload or stable output manifest.

use std::collections::{BTreeMap, BTreeSet};

use geosolve_constraint_editor::{
    CoordinatorError, CurveNumericPropertyKind, DisabledReason, RetainedEditorCoordinator,
    SelectionItem, evaluate_lineage_session_cold_with_inputs,
};
use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentBSplineForm, DocumentConstraintDefinition,
    DocumentCoordinateAxis, DocumentCurveControlAvailability, DocumentCurveControlKind,
    DocumentCurveControlWithholdingReason, DocumentDirectionSense, DocumentEdit,
    DocumentLineSupportRef, DocumentParameterKind, DocumentParameterTarget,
    DocumentRationalConicControl, DocumentScalarBranch, DocumentScalarPropertyRef,
    DocumentScalarUnit, DocumentSolveRequest, ExternalSnapshotSet, GeometryRole,
    MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT, ParameterBatch, ParameterBatchEntry, ParameterValue,
    RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDatum, SketchDocument,
    SolverConfig,
};
use geosolve_sketch_lineage::{
    LineageActionDefinition, LineageDeveloperKey, LineageDocument, LineageDocumentError,
    LineageDocumentId, LineageInputBinding, LineageMutation, LineageOpaqueId, LineageOutput,
    LineageOutputId, LineageOutputKind, LineageOutputRef, LineagePatch, LineageReservation,
    LineageReservationId, LineageReservationKind, LineageSemanticKey, LineageSession, LineageStep,
    LineageStepId, LineageStepRewrite, LineageStepState, VersionedActionPayload,
};
use serde_json::{Value, json};

const CATALOG: &str = include_str!("fixtures/m83_lineage_action_catalog.golden.tsv");
const MAX_SEMANTIC_PORT_KEY_BYTES: usize = 256;

fn semantic(value: impl Into<String>) -> LineageSemanticKey {
    LineageSemanticKey::new(value.into()).expect("test semantic key")
}

fn developer(value: impl Into<String>) -> LineageDeveloperKey {
    LineageDeveloperKey::new(value.into()).expect("test developer key")
}

fn opaque(value: impl Into<String>) -> LineageOpaqueId {
    LineageOpaqueId::new(value.into()).expect("test opaque identity")
}

fn catalog_keys(category: &str) -> Vec<&'static str> {
    CATALOG
        .lines()
        .skip(1)
        .filter_map(|row| {
            let (row_category, key) = row.split_once('\t')?;
            (row_category == category).then_some(key)
        })
        .collect()
}

#[derive(Clone, Copy)]
enum ActionCategory {
    Constraint,
    Dimension,
    Geometry,
    Binding,
}

fn action(
    category: ActionCategory,
    schema: impl Into<String>,
    inputs: Vec<LineageInputBinding>,
    parameters: BTreeMap<String, Value>,
) -> LineageActionDefinition {
    let action = VersionedActionPayload {
        schema: semantic(schema),
        version: 1,
        inputs,
        parameters,
    };
    match category {
        ActionCategory::Constraint => LineageActionDefinition::Constraint { action },
        ActionCategory::Dimension => LineageActionDefinition::Dimension { action },
        ActionCategory::Geometry => LineageActionDefinition::GeometryRecipe { action },
        ActionCategory::Binding => LineageActionDefinition::Binding { action },
    }
}

#[derive(Default)]
struct Allocator {
    step: u64,
    output: u64,
    reservation: u64,
}

impl Allocator {
    fn new() -> Self {
        Self {
            step: 1,
            output: 1,
            reservation: 1,
        }
    }

    fn next_step(&mut self) -> LineageStepId {
        let id = LineageStepId::from_raw(self.step);
        self.step += 1;
        id
    }

    fn output(
        &mut self,
        key: &str,
        kind: LineageOutputKind,
        reservation_kind: Option<LineageReservationKind>,
    ) -> (LineageOutput, Option<LineageReservation>) {
        let output = LineageOutputId::from_raw(self.output);
        self.output += 1;
        let reservation = reservation_kind.map(|kind| {
            let id = LineageReservationId::from_raw(self.reservation);
            self.reservation += 1;
            LineageReservation {
                id,
                key: semantic(key),
                kind,
                persistent_id: opaque(format!("m83-native-{}", id.raw())),
            }
        });
        (
            LineageOutput {
                id: output,
                key: semantic(key),
                kind,
                reservation: reservation.as_ref().map(|value| value.id),
            },
            reservation,
        )
    }
}

fn family_step(
    allocator: &mut Allocator,
    category: ActionCategory,
    family_category: &str,
    key: &str,
    mode: Option<&str>,
    outputs: &[(LineageOutputKind, Option<LineageReservationKind>, &str)],
) -> LineageStep {
    let mut parameters = BTreeMap::new();
    parameters.insert("family".into(), json!(key));
    parameters.insert("mode".into(), json!(mode));
    parameters.insert("edit_generation".into(), json!(0));
    let mut lineage_outputs = Vec::new();
    let mut reservations = Vec::new();
    for (output_kind, reservation_kind, output_key) in outputs {
        let (output, reservation) = allocator.output(output_key, *output_kind, *reservation_kind);
        lineage_outputs.push(output);
        reservations.extend(reservation);
    }
    let suffix = mode.map_or_else(String::new, |mode| format!("-{mode}"));
    let identity_key = format!("m83-{family_category}-{key}{suffix}");
    LineageStep::new(
        allocator.next_step(),
        developer(identity_key),
        format!("{family_category} {key}{suffix}"),
        action(
            category,
            format!("geosolve.catalog-sentinel.v1.{family_category}.{key}"),
            Vec::new(),
            parameters,
        ),
        lineage_outputs,
        reservations,
    )
}

fn rewrite_generation(step: &LineageStep, generation: u64) -> LineageStepRewrite {
    let mut replacement = step.action.clone();
    let parameters = match &mut replacement {
        LineageActionDefinition::Constraint { action }
        | LineageActionDefinition::Dimension { action }
        | LineageActionDefinition::GeometryRecipe { action }
        | LineageActionDefinition::Binding { action } => &mut action.parameters,
        _ => panic!("unexpected action category in W2/W3 fixture"),
    };
    parameters.insert("edit_generation".into(), json!(generation));
    LineageStepRewrite {
        label: step.label.clone(),
        action: replacement,
    }
}

fn insert_all(session: &mut LineageSession, steps: Vec<LineageStep>) {
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            steps
                .into_iter()
                .map(|step| LineageMutation::Insert {
                    before: None,
                    step: Box::new(step),
                })
                .collect::<Vec<_>>(),
        ))
        .expect("insert complete family inventory");
}

fn round_trip_session(session: &LineageSession) {
    let canonical = session
        .to_canonical_session_json()
        .expect("canonical lineage session");
    let restored = LineageSession::from_session_json(&canonical).expect("restore lineage session");
    assert_eq!(
        restored
            .to_canonical_session_json()
            .expect("restored canonical lineage session"),
        canonical
    );
}

fn undo(session: &mut LineageSession) {
    assert!(session.undo().expect("Undo family lifecycle").is_some());
}

fn redo(session: &mut LineageSession) {
    assert!(session.redo().expect("Redo family lifecycle").is_some());
}

fn relation_dimension_cases() -> Vec<(
    ActionCategory,
    &'static str,
    &'static str,
    Option<&'static str>,
)> {
    let mut cases = catalog_keys("constraint")
        .into_iter()
        .map(|key| (ActionCategory::Constraint, "constraint", key, None))
        .collect::<Vec<_>>();
    for key in catalog_keys("dimension") {
        for mode in ["driving", "reference"] {
            cases.push((ActionCategory::Dimension, "dimension", key, Some(mode)));
        }
    }
    cases
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one lifecycle matrix keeps the complete W2 catalog and exact history states adjacent"
)]
fn m83_w2_every_frozen_relation_and_both_dimension_modes_share_the_exact_lineage_lifecycle() {
    assert_eq!(catalog_keys("constraint").len(), 35);
    assert_eq!(catalog_keys("dimension").len(), 8);

    let mut allocator = Allocator::new();
    let mut steps = Vec::new();
    for (category, family_category, key, mode) in relation_dimension_cases() {
        let outputs = match category {
            ActionCategory::Constraint => vec![
                (
                    LineageOutputKind::Constraint,
                    Some(LineageReservationKind::Constraint),
                    "constraint",
                ),
                (
                    LineageOutputKind::Source,
                    Some(LineageReservationKind::Source),
                    "source",
                ),
                (
                    LineageOutputKind::Annotation,
                    Some(LineageReservationKind::Annotation),
                    "annotation-key",
                ),
            ],
            ActionCategory::Dimension => vec![
                (
                    LineageOutputKind::Dimension,
                    Some(LineageReservationKind::Dimension),
                    "dimension",
                ),
                (
                    LineageOutputKind::Source,
                    Some(LineageReservationKind::Source),
                    "source",
                ),
                (
                    LineageOutputKind::Scalar,
                    Some(LineageReservationKind::Scalar),
                    "target-scalar",
                ),
                (
                    LineageOutputKind::Annotation,
                    Some(LineageReservationKind::Annotation),
                    "annotation-key",
                ),
            ],
            _ => unreachable!("W2 cases contain only constraints and dimensions"),
        };
        steps.push(family_step(
            &mut allocator,
            category,
            family_category,
            key,
            mode,
            &outputs,
        ));
    }
    assert_eq!(steps.len(), 35 + 2 * 8);

    let mut session = LineageSession::new(LineageDocument::with_id(LineageDocumentId::from_raw(
        0x83_0203,
    )))
    .expect("lineage session");
    insert_all(&mut session, steps);
    let created = session.document().steps().to_vec();
    let stable_manifests = created
        .iter()
        .map(|step| {
            (
                step.id,
                step.outputs.clone(),
                step.output_identities.clone(),
                step.reservations.clone(),
            )
        })
        .collect::<Vec<_>>();
    let allocator_after_create = session.document().allocator_high_water();
    round_trip_session(&session);

    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            session
                .document()
                .steps()
                .iter()
                .map(|step| LineageMutation::Rewrite {
                    step: step.id,
                    replacement: Box::new(rewrite_generation(step, 1)),
                })
                .collect::<Vec<_>>(),
        ))
        .expect("edit every W2 owner");
    let edited = session.document().steps().to_vec();
    assert_eq!(
        session.document().allocator_high_water(),
        allocator_after_create
    );
    assert_eq!(
        edited
            .iter()
            .map(|step| (
                step.id,
                step.outputs.clone(),
                step.output_identities.clone(),
                step.reservations.clone(),
            ))
            .collect::<Vec<_>>(),
        stable_manifests,
        "source IDs, target scalars, annotation keys, and logical owners must not change on edit"
    );

    let step_ids = session
        .document()
        .steps()
        .iter()
        .map(|step| step.id)
        .collect::<Vec<_>>();
    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            step_ids
                .iter()
                .map(|step| LineageMutation::SetSuppressed {
                    step: *step,
                    suppressed: true,
                })
                .collect::<Vec<_>>(),
        ))
        .expect("suppress every W2 owner");
    let suppressed = session.document().steps().to_vec();
    assert!(
        suppressed
            .iter()
            .all(|step| step.state == LineageStepState::Suppressed)
    );
    round_trip_session(&session);

    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            step_ids
                .iter()
                .map(|step| LineageMutation::SetSuppressed {
                    step: *step,
                    suppressed: false,
                })
                .collect::<Vec<_>>(),
        ))
        .expect("unsuppress every W2 owner");
    let unsuppressed = session.document().steps().to_vec();
    assert_eq!(unsuppressed, edited);

    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            step_ids
                .iter()
                .map(|step| LineageMutation::Tombstone { step: *step })
                .collect::<Vec<_>>(),
        ))
        .expect("delete every W2 owner");
    let deleted = session.document().steps().to_vec();
    assert!(
        deleted
            .iter()
            .all(|step| step.state == LineageStepState::Tombstoned)
    );
    round_trip_session(&session);

    undo(&mut session);
    assert_eq!(session.document().steps(), unsuppressed);
    undo(&mut session);
    assert_eq!(session.document().steps(), suppressed);
    undo(&mut session);
    assert_eq!(session.document().steps(), edited);
    undo(&mut session);
    assert_eq!(session.document().steps(), created);
    undo(&mut session);
    assert!(session.document().steps().is_empty());

    redo(&mut session);
    assert_eq!(session.document().steps(), created);
    redo(&mut session);
    assert_eq!(session.document().steps(), edited);
    redo(&mut session);
    assert_eq!(session.document().steps(), suppressed);
    redo(&mut session);
    assert_eq!(session.document().steps(), unsuppressed);
    redo(&mut session);
    assert_eq!(session.document().steps(), deleted);
    assert_eq!(
        session.document().allocator_high_water(),
        allocator_after_create
    );
    round_trip_session(&session);
}

fn provider_document() -> LineageDocument {
    let mut allocator = Allocator::new();
    let (curve, reservation) = allocator.output(
        "curve",
        LineageOutputKind::Curve,
        Some(LineageReservationKind::Curve),
    );
    let provider = LineageStep::new(
        allocator.next_step(),
        developer("provider"),
        "provider",
        action(
            ActionCategory::Geometry,
            "geosolve.geometry.v1.provider",
            Vec::new(),
            BTreeMap::new(),
        ),
        vec![curve],
        reservation.into_iter().collect(),
    );
    let mut document = LineageDocument::with_id(LineageDocumentId::from_raw(0x83_0204));
    document
        .apply_patch(LineagePatch::new(
            document.identity(),
            vec![LineageMutation::Insert {
                before: None,
                step: Box::new(provider),
            }],
        ))
        .expect("provider");
    document
}

#[test]
fn m83_w2_wrong_kind_input_rejects_atomically_for_every_frozen_family_and_mode() {
    let provider = provider_document();
    let provider_output = provider.steps()[0].outputs[0].id;
    for (case_index, (category, family_category, key, mode)) in
        relation_dimension_cases().into_iter().enumerate()
    {
        let mut document = provider.clone();
        let before = document.to_canonical_json().expect("before rejection");
        let source = LineageOutputRef {
            document: document.id(),
            step: document.steps()[0].id,
            output: provider_output,
            // Both the binding and reference agree that this is a point; the
            // provider's stable port is a Curve, so structural validation must
            // reject before changing the document.
            kind: LineageOutputKind::Point,
        };
        let input = LineageInputBinding {
            key: semantic("operand"),
            kind: LineageOutputKind::Point,
            source,
        };
        // Each case starts from the same cloned one-step document, so the only
        // admissible next IDs are exactly 2. Varying the IDs here would test an
        // allocator gap before reaching the intended wrong-kind sentinel.
        let raw = 2;
        let output_kind = match category {
            ActionCategory::Constraint => LineageOutputKind::Constraint,
            ActionCategory::Dimension => LineageOutputKind::Dimension,
            _ => unreachable!("W2 cases contain only constraints and dimensions"),
        };
        let reservation_kind = match category {
            ActionCategory::Constraint => LineageReservationKind::Constraint,
            ActionCategory::Dimension => LineageReservationKind::Dimension,
            _ => unreachable!("W2 cases contain only constraints and dimensions"),
        };
        let mut parameters = BTreeMap::new();
        parameters.insert("mode".into(), json!(mode));
        let candidate = LineageStep::new(
            LineageStepId::from_raw(raw),
            developer(format!("wrong-kind-{family_category}-{key}-{case_index}")),
            "wrong kind",
            action(
                category,
                format!("geosolve.catalog-sentinel.v1.{family_category}.{key}"),
                vec![input],
                parameters,
            ),
            vec![LineageOutput {
                id: LineageOutputId::from_raw(raw),
                key: semantic("result"),
                kind: output_kind,
                reservation: Some(LineageReservationId::from_raw(raw)),
            }],
            vec![LineageReservation {
                id: LineageReservationId::from_raw(raw),
                key: semantic("result"),
                kind: reservation_kind,
                persistent_id: opaque(format!("wrong-kind-result-{case_index}")),
            }],
        );
        assert!(matches!(
            document.apply_patch(LineagePatch::new(
                document.identity(),
                vec![LineageMutation::Insert {
                    before: None,
                    step: Box::new(candidate),
                }],
            )),
            Err(LineageDocumentError::WrongOutputKind { .. })
        ));
        assert_eq!(
            document.to_canonical_json().expect("after rejection"),
            before,
            "wrong-kind rejection must be neutral for {family_category}.{key} {mode:?}"
        );
    }
}

fn w3_owner_fields() -> Vec<(&'static str, &'static str)> {
    ["curve-control", "curve-property", "geometry-role", "branch"]
        .into_iter()
        .flat_map(|category| {
            catalog_keys(category)
                .into_iter()
                .map(move |key| (category, key))
        })
        .chain(std::iter::once(("activation", "host-activation")))
        .collect()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one owner-field matrix keeps every W3 family, rewrite, and reload invariant adjacent"
)]
fn m83_w3_every_frozen_control_property_role_activation_and_branch_rewrites_its_stable_owner() {
    assert_eq!(catalog_keys("curve-control").len(), 14);
    assert_eq!(catalog_keys("curve-property").len(), 7);
    assert_eq!(catalog_keys("geometry-role").len(), 2);
    assert_eq!(catalog_keys("branch").len(), 27);

    let mut allocator = Allocator::new();
    let mut steps = Vec::new();
    for (category, key) in w3_owner_fields() {
        let (action_category, output_kind) = match category {
            "geometry-role" => (ActionCategory::Binding, LineageOutputKind::GeometryRole),
            "activation" => (ActionCategory::Binding, LineageOutputKind::Activation),
            _ => (ActionCategory::Geometry, LineageOutputKind::Collection),
        };
        steps.push(family_step(
            &mut allocator,
            action_category,
            category,
            key,
            None,
            &[(output_kind, None, "owner-field")],
        ));
    }

    let mut session = LineageSession::new(LineageDocument::with_id(LineageDocumentId::from_raw(
        0x83_0301,
    )))
    .expect("lineage session");
    insert_all(&mut session, steps);
    let authored = session.document().steps().to_vec();
    let allocator_after_create = session.document().allocator_high_water();
    let stable_manifests = authored
        .iter()
        .map(|step| {
            (
                step.id,
                step.outputs.clone(),
                step.output_identities.clone(),
                step.reservations.clone(),
            )
        })
        .collect::<Vec<_>>();

    session
        .apply_patch(LineagePatch::new(
            session.identity(),
            session
                .document()
                .steps()
                .iter()
                .map(|step| LineageMutation::Rewrite {
                    step: step.id,
                    replacement: Box::new(rewrite_generation(step, 1)),
                })
                .collect::<Vec<_>>(),
        ))
        .expect("rewrite every W3 owner field");
    let rewritten = session.document().steps().to_vec();
    assert_eq!(
        session.document().allocator_high_water(),
        allocator_after_create
    );
    assert_eq!(
        rewritten
            .iter()
            .map(|step| (
                step.id,
                step.outputs.clone(),
                step.output_identities.clone(),
                step.reservations.clone(),
            ))
            .collect::<Vec<_>>(),
        stable_manifests,
        "owner rewrites may not replace their stable typed ports"
    );
    assert!(rewritten.iter().all(|step| {
        let parameters = match &step.action {
            LineageActionDefinition::Constraint { action }
            | LineageActionDefinition::Dimension { action }
            | LineageActionDefinition::GeometryRecipe { action }
            | LineageActionDefinition::Binding { action } => &action.parameters,
            _ => return false,
        };
        parameters.get("edit_generation") == Some(&json!(1))
    }));
    round_trip_session(&session);

    undo(&mut session);
    assert_eq!(session.document().steps(), authored);
    redo(&mut session);
    assert_eq!(session.document().steps(), rewritten);
    assert_eq!(
        session.document().allocator_high_water(),
        allocator_after_create
    );
    round_trip_session(&session);
}

#[test]
fn m83_w3_control_property_and_role_catalog_rows_are_tied_to_public_typed_values() {
    let controls = [
        DocumentCurveControlKind::Center,
        DocumentCurveControlKind::StartPoint,
        DocumentCurveControlKind::EndPoint,
        DocumentCurveControlKind::ControlPoint { ordinal: 0 },
        DocumentCurveControlKind::Radius,
        DocumentCurveControlKind::TrimStart,
        DocumentCurveControlKind::TrimEnd,
        DocumentCurveControlKind::MajorAxisPoint,
        DocumentCurveControlKind::MinorAxis,
        DocumentCurveControlKind::RationalMiddle,
        DocumentCurveControlKind::Vertex,
        DocumentCurveControlKind::Focus,
        DocumentCurveControlKind::TransverseAxisPoint,
        DocumentCurveControlKind::ConjugateAxis,
    ];
    assert_eq!(
        controls
            .map(DocumentCurveControlKind::semantic_key)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        catalog_keys("curve-control").into_iter().collect()
    );

    let properties = [
        CurveNumericPropertyKind::Radius,
        CurveNumericPropertyKind::MinorAxisRatio,
        CurveNumericPropertyKind::TrimStart,
        CurveNumericPropertyKind::TrimEnd,
        CurveNumericPropertyKind::SemiConjugate,
        CurveNumericPropertyKind::RationalWeight,
        CurveNumericPropertyKind::NurbsWeight { ordinal: 0 },
    ];
    assert_eq!(
        properties
            .map(CurveNumericPropertyKind::semantic_key)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        catalog_keys("curve-property").into_iter().collect()
    );

    let roles = [GeometryRole::Profile, GeometryRole::Construction].map(|role| match role {
        GeometryRole::Profile => "profile",
        GeometryRole::Construction => "construction",
    });
    assert_eq!(
        roles.into_iter().collect::<BTreeSet<_>>(),
        catalog_keys("geometry-role").into_iter().collect()
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the exact M83-F001 import fixture keeps the oversized native binding, compact logical port, host input and cold-reload evidence adjacent"
)]
fn m83_f001_host_bound_rational_conic_import_uses_compact_ports_without_losing_binding_identity() {
    let mut document = SketchDocument::new(4.0).expect("document");
    let start = document.add_point("start", [0.0, 0.0]).expect("start");
    let end = document.add_point("end", [4.0, 0.0]).expect("end");
    let domain = ScalarDomain::Bounded {
        lower: MIN_RATIONAL_QUADRATIC_MIDDLE_WEIGHT,
        upper: f64::MAX,
    };
    let weight = document
        .add_scalar("weight", 0.5, ScalarUnit::Parameter, domain)
        .expect("weight");
    let curve = document
        .add_curve(
            "host-owned rational conic",
            CurveDefinition::RationalQuadraticConic {
                start,
                weighted_middle: [1.0, 1.5],
                middle_weight: weight,
                end,
            },
        )
        .expect("rational conic");
    let parameter = document
        .add_parameter("weight", DocumentParameterKind::Dimensionless)
        .expect("dimensionless parameter");
    let target = DocumentParameterTarget::DimensionlessFixedScalar(DocumentScalarPropertyRef {
        scalar: weight,
        unit: DocumentScalarUnit::Dimensionless,
        domain,
        branch: DocumentScalarBranch::Dimensionless,
    });
    document
        .add_parameter_binding(parameter, target)
        .expect("host parameter binding");
    let binding = document.parameter_bindings()[0];
    let exact_binding_identity = serde_json::to_string(&binding).expect("binding identity");
    assert!(
        exact_binding_identity.len() > MAX_SEMANTIC_PORT_KEY_BYTES,
        "this regression must retain the exact structured identity that originally overflowed a semantic key: {} bytes",
        exact_binding_identity.len()
    );

    let batch = ParameterBatch::new(
        1,
        vec![ParameterBatchEntry {
            parameter,
            value: ParameterValue::Dimensionless(0.5),
        }],
    )
    .expect("host parameter batch");
    let retained = RetainedSketchDocumentSession::new_with_parameter_batch(
        document,
        batch.clone(),
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("host-bound rational session");
    let coordinator =
        RetainedEditorCoordinator::new(retained).expect("M83-F001 lineage import must succeed");
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .rational_conic_control(curve)
            .expect("rational control"),
        DocumentRationalConicControl::Euclidean {
            middle: [2.0, 3.0],
            weight: 0.5,
        }
    );

    let baseline = &coordinator.lineage_document().steps()[0];
    let binding_outputs = baseline
        .outputs
        .iter()
        .filter(|output| output.kind == LineageOutputKind::ParameterBinding)
        .collect::<Vec<_>>();
    assert_eq!(binding_outputs.len(), 1);
    let binding_output = binding_outputs[0];
    assert!(
        binding_output
            .key
            .as_str()
            .starts_with("parameter-binding-")
    );
    assert!(binding_output.key.as_str().len() <= MAX_SEMANTIC_PORT_KEY_BYTES);
    assert_eq!(binding_output.reservation, None);
    assert_ne!(binding_output.key.as_str(), exact_binding_identity);
    assert!(
        baseline
            .outputs
            .iter()
            .all(|output| output.key.as_str().len() <= MAX_SEMANTIC_PORT_KEY_BYTES)
    );

    let LineageActionDefinition::ImportedBaseline { baseline } = &baseline.action else {
        panic!("first lineage action must remain an honest imported baseline");
    };
    let imported_payload: Value =
        serde_json::from_str(&baseline.payload).expect("imported baseline payload");
    let imported_checkpoint = &imported_payload["checkpoint"];
    let imported_design_json = imported_checkpoint["design_json"]
        .as_str()
        .expect("imported design JSON");
    let imported_design = if imported_checkpoint["design_is_draft_v5"] == json!(true) {
        SketchDocument::from_draft_v5_json(imported_design_json).expect("draft-v5 import")
    } else {
        SketchDocument::from_json(imported_design_json).expect("canonical-v4 import")
    };
    assert_eq!(imported_design.parameter_bindings(), &[binding]);
    assert_eq!(
        serde_json::to_string(&imported_design.parameter_bindings()[0])
            .expect("re-encoded exact binding"),
        exact_binding_identity
    );

    let session_json = coordinator
        .lineage_session_json()
        .expect("canonical lineage session");
    let restored = LineageSession::from_session_json(&session_json).expect("strict session reload");
    assert_eq!(
        restored
            .to_canonical_session_json()
            .expect("canonical session re-export"),
        session_json
    );
    let evidence = evaluate_lineage_session_cold_with_inputs(
        &restored,
        &batch,
        &ExternalSnapshotSet::default(),
    )
    .expect("independently validated cold host-bound rational evaluation");
    assert_eq!(evidence.validated_prefix_count(), 1);

    let cold = RetainedEditorCoordinator::lineage_materialization_checkpoint(&session_json)
        .expect("cold lineage materialization");
    let cold_document = if cold.design_uses_draft_v5() {
        SketchDocument::from_draft_v5_json(cold.design_json()).expect("cold draft-v5 document")
    } else {
        SketchDocument::from_json(cold.design_json()).expect("cold canonical-v4 document")
    };
    assert_eq!(cold_document.parameter_bindings(), &[binding]);
    assert_eq!(
        cold_document
            .rational_conic_control(curve)
            .expect("cold rational control"),
        DocumentRationalConicControl::Euclidean {
            middle: [2.0, 3.0],
            weight: 0.5,
        }
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one datum contract regression keeps Origin/X/Y semantic parameters, protected interaction, history and cold branch evidence adjacent"
)]
fn m83_w2_intrinsic_datums_are_typed_parameters_never_owned_lineage_outputs() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let origin_point = document
        .add_point("origin constrained", [0.0, 0.0])
        .expect("origin point");
    let x_point = document
        .add_point("x constrained", [2.0, 0.0])
        .expect("x point");
    let line_start = document
        .add_point("line start", [0.0, -2.0])
        .expect("line start");
    let line_end = document
        .add_point("line end", [0.0, 2.0])
        .expect("line end");
    let line = document
        .add_curve(
            "datum-collinear support",
            CurveDefinition::Line {
                start: line_start,
                end: line_end,
                branch_direction: [0.0, 1.0],
            },
        )
        .expect("line");
    let symmetric_first = document
        .add_point("symmetric first", [-1.0, 3.0])
        .expect("first symmetric point");
    let symmetric_second = document
        .add_point("symmetric second", [1.0, 3.0])
        .expect("second symmetric point");
    let mut coordinator = RetainedEditorCoordinator::new(
        RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("retained sketch session"),
    )
    .expect("coordinator");

    let definitions = vec![
        DocumentConstraintDefinition::CoincidentWithOrigin {
            point: origin_point,
        },
        DocumentConstraintDefinition::PointOnDatumAxis {
            point: x_point,
            axis: DocumentCoordinateAxis::X,
        },
        DocumentConstraintDefinition::CollinearWithDatumAxis {
            line: DocumentLineSupportRef {
                span: CurveSpan::line(line),
                direction: DocumentDirectionSense::Forward,
            },
            axis: DocumentCoordinateAxis::Y,
        },
        DocumentConstraintDefinition::SymmetricAboutDatumAxis {
            first: symmetric_first,
            second: symmetric_second,
            axis: DocumentCoordinateAxis::Y,
        },
    ];

    for (ordinal, definition) in definitions.iter().cloned().enumerate() {
        let edit = DocumentEdit::CreateConstraint {
            label: format!("lineage datum relation {ordinal}"),
            definition,
        };
        let expected_intent = json!({
            "version": 1,
            "body": serde_json::to_value(&edit).expect("typed datum edit"),
        });
        coordinator
            .apply_edit(coordinator.session().design_identity(), edit)
            .expect("datum relation");
        let step = coordinator
            .lineage_document()
            .steps()
            .last()
            .expect("datum relation owner");
        let LineageActionDefinition::Constraint { action } = &step.action else {
            panic!("datum relation must remain a typed constraint action");
        };
        assert_eq!(
            action.schema.as_str(),
            "geosolve.document-edit.v1.create-constraint"
        );
        let mut actual_intent = action
            .parameters
            .get("authored_intent")
            .cloned()
            .expect("authored datum intent");
        let owner_manifest = actual_intent
            .as_object_mut()
            .expect("object datum intent")
            .remove("owned_output_field_manifest");
        let owner_values = actual_intent
            .as_object_mut()
            .expect("object datum intent")
            .remove("owned_output_fields");
        assert!(
            owner_manifest.is_some(),
            "datum relation {ordinal} must authenticate its writable owner fields"
        );
        assert!(
            owner_values.is_some(),
            "datum relation {ordinal} must retain complete semantic owner values"
        );
        assert_eq!(actual_intent, expected_intent);
        assert!(!action.inputs.is_empty());
        let input_kinds = action
            .inputs
            .iter()
            .map(|input| input.kind)
            .collect::<BTreeSet<_>>();
        assert!(
            action.inputs.iter().all(|input| matches!(
                input.kind,
                LineageOutputKind::Point
                    | LineageOutputKind::Curve
                    | LineageOutputKind::CurveSpan
                    | LineageOutputKind::Source
            )),
            "datum relation {ordinal} inputs must name only ordinary geometry or source-order ports, got {input_kinds:?}"
        );
        assert_eq!(
            step.reservations
                .iter()
                .map(|reservation| reservation.kind)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([
                LineageReservationKind::Constraint,
                LineageReservationKind::Source,
            ]),
            "retained_planar_constraints must publish the ordinary persistent constraint/source identity pair"
        );
        assert_eq!(
            step.outputs
                .iter()
                .filter(|output| output.reservation.is_some())
                .map(|output| output.kind)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([LineageOutputKind::Constraint, LineageOutputKind::Source]),
            "datum operands are parameter-only, while their persistent relation/source remain typed owned outputs"
        );
        assert!(
            step.outputs
                .iter()
                .all(|output| !output.key.as_str().contains("datum")
                    && !output.key.as_str().contains("origin")
                    && !output.key.as_str().contains("axis"))
        );
        assert!(step.reservations.iter().all(|reservation| {
            !reservation.key.as_str().contains("datum")
                && !reservation.key.as_str().contains("origin")
                && !reservation.key.as_str().contains("axis")
        }));
    }

    let live_definitions = coordinator
        .session()
        .design_document()
        .constraints()
        .iter()
        .map(|constraint| constraint.definition.clone())
        .collect::<Vec<_>>();
    assert_eq!(live_definitions, definitions);
    let live_action_steps = coordinator
        .lineage_document()
        .steps()
        .iter()
        .skip(1)
        .cloned()
        .collect::<Vec<_>>();
    let live_allocator = coordinator.lineage_document().allocator_high_water();
    let live_session_json = coordinator
        .lineage_session_json()
        .expect("live lineage session");
    let restored =
        LineageSession::from_session_json(&live_session_json).expect("strict datum session reload");
    assert_eq!(
        restored
            .to_canonical_session_json()
            .expect("canonical datum session re-export"),
        live_session_json
    );

    let cold = RetainedEditorCoordinator::lineage_materialization_checkpoint(&live_session_json)
        .expect("cold datum materialization");
    let cold_document = if cold.design_uses_draft_v5() {
        SketchDocument::from_draft_v5_json(cold.design_json()).expect("cold draft-v5 datum scene")
    } else {
        SketchDocument::from_json(cold.design_json()).expect("cold canonical-v4 datum scene")
    };
    assert_eq!(
        cold_document
            .constraints()
            .iter()
            .map(|constraint| constraint.definition.clone())
            .collect::<Vec<_>>(),
        definitions
    );

    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Datum(SketchDatum::Origin)]);
    let protected_lineage = coordinator
        .lineage_session_json()
        .expect("lineage before protected datum deletion");
    assert!(matches!(
        coordinator.delete_selected(coordinator.session().design_identity()),
        Err(CoordinatorError::ActionUnavailable(
            DisabledReason::ProtectedDatum
        ))
    ));
    assert_eq!(
        coordinator
            .lineage_session_json()
            .expect("lineage after protected datum deletion"),
        protected_lineage
    );

    for _ in &definitions {
        coordinator.undo().expect("undo datum relation");
    }
    assert!(
        coordinator
            .session()
            .design_document()
            .constraints()
            .is_empty()
    );
    for _ in &definitions {
        coordinator.redo().expect("redo datum relation");
    }
    assert_eq!(
        coordinator
            .lineage_document()
            .steps()
            .iter()
            .skip(1)
            .cloned()
            .collect::<Vec<_>>(),
        live_action_steps
    );
    assert_eq!(
        coordinator.lineage_document().allocator_high_water(),
        live_allocator
    );
    assert_eq!(
        coordinator
            .session()
            .design_document()
            .constraints()
            .iter()
            .map(|constraint| constraint.definition.clone())
            .collect::<Vec<_>>(),
        definitions
    );
}

#[test]
fn m83_w3_gauge_owned_nurbs_weight_rejects_without_lineage_or_history_change() {
    let mut document = SketchDocument::new(10.0).expect("document");
    let controls = [[0.0, 0.0], [1.0, 2.0], [3.0, 2.0], [4.0, 0.0]]
        .map(|position| document.add_point("control", position).expect("control"));
    let weights = [1.0, 0.8, 1.2, 1.0]
        .map(|value| {
            document
                .add_scalar(
                    "weight",
                    value,
                    ScalarUnit::Parameter,
                    ScalarDomain::Positive,
                )
                .expect("weight")
        })
        .to_vec();
    let gauge = weights[0];
    let curve = document
        .add_curve(
            "NURBS",
            CurveDefinition::Nurbs {
                form: DocumentBSplineForm::Clamped,
                degree: 2,
                controls: controls.to_vec(),
                weights,
                gauge_weight: gauge,
                knots: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
                span_ids: vec![7, 11],
                next_span_id: 12,
            },
        )
        .expect("NURBS");
    let selected_span = document.curve_spans(curve).expect("NURBS spans")[0];
    let session = RetainedSketchDocumentSession::new(
        document,
        DocumentSolveRequest::default(),
        SolverConfig::default(),
    )
    .expect("retained sketch session");
    let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
    coordinator
        .editor_mut()
        .set_selection([SelectionItem::Curve(selected_span)]);

    let metadata = coordinator
        .selected_curve_property_metadata()
        .expect("selected NURBS metadata");
    assert_eq!(metadata.nurbs_gauge, Some(gauge));
    assert_eq!(
        metadata.numeric[0].availability,
        DocumentCurveControlAvailability::ReadOnly(
            DocumentCurveControlWithholdingReason::GaugeOwned,
        )
    );
    let before_lineage = coordinator
        .lineage_session_json()
        .expect("lineage before rejected gauge edit");
    let before_history = (coordinator.history_len(), coordinator.history_cursor());
    let expected = coordinator.session().design_identity();
    assert!(matches!(
        coordinator.set_curve_numeric_property(
            expected,
            curve,
            CurveNumericPropertyKind::NurbsWeight { ordinal: 0 },
            2.0,
        ),
        Err(CoordinatorError::CurvePropertyUnavailable(
            DocumentCurveControlWithholdingReason::GaugeOwned
        ))
    ));
    assert_eq!(
        coordinator
            .lineage_session_json()
            .expect("lineage after rejected gauge edit"),
        before_lineage
    );
    assert_eq!(
        (coordinator.history_len(), coordinator.history_cursor()),
        before_history
    );
}
