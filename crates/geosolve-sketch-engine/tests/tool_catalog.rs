// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{AuthoringTool, GeometryToolVariant, ModifyTool};
use geosolve_sketch_engine::{ConstructionTool, ToolOperationTool};

#[test]
fn native_tool_projections_preserve_every_existing_wire_command() {
    assert_eq!(
        ConstructionTool::ALL.map(ConstructionTool::variant),
        GeometryToolVariant::ALL
    );
    for tool in ConstructionTool::ALL {
        let wire = serde_json::to_value(tool).unwrap();
        assert_eq!(wire, tool.variant().key().replace('-', "_"));
        assert_eq!(
            serde_json::from_value::<ConstructionTool>(wire).unwrap(),
            tool
        );
    }
    let tools = ToolOperationTool::ALL
        .into_iter()
        .chain([ToolOperationTool::ToggleGeometryRole]);
    let keys = AuthoringTool::ALL
        .into_iter()
        .map(|tool| tool.key().replace('-', "_"))
        .chain(ModifyTool::ALL.into_iter().map(ModifyTool::command_key));
    assert_eq!(tools.clone().count(), 21);
    assert_eq!(tools.clone().count(), keys.clone().count());
    for (tool, key) in tools.zip(keys) {
        let wire = serde_json::to_value(tool).unwrap();
        assert_eq!(wire, key);
        assert_eq!(
            serde_json::from_value::<ToolOperationTool>(wire).unwrap(),
            tool
        );
    }
    assert!(!ToolOperationTool::ALL.contains(&ToolOperationTool::ToggleGeometryRole));
    for invalid in ["unknown", "fixed", "point-distance", "toggle-geometry-role"] {
        assert!(serde_json::from_value::<ToolOperationTool>(invalid.into()).is_err());
    }
    assert!(serde_json::from_value::<ConstructionTool>("sketch-point".into()).is_err());
}
