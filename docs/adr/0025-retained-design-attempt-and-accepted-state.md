# ADR 0025: Retained design, attempt and accepted-state identity

Status: accepted

## Context

The version-4 sketch workflow commits a document only when the corresponding solve
is independently accepted. A production CAD host must instead be able to retain a
structurally valid edit that is currently conflicting, inactive, missing an input or
otherwise unsolved, while continuing to display the last trustworthy geometry.
Treating that retained intent as accepted would create false success. Treating the
old accepted geometry as if it solved the new intent would make audit, persistence
and stale-work checks ambiguous.

M34 therefore needs distinct design, attempt and accepted-state identities before
M41-M47 add activation, host inputs and prepared work. Those identities must also
allow an underconstrained but independently hard-valid state to remain a legitimate
parent state. Zero mobility is not a condition of acceptance.

## Decision

### Three separate identities

One sketch lifecycle has three non-interchangeable identity domains:

- A **design identity** is the document identity plus a monotonically advancing
  design revision. It names one structurally and referentially valid, finite,
  resource-valid statement of design intent. A retained design revision may have no
  accepted solution. Malformed, dangling, non-finite or resource-invalid
  transactions are rejected before allocating a design revision.
- An **attempt identity** is a never-reused document-local attempt revision. It names
  one evaluation of one exact input stamp and records its optional parent accepted
  identity. An attempt may carry a finite candidate and diagnostics, but neither the
  candidate nor its report is accepted geometry. Failure before a finite candidate,
  cancellation and deterministic work exhaustion are still identifiable attempts.
- An **accepted-state identity** is the document identity, a monotonically advancing
  accepted revision and the complete input stamp validated at that revision. It is
  created only after fresh independent hard-row, geometry, domain and branch
  validation of the same finite candidate. It also records the originating attempt
  and design identity.

Comparing bare revision integers across these domains has no meaning. A design
revision does not advance the accepted revision, and an attempt never acquires an
accepted revision by reporting a candidate or success-like numerical termination.
The last accepted state may therefore identify an older design revision than the
current retained design.

### Parent accepted state

Every attempt names exactly one parent accepted-state identity, or explicitly names
no parent when the document has never had an accepted state. The parent is the
authoritative source for warm-start values, previous-state preferences and retained
geometry; it is not a claim that the parent solves the attempted design.

An independently hard-valid accepted state may be underconstrained, bounded or
singular and still be a parent. Acceptance does not require zero equality mobility,
zero bounded mobility or a unique solution. Multiple design revisions and attempts
may descend from the same accepted parent. Rejection, cancellation or an unsolved
design leaves that parent and all of its accepted bytes unchanged.

Persistent identity, not coordinate proximity, maps parent values into a newer
design. New design elements have no accepted counterpart until a later accepted
solve, and removed or inactive elements do not authorize inferred replacements.

### Complete input stamp

Every attempt and accepted state carries one complete input stamp. The conceptual
stamp contains:

- document identity and exact design revision;
- parameter-batch revision and canonical payload digest;
- external-snapshot-set revision and canonical payload digest;
- effective-activation revision and canonical payload digest; and
- solver-policy revision and canonical payload digest.

The solver-policy identity covers every solve option that can affect candidate
selection, hard validation, branch validation or publication. Operation-only
diagnostic, profile and cancellation controls receive their own result evidence and
cannot be used to weaken the acceptance policy. An absent parameter batch, external
snapshot set or activation override is represented by a canonical empty input, not
by an omitted or process-default field.

The stamp is immutable during an attempt. Every result repeats it, and publication
compares every member rather than checking only the design revision. A revision
provides ordering and stale-work evidence; a digest provides exact payload identity.
Neither substitutes for validating the corresponding input. M47 compare-and-swap
publication must reject a candidate if any stamped member differs from current
session input.

### Views and publication

The design view is authoritative only for retained intent. An attempted view is
optional finite evidence tied to its attempt identity and complete input stamp. It
must be visibly and programmatically non-authoritative and cannot supply an accepted
audit, accepted measurement or production profile.

Only the accepted view may be described as solved geometry. Its audit, measurements,
rank, mobility, branch state and later topology results must all identify the same
accepted state and complete input stamp. A rejected attempt may publish diagnostics
about its own design and candidate but cannot replace or relabel accepted evidence.

### Persistence and transition

Sketch wire languages v1 through v4 remain frozen. Their fields and variants are not
expanded to encode retained unsolved intent, attempt identity or host-input stamps.
They continue to mean exactly the accepted-document languages already released.

M34 implements the three-view lifecycle and identity rules. From M34 through M52,
any draft-v5 representation is private, explicitly unsupported and free to change
without migration or compatibility guarantees. A draft-v5 payload is not a released
wire language, supported import or canonical public output, and relabeling draft-v5
syntax as v1-v4 must reject.

M53 alone freezes one final sketch v5 language, its direct deterministic migrations
from frozen v1-v4 and the separate parameter, external-snapshot and desktop-workspace
envelopes. Until that gate passes, v1-v4 remain the only supported sketch wire
languages.

This ADR is an M33 contract decision only. M33 introduces no Rust API, public or
private schema API, session behavior or draft-v5 reader/writer.

## Consequences

- Hosts can retain and repair valid unsolved intent without misrepresenting old or
  attempted geometry as accepted.
- Underconstrained hard-valid geometry remains a normal accepted parent rather than
  being rejected for having mobility.
- Complete stamps make stale diagnostics, prepared jobs and topology output
  distinguishable even when their design revision matches.
- Accepted-state history may advance less often than design or attempt history, so
  consumers must display the applicable identities rather than one overloaded
  revision.
- The production transition can evolve a private draft-v5 model without weakening
  the frozen v1-v4 compatibility contract.
