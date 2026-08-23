<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M83 implementation — Authoritative sketch lineage and deterministic rematerialization

Status: **architecture implementation and read-only Lineage-panel amendment complete; replacement
clean qualification and human UAT pending; not accepted**. ADR 0039 and `docs/M83_GOALS.md` own the
scope. M81 remains accepted product authority until a replacement immutable M83 candidate passes
focused human UAT and receives explicit supervising-human approval. GitHub Pages publication is
deliberately deferred until that approval.

## Product boundary

M83 adds one declarative authority above the existing independently validated domains:

- `geosolve-sketch-lineage` owns canonical action programs, stable typed ports and reservations,
  exact-CAS patches, retained/latest-attempt/last-accepted authority, and one Undo/Redo history;
- `geosolve-constraint-editor` compiles complete workbench actions into lineage, materializes them
  back through ordinary sketch/operation/feature APIs, and performs action-owner reconciliation;
- workspace v7 persists lineage authority and treats all flat sketch/feature/map data as a
  disposable authenticated cache; and
- `geosolve-sketch-lineage-wasm` exposes the same stateful DOM-free engine through
  `geosolve.lineage.rpc.v0`, with a private data-only TypeScript client in
  `packages/geosolve-lineage`.

The existing flat Rust sketch APIs remain supported for low-level embedders. The sole demo
workbench no longer treats its flat coordinator state as peer persistent authority. M83 adds no
residual, Jacobian, constraint equation, rank rule, priority rule, branch heuristic, or success
shortcut.

## Implementation ledger

### L1 — canonical lineage and exact identity flow

The new pure safe-Rust lineage crate provides:

- bounded deterministic lineage-document, session and materialization-map codecs;
- monotonic document, step, output, reservation and revision identities;
- typed owned, aliased, created, continued and retired output flow;
- dependency/cycle, kind, namespace, reservation, lifecycle and allocator validation;
- exact-revision insert, atomic rewrite, suppress, tombstone, cascade/rebind, reorder and complete-
  program reconcile mutations; and
- retained program, latest evaluation attempt, last accepted program/materialization evidence,
  never-reuse lifecycle high-waters and one bounded Undo/Redo history.

`LineageMaterializationMap` distinguishes complete declared ownership from current live writable
authority. Suppression and tombstoning retain declarations/reservations while removing live
reverse bindings. Exact writable-leaf declarations keep separate fields of one persistent object,
such as point X and Y, distinct; coordinator reconciliation authenticates changed leaves through
that reverse map before emitting one atomic multi-owner rewrite.

Intrinsic Origin/X/Y references are document-bound immutable action parameters. They are not
step-output `LineageInputBinding`s because no lineage step owns them; datum-backed relations still
own ordinary Constraint and Source outputs.

### L2 — atomic native materialization

`geosolve-sketch` exposes a narrow reserved-ID materialization batch. It validates the exact base
document namespace and digest, requested kind/order/count, duplicates, reference closure, source
ownership, spline-span cursors and merged identity high-water before one atomic publication. It is
not a general unchecked explicit-ID insertion API.

Canonical parameter and external-snapshot payloads retain exact host-input provenance needed by
historical prefix evaluation. Retained sessions separately preserve the payload used by the latest
attempt and the payload that produced accepted authority when a newer host-input attempt fails.
`SketchDocument::set_scalar_values` supplies one bounded atomic candidate-validation seam for
restoring several host-owned local scalar fallbacks whose intermediate one-at-a-time states would
be invalid. It preserves the existing contact, active-Fillet-angle and NURBS-gauge ownership guards.

### L3 — complete action catalog

The coordinator bridge covers the frozen M81 surface without a fallback family:

- all 25 `GeometryToolVariant` recipes, including modifiers, intrinsic relations, aliases,
  variable-cardinality children, roles and explicit recipe branches;
- all 35 persistent constraint definitions and all eight dimension definitions in both admitted
  Driving and Reference modes;
- every selected-curve control/property, rational ordinary/projective mode, role, activation and
  explicit curve/contact/Fillet/Offset branch family;
- all 12 `SketchOperationKind`s with typed operands and created/continued/retired identity flow;
- native Fillet and Profile Offset as native materializations; and
- computed `FilletSet` feature/corner intent, while generated arcs/fragments remain authenticated
  revision-local output rather than false persistent native identity.

One separately reviewed 195-row lineage-action catalog freezes those mappings. It complements,
rather than replaces, the milestone-neutral 271-row authoring/scene golden.

### L4 — strict/local evaluation and failure authority

`StrictChronological` rebuilds every ready prefix from the imported root and independently solves
and validates it through the sketch and computed-feature owners. Historical non-final prefixes use
their exact authenticated host-input provenance; the final prefix uses the exact caller-supplied
inputs. The accepted digest covers evaluator identity, lineage/input identity, retained and
accepted sketch bytes, computed intent/result evidence and every strict prefix.

Accepted prefix evidence is threaded chronologically into topology-sensitive continuation. Native
Fillet is the first such action: the sketch owner reauthenticates its prepared plan and exact
materialized topology against the accepted upstream prefix before producing a numerical seed.
Direct manipulation likewise compares the staged flat checkpoint to genuine cold accepted bytes,
not to a caller-supplied witness certified circularly.

`DependencyLocal` computes the exact dirty dependency closure, reconstructs an authenticated
unchanged prefix without allocator regression, replays the structural suffix, and evaluates only
the required policy checkpoints. A mandatory cold strict comparison gates publication. Policy
telemetry reports reusable/reconstructed prefix work, replayed suffix work, exact dirty steps and
the separate strict-oracle charge.

A structurally valid edit may advance retained lineage and history when strict cold owning-domain
evaluation rejects it; the prior complete accepted lineage/materialization remains authority. A
provisional live rejection alone is not authoritative: if strict cold replay exactly accepts the
retained program, that canonical graph promotes the same design/attempt and becomes current.
Conversely, a provisional live success cannot survive a strict cold rejection. Stale, cancelled,
exhausted, malformed or non-finite requests do not add history or publish partial state. Projected
direct editing is stricter: a missing owner, incomplete inverse or cold-reproduction mismatch
rejects the complete multi-owner rewrite.

### L5 — workspace v7 and honest migration

Workspace v7 stores the complete lineage session, exact current and accepted host-input payloads,
annotation placement and optional flat caches. Versions v1-v6 still pass their existing strict
decoders, then become one honest `ImportedBaseline` root with no invented recipe/event history.
Existing sketch IDs/high-waters, computed feature/corner/evaluation high-waters, retained-versus-
accepted authority and valid annotations are preserved where present.

An honest retained-invalid/older-accepted imported root carries both exact host-input pairs. Cold
evaluation treats the embedded accepted sketch as a distinct branch selected only by the published
authority digest; it does not synthesize another step. A frozen fixture records the exact v1-v6
outer field languages and proves one-root migration, v7 re-encode and cache-free cold reload.

