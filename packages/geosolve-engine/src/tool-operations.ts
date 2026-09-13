// SPDX-License-Identifier: GPL-3.0-or-later
import type { AcceptedResult } from "./index.js";
import type { EditableDesign, PreparedAuthoring } from "./session.js";
import type { PointGestureViewport } from "./point-gesture.js";
import { decodePointValue } from "./point-gesture.js";
import type { ToolOperationTool } from "./tool-catalog.js";
export type { ToolOperationTool } from "./tool-catalog.js";
export type ToolOperationOperand =
 | { readonly target: "binding"; readonly symbol: string; readonly binding: number; readonly span: number | null; readonly curve_parameter: number | null; readonly occurrence?: ToolCurveOccurrence }
 | { readonly target: "datum"; readonly datum: "origin" | "x_axis" | "y_axis" };
export interface ToolSelectionBinding { readonly symbol: string; readonly binding: number }
export type ToolCurveOccurrence = { readonly kind: "native" } | {
 readonly kind: "fillet_discarded"; readonly source: ToolSelectionBinding; readonly segment: number;
 readonly interval: readonly [number, number]; readonly feature: ToolSelectionBinding; readonly corner: ToolSelectionBinding;
 readonly endpoint: "start" | "end"; readonly base_interval: readonly [number, number];
};
export interface ToolAuthoringOptions {
 readonly tangent_orientation: "aligned" | "opposed";
 readonly curvature_relation: "signed" | "magnitude_same_sign" | "magnitude_opposite_sign";
 readonly continuity: { readonly kind: "g0" | "g1" | "g2" } | { readonly kind: "parametric_c2"; readonly first_rate: number; readonly second_rate: number };
 readonly dimension_mode: "driving" | "reference";
 readonly angle_orientation: "counter_clockwise" | "clockwise";
}
export interface ToolFilletOptions { readonly fillet_radius: number | null; readonly flip_first_side: boolean; readonly flip_second_side: boolean; readonly alternate_arc: boolean }
export interface ToolOperationOptions { readonly authoring_options?: ToolAuthoringOptions | null; readonly fillet_options?: ToolFilletOptions | null; readonly offset_distance?: number | null }
export type ToolOperationEvent =
 | { readonly event: "viewport"; readonly viewport: PointGestureViewport }
 | { readonly event: "move" | "click"; readonly position: readonly [number, number] }
 | { readonly event: "pick"; readonly operand: ToolOperationOperand }
 | { readonly event: "pick_selection"; readonly operands: readonly ToolOperationOperand[] }
 | { readonly event: "complete" | "step_back" | "reset" | "offset_flip" }
 | { readonly event: "authoring_options"; readonly options: ToolAuthoringOptions }
 | { readonly event: "fillet_options"; readonly options: ToolFilletOptions; readonly selected_corner: number | null }
 | { readonly event: "fillet_radius"; readonly radius: number }
 | { readonly event: "offset_distance"; readonly distance: number };
export interface ToolOperationSample { readonly sequence: number; readonly input: ToolOperationEvent }
export interface ToolOperationCommand { readonly basis: string; readonly gesture_id: number; readonly viewport: PointGestureViewport; readonly tool: ToolOperationTool; readonly selection: readonly ToolOperationOperand[]; readonly options?: ToolOperationOptions; readonly samples: readonly ToolOperationSample[]; readonly expected_declarations: readonly Readonly<Record<string, unknown>>[]; readonly expected_mutation?: Readonly<Record<string, unknown>> }
export interface ToolOperationFrame { readonly sequence: number; readonly completed: boolean; readonly can_finish: boolean; readonly has_pending: boolean; readonly can_reset: boolean; readonly can_step_back: boolean; readonly diagnostic: string | null; readonly pending: readonly ToolOperationOperand[]; readonly authoring_options: ToolAuthoringOptions; readonly fillet_options: ToolFilletOptions; readonly fillet_corner_count: number; readonly fillet_corners: readonly { readonly index: number; readonly options: ToolFilletOptions }[]; readonly offset_distance: number | null }
/** Exact independently replayed server compiler preparation. */
export interface PreparedToolOperation {
  readonly ticket: string;
  readonly request: PreparedAuthoring["request"];
  readonly declarations: readonly string[];
}
export interface ToolOperationReplayWitness {
  readonly requiredStableDeclarations: readonly string[];
  readonly allocationMapping: readonly {
    readonly provisional: string; readonly persistent: string;
    readonly provisionalVariable: string; readonly persistentVariable: string;
  }[];
}
export interface PreparedToolOperationReplay extends PreparedToolOperation {
  readonly replay: ToolOperationReplayWitness;
}
/** Native-validated complete candidate; persist before installing through its preparing session. */
export interface PreparedToolOperationCommit {
  readonly ticket: string;
  readonly project: string;
  readonly design: EditableDesign;
  readonly source_design_digest: string;
  readonly result: AcceptedResult;
  readonly declarations: readonly string[];
}
export interface ToolOperationNativeHandle {
  editableToolOperationContext(json: string): string;
  editableToolOperationViewOperands(json: string): string;
  editableToolOperationOperands(json: string): string;
  beginEditableToolOperation(json: string): string;
  advanceEditableToolOperation(json: string): string;
  editableToolOperationScene(json: string): string;
  editableToolOperationPresentation(json: string): string;
  finishEditableToolOperation(json: string): string;
  cancelEditableToolOperation(json: string): void;
  prepareEditableToolOperationReplay?(json: string): string;
  prepareEditableToolOperation(json: string): string;
  resolveEditableToolOperation(json: string): string;
  applyEditableToolOperationCommit(json: string): string;
  releaseEditableToolOperation(json: string): void;
}

/** Own this retained native draft in an authoring worker separate from navigation. */
export class ToolOperationPrediction {
  private consumed = false;
  constructor(readonly initialFrame: ToolOperationFrame, private readonly native: ToolOperationNativeHandle, private readonly assertSessionLive: () => void,
    private readonly routing: { readonly session: number; readonly ticket: string; readonly gesture_id: number }) {}
  advance(sample: ToolOperationSample): ToolOperationFrame {
    this.assertLive();
    return decodePointValue(this.native.advanceEditableToolOperation(JSON.stringify({ ...this.routing, sample })));
  }
  /** Render this detached scene together with the frame's preview and inference guides. */
  presentationJSON(): string {
    this.assertLive(); return this.native.editableToolOperationPresentation(JSON.stringify(this.routing));
  }
  /** Detached geometry without namespace correspondence. */
  sceneJSON(): string { this.assertLive(); return this.native.editableToolOperationScene(JSON.stringify(this.routing)); }
  finish(): ToolOperationCommand {
    this.assertLive();
    try { return decodePointValue(this.native.finishEditableToolOperation(JSON.stringify(this.routing))); }
    finally { this.consumed = true; }
  }
  cancel(): void {
    if (this.consumed) return;
    this.assertSessionLive(); this.native.cancelEditableToolOperation(JSON.stringify(this.routing)); this.consumed = true;
  }
  private assertLive(): void {
    this.assertSessionLive(); if (this.consumed) throw Error("ToolOperation is cancelled or already consumed");
  }
}
