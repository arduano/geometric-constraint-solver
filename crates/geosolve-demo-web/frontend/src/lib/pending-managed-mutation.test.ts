// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";

import {
  PREPARED_MANAGED_MUTATION_FORMAT,
  PREPARED_MANAGED_SOURCE_FORMAT,
  assertWorkbenchSnapshot,
  type PendingManagedMutation,
  type PreparedManagedMutationRequest,
  type PreparedManagedSourceRequest,
  type WorkbenchSnapshot,
} from "./adapter";
import {
  compileManagedSource,
} from "./managed-compiler";
import { MockWorkbenchAdapter } from "./mock-adapter";
import { resolvePendingManagedMutationSnapshot } from "./pending-managed-mutation";

const SOURCE = `"use geosolve sketch";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const edge = $.geometry.segment("edge", {
    start: [0, 0],
    end: [20, 0],
  });
  return { edge };
});
`;

const digest = (digit: string) => digit.repeat(64);

function request(
  mutation: PreparedManagedMutationRequest["ticket"]["mutation"] = {
    mutation: "set_suppressed",
    target: { target: "declaration", declaration: "edge" },
    suppressed: true,
  },
  ticketDigit = "a",
): PreparedManagedMutationRequest {
  const current = compileManagedSource(SOURCE);
  return {
    ticket: {
      format: PREPARED_MANAGED_MUTATION_FORMAT,
      ticketDigest: digest(ticketDigit),
      project: "test-project",
      session: { session: 1, revision: 0, digest: digest("1") },
      acceptedSourceDigest: current.ir.source_digest,
      acceptedIrDigest: current.ir.ir_digest,
      acceptedArtifactDigest: current.artifact.artifact_digest,
      acceptedExpansionDigest: digest("2"),
      declarationNameHighWater: 0,
      candidateDeclarationNameHighWater: 0,
      baseSemanticsDigest: digest("3"),
      candidateSemanticsDigest: digest("4"),
      mutation,
    },
    current,
  };
}

function managedPending(
  prepared = request(),
): PendingManagedMutation {
  return { kind: "managed", request: prepared };
}

function sourcePending(): PendingManagedMutation {
  const current = compileManagedSource(SOURCE);
  const candidateSource = SOURCE.replace("end: [20, 0]", "end: [35, 0]");
  const candidate = compileManagedSource(candidateSource);
  const request: PreparedManagedSourceRequest = {
    ticket: {
      format: PREPARED_MANAGED_SOURCE_FORMAT,
      ticketDigest: digest("b"),
      project: "test-project",
      session: { session: 1, revision: 0, digest: digest("1") },
      acceptedSourceDigest: current.ir.source_digest,
      acceptedIrDigest: current.ir.ir_digest,
      acceptedArtifactDigest: current.artifact.artifact_digest,
      acceptedExpansionDigest: digest("2"),
      declarationNameHighWater: 0,
      candidateInputSourceDigest: candidate.inputSourceDigest,
    },
    current,
    candidateSource,
  };
  return { kind: "source", request };
}

class PendingAdapter extends MockWorkbenchAdapter {
  readonly commands: Array<{ command: string; payload?: unknown }> = [];
  compilerContexts = 0;

  queue(pending: PendingManagedMutation) {
    this.state.pendingManagedMutation = pending;
  }

  override async managedCompilerContext() {
    this.compilerContexts += 1;
    return super.managedCompilerContext();
  }

  override async dispatch(input: { command: string; payload?: unknown }) {
    this.commands.push(structuredClone(input));
    return super.dispatch(input);
  }
}

