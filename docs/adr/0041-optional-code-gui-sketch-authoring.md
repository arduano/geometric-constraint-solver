<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# ADR 0041: Optional code/GUI sketch authoring

Status: accepted for M84. Exact clean-qualified immutable M84-F012 is the current Tailscale UAT
candidate. F011 source
`e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
`015209773f81ec1a254817c65ef2a71b984e3b08`, snapshot
`/tmp/geosolve-m84-f011-uat.ps736NLh` remains historical rollback evidence. Exact F012 source
`84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
`429ed56d2a5b3988d6604079d19e1002f9049d64`, and snapshot
`/tmp/geosolve-m84-f012-uat.nMOymIIM` are current mechanical nomination authority; no UAT
acceptance is claimed. F010 source
`cf463838`, tree
`992e587`, snapshot `/tmp/geosolve-m84-f010-uat.7R5eXQoz`, F009 source `c74651c`, and F007 source
`cc2f05e` are withdrawn historical evidence. The direct-authoring `41e65a4` snapshot, combined
F005/F006 source `ff2e142`, initial `79078ec`, F003 `b9e67bad` and F004 `c2cf160` snapshots are
likewise historical. M84 remains active and unaccepted; GitHub Pages remains on accepted M83.

## Context

M83 provides one stable, order-independent Design Intent Graph, ordinary typed patches and a fast
native materializer/solver. Its TypeScript-shaped source is deliberately data-only. That is enough
for direct declarations, but not for reusable structural authoring such as “Fillet every keyed
corner of this Polyline and adapt when vertices are inserted.” Arbitrary TypeScript cannot be
deterministically reverse-edited by the GUI, and executing it in Rust/WASM would introduce a
second solver/runtime authority.

M84 must combine code-first abstraction, GUI-first direct manipulation and AI-authored reusable
patches without making any of them implement equations. Deployments that need only the solver,
sketch domain, intent graph or headless editor must not link the code system.

## Decision

### Optional adjacent modules

Add pure-Rust `geosolve-sketch-code` beside `geosolve-sketch-intent` and
`geosolve-constraint-editor`, plus `@geosolve/sketch-code` beside `@geosolve/intent`.

- `geosolve-sketch-code` depends only on public intent/editor APIs and owns code projects, managed
  parsing, artifacts, keyed expansion, reconciliation and code-session history.
- No core, sketch, linkage, intent, editor or demo-domain crate depends back on it. Plain editor
  sessions retain their current API, workspace-v8 persistence and history unchanged.
- `@geosolve/sketch-code` depends on `@geosolve/intent`. It supplies typed authoring/build tools,
  not geometry equations or a solver fallback.
- `geosolve-demo-web` may optionally compose the code crate for M84 demonstrations, but continues
  to consume public domain/audit APIs.
- Existing intent/editor layers may expose only neutral stable-member-key reconciliation,
  delegated-checkpoint and prepared-edit seams required by the wrapper. Those APIs contain no
  TypeScript, module, parser, source-file or custom-patch concept and remain useful without M84.

The authoritative code-enabled pipeline is:

```text
managed sketch.ts + pinned patch artifacts
  -> AuthoringProgram
  -> bounded equation-free keyed expansion
  -> ordinary IntentGraph
  -> unchanged Rust materializer and native solver
  -> independent finite/domain/branch/residual validation
```

The last independently validated native scene remains display/interaction authority beneath any
new retained code failure. No new residual, constraint, expression, priority or JavaScript solve
path is introduced.

### Project and source ownership

A code project contains:

```text
sketch.ts                          GeoSolve-managed, bidirectionally editable
patches/*.patch.ts                 user/AI-owned TypeScript; never rewritten by GeoSolve
geosolve.lock.json                 generated module/interface/artifact pins
.geosolve/artifacts/<digest>.json  canonical data-only PatchModuleArtifact values
```

`sketch.ts` begins with the exact directive `"use geosolve managed-v1";`. A bounded, lossless,
pure-Rust CST parser accepts only:

