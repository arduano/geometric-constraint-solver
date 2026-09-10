// SPDX-License-Identifier: GPL-3.0-or-later
import type { AcceptedResult } from "./index.js";
import type { EditableDesign } from "./session.js";
import type { SemanticPathSegment } from "@geosolve/sketch-code/ir";

export type PointOwnerAddress =
  | { readonly owner: "direct_declaration"; readonly declaration: string }
  | { readonly owner: "generated_member"; readonly address: {
      readonly invocation: string; readonly template: readonly string[];
      readonly member_key: readonly string[]; readonly output: readonly string[];
    } };

export interface PointWritableAddress {
  readonly project: string;
  readonly owner: {
    readonly address: PointOwnerAddress;
    readonly allocation: number;
    readonly generation: number;
  };
  readonly output: readonly SemanticPathSegment[];
  readonly field: "point";
}
export type PointGestureTarget =
  | { readonly target: "point"; readonly address: PointWritableAddress }
  | { readonly target: "rectangle_corner"; readonly lower_left: PointWritableAddress; readonly upper_right: PointWritableAddress;
      readonly corner: "lower_left" | "lower_right" | "upper_right" | "upper_left" };
export interface PointGestureViewport {
  readonly screen_size: readonly [number, number];
  readonly model_center: readonly [number, number];
  readonly pixels_per_model_unit: number;
}
export interface PointGestureHandle { readonly target: PointGestureTarget; readonly position: readonly [number, number] }
export interface PointGestureSample { readonly sequence: number; readonly position: readonly [number, number] }
export interface PointGestureCommand {
  readonly basis: string;
  readonly gesture_id: number;
  readonly target: PointGestureTarget;
  readonly viewport: PointGestureViewport;
  readonly samples: readonly PointGestureSample[];
}
export interface PointGestureFrame {
  readonly sequence: number;
  readonly accepted: boolean;
  readonly accepted_position: readonly [number, number];
  readonly work: {
    readonly native_preview_attempts: number;
    readonly intent_materialization_attempts: number;
    readonly computed_evaluation_attempts: number;
    readonly history_publications: number;
  };
}
export interface PointGestureTerminal {
  readonly command: PointGestureCommand;
  readonly accepted_position: readonly [number, number];
}
/** Unpublished candidate. Only the preparing session owns its native install ticket. */
export interface PreparedPointGestureCommit {
  readonly ticket: string;
  readonly project: string;
  readonly design: EditableDesign;
  readonly source_design_digest: string;
  readonly result: AcceptedResult;
}
export interface PointGestureNativeHandle {
  editablePointGestureTargets(id: string): string;
  beginEditablePointGesture(json: string): string;
  advanceEditablePointGesture(json: string): string;
  editablePointGestureScene(json: string): string;
  finishEditablePointGesture(json: string): string;
  cancelEditablePointGesture(json: string): void;
  prepareEditablePointCommit(json: string): string;
  applyEditablePointCommit(json: string): string;
  releaseEditablePointCommit(json: string): void;
}

/** Isolated local prediction; sceneJSON never conveys server acceptance authority.
 * Long solves belong in a dedicated authoring worker, separate from navigation.
 */
export class RetainedPointGesture {
  private consumed = false;
  constructor(
    private readonly native: PointGestureNativeHandle,
    private readonly assertSessionLive: () => void,
    private readonly routing: { readonly session: number; readonly ticket: string; readonly gesture_id: number },
  ) {}
  advance(sample: PointGestureSample): PointGestureFrame {
    this.assertLive();
    return decodePointValue(this.native.advanceEditablePointGesture(JSON.stringify({ ...this.routing, sample })));
  }
  /** Detached JSON consumed only by the presentation adapter. */
  sceneJSON(): string {
    this.assertLive(); return this.native.editablePointGestureScene(JSON.stringify(this.routing));
  }
  finish(): PointGestureTerminal {
    this.assertLive();
    // Native terminal failure consumes the fork too; accepted source stays untouched.
    try { return decodePointValue(this.native.finishEditablePointGesture(JSON.stringify(this.routing))); }
    finally { this.consumed = true; }
  }
  cancel(): void {
    if (this.consumed) return;
    this.assertSessionLive();
    this.native.cancelEditablePointGesture(JSON.stringify(this.routing)); this.consumed = true;
  }
  private assertLive(): void {
    this.assertSessionLive();
    if (this.consumed) throw Error("Point gesture is cancelled or already consumed");
  }
}

export function decodePointValue<T>(json: string): T { return freeze(JSON.parse(json)) as T; }
function freeze(value: unknown): unknown {
  if (value !== null && typeof value === "object" && !Object.isFrozen(value)) {
    Object.values(value).forEach(freeze); Object.freeze(value);
  }
  return value;
}
