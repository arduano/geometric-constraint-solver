// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch_intent::{
    GeometryRecipeKind, InputRole, InputSlot, IntentKey, IntentNodeDraft, IntentNodeKind,
    IntentPortRole, IntentPortSelector, PatchPortRef,
};

#[test]
fn distinct_alias_inputs_keep_their_output_slots_in_a_draft_probe() {
    let midpoint = InputSlot::new(InputRole::Point, 0);
    let end = InputSlot::new(InputRole::Point, 1);
    let draft = IntentNodeDraft::new(
        IntentNodeKind::Geometry {
            recipe: GeometryRecipeKind::MidpointLine,
        },
        IntentKey::new("midpointLine").unwrap(),
    )
    .with_input(
        midpoint,
        PatchPortRef::Alias {
            node: IntentKey::new("midpoint").unwrap(),
            selector: IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
        },
    )
    .with_input(
        end,
        PatchPortRef::Alias {
            node: IntentKey::new("end").unwrap(),
            selector: IntentPortSelector::Node {
                role: IntentPortRole::Primary,
                index: 0,
            },
        },
    );

    let outputs = draft.output_descriptors().unwrap();
    let alias_for = |role| {
        outputs
            .iter()
            .find(|output| output.selector == IntentPortSelector::Node { role, index: 0 })
            .unwrap()
            .alias_input
    };
    assert_eq!(alias_for(IntentPortRole::Midpoint), Some(midpoint));
    assert_eq!(alias_for(IntentPortRole::End), Some(end));
    assert_eq!(alias_for(IntentPortRole::Start), None);
}
