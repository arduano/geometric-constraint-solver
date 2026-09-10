// SPDX-License-Identifier: GPL-3.0-or-later
import type { DeclarationRow, NavigationSnapshot } from "./adapter";
export interface SourceProjection {
  readonly path:string;readonly canonicalPath:string;
  readonly spans:readonly {declaration:string;canonical:{from:number;to:number};raw:{from:number;to:number}}[];
}
type Span={path:string;from:number;to:number};
function overlaps(a:{from:number;to:number},b:{from:number;to:number}){return a.from===a.to?a.from>=b.from&&a.from<b.to:a.from<b.to&&a.to>b.from;}
export function projectAuthoredSource(span:Span,projection?:SourceProjection):Span|undefined{
  if(!projection||span.path!==projection.canonicalPath)return undefined;
  const entries=projection.spans.filter(item=>overlaps(span,item.canonical));
  if(!entries.length)return undefined;
  return {path:projection.path,from:Math.min(...entries.map(item=>item.raw.from)),to:Math.max(...entries.map(item=>item.raw.to))};
}
export function projectCanonicalSource(span:Span,projection?:SourceProjection):Span{
  if(!projection||span.path!==projection.path)throw Error("This source file has no accepted sketch declarations");
  const entries=projection.spans.filter(item=>overlaps(span,item.raw));
  if(!entries.length)throw Error("Select a source declaration to show its sketch objects");
  return {path:projection.canonicalPath,from:Math.min(...entries.map(item=>item.canonical.from)),to:Math.max(...entries.map(item=>item.canonical.to))};
}
export function projectSourceNavigation(navigation:NavigationSnapshot|undefined,explorer:readonly DeclarationRow[],projection?:SourceProjection){
  const rows=(values:readonly DeclarationRow[]):DeclarationRow[]=>values.map(row=>({...row,source:row.source&&projectAuthoredSource(row.source,projection),children:rows(row.children),
    capabilities:{...row.capabilities,navigate:row.source&&!projectAuthoredSource(row.source,projection)?{enabled:false,reason:"This object has no authored declaration source range"}:row.capabilities.navigate}}));
  return {explorer:rows(explorer),navigation:navigation&&{...navigation,sources:navigation.sources.flatMap(span=>{const mapped=projectAuthoredSource(span,projection);return mapped?[mapped]:[];}),
    canNavigateSource:navigation.canNavigateSource&&!!projection,unavailableReason:projection?navigation.unavailableReason:"Accepted source correspondence is unavailable"}};
}