- static imports;
- one `sketch(($) => { ... })` body;
- single-name builder declarations with stable symbol strings;
- finite literal numbers, units, strings, booleans, `null`, objects and arrays;
- references to earlier declarations/outputs;
- closed organization calls and one final output record.

It rejects destructuring, spreads, computed/dynamic calls, loops, conditions, mutation, nested
functions, arbitrary expressions, `await`, dynamic imports, forward references and duplicate
symbols/keys. Managed source is limited to 4 MiB. Comments, formatting, imports and unrelated
spans are retained exactly.

GUI actions first produce an authenticated `ManagedEditPlan`, rewrite only owned CST spans, parse
and expand the candidate, cold-materialize it and publish source/program/graph once. Direct
declaration fields and references rewrite their declaration; organization changes rewrite only
`$.organize`; generated properties use declared edit lenses; a free generated-handle drag adds an
explicit managed `overrides` entry; Reset to code removes that override. A target without a lens
or permitted override is visibly read-only. An explicit conversion API may initialize a code
project from an M83 editor checkpoint using descriptor-backed managed declarations and honest
bootstrap declarations; conversion is never implicit.

Public `CodeProject::managed_only(ProjectKey, source)` is the smallest direct code-only host seam.
It accepts a complete managed-v1 source with a valid project brand and no pinned artifacts, and
rejects invalid source/brands or custom patch imports whose artifact authority is absent. This
constructor establishes project authority only; parsing alone never establishes geometry or
solver authority. The ordinary expansion, intent materialization, native solve and independent
validation path remains mandatory before a session can publish accepted geometry.

The demo workbench exposes this seam only for an exact canonical fresh workspace: current and
accepted semantic identities must match, the accepted native/computed scene must independently
validate as empty and only the canonical document foundation may exist. Code then shows one
**Start from code** action and eight genuine bundled-project cards. Starting creates a distinct
persisted `CodeProjectOrigin::Authored` project with complete editable artifact-free source, not a
promotion or fabricated GUI history. Its rectangle-plus-diagonal starter uses lexical
`frame.corners.*` dependencies. Valid Apply, retained-invalid intent, whole-source replacement,
Undo/Redo, persistence, reload and repro retain the existing atomic code-session authority. This
entry is not a ninth bundled demonstration and does not alter the eight-demo ledger.

The M83 `IntentSourceSnapshot` remains a data-only audit/RPC/Inspector projection and is labelled
**Intent IR** in code-enabled presentation. It is never presented as authoring TypeScript. The
optional conversion API projects supported declarations in dependency order and emits every
dependency as an earlier lexical branded value, for example
`start: frame.corners.lowerLeft`. It never embeds `{ declaration, output, kind }` transport objects
or repeats a dependency identity string in managed source. Promotion creates a genuine
`CodeProject`/`SketchCodeSession`; a read-only preview alone does not acquire code authority. The
all-or-nothing promotion may omit only the exact canonical fresh-workspace document foundation;
other bootstrap geometry remains unsupported rather than disappearing. Direct line expansion
aliases the exact referenced native points, and same-cell line-branch normalization is limited to
Segments owned by the current code expansion so ordinary GUI Segments remain explicit.

The same all-or-nothing conversion supports connected Segments through lexical `.start`/`.end`
members and an ordinary computed Fillet through a distinct direct `$.computed.filletSet`
declaration. Every corner contains exactly two ordered lexical `NativeCurveSpanRef` parents and
explicit parameter, winding, contact neighborhood, normal side, retained endpoint, periodic
anchor, endpoint order, sweep and suppression. The central Rust declaration-result descriptor
catalog, not handwritten TypeScript aliases, generates that brand only for direct line spans,
rectangle edges and Polyline segments. A computed host Fillet arc is deliberately not native-
branded and Rust lowering rejects any such host output even if static typing is bypassed. Radius
accepts a positive finite model-unit number or branded `mm(...)`; a forged unit record or another
unit spelling is outside managed-v1. Direct lowering reconstructs the existing Intent
`ComputedFeature::FilletSet` without invoking Fillet authoring heuristics or a new solver path. Its
result is an opaque `FilletSetFeature`, not a claim that evaluated child arcs are native curve-span
ports. One checked-in managed-v1 line/Horizontal/Vertical/line/Fillet source is shared by
TypeScript compilation, Rust parsing and cold materialization so those boundaries cannot validate
divergent fixtures.
Existing inferred Horizontal and Vertical constraints in that ordinary closure are represented by
managed `$.constraint.horizontal`/`$.constraint.vertical` calls over the same lexical native-span
members. Suppression is explicit and direct lowering selects the existing Intent constraint kind;
this is adapter coverage, not a new relation or equation.
Projection also requires the accepted validation semantic to equal the exact current retained
intent semantic identity. Retained-failed intent therefore renders a visible unavailable Code
state without Promote instead of combining prior accepted geometry with current wiring.

