// SPDX-License-Identifier: GPL-3.0-or-later

// Keep the heavyweight TypeScript parser in a lazy browser chunk. Rust still
// validates every returned byte and remains sole materialization/solver
// authority.
export {
  MANAGED_SKETCH_SOURCE_LIMIT,
  ManagedCompileError,
  ManagedMutationError,
  applyManagedSketchMutation,
  compileManagedSource,
  type CompiledManagedSource,
  type ManagedSketchMutation,
  type ManagedSourceSpan,
  type ManagedMutationReceipt,
} from "../../../../../packages/geosolve-sketch-code/src/managed";
