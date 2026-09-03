// SPDX-License-Identifier: GPL-3.0-or-later
import type { SVGProps } from "react";
import type { ToolIconDefinition } from "../lib/tool-catalog";
import { cn } from "../lib/cn";

type IconElement =
  | { tag: "path"; props: SVGProps<SVGPathElement> }
  | { tag: "circle"; props: SVGProps<SVGCircleElement> }
  | { tag: "rect"; props: SVGProps<SVGRectElement> }
  | { tag: "ellipse"; props: SVGProps<SVGEllipseElement> };

const parsed = new Map<string, IconElement[]>();
const ALLOWED_ATTRIBUTES: Record<IconElement["tag"], ReadonlySet<string>> = {
  path: new Set(["d", "stroke-dasharray"]),
  circle: new Set(["cx", "cy", "r"]),
  rect: new Set(["x", "y", "width", "height", "rx"]),
  ellipse: new Set(["rx", "ry"]),
};

/** Renders the closed Rust SVG vocabulary as ordinary React elements. */
export function ToolIcon({ icon, className }: { icon: ToolIconDefinition; className?: string }) {
  const elements = parseElements(icon);
  return (
    <svg
      aria-hidden="true"
      className={cn("size-5 shrink-0 fill-none stroke-current [stroke-linecap:round] [stroke-linejoin:round] [stroke-width:1.65]", className)}
      data-icon-key={icon.key}
      focusable="false"
      viewBox="-10 -10 20 20"
    >
      {elements.map((element, index) => {
        switch (element.tag) {
          case "path": return <path key={index} {...element.props} />;
          case "circle": return <circle key={index} {...element.props} />;
          case "rect": return <rect key={index} {...element.props} />;
          case "ellipse": return <ellipse key={index} {...element.props} />;
        }
      })}
    </svg>
  );
}
function parseElements(icon: ToolIconDefinition) {
  const cached = parsed.get(icon.key);
  if (cached) return cached;
  const elements: IconElement[] = [];
  const elementPattern = /<(path|circle|rect|ellipse)\b([^>]*)\/>/g;
  for (const match of icon.svg.matchAll(elementPattern)) {
    const tag = match[1] as IconElement["tag"];
    const attributes: Record<string, string> = {};
    for (const attribute of match[2].matchAll(/([a-z-]+)="([^"]*)"/g)) {
      if (!ALLOWED_ATTRIBUTES[tag].has(attribute[1])) throw new Error(`Unsupported ${tag} attribute in tool icon ${icon.key}: ${attribute[1]}`);
      attributes[attribute[1] === "stroke-dasharray" ? "strokeDasharray" : attribute[1]] = attribute[2];
    }
    elements.push({ tag, props: attributes } as IconElement);
  }
  if (!elements.length) throw new Error(`Tool icon contains no renderable geometry: ${icon.key}`);
  parsed.set(icon.key, elements);
  return elements;
}
