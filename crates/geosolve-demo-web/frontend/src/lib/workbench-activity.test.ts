// SPDX-License-Identifier: GPL-3.0-or-later
import { afterEach, expect, it, vi } from "vitest";
import { WorkbenchActivity } from "./workbench-activity";

afterEach(() => vi.useRealTimers());

it("does not flash for fast work, appears at 500 ms and retains overlapping work", () => {
  vi.useFakeTimers();
  const activity = new WorkbenchActivity();
  const changes = vi.fn();
  const pending = vi.fn();
  activity.subscribePending(() => pending(activity.getPendingSnapshot()));
  const unsubscribe = activity.subscribe(() => changes(activity.getSnapshot()));
  const fast = activity.begin();
  vi.advanceTimersByTime(499); fast();
  vi.advanceTimersByTime(1000);
  expect(changes).not.toHaveBeenCalled();
  expect(pending.mock.calls).toEqual([[true], [false]]);
  const first = activity.begin();
  vi.advanceTimersByTime(400);
  const second = activity.begin();
  vi.advanceTimersByTime(99);
  expect(activity.getSnapshot()).toBe(false);
  vi.advanceTimersByTime(1);
  expect(changes.mock.calls).toEqual([[true]]);
  first(); first();
  expect(activity.getSnapshot()).toBe(true);
  second();
  expect(changes.mock.calls).toEqual([[true], [false]]);
  expect(pending.mock.calls).toEqual([[true], [false], [true], [false]]);
  unsubscribe();
  expect(vi.getTimerCount()).toBe(0);
});

it("clears on rejection or synchronous failure without swallowing the failure", async () => {
  vi.useFakeTimers();
  const activity = new WorkbenchActivity();
  let fail!: (error: Error) => void;
  const failure = Error("solve failed");
  const result = activity.track(() => new Promise((_resolve, reject) => { fail = reject; }));
  const assertion = expect(result).rejects.toBe(failure);
  vi.advanceTimersByTime(500);
  expect(activity.getSnapshot()).toBe(true);
  fail(failure);
  await assertion;
  expect(activity.getSnapshot()).toBe(false);
  await expect(activity.track(() => { throw failure; })).rejects.toBe(failure);
  expect(vi.getTimerCount()).toBe(0);
});

it("late completions after disposal cannot retire newer work and unsubscribing is safe", () => {
  vi.useFakeTimers();
  const activity = new WorkbenchActivity();
  const listener = vi.fn();
  const unsubscribe = activity.subscribe(listener);
  const stale = activity.begin();
  vi.advanceTimersByTime(500);
  activity.reset();
  const current = activity.begin();
  stale();
  vi.advanceTimersByTime(500);
  expect(activity.getSnapshot()).toBe(true);
  expect(listener).toHaveBeenCalledTimes(3);
  unsubscribe(); current();
  expect(listener).toHaveBeenCalledTimes(3);
  expect(activity.getSnapshot()).toBe(false);
  expect(vi.getTimerCount()).toBe(0);
});
