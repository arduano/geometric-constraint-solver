# ADR 0022: Associative line-line fillet foundation

Status: accepted

## Context

M27 needs one independently validated line-line fillet slice before M28 broadens
the curve-family matrix and introduces visible parent trimming. A fillet must
remain associated with solved parent contacts, but creating a second persistent
topology store or treating fixed arc angles as authoritative would duplicate the
existing sketch graph and allow stale endpoints.

The existing document already owns ordinary circular arcs, bounded contacts,
driving/reference radius dimensions, source identity, history and canonical JSON.
The runtime already owns geometry-generic curve jets and accepted latent contact
parameters. The foundation should compose those seams rather than add browser
equations or persistent region semantics.

## Decision

### Persistent representation and ownership

A line-line fillet is one `LineLineFillet` document constraint. Its persistent
constraint ID is the association identity. It references:

- one ordinary `CircularArc` output;
- two ordinary bounded `ContactSlot`s, one on each directed parent line span;
- an explicit left/right normal side for each parent; and
- an explicit first/second endpoint order.

The output arc retains its ordinary positive radius scalar, explicit clockwise or
counterclockwise sweep, and ordinary driving/reference radius dimension. Parent
lines remain visibly and topologically untrimmed in M27.

The association owns its two contacts and their latent parameter scalars under the
existing constraint-contact deletion rules. The center, arc, arc-owned scalars and
radius dimension remain ordinary entities. Deleting the association therefore
explodes the construction: contacts are removed and the last accepted arc remains
ordinary frozen geometry. Deleting the referenced arc while the association is
active, including dependent/cascading selection deletion, is rejected as
`ObjectInUse`. M28 may add explicit trim-view ownership but does not reinterpret
this M27 behavior.

Suppression disables the association equations and freezes the arc so its endpoint
scalars may be edited, but it does not release output ownership. The association
must be deleted explicitly before the arc, center or arc-owned scalars can be
cascade-deleted.

An active association makes the arc start/end angle scalars derived and directly
non-editable. Accepted solves project newly derived angles back through the same
persistent scalar IDs. Sweep edits and an atomic fillet-branch edit remain explicit
operations.

The output remains an ordinary arc for rendering, hit testing, equation-free
curvature/radius measurement,
selection, radius dimensions and read-only curve queries. M27 rejects attaching
another executable point/contact/tangency/continuity source to that arc because
generic incidence does not yet differentiate its contact-derived endpoint angles.
M28 removes this restriction only when that incidence is represented truthfully in
the common curve-jet path.

### Equations and branch state

For parent line jet `C_i(t_i)`, increasing unit tangent `T_i`, left normal
`N_i = left(T_i)`, side sign `s_i`, fillet center `O` and radius `r`, the association
contributes four hard Cartesian rows:

```text
O - C_1(t_1) - s_1 r N_1 = 0
O - C_2(t_2) - s_2 r N_2 = 0
```

Each row is normalized by model scale and has a structured audit descriptor. The
implementation uses the common local-AD curve-jet incidence even though M27 admits
only bounded line/polyline spans. M28 can broaden that internal residual without
adding pair-specific equations.

Both contacts use strict interior neighborhoods on `[0, 1]`; an escaped endpoint
is rejected rather than silently switching to supporting-line semantics. Directed
parent branches are enforced while the association is active because left/right
side meaning depends on endpoint order. Parallel, collinear and numerically
unresolved near-parallel parents are invalid fillet geometry.

A driving radius dimension adds the ordinary radius equation. A reference radius
adds no equation and reports the solved radius, leaving the expected regular
one-dimensional fillet family for fixed parents.

### Derived arc and independent validation

After each candidate solve and latent normalization, both accepted parent contacts
are evaluated independently. Endpoint order selects which contact is arc start and
end. Their center-relative `atan2` angles are unwrapped near retained state and the
arc's explicit sweep determines the finite nonzero signed traversal. These derived
angles are staged before any success-like publication.

Independent validation reconstructs and checks:

- finite regular parent jets and strict bounded neighborhoods;
- nonparallel directed parent tangents;
- all four normalized center/normal rows;
- radius agreement and tangent/radial orthogonality at both contacts;
- both selected normal-side signs;
- endpoint-order correspondence and explicit sweep sign;
- canonical signed sweep recomputed from both stored endpoint angles and both
  solved contact vectors; and
- evaluated arc endpoints against the solved contacts.

Any non-finite, zero-radius, escaped, side-flipped, ambiguous, residual-invalid or
sweep-invalid candidate rejects transactionally and retains the prior accepted
document/runtime/history state.

### Persistence and consumers

Sketch JSON advances to version 3. Versions 1 and 2 receive frozen constraint wire
DTOs and migrate deterministically; neither older label may accept fillet syntax.
Version 3 serializes the association, contacts, arc, branch state and ordinary
dimension explicitly.

The WASM demo may render the ordinary accepted arc through public curve APIs. It
must not derive endpoints, duplicate equations or offer direct trim-angle handles
for an actively associated arc.

## Consequences

- M27 establishes truthful association and branch semantics without claiming
  parent trimming or general CAD topology.
- The output arc and radius dimension remain usable through existing public APIs.
- Exploding a fillet is explicit and leaves ordinary accepted geometry.
- Generic curve-family incidence and visible trim views remain M28 work.
