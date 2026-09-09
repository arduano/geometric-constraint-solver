// SPDX-License-Identifier: GPL-3.0-or-later
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { AuthoringEditContext, useAuthoringField, type AuthoringEditLifecycle } from "./authoring-edit";

function Field({ value, onCommit }: { value: string; onCommit: (value: string) => void }) {
  const field = useAuthoringField("Description", value);
  return <textarea aria-label="Description" value={field.value} onFocus={() => field.focus()}
    onChange={(event) => field.change(event.currentTarget.value)}
    onBlur={(event) => field.submit(event.currentTarget.value, () => onCommit(event.currentTarget.value))}
    onKeyDown={(event) => {
      if (event.key === "Escape") field.cancel();
      if (event.key === "Enter" && event.ctrlKey) {
        const value = event.currentTarget.value;
        field.submit(value, () => onCommit(value));
      }
    }} />;
}

describe("authoring field intent", () => {
  it("retains unsubmitted multiline input when a different accepted value arrives", () => {
    const lifecycle: AuthoringEditLifecycle = { beginFieldEdit: vi.fn(), changeFieldEdit: vi.fn(), cancelFieldEdit: vi.fn(), commitFieldEdit: (_id, action) => action() };
    const onCommit = vi.fn();
    const component = (value: string) => <AuthoringEditContext.Provider value={lifecycle}><Field value={value} onCommit={onCommit} /></AuthoringEditContext.Provider>;
    const view = render(component("before"));
    const field = screen.getByRole("textbox");
    fireEvent.focus(field); fireEvent.change(field, { target: { value: "draft\nsecond line" } });
    vi.mocked(lifecycle.cancelFieldEdit).mockClear();
    view.rerender(component("external"));
    expect(field).toHaveValue("draft\nsecond line");
    expect(lifecycle.cancelFieldEdit).not.toHaveBeenCalled();
    fireEvent.keyDown(field, { key: "Escape" });
    expect(field).toHaveValue("external");
    expect(lifecycle.cancelFieldEdit).toHaveBeenCalledTimes(1);
    expect(onCommit).not.toHaveBeenCalled();
  });

  it("acknowledges an unchanged submission once while preserving later edits and cancel", () => {
    const onCommit = vi.fn();
    const view = render(<Field value="before" onCommit={onCommit} />);
    const field = screen.getByRole("textbox");
    fireEvent.change(field, { target: { value: "first" } }); fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    fireEvent.blur(field); expect(onCommit).toHaveBeenCalledExactlyOnceWith("first");
    fireEvent.change(field, { target: { value: "second" } });
    view.rerender(<Field value="first" onCommit={onCommit} />);
    expect(field).toHaveValue("second");
    fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    expect(onCommit).toHaveBeenLastCalledWith("second");
    view.rerender(<Field value="second" onCommit={onCommit} />);
    expect(field).toHaveValue("second");
    fireEvent.blur(field); expect(onCommit).toHaveBeenCalledTimes(2);
  });

  it("a newly changed draft may retry the same text after an earlier rejected submission", () => {
    const onCommit = vi.fn();
    render(<Field value="before" onCommit={onCommit} />);
    const field = screen.getByRole("textbox");
    fireEvent.change(field, { target: { value: "retry" } }); fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    fireEvent.blur(field); expect(onCommit).toHaveBeenCalledTimes(1);
    fireEvent.change(field, { target: { value: "retry changed" } });
    fireEvent.change(field, { target: { value: "retry" } }); fireEvent.keyDown(field, { key: "Enter", ctrlKey: true });
    expect(onCommit).toHaveBeenCalledTimes(2);
  });
});