On v7 load, current accepted authority consumes the canonical accepted sketch bytes returned by
authenticated chronological evaluation of the exact current lineage/input pair. When the current
attempt is rejected, the visible older accepted authority uses the same evidence contract at its
exact historical lineage/input pair. Independently solving either flattened authored seed after
that evidence would create a second authority and can lose topology-sensitive continuation. A missing, corrupt,
stale, swapped or merely independently valid-but-different flat cache is discarded. Allocator-only
metadata may remain ahead when it satisfies monotonic high-water checks.

### L6 — stateful DOM-free RPC and TypeScript boundary

`geosolve.lineage.rpc.v0` supports create/import/load, inspect, exact patch/reconcile/owner rewrite,
policy change, engine-owned evaluation, Undo/Redo and canonical export. Envelopes carry
correlation/session identity and opaque string revisions/IDs. Rust retains state and repeats all
schema, dependency, host-input and owning-domain checks.

RPC `rewrite_owners` is an atomic exact-CAS batch of structural step rewrites. It is not the
coordinator's projected-drag reconciliation, which additionally derives every affected owner from
accepted geometry and requires cold reproduction before publication. Editor-compiled actions carry
private authenticated materialization intent and are cold-executable through the workbench
evaluator. A caller-authored generic action with a registered schema may still be retained,
inspected, reordered and undone as structural lineage, but it deliberately fails cold evaluation
with `workbench_materialization_unsupported` because the RPC cannot invent missing typed workbench
intent. The private TypeScript package is therefore a data binding for structural lineage and
loaded editor-compiled programs, not an OpenSCAD-like executable geometry builder in M83.

Untrusted serialized accepted authority is never accepted from a self-consistent session checksum
alone. Workspace-v7 restoration cold-reproduces accepted materialization with its exact persisted
host inputs. The standalone RPC session codec deliberately strips caller-certified current and
historical acceptance while preserving the complete declarative program/history; an explicit
ordinary cold `evaluate` call must establish fresh accepted authority before the RPC exposes it.
The WASM crate has no DOM, `web-sys`, browser storage, renderer, solve callback or start hook. The
TypeScript package supplies only branded data types, exhaustive catalog constants and request
builders; it is private and contains no geometry or acceptance logic.

The Rust RPC adapter maps wrong-kind ports, invalid lifecycle transitions and ID/revision
exhaustion to stable error classes. The TypeScript client has method-indexed parameter, result and
response types rather than a caller-selected result cast. Its runtime decoder validates exact
envelope and method-specific result shape, protocol/request/method correlation, envelope/result
session and document identity, closed error/evaluation-failure codes, and accepted/failed
evaluation cross-field invariants before updating retained session state. Malformed transport
replies throw a typed response violation and cannot become engine evidence. A UTF-8 byte preflight
also keeps outgoing requests within the same 16 MiB bound as Rust, because a request rejected
before Rust can decode its envelope cannot carry trustworthy correlation fields.

### L7 — read-only retained-program presentation

The ordinary workbench now places a secondary **Lineage** panel beside Sketch Tree on wide desktop
layouts and stacks it beneath the tree on compact desktop layouts. The wrapper disappears with the
tree at the existing narrow-workbench cutoff. The panel is deliberately inspect-only: its rows are
list items rather than buttons and expose no selection, delete, reorder or rewrite route.

Every render borrows `RetainedEditorCoordinator::lineage_document()` and derives chronological
markup anew. Rows show a friendly action/schema name, the retained owner label, action category,
input/output counts, exact schema/version, compact visible and full machine-readable stable step
identity, and explicit Live/Suppressed/Deleted state. The header shows current action count and
lineage revision. A distinct history strip consumes `history_cursor`, `history_len`, `can_undo` and
`can_redo`, making clear that Undo/Redo traverses versions of the current program rather than being
the program itself. Browser state stores no `LineageStep`, parses no lineage JSON and owns no
lineage mutation semantics.

Pure presentation tests cover chronological identity, imported/suppressed/tombstoned rows,
input/output counts, hostile attribute/text escaping, empty programs and in-place rewrite without
an appended event. A real coordinator regression creates one point, then Undo/Redo, and checks the
panel source plus action count/cursor/availability against current Rust authority at each position.
The M83-W11 source sentinel requires direct lineage/history accessors and rejects JSON parsing or a
browser-side `Vec<LineageStep>`. Static HTML/CSS tests cover unique accessible ownership, wide
adjacency, compact stacking and narrow hiding. Review moved the split/stack breakpoint to 96rem so
the canvas HUD keeps its ordinary width, advances the compact outer grid at 80rem so the Inspector
cannot clip in the intervening viewport band, and gives the bounded four-digit history cursor a
two-row layout. It also removes whole-row opacity from retained non-live steps, raises small-text
contrast/size and makes the durable developer key visible. The complete demo library passes
172/172 and focused warnings-denied demo Clippy passes on the amendment worktree.

## Finding ledger

### M83-F001 — oversized semantic key for host-bound parameter identity

The first bridge embedded a complete structured host-binding identity into a semantic port key,
which exceeded the bounded key contract. The repair uses compact deterministic ordinal semantic
keys while retaining the complete typed identity in action/reservation evidence. The focused
rational-conic host-binding regression proves import, cold evaluation and reload.

### M83-F002 — historical prefix lost removed host-input provenance

A later edit that removed a parameter binding or external snapshot made an earlier strict prefix
evaluate with the final input set. The independently reproduced failure reported the earlier
lineage step as missing its required input. Every baseline/action now retains a bounded canonical
parameter/snapshot pair authenticated by its external-input stamp; non-final prefixes consume that
historical pair. Focused regressions cover both parameter removal and external-snapshot removal.

### M83-F003 — retained datum relations lacked complete typed identity mapping

The first catalog bridge omitted draft-v5 `retained_planar_constraints` from Constraint/Source
materialized identity collection. Inventory-driven datum coverage exposed the missing mapping.
Both collection classification and persistent-source extraction now include retained planar
constraints; intrinsic datums themselves remain immutable document-bound parameters, not owned
outputs.

### M83-F004 — workspace cache admitted a different valid underconstrained solution

The v7 cache check authenticated structural intent and independent validity but did not require
the cached accepted geometry to equal cold accepted lineage materialization. A regression changed
an underconstrained accepted point from `[1, 2]` to another independently valid `[17, -9]`; the
alternate survived before the repair. Cache compatibility now compares exact semantic accepted
materialization while retaining separate monotonic allocator checks.

### M83-F005 — continuation declarations could steal an unchanged writable leaf

The first reverse-owner check authenticated changed leaves but allowed a later continuation step
to declare authority over an unchanged predecessor leaf. A self-consistent loaded session could
therefore move reverse write-back authority without changing cold geometry. Validation now
recompiles the complete writable-leaf manifest chronologically from authenticated predecessor
state. `loaded_continuation_manifest_cannot_claim_unchanged_leaf` and the baseline omitted-leaf
regression reject both ordinary and imported omissions before publication.

### M83-F006 — action ports and lifecycle declarations were caller-forgeable

