// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_core::SolverConfig;
use geosolve_sketch::{
    AlphaScenarioIds, AlphaScenarioKind, CurveId, DocumentElementId, DocumentError, DocumentId,
    DocumentObjectId, DocumentSourceOwner, PersistentId, SketchAttributeError, SketchAttributes,
    SketchDocument, SketchDocumentSession, alpha_scenario,
};

#[derive(Debug, Eq, PartialEq)]
struct HostAttribute {
    layer: u32,
    color: &'static str,
}

#[test]
fn persistent_element_and_source_views_cover_the_complete_document_graph() {
    let fixture = alpha_scenario(AlphaScenarioKind::A8, 1.0).unwrap();
    let document = &fixture.document;
    let mut elements = vec![DocumentElementId::Document(document.id())];
    elements.extend(
        document
            .points()
            .iter()
            .map(|value| DocumentElementId::from(value.id)),
    );
    elements.extend(
        document
            .scalars()
            .iter()
            .map(|value| DocumentElementId::from(value.id)),
    );
    elements.extend(
        document
            .curves()
            .iter()
            .map(|value| DocumentElementId::from(value.id)),
    );
    elements.extend(
        document
            .contacts()
            .iter()
            .map(|value| DocumentElementId::from(value.id)),
    );
    elements.extend(
        document
            .constraints()
            .iter()
            .map(|value| DocumentElementId::from(value.id)),
    );
    elements.extend(
        document
            .dimensions()
            .iter()
            .map(|value| DocumentElementId::from(value.id)),
    );
    elements.extend(
        document
            .source_order()
            .iter()
            .copied()
            .map(DocumentElementId::from),
    );

    for element in elements {
        assert!(document.contains_element(element));
        assert_eq!(document.element(element.persistent_id()), Some(element));
        assert!(!element.kind().is_empty());
    }
    let sources = document.sources().collect::<Vec<_>>();
    assert_eq!(sources.len(), document.source_order().len());
    assert_eq!(
        sources.iter().map(|source| source.id).collect::<Vec<_>>(),
        document.source_order()
    );
    for source in sources {
        assert!(!source.label.is_empty());
        match source.owner {
            DocumentSourceOwner::Constraint(owner) => {
                let constraint = document.constraint(owner).unwrap();
                assert_eq!(constraint.source_id, source.id);
                assert_eq!(constraint.suppressed, source.suppressed);
            }
            DocumentSourceOwner::Dimension(owner) => {
                let dimension = document.dimension(owner).unwrap();
                assert_eq!(dimension.source_id, source.id);
                assert_eq!(dimension.suppressed, source.suppressed);
            }
        }
    }
}

#[test]
fn typed_attributes_reject_foreign_missing_and_wrong_kind_targets() {
    let fixture = alpha_scenario(AlphaScenarioKind::A1, 1.0).unwrap();
    let AlphaScenarioIds::A1(ids) = fixture.ids else {
        unreachable!()
    };
    let mut attributes = SketchAttributes::new(&fixture.document);
    let point = ids.rectangle.points[0];
    attributes
        .insert(
            &fixture.document,
            point,
            HostAttribute {
                layer: 3,
                color: "construction",
            },
        )
        .unwrap();
    assert_eq!(attributes.document_id(), fixture.document.id());
    assert_eq!(attributes.len(), 1);
    assert_eq!(attributes.get(point).unwrap().layer, 3);
    assert_eq!(attributes.iter().next().unwrap().0, point.into());

    let foreign = SketchDocument::with_id(
        1.0,
        DocumentId(PersistentId::from_u128(0xf000_0000_0000_0000)),
    )
    .unwrap();
    assert!(matches!(
        attributes.insert(
            &foreign,
            foreign.id(),
            HostAttribute {
                layer: 0,
                color: "foreign",
            },
        ),
        Err(SketchAttributeError::ForeignDocument { .. })
    ));

    let wrong_kind = CurveId(point.0);
    assert!(matches!(
        attributes.insert(
            &fixture.document,
            wrong_kind,
            HostAttribute {
                layer: 0,
                color: "wrong",
            },
        ),
        Err(SketchAttributeError::WrongElementKind {
            requested: "curve",
            actual: "point",
            ..
        })
    ));
    let missing = CurveId(PersistentId::from_u128(u128::MAX - 1));
    assert!(matches!(
        attributes.insert(
            &fixture.document,
            missing,
            HostAttribute {
                layer: 0,
                color: "missing",
            },
        ),
        Err(SketchAttributeError::UnknownElement { kind: "curve", .. })
    ));
}

