// SPDX-License-Identifier: GPL-3.0-or-later
import { useSyncExternalStore } from "react";
import type { WorkbenchAdapter } from "../lib/adapter";

const idle = () => false;
const subscribeIdle = () => () => {};

export function useWorkbenchBusy(adapter: WorkbenchAdapter): boolean {
  return useSyncExternalStore(adapter.activity?.subscribe ?? subscribeIdle, adapter.activity?.getSnapshot ?? idle, idle);
}