Writable keys alone did not authenticate ordinary action inputs, outputs, reservations or identity
flows. Hostile sessions could retarget a same-kind input, add a ghost output/reservation, swap
persistent reservation IDs or replace `Created` with `Continued`/owned-logical flow while retaining
byte-identical geometry. The coordinator now recompiles a complete schema-specific action manifest
from exact prior ports plus the structural delta and compares every input, output, identity flow,
reservation and writable leaf. Focused hostile regressions cover each forgery and all current,
accepted, Undo and Redo documents.

### M83-F007 — imported-baseline manifests were not independently complete

An imported root previously trusted its declared writable set after its opaque payload decoded. A
forged root could omit an authored leaf and still present a structurally valid program. Baseline
validation now derives every entity, typed persistent output/reservation and writable field from
the authenticated checkpoint payload and requires an exact manifest match.

### M83-F008 — hostile RPC load could replace a valid in-memory session

RPC load validation initially concentrated on the current document. Complete session validation
now authenticates current and last-accepted programs plus every Undo/Redo checkpoint before the
candidate replaces engine state. `forged_load_rejects_atomically_and_preserves_previous_session`
proves a forged accepted/history manifest returns `invalid_editor_authority` while the previous
session remains byte-identical.

### M83-F009 — deleting imported geometry rewrote the immutable baseline owner

Persistent deletion was first treated as an ordinary field rewrite. For an imported object this
removed the object from the root payload while leaving the root's original output manifest, so the
root no longer authenticated itself. Deleting an imported persistent entity now appends a later
lifecycle action with explicit `Retired` flow. The baseline payload/output manifest remains
immutable; Undo removes the retirement action, Redo restores it, and its identity is never reused.

### M83-F010 — revision-local allocator exhaustion poisoned accepted lineage

The frozen `scene.current-native.withheld` row exposed that cold lineage validation consumed the
host's computed-edge evaluation allocator. A deliberately exhausted revision-local allocator then
rejected coordinator construction even though native sketch authority and feature intent were
unchanged and the established product contract was to withhold computed presentation. Cold lineage
evaluation now uses a deterministic scratch computed identity space; the exact host high-water
remains session auxiliary lifecycle state and is restored unchanged. A focused owning-layer
regression plus the previously frozen scene row prove the distinction.

### M83-F011 — truthful computed dispositions rejected accepted native lineage

Cold evaluation initially treated any persistent computed feature that was `Failed`, or any
bounded computed evaluation whose generated output was withheld, as failure of the complete
lineage materialization. That contradicted the existing owning-domain contract: independently
solved native sketch state and persistent feature intent remain authoritative while each computed
feature truthfully reports `Current`, `Suppressed`, `Failed` or `Withheld`. Cold evidence now
authenticates that exact disposition and its diagnostic in the materialization digest. Projected
direct manipulation deliberately retains its stricter all-current publication gate. Focused cold
evidence and mixed Current/Failed replay/reload regressions cover the distinction.

### M83-F012 — exact-position releases created fictional edits

Releasing a point or projected manipulation at its exact retained position could still stage a
lineage rewrite and add history/transcript evidence even though no authored value changed. Point
and multi-owner lineage staging now return no mutation for an exact semantic no-op. The retained
lineage revision, Undo/Redo cursor, transcript and evaluation authority remain byte-identical, and
a mismatched release remains retryable before the correct no-op release.

### M83-F013 — abandoned history identities could be rebound

Per-document validation prevented duplicate live identities but did not by itself prevent a
replacement program from reusing an abandoned step, output, reservation or persistent identity
with different meaning. `LineageSession` now derives one cross-history identity ledger from
current, accepted, Undo and Redo documents before admitting a patch, reconcile or decoded session.
Any changed step key/action schema/manifest, output declaration, reservation declaration or
persistent binding rejects atomically before entering history. Core and coordinator regressions
cover current, accepted, Undo, Redo and abandoned-branch reuse. The final authority audit found
one narrower complete-program hole after a divergent edit had already cleared Redo: the abandoned
binding was no longer present in any retained document even though its numeric lifecycle cursor
survived. Reconciliation now also rejects any step, output or reservation ID below the retained
session high-water unless that exact identity remains bound in retained history. Table-driven core
coverage exercises all three identity classes after divergence, and the stateful RPC regression
requires byte-exact atomic rejection.

### M83-F014 — a self-asserted accepted digest could pass workspace validation

A structurally self-consistent session checksum authenticated only what the payload claimed; a
caller could replace the accepted materialization digest and recompute that checksum. Workspace
validation now cold-reproduces the asserted accepted program under its exact persisted host inputs
and compares the resulting owning-domain digest. A self-asserted digest is rejected before it can
replace live authority.

### M83-F015 — session restore trusted derived acceptance and lost retained lifecycle cursors

Restoring a lineage session could combine a genuine declarative program with unauthenticated flat
accepted bytes, and could omit never-reuse high-waters retained only by accepted, Undo or Redo
positions. Restore now stages a cold owning-domain reconstruction, independently reproduces the
last-accepted program with its historical inputs when retained intent is failed, and merges every
history-retained sketch/feature/evaluation lifecycle cursor before atomic publication. A hostile
restore remains byte-neutral; a genuine history-bearing restore preserves abandoned reservations
and revision high-waters.

### M83-F016 — Undo/Redo trusted forged historical accepted caches

History traversal previously treated its revision-local flat checkpoint as accepted geometry
authority. For an underconstrained sketch, a different independently valid solution could
therefore be inserted into that cache and substituted on Undo/Redo. Traversal now restores the
declarative lineage position and cold-reproduces accepted authority through its owning domains;
flat checkpoint bytes are repaired only after successful staging. Focused regressions cover corrupt
design caches, a forged alternate accepted solution and atomic/retryable traversal failure.

### M83-F017 — action identity could change during a rewrite

The structural rewrite path originally authenticated ports and materialized values without making
the existing action kind, schema and schema version immutable. A caller could therefore reuse one
step identity for a different action meaning. Rewrite validation now requires those three action-
identity fields to match the historical declaration exactly; changes must use a newly allocated
step. Core and RPC regressions cover current, accepted, Undo and Redo authority and preserve the
stable `invalid_editor_authority` boundary classification.

### M83-F018 — maximum generic persistent IDs could panic output indexing

Generic persistent-ID fallback output indices were derived by adding a family offset to the raw
bounded ID. At the maximum admitted ID that addition overflowed in debug builds and could panic
instead of returning a typed result. The mapping now uses a non-overflowing bounded encoding and
the exact maximum identity is exercised through complete manifest compilation and cold replay.

### M83-F019 — RPC callers could forge nonpublishing evaluation evidence

An early RPC surface let the caller submit a claimed cancelled, exhausted or stale evaluation
attempt directly. Although such evidence did not publish geometry, it still belonged to the
engine's execution history and was not caller-certifiable. The method is no longer part of
`geosolve.lineage.rpc.v0`; an attempted `record_nonpublishing` request returns `unknown_method` and
leaves the canonical session byte-identical. Only actual bounded engine execution may record an
evaluation disposition.

