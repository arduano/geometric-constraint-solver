// SPDX-License-Identifier: GPL-3.0-or-later
// Equation-free normalized history coordinates over native ManagedValue trees.
import { compilerMetadata, suppressionChanges } from "./collaboration-domain-structure.mjs";
export const sorted = (value) => Array.isArray(value) ? value.map(sorted) : value && typeof value === "object" ? Object.fromEntries(Object.keys(value).sort().map((key) => [key, sorted(value[key])])) : value;
export const same = (a, b) => JSON.stringify(sorted(a)) === JSON.stringify(sorted(b));
const absent = Object.freeze({ state: "absent" });
const fail = (message) => { throw Object.assign(Error(message), { code: "reconciliation_conflict" }); };
const prefix = (a, b) => a.length <= b.length && a.every((segment, index) => same(segment, b[index]));
const pathKey = (path) => JSON.stringify(sorted(path));
export const semanticPointLens = ({ owner, ...address }, codec) => sorted({ ...address, owner: owner.address, ...(codec ? { codec } : {}) });
export const pointPropertyKey = (address, codec) => JSON.stringify(["point", semanticPointLens(address, codec)]);
export function pointCodec(inventory, address) {
  const owner = address.owner.address, declaration = owner.owner === "direct_declaration" ? owner.declaration : owner.address.invocation;
  const payload = inventory.objects.find((item) => item.declaration === declaration)?.payload;
  if (!payload) fail("Point has no source-owned codec");
  return { builder: payload.statement.builder_path ?? ["binding"], patch: payload.statement.patch,
    ...(payload.statement.patch ? { context: payload.patchContext } : {}) };
}
const propertyKey = (path) => JSON.stringify(["source", sorted(path)]);
const nodeValue = (value) => ({ state: "value", value });
function flatten(value, path, out) {
  if (value.kind === "object") {
    const keys = Object.keys(value.value).sort(); out.set(pathKey(path), { path, value: { state: "object", keys } });
    for (const key of keys) flatten(value.value[key], [...path, key], out);
  } else if (value.kind === "array") {
    const keyed = value.value.length > 0 && value.value.every((child) => child.kind === "object" && child.value.key?.kind === "string");
    const members = value.value.map((child, index) => keyed ? { member: child.value.key.value } : index);
    if (new Set(members.map(pathKey)).size !== members.length) fail("Canonical array membership is ambiguous");
    out.set(pathKey(path), { path, value: { state: "array", members } });
    value.value.forEach((child, index) => flatten(child, [...path, members[index]], out));
  } else out.set(pathKey(path), { path, value: nodeValue(value) });
}
function rootValue(inventory, declaration, metadata) {
  const value = structuredClone(inventory.properties.find((item) => item.declaration === declaration && item.path.length === 0)?.value);
  if (!value) fail("Canonical source owner has no native root value");
  const item = inventory.objects.find((item) => item.declaration === declaration);
  const direct = item.payload.statement.statement === "declaration" && item.payload.statement.patch === null;
  if (direct && value.kind === "object") for (const field of metadata.filter((entry) => entry.target.target === "declaration" && entry.target.declaration === declaration)) delete value.value[field.property];
  return value;
}
export function canonicalSourceState(compiled, inventory) {
  const metadata = compilerMetadata(compiled), state = new Map();
  for (const object of inventory.objects) {
    const nodes = new Map(); flatten(rootValue(inventory, object.declaration, metadata), [], nodes);
    state.set(object.object, { declaration: object.declaration, nodes });
  }
  return { state, metadata };
}
export function canonicalPropertyChanges(beforeCompiled, before, afterCompiled, after, explicit = [], explicitMetadata) {
  const a = canonicalSourceState(beforeCompiled, before), b = canonicalSourceState(afterCompiled, after), result = [];
  for (const [object, current] of b.state) {
    const previous = a.state.get(object); if (!previous) continue;
    const scopes = explicit.filter((entry) => entry.declaration === current.declaration).map((entry) => entry.path);
    for (const key of new Set([...previous.nodes.keys(), ...current.nodes.keys()])) {
      const old = previous.nodes.get(key), next = current.nodes.get(key), path = (next ?? old).path;
      if (!same(old?.value ?? absent, next?.value ?? absent) || scopes.some((scope) => prefix(scope, path))) result.push({ object, property: propertyKey(path), before: old?.value ?? absent, after: next?.value ?? absent });
    }
  }
  const metadataKey = (entry) => JSON.stringify([entry.object, entry.target.target, entry.property]);
  const prior = new Map(a.metadata.map((entry) => [metadataKey(entry), entry]));
  for (const item of b.metadata) {
    const old = prior.get(metadataKey(item)); if (!old) continue;
    const declaration = item.target.declaration;
    const sourceObject = after.objects.find((entry) => entry.declaration === declaration);
    const direct = item.target.target === "declaration" && sourceObject?.payload.statement.patch === null;
    const explicitSource = direct && explicit.some((scope) => scope.declaration === declaration && prefix(scope.path, [item.property]));
    const metadataScopes = Array.isArray(explicitMetadata) ? explicitMetadata : [explicitMetadata];
    const owned = explicitSource || metadataScopes.some((scope) => scope?.mutation === "set_metadata" && same(scope.target, item.target) && scope.property === item.property);
    if (!same(old.value, item.value) || owned) result.push({ object: item.object, property: JSON.stringify(["metadata", item.target.target, item.property]), before: old.value, after: item.value });
  }
  result.push(...suppressionChanges(beforeCompiled, afterCompiled, explicitMetadata));
  return result;
}
export function deepestTouches(touches) {
  return touches.filter((entry) => !touches.some((other) => entry.declaration === other.declaration && entry.path.length < other.path.length && prefix(entry.path, other.path)));
}
export function canonicalSourceWrites(compiled, inventory, changes) {
  const { state, metadata } = canonicalSourceState(compiled, inventory), owners = new Map(), seen = new Set();
  for (const change of changes) {
    const [kind, path] = JSON.parse(change.address.property); if (kind !== "source") continue;
    if (!Array.isArray(path)) fail("Canonical source history path is malformed");
    const object = change.address.target.object, old = state.get(object); if (!old) fail("Canonical source inverse owner is absent");
    const key = pathKey(path), identity = JSON.stringify([object, key]);
    if (seen.has(identity)) fail("Canonical source property is duplicated"); seen.add(identity);
    if (!same(old.nodes.get(key)?.value ?? absent, change.before)) fail("Canonical source inverse value changed");
    let current = owners.get(object); if (!current) { current = { ...old, nodes: new Map(old.nodes) }; owners.set(object, current); }
    if (change.after?.state === "absent") current.nodes.delete(key); else current.nodes.set(key, { path, value: change.after });
  }
  const build = (nodes, path, visited) => {
    const key = pathKey(path), node = nodes.get(key)?.value; if (!node || visited.has(key)) fail("Canonical source reconstruction has missing or repeated nodes"); visited.add(key);
    if (node.state === "value" && node.value && !["array", "object"].includes(node.value.kind)) return node.value;
    if (node.state === "object" && Array.isArray(node.keys) && node.keys.every((key) => typeof key === "string") && new Set(node.keys).size === node.keys.length) return { kind: "object", value: Object.fromEntries(node.keys.map((key) => [key, build(nodes, [...path, key], visited)])) };
    if (node.state === "array" && Array.isArray(node.members) && new Set(node.members.map(pathKey)).size === node.members.length) return { kind: "array", value: node.members.map((member) => build(nodes, [...path, member], visited)) };
    fail("Malformed canonical source property value");
  };
  return [...owners].map(([object, owner]) => {
    const visited = new Set(), value = build(owner.nodes, [], visited); if (visited.size !== owner.nodes.size) fail("Canonical source inverse left orphaned member properties");
    const item = inventory.objects.find((entry) => entry.object === object);
    if (item.payload.statement.statement === "declaration" && item.payload.statement.patch === null && value.kind === "object") {
      for (const field of metadata.filter((entry) => entry.target.target === "declaration" && entry.target.declaration === owner.declaration && entry.value !== null)) value.value[field.property] = field.value;
    }
    return { declaration: owner.declaration, path: [], value };
  }).filter((write) => !same(write.value, inventory.properties.find((item) => item.declaration === write.declaration && item.path.length === 0)?.value));
}
