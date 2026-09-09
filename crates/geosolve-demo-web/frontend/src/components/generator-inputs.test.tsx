// SPDX-License-Identifier: GPL-3.0-or-later
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AuthoringEditContext, type AuthoringEditLifecycle } from "../lib/authoring-edit";
import { GeneratorInputs, type GeneratorInputDefinition } from "./generator-inputs";

const definitions: Record<string, GeneratorInputDefinition> = {
  columns: { type: "integer", default: 3, min: 1, max: 8, label: "Columns", description: "Grid cells across." },
  clearance: { type: "number", default: 0.5, min: 0, max: 2, unit: "mm", label: "Clearance" },
  rounded: { type: "boolean", default: true, label: "Rounded corners", description: "Round the outer profile." },
  mounting: { type: "choice", choices: ["magnets", "screws", "none"], default: "magnets", label: "Mounting" },
  title: { type: "string", default: "Tray", label: "Part name" },
};
const defaults = { columns: 3, clearance: 0.5, rounded: true, mounting: "magnets", title: "Tray" };
const field = (label: string) => screen.getByRole("textbox", { name: label });
const apply = (label: string) => fireEvent.click(screen.getByRole("button", { name: `Apply ${label}` }));

describe("source-driven generator inputs", () => {
  it("renders all input kinds and source presentation with explicit application", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    render(<GeneratorInputs definitions={definitions} values={{}} onApply={onApply} />);
    expect(field("Columns")).toHaveValue("3");
    expect(field("Columns")).toHaveAccessibleDescription("Grid cells across. Whole number, minimum 1, maximum 8.");
    expect(field("Clearance")).toHaveValue("0.5");
    expect(screen.getByText("mm")).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Rounded corners" })).toBeChecked();
    expect(screen.getByRole("checkbox")).toHaveAccessibleDescription("Round the outer profile.");
    expect(screen.getByRole("combobox", { name: "Mounting" })).toHaveValue("magnets");
    expect(field("Part name")).toHaveValue("Tray");
    expect(screen.getByRole("button", { name: "Apply Columns" })).toBeDisabled();
    fireEvent.change(field("Columns"), { target: { value: "4" } });
    fireEvent.blur(field("Columns"));
    expect(onApply).not.toHaveBeenCalled();
    apply("Columns");
    await waitFor(() => expect(onApply).toHaveBeenLastCalledWith({ ...defaults, columns: 4 }));
    fireEvent.change(field("Clearance"), { target: { value: "1.25" } }); apply("Clearance");
    await waitFor(() => expect(onApply).toHaveBeenLastCalledWith({ ...defaults, clearance: 1.25 }));
    fireEvent.click(screen.getByRole("checkbox")); apply("Rounded corners");
    await waitFor(() => expect(onApply).toHaveBeenLastCalledWith({ ...defaults, rounded: false }));
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "screws" } }); apply("Mounting");
    await waitFor(() => expect(onApply).toHaveBeenLastCalledWith({ ...defaults, mounting: "screws" }));
    fireEvent.change(field("Part name"), { target: { value: "My tray" } });
    fireEvent.keyDown(field("Part name"), { key: "Enter" });
    await waitFor(() => expect(onApply).toHaveBeenLastCalledWith({ ...defaults, title: "My tray" }));
  });

  it.each([
    ["Columns", "1.5", "whole number"],
    ["Columns", "0", "at least 1"],
    ["Columns", "9", "at most 8"],
    ["Columns", "9007199254740992", "safe integer range"],
    ["Clearance", "", "finite number"],
    ["Clearance", "abc", "finite number"],
    ["Clearance", "Infinity", "finite number"],
    ["Clearance", "-0.1", "at least 0"],
    ["Clearance", "2.1", "at most 2"],
  ])("retains invalid %s text %s without a request", (label, value, message) => {
    const onApply = vi.fn();
    render(<GeneratorInputs definitions={definitions} values={{}} onApply={onApply} />);
    fireEvent.change(field(label), { target: { value } }); apply(label);
    expect(field(label)).toHaveValue(value);
    expect(field(label)).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("alert")).toHaveTextContent(message);
    expect(onApply).not.toHaveBeenCalled();
  });

  it("binds focus, changes and application to the shared folder field lifecycle", async () => {
    let committing = false;
    const lifecycle: AuthoringEditLifecycle = {
      beginFieldEdit: vi.fn(), changeFieldEdit: vi.fn(), cancelFieldEdit: vi.fn(),
      commitFieldEdit: vi.fn((_id, action) => { committing = true; action(); committing = false; }),
    };
    const onApply = vi.fn(async () => { expect(committing).toBe(true); });
    render(<AuthoringEditContext.Provider value={lifecycle}><GeneratorInputs definitions={definitions} values={{ columns: 5 }} onApply={onApply} /></AuthoringEditContext.Provider>);
    fireEvent.focus(field("Columns"));
    expect(lifecycle.beginFieldEdit).toHaveBeenCalledWith(expect.any(String), "Columns");
    fireEvent.change(field("Columns"), { target: { value: "6" } }); apply("Columns");
    expect(lifecycle.changeFieldEdit).toHaveBeenCalledWith(expect.any(String), "Columns", "6");
    expect(lifecycle.commitFieldEdit).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(onApply).toHaveBeenCalledWith({ ...defaults, columns: 6 }));
  });

  it("retains a rejected draft and its error and permits retrying unchanged text", async () => {
    const onApply = vi.fn().mockRejectedValueOnce(new Error("Disk changed; reload before retrying.")).mockResolvedValue(undefined);
    render(<GeneratorInputs definitions={definitions} values={{}} onApply={onApply} />);
    fireEvent.change(field("Columns"), { target: { value: "5" } }); apply("Columns");
    expect(await screen.findByRole("alert")).toHaveTextContent("Disk changed; reload before retrying.");
    expect(field("Columns")).toHaveValue("5");
    await waitFor(() => expect(screen.getByRole("button", { name: "Apply Columns" })).toBeEnabled());
    apply("Columns");
    await waitFor(() => expect(onApply).toHaveBeenCalledTimes(2));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("preserves a newer draft when an older submitted result arrives", async () => {
    let finish: (() => void) | undefined;
    const onApply = vi.fn(() => new Promise<void>((resolve) => { finish = resolve; }));
    const view = render(<GeneratorInputs definitions={definitions} values={{}} onApply={onApply} />);
    fireEvent.change(field("Columns"), { target: { value: "4" } }); apply("Columns");
    fireEvent.change(field("Columns"), { target: { value: "5" } });
    view.rerender(<GeneratorInputs definitions={definitions} values={{ columns: 4 }} onApply={onApply} />);
    await act(async () => finish?.());
    expect(field("Columns")).toHaveValue("5");
    apply("Columns");
    expect(onApply).toHaveBeenLastCalledWith({ ...defaults, columns: 5 });
    await act(async () => finish?.());
  });

  it("preserves pending text through lost lease and background accepted changes", () => {
    const onApply = vi.fn();
    const view = render(<GeneratorInputs definitions={definitions} values={{}} onApply={onApply} />);
    fireEvent.change(field("Columns"), { target: { value: "6" } });
    view.rerender(<GeneratorInputs definitions={definitions} values={{ columns: 4 }} disabledReason="Another tab owns this folder." onApply={onApply} />);
    expect(field("Columns")).toHaveValue("6");
    expect(field("Columns")).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent("Another tab owns this folder.");
    apply("Columns"); expect(onApply).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Revert Columns" }));
    expect(field("Columns")).toHaveValue("4");
    view.rerender(<GeneratorInputs definitions={definitions} values={{ columns: 4 }} onApply={onApply} />);
    expect(field("Columns")).toBeEnabled();
  });

  it("canonicalizes a value equal to accepted input and cancels the draft safely", () => {
    const lifecycle: AuthoringEditLifecycle = { beginFieldEdit: vi.fn(), changeFieldEdit: vi.fn(), cancelFieldEdit: vi.fn(), commitFieldEdit: vi.fn() };
    const onApply = vi.fn();
    render(<AuthoringEditContext.Provider value={lifecycle}><GeneratorInputs definitions={definitions} values={{}} onApply={onApply} /></AuthoringEditContext.Provider>);
    vi.mocked(lifecycle.cancelFieldEdit).mockClear();
    fireEvent.change(field("Columns"), { target: { value: "3.0" } }); apply("Columns");
    expect(field("Columns")).toHaveValue("3");
    expect(lifecycle.cancelFieldEdit).toHaveBeenCalledTimes(1);
    expect(lifecycle.commitFieldEdit).not.toHaveBeenCalled();
    expect(onApply).not.toHaveBeenCalled();
    fireEvent.change(field("Part name"), { target: { value: "Draft" } });
    fireEvent.keyDown(field("Part name"), { key: "Escape" });
    expect(field("Part name")).toHaveValue("Tray");
  });
});
