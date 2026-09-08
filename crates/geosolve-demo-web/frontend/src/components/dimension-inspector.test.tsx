// SPDX-License-Identifier: GPL-3.0-or-later
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { DimensionEntry, DimensionsSnapshot } from "../lib/adapter";
import { CanvasControls } from "./canvas-controls";
import { DimensionInspector, type DimensionPanelActions } from "./dimension-inspector";

const dimension = (id: string, changes: Partial<DimensionEntry> = {}): DimensionEntry => ({ id, label: id, value: "12", unit: "mm", kind: "Distance", reference: false, generated: false, pinned: false, visible: false, focused: false, editable: true, ...changes });
const state = (changes: Partial<DimensionsSnapshot> = {}): DimensionsSnapshot => ({ mode: "focused", pinCount: 0, entries: [dimension("Width", { visible: true }), dimension("Span", { value: "54", reference: true, editable: false }), dimension("Half width", { generated: true, value: "6" })], parameters: [{ id: "channel-width", label: "Channel width", value: "12", unit: "mm", editable: true }], ...changes });
const actions = (): DimensionPanelActions => ({ onFocus: vi.fn(), onPin: vi.fn(), onClearPins: vi.fn(), onEdit: vi.fn() });

describe("focused dimension inspection", () => {
  it("offers explicit display modes without invoking camera controls", async () => {
    const onCommand = vi.fn(); const onDimensionMode = vi.fn(); const user = userEvent.setup();
    const view = render(<CanvasControls gridVisible onCommand={onCommand} onDimensionMode={onDimensionMode} />);
    const display = screen.getByRole("combobox", { name: "Dimension display" });
    expect(display).toHaveValue("focused");
    expect(display).toHaveAttribute("title", expect.stringContaining("Focused shows key dimensions"));
    await user.selectOptions(display, "all");
    expect(onDimensionMode).toHaveBeenLastCalledWith("all");
    view.rerender(<CanvasControls gridVisible dimensionMode="all" onCommand={onCommand} onDimensionMode={onDimensionMode} />);
    await user.selectOptions(display, "hidden");
    expect(onDimensionMode).toHaveBeenLastCalledWith("hidden");
    expect(onCommand).not.toHaveBeenCalled();
    expect(display).toHaveFocus();
  });

  it("puts public parameters first, keeps generated rows collapsed and exposes every contextual measurement", async () => {
    const user = userEvent.setup(); const handlers = actions();
    render(<DimensionInspector dimensions={state()} actions={handlers} onParameterEdit={vi.fn()} />);
    expect(screen.getByText("1 of 3 measurements shown on canvas.")).toBeVisible();
    const parameter = screen.getByRole("textbox", { name: "Channel width value" });
    const width = screen.getByRole("button", { name: "Inspect Width" });
    expect(parameter.compareDocumentPosition(width) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByLabelText("Span value")).toHaveTextContent("(54 mm)");
    expect(screen.getByRole("button", { name: "Inspect Half width" })).not.toBeVisible();
    await user.click(screen.getByText(/Generated dimensions/));
    const generated = screen.getByRole("button", { name: "Inspect Half width" });
    await user.click(generated);
    expect(handlers.onFocus).toHaveBeenCalledWith("Half width");
    expect(generated).toHaveFocus();
  });

  it("keeps key measurements and public parameters accessible when their canvas labels are suppressed", () => {
    const handlers = actions();
    render(<DimensionInspector dimensions={state({
      entries: [dimension("Width", { defaultPriority: true }), dimension("Patch pitch", { defaultPriority: true, generated: true }), dimension("Half width", { generated: true })],
      parameters: [{ id: "channel-width", label: "Channel width", value: "12", unit: "mm", editable: true, defaultPriority: true }],
    })} actions={handlers} onParameterEdit={vi.fn()} />);
    expect(screen.getByText("0 of 3 measurements shown on canvas.")).toBeVisible();
    expect(screen.getByText("0/4 pinned")).toBeVisible();
    const width = screen.getByRole("button", { name: "Inspect Width" });
    const pitch = screen.getByRole("button", { name: "Inspect Patch pitch" });
    expect(width).toBeVisible();
    expect(width).toHaveTextContent("Key dimension · Editable · In Inspector");
    expect(pitch).toBeVisible();
    expect(pitch).toHaveTextContent("Key dimension");
    expect(screen.getByRole("button", { name: "Inspect Half width" })).not.toBeVisible();
    const parameters = within(screen.getByRole("group", { name: "Dimensional parameters" }));
    expect(parameters.getByText("Key dimension")).toBeVisible();
    expect(parameters.getByRole("textbox", { name: "Channel width value" })).toHaveValue("12");
    expect(handlers.onPin).not.toHaveBeenCalled();
  });

  it("edits accepted values once per submission and cancels drafts without changing focus", async () => {
    const handlers = actions(); const onParameterEdit = vi.fn(); const user = userEvent.setup();
    render(<DimensionInspector dimensions={state()} actions={handlers} onParameterEdit={onParameterEdit} />);
    const width = screen.getByRole("textbox", { name: "Width value" });
    await user.click(width);
    expect(handlers.onFocus).toHaveBeenCalledWith("Width");
    await user.clear(width); await user.type(width, "14{Enter}");
    expect(handlers.onEdit).toHaveBeenCalledExactlyOnceWith("Width", "14");
    await user.tab();
    expect(handlers.onEdit).toHaveBeenCalledTimes(1);
    await user.click(width); await user.clear(width); await user.type(width, "99{Escape}");
    expect(width).toHaveValue("12"); expect(width).toHaveFocus();
    await user.tab();
    expect(handlers.onEdit).toHaveBeenCalledTimes(1);
    const parameter = screen.getByRole("textbox", { name: "Channel width value" });
    fireEvent.change(parameter, { target: { value: "16" } }); fireEvent.blur(parameter);
    expect(onParameterEdit).toHaveBeenCalledExactlyOnceWith("channel-width", "16");
  });

  it("enforces four pins while preserving individual unpin and clear actions", async () => {
    const handlers = actions(); const user = userEvent.setup();
    render(<DimensionInspector dimensions={state({ pinCount: 4, entries: [dimension("Width", { pinned: true }), dimension("Span")] })} actions={handlers} onParameterEdit={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Pin Span" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Unpin Width" }));
    expect(handlers.onPin).toHaveBeenCalledExactlyOnceWith("Width", false);
    await user.click(screen.getByRole("button", { name: "Clear pins" }));
    expect(handlers.onClearPins).toHaveBeenCalledTimes(1);
  });

  it("retains the focused input across an accepted edit while using the new command authority", async () => {
    const handlers = actions(); const user = userEvent.setup();
    const before = dimension("authority-before", { label: "Width", rowKey: "width-identity" });
    const view = render(<DimensionInspector dimensions={state({ entries: [before] })} actions={handlers} onParameterEdit={vi.fn()} />);
    const input = screen.getByRole("textbox", { name: "Width value" });
    await user.click(input); await user.clear(input); await user.type(input, "14{Enter}");
    view.rerender(<DimensionInspector dimensions={state({ entries: [{ ...before, id: "authority-after", value: "14" }] })} actions={handlers} onParameterEdit={vi.fn()} />);
    expect(screen.getByRole("textbox", { name: "Width value" })).toBe(input);
    expect(input).toHaveFocus();
    await user.clear(input); await user.type(input, "16{Enter}");
    expect(handlers.onEdit).toHaveBeenLastCalledWith("authority-after", "16");
  });

  it("keeps generated pins accessible after selection changes and reload", () => {
    render(<DimensionInspector dimensions={state({ pinCount: 1, entries: [dimension("Half width", { generated: true, pinned: true })], parameters: [] })} actions={actions()} onParameterEdit={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Unpin Half width" })).toBeVisible();
  });

  it("keeps inspection available in Hidden mode and during source drafts, but blocks edits", async () => {
    const handlers = actions(); const user = userEvent.setup();
    render(<DimensionInspector dimensions={state({ mode: "hidden" })} actions={handlers} editingBlocked="Apply or Revert the source draft before editing dimensions." onParameterEdit={vi.fn()} />);
    const section = within(screen.getByRole("region", { name: "Dimensions" }));
    expect(section.getByText(/Canvas dimensions are hidden/)).toBeVisible();
    expect(section.getByRole("textbox", { name: "Width value" })).toBeDisabled();
    await user.click(section.getByRole("button", { name: "Inspect Width" }));
    expect(handlers.onFocus).toHaveBeenCalledWith("Width");
    await user.click(section.getByRole("button", { name: "Pin Width" }));
    expect(handlers.onPin).toHaveBeenCalledWith("Width", true);
  });

  it("disables cross-view inspection during captured gestures and defaults legacy snapshots to an empty focused overview", () => {
    const view = render(<DimensionInspector dimensions={state()} actions={actions()} inspectionBlocked="Finish the current gesture." onParameterEdit={vi.fn()} />);
    expect(screen.getByRole("button", { name: "Inspect Width" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Pin Width" })).toBeDisabled();
    view.rerender(<DimensionInspector actions={actions()} onParameterEdit={vi.fn()} />);
    expect(screen.getByText(/Select geometry or pause over it/)).toBeVisible();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });
});
