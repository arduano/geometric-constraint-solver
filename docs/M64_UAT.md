# M64 editable samples UAT

Status: approved by the supervising human on 2026-07-30.

Candidate URL: `http://100.94.63.83:8080/`

## Review setup

1. Open the candidate in a fresh tab.
2. Use **Samples** in the top command bar.
3. Confirm the selector has only **Mechanisms**, **Constraints & dimensions**, and
   **Curves & constructions**, with each leaf opening in one right-hand flyout.
4. Reopening a sample restores its pristine document; there is intentionally no guide, reset or
   exit control.

## Scorecard

Record Pass, Concern or Blocker for each item.

### M64-U1 — ordinary editable workspace

1. Open **Drafting compass · 1 DOF**.
2. Select a fixed constraint in the tree, delete it, then Undo.
3. Draw a new line and apply any compatible constraint.
4. Refresh the page.

Expected: Delete and Undo work, ordinary authoring remains enabled, and the edited sample
autosaves/restores like any other workspace.

Rating: Pass.

### M64-U2 — mechanism mobility

Open and drag representative free joints in:

- **Four-bar coupler · 1 DOF**
- **Pantograph linkage · 2 DOF**
- **Three-link drawing arm · 3 DOF**
- **Scissor jack · 1 DOF**
- **Five-stage scissor tower · 1 DOF**

Expected: motion is emergent from constraints, finite and continuous; the examples are neither
fixed-only nor read-only.

Rating: Pass.

### M64-U3 — independent twin rollers

1. Open **Twin-roller cam · 2 DOF**.
2. Drag the left roller along the cam and observe the right roller.
3. Drag the right roller and observe the left roller.

Expected: the passive roller remains stable while the dragged roller follows its own tangency.
The tab remains responsive and either roller can subsequently be used as the driver.

Rating: Pass.

### M64-U4 — constraints and curves

Inspect:

- **Tangent and radial-normal construction**
- **Angle and dimension annotations**
- **Contextual constraint annotations**
- **Curve family gallery**
- **Periodic NURBS specimen**

Expected: accepted annotations remain selectable/editable, branch/target controls appear when
applicable, curve families render correctly, and zoom/pan works for large scenes.

Rating: Pass.

### M64-U5 — catalog cleanup

Expected:

- no milestone-named folders;
- no third-level overflow menu;
- no guided questions, scripted action buttons, verification checklist, transcript, evidence,
  reset or exit UI;
- no sample prevents ordinary editing;
- switching samples replaces the current workspace rather than hiding it behind a special mode.

Rating: Pass.

## Findings

Add each finding as `M64-Fnnn` with the sample key, exact actions, observed result and expected
result. Objective solver or state failures require an owning-layer regression before recheck.

No findings were reported in the approval review.

## Approval

Approval record (2026-07-30): the supervising human reported satisfaction with the candidate and
explicitly asked to close M64. This approves the complete scorecard and closes M64 for its recorded
scope.
