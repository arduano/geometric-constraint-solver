// SPDX-License-Identifier: GPL-3.0-or-later

export const TOOL_CATALOG_VERSION = 1 as const;

export interface ToolIconDefinition {
  key: string;
  svg: string;
}

export interface ToolCommandDefinition {
  stableId: string;
  toolId: string;
  label: string;
  group: string;
  icon: ToolIconDefinition;
}

export interface ToolSectionDefinition {
  id: "sketch" | "constraint" | "dimension" | "modify";
  label: string;
  description: string;
  commands: ToolCommandDefinition[];
}

export interface ToolCatalog {
  version: typeof TOOL_CATALOG_VERSION;
  select: ToolCommandDefinition;
  sections: ToolSectionDefinition[];
  geometryRole: ToolCommandDefinition;
}

/** Rejects malformed or unexpectedly active markup at the one-time Rust capability boundary. */
export function assertToolCatalog(value: ToolCatalog): ToolCatalog {
  if (value.version !== TOOL_CATALOG_VERSION || !Array.isArray(value.sections)) {
    throw new Error("Unsupported or malformed tool catalog");
  }
  const sections = new Set(["sketch", "constraint", "dimension", "modify"]);
  const commands = [value.select, value.geometryRole, ...value.sections.flatMap((section) => {
    if (!sections.delete(section.id) || !section.label.trim() || !section.description.trim() || !Array.isArray(section.commands)) {
      throw new Error("Tool catalog contains an invalid or duplicate section");
    }
    return section.commands;
  })];
  if (sections.size) throw new Error("Tool catalog is missing a required section");

  const stableIds = new Set<string>();
  const toolIds = new Set<string>();
  for (const command of commands) {
    if (![command.stableId, command.toolId, command.label, command.group, command.icon?.key, command.icon?.svg].every((field) => typeof field === "string" && field.trim())) {
      throw new Error("Tool catalog contains incomplete command metadata");
    }
    if (stableIds.has(command.stableId) || toolIds.has(command.toolId)) throw new Error(`Tool catalog contains duplicate command identity: ${command.toolId}`);
    stableIds.add(command.stableId);
    toolIds.add(command.toolId);
    assertStaticIcon(command.icon);
  }
  return value;
}

function assertStaticIcon(icon: ToolIconDefinition) {
  const svg = icon.svg;
  if (!svg.startsWith('<svg class="wb-palette-icon"') || !svg.endsWith("</svg>") || !svg.includes(`data-icon-key="${icon.key}"`)) {
    throw new Error(`Tool icon does not match its declared key: ${icon.key}`);
  }
  if (/<(?:script|style|text|image|use|foreignObject)\b|\bon[a-z]+\s*=|\b(?:href|style)\s*=|url\s*\(/i.test(svg)) {
    throw new Error(`Tool icon contains unsupported active markup: ${icon.key}`);
  }
  const tags = svg.match(/<\/?([a-zA-Z][\w-]*)\b/g)?.map((tag) => tag.replace(/^<\/?/, "")) ?? [];
  if (tags.some((tag) => !["svg", "path", "circle", "rect", "ellipse"].includes(tag))) {
    throw new Error(`Tool icon contains an unsupported element: ${icon.key}`);
  }
}

export function toolLabel(catalog: ToolCatalog, toolId: string) {
  if (catalog.select.toolId === toolId) return catalog.select.label;
  if (catalog.geometryRole.toolId === toolId) return catalog.geometryRole.label;
  return catalog.sections.flatMap((section) => section.commands).find((command) => command.toolId === toolId)?.label ?? toolId;
}

export function toolSection(catalog: ToolCatalog, toolId: string) {
  return catalog.sections.find((section) => section.commands.some((command) => command.toolId === toolId));
}