Code remains discoverable even when another unsupported declaration makes complete conversion
fail. In that state the workbench renders an escaped read-only conversion diagnostic, keeps Intent
IR available as the audit fallback and withholds Promote. Supported complete scenes show the
read-only managed preview and may be promoted atomically as before.

Invalid-subset text remains a non-canonical editor draft and changes no project or scene. Valid
source with a missing/tampered artifact, expansion error, dangling dependency or invalid geometry
is retained as failed code intent while the previous accepted scene stays visible and Undo remains
available.

### Custom patch artifacts

Custom files use `definePatch`, typed input schemas and ordinary trusted TypeScript helpers. The
package exposes structural recording combinators including `p.each` and `p.mapRecord`; symbolic
dynamic collections cannot be traversed with an unrecorded ordinary runtime loop.

An explicit caller-owned Node build step evaluates trusted custom modules and emits canonical
`PatchModuleArtifact` data. Rust, WASM, workspace load and the browser never evaluate those files.
Each artifact pins source and interface SHA-256, SDK ABI, typed schemas, edit lenses and a bounded
equation-free template DAG containing only existing declaration families. One artifact is at most
16 MiB; the complete code project remains within the existing 64 MiB workspace/reproduction
ceiling. Remote packages, network, time, randomness, async execution and browser `eval` are absent
from runtime authority. Artifacts are treated as untrusted bounded data and fully validated before
expansion.

### Typed references and outputs

The TypeScript surface exposes branded project-local references rather than raw wire IDs:

```ts
type FeatureRef<Project, Kind, Outputs>
type OutputRef<Project, PortKind>
type FeatureRecord<R extends Record<PropertyKey, FeatureRef<any, any, any>>>
type KeyedFeatureCollection<K extends PropertyKey, V> = {
  readonly keys: readonly K[];
  readonly byKey: Readonly<Record<K, V>>;
};
type DerivedFeatureCollection<Owner, Key, V>
```

Semantic references encode declaration symbol, semantic output path, expected port kind and
identity generation internally. A code-facing dependency must originate as a lexical declaration/
member expression whose parsed value is `ManagedValue::Reference`; branding a raw string is not
sufficient. Raw strings, transport DTOs, foreign-project and forged reserved-project references,
kind mismatch and misspelled members reject before expansion.
High-level result definitions are generated from the central Rust declaration descriptors and
checked for Rust/TypeScript schema parity. An axis-aligned rectangle exposes
`corners.{lowerLeft,lowerRight,upperRight,upperLeft}`,
`edges.{bottom,right,top,left}` and `profile`; other recipes use their descriptor-owned semantic
roles. A Fillet record preserves its input record keys in its mapped output type. A Fillet-every-
corner patch returns a collection keyed by the owning Polyline corner keys; statically known vertex
keys infer a literal key union, while externally variable collections use branded key lookup.
Direct Polyline lowering also publishes `vertices` and `segments` as keyed root collections beside
the existing exact member paths and `filletableCorners`. `vertices` maps every authored key to its
native Point port; `segments` maps each directed span's starting key to its native CurveSpan port.
Those roots may be returned directly or consumed by recorded artifact `each`/mapping rules without
copying coordinates, exposing raw native IDs or introducing ordinal identity.

### Keyed expansion and reconciliation

