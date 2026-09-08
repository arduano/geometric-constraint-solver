// SPDX-License-Identifier: GPL-3.0-or-later
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { AuthoringMetadataSnapshot, DimensionEntry } from "../lib/adapter";
import { AuthoringDocumentProperties, AuthoringMetadataEditor, type AuthoringMetadataActions } from "./authoring-metadata";
import { DimensionInspector } from "./dimension-inspector";
import { ParametersView } from "./side-panels";
import { MockWorkbenchAdapter } from "../lib/mock-adapter";

const metadata = (changes: Partial<AuthoringMetadataSnapshot> = {}): AuthoringMetadataSnapshot => ({ authority: "accepted-source", target: { kind: "dimension", id: "width" }, editable: true, isKeyConstraint: true, hasKeyOverride: true, ...changes });
const actions = (): AuthoringMetadataActions => ({ onEdit: vi.fn(), onExtract: vi.fn() });

describe("source-owned authoring properties", () => {
  it("sends explicit overview intent and a distinct reset without touching personal pins", async () => {
    const handlers = actions(); const user = userEvent.setup();
    render(<AuthoringMetadataEditor metadata={metadata()} label="Width" actions={handlers} />);
    const checkbox = screen.getByRole("checkbox", { name: "Show Width in overview" });
    expect(checkbox).toBeChecked();
    expect(checkbox).toHaveAccessibleDescription("Keep this measurement available without selecting its geometry.");
    checkbox.focus(); await user.keyboard(" ");
    expect(handlers.onEdit).toHaveBeenLastCalledWith({ authority: "accepted-source", target: { kind: "dimension", id: "width" }, changes: { isKeyConstraint: false } });
    expect(checkbox).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Reset Width to default" }));
    expect(handlers.onEdit).toHaveBeenLastCalledWith({ authority: "accepted-source", target: { kind: "dimension", id: "width" }, changes: { isKeyConstraint: null } });
    expect(handlers.onExtract).not.toHaveBeenCalled();
  });

  it("keeps name edits, focus and source identity across accepted metadata publication", async () => {
    const handlers = actions(); const user = userEvent.setup();
    const before = metadata({ label: "Width" });
    const view = render(<AuthoringMetadataEditor metadata={before} label="Width" actions={handlers} />);
    await user.click(screen.getByText("Name and description"));
    const input = screen.getByRole("textbox", { name: "Width name" });
    await user.click(input); await user.clear(input); await user.type(input, "Passage width{Enter}");
    expect(handlers.onEdit).toHaveBeenCalledExactlyOnceWith({ authority: "accepted-source", target: before.target, changes: { label: "Passage width" } });
    view.rerender(<AuthoringMetadataEditor metadata={{ ...before, authority: "new-source", label: "Passage width" }} label="Passage width" actions={handlers} />);
    expect(screen.getByRole("textbox", { name: "Passage width name" })).toBe(input);
    expect(input).toHaveFocus();
    await user.tab(); expect(handlers.onEdit).toHaveBeenCalledTimes(1);
    await user.click(input); await user.clear(input); await user.type(input, "ignored{Escape}");
    expect(input).toHaveValue("Passage width"); expect(input).toHaveFocus();
    await user.clear(input); await user.keyboard("{Enter}");
    expect(handlers.onEdit).toHaveBeenLastCalledWith({ authority: "new-source", target: before.target, changes: { label: null } });
  });

  it("renders descriptions as plain text and commits multiline help only on blur or the explicit shortcut", async () => {
    const handlers = actions(); const user = userEvent.setup();
    const description = "<img src=x onerror=alert(1)>";
    const view = render(<AuthoringMetadataEditor metadata={metadata({ description })} label="Width" actions={handlers} />);
    expect(screen.getByText(description, { selector: "p" })).toBeVisible();
    expect(view.container.querySelector("img")).toBeNull();
    await user.click(screen.getByText("Name and description"));
    const input = screen.getByRole("textbox", { name: "Width description" });
    fireEvent.change(input, { target: { value: "First line\nSecond line" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(handlers.onEdit).not.toHaveBeenCalled();
    fireEvent.keyDown(input, { key: "Enter", ctrlKey: true });
    fireEvent.blur(input);
    expect(handlers.onEdit).toHaveBeenCalledExactlyOnceWith({ authority: "accepted-source", target: { kind: "dimension", id: "width" }, changes: { description: "First line\nSecond line" } });
  });

  it("uses parameter naming and offers authenticated extraction without granting inline metadata editing", async () => {
    const handlers = actions(); const user = userEvent.setup();
    const inline = metadata({ target: { kind: "parameter", id: "input-width" }, isKeyConstraint: undefined, isKeyParameter: true, editable: false, canExtract: true, reason: "Make this value a named parameter to customize its presentation." });
    const view = render(<AuthoringMetadataEditor metadata={inline} label="Channel width" actions={handlers} extractionId="native-control" />);
    expect(screen.getByRole("checkbox", { name: "Show Channel width in overview" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Make Channel width a named parameter" }));
    expect(handlers.onExtract).toHaveBeenCalledExactlyOnceWith({ authority: "accepted-source", id: "native-control", label: "Channel width", isKeyParameter: true });
    expect(handlers.onEdit).not.toHaveBeenCalled();
    view.rerender(<AuthoringMetadataEditor metadata={{ ...inline, editable: true, canExtract: false, hasKeyOverride: false }} label="Channel width" actions={handlers} />);
    await user.click(screen.getByRole("checkbox", { name: "Show Channel width in overview" }));
    expect(handlers.onEdit).toHaveBeenLastCalledWith({ authority: "accepted-source", target: inline.target, changes: { isKeyParameter: false } });
  });

  it.each(["Apply or Revert the source draft.", "Finish the current gesture.", "Source publication is pending."])("blocks every metadata action while %s", async (reason) => {
    const handlers = actions(); const user = userEvent.setup();
    render(<AuthoringMetadataEditor metadata={metadata({ canExtract: true })} label="Width" actions={handlers} extractionId="width-control" blockedReason={reason} />);
    await user.click(screen.getByText("Name and description"));
    expect(screen.getByRole("checkbox")).toBeDisabled();
    expect(screen.getByRole("button", { name: "Reset Width to default" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Make Width a named parameter" })).toBeDisabled();
    expect(screen.getByRole("textbox", { name: "Width name" })).toBeDisabled();
    expect(screen.getByRole("textbox", { name: "Width description" })).toBeDisabled();
    expect(handlers.onEdit).not.toHaveBeenCalled(); expect(handlers.onExtract).not.toHaveBeenCalled();
  });

  it("edits reference measurement presentation while its measured value stays read only", async () => {
    const handlers = actions(); const user = userEvent.setup();
    const entry: DimensionEntry = { id: "reference-width", label: "Reference width", value: "12", unit: "mm", kind: "Distance", reference: true, editable: false, generated: false, pinned: false, focused: false, visible: false, metadata: metadata() };
    const dimensionActions = { onFocus: vi.fn(), onPin: vi.fn(), onClearPins: vi.fn(), onEdit: vi.fn() };
    render(<DimensionInspector dimensions={{ entries: [entry], allMeasurements: [entry], parameters: [], pinCount: 0, mode: "focused" }} actions={dimensionActions} onParameterEdit={vi.fn()} metadataActions={handlers} />);
    expect(screen.getByLabelText("Reference width value")).toHaveTextContent("(12 mm)");
    await user.click(screen.getByRole("checkbox", { name: "Show Reference width in overview" }));
    expect(handlers.onEdit).toHaveBeenCalledTimes(1);
    expect(dimensionActions.onEdit).not.toHaveBeenCalled();
  });

  it.each([false, true])("discovers an unmarked measurement without removing the focused discovery row (generated: %s)", async (generated) => {
    const user = userEvent.setup(); const handlers = { onFocus: vi.fn(), onPin: vi.fn(), onClearPins: vi.fn(), onEdit: vi.fn() };
    const entry: DimensionEntry = { id: "width-before", rowKey: "stable-width", label: "Width", value: "12", kind: "Distance", reference: false, editable: true, generated, pinned: false, focused: false, visible: false };
    const state = { entries: [], allMeasurements: [entry], parameters: [], pinCount: 0, mode: "focused" as const };
    const view = render(<DimensionInspector dimensions={state} actions={handlers} onParameterEdit={vi.fn()} />);
    await user.click(screen.getByText(/All measurements/));
    const discover = screen.getByRole("button", { name: "Show details for Width" });
    await user.click(discover); expect(handlers.onFocus).toHaveBeenCalledWith("width-before");
    const selected = { ...entry, id: "width-after", focused: true };
    view.rerender(<DimensionInspector dimensions={{ ...state, entries: [selected], allMeasurements: [selected] }} actions={handlers} onParameterEdit={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Inspect Width" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Show details for Width" })).toBe(discover);
    expect(discover).toHaveFocus();
    await user.keyboard("{Enter}"); expect(handlers.onFocus).toHaveBeenLastCalledWith("width-after");
  });

  it("edits document properties and an explicit all-authored overview default", async () => {
    const handlers = actions(); const user = userEvent.setup();
    render(<AuthoringDocumentProperties document={{ authority: "document-source", title: "Gridfinity", description: "Stackable bin", areKeyConstraintsByDefault: true, editable: true }} actions={handlers} />);
    await user.click(screen.getByText("Document properties"));
    const checkbox = screen.getByRole("checkbox", { name: "Show authored dimensions in overview by default" });
    expect(checkbox).toBeChecked(); await user.click(checkbox);
    expect(handlers.onEdit).toHaveBeenLastCalledWith({ authority: "document-source", target: { kind: "document" }, changes: { areKeyConstraintsByDefault: false } });
    const title = screen.getByRole("textbox", { name: "Document title" });
    fireEvent.change(title, { target: { value: "My bin" } }); fireEvent.blur(title);
    expect(handlers.onEdit).toHaveBeenLastCalledWith({ authority: "document-source", target: { kind: "document" }, changes: { title: "My bin" } });
  });

  it("keeps shared parameter consumers together and equal-valued parameters independently editable across publication", async () => {
    const user = userEvent.setup(); const onEdit = vi.fn();
    const snapshot = await new MockWorkbenchAdapter().snapshot();
    snapshot.parameters = [
      { id: "shared-before", rowKey: "shared-width", label: "Channel width", value: "12", unit: "mm", editable: true, consumers: ["Upper channel", "Middle channel", "Lower channel", "Stair channel"] },
      { id: "other", rowKey: "other-width", label: "Slot width", value: "12", unit: "mm", editable: true, consumers: ["Mounting slot"] },
    ];
    const view = render(<ParametersView snapshot={snapshot} onEdit={onEdit} blocked={false} />);
    expect(screen.getByText("Used by Upper channel, Middle channel, Lower channel, Stair channel")).toBeVisible();
    const input = screen.getByRole("textbox", { name: "Channel width" });
    expect(screen.getAllByRole("textbox")).toHaveLength(2);
    await user.click(input); await user.clear(input); await user.type(input, "14{Enter}");
    expect(onEdit).toHaveBeenCalledExactlyOnceWith("shared-before", "14");
    view.rerender(<ParametersView snapshot={{ ...snapshot, parameters: [{ ...snapshot.parameters[0], id: "shared-after", value: "14" }, snapshot.parameters[1]] }} onEdit={onEdit} blocked={false} />);
    expect(screen.getByRole("textbox", { name: "Channel width" })).toBe(input);
    expect(input).toHaveFocus();
    await user.tab(); expect(onEdit).toHaveBeenCalledTimes(1);
    expect(screen.getByRole("textbox", { name: "Slot width" })).toHaveValue("12");
    await user.click(input); await user.clear(input); await user.type(input, "16{Enter}");
    expect(onEdit).toHaveBeenLastCalledWith("shared-after", "16");
  });
});