### M83-F020 — historical accepted authority was authenticated only on traversal

Workspace restoration cold-authenticated the top-level last-accepted program, but accepted
authority retained only in Undo or Redo was checked later when the user traversed history. That was
fail-closed at use but weaker than workspace-v7's load-time authority contract. Restoration now
walks cloned history, deduplicates every distinct accepted stamp and cold-reproduces each with its
exact historical host inputs before publishing any workspace field. A forged accepted digest held
only in an Undo checkpoint rejects eagerly and preserves the target coordinator byte-for-byte.

### M83-F021 — serialized attempt metadata survived workspace reconstruction

A structurally valid workspace could supply a forged latest `Failed`, `Pending`, `Cancelled`,
`Exhausted` or `Stale` attempt even though the flat scene was rebuilt cold. Restore now derives the
current host inputs from the independently decoded workspace payloads, executes the ordinary
owning-domain evaluator and replaces that metadata with fresh accepted-or-failed evidence. All
five serialized dispositions are covered; a valid retained program reconstructs `Accepted`
authority with the real input/materialization stamp and no caller diagnostic.

### M83-F022 — wrong-document exact requests were reported as stale revisions

The RPC's direct expected-identity guard collapsed a foreign document and a stale revision into
`stale_revision`, unlike patch-backed methods which returned `wrong_document`. The direct guard now
classifies document identity first and revision/digest second. Its focused regression also proves
the failed request leaves the exported session byte-identical.

### M83-F023 — tagged branch values lost stable writable ownership

The M55 contact-branch matrix exposed a bridge-only failure when a contact changed from a bounded
domain to its supporting line. The first structural owner map split tagged values into
shape-dependent leaves such as `domain.lower`; changing the variant removed those leaves before
the owner rewrite could authenticate them and returned `MissingOwner`. `domain` and
`neighborhood` are now stable atomic writable leaves, and action-shape authentication uses the
same boundary. The complete 17-case M55 branch suite and the focused
`branch_variant_shape_changes_keep_stable_atomic_owner_leaves` regression pass without changing a
contact equation or branch rule.

### M83-F024 — projected feature replay retained a temporary drag target

The first complete release gate reproduced an existing M70B retained-movement regression:
`m70b_f005_projected_gesture_crosses_cardinal_mark_and_commits_exact_branch` rejected transcript
replay with `StaleComputedFeatureCandidate`. M83's direct-manipulation path correctly promotes the
accepted projected document into drag-free durable intent, but the recorded-edit replay path
reapplied the native edit without performing that same normalization. Replay now preserves an
ordinary programmatic edit when its prepared input already matches, and otherwise performs the
same projection promotion before authenticating the exact recorded after-input. All nine M70B
retained-movement cases pass, covering projected and programmatic edits, unrelated failed
features, branch continuity, atomic rejection, Undo/Redo, transcript replay and cold restore.

### M83-F025 — historical host-only input bytes could disappear behind an opaque stamp

An accepted parameter/snapshot pair can change without creating a lineage action. If that
authority later existed only in Undo or Redo, and the current position accepted another host-only
pair, workspace v7 retained the historical stamp but could lose the exact bytes needed to
authenticate it. Restoration therefore failed closed even for a legitimate workspace. A private,
bounded, canonical coordinator ledger now retains exactly the stamp-authenticated input pairs
referenced by current/Undo/Redo accepted authority; it lives beside the generic lineage session in
workspace v7 and does not enter the public lineage or RPC schema. Load resolves every accepted
stamp from this ledger, current/accepted workspace inputs or authenticated action provenance,
cold-validates every distinct authority, and rejects a missing, noncanonical, oversized,
duplicate or stamp-mismatched payload before publication. Focused regressions cover a host-only
pair held only in Redo, a missing ledger, current live-input preservation on Redo, and a failed
current attempt whose visible accepted state must be rebuilt from a ledger pair rather than the
last action's older inputs. The public paired-restore boundary additionally rejects noncanonical,
duplicate, stamp-mismatched and wrong-session companions while preserving the complete live
coordinator byte-for-byte; its Rustdoc and API-compatibility record state that lineage session,
ledger and exact current/accepted host payloads form one workspace bundle.

### M83-F026 — direct promotion authored an effective host value as a local fallback

The first committed candidate attempt, source `9bbbedd`, independently reproduced the existing
M77-F009 rational-control regression. A projected direct edit correctly used the host-supplied
effective weight `0.8`, but promotion copied the complete accepted document into retained intent
and changed the authored fallback from `0.5` to `0.8`. The candidate was withdrawn immediately;
its clean-gate log is `/tmp/geosolve-m83-clean-gate.9bbbedd.nix.log`, 71,999 bytes and 839 lines,
with SHA-256
`0108b29d32fcc761a77c9da388fb3075570db4a1609d166717f1264732cf622b`.

Direct-manipulation promotion now reifies accepted projected geometry while copying every
host-bound driving-dimension or dimensionless-property scalar fallback from retained design
intent. Activation bindings have no scalar fallback. Cold owner-rewrite qualification separately
certifies the staged accepted checkpoint under the exact host inputs, so the retained fallback is
not misrepresented as effective accepted state. The exact pre-existing M77-F009 regression, all
16 curve-control coordinator cases, and a focused driving-dimension fallback unit regression pass;
the replacement must complete a fresh clean gate before nomination.

### M83-F027 — direct promotion trusted a caller-supplied accepted witness

The first completion audit found that projected owner reconciliation materialized the rewritten
lineage, but then certified `next.accepted_json` supplied by the staged flat checkpoint and compared
those bytes back to the same checkpoint. For an underconstrained graph, a different finite and
independently valid solution with unchanged topology could therefore satisfy this circular proof.
Reconciliation now records and returns genuine cold lineage evaluation evidence under the exact
candidate host inputs. The staged accepted checkpoint must equal the owning sketch domain's
canonical accepted bytes from that evidence, while an independent retained-session validation
still certifies hard residuals. The focused regression substitutes `[8, 9]` for an exact projected
point witness, proves atomic rejection and then retries the genuine projection successfully.

### M83-F028 — native Fillet prefixes were solved without topology continuation

Strict and dependency-local cold evaluation originally solved every structural prefix as an
independent fresh sketch. That is insufficient for a native Fillet: its prepared plan is derived
from the accepted upstream geometry, while retained upstream coordinates can deliberately differ.
Cold evaluation now threads each independently accepted prefix into the next checkpoint. At a
native Fillet boundary, the owning sketch domain reauthenticates the complete prepared request,
source definitions and tangents, explicit branch/orientation choices, reserved IDs, labels,
source ownership, unaffected objects and materialized identity delta before producing a numerical
continuation seed. That seed is still accepted only through the ordinary session boundary and
independent residual validation.