describe("prepared managed browser compilation", () => {
  it("compiles only the exact Rust-prepared raw source before resolving", async () => {
    const adapter = new PendingAdapter();
    adapter.queue(sourcePending());

    const settled = await resolvePendingManagedMutationSnapshot(
      adapter,
      await adapter.snapshot(),
    );

    expect(settled.pendingManagedMutation).toBeUndefined();
    expect(adapter.compilerContexts).toBe(1);
    expect(adapter.commands).toHaveLength(1);
    expect(adapter.commands[0]).toMatchObject({
      command: "managed.mutation.resolve",
      payload: {
        ticketDigest: digest("b"),
        baseSourceDigest: request().current.ir.source_digest,
        compiled: { normalizedSource: expect.stringContaining("end: [35, 0]") },
      },
    });
  });

  it("returns one flattened compiler receipt to Rust before exposing the snapshot", async () => {
    const adapter = new PendingAdapter();
    adapter.queue(managedPending());

    const settled = await resolvePendingManagedMutationSnapshot(
      adapter,
      await adapter.snapshot(),
    );

    expect(settled.pendingManagedMutation).toBeUndefined();
    expect(adapter.compilerContexts).toBe(1);
    expect(adapter.commands).toHaveLength(1);
    expect(adapter.commands[0]).toMatchObject({
      command: "managed.mutation.resolve",
      payload: {
        ticketDigest: digest("a"),
        baseSourceDigest: request().current.ir.source_digest,
        compiled: { normalizedSource: expect.stringContaining("$.suppress(edge);") },
      },
    });
    expect(adapter.commands[0]?.payload).not.toHaveProperty("receipt");
  });

  it("accepts the runtime-provenance value mutation used by managed controls", async () => {
    const adapter = new PendingAdapter();
    adapter.queue(managedPending(request({
      mutation: "set_value",
      declaration: "edge",
      path: ["end", 0],
      expected: { kind: "number", value: 20 },
      value: { kind: "number", value: 30 },
    })));

    const settled = await resolvePendingManagedMutationSnapshot(
      adapter,
      await adapter.snapshot(),
    );

    expect(settled.pendingManagedMutation).toBeUndefined();
    expect(adapter.commands[0]).toMatchObject({
      command: "managed.mutation.resolve",
      payload: {
        compiled: { normalizedSource: expect.stringContaining("end: [30, 0]") },
      },
    });
  });

  it("aborts a compiler refusal and returns Rust's restored accepted snapshot", async () => {
    const adapter = new PendingAdapter();
    adapter.queue(managedPending(request({
      mutation: "delete",
      target: { target: "declaration", declaration: "missing" },
    })));

    const settled = await resolvePendingManagedMutationSnapshot(
      adapter,
      await adapter.snapshot(),
    );

    expect(settled.pendingManagedMutation).toBeUndefined();
    expect(adapter.commands).toHaveLength(1);
    expect(adapter.commands[0]).toMatchObject({
      command: "managed.mutation.abort",
      payload: {
        ticketDigest: digest("a"),
        candidateSource: request().current.normalizedSource,
        diagnostic: expect.stringMatching(/missing|unknown declaration/u),
        span: { start: 0, end: 0 },
      },
    });
    expect(settled.problems.at(-1)?.detail).toMatch(/missing|unknown declaration/u);
    expect(settled.source.files[0]?.contents).toBe(request().current.normalizedSource);
    expect(settled.source.dirty).toBe(true);
    expect(settled.project.status).toBe("failed");
  });

  it("does not recursively compile another pending request returned by resolve", async () => {
    class RecursiveAdapter extends PendingAdapter {
      override async dispatch(input: { command: string; payload?: unknown }) {
        const settled = await super.dispatch(input);
        if (input.command === "managed.mutation.resolve") {
          const another = managedPending(request(undefined, "b"));
          this.state.pendingManagedMutation = another;
          settled.pendingManagedMutation = another;
        }
        return settled;
      }
    }
    const adapter = new RecursiveAdapter();
    adapter.queue(managedPending());

    await expect(resolvePendingManagedMutationSnapshot(
      adapter,
      await adapter.snapshot(),
    )).rejects.toThrow("returned another pending compiler request");
    expect(adapter.compilerContexts).toBe(1);
    expect(adapter.commands.map(({ command }) => command)).toEqual([
      "managed.mutation.resolve",
    ]);
  });

  it("aborts an authenticated terminal native rejection so pointer input is not left blocked", async () => {
    class RetainedNativeFailureAdapter extends PendingAdapter {
      override async dispatch(input: { command: string; payload?: unknown }) {
        if (input.command !== "managed.mutation.resolve") {
          return super.dispatch(input);
        }
        this.commands.push(structuredClone(input));
        this.state.problems = [{
          id: "workbench-interaction",
          severity: "error",
          title: "Workbench action was retained",
          detail: "terminal code drag differs from its independently staged native authority in sketch documents",
        }];
        this.state.project.status = "failed";
        return structuredClone(this.state);
      }
    }
    const adapter = new RetainedNativeFailureAdapter();
    adapter.queue(managedPending());

    const settled = await resolvePendingManagedMutationSnapshot(
      adapter,
      await adapter.snapshot(),
    );

    expect(settled.pendingManagedMutation).toBeUndefined();
    expect(adapter.commands.map(({ command }) => command)).toEqual([
      "managed.mutation.resolve",
      "managed.mutation.abort",
    ]);
    expect(adapter.commands[1]).toMatchObject({
      command: "managed.mutation.abort",
      payload: {
        ticketDigest: digest("a"),
        candidateSource: expect.stringContaining("$.suppress(edge);"),
        diagnostic: "terminal code drag differs from its independently staged native authority in sketch documents",
        span: { start: 0, end: 0 },
      },
    });
    expect(settled.problems.at(-1)?.detail).toBe(
      "terminal code drag differs from its independently staged native authority in sketch documents",
    );
  });

  it("consumes a terminal declaration-relabel rejection before the next pointer gesture", async () => {
    const relabelFailure =
      "declaration relabel witness is not owned by its allocated source declaration";
    class RelabelFailureAdapter extends PendingAdapter {
      pointerCalls = 0;

      override async dispatch(input: { command: string; payload?: unknown }) {
        if (input.command !== "managed.mutation.resolve") {
          return super.dispatch(input);
        }
        this.commands.push(structuredClone(input));
        this.state.problems = [{
          id: "workbench-interaction",
          severity: "error",
          title: "Workbench action was retained",
          detail: relabelFailure,
        }];
        this.state.project.status = "failed";
        return structuredClone(this.state);
      }

      override async pointer(input: Parameters<PendingAdapter["pointer"]>[0]) {
        if (this.state.pendingManagedMutation) {
          throw new Error(
            "pointer input is unavailable while a managed-source mutation is compiling",
          );
        }
        this.pointerCalls += 1;
        return super.pointer(input);
      }
    }
    const adapter = new RelabelFailureAdapter();
    adapter.queue(managedPending());

    const settled = await resolvePendingManagedMutationSnapshot(
      adapter,
      await adapter.snapshot(),
    );

    expect(settled.pendingManagedMutation).toBeUndefined();
    expect(adapter.commands.map(({ command }) => command)).toEqual([
      "managed.mutation.resolve",
      "managed.mutation.abort",
    ]);
    expect(adapter.commands[1]).toMatchObject({
      command: "managed.mutation.abort",
      payload: {
        ticketDigest: digest("a"),
        candidateSource: expect.stringContaining("$.suppress(edge);"),
        diagnostic: relabelFailure,
        span: { start: 0, end: 0 },
      },
    });
    await expect(adapter.pointer({
      version: 1,
      phase: "move",
      pointerId: 1,
      x: 10,
      y: 20,
      buttons: 0,
      modifiers: {
        alt: false,
        ctrl: false,
        meta: false,
        shift: false,
      },
    })).resolves.toBeNull();
    expect(adapter.pointerCalls).toBe(1);
  });

  it("keeps a stale same-ticket compiler receipt pending for a corrected receipt", async () => {
    class StaleReceiptAdapter extends PendingAdapter {
      override async dispatch(input: { command: string; payload?: unknown }) {
        if (input.command !== "managed.mutation.resolve") {
          return super.dispatch(input);
        }
        this.commands.push(structuredClone(input));
        this.state.problems = [{
          id: "workbench-interaction",
          severity: "error",
          title: "Workbench action was retained",
          detail: "prepared managed ticket is stale: accepted authority changed",
        }];
        this.state.project.status = "failed";
        return structuredClone(this.state);
      }
    }
    const adapter = new StaleReceiptAdapter();
    adapter.queue(managedPending());

    await expect(resolvePendingManagedMutationSnapshot(
      adapter,
      await adapter.snapshot(),
    )).rejects.toThrow(
      "Managed mutation resolve was rejected: prepared managed ticket is stale: accepted authority changed",
    );
    expect(adapter.commands.map(({ command }) => command)).toEqual([
      "managed.mutation.resolve",
    ]);
    expect((await adapter.snapshot()).pendingManagedMutation).toBeDefined();
  });

  it("does not abort a different pending candidate returned after resolution", async () => {
    class ReplacedPendingAdapter extends PendingAdapter {
      override async dispatch(input: { command: string; payload?: unknown }) {
        if (input.command !== "managed.mutation.resolve") {
          return super.dispatch(input);
        }
        this.commands.push(structuredClone(input));
        this.state.pendingManagedMutation = managedPending(request(undefined, "b"));
        this.state.problems = [{
          id: "workbench-interaction",
          severity: "error",
          title: "Workbench action was retained",
          detail: "terminal code drag differs from its independently staged native authority in sketch documents",
        }];
        this.state.project.status = "failed";
        return structuredClone(this.state);
      }
    }
    const adapter = new ReplacedPendingAdapter();
    adapter.queue(managedPending());

    await expect(resolvePendingManagedMutationSnapshot(
      adapter,
      await adapter.snapshot(),
    )).rejects.toThrow(
      "Managed mutation resolve was rejected: terminal code drag differs from its independently staged native authority in sketch documents",
    );
    expect(adapter.commands.map(({ command }) => command)).toEqual([
      "managed.mutation.resolve",
    ]);
    expect(
      (await adapter.snapshot()).pendingManagedMutation?.request.ticket.ticketDigest,
    ).toBe(digest("b"));
  });

  it("coalesces duplicate delivery of the same pending ticket", async () => {
    const adapter = new PendingAdapter();
    adapter.queue(managedPending());
    const pending = await adapter.snapshot();

    const [first, second] = await Promise.all([
      resolvePendingManagedMutationSnapshot(adapter, pending),
      resolvePendingManagedMutationSnapshot(adapter, structuredClone(pending)),
    ]);

    expect(first).toEqual(second);
    expect(adapter.compilerContexts).toBe(1);
    expect(adapter.commands.map(({ command }) => command)).toEqual([
      "managed.mutation.resolve",
    ]);
  });

  it("rejects malformed pending authority at the adapter snapshot boundary", async () => {
    const adapter = new PendingAdapter();
    adapter.queue(managedPending());
    const malformed = await adapter.snapshot() as WorkbenchSnapshot;
    const pending = malformed.pendingManagedMutation!;
    if (pending.kind !== "managed") throw new Error("expected managed test request");
    pending.request.ticket.ticketDigest = "not-a-digest";
    expect(() => assertWorkbenchSnapshot(malformed)).toThrow(
      "Unsupported or malformed workbench snapshot",
    );
  });
});
