// SPDX-License-Identifier: GPL-3.0-or-later
import { footprint, type FootprintInputs } from "./generator";
import { EVALUATION_TIMEOUT_MS, type GenerateResponse } from "./protocol";

const form = document.querySelector<HTMLFormElement>("#settings")!;
const controls = document.querySelector<HTMLElement>("#controls")!;
const status = document.querySelector<HTMLElement>("#status")!;
const preview = document.querySelector<SVGSVGElement>("#preview")!;
const artwork = document.querySelector<SVGGElement>("#artwork")!;
const metrics = document.querySelector<HTMLElement>("#metrics")!;
const exportButton = document.querySelector<HTMLButtonElement>("#export")!;
const cancelButton = document.querySelector<HTMLButtonElement>("#cancel")!;
const empty = document.querySelector<HTMLElement>("#empty")!;
const inputFields = new Map<string, HTMLInputElement | HTMLSelectElement>();
let worker: Worker | undefined;
let timeout: ReturnType<typeof setTimeout> | undefined;
let debounce: ReturnType<typeof setTimeout> | undefined;
let revision = 0;
let accepted: Extract<GenerateResponse, { status: "accepted" }> | undefined;

for (const [name, definition] of Object.entries(footprint.inputs)) {
  const label = document.createElement("label");
  label.className = `control ${definition.type === "boolean" ? "toggle-control" : ""}`;
  const heading = document.createElement("span"); heading.className = "control-label";
  heading.textContent = definition.label ?? name;
  const help = document.createElement("small"); help.id = `help-${name}`; help.textContent = definition.description ?? "";
  let input: HTMLInputElement | HTMLSelectElement;
  if (definition.type === "choice") {
    input = document.createElement("select");
    for (const value of definition.choices) {
      const option = document.createElement("option"); option.value = value;
      option.textContent = ({ magnets: "6 mm magnet bores", screws: "3 mm screw bores", none: "No mounting holes" })[value] ?? value;
      input.append(option);
    }
    input.value = definition.default;
  } else {
    input = document.createElement("input");
    if (definition.type === "boolean") { input.type = "checkbox"; input.checked = definition.default; }
    else {
      input.type = "number"; input.required = true; input.value = String(definition.default);
      input.min = String(definition.min); input.max = String(definition.max);
      input.step = definition.type === "integer" ? "1" : "0.1";
      if ("unit" in definition) heading.textContent += ` · ${definition.unit}`;
    }
  }
  input.id = `input-${name}`; input.name = name; input.setAttribute("aria-label", definition.label ?? name); input.setAttribute("aria-describedby", help.id);
  if (definition.type === "boolean") label.append(input, heading, help);
  else label.append(heading, input, help);
  controls.append(label); inputFields.set(name, input);
}

function readInputs(): FootprintInputs {
  const values = Object.fromEntries([...inputFields].map(([name, input]) => [name,
    input instanceof HTMLInputElement ? input.type === "checkbox" ? input.checked : input.valueAsNumber : input.value,
  ]));
  return footprint.parseInputs(values);
}

function message(text: string, kind: "busy" | "ready" | "error" | "idle") {
  status.textContent = text; status.dataset.state = kind;
  preview.setAttribute("aria-busy", String(kind === "busy"));
}

function stop() {
  worker?.terminate(); worker = undefined;
  clearTimeout(timeout); timeout = undefined;
  clearTimeout(debounce); debounce = undefined;
  cancelButton.hidden = true;
}

function path(points: readonly (readonly [number, number])[]) {
  return points.map(([x, y], index) => `${index ? "L" : "M"}${x},${-y}`).join(" ") + " Z";
}