Every dynamic member has an explicit semantic key. Its stable path is:

```text
invocation identity / template path / member-key path / output path
```

Matching paths retain node, child, port, reservation, native, override and Fillet-corner
identities. New keys allocate above high-water. Removed keys receive durable tombstones. Reusing a
removed key outside Undo creates a new generation. Reordering keys changes presentation only.

For a Polyline, authored vertex keys are authoritative; a directed segment follows its starting
vertex key; each interior corner and generated Fillet uses the vertex key. Inserting a vertex
preserves one existing split-segment continuation and creates only the new keyed continuation.
Renaming a key is delete-plus-create. Removing a generated output with ordinary outside dependents
rejects with an exact diagnostic; it never cascades or retargets silently.

### Transactions, history and interaction

`SketchCodeSession` owns one bounded history checkpoint over code-project files, artifact locks,
the `AuthoringProgram`, expansion/provenance map, semantic interaction overlay and one nested editor
checkpoint. Its inner editor runs in delegated-history mode, so no mirrored Undo stack exists. One
accepted code, GUI, organization or terminal-drag action creates one composite history entry.

The bounded semantic interaction overlay stores finite Cartesian point drafts by project, semantic
owner, output path, writable field and never-reused owner generation. M84 exposes no writable
scalar overlay; scalar changes continue through authenticated managed-source lenses. The overlay
contains no intent/native ID and adds no equation, solver priority or constraint. Precedence is
explicit for one writable point: typed overlay draft > legacy generated override > managed source
seed. Reset removes the complete semantic edit bundle and restores the applicable lower tier.
Equal duplicate point writes in one terminal bundle collapse under deterministic audit-provenance
selection; unequal same-tier writes to one semantic address reject atomically.

Shared-point drag ownership is explicit. With no preferred semantic declaration, or with the
producer selected, ordinary producer ownership wins and attached consumers follow. A uniquely
selected referenced consumer detaches only that consumer. The detachment truthfully replaces its
projected Segment, so that Segment's intent/native identity may change; the code-owner generation
remains stable, and retained code-owned and ordinary GUI dependents rebind to the replacement.
Repeated drags of the detached consumer use its own point lens, and Undo/Redo restores and reapplies
the complete attachment/overlay/editor checkpoint. Rectangle-corner drags update the two canonical
point seeds atomically. Multiple matching lenses for the selected declaration reject without
changing source, overlay, accepted scene or history. Unknown, stale, type-mismatched or non-finite
drafts reject before expansion.

Transactions stage managed source, parsing, keyed expansion, graph construction, the existing cold
materializer and independent validation before one publication. Pointer frames never parse source,
expand patches, serialize projects or rebuild durable panels. They use the existing retained native
preview. Terminal placement publication reuses the newest authenticated accepted preview and must
match the staged cold result before committing once. Canvas selection maps projected declarations
back through accepted expansion provenance; it must never decode a hashed implementation alias.
A semantic-delete target captures the exact code-session identity and accepted expansion alias
together with either its managed declaration or its generation-authenticated generated-child
address. Execution reauthenticates that complete token, so a stale same-named declaration cannot
be deleted after another revision. Deleting a managed declaration edits its source declaration and
exact code-owned dependent closure, never only an expanded node. Surviving code-owned and ordinary
GUI dependents are retained/rebound. Deleting a generated child instead adds a reversible semantic
suppression and keeps its owning invocation. A dirty managed draft, retained code failure, stale
target or GUI-owned selection cannot use the semantic-delete route; GUI-owned selection remains on
ordinary editor deletion.

The code session stores separate current and accepted overlays. A parseable structural attempt
that later fails native publication retains its deterministically pruned current overlay: drafts
and generated-child suppressions whose owners disappeared are removed, while the exact accepted
overlay/editor scene remain authoritative. Save/reload/repro and Undo/Redo preserve both sides of
that retained-failure transaction.

