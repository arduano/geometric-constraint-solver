// SPDX-License-Identifier: GPL-3.0-or-later

import type {
  PendingManagedMutation,
  WorkbenchAdapter,
  WorkbenchSnapshot,
} from "./adapter";
import { assertWorkbenchSnapshot } from "./adapter";
import type {
  ManagedMutationReceipt,
  ManagedSourceSpan,
} from "./managed-compiler";

const MANAGED_COMPILER_DIAGNOSTIC_LIMIT = 64 * 1024;
const MANAGED_COMPILER_SOURCE_LIMIT = 4 * 1024 * 1024;
const TERMINAL_NATIVE_REJECTION_PREFIXES = [
  "managed canvas candidate failed cold native materialization:",
  "terminal code drag differs from its independently staged native authority in ",
  "declaration relabel witness ",
  "resolved managed mutation lost its selected declaration",
  "resolved managed mutation could not restore selection",
] as const;

interface InFlightResolution {
  readonly ticketDigest: string;
  readonly result: Promise<WorkbenchSnapshot>;
}

const inFlightResolutions = new WeakMap<WorkbenchAdapter, InFlightResolution>();

/**
 * Resolve at most one Rust-prepared managed mutation before a snapshot may be
 * painted or persisted. Rust authenticates the returned receipt and remains
 * the sole publication authority.
 */
export async function resolvePendingManagedMutationSnapshot(
  adapter: WorkbenchAdapter,
  snapshot: WorkbenchSnapshot,
): Promise<WorkbenchSnapshot> {
  const candidate = assertWorkbenchSnapshot(snapshot);
  const pending = candidate.pendingManagedMutation;
  const active = inFlightResolutions.get(adapter);
  if (!pending) return active?.result ?? candidate;

  const ticketDigest = pending.request.ticket.ticketDigest;
  if (active) {
    if (active.ticketDigest !== ticketDigest) {
      throw new Error(
        "Adapter returned a different managed mutation while compilation was in flight",
      );
    }
    return active.result;
  }

  const result = resolvePreparedManagedMutation(adapter, pending, ticketDigest);
  const resolution = { ticketDigest, result };
  inFlightResolutions.set(adapter, resolution);
  void result.finally(() => {
    if (inFlightResolutions.get(adapter) === resolution) {
      inFlightResolutions.delete(adapter);
    }
  }).catch(() => undefined);
  return result;
}

async function resolvePreparedManagedMutation(
  adapter: WorkbenchAdapter,
  pending: PendingManagedMutation,
  ticketDigest: string,
): Promise<WorkbenchSnapshot> {
  let resolutionPayload: Record<string, unknown>;
  let candidateSource = pending.kind === "source"
    ? pending.request.candidateSource
    : pending.request.current.normalizedSource;
  try {
    const { applyManagedSketchMutation, compileManagedSource } = await import("./managed-compiler");
    const compilerContext = await adapter.managedCompilerContext();
    if (compilerContext.version !== 1) {
      throw new Error("Unsupported managed compiler context");
    }
    const options = { patches: compilerContext.patches as never };
    const receipt: ManagedMutationReceipt = pending.kind === "source"
      ? (() => {
        const compiled = compileManagedSource(candidateSource, options);
        return {
          baseSourceDigest: pending.request.current.ir.source_digest,
          candidateSourceDigest: compiled.ir.source_digest,
          compiled,
        };
      })()
      : applyManagedSketchMutation(
        pending.request.current,
        pending.request.ticket.mutation,
        options,
      );
    if (pending.kind === "managed") candidateSource = receipt.compiled.normalizedSource;
    resolutionPayload = { ticketDigest, ...receipt };
  } catch (error) {
    const failure = positionedCompilerFailure(error, candidateSource);
    candidateSource = failure.candidateSource;
    const diagnostic = boundedUtf8Text(
      errorText(error),
      MANAGED_COMPILER_DIAGNOSTIC_LIMIT,
    );
    const restored = await adapter.dispatch({
      version: 1,
      command: "managed.mutation.abort",
      payload: { ticketDigest, candidateSource, diagnostic, span: failure.span },
    });
    return requireSettledSnapshot(restored, "abort");
  }

  const resolved = await adapter.dispatch({
    version: 1,
    command: "managed.mutation.resolve",
    payload: resolutionPayload,
  });
  const checked = assertWorkbenchSnapshot(resolved);
  const rejection = terminalNativeRejection(checked, pending);
  if (rejection) {
    const restored = await adapter.dispatch({
      version: 1,
      command: "managed.mutation.abort",
      payload: {
        ticketDigest,
        candidateSource,
        diagnostic: boundedUtf8Text(
          rejection.detail,
          MANAGED_COMPILER_DIAGNOSTIC_LIMIT,
        ),
        span: { start: 0, end: 0 },
      },
    });
    return requireSettledSnapshot(restored, "abort");
  }
  return requireSettledSnapshot(checked, "resolve");
}

