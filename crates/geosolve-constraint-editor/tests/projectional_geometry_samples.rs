// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstructionPoint, ConstructionProposal, GeometryToolVariant, ProjectionalGeometrySamples,
    projectional_geometry_draft_from_plan, projectional_geometry_plan_from_samples,
};
use geosolve_sketch_intent::{
    GeometryRecipeKind, InputRole, InputSlot, IntentKey, IntentPortKind, IntentPortRole,
    IntentPortSelector, LeafField,
};

#[test]
fn diameter_samples_derive_one_native_center_without_aliasing_sample_handles() {
    let plan = projectional_geometry_plan_from_samples(
        GeometryToolVariant::TwoPointDiameterCircle,
        &ProjectionalGeometrySamples::profile(vec![[0.0, 0.0], [4.0, 0.0]]),
    )
    .expect("finite separated diameter samples form one exact circle plan");
    assert_eq!(
        plan.proposal,
        ConstructionProposal::Circle {
            center: ConstructionPoint::New([2.0, 0.0]),
            radius: 2.0,
        }
    );
    assert_eq!(plan.curve_roles.len(), 1);

    let projected = projectional_geometry_draft_from_plan(
        IntentKey::new("diameterCircle").unwrap(),
        GeometryToolVariant::TwoPointDiameterCircle,
        &plan,
    )
    .expect("the construction plan produces one canonical Intent draft");
    let draft = projected.draft;
    assert_eq!(
        draft.kind,
        geosolve_sketch_intent::IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::TwoPointDiameterCircle,
        }
    );
    let outputs = draft
        .output_descriptors()
        .expect("the derived draft has a valid native output contract");
    for role in [IntentPortRole::Start, IntentPortRole::End] {
        let sample = outputs
            .iter()
            .find(|output| output.selector == IntentPortSelector::Node { role, index: 0 })
            .expect("diameter sample handle is projected");
        assert_eq!(sample.kind, IntentPortKind::HandlePoint);
        assert!(sample.writable.is_empty());
        assert_eq!(sample.alias_input, None);
    }
    let center = outputs
        .iter()
        .find(|output| {
            output.selector
                == IntentPortSelector::Node {
                    role: IntentPortRole::Center,
                    index: 0,
                }
        })
        .expect("diameter circle owns its derived center");
    assert_eq!(center.kind, IntentPortKind::Point);
    assert_eq!(center.writable, [LeafField::X, LeafField::Y]);
    assert_eq!(center.alias_input, None);
    assert!(
        !draft
            .inputs
            .contains_key(&InputSlot::new(InputRole::Point, 0))
    );
    assert!(
        !draft
            .inputs
            .contains_key(&InputSlot::new(InputRole::Point, 1))
    );
}

#[test]
fn aligned_rectangle_publishes_authored_and_derived_native_corner_seeds() {
    let plan = projectional_geometry_plan_from_samples(
        GeometryToolVariant::TwoPointAlignedRectangle,
        &ProjectionalGeometrySamples::profile(vec![[2.0, 3.0], [8.0, 11.0]]),
    )
    .expect("two separated corner samples form one exact rectangle plan");
    let projected = projectional_geometry_draft_from_plan(
        IntentKey::new("rectangle").unwrap(),
        GeometryToolVariant::TwoPointAlignedRectangle,
        &plan,
    )
    .expect("the rectangle plan produces one canonical Intent draft");

    let corner = |index| IntentPortSelector::Node {
        role: IntentPortRole::Corner,
        index,
    };
    assert_eq!(
        projected.point_seeds,
        [
            (corner(0), [2.0, 3.0]),
            (corner(1), [8.0, 3.0]),
            (corner(2), [8.0, 11.0]),
            (corner(3), [2.0, 11.0]),
        ]
        .into_iter()
        .collect()
    );
}
