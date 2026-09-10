// SPDX-License-Identifier: GPL-3.0-or-later
import initializeWasm, * as wasm from "../generated/geosolve_demo_web.js";
import type { WorkbenchSnapshot } from "./adapter";

/** Opaque, versioned Rust-owned transport; JavaScript never interprets geometry or selection IDs. */
export type InteractionSeed = Record<string, unknown>;
export type InteractionState = Record<string, unknown>;
export interface LocalInteractionUpdate {
  frame: WorkbenchSnapshot["frame"];
  state: InteractionState;
  selectionChanged: boolean;
  /** Rust verified the authoritative preview matches this exact camera and selection. */
  serverFrameCompatible: boolean;
}
export type LocalInteractionMethod = "authoringPointer" | "construct" | "replace" | "dispatch" | "pointer" | "wheel" | "resize" | "cancel" | "state";
export interface LocalInteractionRequest { id: number; method: LocalInteractionMethod; input?: unknown; }
export type LocalInteractionResponse = { id: number; result: LocalInteractionUpdate | InteractionState | null } | { id: number; error: string };
export interface InteractionHandle {
  replace(input: string): string;
  dispatch(input: string): string;
  pointer(input: string): string;
  wheel(input: string): string;
  resize(input: string): string;
  cancel(input: string): string;
  authoringPointer(input: string): string;
  state(): string;
  free(): void;
}
export type InteractionConstructor = new (seed: string) => InteractionHandle;

export function createLocalInteractionHandler(
  constructor: Promise<InteractionConstructor>,
  reply: (response: LocalInteractionResponse) => void,
) {
  let handle: InteractionHandle | undefined;
  let tail = Promise.resolve();
  void constructor.catch(() => undefined);
  return ({ data }: MessageEvent<LocalInteractionRequest>) => {
    const run = async () => {
      try {
        if (!Number.isSafeInteger(data?.id) || data.id < 1) throw Error("Invalid local interaction request");
        let result: string;
        if (data.method === "construct") {
          const Constructor = await constructor;
          handle?.free();
          handle = new Constructor(JSON.stringify(data.input));
          result = handle.replace(JSON.stringify({ seed: data.input, preserveSelection: false }));
        } else {
          if (!handle) throw Error("Local interaction has not been initialized");
          if (data.method === "state") result = handle.state();
          else if (["replace", "dispatch", "pointer", "wheel", "resize", "cancel", "authoringPointer"].includes(data.method)) {
            result = handle[data.method](JSON.stringify(data.input));
          } else throw Error("Unsupported local interaction operation");
        }
        reply({ id: data.id, result: JSON.parse(result) });
      } catch (error) {
        reply({ id: data?.id, error: error instanceof Error ? error.message : String(error) });
      }
    };
    tail = tail.then(run, run);
  };
}

if (typeof document === "undefined" && typeof self !== "undefined") {
  const constructor = initializeWasm().then(() => {
    const Constructor = (wasm as unknown as { InteractionHandle?: InteractionConstructor }).InteractionHandle;
    if (!Constructor) throw Error("This build does not support local canvas interaction");
    return Constructor;
  });
  self.onmessage = createLocalInteractionHandler(constructor, (response) => self.postMessage(response));
}
