// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch::{
    ContactNeighborhood, CurveId, CurveSpan, DocumentArcSweep, DocumentCurveNormalSide,
    DocumentFilletEndpointOrder, DocumentFilletTrimEndpoint, DocumentId, PersistentId,
};
use geosolve_sketch_features::{
    ComputedFeatureAllocatorHighWater, ComputedFeatureCornerId, ComputedFeatureDocument,
    ComputedFeatureDocumentId, ComputedFeatureId, ComputedFeatureLifecycleHighWater,
    ComputedFeatureObjectBootstrap, ComputedFeatureRevision, ComputedFilletParent,
    NativeCurveSpanSource, NewComputedFilletCorner,
};

fn corner(first: u128, second: u128) -> NewComputedFilletCorner {
    let parent = |curve| ComputedFilletParent {
        source: NativeCurveSpanSource {
            span: CurveSpan::line(CurveId(PersistentId::from_u128(curve))),
        },
        picked_parameter: 0.5,
        winding: 0,
        neighborhood: ContactNeighborhood::Interior,
        normal_side: DocumentCurveNormalSide::Left,
        retained_endpoint: DocumentFilletTrimEndpoint::End,
        periodic_anchor: None,
    };
    NewComputedFilletCorner {
        first: parent(first),
        second: parent(second),
        endpoint_order: DocumentFilletEndpointOrder::FirstThenSecond,
        sweep: DocumentArcSweep::CounterClockwise,
    }
}

#[test]
fn exact_feature_objects_reconstruct_independently_of_input_order() {
    let sketch = DocumentId(PersistentId::from_u128(0x8300_0001));
    let document_id = ComputedFeatureDocumentId::from_raw(0x8300_0002);
    let mut original = ComputedFeatureDocument::with_id(sketch, document_id);
    original
        .create_fillet_set("first", 0.5, vec![corner(0x10, 0x20)])
        .unwrap();
    original
        .create_fillet_set("second", 0.75, vec![corner(0x30, 0x40)])
        .unwrap();
    let lifecycle = ComputedFeatureLifecycleHighWater {
        revision: ComputedFeatureRevision::from_raw(original.revision().raw() + 7),
        allocator: ComputedFeatureAllocatorHighWater {
            next_feature_id: ComputedFeatureId::from_raw(
                original.allocator_high_water().next_feature_id.raw() + 11,
            ),
            next_corner_id: ComputedFeatureCornerId::from_raw(
                original.allocator_high_water().next_corner_id.raw() + 13,
            ),
        },
    };
    let mut bootstrap = ComputedFeatureObjectBootstrap::new(
        sketch,
        document_id,
        original.revision(),
        original.allocator_high_water(),
        lifecycle,
    )
    .unwrap();
    for feature in original.features().iter().rev().cloned() {
        bootstrap.push_feature(feature);
    }
    assert_eq!(bootstrap.lifecycle_high_water(), lifecycle);
    let reconstructed = bootstrap.finish().unwrap();
    assert_eq!(reconstructed, original);
    assert_eq!(
        reconstructed.to_json().unwrap(),
        original.to_json().unwrap()
    );
}

#[test]
fn trailing_lifecycle_or_duplicate_feature_rejects() {
    let sketch = DocumentId(PersistentId::from_u128(0x8300_0101));
    let document_id = ComputedFeatureDocumentId::from_raw(0x8300_0102);
    let allocator = ComputedFeatureAllocatorHighWater {
        next_feature_id: ComputedFeatureId::from_raw(2),
        next_corner_id: ComputedFeatureCornerId::from_raw(2),
    };
    assert!(
        ComputedFeatureObjectBootstrap::new(
            sketch,
            document_id,
            ComputedFeatureRevision::from_raw(2),
            allocator,
            ComputedFeatureLifecycleHighWater {
                revision: ComputedFeatureRevision::from_raw(1),
                allocator,
            },
        )
        .is_err()
    );

    let mut original = ComputedFeatureDocument::with_id(sketch, document_id);
    original
        .create_fillet_set("one", 0.5, vec![corner(0x50, 0x60)])
        .unwrap();
    let mut bootstrap = ComputedFeatureObjectBootstrap::new(
        sketch,
        document_id,
        original.revision(),
        original.allocator_high_water(),
        original.lifecycle_high_water(),
    )
    .unwrap();
    bootstrap.push_feature(original.features()[0].clone());
    bootstrap.push_feature(original.features()[0].clone());
    assert!(bootstrap.finish().is_err());
}
