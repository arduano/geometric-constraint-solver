// SPDX-License-Identifier: GPL-3.0-or-later
import { render, screen, fireEvent } from "@testing-library/react";
import { expect, it, vi } from "vitest";
import { AuthoringOptions } from "./authoring-options";
import type { ConstructionFrame } from "../../../../../packages/geosolve-engine/src/construction";
import type { ToolOperationFrame } from "../../../../../packages/geosolve-engine/src/tool-operations";
const operation:ToolOperationFrame={sequence:0,completed:false,can_finish:false,has_pending:false,can_reset:false,can_step_back:false,diagnostic:null,pending:[],authoring_options:{tangent_orientation:"aligned",curvature_relation:"signed",continuity:{kind:"g1"},dimension_mode:"driving",angle_orientation:"counter_clockwise"},fillet_options:{fillet_radius:3,flip_first_side:false,flip_second_side:false,alternate_arc:false},fillet_corner_count:2,fillet_corners:[],offset_distance:5};
const construction:ConstructionFrame={sequence:1,completed:false,can_finish:false,has_pending:false,can_reset:false,can_step_back:false,can_flip_branch:true,can_cycle_inference:true,stage:"End",conic_options:{minor_axis_ratio:0.5,arc_start:0,arc_end:1,arc_sweep:"counter_clockwise",middle_weight:2,trim_start:-1,trim_end:1,semi_conjugate:3,hyperbola_branch:"positive"},nurbs_options:{form:"clamped",degree:3,weights:[1,2,1],gauge_index:0},preview:null,inference_guides:[],adjusted_position:null,diagnostic:null};
it("shows the active native family options and submits explicit choices without changing other options",()=>{
  const dispatch=vi.fn();const rendered=render(<AuthoringOptions tool="oriented-angle" context={{operation}} dispatch={dispatch}/>);
  fireEvent.change(screen.getByLabelText("Dimension"),{target:{value:"reference"}});
  expect(dispatch).toHaveBeenLastCalledWith("tool.operation.input",{event:"authoring_options",options:{...operation.authoring_options,dimension_mode:"reference"}});
  expect(screen.queryByLabelText("Radius")).not.toBeInTheDocument();
  rendered.rerender(<AuthoringOptions tool="fillet" context={{operation}} dispatch={dispatch}/>);
  fireEvent.change(screen.getByLabelText("Radius"),{target:{value:"7"}});fireEvent.blur(screen.getByLabelText("Radius"));
  expect(dispatch).toHaveBeenLastCalledWith("tool.operation.input",{event:"fillet_radius",radius:7});
  fireEvent.click(screen.getByLabelText("Alternate arc"));
  expect(dispatch).toHaveBeenLastCalledWith("tool.operation.input",{event:"fillet_options",options:{...operation.fillet_options,alternate_arc:true},selected_corner:null});
});
it("retains native weights and uses native capability flags for sweep and snap actions",()=>{
  const dispatch=vi.fn();const rendered=render(<AuthoringOptions tool="open-control-nurbs" context={{construction}} dispatch={dispatch}/>);
  expect(screen.getByLabelText("Control weights")).toHaveValue("1, 2, 1");
  fireEvent.change(screen.getByLabelText("Degree"),{target:{value:"2"}});fireEvent.blur(screen.getByLabelText("Degree"));
  expect(dispatch).toHaveBeenLastCalledWith("tool.construction.input",{event:"nurbs_options",options:{...construction.nurbs_options,degree:2}});
  fireEvent.click(screen.getByRole("button",{name:/Next snap/}));expect(dispatch).toHaveBeenLastCalledWith("tool.construction.input",{event:"cycle_inference"});
  rendered.rerender(<AuthoringOptions tool="segment" context={{construction:{...construction,can_flip_branch:false,can_cycle_inference:false}}} dispatch={dispatch}/>);
  expect(screen.queryByRole("button",{name:/Flip sweep/})).not.toBeInTheDocument();expect(screen.queryByRole("button",{name:/Next snap/})).not.toBeInTheDocument();expect(screen.queryByLabelText("Control weights")).not.toBeInTheDocument();
});
it("does not turn editing a value into Finish and preserves rejected native diagnostics",()=>{
  const dispatch=vi.fn();render(<AuthoringOptions tool="offset" context={{operation:{...operation,diagnostic:"Choose a closed profile"}}} dispatch={dispatch}/>);
  const value=screen.getByLabelText("Offset distance");fireEvent.change(value,{target:{value:"invalid"}});fireEvent.blur(value);
  expect(dispatch).not.toHaveBeenCalled();expect(screen.getByRole("status")).toHaveTextContent("Choose a closed profile");
  fireEvent.click(screen.getByRole("button",{name:"Flip offset"}));expect(dispatch).toHaveBeenCalledWith("tool.operation.input",{event:"offset_flip"});
});

it("edits an explicit Fillet corner using its own native branch choices",()=>{
  const dispatch=vi.fn();
  const options={...operation.fillet_options,flip_first_side:true,alternate_arc:true};
  render(<AuthoringOptions tool="fillet" context={{operation:{...operation,fillet_corners:[{index:1,options}]}}} dispatch={dispatch}/>);
  fireEvent.change(screen.getByLabelText("Branch target"),{target:{value:"1"}});
  expect(screen.getByLabelText("Flip first side")).toBeChecked();expect(screen.getByLabelText("Alternate arc")).toBeChecked();
  fireEvent.click(screen.getByLabelText("Flip second side"));
  expect(dispatch).toHaveBeenLastCalledWith("tool.operation.input",{event:"fillet_options",selected_corner:1,options:{...options,flip_second_side:true}});
  fireEvent.change(screen.getByLabelText("Branch target"),{target:{value:"next"}});
  expect(screen.getByLabelText("Alternate arc")).not.toBeChecked();
});
