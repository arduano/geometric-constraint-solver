// SPDX-License-Identifier: GPL-3.0-or-later
import type { AcceptedResult } from "./index.js";
import type { EditableDesign, PreparedAuthoring } from "./session.js";
import type { PointGestureViewport } from "./point-gesture.js";
import { decodePointValue } from "./point-gesture.js";

export type ConstructionTool =
  | "sketch_point"
  | "segment"
  | "polyline"
  | "midpoint_line"
  | "two_point_aligned_rectangle"
  | "three_point_corner_rectangle"
  | "center_rectangle"
  | "three_point_center_rectangle"
  | "center_radius_circle"
  | "two_point_diameter_circle"
  | "three_point_circle"
  | "center_arc"
  | "three_point_arc"
  | "tangent_arc"
  | "center_axes_ellipse"
  | "axis_endpoints_ellipse"
  | "center_axes_elliptical_arc"
  | "axis_endpoints_elliptical_arc"
  | "quadratic_bezier"
  | "cubic_bezier"
  | "rational_quadratic_conic"
  | "parabola"
  | "hyperbola"
  | "open_control_nurbs"
  | "periodic_control_nurbs";
export interface ConstructionConicOptions {
  readonly minor_axis_ratio: number;
  readonly arc_start: number;
  readonly arc_end: number;
  readonly arc_sweep: "clockwise" | "counter_clockwise";
  readonly middle_weight: number;
  readonly trim_start: number;
  readonly trim_end: number;
  readonly semi_conjugate: number;
  readonly hyperbola_branch: "positive" | "negative";
}
export interface ConstructionNurbsOptions {
  readonly form: "clamped" | "periodic";
  readonly degree: number;
  readonly weights: readonly number[];
  readonly gauge_index: number;
}
export type ConstructionEvent =
  | { readonly event: "viewport"; readonly viewport: PointGestureViewport }
  | { readonly event: "move" | "click"; readonly position: readonly [number, number]; readonly suppressed: boolean; readonly regularized: boolean }
  | { readonly event: "complete" | "step_back" | "reset" | "flip_branch" | "cycle_inference" }
  | { readonly event: "conic_options"; readonly options: ConstructionConicOptions }
  | { readonly event: "nurbs_options"; readonly options: ConstructionNurbsOptions };
export interface ConstructionSample { readonly sequence: number; readonly input: ConstructionEvent }
export interface ConstructionCommand {
  readonly basis: string;
  readonly gesture_id: number;
  readonly viewport: PointGestureViewport;
  readonly tool: ConstructionTool;
  readonly role: "profile" | "construction";
  readonly samples: readonly ConstructionSample[];
  /** Source-level resolved operand/branch witnesses; server recomputes rather than trusting them. */
  readonly expected_declarations: readonly Readonly<Record<string, unknown>>[];
}
export type ConstructionCurveKind = "quadratic_bezier" | "cubic_bezier" | "ellipse" | "elliptical_arc" | "rational_quadratic_conic" | "parabola" | "hyperbola" | "nurbs";
export type ConstructionGuide =
  | { readonly kind: "point"; readonly position: readonly [number, number] }
  | { readonly kind: "polyline"; readonly points: readonly (readonly [number, number])[]; readonly closed: boolean }
  | { readonly kind: "rectangle"; readonly first: readonly [number, number]; readonly second: readonly [number, number] }
  | { readonly kind: "circle"; readonly center: readonly [number, number]; readonly radius: number }
  | { readonly kind: "arc_radius"; readonly center: readonly [number, number]; readonly start: readonly [number, number] }
  | { readonly kind: "elliptical_arc_support"; readonly center: readonly [number, number]; readonly major_axis_point: readonly [number, number]; readonly support_points: readonly (readonly [number, number])[]; readonly trim_start: readonly [number, number] | null }
  | { readonly kind: "control_polygon"; readonly curve_kind: ConstructionCurveKind; readonly points: readonly (readonly [number, number])[] }
  | { readonly kind: "circular_arc"; readonly center: readonly [number, number]; readonly start: readonly [number, number]; readonly end: readonly [number, number]; readonly radius: number; readonly sweep_radians: number; readonly large_arc: boolean; readonly sweep: "clockwise" | "counter_clockwise" }
  | { readonly kind: "advanced_curve"; readonly curve_kind: ConstructionCurveKind; readonly control_points: readonly (readonly [number, number])[]; readonly curve_points: readonly (readonly [number, number])[] };