Focused coverage includes consecutive Fillets, a Fillet retained inside a reusable
dependency-local prefix, host-only radius input, forged request/plan/ID/topology, and Undo followed
by divergent Fillet authoring. Abandoned reservations remain above session-global never-reuse
high-water. The change adds no residual, Jacobian, Fillet equation or branch heuristic.

### M83-F029 — legacy migration lost distinct embedded accepted authority

The previous migration evidence relabelled current-shaped workspaces and did not exercise an honest
retained-invalid/older-accepted payload from every historical outer schema. Once genuine fixtures
were introduced, the imported root's retained branch rejected as expected but its distinct older
accepted sketch was not independently reconstructed. `ImportedCoordinatorBaseline` now optionally
stores the exact accepted host inputs next to the retained inputs. Cold authority evaluation can
select the embedded accepted baseline branch, but only when its independently reproduced
materialization digest is the session's published accepted authority.

A related restore audit found that input-free structural decode had begun demanding this historical
pair before workspace v7 attached its private ledger. Input-free materialization now preserves the
embedded branch only for the matching distinct legacy authority; ordinary host-only accepted
programs materialize structurally first and authenticate their exact input pair during staged
restore. The checked-in v1-v6 fixture has six rows, 20,950 bytes and SHA-256
`af5e6398578dc02bc37df0a4ad8b9b0ff96295c98e6dff3992db652b4d29d4a1`. Every row passes its strict
historical decoder, retains failed-current/older-accepted semantics and one imported root,
re-encodes to v7, then reconstructs from lineage with the disposable flat cache removed. Separate
failed-current and Redo-only host-input regressions cover ordinary non-embedded authority.

### M83-F030 — historical restoration independently re-solved flattened accepted intent

Coordinator history restoration and workspace-v7 cache recovery authenticated the older accepted
lineage revision and its host inputs, but then discarded the owning-domain accepted evidence and
independently solved the flattened authored checkpoint. A native Fillet can have an exact accepted
topology-sensitive continuation whose coordinates differ from that authored seed, so the second
solve could publish a different valid solution or fail to reconstruct the visible fallback.

Chronological accepted materialization now returns its canonical accepted sketch evidence together
with the authenticated checkpoint and input pair. Coordinator restore, Undo/Redo and both current-
accepted and rejected-current workspace recovery consume those exact bytes directly, retain only
monotonic identity high-water beyond them, and independently validate the reconstructed hard
residuals. The two routes differ only in authority: one evaluates the exact current accepted
lineage/input pair, while the other evaluates the exact older accepted pair beneath a rejected
current attempt. Focused cache-free regressions freeze both sides, including topology-sensitive
native Fillets whose accepted centers make their canonical accepted bytes differ from flattened
authored intent and a historical case with a later conflicting anchor.

### M83-F031 — TypeScript RPC boundary trusted untyped results and unbounded requests

The first TypeScript response hardening authenticated the outer protocol/request/method/session
envelope, but accepted any finite JSON `result` through a caller-selected generic cast. A
method-incompatible result, a result whose identity belonged to another document/session, or a
malformed snapshot/evaluation could therefore become typed client evidence. In particular, a
syntactically correlated successful `load` could replace the retained client session before its
snapshot identity and session fields were authenticated. Stable-looking but unregistered error
codes were also admitted. Separately, the client could send more than Rust's 16 MiB request bound;
Rust necessarily rejects such bytes before envelope decoding, so that response cannot be
correlated to a trusted request identity.

The client now derives parameters and results from closed `RpcParamsByMethod` and
`RpcResultByMethod` maps and removes caller-selected success typing. Exact runtime decoders cover
snapshot, mutation, identity, evaluation and export results; they validate branded ID shapes,
result-to-envelope document/session identity, the immutable parameters that actually entered
transport, retained-current exact-CAS identity, accepted snapshot pairing, load/import lifecycle
postconditions, exact mutation/reconcile/Undo/Redo transitions, and accepted/failed attempt,
evaluation-plan and authority consistency. Returned step/history arrays are bounded at the matching
Rust limits. RPC and domain-evaluation error-code lists are closed and runtime-tested against their
Rust sources. Oversized serialized UTF-8 requests throw
`LineageRpcRequestError` with `request_too_large` before the transport is invoked. Compile-time
misuse tests cover method-specific and exactly empty parameter objects, including the rewrite-only
owner batch; runtime regressions cover every method result family, foreign/stale identities,
post-call parameter mutation, malformed and method-incompatible results, impossible dependency
plans, unknown codes, uncorrelated pre-envelope errors, and preservation of the prior session after
malformed ordinary and session-start replies. Rust remains the sole
lineage and geometry semantic validator; this repair authenticates only the TypeScript protocol
boundary and changes no solver or accepted-scene mathematics.

The final F031 protocol audit found five narrower cross-call holes. Identity-only Undo/Redo replies
did not expose the restored evaluation policy or historical accepted authority, so the client had
to discard both and later policy/evaluation forgeries could pass. Both methods now return the
ordinary authoritative snapshot shape; the client defensively copies and retains its policy and
last-accepted stamp, and mirrors their bounded checkpoint history across successful edits and
traversal. Standalone Load derives historical policy from each exact serialized checkpoint but
records accepted authority as `null`, matching Rust's mandatory stripping of current and historical
evaluator evidence. Undo/Redo snapshot policy, accepted authority and history lengths must match
the exact retained checkpoint. Failed evaluation must preserve the prior accepted stamp exactly
(including `null`), and subsequent Inspect, SetPolicy and Evaluate replies remain correlated. An
unchanged generic mutation containing `SetEvaluationPolicy` must end at the retained policy; a
forged no-op response cannot silently change the client's policy correlation state.

Blocked plan rows must name at least one earlier non-ready dependency. Mutation results may report
tombstones only when the request contains a direct Tombstone or DeleteSubtree operation; without
subtree closure, every reported tombstone must be one of the directly named steps. A changed batch
containing only deletion operations must report at least one tombstone, and a target inserted then
deleted in the same batch cannot be omitted. A mixed batch may still omit a requested direct
Tombstone when that step was already tombstoned and another mutation caused the revision change;
deciding that prior declarative state would turn this data binding into a second lineage model.
Runtime negative tests cover forged history policy/authority from both local edits and standalone
Load, null and non-null failure authority, an unchanged generic policy mutation, empty blockers,
insert/rewrite-only invented tombstones, changed deletion-only omission, inserted-then-deleted
omission, and the legitimate mixed no-op omission. These remain protocol/state correlations rather
than TypeScript lineage or geometry semantics.

### M83-F032 — a live solve was mistaken for canonical lineage acceptance

A raw center edit on the existing flexible line-circle Fillet exposed a final authority inversion.
The ordinary retained solve and strict chronological lineage replay both validly accepted the
underconstrained graph, but could choose different finite solutions. The coordinator required the
cold result to equal the transaction-local live accepted cache, so a valid authored mutation could
reject merely because two legitimate solves chose different coordinates. Other sketch-plus-feature
paths also staged computed output from the live solution before recording lineage, risking a flat
scene that did not equal its purported canonical program.

