// SPDX-License-Identifier: GPL-3.0-or-later
import { describe, expect, it } from "vitest";
import { assertWorkbenchSnapshot, type AuthoringMetadataSnapshot, type DimensionsSnapshot, type NavigationSnapshot } from "./adapter";
import { MockWorkbenchAdapter } from "./mock-adapter";

const navigation: NavigationSnapshot = {
  authority: "accepted-source-and-scene",
  selectionKey: "selection",
  rows: [{ id: "group:sketch", state: "partial" }, { id: "line-1", state: "selected" }],
  sources: [{ path: "sketch.ts", from: 10, to: 20 }],
  itemCount: 3,
  canNavigateSource: true,
};

describe("M95 navigation snapshot transport", () => {
  it("accepts authenticated navigation and legacy snapshots during migration", async () => {
    const snapshot = await new MockWorkbenchAdapter().snapshot();
    expect(assertWorkbenchSnapshot(snapshot)).toBe(snapshot);
    snapshot.navigation = navigation;
    expect(assertWorkbenchSnapshot(snapshot).navigation).toEqual(navigation);
  });

  it.each([
    { authority: "" }, { itemCount: -1 }, { canNavigateSource: 1 },
    { rows: [{ id: "x", state: "mixed" }] },
    { sources: [{ path: "sketch.ts", from: 5, to: 4 }] },
    { sources: [{ path: "sketch.ts", from: Number.NaN, to: 4 }] },
    { sources: [{ path: "sketch.ts", from: 0, to: Number.POSITIVE_INFINITY }] },
    { notice: 42 },
  ])("rejects malformed navigation fields %j", async (changes) => {
    const snapshot = await new MockWorkbenchAdapter().snapshot();
    snapshot.navigation = { ...navigation, ...changes } as NavigationSnapshot;
    expect(() => assertWorkbenchSnapshot(snapshot)).toThrow("Unsupported or malformed workbench snapshot");
  });
});

describe("source authoring metadata snapshot transport", () => {
  const metadata: AuthoringMetadataSnapshot = { authority: "accepted-source", target: { kind: "parameter", id: "width" }, label: "Channel width", description: "Full passage width", isKeyParameter: true, hasKeyOverride: true, editable: true, canExtract: false };
  it("accepts source-owned document and parameter presentation without altering authority", async () => {
    const snapshot = await new MockWorkbenchAdapter().snapshot();
    snapshot.parameters[0].metadata = metadata;
    snapshot.authoringDocument = { authority: "accepted-source", title: "Manifold", description: "Water passages", areKeyConstraintsByDefault: false, editable: true };
    expect(assertWorkbenchSnapshot(snapshot)).toBe(snapshot);
  });
  it.each([
    { authority: "" }, { target: { kind: "opaque", id: "width" } }, { target: { kind: "parameter", id: "" } },
    { editable: "true" }, { isKeyParameter: 1 }, { isKeyConstraint: true }, { description: { html: "injected" } }, { hasKeyOverride: "false" }, { canExtract: 1 },
  ])("rejects malformed authored presentation %j", async (changes) => {
    const snapshot = await new MockWorkbenchAdapter().snapshot();
    snapshot.parameters[0].metadata = { ...metadata, ...changes } as AuthoringMetadataSnapshot;
    expect(() => assertWorkbenchSnapshot(snapshot)).toThrow("Unsupported or malformed workbench snapshot");
  });
  it("rejects malformed discovery and document defaults", async () => {
    const snapshot = await new MockWorkbenchAdapter().snapshot();
    snapshot.authoringDocument = { authority: "accepted-source", title: "Title", description: "", areKeyConstraintsByDefault: "true" as unknown as boolean, editable: true };
    expect(() => assertWorkbenchSnapshot(snapshot)).toThrow("Unsupported or malformed workbench snapshot");
    delete snapshot.authoringDocument;
    snapshot.dimensions = { mode: "focused", entries: [], parameters: [], pinCount: 0, allMeasurements: [{}] as DimensionsSnapshot["entries"] };
    expect(() => assertWorkbenchSnapshot(snapshot)).toThrow("Unsupported or malformed workbench snapshot");
  });
});

describe("M97 dimension snapshot transport", () => {
  const dimensions: DimensionsSnapshot = { mode: "focused", pinCount: 1, parameters: [{ id: "width", label: "Channel width", value: "12", unit: "mm", editable: true }], entries: [{ id: "d:1", label: "Half width", value: "6", unit: "mm", kind: "Offset", reference: false, generated: true, pinned: true, visible: false, focused: false, editable: true }] };
  it("accepts complete contextual metadata and snapshots without the optional extension", async () => {
    const snapshot = await new MockWorkbenchAdapter().snapshot();
    expect(assertWorkbenchSnapshot(snapshot)).toBe(snapshot);
    snapshot.dimensions = dimensions;
    expect(assertWorkbenchSnapshot(snapshot).dimensions).toEqual(dimensions);
  });
  it.each([
    { mode: "everything" }, { pinCount: 5 }, { pinCount: -1 }, { pinCount: 1.5 },
    { entries: [{ ...dimensions.entries[0], visible: "false" }] },
    { entries: [{ ...dimensions.entries[0], reason: 42 }] },
    { entries: [...dimensions.entries, ...dimensions.entries] },
    { parameters: [{ ...dimensions.parameters[0], value: 12 }] },
  ])("rejects malformed metadata %j", async (changes) => {
    const snapshot = await new MockWorkbenchAdapter().snapshot();
    snapshot.dimensions = { ...dimensions, ...changes } as DimensionsSnapshot;
    expect(() => assertWorkbenchSnapshot(snapshot)).toThrow("Unsupported or malformed workbench snapshot");
  });
});
