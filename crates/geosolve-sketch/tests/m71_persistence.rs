// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    CurveDefinition, CurveSpan, DocumentCenterRef, DocumentConstraintDefinition,
    DocumentDirectionSense, DocumentElementId, DocumentError, DocumentId, DocumentLineSupportRef,
    DocumentSourceId, MAX_DOCUMENT_JSON_BYTES, MAX_DOCUMENT_OBJECTS, PersistentId, ScalarDomain,
    ScalarUnit, SketchDocument,
};

fn circle(
    document: &mut SketchDocument,
    label: &str,
    center: geosolve_sketch::DesignPointId,
) -> geosolve_sketch::CurveId {
    let radius = document
        .add_scalar(
            format!("{label} radius"),
            1.0,
            ScalarUnit::Length,
            ScalarDomain::Positive,
        )
        .unwrap();
    document
        .add_curve(label, CurveDefinition::Circle { center, radius })
        .unwrap()
}

fn line(
    document: &mut SketchDocument,
    label: &str,
    start: geosolve_sketch::DesignPointId,
    end: geosolve_sketch::DesignPointId,
) -> geosolve_sketch::CurveId {
    document
        .add_curve(
            label,
            CurveDefinition::Line {
                start,
                end,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap()
}

fn m71_document() -> SketchDocument {
    let mut document = SketchDocument::new(1.0).unwrap();
    let points = [[0.0, 0.0], [2.0, 0.0], [0.0, 2.0], [2.0, 2.0]]
        .map(|position| document.add_point("point", position).unwrap());
    let circles = [
        circle(&mut document, "first circle", points[0]),
        circle(&mut document, "second circle", points[2]),
    ];
    let lines = [
        line(&mut document, "first line", points[0], points[1]),
        line(&mut document, "second line", points[2], points[3]),
    ];
    document
        .add_constraint(
            "point horizontal",
            DocumentConstraintDefinition::HorizontalPoints {
                first: points[0],
                second: points[1],
            },
        )
        .unwrap();
    let vertical = document
        .add_constraint(
            "point vertical",
            DocumentConstraintDefinition::VerticalPoints {
                first: points[0],
                second: points[2],
            },
        )
        .unwrap();
    document
        .set_element_user_suppressed(DocumentElementId::Constraint(vertical), true)
        .unwrap();
    document
        .add_constraint(
            "circle concentric",
            DocumentConstraintDefinition::Concentric {
                first: DocumentCenterRef { curve: circles[0] },
                second: DocumentCenterRef { curve: circles[1] },
            },
        )
        .unwrap();
    document
        .add_constraint(
            "line collinear",
            DocumentConstraintDefinition::Collinear {
                first: DocumentLineSupportRef {
                    span: CurveSpan::line(lines[0]),
                    direction: DocumentDirectionSense::Forward,
                },
                second: DocumentLineSupportRef {
                    span: CurveSpan::line(lines[1]),
                    direction: DocumentDirectionSense::Reverse,
                },
            },
        )
        .unwrap();
    document
        .add_constraint(
            "point horizontal to midpoint",
            DocumentConstraintDefinition::HorizontalPointToMidpoint {
                point: points[3],
                line: CurveSpan::line(lines[0]),
            },
        )
        .unwrap();
    document
        .add_constraint(
            "point vertical to midpoint",
            DocumentConstraintDefinition::VerticalPointToMidpoint {
                point: points[1],
                line: CurveSpan::line(lines[1]),
            },
        )
        .unwrap();
    document
}

fn mixed_m71_document() -> SketchDocument {
    let mut document = m71_document();
    let point = document.points()[0].id;
    let target = document.points()[0].position;
    document
        .add_constraint(
            "ordinary fixed point",
            DocumentConstraintDefinition::FixedPoint { point, target },
        )
        .unwrap();
    document
}

fn encoded_id(value: u128) -> serde_json::Value {
    serde_json::Value::String(PersistentId::from_u128(value).to_string())
}

fn mutate_json(json: &str, mutation: impl FnOnce(&mut serde_json::Value)) -> String {
    let mut value: serde_json::Value = serde_json::from_str(json).unwrap();
    mutation(&mut value);
    serde_json::to_string(&value).unwrap()
}

#[test]
fn frozen_v4_and_m71_empty_draft_bytes_remain_exact() {
    let document = SketchDocument::with_id(
        1.0,
        geosolve_sketch::DocumentId(geosolve_sketch::PersistentId::from_u128(1)),
    )
    .unwrap();
    let canonical = concat!(
        "{\"version\":4,\"id\":\"00000000000000000000000000000001\",",
        "\"next_id\":\"00000000000000000000000000000002\",\"model_scale\":1.0,",
        "\"points\":[],\"scalars\":[],\"curves\":[],\"contacts\":[],",
        "\"trim_views\":[],\"constraints\":[],\"dimensions\":[],\"source_order\":[]}"
    );
    assert_eq!(document.to_canonical_json().unwrap(), canonical);

    let draft = format!(
        "{{\"version\":5,\"document\":{canonical},\"geometry_roles\":[],\"user_inactive_elements\":[],\"host_activation\":null,\"parameters\":[],\"parameter_bindings\":[],\"parameter_outputs\":[],\"external_bindings\":[]}}"
    );
    assert_eq!(document.to_draft_v5_json().unwrap(), draft);
    assert!(!draft.contains("retained_planar_constraints"));
    assert_eq!(
        SketchDocument::from_draft_v5_json(&draft)
            .unwrap()
            .to_draft_v5_json()
            .unwrap(),
        draft
    );
}

#[test]
fn all_retained_planar_records_round_trip_only_through_draft_v5() {
    let document = m71_document();
    assert!(matches!(
        document.to_canonical_json(),
        Err(DocumentError::UnsupportedM71State)
    ));

    let draft = document.to_draft_v5_json().unwrap();
    let value: serde_json::Value = serde_json::from_str(&draft).unwrap();
    assert_eq!(
        value["retained_planar_constraints"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let definitions = value["retained_planar_constraints"]
        .as_array()
        .unwrap()
        .iter()
        .map(|record| &record["definition"])
        .collect::<Vec<_>>();
    assert!(definitions.iter().any(|definition| {
        definition["kind"] == "horizontal_point_to_midpoint"
            && definition["point"].is_string()
            && definition["line"]["curve"].is_string()
            && definition["line"]["segment"] == 0
    }));
    assert!(definitions.iter().any(|definition| {
        definition["kind"] == "vertical_point_to_midpoint"
            && definition["point"].is_string()
            && definition["line"]["curve"].is_string()
            && definition["line"]["segment"] == 0
    }));
    assert!(
        value["document"]["constraints"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let restored = SketchDocument::from_draft_v5_json(&draft).unwrap();
    assert_eq!(restored.constraints(), document.constraints());
    assert_eq!(restored.source_order(), document.source_order());
    assert_eq!(restored.to_draft_v5_json().unwrap(), draft);

    let disguised_v4 = serde_json::to_string(&value["document"]).unwrap();
    assert!(SketchDocument::from_json(&disguised_v4).is_err());
}

#[test]
fn draft_v5_rejects_identity_collisions_between_embedded_and_side_constraints() {
    let draft = mixed_m71_document().to_draft_v5_json().unwrap();

    let duplicate_constraint_id = mutate_json(&draft, |value| {
        value["retained_planar_constraints"][0]["id"] =
            value["document"]["constraints"][0]["id"].clone();
    });
    assert!(matches!(
        SketchDocument::from_draft_v5_json(&duplicate_constraint_id),
        Err(DocumentError::DuplicateId(_))
    ));

    let duplicate_source_id = mutate_json(&draft, |value| {
        value["retained_planar_constraints"][0]["source_id"] =
            value["document"]["constraints"][0]["source_id"].clone();
    });
    assert!(matches!(
        SketchDocument::from_draft_v5_json(&duplicate_source_id),
        Err(DocumentError::DuplicateId(_))
    ));
}

#[test]
fn draft_v5_rejects_unknown_retained_planar_syntax() {
    let draft = m71_document().to_draft_v5_json().unwrap();

    let unknown_record_field = mutate_json(&draft, |value| {
        value["retained_planar_constraints"][0]["future_field"] = serde_json::Value::Bool(true);
    });
    assert!(matches!(
        SketchDocument::from_draft_v5_json(&unknown_record_field),
        Err(DocumentError::Json(_))
    ));

    let unknown_definition_field = mutate_json(&draft, |value| {
        value["retained_planar_constraints"][0]["definition"]["future_field"] =
            serde_json::Value::Bool(true);
    });
    assert!(matches!(
        SketchDocument::from_draft_v5_json(&unknown_definition_field),
        Err(DocumentError::Json(_))
    ));

    let unknown_definition_kind = mutate_json(&draft, |value| {
        value["retained_planar_constraints"][0]["definition"]["kind"] =
            serde_json::Value::String("future_relation".into());
    });
    assert!(matches!(
        SketchDocument::from_draft_v5_json(&unknown_definition_kind),
        Err(DocumentError::Json(_))
    ));
}

#[test]
fn frozen_v4_rejects_every_m71_definition_even_when_injected_into_embedded_constraints() {
    let draft = m71_document().to_draft_v5_json().unwrap();
    let root: serde_json::Value = serde_json::from_str(&draft).unwrap();
    let retained = root["retained_planar_constraints"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(retained.len(), 6);

    for record in retained {
        let mut injected = root.clone();
        injected["document"]["constraints"]
            .as_array_mut()
            .unwrap()
            .push(record);

        let embedded_v4 = serde_json::to_string(&injected["document"]).unwrap();
        assert!(matches!(
            SketchDocument::from_json(&embedded_v4),
            Err(DocumentError::Json(_))
        ));

        let enclosing_draft = serde_json::to_string(&injected).unwrap();
        assert!(matches!(
            SketchDocument::from_draft_v5_json(&enclosing_draft),
            Err(DocumentError::Json(_))
        ));
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one corruption matrix keeps cross-record identity, ordering, operands, and next-ID accounting contiguous"
)]
fn draft_v5_rejects_side_identity_order_and_operand_corruption() {
    let document = m71_document();
    let draft = document.to_draft_v5_json().unwrap();

    let duplicate_id = mutate_json(&draft, |value| {
        let records = value["retained_planar_constraints"].as_array_mut().unwrap();
        records[1]["id"] = records[0]["id"].clone();
    });
    assert!(SketchDocument::from_draft_v5_json(&duplicate_id).is_err());

    let duplicate_source = mutate_json(&draft, |value| {
        let records = value["retained_planar_constraints"].as_array_mut().unwrap();
        records[1]["source_id"] = records[0]["source_id"].clone();
    });
    assert!(SketchDocument::from_draft_v5_json(&duplicate_source).is_err());

    let missing_source = mutate_json(&draft, |value| {
        let source = value["retained_planar_constraints"][0]["source_id"].clone();
        value["document"]["source_order"]
            .as_array_mut()
            .unwrap()
            .retain(|entry| entry != &source);
    });
    assert!(SketchDocument::from_draft_v5_json(&missing_source).is_err());

    let duplicated_order = mutate_json(&draft, |value| {
        let source = value["document"]["source_order"][0].clone();
        value["document"]["source_order"]
            .as_array_mut()
            .unwrap()
            .push(source);
    });
    assert!(SketchDocument::from_draft_v5_json(&duplicated_order).is_err());

    let foreign_order_entry = mutate_json(&draft, |value| {
        value["document"]["source_order"]
            .as_array_mut()
            .unwrap()
            .push(encoded_id(u128::MAX));
    });
    assert!(matches!(
        SketchDocument::from_draft_v5_json(&foreign_order_entry),
        Err(DocumentError::InvalidField {
            field: "source_order",
            ..
        })
    ));

    let unknown_operand = mutate_json(&draft, |value| {
        value["retained_planar_constraints"][0]["definition"]["first"] =
            serde_json::Value::String("ffffffffffffffffffffffffffffffff".into());
    });
    assert!(SketchDocument::from_draft_v5_json(&unknown_operand).is_err());

    let unknown_midpoint_point = mutate_json(&draft, |value| {
        let record = value["retained_planar_constraints"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|record| {
                record["definition"]["kind"]
                    == serde_json::Value::String("horizontal_point_to_midpoint".into())
            })
            .unwrap();
        record["definition"]["point"] =
            serde_json::Value::String("ffffffffffffffffffffffffffffffff".into());
    });
    assert!(SketchDocument::from_draft_v5_json(&unknown_midpoint_point).is_err());

    let unknown_midpoint_line = mutate_json(&draft, |value| {
        let record = value["retained_planar_constraints"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|record| {
                record["definition"]["kind"]
                    == serde_json::Value::String("vertical_point_to_midpoint".into())
            })
            .unwrap();
        record["definition"]["line"]["curve"] =
            serde_json::Value::String("ffffffffffffffffffffffffffffffff".into());
    });
    assert!(SketchDocument::from_draft_v5_json(&unknown_midpoint_line).is_err());

    let invalid_midpoint_span = mutate_json(&draft, |value| {
        let record = value["retained_planar_constraints"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|record| {
                record["definition"]["kind"]
                    == serde_json::Value::String("horizontal_point_to_midpoint".into())
            })
            .unwrap();
        record["definition"]["line"]["segment"] = serde_json::Value::from(1);
    });
    assert!(SketchDocument::from_draft_v5_json(&invalid_midpoint_span).is_err());

    let invalid_next_id = mutate_json(&draft, |value| {
        value["document"]["next_id"] = serde_json::Value::String(document.id().0.to_string());
    });
    assert!(SketchDocument::from_draft_v5_json(&invalid_next_id).is_err());

    let foreign_source = mutate_json(&draft, |value| {
        value["retained_planar_constraints"][0]["source_id"] = encoded_id(u128::MAX);
    });
    assert!(SketchDocument::from_draft_v5_json(&foreign_source).is_err());

    let foreign_source_with_matching_order = mutate_json(&draft, |value| {
        let old_source = value["retained_planar_constraints"][0]["source_id"].clone();
        let foreign_source = encoded_id(u128::MAX);
        value["retained_planar_constraints"][0]["source_id"] = foreign_source.clone();
        let order = value["document"]["source_order"].as_array_mut().unwrap();
        let entry = order
            .iter_mut()
            .find(|entry| **entry == old_source)
            .unwrap();
        *entry = foreign_source;
    });
    assert!(matches!(
        SketchDocument::from_draft_v5_json(&foreign_source_with_matching_order),
        Err(DocumentError::InvalidField {
            field: "next_id",
            ..
        })
    ));

    let reversed_order = mutate_json(&draft, |value| {
        value["document"]["source_order"]
            .as_array_mut()
            .unwrap()
            .reverse();
    });
    let restored = SketchDocument::from_draft_v5_json(&reversed_order).unwrap();
    let expected_order = document
        .source_order()
        .iter()
        .rev()
        .copied()
        .collect::<Vec<DocumentSourceId>>();
    assert_eq!(restored.source_order(), expected_order);
}

#[test]
fn draft_v5_applies_complete_object_accounting_after_side_record_merge() {
    const SIDE_CONSTRAINTS: usize = MAX_DOCUMENT_OBJECTS / 2 - 1;

    let mut document =
        SketchDocument::with_id(1.0, DocumentId(PersistentId::from_u128(1))).unwrap();
    let first = document.add_point("a", [0.0, 0.0]).unwrap();
    let second = document.add_point("b", [1.0, 0.0]).unwrap();
    document
        .add_constraint(
            "horizontal",
            DocumentConstraintDefinition::HorizontalPoints { first, second },
        )
        .unwrap();

    let draft = document.to_draft_v5_json().unwrap();
    let mut value: serde_json::Value = serde_json::from_str(&draft).unwrap();
    let template = value["retained_planar_constraints"][0].clone();
    let mut records = Vec::with_capacity(SIDE_CONSTRAINTS);
    let mut source_order = Vec::with_capacity(SIDE_CONSTRAINTS);
    for index in 0..SIDE_CONSTRAINTS {
        let mut record = template.clone();
        let constraint_id = 4 + 2 * index as u128;
        let source_id = constraint_id + 1;
        record["id"] = encoded_id(constraint_id);
        record["source_id"] = encoded_id(source_id);
        record["label"] = serde_json::Value::String("x".into());
        records.push(record);
        source_order.push(encoded_id(source_id));
    }
    value["retained_planar_constraints"] = serde_json::Value::Array(records);
    value["document"]["source_order"] = serde_json::Value::Array(source_order);
    value["document"]["next_id"] = encoded_id(4 + 2 * SIDE_CONSTRAINTS as u128);

    let oversized_after_merge = serde_json::to_string(&value).unwrap();
    assert!(oversized_after_merge.len() < MAX_DOCUMENT_JSON_BYTES);
    assert!(matches!(
        SketchDocument::from_draft_v5_json(&oversized_after_merge),
        Err(DocumentError::ResourceLimit {
            resource: "objects",
            actual: 100_001,
            limit: MAX_DOCUMENT_OBJECTS,
        })
    ));
}
