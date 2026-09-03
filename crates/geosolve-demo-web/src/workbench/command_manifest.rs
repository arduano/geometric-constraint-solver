// SPDX-License-Identifier: GPL-3.0-or-later
//! Closed M88 command and sample inventory.
//!
//! Geometry, relation, dimension, feature, and sample entries are derived
//! directly from their owning catalogs.  Auxiliary presentation commands are
//! explicit because they have no domain catalog and need stable identities of
//! their own before the old markup is replaced.

use geosolve_constraint_editor::{EditorTool, GeometryToolFamily, GeometryToolVariant};
use serde::Serialize;

const TOOL_CATALOG_VERSION: u8 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolIconSnapshot {
    pub(crate) key: String,
    pub(crate) svg: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolCommandSnapshot {
    pub(crate) stable_id: String,
    pub(crate) tool_id: String,
    pub(crate) label: String,
    pub(crate) group: String,
    pub(crate) icon: ToolIconSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolSectionSnapshot {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) description: &'static str,
    pub(crate) commands: Vec<ToolCommandSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ToolCatalogSnapshot {
    pub(crate) version: u8,
    pub(crate) select: ToolCommandSnapshot,
    pub(crate) sections: Vec<ToolSectionSnapshot>,
    pub(crate) geometry_role: ToolCommandSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandCategory {
    App,
    Workspace,
    Sketch,
    Constraint,
    Dimension,
    Modify,
    Inspector,
    Display,
    Code,
    Diagnostic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandTier {
    Primary,
    Contextual,
    Advanced,
    Diagnostic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CommandReachability {
    pub(crate) pointer: bool,
    pub(crate) keyboard: bool,
    pub(crate) deliberate_actions: u8,
    pub(crate) hover_required: bool,
}

impl CommandReachability {
    const fn one_action() -> Self {
        Self {
            pointer: true,
            keyboard: true,
            deliberate_actions: 1,
            hover_required: false,
        }
    }

    const fn two_actions() -> Self {
        Self {
            pointer: true,
            keyboard: true,
            deliberate_actions: 2,
            hover_required: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandBinding {
    /// A route present literally in the incoming `index.html`.
    StaticDom {
        attribute: &'static str,
        value: &'static str,
    },
    /// Markup emitted from a closed Rust catalog at render time.
    GeneratedDom {
        attribute: &'static str,
        value: &'static str,
    },
    /// A select/radio option in incoming markup.
    OptionValue {
        control_id: &'static str,
        value: &'static str,
    },
    /// Additive M88 presentation route, intentionally absent from old markup.
    M88Presentation,
    /// A headless contextual variant whose concrete owner/action ID is scene-derived.
    HeadlessContext,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CommandOrigin {
    Select,
    GeometryFamily(GeometryToolFamily),
    GeometryVariant(GeometryToolVariant),
    ConstraintCatalog,
    DimensionCatalog,
    FeatureCatalog,
    OffsetCatalog,
    Auxiliary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CommandManifestEntry {
    pub(crate) stable_id: String,
    pub(crate) label: String,
    pub(crate) category: CommandCategory,
    pub(crate) tier: CommandTier,
    pub(crate) reachability: CommandReachability,
    pub(crate) binding: CommandBinding,
    pub(crate) origin: CommandOrigin,
}

impl CommandManifestEntry {
    fn catalog(
        stable_id: String,
        label: impl Into<String>,
        category: CommandCategory,
        tier: CommandTier,
        reachability: CommandReachability,
        binding: CommandBinding,
        origin: CommandOrigin,
    ) -> Self {
        Self {
            stable_id,
            label: label.into(),
            category,
            tier,
            reachability,
            binding,
            origin,
        }
    }
}

fn tier_for_geometry_family(family: GeometryToolFamily) -> CommandTier {
    match family {
        GeometryToolFamily::Point
        | GeometryToolFamily::Lines
        | GeometryToolFamily::Rectangles
        | GeometryToolFamily::Circles
        | GeometryToolFamily::Arcs => CommandTier::Primary,
        GeometryToolFamily::Ellipses | GeometryToolFamily::Beziers => CommandTier::Contextual,
        _ => CommandTier::Advanced,
    }
}

fn tier_for_constraint(key: &str) -> CommandTier {
    match key {
        "coincident" | "horizontal" | "vertical" | "parallel" | "perpendicular" => {
            CommandTier::Primary
        }
        "lock" | "concentric" | "collinear" | "equal" | "midpoint" | "symmetric" | "tangent" => {
            CommandTier::Contextual
        }
        _ => CommandTier::Advanced,
    }
}

/// Complete command inventory in stable semantic order.
pub(crate) fn command_manifest() -> Vec<CommandManifestEntry> {
    let mut entries = Vec::new();
    entries.push(CommandManifestEntry::catalog(
        "sketch.select".to_owned(),
        "Select",
        CommandCategory::Sketch,
        CommandTier::Primary,
        CommandReachability::one_action(),
        CommandBinding::StaticDom {
            attribute: "data-wb-tool",
            value: "select",
        },
        CommandOrigin::Select,
    ));

    for family in GeometryToolFamily::ALL {
        entries.push(CommandManifestEntry::catalog(
            format!("sketch.family.{}", family.key()),
            super::geometry_palette::family_label(family),
            CommandCategory::Sketch,
            tier_for_geometry_family(family),
            CommandReachability::one_action(),
            CommandBinding::StaticDom {
                attribute: "data-wb-geometry-family",
                value: family.key(),
            },
            CommandOrigin::GeometryFamily(family),
        ));
    }
    for variant in GeometryToolVariant::ALL {
        entries.push(CommandManifestEntry::catalog(
            format!("sketch.variant.{}", variant.key()),
            super::geometry_palette::variant_label(variant),
            CommandCategory::Sketch,
            tier_for_geometry_family(variant.family()),
            CommandReachability::two_actions(),
            CommandBinding::GeneratedDom {
                attribute: "data-wb-geometry-variant",
                value: variant.key(),
            },
            CommandOrigin::GeometryVariant(variant),
        ));
    }

    for (key, label, _) in super::action_surface::CONSTRAINT_ACTIONS {
        entries.push(CommandManifestEntry::catalog(
            format!("constraint.{key}"),
            label,
            CommandCategory::Constraint,
            tier_for_constraint(key),
            CommandReachability::two_actions(),
            CommandBinding::StaticDom {
                attribute: "data-wb-authoring",
                value: key,
            },
            CommandOrigin::ConstraintCatalog,
        ));
    }
    for (key, label, _) in super::action_surface::DIMENSION_ACTIONS {
        entries.push(CommandManifestEntry::catalog(
            format!("dimension.{key}"),
            label,
            CommandCategory::Dimension,
            CommandTier::Primary,
            CommandReachability::two_actions(),
            CommandBinding::StaticDom {
                attribute: "data-wb-authoring",
                value: key,
            },
            CommandOrigin::DimensionCatalog,
        ));
    }
    for (key, label, _) in super::action_surface::FEATURE_ACTIONS {
        entries.push(CommandManifestEntry::catalog(
            format!("modify.{key}"),
            label,
            CommandCategory::Modify,
            CommandTier::Primary,
            CommandReachability::two_actions(),
            CommandBinding::StaticDom {
                attribute: "data-wb-feature",
                value: key,
            },
            CommandOrigin::FeatureCatalog,
        ));
    }
    for (key, label) in super::action_surface::OFFSET_ACTIONS {
        entries.push(CommandManifestEntry::catalog(
            format!("modify.{key}"),
            label,
            CommandCategory::Modify,
            CommandTier::Primary,
            CommandReachability::two_actions(),
            CommandBinding::StaticDom {
                attribute: "data-wb-offset",
                value: key,
            },
            CommandOrigin::OffsetCatalog,
        ));
    }

    append_auxiliary_commands(&mut entries);
    entries
}

/// Static CAD-tool presentation data for non-Rust shells.
///
/// This is intentionally separate from the hot mutable workbench snapshot: a
/// host fetches it once, while every icon and command identity still comes
/// from the same Rust catalogs used by the retained workbench.
// Keeping the four bounded semantic sections together makes the static wire
// catalog auditable against the closed command manifest.
#[allow(clippy::too_many_lines)]
pub(crate) fn tool_catalog() -> ToolCatalogSnapshot {
    let manifest = command_manifest();
    let select = manifest
        .iter()
        .find(|entry| entry.origin == CommandOrigin::Select)
        .map(|entry| {
            tool_command(
                entry,
                "Selection",
                "geometry-select",
                super::icons::geometry_tool_icon_markup(EditorTool::Select),
            )
        })
        .expect("the closed command manifest contains Select");

    let geometry = manifest
        .iter()
        .filter_map(|entry| match entry.origin {
            CommandOrigin::GeometryVariant(variant) => Some(tool_command(
                entry,
                super::geometry_palette::family_label(variant.family()),
                &format!("geometry-{}", variant.key()),
                super::icons::geometry_variant_icon_markup(variant),
            )),
            _ => None,
        })
        .collect();
    let constraints = manifest
        .iter()
        .filter(|entry| entry.origin == CommandOrigin::ConstraintCatalog)
        .map(|entry| {
            let key = tool_id(entry);
            let tool = super::action_surface::authoring_tool_from_key(key)
                .expect("constraint manifest rows retain their typed action");
            tool_command(
                entry,
                constraint_group(key),
                key,
                super::icons::authoring_icon_markup(tool),
            )
        })
        .collect();
    let dimensions = manifest
        .iter()
        .filter(|entry| entry.origin == CommandOrigin::DimensionCatalog)
        .map(|entry| {
            let key = tool_id(entry);
            let tool = super::action_surface::authoring_tool_from_key(key)
                .expect("dimension manifest rows retain their typed action");
            tool_command(
                entry,
                dimension_group(key),
                key,
                super::icons::authoring_icon_markup(tool),
            )
        })
        .collect();
    let modify = manifest
        .iter()
        .filter_map(|entry| match entry.origin {
            CommandOrigin::FeatureCatalog => {
                let key = tool_id(entry);
                let tool = super::action_surface::feature_tool_from_key(key)
                    .expect("feature manifest rows retain their typed action");
                Some(tool_command(
                    entry,
                    "Curve operations",
                    &format!("feature-{key}"),
                    super::icons::feature_icon_markup(tool),
                ))
            }
            CommandOrigin::OffsetCatalog => Some(tool_command(
                entry,
                "Curve operations",
                "modify-offset",
                super::icons::offset_icon_markup(),
            )),
            _ => None,
        })
        .collect();
    let geometry_role_entry = manifest
        .iter()
        .find(|entry| entry.stable_id == "inspector.geometry-role")
        .expect("the closed command manifest contains geometry role");

    ToolCatalogSnapshot {
        version: TOOL_CATALOG_VERSION,
        select,
        sections: vec![
            ToolSectionSnapshot {
                id: "sketch",
                label: "Sketch",
                description: "Draw points, lines, profiles, and advanced curves.",
                commands: geometry,
            },
            ToolSectionSnapshot {
                id: "constraint",
                label: "Constraint",
                description: "Relate geometry by placement, orientation, and continuity.",
                commands: constraints,
            },
            ToolSectionSnapshot {
                id: "dimension",
                label: "Dimension",
                description: "Control linear, circular, and angular measurements.",
                commands: dimensions,
            },
            ToolSectionSnapshot {
                id: "modify",
                label: "Modify",
                description: "Create profile operations without changing drawing mode.",
                commands: modify,
            },
        ],
        geometry_role: tool_command(
            geometry_role_entry,
            "Geometry role",
            "geometry-role-construction",
            super::icons::construction_role_icon_markup(),
        ),
    }
}

fn tool_command(
    entry: &CommandManifestEntry,
    group: &str,
    icon_key: &str,
    svg: String,
) -> ToolCommandSnapshot {
    ToolCommandSnapshot {
        stable_id: entry.stable_id.clone(),
        tool_id: tool_id(entry).to_owned(),
        label: entry.label.clone(),
        group: group.to_owned(),
        icon: ToolIconSnapshot {
            key: icon_key.to_owned(),
            svg,
        },
    }
}

fn tool_id(entry: &CommandManifestEntry) -> &str {
    match &entry.binding {
        CommandBinding::StaticDom { value, .. } | CommandBinding::GeneratedDom { value, .. } => {
            value
        }
        _ => panic!(
            "tool-catalog entry `{}` requires a direct command binding",
            entry.stable_id
        ),
    }
}

fn constraint_group(key: &str) -> &'static str {
    match key {
        "lock" | "coincident" | "concentric" | "collinear" | "midpoint" => "Placement",
        "horizontal" | "vertical" | "parallel" | "perpendicular" => "Orientation",
        "equal" | "symmetric" => "Equality & symmetry",
        "tangent" | "continuity" => "Curve join",
        _ => "Constraints",
    }
}

fn dimension_group(key: &str) -> &'static str {
    match key {
        "point-distance" | "segment-length" => "Linear",
        "radius" | "diameter" => "Circular",
        "oriented-angle" => "Angular",
        _ => "Dimensions",
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed auxiliary inventory stays contiguous so its stable semantic order is reviewable"
)]
fn append_auxiliary_commands(entries: &mut Vec<CommandManifestEntry>) {
    let mut add = |id: &'static str,
                   label: &'static str,
                   category: CommandCategory,
                   tier: CommandTier,
                   depth: u8,
                   binding: CommandBinding| {
        entries.push(CommandManifestEntry::catalog(
            id.to_owned(),
            label,
            category,
            tier,
            if depth == 1 {
                CommandReachability::one_action()
            } else {
                CommandReachability::two_actions()
            },
            binding,
            CommandOrigin::Auxiliary,
        ));
    };

    macro_rules! dom {
        ($id:literal, $label:literal, $category:expr, $tier:expr, $depth:literal,
         $attribute:literal, $value:literal) => {
            add(
                $id,
                $label,
                $category,
                $tier,
                $depth,
                CommandBinding::StaticDom {
                    attribute: $attribute,
                    value: $value,
                },
            )
        };
    }
    macro_rules! generated {
        ($id:literal, $label:literal, $category:expr, $tier:expr, $value:literal) => {
            add(
                $id,
                $label,
                $category,
                $tier,
                2,
                CommandBinding::GeneratedDom {
                    attribute: "data-code-action",
                    value: $value,
                },
            )
        };
    }
    macro_rules! option {
        ($id:literal, $label:literal, $category:expr, $tier:expr,
         $control:literal, $value:literal) => {
            add(
                $id,
                $label,
                $category,
                $tier,
                2,
                CommandBinding::OptionValue {
                    control_id: $control,
                    value: $value,
                },
            )
        };
    }
    macro_rules! m88 {
        ($id:literal, $label:literal, $category:expr, $tier:expr, $depth:literal) => {
            add(
                $id,
                $label,
                $category,
                $tier,
                $depth,
                CommandBinding::M88Presentation,
            )
        };
    }
    macro_rules! headless {
        ($id:literal, $label:literal, $category:expr, $tier:expr) => {
            add(
                $id,
                $label,
                $category,
                $tier,
                2,
                CommandBinding::HeadlessContext,
            )
        };
    }

    // App/project actions.
    m88!(
        "app.file-menu",
        "File / project",
        CommandCategory::App,
        CommandTier::Primary,
        1
    );
    dom!(
        "app.new",
        "New sketch",
        CommandCategory::App,
        CommandTier::Primary,
        1,
        "data-wb-action",
        "new"
    );
    generated!(
        "app.start-from-code",
        "Start from code",
        CommandCategory::App,
        CommandTier::Primary,
        "start-authored"
    );
    dom!(
        "app.undo",
        "Undo",
        CommandCategory::App,
        CommandTier::Primary,
        1,
        "data-wb-action",
        "undo"
    );
    dom!(
        "app.redo",
        "Redo",
        CommandCategory::App,
        CommandTier::Primary,
        1,
        "data-wb-action",
        "redo"
    );
    dom!(
        "app.export-png",
        "Export PNG",
        CommandCategory::App,
        CommandTier::Primary,
        2,
        "data-wb-action",
        "export-png"
    );
    m88!(
        "app.start-open",
        "Start / open",
        CommandCategory::App,
        CommandTier::Primary,
        1
    );

    // M88 workspace shell and rehosted tabs.
    m88!(
        "workspace.mode.design",
        "Design mode",
        CommandCategory::Workspace,
        CommandTier::Primary,
        1
    );
    m88!(
        "workspace.mode.split",
        "Split mode",
        CommandCategory::Workspace,
        CommandTier::Primary,
        1
    );
    m88!(
        "workspace.mode.code",
        "Code mode",
        CommandCategory::Workspace,
        CommandTier::Primary,
        1
    );
    m88!(
        "workspace.explorer.toggle",
        "Toggle Explorer",
        CommandCategory::Workspace,
        CommandTier::Contextual,
        1
    );
    m88!(
        "workspace.explorer.objects",
        "Explorer Objects",
        CommandCategory::Workspace,
        CommandTier::Contextual,
        2
    );
    m88!(
        "workspace.explorer.outline",
        "Explorer Outline",
        CommandCategory::Workspace,
        CommandTier::Contextual,
        2
    );
    m88!(
        "workspace.explorer.intent-ir",
        "Explorer Intent IR",
        CommandCategory::Workspace,
        CommandTier::Advanced,
        2
    );
    m88!(
        "workspace.explorer.history",
        "Explorer History",
        CommandCategory::Workspace,
        CommandTier::Advanced,
        2
    );
    m88!(
        "workspace.context.toggle",
        "Toggle contextual panel",
        CommandCategory::Workspace,
        CommandTier::Contextual,
        1
    );
    m88!(
        "workspace.context.inspector",
        "Inspector tab",
        CommandCategory::Workspace,
        CommandTier::Primary,
        1
    );
    m88!(
        "workspace.context.parameters",
        "Parameters tab",
        CommandCategory::Workspace,
        CommandTier::Primary,
        1
    );
    m88!(
        "workspace.context.problems",
        "Problems tab",
        CommandCategory::Workspace,
        CommandTier::Primary,
        1
    );
    m88!(
        "workspace.code.parameters",
        "Code Parameters",
        CommandCategory::Code,
        CommandTier::Contextual,
        1
    );
    m88!(
        "workspace.code.problems",
        "Code Problems",
        CommandCategory::Code,
        CommandTier::Contextual,
        1
    );
    m88!(
        "workspace.code.generated",
        "Code Generated",
        CommandCategory::Code,
        CommandTier::Advanced,
        1
    );
    m88!(
        "workspace.code.artifacts",
        "Code Artifacts",
        CommandCategory::Code,
        CommandTier::Advanced,
        1
    );
    m88!(
        "workspace.pane.reset-explorer",
        "Reset Explorer width",
        CommandCategory::Workspace,
        CommandTier::Advanced,
        2
    );
    m88!(
        "workspace.pane.reset-split",
        "Reset Split panes",
        CommandCategory::Workspace,
        CommandTier::Contextual,
        2
    );
    m88!(
        "workspace.pane.reset-context",
        "Reset contextual width",
        CommandCategory::Workspace,
        CommandTier::Advanced,
        2
    );

    // Tool completion and fixed option variants.
    dom!(
        "sketch.finish",
        "Finish current tool",
        CommandCategory::Sketch,
        CommandTier::Primary,
        1,
        "data-wb-action",
        "finish"
    );
    dom!(
        "sketch.cancel",
        "Cancel current tool",
        CommandCategory::Sketch,
        CommandTier::Primary,
        1,
        "data-wb-action",
        "cancel"
    );
    option!(
        "constraint.equal.signed",
        "Equal curvature · signed",
        CommandCategory::Constraint,
        CommandTier::Advanced,
        "wb-authoring-curvature",
        "signed"
    );
    option!(
        "constraint.equal.same-sign",
        "Equal curvature · magnitude same sign",
        CommandCategory::Constraint,
        CommandTier::Advanced,
        "wb-authoring-curvature",
        "same-sign"
    );
    option!(
        "constraint.equal.opposite-sign",
        "Equal curvature · magnitude opposite sign",
        CommandCategory::Constraint,
        CommandTier::Advanced,
        "wb-authoring-curvature",
        "opposite-sign"
    );
    option!(
        "constraint.tangent.aligned",
        "Tangent orientation · aligned",
        CommandCategory::Constraint,
        CommandTier::Advanced,
        "wb-authoring-tangent-orientation",
        "aligned"
    );
    option!(
        "constraint.tangent.opposed",
        "Tangent orientation · opposed",
        CommandCategory::Constraint,
        CommandTier::Advanced,
        "wb-authoring-tangent-orientation",
        "opposed"
    );
    option!(
        "constraint.continuity.g0",
        "Continuity · G0",
        CommandCategory::Constraint,
        CommandTier::Advanced,
        "wb-authoring-continuity",
        "g0"
    );
    option!(
        "constraint.continuity.g1",
        "Continuity · G1",
        CommandCategory::Constraint,
        CommandTier::Advanced,
        "wb-authoring-continuity",
        "g1"
    );
    option!(
        "constraint.continuity.g2",
        "Continuity · G2",
        CommandCategory::Constraint,
        CommandTier::Advanced,
        "wb-authoring-continuity",
        "g2"
    );
    option!(
        "constraint.continuity.c2",
        "Continuity · parametric C2",
        CommandCategory::Constraint,
        CommandTier::Advanced,
        "wb-authoring-continuity",
        "c2"
    );
    option!(
        "dimension.mode.driving",
        "Dimension mode · driving",
        CommandCategory::Dimension,
        CommandTier::Contextual,
        "wb-authoring-dimension-mode",
        "driving"
    );
    option!(
        "dimension.mode.reference",
        "Dimension mode · reference",
        CommandCategory::Dimension,
        CommandTier::Contextual,
        "wb-authoring-dimension-mode",
        "reference"
    );
    option!(
        "dimension.angle.counter-clockwise",
        "Angle direction · counter-clockwise",
        CommandCategory::Dimension,
        CommandTier::Advanced,
        "wb-authoring-angle-orientation",
        "counter-clockwise"
    );
    option!(
        "dimension.angle.clockwise",
        "Angle direction · clockwise",
        CommandCategory::Dimension,
        CommandTier::Advanced,
        "wb-authoring-angle-orientation",
        "clockwise"
    );
    option!(
        "sketch.arc.counter-clockwise",
        "Arc sweep · counter-clockwise",
        CommandCategory::Sketch,
        CommandTier::Advanced,
        "wb-conic-arc-sweep",
        "counter-clockwise"
    );
    option!(
        "sketch.arc.clockwise",
        "Arc sweep · clockwise",
        CommandCategory::Sketch,
        CommandTier::Advanced,
        "wb-conic-arc-sweep",
        "clockwise"
    );
    option!(
        "sketch.hyperbola.positive",
        "Hyperbola branch · positive",
        CommandCategory::Sketch,
        CommandTier::Advanced,
        "wb-conic-hyperbola-branch",
        "positive"
    );
    option!(
        "sketch.hyperbola.negative",
        "Hyperbola branch · negative",
        CommandCategory::Sketch,
        CommandTier::Advanced,
        "wb-conic-hyperbola-branch",
        "negative"
    );

    // Modify and explicit Fillet branch actions.
    dom!(
        "modify.fillet.apply-computed",
        "Apply computed Fillet",
        CommandCategory::Modify,
        CommandTier::Contextual,
        2,
        "data-wb-action",
        "feature-apply"
    );
    dom!(
        "modify.fillet.apply-native",
        "Apply native profile Fillet",
        CommandCategory::Modify,
        CommandTier::Advanced,
        2,
        "data-wb-action",
        "feature-apply-native"
    );
    dom!(
        "modify.offset.flip",
        "Flip Offset direction",
        CommandCategory::Modify,
        CommandTier::Contextual,
        2,
        "data-wb-action",
        "offset-flip"
    );
    dom!(
        "modify.offset.apply",
        "Apply Offset",
        CommandCategory::Modify,
        CommandTier::Contextual,
        2,
        "data-wb-action",
        "offset-apply"
    );
    dom!(
        "modify.offset.cancel",
        "Cancel Offset",
        CommandCategory::Modify,
        CommandTier::Contextual,
        2,
        "data-wb-action",
        "offset-cancel"
    );
    headless!(
        "modify.fillet.reverse-first",
        "Reverse first retained direction",
        CommandCategory::Modify,
        CommandTier::Advanced
    );
    headless!(
        "modify.fillet.reverse-second",
        "Reverse second retained direction",
        CommandCategory::Modify,
        CommandTier::Advanced
    );
    headless!(
        "modify.fillet.complementary-arc",
        "Use complementary Fillet arc",
        CommandCategory::Modify,
        CommandTier::Advanced
    );
    headless!(
        "modify.fillet.normal-left-left",
        "Fillet local alternative · left / left",
        CommandCategory::Modify,
        CommandTier::Advanced
    );
    headless!(
        "modify.fillet.normal-left-right",
        "Fillet local alternative · left / right",
        CommandCategory::Modify,
        CommandTier::Advanced
    );
    headless!(
        "modify.fillet.normal-right-left",
        "Fillet local alternative · right / left",
        CommandCategory::Modify,
        CommandTier::Advanced
    );
    headless!(
        "modify.fillet.normal-right-right",
        "Fillet local alternative · right / right",
        CommandCategory::Modify,
        CommandTier::Advanced
    );

    // Inspector routes and exact property/branch families.
    dom!(
        "inspector.clear-selection",
        "Clear selection",
        CommandCategory::Inspector,
        CommandTier::Contextual,
        1,
        "data-wb-action",
        "clear-selection"
    );
    dom!(
        "inspector.delete",
        "Delete selected",
        CommandCategory::Inspector,
        CommandTier::Contextual,
        1,
        "data-wb-action",
        "delete"
    );
    dom!(
        "inspector.geometry-role",
        "Toggle Profile / Construction",
        CommandCategory::Inspector,
        CommandTier::Contextual,
        2,
        "data-wb-action",
        "geometry-role"
    );
    headless!(
        "inspector.geometry-role.profile",
        "Geometry role · Profile",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.geometry-role.construction",
        "Geometry role · Construction",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    dom!(
        "inspector.dimension-target",
        "Update dimension target",
        CommandCategory::Inspector,
        CommandTier::Contextual,
        1,
        "data-wb-action",
        "dimension-target"
    );
    dom!(
        "inspector.profile-offset-flip",
        "Flip accepted Profile Offset",
        CommandCategory::Inspector,
        CommandTier::Advanced,
        2,
        "data-wb-action",
        "profile-offset-flip"
    );
    dom!(
        "inspector.contact-branches",
        "Apply contact branches",
        CommandCategory::Inspector,
        CommandTier::Advanced,
        2,
        "data-wb-action",
        "contact-branches"
    );
    dom!(
        "inspector.angle-orientation",
        "Apply angle orientation",
        CommandCategory::Inspector,
        CommandTier::Advanced,
        2,
        "data-wb-action",
        "angle-orientation"
    );
    dom!(
        "inspector.fillet-radius",
        "Update shared Fillet radius",
        CommandCategory::Inspector,
        CommandTier::Contextual,
        1,
        "data-wb-action",
        "feature-radius"
    );
    dom!(
        "inspector.fillet-suppression",
        "Suppress / restore Fillet",
        CommandCategory::Inspector,
        CommandTier::Advanced,
        2,
        "data-wb-action",
        "feature-suppression"
    );
    headless!(
        "inspector.curve.rational-middle",
        "Apply rational middle control",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.radius",
        "Apply curve radius",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.minor-axis-ratio",
        "Apply minor-axis ratio",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.trim-start",
        "Apply curve trim start",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.trim-end",
        "Apply curve trim end",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.semi-conjugate",
        "Apply hyperbola semi-conjugate",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.rational-weight",
        "Apply rational weight",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.nurbs-weight",
        "Apply NURBS control weight",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.nurbs-gauge",
        "Make NURBS weight gauge",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.sweep",
        "Apply curve sweep",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );
    headless!(
        "inspector.curve.hyperbola-branch",
        "Apply hyperbola branch",
        CommandCategory::Inspector,
        CommandTier::Advanced
    );

    // Canvas display and camera commands.
    dom!(
        "display.options",
        "Toggle grid",
        CommandCategory::Display,
        CommandTier::Contextual,
        1,
        "data-wb-option",
        "construction-display"
    );
    dom!(
        "display.zoom-in",
        "Zoom in",
        CommandCategory::Display,
        CommandTier::Contextual,
        1,
        "data-wb-action",
        "zoom-in"
    );
    dom!(
        "display.zoom-out",
        "Zoom out",
        CommandCategory::Display,
        CommandTier::Contextual,
        1,
        "data-wb-action",
        "zoom-out"
    );
    dom!(
        "display.fit",
        "Fit sketch",
        CommandCategory::Display,
        CommandTier::Contextual,
        1,
        "data-wb-action",
        "zoom-fit"
    );
    dom!(
        "display.origin",
        "Center on origin",
        CommandCategory::Display,
        CommandTier::Contextual,
        1,
        "data-wb-action",
        "zoom-origin"
    );
    option!(
        "display.pick.all",
        "Pick all geometry",
        CommandCategory::Display,
        CommandTier::Advanced,
        "wb-geometry-pick-scope",
        "all"
    );
    option!(
        "display.pick.profile",
        "Pick Profile geometry",
        CommandCategory::Display,
        CommandTier::Advanced,
        "wb-geometry-pick-scope",
        "profile"
    );
    option!(
        "display.pick.construction",
        "Pick Construction geometry",
        CommandCategory::Display,
        CommandTier::Advanced,
        "wb-geometry-pick-scope",
        "construction"
    );
    headless!(
        "display.reference-geometry",
        "Show reference geometry",
        CommandCategory::Display,
        CommandTier::Contextual
    );
    headless!(
        "display.grid",
        "Show grid",
        CommandCategory::Display,
        CommandTier::Contextual
    );
    headless!(
        "display.annotations",
        "Show annotations",
        CommandCategory::Display,
        CommandTier::Contextual
    );
    headless!(
        "display.all-constraints",
        "Show all constraint marks",
        CommandCategory::Display,
        CommandTier::Advanced
    );
    headless!(
        "display.explicit-construction",
        "Show explicit Construction",
        CommandCategory::Display,
        CommandTier::Advanced
    );
    headless!(
        "display.implicit-construction",
        "Show Fillet-hidden portions",
        CommandCategory::Display,
        CommandTier::Advanced
    );
    dom!(
        "display.annotation-reset-selected",
        "Reset selected annotations",
        CommandCategory::Display,
        CommandTier::Advanced,
        2,
        "data-wb-action",
        "annotation-reset-selected"
    );
    dom!(
        "display.annotation-reset-all",
        "Reset all annotations",
        CommandCategory::Display,
        CommandTier::Advanced,
        2,
        "data-wb-action",
        "annotation-reset-all"
    );

    // Every current code action plus M88 source/handoff actions.
    generated!(
        "code.apply",
        "Apply managed source",
        CommandCategory::Code,
        CommandTier::Primary,
        "apply"
    );
    generated!(
        "code.revert",
        "Revert managed source",
        CommandCategory::Code,
        CommandTier::Primary,
        "revert"
    );
    headless!(
        "code.select-file",
        "Select code file",
        CommandCategory::Code,
        CommandTier::Contextual
    );
    dom!(
        "code.export-canonical",
        "Export canonical project",
        CommandCategory::Code,
        CommandTier::Contextual,
        2,
        "data-wb-action",
        "project-export"
    );
    dom!(
        "code.export-source",
        "Export accepted managed source",
        CommandCategory::Code,
        CommandTier::Contextual,
        2,
        "data-wb-action",
        "source-export"
    );
    dom!(
        "code.import-canonical",
        "Import canonical project",
        CommandCategory::Code,
        CommandTier::Contextual,
        2,
        "data-wb-action",
        "project-import"
    );
    dom!(
        "code.download-raw-draft",
        "Download raw draft source",
        CommandCategory::Code,
        CommandTier::Advanced,
        2,
        "data-wb-action",
        "draft-export"
    );

    // Problems and file-first diagnostic transport.
    dom!(
        "diagnostic.problems-open",
        "Open Problems",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        1,
        "data-wb-action",
        "problems"
    );
    dom!(
        "diagnostic.problems-close",
        "Close Problems",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        1,
        "data-wb-action",
        "problems-close"
    );
    dom!(
        "diagnostic.repro-copy",
        "Copy reproduction",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        2,
        "data-wb-action",
        "reproduction-copy"
    );
    dom!(
        "diagnostic.repro-open",
        "Open reproduction input",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        2,
        "data-wb-action",
        "reproduction-open"
    );
    dom!(
        "diagnostic.repro-load",
        "Load reproduction",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        2,
        "data-wb-action",
        "reproduction-load"
    );
    dom!(
        "diagnostic.repro-select",
        "Select reproduction text",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        2,
        "data-wb-action",
        "reproduction-select"
    );
    dom!(
        "diagnostic.repro-close",
        "Close reproduction input",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        2,
        "data-wb-action",
        "reproduction-close"
    );
    dom!(
        "diagnostic.repro-download",
        "Download reproduction file",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        2,
        "data-wb-action",
        "reproduction-download"
    );
    dom!(
        "diagnostic.trace-copy",
        "Copy interaction trace",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        2,
        "data-wb-action",
        "interaction-trace-copy"
    );
    dom!(
        "diagnostic.trace-download",
        "Download interaction trace",
        CommandCategory::Diagnostic,
        CommandTier::Diagnostic,
        2,
        "data-wb-action",
        "interaction-trace-download"
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SampleKind {
    Native,
    Code,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SampleManifestEntry {
    pub(crate) stable_id: String,
    pub(crate) key: &'static str,
    pub(crate) title: &'static str,
    pub(crate) group: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) kind: SampleKind,
    pub(crate) reachability: CommandReachability,
}

/// All 37 user-visible examples in their source-authoritative order. The first
/// 25 retain direct-native constructors only as reference/oracle fixtures.
pub(crate) fn sample_manifest() -> Vec<SampleManifestEntry> {
    let mut samples = Vec::with_capacity(37);
    for group in super::samples::GROUPS {
        for definition in group.samples {
            samples.push(SampleManifestEntry {
                stable_id: format!("sample.code.{}", definition.id.key()),
                key: definition.id.key(),
                title: definition.title,
                group: group.title,
                summary: "Editable parametric sketch sample",
                kind: SampleKind::Code,
                reachability: CommandReachability::two_actions(),
            });
        }
    }
    for demo in geosolve_sketch_code::bundled_code_project_demos() {
        samples.push(SampleManifestEntry {
            stable_id: format!("sample.code.{}", demo.id.key()),
            key: demo.id.key(),
            title: demo.title,
            group: demo.id.semantic_group(),
            summary: demo.summary(),
            kind: SampleKind::Code,
            reachability: CommandReachability::two_actions(),
        });
    }
    samples
}

pub(crate) fn search_samples(query: &str) -> Vec<SampleManifestEntry> {
    let terms = query
        .split_whitespace()
        .map(str::to_lowercase)
        .collect::<Vec<_>>();
    sample_manifest()
        .into_iter()
        .filter(|sample| {
            let searchable = format!(
                "{} {} {} {} {}",
                sample.stable_id, sample.key, sample.title, sample.group, sample.summary
            )
            .to_lowercase();
            terms.iter().all(|term| searchable.contains(term))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn stable_command_and_sample_identities_are_globally_unique() {
        let commands = command_manifest();
        let samples = sample_manifest();
        let mut identities = BTreeSet::new();
        for command in &commands {
            assert!(
                identities.insert(command.stable_id.clone()),
                "duplicate command identity {}",
                command.stable_id
            );
            assert!(!command.label.trim().is_empty());
        }
        for sample in &samples {
            assert!(
                identities.insert(sample.stable_id.clone()),
                "duplicate sample identity {}",
                sample.stable_id
            );
            assert!(!sample.title.trim().is_empty());
        }
    }

    #[test]
    fn authoritative_catalogs_have_one_for_one_manifest_parity() {
        let commands = command_manifest();
        let families = commands
            .iter()
            .filter_map(|entry| match entry.origin {
                CommandOrigin::GeometryFamily(value) => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        let variants = commands
            .iter()
            .filter_map(|entry| match entry.origin {
                CommandOrigin::GeometryVariant(value) => Some(value),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(families, GeometryToolFamily::ALL);
        assert_eq!(variants, GeometryToolVariant::ALL);
        assert_eq!(
            commands
                .iter()
                .filter(|entry| entry.origin == CommandOrigin::ConstraintCatalog)
                .map(|entry| entry.stable_id.as_str())
                .collect::<Vec<_>>(),
            super::super::action_surface::CONSTRAINT_ACTIONS
                .iter()
                .map(|(key, _, _)| format!("constraint.{key}"))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            commands
                .iter()
                .filter(|entry| entry.origin == CommandOrigin::DimensionCatalog)
                .count(),
            5
        );
        assert_eq!(
            commands
                .iter()
                .filter(|entry| entry.origin == CommandOrigin::FeatureCatalog)
                .count(),
            1
        );
        assert_eq!(
            commands
                .iter()
                .filter(|entry| entry.origin == CommandOrigin::OffsetCatalog)
                .count(),
            1
        );

        let samples = sample_manifest();
        assert_eq!(
            samples
                .iter()
                .filter(|sample| sample.kind == SampleKind::Native)
                .count(),
            0
        );
        assert_eq!(
            samples
                .iter()
                .filter(|sample| sample.kind == SampleKind::Code)
                .count(),
            37
        );
        assert_eq!(samples.len(), 37);
        assert_eq!(
            samples
                .iter()
                .filter(|sample| {
                    super::super::samples::SampleId::from_key(sample.key).is_some()
                })
                .map(|sample| sample.key)
                .collect::<Vec<_>>(),
            super::super::samples::SampleId::ALL.map(super::super::samples::SampleId::key)
        );
        assert_eq!(
            samples
                .iter()
                .skip(super::super::samples::SampleId::ALL.len())
                .map(|sample| sample.key)
                .collect::<Vec<_>>(),
            geosolve_sketch_code::bundled_code_project_demos()
                .iter()
                .map(|demo| demo.id.key())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn every_command_and_sample_is_pointer_and_keyboard_reachable_without_hover() {
        for entry in command_manifest() {
            assert!(
                entry.reachability.pointer,
                "{} lacks pointer reach",
                entry.stable_id
            );
            assert!(
                entry.reachability.keyboard,
                "{} lacks keyboard reach",
                entry.stable_id
            );
            assert!(entry.reachability.deliberate_actions <= 2);
            assert!(!entry.reachability.hover_required);
        }
        for sample in sample_manifest() {
            assert!(sample.reachability.pointer);
            assert!(sample.reachability.keyboard);
            assert_eq!(sample.reachability.deliberate_actions, 2);
            assert!(!sample.reachability.hover_required);
        }
    }

    #[test]
    fn auxiliary_inventory_freezes_required_app_code_branch_display_and_m88_routes() {
        let identities = command_manifest()
            .into_iter()
            .filter(|entry| entry.origin == CommandOrigin::Auxiliary)
            .map(|entry| entry.stable_id)
            .collect::<BTreeSet<_>>();
        for required in [
            "app.new",
            "app.start-from-code",
            "app.undo",
            "app.redo",
            "app.export-png",
            "sketch.finish",
            "sketch.cancel",
            "modify.fillet.reverse-first",
            "modify.fillet.normal-right-right",
            "display.annotations",
            "inspector.contact-branches",
            "code.apply",
            "code.revert",
            "code.export-canonical",
            "code.export-source",
            "code.import-canonical",
            "code.download-raw-draft",
            "diagnostic.repro-copy",
            "diagnostic.repro-load",
            "diagnostic.repro-download",
            "diagnostic.trace-copy",
            "diagnostic.trace-download",
            "workspace.mode.design",
            "workspace.mode.split",
            "workspace.mode.code",
            "workspace.explorer.objects",
            "workspace.context.inspector",
            "workspace.code.generated",
        ] {
            assert!(identities.contains(required), "missing {required}");
        }
        assert_eq!(identities.len(), 112, "review additions one for one");
    }

    #[test]
    fn sample_search_is_case_insensitive_across_all_thirty_seven_entries() {
        let samples = sample_manifest();
        assert_eq!(search_samples(""), samples);
        for sample in samples {
            let query = sample.title.to_uppercase();
            let results = search_samples(&query);
            assert!(
                results
                    .iter()
                    .any(|result| result.stable_id == sample.stable_id),
                "uppercase title did not find {}",
                sample.stable_id
            );
            let key_query = sample.key.to_uppercase();
            assert!(
                search_samples(&key_query)
                    .iter()
                    .any(|result| result.stable_id == sample.stable_id),
                "uppercase key did not find {}",
                sample.stable_id
            );
        }
        assert_eq!(search_samples("typed PANEL").len(), 1);
        assert_eq!(search_samples("GRIDFINITY standard PROFILE").len(), 1);
        assert!(search_samples("definitely-not-a-sample").is_empty());
    }
}