Adding those authorities intentionally and incompatibly bumps the still-unreleased optional wire
identifiers to `geosolve-sketch-code-session-v2` and `geosolve-code-workbench-v2`. Older M84
prototype payloads reject instead of being silently reinterpreted; there is no compatibility claim
for an unaccepted milestone. The plain M83 workspace-v8 envelope and deployments that do not link
`geosolve-sketch-code` remain unchanged. The code-project persistence envelope stores every file,
artifact, lock, expansion provenance, current/accepted overlay, nested accepted intent checkpoint
and unified history, and validates the complete candidate before atomic replacement.

M84-F006 preserves this decision under adversarial inputs. Imported session identities are capped
before allocator adoption, persisted managed drafts retain the 4 MiB source ceiling, and Reset plus
Restore use typed canonical tokens. Retained-failure pruning authenticates direct and generated
owners exactly. Generated-reference detachment and replacement carry explicit provenance through
dependent rebinding, transient cancellation, repeated drag and Undo/Redo. Same-tier seed conflicts
compare persisted IEEE bits, so bit-identical values collapse while `+0.0` and `-0.0` reject in
either order. These checks harden persistence and transaction authority; they add no geometry
equation, constraint, priority, tolerance or branch rule.

M84-F007 makes pointer-down, rather than terminal checkpoint inference, the semantic edit authority.
One exact point lens is authenticated from accepted expansion provenance and retained across all
native preview frames. No selection chooses a unique producer, explicit producer selection retains
shared attachment and explicit consumer selection retains local detachment. That lens alone
authorizes release; solver-coupled motion cannot become independently authorized sibling intent.
The pending route stores the exact `CodeSessionIdentity`, pointer and lens. Only the dedicated
authenticated terminal publisher for that pointer may consume it: generic saves reject without
consumption, while foreign/reentrant preparation and foreign terminals reject while preserving the
original route. Every non-pointer durable workbench route cancels captured canvas authority before
deriving or changing durable state, independent of viewport lookup; platform capture release is
best-effort. A defensive generic-save rejection preserves a still-live native editor/token instead
of restoring underneath it. No-motion release/cancel is history-neutral; Apply/Undo invalidates a
stale terminal without reverting newer accepted authority.
Ordinary GUI-owned points retain the delegated editor path. This resolves terminal authority without
weakening F006's bit-exact conflict rule or adding solver behavior.

M84-F010 refines durability while preserving F007 authentication. A valid release compares the
exact authenticated origin—post-detachment for a selected consumer—to the accepted terminal
checkpoint, classifies only provenance-owned semantic Cartesian leaves and atomically stages the
complete solver-coupled movement closure. The authenticated lens must be present; unrelated sibling
edits remain forbidden. Cold rematerialization always proves parity for design and current accepted
documents, computed features, ownership and allocator state, using
`accepted_state_for_current_input()` as solved authority. Ordinary point aliases remain bit-exact.
Rectangle corners are four lenses over two seeds: the authenticated corner and diagonal opposite
are exact anchors; only the two redundant adjacent aliases may normalize under tightly bounded
finite roundoff. Different signed zero, non-finite values and material disagreement reject. This
changes transaction/parity policy only, not equations, constraints, priority, tolerance or branch
state.

M84-F008 corrects only code-project presentation and sample content. Project installation now fits
the camera to the accepted composed scene, falling back to the canonical Origin camera only for
empty or unavailable authority. Rounded Polyline uses radius `mm(4)` so its four already-valid
computed Fillets remain visibly separated from point markers. Native adapter regressions require
an off-origin accepted scene to fit inside the viewport and require exactly four finite, visibly
separated radius-4 Fillet paths. No architectural or numerical solver behavior changes.

M84-F009 makes custom-patch result routing explicit. The caller-owned compiler records an exact
nullable `result_output` on each data-only template, independently of the selected value's renamed
or nested public path. Rust publishes only that authenticated selection, preserves nested root
collections and no longer infers a value from a one-child prefix or canonical `BTreeMap` order.
Collection callbacks likewise record their selected output and fail closed when absent. This
preserves exact full output paths and prevents cases such as Mounting Plate `plate.profile`
resolving to the unrelated `ne` Point. Exact alias/nesting/mapping regressions and every bundled
output check both declared reference kind and expanded target kind. This changes only optional-layer semantic routing; native
geometry, solver equations, constraints, priority, tolerance and branch behavior remain unchanged.