Strict cold lineage replay now owns accepted geometry. Ordinary sketch mutations use a two-pass
transaction: record once to obtain cold owning-domain evidence, replace the staged session's
accepted graph from that evidence, recompute computed-feature continuation and allocator state,
then record the final action from the untouched prior lineage and publish every owner atomically.
Offset, native Fillet and recorded sketch-plus-feature replay share the same staged helper;
construction, operation, host-parameter, external-snapshot, reattempt and Undo/Redo paths likewise
canonicalize before publication. The four feature-only callers retain their narrower intent path,
but that path now also stages strict-cold accepted sketch evidence. It preserves an exact held
computed preview when its accepted sketch bytes already equal cold authority and recomputes output
only when an underconstrained imported root produces a different canonical graph; neither case can
leave a stale live sketch cache or split publication owners.

`RetainedSketchDocumentSession::replace_current_accepted_materialization` is the narrow owning
seam. It requires exact prepared-input compare-and-swap, independently certifies the supplied graph
under the current request and host inputs, checks retained topology, preserves public design and
attempt identities plus continuation provenance and semantic catalog reservations, merges
persistent identity high-water, and advances only the process-local prepared-state epoch so
outstanding work becomes stale. Exact byte-identical current accepted evidence is a complete no-op;
changed canonical bytes and promotion of the exact current live-rejected attempt each allocate
precisely the next accepted revision. It cannot rewrite an older design or attempt. Initial accepted-baseline import still
authenticates the exact embedded accepted graph rather than silently substituting another valid
underconstrained solution.

The sketch-owner replacement suite covers exact identity-neutral replacement, rejected-attempt
promotion, host-input and continuation provenance, prepared-work invalidation, stale/older input,
foreign namespace, invalid graph and incompatible topology/activation. Coordinator regressions
cover the flexible-Fillet raw move and a distinct accepted underconstrained import baseline.
Native-Fillet lineage, workspace-v7 current/historical cache-free restoration, the complete
persistence module and the complete editor library remain green. No residual, Jacobian, branch
heuristic, solve tolerance or priority policy changed.

### M83-F033 — provisional live rejection bypassed the strict oracle

The F032 authority audit found the inverse publication gap. Ordinary lineage recording invoked
strict cold replay only after the provisional live solve reported acceptance; a live rejection was
recorded immediately as retained failure. That made a staging heuristic outrank ADR 0039's declared
correctness oracle and left a valid retained program rejected whenever cold chronological replay
could accept it from authenticated prefix evidence.

Ordinary evaluation now invokes strict cold replay unconditionally. Exact cold acceptance records
accepted lineage evidence and replaces or promotes the staged sketch session even when the live
attempt rejected. When both paths reject, the retained failed lineage position uses cold failure
attribution and preserves the complete older accepted authority. A live success followed by cold
rejection rolls the candidate transaction back atomically. Imported-baseline authentication keeps
its distinct embedded-accepted fallback, and projected owner reconciliation still requires exact
caller/cold equality rather than adopting this ordinary canonicalization policy.

The lower sketch seam regression constructs one coherent newer rejected attempt and proves that
promotion preserves design/attempt/parent/input provenance while allocating exactly one accepted
revision. Coordinator regressions separately prove live-rejected/cold-accepted promotion,
live-rejected/cold-rejected retention and live-accepted/cold-mismatch rollback. Public mutation
outcomes are derived after canonical publication so their accepted identity agrees with the final
current session rather than the provisional live result. The carried M70B radial-Normal scene
regression now asserts that same rule: a divergent retained fixed-point seed is promoted to its
strict-cold current accepted graph, and downstream contact authoring continues to measure the exact
canonical accepted geometry rather than depending on the superseded live-rejection disposition.

### M83-F034 — post-refresh checkpoint failure leaked auxiliary lineage high-water

The final transaction audit injected failure into the checkpoint immediately after computed-
feature refresh. That refresh had already retained its revision-local computed-evaluation cursor in
the live lineage sidecar. The existing rollback restored session, feature document, allocator and
computed caches, but omitted the earlier lineage clone, so a failed mutation could advance
auxiliary authority without history or transcript.

That checkpoint branch now restores the complete prior lineage alongside every other staged owner.
The focused regression proves exact session, accepted graph, lineage session, host-input ledger,
feature identity, allocator, computed snapshot/problem, checkpoint, history and transcript
retention, then retries the same edit successfully once.

### M83-F035 — legacy flat reload published a prefix before lineage import

The public compatibility `reload(RestoreCheckpoint)` route decoded and validated its sketch and
feature candidates first, but then began replacing the live allocator, session, features and
transient state before its final checkpoint and imported-lineage construction had succeeded. A
late failure could therefore expose a half-restored coordinator even though workspace-v7 lineage
restore already used a staged publication path.

Legacy reload now prepares the restored session, rebased feature document, merged allocator,
computed snapshot/problem, final checkpoint and honest imported baseline entirely in locals. Only
after every fallible operation succeeds does one publication replace the live coordinator and
reset its compatibility history. An injected late lineage-import failure regression proves exact
retention of lineage/ledger, session and accepted identities/bytes, feature and computed state,
allocator, lifecycle, selection, history and transcript, followed by a successful retry.

### M83-F036 — feature-only cold publication discarded exact held previews

The authority audit found four computed-feature-only callers that recorded fresh strict lineage
evidence but left the live accepted sketch cache in place. An underconstrained imported baseline
could therefore publish feature intent and computed output for one valid live solution beside a
lineage session whose canonical accepted bytes described another. The first repair rebuilt computed
output unconditionally from cold evidence; that fixed split authority but consumed a second
`ComputedEvaluationRevision` and violated the established interaction contract that pointer release
publishes the exact last Current preview.

Feature-only staging now carries the already authenticated provisional snapshot and allocator. It
compares the provisional accepted sketch bytes and encoding with genuine strict-cold evidence. An
exact match retains that snapshot, allocator and visible evaluation revision. A mismatch replaces
the scratch session's accepted materialization, evaluates computed features against the canonical
sketch, records the final checkpoint again from the untouched prior lineage and verifies that both
cold evaluations returned identical accepted bytes. Creation, absolute configuration, held radius/
contact preview and generic suppress/delete mutations all use this one atomic publication seam.
The distinct-root regression proves canonical sketch and Current computed output agree; the existing
radius/contact gesture regressions prove exact preview revision retention. No equation, tolerance,
branch, solver-priority or public API changes.

### M83-F037 — changed accepted bytes reused the prior accepted identity

The lower F032 replacement seam initially preserved the current accepted identity whenever the
candidate document compared equal through Rust `PartialEq`. That made a changed canonical graph
look like the same accepted revision; in particular, `+0.0` and `-0.0` compare numerically equal
while their canonical bytes and lineage digest differ. The no-op shortcut now compares exact
canonical draft-v5 bytes. Byte-identical evidence changes no identity, epoch, audit or provenance;
any byte change and every rejected-attempt promotion allocate exactly the next accepted revision.
The 11-case owner suite includes exact no-op, signed zero, changed underconstrained geometry,
promotion, host/continuation provenance, stale prepared work and atomic invalid/foreign/topology
rejection.

