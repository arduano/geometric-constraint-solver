// SPDX-License-Identifier: GPL-3.0-or-later
import type { EditableSessionToken } from "./session.js";

/** Native identities are opaque to hosts; correspondence is resolved by Rust. */
export interface PresentationBindings {
  readonly document: unknown;
  readonly nodes: Readonly<Record<string, readonly Readonly<Record<string, unknown>>[]>>;
}
export interface AcceptedInspection {
  readonly resultId: string;
  readonly scene: string;
  readonly bindings: PresentationBindings;
  readonly intent: Readonly<Record<string, unknown>>;
}
export interface ManagedNavigationIndex {
  readonly source: string;
  readonly source_digest: string;
  readonly blocked_reason: string | null;
  readonly entries: readonly {
    readonly id: string;
    readonly nodes: readonly unknown[];
    readonly exact_bindings: readonly Readonly<Record<string, unknown>>[] | null;
    readonly source_start: number;
    readonly source_end: number;
  }[];
}
/** A read-only projection of one exact accepted source/native authority. */
export interface EditableInspection {
  readonly token: EditableSessionToken;
  readonly accepted: AcceptedInspection;
  readonly navigation: ManagedNavigationIndex;
}
export interface InspectionNativeHandle {
  inspectEditableSession(json: string): string;
  editableInteractionSeed(json: string): string;
}

/** Fitted CSS extent; geometry and opaque selection identities remain native-owned. */
export interface InteractionViewport { readonly width: number; readonly height: number; readonly pixelRatio?: number }
export type InteractionSeed = Readonly<Record<string, unknown>>;
