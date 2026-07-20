# ADR 0016: Spatial continuation publication and gauge

Status: accepted

## Context

ADR 0011 defines adaptive natural and pseudo-arclength continuation for the
planar compatibility model. M20 subsequently established spatial hinge and
translation coordinates, explicit mode monitors, private six-coordinate gauges
and separately validated ungauged physical publication. M23 must combine those
contracts without allowing a private gauge, active continuation parameter or
pseudo-arclength control row to alter public physical rank, audit or mode state.

Spatial position drivers are not all affine in their target. Translation has
target derivative `-1`, while the hinge clock row

```text
cos(lambda) * (y_1 dot x_2) - sin(lambda) * (x_1 dot x_2)
```

has a configuration-dependent derivative with respect to `lambda`. Duplicating
or approximating that equation in continuation orchestration would detach the
tangent from the executable source.

## Decision

### Selected driver and tangent

`SpatialAssemblySession::continue_driver` is revision checked and accepts one
hinge or translation position-driver `SpatialSourceId`, an explicit natural or
pseudo-arclength mode and the shared normalized adaptive-step policy. Exactly one
hard driver may own the selected coordinate for this single-parameter API.

The selected driver is internally compiled with one active scalar parameter:

- hinge parameters use unit step scale and retain the stored winding;
- translation parameters use the assembly model scale;
- both active forms share their fixed driver's equation and body Jacobian code;
- the active scalar Jacobian is checked independently by central differences.

The accepted continuation tangent is formed from an ordinary fixed-driver hard
linearization plus the normalized scalar column read from a separately compiled
active-parameter linearization. Both private problems use the accepted floating
component gauge references. Every body pose and the active scalar must remain
bit-for-bit attached to the accepted snapshot while the tangent is constructed.
Core then applies the accepted rank threshold and requires exactly one augmented
right-null direction.

Mode-only relationships can connect several physical hard components while
contributing no equality row. Continuation rejects internal mobility outside the
selected physical hard component independently of which body is the numerical
gauge reference. A configuration-dependent null direction inside the selected
hard component, including a fixed-driver fold, is not rejected preemptively;
the authoritative augmented-nullity test decides whether the released-driver
path is one-dimensional.

### Predictor, corrector and publication

Natural continuation stages the predicted target together with predicted poses
before checking source branches and explicit mode monitors. It verifies the
post-corrector tangent before commit and stops with
`PseudoArclengthRequired` before a parameter reversal. It never switches modes
implicitly.

Pseudo-arclength continuation temporarily adds the active parameter and one core
manifold control row over normalized `Pose3` local differences. The augmented
problem, its private gauges, parameter, source, report, rank and audit are
ephemeral. A successful augmented candidate seeds a separate ordinary
fixed-driver `SpatialAssemblySession`; only that independently validated physical
result can become a public sample.

Each accepted sample is one revision-checked clone-and-swap commit. Rejected
retries consume no revision. A later failure retains the accepted prefix, while
a call with no accepted sample leaves every accepted view unchanged. Every call,
including a zero-distance natural request, first performs fresh ordinary
validation. Zero-distance completion consumes no revision. A success-like path
cannot be reported solely from an absolute epsilon shortcut or a signed-zero bit
change; a positive path must contain at least one representable physical sample.

Public samples contain the ordinary physical solve plus only the augmented
corrector's backend and typed sparse-fallback summary. They never contain
ephemeral source mappings, residuals, audit rows, rank, nullity or spectrum.

### Branch boundaries and explicit mode changes

Every accepted spatial solve publishes typed finite boundary evaluations for
fixed/frame-offset diagonal false roots, source axis parity, prismatic clock,
hinge-driver positive roots, hinge principal cuts and explicit axis-parity,
plane-side and signed-volume monitors. Clearance is dimensionless; plane-side
distance is divided by model scale. A retained latch enters the boundary band at
clearance `2e-3` and leaves it at `4e-3`. Initial states inside the leave threshold
are conservatively near.

Predictor measurements are finite relaxed evaluations, so a source or monitor
can produce a typed event before the stricter `1e-3` physical branch validator
rejects it. Predictor latches are provisional. Only a separately accepted
ordinary corrected endpoint inherits latch state and publishes its events.
Continuation stops after accepting an endpoint that enters the band, or without
publication when a predictor reaches the strict margin. An attempted pseudo path
through `-pi`/`pi` is a typed `CrossingAttempted` event; it never wraps a target or
changes winding implicitly.

`SpatialAssemblySession::change_modes` lowers explicit source/monitor parity,
plane side, signed-volume orientation and hinge principal-cut changes plus their
companion seeds into one existing revision-checked clone/solve/validate/swap
transaction. A hinge cut atomically updates its coordinate, all associated
drivers and winding monitors. Every changed boundary must be beyond the leave
threshold in the accepted replacement; any error retains all prior views.

These checks observe predictor and corrected endpoints plus the analytically
known selected-driver principal cut. They do not claim deterministic
interval-global event tracing or global root enumeration.

## Consequences

- Monotone hinge and translation paths work for grounded and floating spatial
  assemblies at model scales `1e-6`, `1` and `1e6`.
- Natural continuation stops before the embedded spatial slider-crank fold;
  explicit pseudo-arclength continuation crosses it in both orientations.
- Common-left `SE(3)` transforms and forced dense/sparse correctors preserve the
  independently validated physical endpoint, rank, mobility and retained modes.
- Private gauges and pseudo equations remain absent from every published source,
  audit and physical rank surface.
- Typed endpoint events distinguish provisional predictors from accepted
  corrected samples, while hysteresis state changes only with accepted sessions.
- Periodic hinge cuts require an explicit atomic winding/mode transaction.
- Multi-driver continuation is not implied. M23 velocity work may accept several
  prescribed driver rates without changing this single-path parameter contract.