### M83-F038 — same-byte reload ignored a higher saved lifecycle

Legacy flat reload preserved a healthy live sketch whenever design and accepted bytes matched,
even when the saved checkpoint carried a higher attempt or accepted high-water. That shortcut could
discard observed revision lifecycle and later reuse an identity. It is now eligible only when the
live design/attempt/accepted high-waters already cover the saved values. The focused regression
uses identical bytes with a newer saved reattempt, requires restoration above both lifecycles, and
then proves the next reattempt advances again.

### M83-F039 — construction acknowledgement preceded lineage publication

M79's authenticated redundant-direction retry publishes an effective plan with its redundant
Horizontal relation omitted while the editor retains the original pending plan. The coordinator
marked that original plan published before fallible lineage recording, cold accepted replacement,
computed refresh and final checkpoint work. A late failure could therefore make the pending token
falsely acknowledgeable despite publishing no history or scene.

Construction publication is now split into fallible preparation and an infallible live swap.
Preparation runs every checkpoint, lineage record, strict-cold replacement, computed reevaluation
and auxiliary-high-water step in clones. Only after it succeeds does the coordinator mark the exact
authenticated pending plan and immediately swap the prepared state; the controlled path also
passes `BeforeCommit` before marking. The exact M79 gesture regression injects record failure,
proves byte/identity-neutral complete authority and nonpositive acknowledgement, retries once, and
proves one effective-plan publication and successful acknowledgement.

### M83-F040 — construction returned provisional accepted identity

The same audit found that construction APIs returned the trial solve's `MutationOutcome` after
strict-cold replacement could select a different valid underconstrained graph and allocate a fresh
accepted revision. Preparation also did not explicitly reject the generic `None` evaluation
disposition, even though that branch is defensive rather than reachable from a successfully
accepted construction under the current evaluator contract. Both ordinary and controlled paths
now require `Some` accepted evidence, rebuild computed companions from its graph, and derive
design, attempt and accepted identities from the final prepared session. The focused imported-
polyline regression forces different provisional/cold bytes and proves both APIs return the final
canonical identity and scene.

## Qualification record

Earlier development-stage focused and collateral evidence, before the clean nomination recorded
below:

```bash
cargo test --locked -p geosolve-sketch-lineage --all-features --no-fail-fast
```

Result: 29 passed, zero failed: 21 canonical/session tests and eight materialization-map tests.

```bash
cargo test --locked -p geosolve-constraint-editor \
  coordinator::lineage::tests --lib
```

Result: 22 passed, zero failed, including complete-manifest, imported-root, stable tagged-branch
owners and historical-authority forgery regressions. The ledger codec and public paired-restore
atomicity cases pass separately within the editor library suite.

```bash
cargo test --locked -p geosolve-constraint-editor \
  --test m83_lineage_behavior \
  --test m83_lineage_catalog \
  --test m83_lineage_historical_inputs \
  --test m83_lineage_operations \
  --test m83_w2_w3_catalog_lifecycle --no-fail-fast
```

Result: 21 passed, zero failed. The frozen action catalog has 194 catalog entries plus its header
(195 physical lines), with SHA-256
`8a45fde5691f82adb1afb9e30b1d71fff24884667453ab46fd275fe0caad922a`.

The complete editor and demo library suites subsequently passed 460/460 and 167/167. Focused
workspace-v7 authority passed 10/10, the complete persistence module passed 31/31, lineage restore
passed 3/3 and Undo/Redo collateral passed 23/23. The M70B retained-movement suite passed 9/9,
including the projected-release transcript-replay regression that opened M83-F024.

The first attempted clean gate from committed source `9bbbedd` ran on 2026-08-22 from 11:40:02
through 11:42:52 AEST and exited nonzero at M77-F009, after 15/16 curve-control cases passed. That
gate is withdrawn defect-reproduction evidence only and does not nominate its source or `dist`.
After the M83-F026 repair, the complete editor library plus M70B retained movement, M77 curve
properties and focused M83 behavior/history collateral passed 459/459, 9/9, 6/6 and 7/7
respectively on the development worktree.

Completion hardening then passed the complete `geosolve-sketch --all-features` and
`geosolve-constraint-editor --all-features` test targets. The new scalar-batch owner suite passed
3/3; the native-Fillet lineage suite passed 5/5; the frozen v1-v6 migration matrix passed 1/1; and
the complete demo library passed 167/167. The 271-row golden survey, `--check` and
`--require-clean` each passed byte-unchanged.

After M83-F030, the exact owning and crossed-adapter commands were repeated on the development
worktree:

```bash
cargo test --locked -p geosolve-constraint-editor \
  --test m83_native_fillet_lineage --no-fail-fast
cargo test --locked -p geosolve-demo-web workspace_v7 --lib
cargo test --locked -p geosolve-demo-web workbench::persistence::tests --lib
```

Result: native-Fillet lineage passed 5/5, workspace-v7 authority passed 10/10 and the complete
persistence module passed 31/31. The paired cache-free native-Fillet regressions distinctly prove
direct consumption of exact cold-authenticated current and historical accepted bytes.

```bash
cargo test --locked -p geosolve-sketch-lineage-wasm --all-features --no-fail-fast
nix-shell --run 'env CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER=wasm-bindgen-test-runner \
  cargo test --locked -p geosolve-sketch-lineage-wasm --test m83_rpc_parity \
  --target wasm32-unknown-unknown'
npm --prefix packages/geosolve-lineage test
```

Result: fourteen RPC/schema unit tests, one native transcript-parity test, one actual-WASM
transcript-parity test and both TypeScript compile/runtime checks passed. The frozen RPC transcript
has 28 responses, 117,156 bytes and FNV-1a `7063e9c6b5b5248c`. The larger frozen byte count is the
intentional result of returning complete restored snapshots from Undo and Redo; native and actual
WASM replay agree on the new bytes.

The package-only command was repeated after the M83-F031 method-indexed decoder and request-bound
repair on the development worktree: `tsc --noEmit` and the Node runtime suite both exited zero.
This is focused development evidence, not a clean-candidate nomination.

After M83-F032/F033, the accepted-replacement owner suite passed 9/9 and the focused strict-cold
lineage/coordinator promotion and rejection regressions pass. After F034/F035, their two exact
failure-retention regressions pass. F036's distinct-root regression and exact held-preview
collateral pass, and the complete editor library passed 470/470; native-Fillet lineage passed
5/5; workspace-v7 authority passed 10/10; and the complete persistence module passed 31/31.
M71 midpoint-axis collateral passed 2/2 and the projected-drag collateral passed 1/1. Targeted
warnings-denied Clippy for `geosolve-sketch` and `geosolve-constraint-editor`, formatting and diff
hygiene pass on the implementation worktree.

