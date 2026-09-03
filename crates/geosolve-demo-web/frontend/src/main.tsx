// SPDX-License-Identifier: GPL-3.0-or-later
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import * as Tooltip from "@radix-ui/react-tooltip";
import App from "./App";
import { WasmWorkbenchAdapter, type JsonWorkbenchHandleConstructor } from "./lib/wasm-adapter";
import "./styles.css";

async function mount() {
  let adapter;
  if (import.meta.env.PROD && import.meta.env.VITE_GEOSOLVE_MOCK !== "1") {
    const wasm = await import("./generated/geosolve_demo_web.js");
    await wasm.default();
    const Handle = (wasm as unknown as { WorkbenchHandle?: JsonWorkbenchHandleConstructor }).WorkbenchHandle;
    if (!Handle) throw new Error("The generated WASM package does not expose WorkbenchHandle");
    adapter = new WasmWorkbenchAdapter(Handle);
  }
  createRoot(document.getElementById("root")!).render(<StrictMode><Tooltip.Provider delayDuration={450}><App adapter={adapter} /></Tooltip.Provider></StrictMode>);
}

void mount().catch((error: unknown) => {
  const root = document.getElementById("root")!;
  root.innerHTML = `<main class="bootstrap-error"><h1>Workbench unavailable</h1><p></p></main>`;
  root.querySelector("p")!.textContent = error instanceof Error ? error.message : String(error);
});
