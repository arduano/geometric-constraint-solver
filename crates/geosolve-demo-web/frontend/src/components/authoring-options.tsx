// SPDX-License-Identifier: GPL-3.0-or-later
import { useEffect, useState } from "react";
import type { WorkbenchSnapshot } from "../lib/adapter";
import type { ConstructionEvent } from "../../../../../packages/geosolve-engine/src/construction";
import type { ToolOperationEvent } from "../../../../../packages/geosolve-engine/src/tool-operations";
import { Button } from "./ui/button";

type Props = { tool: string; context: NonNullable<WorkbenchSnapshot["authoringContext"]>; disabled?: boolean; dispatch(command: string, payload?: unknown): void };
/** Values and available native actions belong to the retained Rust tool. Inputs
 * only submit explicit user choices; they never predict document geometry. */
export function AuthoringOptions({ tool, context, disabled, dispatch }: Props) {
  const construction = context.construction, operation = context.operation;
  const [corner,setCorner]=useState<number|null>(null);
  const selectedCorner=operation?.fillet_corners.find(candidate=>candidate.index===corner);
  useEffect(()=>{if(!selectedCorner)setCorner(null);},[tool,selectedCorner]);
  const construct = (input: ConstructionEvent) => dispatch("tool.construction.input", input);
  const operate = (input: ToolOperationEvent) => dispatch("tool.operation.input", input);
  const options = operation?.authoring_options, fillet = selectedCorner?.options??operation?.fillet_options;
  const dimension = ["point-distance", "segment-length", "radius", "diameter", "oriented-angle"].includes(tool);
  const diagnostic = construction?.diagnostic ?? operation?.diagnostic;
  return <fieldset disabled={disabled || construction?.completed || operation?.completed} aria-label="Active tool options" className="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-2 border-b border-border bg-surface px-3 py-2 text-xs">
    {construction && <>
      <Button size="compact" variant="ghost" disabled={!construction.can_step_back} onClick={() => dispatch("tool.step_back")}>Step back</Button>
      {construction.can_flip_branch && <Button size="compact" variant="ghost" onClick={() => construct({ event: "flip_branch" })}>Flip sweep <kbd>F</kbd></Button>}
      {construction.can_cycle_inference && <Button size="compact" variant="ghost" onClick={() => construct({ event: "cycle_inference" })}>Next snap <kbd>Tab</kbd></Button>}
      {construction.conic_options && tool === "rational-quadratic-conic" && <NumberOption label="Middle weight" value={construction.conic_options.middle_weight} onChange={middle_weight => construct({ event: "conic_options", options: { ...construction.conic_options, middle_weight } })} />}
      {construction.conic_options && ["parabola", "hyperbola"].includes(tool) && <>
        <NumberOption label="Trim start" value={construction.conic_options.trim_start} onChange={trim_start => construct({ event: "conic_options", options: { ...construction.conic_options, trim_start } })} />
        <NumberOption label="Trim end" value={construction.conic_options.trim_end} onChange={trim_end => construct({ event: "conic_options", options: { ...construction.conic_options, trim_end } })} />
        {tool === "hyperbola" && <><NumberOption label="Conjugate extent" value={construction.conic_options.semi_conjugate} onChange={semi_conjugate => construct({ event: "conic_options", options: { ...construction.conic_options, semi_conjugate } })} /><Choice label="Branch" value={construction.conic_options.hyperbola_branch} values={["positive", "negative"]} onChange={hyperbola_branch => construct({ event: "conic_options", options: { ...construction.conic_options, hyperbola_branch } })} /></>}
      </>}
      {construction.nurbs_options && ["open-control-nurbs", "periodic-control-nurbs"].includes(tool) && <>
        <NumberOption label="Degree" value={construction.nurbs_options.degree} onChange={degree => construct({ event: "nurbs_options", options: { ...construction.nurbs_options, degree } })} />
        <NumberOption label="Gauge index" value={construction.nurbs_options.gauge_index} onChange={gauge_index => construct({ event: "nurbs_options", options: { ...construction.nurbs_options, gauge_index } })} />
        <TextOption label="Control weights" value={construction.nurbs_options.weights.join(", ")} placeholder="Unit weights" validate={value => value.trim() && !value.split(/[\s,]+/u).map(Number).every(Number.isFinite) ? "Enter finite control weights" : undefined} onChange={value => { const weights = value.trim() ? value.split(/[\s,]+/u).map(Number) : []; if (weights.every(Number.isFinite)) construct({ event: "nurbs_options", options: { ...construction.nurbs_options, weights } }); }} />
      </>}
    </>}
    {operation && <Button size="compact" variant="ghost" disabled={!operation.can_reset} onClick={() => operate({ event: "reset" })}>Clear picks</Button>}
    {options && dimension && <Choice label="Dimension" value={options.dimension_mode} values={["driving", "reference"]} onChange={dimension_mode => operate({ event: "authoring_options", options: { ...options, dimension_mode } })} />}
    {options && tool === "oriented-angle" && <Choice label="Orientation" value={options.angle_orientation} values={["counter_clockwise", "clockwise"]} onChange={angle_orientation => operate({ event: "authoring_options", options: { ...options, angle_orientation } })} />}
    {options && ["tangent", "continuity"].includes(tool) && <Choice label="Tangents" value={options.tangent_orientation} values={["aligned", "opposed"]} onChange={tangent_orientation => operate({ event: "authoring_options", options: { ...options, tangent_orientation } })} />}
    {options && tool === "continuity" && <>
      <Choice label="Continuity" value={options.continuity.kind} values={["g0", "g1", "g2", "parametric_c2"]} onChange={kind => operate({ event: "authoring_options", options: { ...options, continuity: kind === "parametric_c2" ? { kind, first_rate: 1, second_rate: 1 } : { kind } } })} />
      {options.continuity.kind === "parametric_c2" && <><NumberOption label="First rate" value={options.continuity.first_rate} onChange={first_rate => operate({ event: "authoring_options", options: { ...options, continuity: { ...options.continuity as Extract<typeof options.continuity, { kind: "parametric_c2" }>, first_rate } } })} /><NumberOption label="Second rate" value={options.continuity.second_rate} onChange={second_rate => operate({ event: "authoring_options", options: { ...options, continuity: { ...options.continuity as Extract<typeof options.continuity, { kind: "parametric_c2" }>, second_rate } } })} /></>}
      <Choice label="Curvature" value={options.curvature_relation} values={["signed", "magnitude_same_sign", "magnitude_opposite_sign"]} onChange={curvature_relation => operate({ event: "authoring_options", options: { ...options, curvature_relation } })} />
    </>}
    {operation && fillet && tool === "fillet" && <>
      {operation.fillet_corners.length>0&&<label className="flex items-center gap-1.5">Branch target<select className="h-7 rounded border border-border bg-raised px-1.5" value={corner??"next"} onChange={event=>setCorner(event.target.value==="next"?null:Number(event.target.value))}><option value="next">Next corner</option>{operation.fillet_corners.map(corner=><option key={corner.index} value={corner.index}>Corner {corner.index+1}</option>)}</select></label>}
      <NumberOption label="Radius" value={fillet.fillet_radius ?? undefined} onChange={radius => operate({ event: "fillet_radius", radius })} />
      <Toggle label="Flip first side" value={fillet.flip_first_side} onChange={flip_first_side => operate({ event: "fillet_options", options: { ...fillet, flip_first_side }, selected_corner: corner })} />
      <Toggle label="Flip second side" value={fillet.flip_second_side} onChange={flip_second_side => operate({ event: "fillet_options", options: { ...fillet, flip_second_side }, selected_corner: corner })} />
      <Toggle label="Alternate arc" value={fillet.alternate_arc} onChange={alternate_arc => operate({ event: "fillet_options", options: { ...fillet, alternate_arc }, selected_corner: corner })} />
      <span className="text-muted">{operation.fillet_corner_count} {operation.fillet_corner_count === 1 ? "corner" : "corners"}</span>
    </>}
    {operation && tool === "offset" && <><NumberOption label="Offset distance" value={operation.offset_distance ?? undefined} onChange={distance => operate({ event: "offset_distance", distance })} /><Button size="compact" variant="ghost" onClick={() => operate({ event: "offset_flip" })}>Flip offset</Button></>}
    {diagnostic && <span role="status" className="basis-full text-amber-200">{diagnostic}</span>}
  </fieldset>;
}
function Choice<T extends string>({ label, value, values, onChange }: { label: string; value: T; values: readonly T[]; onChange(value: T): void }) {
  return <label className="flex items-center gap-1.5">{label}<select className="h-7 rounded border border-border bg-raised px-1.5" value={value} onChange={event => onChange(event.target.value as T)}>{values.map(value => <option key={value} value={value}>{value.replaceAll("_", " ")}</option>)}</select></label>;
}
function Toggle({ label, value, onChange }: { label: string; value: boolean; onChange(value: boolean): void }) {
  return <label className="flex items-center gap-1.5"><input type="checkbox" checked={value} onChange={event => onChange(event.target.checked)} />{label}</label>;
}
function NumberOption({ label, value, onChange }: { label: string; value?: number; onChange(value: number): void }) {
  return <TextOption label={label} value={value === undefined ? "" : String(value)} validate={text => !text.trim() || !Number.isFinite(Number(text)) ? "Enter a finite number" : undefined} onChange={text => { if (text.trim() && Number.isFinite(Number(text))) onChange(Number(text)); }} />;
}
function TextOption({ label, value, placeholder, validate, onChange }: { label: string; value: string; placeholder?: string; validate?(value:string):string|undefined; onChange(value: string): void }) {
  const [draft, setDraft] = useState(value);
  const [invalid,setInvalid]=useState<string>();
  useEffect(() => {setDraft(value);setInvalid(undefined);}, [value]);
  const commit = () => { if (draft !== value) {const issue=validate?.(draft);setInvalid(issue);if(!issue)onChange(draft);} };
  return <label className="flex items-center gap-1.5">{label}<input className="h-7 w-24 rounded border border-border bg-raised px-1.5" value={draft} placeholder={placeholder} aria-invalid={Boolean(invalid)} title={invalid} onChange={event => {setDraft(event.target.value);setInvalid(undefined);}} onBlur={commit} onKeyDown={event => { if (event.key === "Enter") { event.preventDefault(); event.stopPropagation(); event.currentTarget.blur(); } else if (event.key === "Escape") { event.stopPropagation(); setDraft(value);setInvalid(undefined); } }} />{invalid&&<span className="text-amber-200" role="alert">{invalid}</span>}</label>;
}