The final F037-F040 authority pass expands the accepted-replacement owner suite to 11/11; exact
byte no-op, signed-zero byte change, changed canonical identity, rejected-attempt promotion and all
atomic rejection cases pass. The F039 M79 effective-plan failure/retry and F040 ordinary/controlled
underconstrained construction identity regressions pass, as do the original M79 integration and
controlled pre-commit cancellation collateral. The complete editor library passes 473/473 and
warnings-denied editor all-feature Clippy, formatting and diff hygiene pass. These remain
development-worktree evidence until committed-source qualification below completes.

Workspace-v7 cold/cache authority tests pass 10/10 and historical-host-input tests pass 2/2;
cache-free workbench routing/reload tests pass 2/2; the frozen strict v1-v6 migration matrix
passes. The imported-deletion M78 collateral regression and the revision-local
computed-allocator regression pass.

Workspace-wide warnings-denied all-target/all-feature Clippy, locked all-feature workspace tests,
workspace Rustdoc, the TypeScript compile/runtime suite, actual-WASM RPC parity,
`cargo fmt --all -- --check` and `git diff --check` pass on the implementation worktree.

The generic golden `--require-clean` pass matches all 271 reviewed rows unchanged. Three rows that
timed out only while a duplicate concurrent oracle consumed the host each passed exactly in
0.5–2.2 seconds after that contention was removed; the subsequent complete clean-oracle run passed.
The fixture remains 271 catalog entries plus its header, with SHA-256
`cb09894516c7482aab6d1a49b34c1c3c95494e7cd6eac06547ac87e0b08de797` at this checkpoint.

### Superseded pre-panel candidate qualification and immutable nomination

The following remains historical evidence and is withdrawn from current M83-W12 because it does
not include L7. Exact product source `d378f7b31f56b43af787202dc1ebb92b7d199f84`, tree
`25a47e821cc80ff62d1891cfc7095d10fb2ec87f`, ran the following from a clean worktree and exited
zero on 2026-08-23 at 02:06:03 AEST:

```bash
env NO_COLOR=true nix-shell shell.nix --run './scripts/release-gate.sh'
```

The log is `/tmp/geosolve-m83-release-gate.log`, 324,204 bytes, SHA-256
`b38c7c5a46408e5237109b3ac64d6c75f340479920baefaa933189c90ae7692e`. The gate passed Cargo
metadata/offline resolution, formatting and diff hygiene, warnings-denied all-target/all-feature
workspace Clippy, locked all-feature workspace tests, the exact clean 271-row golden oracle,
M70/M71/M74/M75/M76/M77/M79/M83 native/WASM parity, the demo WASM check, TypeScript install/
compile/runtime checks, warnings-denied Rustdoc, benchmark compilation, M14 and M32 budgets, the
ignored 256-moving-body sparse crossover in 127.73 seconds, licence/package checks and Trunk
0.21.14 release assembly. The only diagnostics were the repository's pre-existing non-failing
Cargo notices for packages declaring both `license` and `license-file`.

Two independent focused architecture/API/publication audits found no release blocker. They
confirmed downward crate dependencies, stable typed identity/ownership, strict-cold publication,
canonical-byte accepted identity, staged failure atomicity, construction-token acknowledgement and
final-identity reporting. Two future-hardening notes are deliberately non-blocking: combine the
existing strict-cold-rejection and pending-token regressions in one case, and carry the
authenticated token inside a prepared publication if this currently synchronous seam ever becomes
asynchronous.

Without rebuilding, the gate-produced `crates/geosolve-demo-web/dist` was copied to
`/tmp/geosolve-m83-uat.RwXTfs`, byte-compared with the source before and after freezing, and made
read-only: directory `0555`, seven regular non-symlink files `0444`. The C-locale ordered manifest
has aggregate SHA-256
`ee2695ca55e803cbdeb8f6cd5a1ff632e59fe583428807f36e29ad5f2fbebd51`:

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| `API_COMPATIBILITY.md` | 35,307 | `b81b2c03fce26f78f3dca7702543fd6d0c63d98e09b4605071d512d4d2395389` |
| `LICENSE` | 35,148 | `ca372a7d92560b1fa9f6d832b440e8bcd62d9adfa8870c98287deab66d98310e` |
| `THIRD_PARTY_LICENSES.md` | 3,120 | `61a118f17bbdb7a1ad563fceabeb26b0cf9d03eac77048bb0a20a639faa11803` |
| `geosolve-demo-web-2819e61d7e58f9ff.js` | 33,750 | `9ce23fee79972aaa64f5c2353b35f2fbec7b15faa19493caac2af923bbe15ffc` |
| `geosolve-demo-web-2819e61d7e58f9ff_bg.wasm` | 9,914,526 | `0d71b4a21592238d61d9b041e56864d40937bc963b91721e16fa38443bd2b998` |
| `index.html` | 31,033 | `8e9e058305c72c237ca5b912b92aa51d02a0ec8d44ad74bd29aebc6f27da89ba` |
| `styles-a41d7984178d1121.css` | 38,291 | `957c7809eab90b61a2a72266af8f8660390b8c04fcce7b6c9e06398582097bbf` |

Temporary service `geosolve-m83-temp-uat.service`, PID `1270424`, first served only that snapshot
at `100.94.63.83:18080`. Proxy-disabled, cache-bypassed requests with identity encoding for `/`
and every file returned HTTP 200 from the exact Tailscale address with zero redirects, no
`Location` or `Content-Encoding`, exact media type, `Content-Length`, downloaded length, SHA-256
and body bytes; `/` equals `index.html`. Evidence is
`/tmp/geosolve-m83-temp-verify.GLWzwt/results.tsv`, SHA-256
`3217083aa1a3f9e56d02dcdb30f8c518b35d27767676abc531aa2356f1632ba1`.

Only after that pass, `geosolve-m83-uat.service`, PID `1273798`, began serving the identical frozen
directory at `http://100.94.63.83:8080/`. The independent eight-request final ledger at
`/tmp/geosolve-m83-final-verify.yMsi3f/results.tsv` is byte-identical and has the same SHA-256; its
fetched manifest matches the frozen aggregate above. The temporary listener was then retired.
The retained service remains live only for continuity while the L7 replacement is qualified. These
evidence-only documentation changes are descendants of the nominated source/tree and do not
rebuild or replace its product bytes.

## Known limitations and next gate

M83 intentionally does not add arbitrary-curve/computed Offset, topology-changing Offset,
computed-on-computed features, B-rep/PDM naming, formulas/configurations/units, collaboration,
TypeScript source rewriting, a browser script editor or npm publication.

The immediate gate is clean committed-source qualification and immutable replacement nomination
for L7. The remaining product gate is then the focused scorecard in `docs/M83_UAT.md` and explicit
supervising-human acceptance. M83 must not close or deploy to GitHub Pages before that decision. If
UAT opens a finding, the immutable candidate is withdrawn and the exact owning-layer defect
workflow applies; otherwise those replacement nominated semantics proceed through the standard
Pages build and exact hosted-byte verification.
