// SPDX-License-Identifier: GPL-3.0-or-later
import {describe,it,expect} from "vitest";
import {projectAuthoredSource,projectCanonicalSource,type SourceProjection} from "./collaboration-source-projection";
const projection:SourceProjection={path:"parts/manifold.ts",canonicalPath:"sketch.ts",spans:[{declaration:"bore",canonical:{from:20,to:80},raw:{from:145,to:226}},{declaration:"other",canonical:{from:81,to:141},raw:{from:280,to:355}}]};
describe("authored/canonical declaration source correspondence",()=>{
  it("projects an inner normalized span to its actual authored declaration",()=>{
    expect(projectAuthoredSource({path:"sketch.ts",from:24,to:30},projection)).toEqual({path:"parts/manifold.ts",from:145,to:226});
    expect(projectCanonicalSource({path:"parts/manifold.ts",from:301,to:301},projection)).toEqual({path:"sketch.ts",from:81,to:141});
  });
  it("keeps multi-declaration selections and refuses unmapped files/comments",()=>{
    expect(projectCanonicalSource({path:"parts/manifold.ts",from:150,to:302},projection)).toEqual({path:"sketch.ts",from:20,to:141});
    expect(()=>projectCanonicalSource({path:"parts/manifold.ts",from:250,to:250},projection)).toThrow(/Select a source declaration/);
    expect(()=>projectCanonicalSource({path:"unrelated.ts",from:150,to:150},projection)).toThrow(/no accepted sketch/);
    expect(projectAuthoredSource({path:"sketch.ts",from:0,to:10},projection)).toBeUndefined();
  });
});