M84-F012 does not change the optional-code architecture or any durable authoring contract. One
default-true transient flag on the shared headless scene DTO governs both constraint/dimension
annotation paint and annotation picking; flat and code workbenches expose it as session-local
presentation state. Hidden annotations, including selected/problem-forced ones, publish no hit
authority and cannot displace underlying geometry/datum pointer ownership, while derived layout and
Fillet affordances remain intact. The flag is absent from documents, accepted identities, history,
Intent IR, code sessions, persistence and repro. PNG export clones the already-composed SVG, so
annotation inclusion is WYSIWYG, and export-only cleanup strips draft/inference/provisional paint
without mutating the live canvas. This presentation/picking amendment adds no equation, branch,
priority or persistence schema.

## Consequences

- Reusable TypeScript can express higher-order structural design while the GUI safely rewrites a
  deliberately small managed surface.
- Dynamic topology inside a patch receives stable semantic naming and explicit generations rather
  than ordinal IDs, without claiming general B-rep topological naming.
- The optional layer is substantial, but it cannot weaken the existing native solver or plain
  deployment boundary.
- User-owned custom files remain code-owned. GeoSolve can change only managed calls, literals,
  organization, declared lens inputs and explicit overrides.
- Exact source `41e65a4f8c92179412ba2e06f44692377cd5fe51`, tree
  `d31b805549a29433e157074bc181517bdb50fb67`, and its immutable snapshot are historical
  direct-authoring evidence only. Combined F005/F006 source `ff2e142` and its frozen candidate are
  also withdrawn by F007. Historical F007 source
  `cc2f05ed97500f4bae4c0da6839362dbbc8c2e53`, tree
  `6b8fc417ac9464843ac14fb350a8e5f794b1cdb1`, passes the clean gate and is frozen without rebuild
  at `/tmp/geosolve-m84-f007-uat.KgW8fpLf`, aggregate
  `8f03810911b1ff96c4f825e005125250db804f463389953e937005ec505b7ab9`. Exact temporary and
  retained Tailscale verification plus the 14-case browser matrix pass on both endpoints; PID
  `62376` is retired. Withdrawn F009 source `c74651cc82506e31926042df65a1eeec08a6af9d`, tree
  `a904584410ca9a8cd3112d17ad70c0e84c29e8d9`, passes the complete clean gate. Its immutable
  no-rebuild snapshot `/tmp/geosolve-m84-f009-uat.q8cKIN3v`, aggregate
  `23f2f839f2a3be6b722ae26cb548f0a19ce2f3d6afac90d5f913938a042d1c1f`, passes byte-identical
  temporary/retained HTTP verification and the 14-case browser matrix on both endpoints. It is
  withdrawn by F010; PID `3965271` is retired and the snapshot is preserved.