#[test]
fn dormant_attributes_survive_delete_undo_redo_until_explicit_cleanup() {
    let fixture = alpha_scenario(AlphaScenarioKind::A1, 1.0).unwrap();
    let AlphaScenarioIds::A1(ids) = fixture.ids else {
        unreachable!()
    };
    let mut session =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    let dimension = ids.diagonal;
    let source = session.document().dimension(dimension).unwrap().source_id;
    let mut attributes = SketchAttributes::new(session.document());
    attributes
        .insert(session.document(), dimension, "dimension")
        .unwrap();
    attributes
        .insert(session.document(), source, "source")
        .unwrap();

    let deleted = session
        .transact(
            session.revision(),
            "delete diagonal dimension",
            |document| document.remove_with_owned_state(DocumentObjectId::Dimension(dimension)),
        )
        .unwrap();
    assert!(deleted.outcome.effect.is_some());
    assert_eq!(attributes.get(dimension), Some(&"dimension"));
    assert_eq!(
        attributes.get_live(session.document(), dimension).unwrap(),
        None
    );
    assert_eq!(
        attributes.orphaned_targets(session.document()).unwrap(),
        vec![dimension.into(), source.into()]
    );

    session.undo(session.revision()).unwrap();
    assert_eq!(
        attributes.get_live(session.document(), dimension).unwrap(),
        Some(&"dimension")
    );
    assert_eq!(
        attributes.get_live(session.document(), source).unwrap(),
        Some(&"source")
    );
    assert!(
        attributes
            .orphaned_targets(session.document())
            .unwrap()
            .is_empty()
    );

    session.redo(session.revision()).unwrap();
    assert_eq!(
        attributes.get_live(session.document(), dimension).unwrap(),
        None
    );
    assert_eq!(attributes.retain_live(session.document()).unwrap(), 2);
    assert!(attributes.is_empty());
}

#[test]
fn application_attributes_cannot_change_solver_or_canonical_document_state() {
    let fixture = alpha_scenario(AlphaScenarioKind::A1, 1.0).unwrap();
    let AlphaScenarioIds::A1(ids) = fixture.ids else {
        unreachable!()
    };
    let session =
        SketchDocumentSession::new(fixture.document, fixture.request, SolverConfig::default())
            .unwrap();
    let before_json = session.export_json().unwrap();
    let before_result = session.runtime().accepted_result().clone();
    let before_revision = session.revision();
    let mut attributes = SketchAttributes::new(session.document());
    attributes
        .insert(
            session.document(),
            ids.rectangle.curves[0],
            HostAttribute {
                layer: 7,
                color: "profile",
            },
        )
        .unwrap();

    assert_eq!(session.export_json().unwrap(), before_json);
    assert_eq!(session.runtime().accepted_result(), &before_result);
    assert_eq!(session.revision(), before_revision);
    assert!(!before_json.contains("profile"));
}

#[test]
fn prior_wire_payloads_are_frozen_and_dispatch_rejects_unknown_versions() {
    let document = SketchDocument::with_id(1.0, DocumentId(PersistentId::from_u128(1))).unwrap();
    let version_four = concat!(
        "{\"version\":4,\"id\":\"00000000000000000000000000000001\",",
        "\"next_id\":\"00000000000000000000000000000002\",\"model_scale\":1.0,",
        "\"points\":[],\"scalars\":[],\"curves\":[],\"contacts\":[],",
        "\"trim_views\":[],\"constraints\":[],\"dimensions\":[],\"source_order\":[]}"
    );
    let version_three = version_four
        .replacen("\"version\":4", "\"version\":3", 1)
        .replacen("\"trim_views\":[],", "", 1);
    let version_two = version_three.replacen("\"version\":3", "\"version\":2", 1);
    let version_one = version_three.replacen("\"version\":3", "\"version\":1", 1);
    assert_eq!(document.to_canonical_json().unwrap(), version_four);
    for prior in [&version_one, &version_two, &version_three] {
        assert_eq!(
            SketchDocument::from_json(prior)
                .unwrap()
                .to_canonical_json()
                .unwrap(),
            version_four
        );
    }

    let unsupported = version_four.replacen("\"version\":4", "\"version\":5", 1);
    assert!(matches!(
        SketchDocument::from_json(&unsupported),
        Err(DocumentError::UnsupportedVersion {
            actual: 5,
            expected: 4,
        })
    ));
    let unknown = version_four.replacen('{', "{\"attributes\":[],", 1);
    assert!(SketchDocument::from_json(&unknown).is_err());
}