export interface ConstructionFrame {
  readonly sequence: number;
  readonly completed: boolean;
  readonly can_finish: boolean;
  readonly has_pending: boolean;
  readonly can_reset: boolean;
  readonly can_step_back: boolean;
  readonly can_cycle_inference: boolean;
  readonly can_flip_branch: boolean;
  readonly stage: string | null;
  readonly conic_options: ConstructionConicOptions;
  readonly nurbs_options: ConstructionNurbsOptions;
  readonly preview: ConstructionGuide | null;
  readonly inference_guides: readonly ConstructionGuide[];
  readonly adjusted_position: readonly [number, number] | null;
  readonly diagnostic: string | null;
}
/** Exact independently replayed server compiler preparation. */
export interface PreparedConstruction {
  readonly ticket: string;
  readonly request: PreparedAuthoring["request"];
  readonly declarations: readonly string[];
}
export interface ConstructionReplayWitness {
  readonly requiredStableDeclarations: readonly string[];
  readonly allocationMapping: readonly {
    readonly provisional: string; readonly persistent: string;
    readonly provisionalVariable: string; readonly persistentVariable: string;
  }[];
}
export interface PreparedConstructionReplay extends PreparedConstruction {
  readonly replay: ConstructionReplayWitness;
}
/** Native-validated complete candidate; persist before installing through its preparing session. */
export interface PreparedConstructionCommit {
  readonly ticket: string;
  readonly project: string;
  readonly design: EditableDesign;
  readonly source_design_digest: string;
  readonly result: AcceptedResult;
  readonly declarations: readonly string[];
}
export interface ConstructionNativeHandle {
  beginEditableConstruction(json: string): string;
  advanceEditableConstruction(json: string): string;
  editableConstructionScene(json: string): string;
  editableConstructionPresentation(json: string): string;
  finishEditableConstruction(json: string): string;
  cancelEditableConstruction(json: string): void;
  prepareEditableConstructionReplay?(json: string): string;
  prepareEditableConstruction(json: string): string;
  resolveEditableConstruction(json: string): string;
  applyEditableConstructionCommit(json: string): string;
  releaseEditableConstruction(json: string): void;
}

/** Own this retained native draft in an authoring worker separate from navigation. */
export class ConstructionPrediction {
  private consumed = false;
  constructor(readonly initialFrame: ConstructionFrame, private readonly native: ConstructionNativeHandle, private readonly assertSessionLive: () => void,
    private readonly routing: { readonly session: number; readonly ticket: string; readonly gesture_id: number }) {}
  advance(sample: ConstructionSample): ConstructionFrame {
    this.assertLive();
    return decodePointValue(this.native.advanceEditableConstruction(JSON.stringify({ ...this.routing, sample })));
  }
  /** Render this detached scene together with the frame's preview and inference guides. */
  presentationJSON(): string {
    this.assertLive(); return this.native.editableConstructionPresentation(JSON.stringify(this.routing));
  }
  /** Detached geometry without namespace correspondence. */
  sceneJSON(): string { this.assertLive(); return this.native.editableConstructionScene(JSON.stringify(this.routing)); }
  finish(): ConstructionCommand {
    this.assertLive();
    try { return decodePointValue(this.native.finishEditableConstruction(JSON.stringify(this.routing))); }
    finally { this.consumed = true; }
  }
  cancel(): void {
    if (this.consumed) return;
    this.assertSessionLive(); this.native.cancelEditableConstruction(JSON.stringify(this.routing)); this.consumed = true;
  }
  private assertLive(): void {
    this.assertSessionLive(); if (this.consumed) throw Error("Construction is cancelled or already consumed");
  }
}