- Historical F010 source `cf463838625e42ba9a0f58fe6e061dd7032c753d`, tree
  `992e587609e61768a9af76af193df2fad8325829`, passes the clean gate from
  15:58:23.055857854 through 16:17:18.303733569 AEST on 2026-08-27, exit 0. Its 6,209-line,
  420,425-byte log has SHA-256
  `bf57345266005a85b6da20f1105c3cf126d2492ba91e413ef0f07c5d38d3b28a` and final Trunk success;
  golden/M84-ledger SHA-256 values remain
  `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
  `bff42b987f8f8e09c941aa827baedf3a2ae793c5b93d37409e3bfa1eec8dff18`. Its exact no-rebuild
  seven-file snapshot `/tmp/geosolve-m84-f010-uat.7R5eXQoz`, modes `0555`/`0444`, aggregate
  `ca2302e0e0a1f08525be98202d593c72de1af303b664f6ff7a64a70727e7f72e`, is retained with evidence
  at `/tmp/geosolve-m84-f010-freeze-evidence.sXWXNG0Z`. Temporary/retained eight-path ledgers are
  byte-identical at SHA-256
  `57f2f4c2b47a11db8fc76a7f6a2e3d30555cb36e96b191081454a4c47fb85cbe`; focused Compass 1/1 and
  carried 14/14 browser cases pass on both endpoints. M84-F011 withdraws this nomination; PID
  `650971` and temporary PIDs `238809`/`621532` are retired, and the snapshot remains historical
  rollback evidence.
- Historical F011 rollback source `e28721a0ee4eeac1da44b65bf302d071d208178b`, tree
  `015209773f81ec1a254817c65ef2a71b984e3b08`, passes the complete clean gate from
  18:38:08.068586771 through 18:55:55.205076185 AEST on 2026-08-27, exit 0. Its 6,251-line,
  422,664-byte log has SHA-256
  `ceea545929196981e2790a822b384596642f63a6ef93ecea98f76799cdd7e353` and final Trunk success;
  golden/nine-demo-ledger SHA-256 values are
  `cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` and
  `c610a229e490467f59c9d57f334c96f23f61ea98a713eb2c98daa3c773eab66f`. Its exact no-rebuild
  seven-file snapshot `/tmp/geosolve-m84-f011-uat.ps736NLh`, modes `0555`/`0444`, aggregate
  `056193f4af17437da5430dc86059ad4c4b73ec62e959a461935ca29153b10fc2`, is retained with evidence
  at `/tmp/geosolve-m84-f011-freeze-evidence.4deymgss`. Temporary/retained eight-path ledgers are
  byte-identical at SHA-256
  `9339301ea57feb293a27256795344f88805046426e3659bae0f750b67b251b94`; final focused frozen
  manifold/PNG/authority cases pass 1/1 on each endpoint and preserve lifecycle, history length,
  project title and viewport markup authority. PID `1485656`, invocation
  `f04bc05089d94947b7a24d8ec6a6f26d`, served only these immutable bytes at
  `http://100.94.63.83:8080/`. M84-F012 withdraws this nomination; PID `1485656` is retired and the
  exact snapshot remains rollback evidence.
- Current F012 source `84dd7683cd082cc5f5cc8f0dd8231805cb2967a3`, tree
  `429ed56d2a5b3988d6604079d19e1002f9049d64`, passes the complete clean gate from
  20:36:27.840305756 through 21:01:30.826307821 AEST on 2026-08-27, exit 0. Its 6,274-line,
  424,393-byte log has SHA-256
  `04e35c73fe92ca3e089b87bd13b5221c60835b72c9eeba5ed38916b51150004a` and final Trunk success.
  Its exact seven-file, zero-symlink no-rebuild snapshot `/tmp/geosolve-m84-f012-uat.nMOymIIM`,
  modes `0555`/`0444`, aggregate
  `166abc1298220090ba4c8b0a37a176fb4f945cceae68771efbd601acc1970169`, is retained with evidence
  at `/tmp/geosolve-m84-f012-freeze-evidence.qua6ci1b`. Temporary/retained eight-path ledgers are
  byte-identical at SHA-256
  `66fcd4c852baab5290605066ec856239af7c4f033cef55a4dfd5fb86058645ba`; focused annotation paint/
  pick, authority-neutrality, exact-restoration and visible/hidden PNG cases pass 1/1 on each
  endpoint. Retained `geosolve-m84-uat.service`, PID `2241323`, invocation
  `b621b1a43b8c4ee281f1e8edddf10e57`, serves only this immutable snapshot at
  `http://100.94.63.83:8080/`; the temporary service is retired. Refreshed U1-U16, acceptance and
  public publication remain pending.

## Rejected alternatives

- **Arbitrary bidirectional TypeScript AST rewriting:** cannot preserve user meaning safely.
- **Executing custom code in browser/Rust:** creates nondeterministic runtime authority.
- **JavaScript constraints or formulas:** bypass the stable native solver and are out of scope.
- **Ordinal dynamic identities:** make insertion/reorder silently retarget features.
- **One mirrored code and editor history each:** permits divergent Undo authority.
- **Making code support a base-crate dependency:** prevents solver/editor-only deployments.