/**
 * A native rejection happens only after Rust has authenticated the compiler
 * receipt. Consume it as a failed browser attempt so the exact prepared ticket
 * cannot keep the global input guard latched. Receipt/authentication failures
 * deliberately remain pending and retryable at the Rust boundary.
 */
function terminalNativeRejection(
  snapshot: WorkbenchSnapshot,
  original: PendingManagedMutation,
): { readonly detail: string } | undefined {
  const pending = snapshot.pendingManagedMutation;
  if (!pending || !sameJsonValue(pending, original)) return undefined;
  const retained = snapshot.problems.find(
    (problem) => problem.id === "workbench-interaction",
  );
  if (
    !retained || snapshot.project.status !== "failed" ||
    !TERMINAL_NATIVE_REJECTION_PREFIXES.some((prefix) =>
      retained.detail.startsWith(prefix)
    )
  ) return undefined;
  return retained;
}

function sameJsonValue(left: unknown, right: unknown): boolean {
  if (Object.is(left, right)) return true;
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left) && Array.isArray(right) &&
      left.length === right.length &&
      left.every((value, index) => sameJsonValue(value, right[index]));
  }
  if (
    typeof left !== "object" || left === null ||
    typeof right !== "object" || right === null
  ) return false;
  const leftRecord = left as Record<string, unknown>;
  const rightRecord = right as Record<string, unknown>;
  const leftKeys = Object.keys(leftRecord).sort();
  const rightKeys = Object.keys(rightRecord).sort();
  return leftKeys.length === rightKeys.length &&
    leftKeys.every((key, index) =>
      key === rightKeys[index] && sameJsonValue(leftRecord[key], rightRecord[key])
    );
}

function requireSettledSnapshot(
  snapshot: WorkbenchSnapshot,
  operation: "resolve" | "abort",
): WorkbenchSnapshot {
  const checked = assertWorkbenchSnapshot(snapshot);
  if (checked.pendingManagedMutation) {
    const retained = checked.problems.find(
      (problem) => problem.id === "workbench-interaction",
    );
    if (retained) {
      throw new Error(
        `Managed mutation ${operation} was rejected: ${retained.detail}`,
      );
    }
    throw new Error(
      `Managed mutation ${operation} returned another pending compiler request`,
    );
  }
  return checked;
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function utf8Length(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

function positionedCompilerFailure(
  error: unknown,
  fallbackSource: string,
): { readonly candidateSource: string; readonly span: ManagedSourceSpan } {
  const detail = typeof error === "object" && error !== null
    ? error as { readonly candidateSource?: unknown; readonly span?: unknown }
    : {};
  const candidateSource = typeof detail.candidateSource === "string" &&
      utf8Length(detail.candidateSource) <= MANAGED_COMPILER_SOURCE_LIMIT
    ? detail.candidateSource
    : fallbackSource;
  const sourceBytes = utf8Length(candidateSource);
  const span = detail.span;
  if (
    typeof span === "object" && span !== null &&
    Number.isSafeInteger((span as { start?: unknown }).start) &&
    Number.isSafeInteger((span as { end?: unknown }).end)
  ) {
    const { start, end } = span as { start: number; end: number };
    if (start >= 0 && end >= start && end <= sourceBytes) {
      return { candidateSource, span: { start, end } };
    }
  }
  return { candidateSource, span: { start: 0, end: 0 } };
}

function boundedUtf8Text(value: string, limit: number): string {
  const bytes = new TextEncoder().encode(value);
  if (bytes.byteLength <= limit) return value;
  const suffix = new TextEncoder().encode("…");
  let end = limit - suffix.byteLength;
  const decoder = new TextDecoder("utf-8", { fatal: true });
  while (end > 0) {
    try {
      return decoder.decode(bytes.slice(0, end)) + "…";
    } catch {
      end -= 1;
    }
  }
  return "…";
}