function draw(result: Extract<GenerateResponse, { status: "accepted" }>) {
  const regions = result.profiles.regions;
  const points = regions.flatMap((region) => region.outer);
  if (!points.length) throw Error("The accepted result contains no footprint");
  const xs = points.map(([x]) => x), ys = points.map(([, y]) => y);
  const left = Math.min(...xs), right = Math.max(...xs), bottom = Math.min(...ys), top = Math.max(...ys);
  const width = right - left, height = top - bottom;
  const margin = Math.max(width, height) * 0.12;
  preview.setAttribute("viewBox", `${left - margin} ${-top - margin} ${width + 2 * margin} ${height + 2 * margin}`);
  artwork.replaceChildren();
  for (const region of regions) {
    const outline = document.createElementNS("http://www.w3.org/2000/svg", "path");
    outline.setAttribute("d", [path(region.outer), ...region.holes.map(path)].join(" "));
    outline.setAttribute("fill-rule", "evenodd"); outline.setAttribute("class", "footprint");
    artwork.append(outline);
  }
  const line = (x1: number, y1: number, x2: number, y2: number) => {
    const element = document.createElementNS("http://www.w3.org/2000/svg", "line");
    for (const [key, value] of Object.entries({ x1, y1, x2, y2 })) element.setAttribute(key, String(value));
    element.classList.add("cell-guide"); artwork.append(element);
  };
  for (let column = 1; column < result.inputs.columns; column++) {
    const x = (column - result.inputs.columns / 2) * 42; line(x, -top, x, -bottom);
  }
  for (let row = 1; row < result.inputs.rows; row++) {
    const y = (row - result.inputs.rows / 2) * 42; line(left, y, right, y);
  }
  const boreCount = regions.reduce((count, region) => count + region.holes.length, 0);
  document.querySelector("#width")!.textContent = `${width.toFixed(1)} mm`;
  document.querySelector("#height")!.textContent = `${height.toFixed(1)} mm`;
  document.querySelector("#bores")!.textContent = String(boreCount);
  document.querySelector("#size")!.textContent = `${result.inputs.columns} × ${result.inputs.rows}`;
  metrics.hidden = false; empty.hidden = true;
  preview.dataset.inputDigest = result.inputDigest;
  preview.dataset.revision = String(result.revision);
}

function regenerate() {
  stop();
  const request = ++revision;
  let inputs: FootprintInputs;
  try { inputs = readInputs(); }
  catch (error) { message(`${error instanceof Error ? error.message : error}. ${accepted ? "Previous footprint retained." : "Check the values to begin."}`, "error"); return; }
  message(accepted ? "Updating footprint… Previous result remains available." : "Preparing your footprint…", "busy");
  cancelButton.hidden = false;
  const next = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  worker = next;
  next.onmessage = ({ data }: MessageEvent<GenerateResponse>) => {
    if (worker !== next || data.revision !== revision) return;
    stop();
    if (data.status === "rejected") { message(`${data.detail} ${accepted ? "Previous footprint retained." : "Adjust the settings and try again."}`, "error"); return; }
    try {
      draw(data); accepted = data; exportButton.disabled = false;
      message(`Ready · ${data.inputs.columns * data.inputs.rows} cells · ${Math.round(data.elapsedMs)} ms`, "ready");
    } catch (error) { message(String(error), "error"); }
  };
  next.onerror = (event) => {
    if (worker !== next) return;
    stop(); message(`Could not generate this footprint. ${event.message} ${accepted ? "Previous footprint retained." : ""}`, "error");
  };
  timeout = setTimeout(() => {
    if (worker !== next) return;
    stop(); message(`Generation took too long and was stopped. ${accepted ? "Previous footprint retained." : "Try fewer cells."}`, "error");
  }, EVALUATION_TIMEOUT_MS);
  next.postMessage({ revision: request, inputs });
}

form.addEventListener("submit", (event) => { event.preventDefault(); regenerate(); });
form.addEventListener("input", () => {
  stop(); revision++; // Supersede immediately, including while debounce is pending.
  message(accepted ? "Settings changed · previous footprint retained while updating…" : "Updating settings…", "busy");
  debounce = setTimeout(regenerate, 220);
});
cancelButton.addEventListener("click", () => { stop(); revision++; message(accepted ? "Update cancelled · previous footprint retained." : "Generation cancelled.", "idle"); });
document.querySelector("#reset")!.addEventListener("click", () => {
  for (const [name, input] of inputFields) {
    const value = footprint.inputs[name as keyof typeof footprint.inputs].default;
    if (input instanceof HTMLInputElement && input.type === "checkbox") input.checked = Boolean(value);
    else input.value = String(value);
  }
  regenerate();
});
exportButton.addEventListener("click", () => {
  if (!accepted) return;
  const blob = new Blob([JSON.stringify({ ...accepted.profiles, generator: { name: "modular-storage-footprint", inputs: accepted.inputs, input_digest: accepted.inputDigest } }, null, 2) + "\n"], { type: "application/json" });
  const url = URL.createObjectURL(blob); const link = document.createElement("a");
  link.href = url; link.download = `footprint-${accepted.inputs.columns}x${accepted.inputs.rows}.json`; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
});
window.addEventListener("pagehide", stop);
regenerate();
