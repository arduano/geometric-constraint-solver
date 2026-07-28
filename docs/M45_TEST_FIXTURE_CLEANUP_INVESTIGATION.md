<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M45 test and fixture cleanup investigation

## Status and parent disposition

This completed M45 inventory is the source evidence for cleanup M46-M50. `PLAN.md` and
`docs/M46_DIRECT_TEST_REPLACEMENT.md` now resolve its open ownership choices: all durable
claims move to direct Rust/WASM owning-layer tests, browser-only delivery is explicitly
retired, M47 removed M44 E2E, M48 removed M40 E2E, and M49 closed the M14/92-test
ledger before M50 deleted the old runtime and all remaining browser infrastructure.
M51-M52 subsequently completed; M53 now owns the human review over the sole workbench.

## Requirements

- Establish the legacy-versus-replacement UI boundary and classify useful, transitional,
  and obsolete tests/demo fixtures before implementation cleanup
  (`docs/M45_CLEANUP_PLAN.md#parent-cleanup-decisions`).
- Preserve all ten M45 UAT verification points with replacement fixtures or focused
  regressions before retiring the temporary deterministic fixture
  (`docs/M53_UAT.md#preserved-verification-points`).
- Do not represent the deferred full M14 browser suite as passed or weaken its assertions
  (`docs/M53_UAT.md#archived-pre-cleanup-record`).
- The browser remains a non-authoritative, disposable public-API consumer; deterministic
  interaction policy belongs to `geosolve-constraint-editor` (`PLAN.md:75-89, 105-111`).

## Evidence and source pointers

### Durable domain and headless-oracle coverage

- `crates/geosolve-sketch/tests/m14.rs:40-1145` contains 12 native M14 regressions.
  Its named coverage includes scale-invariant alpha fixtures and explicit branches
  (line 42), diagnostic/rank behavior (193), stress mechanisms (273-643), canonical
  A1/A2 workflow (720), A3/A4 contact branch retention (844), A5/A8 persistence (1002),
  and A6/A7/A9 atomic failures/history/import behavior (1145). This is durable domain
  behavior, not browser-fixture ownership.
- `crates/geosolve-sketch/tests/m13.rs:9-166` has four durable public-API boundary
  regressions for compound transactions, rejected atomicity, projected contact drag, and
  browser-domain-free contact creation/deletion.
- `crates/geosolve-sketch/tests/m41.rs:34-443`, `m42.rs:75-1031`, and
  `m43.rs:86-777` are durable domain contracts for construction/activity, host parameter
  batches/bindings, and immutable external snapshots/rebinding respectively.
- The M40 deterministic native oracle is
  `crates/geosolve-constraint-editor/src/qualification.rs:20-37,89-103,171-202,235-2037`.
  Its checked-in transition corpus is
  `crates/geosolve-constraint-editor/tests/m40_transition_corpus.json`, and native tests
  enforce the oracle, byte-identical golden report, and frozen evidence IDs at
  `qualification.rs:2041-2069`. These artifacts are durable as headless qualification
  evidence, although M40 browser duplication is transitional.

### Browser/demo ownership observed so far

- The old playground is explicitly non-authoritative and replaceable
  (`START_HERE.md:12-17`; `PLAN.md:75-89`). Its legacy inline test bulk is in
  `crates/geosolve-demo-web/src/playground.rs:6053-9155` (reported 51 tests) and
  `crates/geosolve-demo-web/src/lib.rs:4791-6793` (reported 41 tests). These need
  per-test classification; their location in legacy UI code is not itself evidence to
  delete a domain assertion.
- M40 browser qualification remains a transitional adapter/cross-channel check:
  `crates/geosolve-demo-web/e2e/m40.mjs:241-268` asserts the browser owns no interaction
  policy/equations and that release-WASM corpus output equals the native golden. It also
  exercises browser wiring for retained editor behavior later in the file. Historical
  qualification is 14/14, but M45 preparation intentionally does not run it
  (`docs/M53_UAT.md#archived-pre-cleanup-record`).
- The focused workbench replacement tests cover route isolation
  (`src/workbench/routing.rs:21-30`), preview terminal behavior
  (`effect_adapter.rs:52-101`), accepted-scene identity and arc flags
  (`scene.rs:310-345`), host markup evidence (`panels.rs:558-606`), and host-state flows
  (`host_state.rs:386-529`).
- `crates/geosolve-demo-web/e2e/m44.mjs:18-25` has six frozen fresh-profile coverage
  groups. M44 focused qualification passed 6/6 and the focused demo-web suite reported
  103 tests (`docs/M44_IMPLEMENTATION.md#historical-deferred-gate-record`;
  `docs/M53_UAT.md#archived-pre-cleanup-record`).

### Temporary M45 fixture

- `crates/geosolve-demo-web/src/workbench/host_state.rs` implements a deterministic,
  in-memory fixture using public typed APIs (`docs/M44_IMPLEMENTATION.md#requirements`),
  with four native tests at lines 386-529. The sidecar is neither canonical sketch state
  nor workspace persistence (`docs/M44_IMPLEMENTATION.md#requirements`;
  `docs/M53_UAT.md#archived-pre-cleanup-record`).
- It is retained only while this investigation establishes replacement boundaries and is
  explicitly scheduled for deletion/replacement—not immediate deletion
  (`docs/M45_CLEANUP_PLAN.md#ordered-cleanup-boundary`;
  `docs/M53_UAT.md#archived-pre-cleanup-record`).

### Exact M45-point coverage and replacement boundary

The six M44 browser groups are a useful index, not a substitute for domain tests:

| M45 UAT point(s) | Focused evidence at the M45 snapshot | Retirement boundary |
| --- | --- | --- |
| 1 | `m44.construction-profile` (`e2e/m44.mjs:171-188`); public-transaction test (`host_state.rs:449-482`) | Retain a focused construction/profile consumer regression. |
| 2–3 | `m44.suppression-dimension-mode` (`m44.mjs:190-202`); activity-closure native test (`host_state.rs:502-532`) | Retain focused activity reason and presentation tests. |
| 4 | `m44.parameters-bindings-proposals` (`m44.mjs:204-225`); fixture acceptance (`host_state.rs:387-432`) | Retain an atomic typed-batch/proposal regression independent of a broad fixture. |
| 5 | `m44.identities-retention` (`m44.mjs:227-240`); failed-input retention (`host_state.rs:435-446`) | Retain invalid/stale/recovery assertions with accepted identity evidence. |
| 6–7 | `m44.external-rebind-retention` (`m44.mjs:242-261`); topology/rebind native path (`host_state.rs:484-499`) | Retain missing/stale/topology/rebind/fresh-snapshot regression. |
| 8 | identities group (`m44.mjs:227-240`) plus accepted-scene test (`workbench/scene.rs:329-345`) | Retain explicit design/attempt/accepted rendering contract. |
| 9 | `m44.host-boundary` capture assertions (`m44.mjs:263-293`) | Replace only with a smaller deterministic capture fixture that still records exact typed inputs and accepted/attempted evidence. |
| 10 | Aggregate groups above; the natural-use pass is human-only (`docs/M53_UAT.md#post-cleanup-m53-procedure-3045-minutes`) | Preserve objective state transitions; resume human clarity judgment only after replacement is available. |

The M45 fixture's intentional values and transitions were concentrated in
`host_state.rs:40-185` and `189-324`: one rectangle, two shared driving-dimension
bindings, one output proposal, an activation binding, a line external binding, and action
strings for valid/invalid/stale parameter and missing/stale/topology/rebind/fresh external
input. That concentration is why it is appropriate as temporary UAT evidence but too broad
to become durable product fixture state.

### Legacy test and fixture classifications

- **Durable only if moved out of legacy UI code:** legacy test clusters that validate
  capsules, exact typed external evidence, visual-profile certification, curve authoring,
  or scale-safe rendering. For example `playground.rs:6163-6431` owns a non-authoritative
  scene-capsule codec/evidence format, while `playground.rs:6434-6536` mixes domain
  acceptance with markup/storage assertions. Split portable format/domain assertions from
  disposable playground presentation before removing their home.
- **Obsolete presentation candidates:** selector/option inventory and legacy scene markup
  assertions, e.g. `playground.rs:6101-6160` and `lib.rs:4827-4963`, because the default
  route is now the workbench and the legacy lab requires the explicit `#/dev/lab` route
  (`workbench/routing.rs:9-26`). Keep only an explicit-route smoke test while the lab is
  still shipped.
- **Transitional adapter tests:** old demo-web live-scene tests that prove a public result
  reaches SVG/audit markup, such as `lib.rs:4791-4823`, and M40 browser wiring. They are
  useful until equivalent focused workbench presentation coverage exists, but do not own
  solver correctness.
- **Obsolete after durable migration:** the legacy full `e2e/m14.mjs` as an M45 gate. Its
  non-browser scenario correctness belongs to M13/M14 domain tests; any unique browser
  semantic accessibility/identity assertion must be rehomed into direct presentation or
  editor tests; browser-only pointer/focus delivery retires. It must remain recorded as
  incomplete until M50 removes it.

## Decisions / inferred constraints

- **Keep:** native sketch M13/M14/M41/M42/M43 behavior tests and the M40 corpus/golden
  oracle. They assert reusable public/domain or headless-editor contracts that a UI
  replacement must not own.
- **Keep, but focus:** direct M44 workbench tests are useful input to M47. Historical M44
  browser checks are replaced and deleted there.
- **Transitional:** M40 browser qualification is historical cross-channel evidence. M48
  retains the native corpus/golden, directly tests durable adapter claims, and deletes it.
- **Candidate obsolete after assertion migration:** the legacy playground UI and its
  duplicate UI-specific inline tests, plus the costly legacy M14 E2E. No removal decision
  is justified until each unique assertion is mapped to a durable domain, headless, or
  focused-workbench replacement.
- The temporary fixture must be replaced by smaller focused fixtures/regressions that
  collectively cover every M45 UAT point, rather than promoted to persistence or product
  state.
- The M45 and M40 serving scripts are transitional historical infrastructure. M48 removes
  `serve-m40.sh`; M50 removes `serve-m45.sh`. M52 prepares a minimal post-cleanup manual
  entry point for M53 without restoring automated browser E2E.

### Ordered cleanup decision

1. M46 freezes this replacement/retirement matrix without deleting current fixtures.
2. M47 replaces the broad deterministic host fixture with focused direct groups and removes
   the M44 script/controls.
3. M48 directly qualifies the workbench/editor/persistence/WASM adapter and removes M40 E2E.
4. M49 extracts retained capsule/profile/advanced/spatial semantics into suitable non-UI
   owners and records retirement of browser rendering/delivery.
5. M50 removes the legacy-lab route, M14 E2E, remaining legacy consumer tests and obsolete
   infrastructure together. No legacy smoke route remains.

Focused non-browser validation after each cleanup batch (no costly browser suite):

```bash
nix-shell shell.nix --run 'cargo fmt --all -- --check'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch --test m13 --test m14 --test m41 --test m42 --test m43'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor'
nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features'
nix-shell shell.nix --run 'cargo clippy --locked -p geosolve-demo-web --all-targets --all-features --no-deps -- -D warnings'
nix-shell shell.nix --run 'cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown'
```

## Resolved questions

- M46 assigns every one of the 16 class-E entries a direct successor or reviewed retirement.
- Scene-capsule semantics remain only if M49 gives them a direct codec/evidence owner;
  otherwise the format and glue retire in M50. It is not currently a supported product API.
- No M40 browser smoke survives. Durable adapter contracts move to direct tests in M48;
  browser-only delivery retires.

## Exhaustive legacy consumer-test classification

Classes at the M45 snapshot: **A** = durable assertion to migrate; **B** = transitional
browser-adapter assertion; **C** = duplicate with the stated replacement; **D** = legacy
presentation/demo retirement candidate; **E** = product-scope/ownership decision then still
required. “Migrate” does not mean copy the legacy test: retain only its non-presentation
contract in the named owner. M46 resolves every E row below; the table wording remains the
historical pre-decision inventory.

### `src/playground.rs` inline tests (51)

| Test and source pointer | Class | Disposition / exact replacement evidence |
| --- | --- | --- |
| `viewport_transform_zoom_and_hit_geometry_round_trip` (`playground.rs:6054`) | B | Browser coordinate/finite-grid adapter; keep only while `#/dev/lab` ships. |
| `alpha_scale_extremes_fit_inside_the_editable_canvas` (`:6087`) | B | Scale-in-canvas presentation; domain scale behavior is M14 (`tests/m14.rs:40-1145`). |
| `public_domain_example_selector_keys_are_visible` (`:6101`) | D | Legacy selector/HTML inventory. |
| `scene_capsule_codec_and_profile_options_round_trip_deterministically` (`:6163`) | E | Diagnostic capsule support/owner is unresolved (`Open questions`). |
| `malformed_scene_capsules_retain_the_accepted_document_atomically` (`:6213`) | E | Same capsule ownership decision; atomic document semantics otherwise belong in sketch tests. |
| `scene_capsule_status_is_non_authoritative` (`:6258`) | E | Same capsule decision. |
| `scene_capsule_decodes_exact_external_evidence_without_publishing_it` (`:6273`) | E | Same capsule decision; M43 covers external snapshot semantics (`tests/m43.rs:86-777`). |
| `exact_nurbs_control_authoring_refreshes_certified_self_root_evidence` (`:6434`) | A | Move visual-profile/NURBS semantic part to sketch; discard legacy markup check. |
| `m19_conic_examples_are_editable_accepted_documents_at_all_scales` (`:6489`) | A | Move accepted-document/scale assertions to sketch; legacy SVG/storage checks retire. |
| `every_conic_tool_creates_exact_persistent_state_atomically_and_cascade_deletes` (`:6585`) | A | Split document creation/delete atomicity into sketch tests. |
| `post_draw_trim_and_homogeneous_handles_drag_as_one_transaction` (`:6808`) | A | Retain transaction/branch semantics in sketch; handle UI is B. |
| `invalid_configuration_handle_targets_retain_document_and_history` (`:6898`) | A | Move invalid-edit atomicity to sketch session tests. |
| `associative_line_fillet_arc_has_no_direct_trim_handles` (`:6928`) | A | Move fillet ownership/handle eligibility semantic to sketch. |
| `conic_previews_use_clone_only_persistent_sampling_and_omit_invalid_candidates` (`:6972`) | B | Preview rendering and draft DOM are adapter-only. |
| `conic_failures_retain_all_accepted_state_and_full_drafts_retry_without_extra_clicks` (`:7061`) | A | Move failed document edit retention; draft retry affordance is B. |
| `conic_tool_ui_is_complete_and_spatially_hidden` (`:7158`) | D | Legacy controls, CSS, and responsive layout inventory. |
| `m20_spatial_examples_render_accepted_features_and_physical_reports_at_all_scales` (`:7227`) | E | Workbench product scope does not yet establish a spatial consumer. |
| `spatial_mode_rejects_hidden_sketch_edits_and_has_no_storage_payload` (`:7372`) | E | Same spatial-mode product decision. |
| `advanced_constraint_stress_examples_render_valid_public_documents` (`:7408`) | C | Scenario/rank behavior is covered by M14 (`tests/m14.rs:273-643`); legacy labels/SVG retire. |
| `straight_curves_use_only_their_exact_endpoints` (`:7519`) | A | Move sampling contract to sketch. |
| `imported_full_ellipse_samples_its_complete_period` (`:7549`) | A | Move public curve sampling contract to sketch. |
| `imported_bspline_samples_every_public_semantic_span` (`:7601`) | A | Move public span sampling contract to sketch. |
| `m28_visible_intervals_drive_every_curve_consumer_and_explode_cleanly` (`:7647`) | A | Move visible-interval/delete semantics to sketch; focused M28 browser smoke is B. |
| `failed_nurbs_sampling_is_not_connected_and_is_reported` (`:7872`) | A | Move sampling failure/no-fabrication semantic to sketch. |
| `every_alpha_draw_tool_creates_one_atomic_history_entry` (`:7930`) | E | Whether full freeform authoring is a workbench product capability is unresolved. |
| `every_draw_tool_has_a_staged_primitive_preview` (`:7979`) | E | Same authoring-scope decision. |
| `pointer_cancel_and_invalid_completion_retain_the_staged_draft` (`:8033`) | E | Same authoring-scope decision; pointer wiring itself is B. |
| `deleting_each_new_shape_removes_its_generated_controls` (`:8061`) | E | Same authoring-scope decision. |
| `selection_constraints_dimensions_drag_history_and_json_use_document_session` (`:8090`) | E | Broad legacy authoring workflow; split only after replacement scope is chosen. |
| `free_line_drag_crosses_its_inactive_branch` (`:8156`) | A | Move explicit branch-state semantic to sketch. |
| `a5_line_endpoint_drag_stabilizes_the_opposite_bezier_handle` (`:8223`) | C | M14 A5 branch/stability coverage (`tests/m14.rs:1002-1145`). |
| `drawn_rectangle_has_free_size_and_full_geometry_delete_cascades` (`:8281`) | E | Full authoring workflow scope unresolved. |
| `inference_is_provisional_until_confirmed` (`:8367`) | E | Inference is not yet assigned to workbench or editor product scope. |
| `page_exposes_document_tools_mobile_input_and_accepted_diagnostics` (`:8381`) | D | Legacy page/CSS inventory. |
| `click_without_motion_preserves_history_and_polyline_spans_multiselect` (`:8446`) | E | Legacy selection model has no named workbench replacement. |
| `conflict_attempt_is_mapped_separately_from_retained_accepted_view` (`:8496`) | A | Move accepted-versus-attempt semantic to sketch; workbench scene test complements it (`workbench/scene.rs:329-345`). |
| `explicit_arc_branch_reference_measurement_and_imported_labels_render_truthfully` (`:8520`) | A | Move branch/import escaping semantics; legacy SVG check retires. |
| `deleting_a_contact_constraint_removes_its_owned_hidden_state` (`:8577`) | A | Move ownership cascade to sketch. |
| `endpoint_tangency_and_persisted_branch_edits_use_explicit_state` (`:8609`) | C | M14 A3/A4 explicit contact branches (`tests/m14.rs:844-1001`). |
| `paired_contacts_keep_independent_neighborhoods_and_touch_selection` (`:8698`) | A | Move independent-contact state semantic; touch selection is B. |
| `autosave_payload_retries_until_browser_confirms_storage` (`:8773`) | B | Browser storage adapter only. |
| `all_constraint_buttons_create_their_public_document_definition` (`:8782`) | E | Full authoring constraint palette scope unresolved. |
| `every_dimension_kind_supports_reference_display_and_driving_edit` (`:8854`) | E | Full authoring dimension palette scope unresolved. |
| `visual_profile_overlay_is_read_only_and_has_no_interaction_identity` (`:8913`) | B | Focused rendering/interaction adapter assertion. |
| `curved_profile_edges_have_adaptive_interior_points_in_directed_order` (`:8933`) | A | Move profile sampling semantic to sketch. |
| `reverse_directed_profile_parameters_are_not_reordered` (`:8984`) | A | Move directed-profile semantic to sketch. |
| `nested_profile_holes_share_one_even_odd_overlay_path` (`:9036`) | B | SVG composition adapter. |
| `native_budget_scene_never_gains_a_web_overlay` (`:9055`) | B | Native status is durable; web overlay absence is adapter-only. |
| `web_budget_failure_omits_whole_face_without_changing_native_status` (`:9073`) | B | Web render-budget adapter. |
| `sampled_profile_gap_omits_whole_face_instead_of_drawing_connector` (`:9119`) | B | Web rendering safety adapter. |
| `box_selection_and_pan_gestures_are_web_only_and_deterministic` (`:9156`) | B | Browser gesture adapter. |

### `src/lib.rs` legacy/non-workbench inline tests (41)

All are legacy live-demo consumer tests, rather than workbench tests.  The tests that assert
solver/retention/branch semantics are **A** until relocated; exact markup, viewport, CSS,
pointer, and accessibility wiring is **B**; old scenario selector/demo inventory is **D**.

| Class | Tests (source lines) |
| --- | --- |
| A | `s3_action_switches_explicit_modes_on_the_positive_branch_transactionally` (`:4966`); `arc_drag_updates_committed_span_and_rejects_escape_without_republishing` (`:5068`); `auto_radius_scene_starts_accepted_with_two_dof_and_no_circle_driver` (`:5146`); `auto_radius_two_dimensional_drags_solve_distinct_radii_contacts_and_release` (`:5275`); `auto_radius_invalid_span_side_and_zero_radius_requests_retain_all_published_state` (`:5332`); `tangent_glide_updates_contacts_and_rejects_supporting_line_escape` (`:5412`); `rejection_wording_uses_typed_classification_in_banner_and_curve_hud` (`:5495`, typed mapping only); `ambiguous_auto_radius_scale_has_truthful_typed_retention_ui` (`:5552`, retention only); `rebuilding_an_m7_scene_resets_geometry_branch_and_contact_state` (`:5624`); `s2_initializes_from_expected_rejection_with_only_typed_width_conflicts` (`:5723`); `horizontal_rail_drag_projects_to_one_dof_and_release_preserves_position` (`:5860`); `coincident_pair_drag_moves_both_points_and_release_preserves_common_position` (`:5895`); `rejected_attempt_renders_retained_geometry_and_display_audit` (`:6218`, retention only); `api_error_keeps_display_and_diagnostics_without_a_stale_attempt_report` (`:6266`); `retained_diagnostics_can_fall_back_to_audit_and_hide_invalid_rank` (`:6286`); `incomplete_empty_diagnostics_are_never_rendered_as_none` (`:6308`); `radius_cue_uses_the_public_distance_dimension_target` (`:6334`); `l1_l2_l3_states_start_accepted_with_explicit_branches_and_valid_velocity` (`:6417`); `linkage_state_drives_low_mid_high_with_bounded_validated_continuation` (`:6460`); `exact_toggle_failure_keeps_retained_linkage_display_and_diagnostics` (`:6605`); `accepted_position_with_forced_velocity_failure_rolls_back_atomically` (`:6649`). |
| B | `live_s1_view_comes_from_an_accepted_sketch_result` (`:4792`); `s1_has_no_static_audit_or_handwritten_equation_templates` (`:4817`); `auto_radius_svg_title_uses_rank_valid_report_mobility` (`:5249`); `generic_scene_action_is_visible_only_for_s3_and_uses_a_native_button` (`:5585`); `s2_render_uses_retained_geometry_display_audit_and_expected_conflict_status` (`:5793`); `every_live_scene_renders_only_its_evaluated_display_audit_rows` (`:5933`); `model_svg_and_client_view_box_transforms_round_trip` (`:5997`); `m7_arc_and_tangent_client_model_transforms_follow_the_responsive_viewport` (`:6020`); `auto_radius_mobile_transform_and_center_hit_target_remain_usable` (`:6054`); `arc_ccw_240_svg_path_has_exact_large_arc_and_screen_sweep_flags` (`:6094`); `auto_radius_ccw_300_svg_path_has_exact_large_arc_and_screen_sweep_flags` (`:6109`); `viewport_drag_state_and_tangent_endpoint_styles_are_explicit` (`:6123`); `outside_rail_and_coincident_drags_retain_fully_visible_handles` (`:6157`); `pointer_start_requires_one_primary_pointer_and_left_mouse_button` (`:6353`); `interaction_does_not_advertise_an_inaccessible_svg_button` (`:6364`); `viewport_css_preserves_exact_ratio_and_hit_target_is_large_enough_when_narrow` (`:6385`); `linkage_rendering_uses_display_geometry_audit_and_driver_source_identity` (`:6512`); `linkage_degree_controls_and_scene_transforms_are_pure_and_accessible` (`:6741`); `dynamic_audit_strings_are_html_escaped` (`:6794`). |
| D | `all_eleven_selectors_and_names_map_to_fresh_domain_scene_kinds` (`:4827`): legacy demo selector/page inventory. |

The `lib.rs` count is 21 A + 19 B + 1 D = **41**.  The `playground.rs` count is 19 A +
10 B + 3 C + 3 D + 16 E = **51**.

### `e2e/m14.mjs` coverage groups

| Group (definition; normal-flow invocation) | Class | Disposition |
| --- | --- | --- |
| `layoutPrioritySuite` (`:2367`; `:2428,2466`) | D | Legacy page layout only. |
| `scenarioSuite` / `scaleWorkflow` / `historySuite` / `creationSuite` / `stressExampleSuite` / `reportedRegressionSuite` (`:2047/:1879/:1801/:520/:576/:2175`; `:2453,2467`) | E | Broad legacy authoring product scope; M13/M14 replace domain correctness, not all pointer/authoring UX. |
| `conicCreationSuite` (`:1581`; `:2454`) | E | Same unresolved authoring-surface decision. |
| `newDomainExampleSuite` (`:731`; `:2455`) | E | Conic/spatial legacy-demo product scope unresolved. |
| `m28VisibleTrimSuite` (`:802`; `:2456,2469`) | A | Migrate semantic interval/delete assertions to sketch; retain at most a small focused workbench adapter smoke. |
| `m30DesktopSuite` / `m30MobileSmokeSuite` (`:943/:1025`; `:2457,2470`) | E | Legacy interaction product scope unresolved. |
| `m31DesktopSuite` / `m31MobileSmokeSuite` (`:1144/:1320`; `:2458,2471`) | E | Profile presentation ownership/replacement not selected. |
| `m32DesktopSuite` / `m32BrowserPerformanceSuite` (`:1375/:1535`; `:2459-2460`) | E | Performance/browser budget replacement requires an explicit workbench performance contract. |
| `fileSuite` (`:2239`; `:2461,2472`) | E | Legacy import/download UI scope unresolved; document persistence semantics are separately durable. |
| `recoverySuite` (`:2255`; `:2462,2473`) | B | Browser local-storage/backup adapter; domain atomicity is covered by M13/M14. |
| `branchHistoryRecoverySuite` (`:2322`; `:2463,2474`) | B | Browser persistence adapter; branch/history semantics are M14 durable coverage. |
| `renderBudgets` (`:2351`; `:2464`) | E | No replacement workbench performance budget has been chosen. |
| `mobileConicSuite` (`:1755`; `:2468`) | E | Same unresolved authoring-surface decision. |

### Coupled-removal prerequisites and totals

- Do **not** remove a legacy A test until its named semantic assertion is added to the
  appropriate sketch/editor owner; do **not** retain its legacy markup merely because the
  semantic test moved.
- Remove B tests together with the legacy lab route/adapter, unless a deliberately smaller
  workbench smoke is introduced.  Current route evidence is `workbench/routing.rs:9-26`.
- C removals require no new assertion: their exact replacement is the cited M13/M14 native
  suite.  D removals are coupled to removal of the legacy route/page assets or retention of a
  consciously supported lab smoke.  E entries require a parent product-scope decision before
  either migration or retirement.
- Inline-test total: **92** = A **40**, B **29**, C **3**, D **4**, E **16**.  E2E groups:
  A **1**, B **2**, D **1**, E **9** (combined paired suites count as their normal-flow group).
- The full M14 E2E suite remains **deferred/not run**, not passed.

### Parent disposition of class-E ownership

M46 resolves the pre-decision E rows as follows:

| M45 E rows | Final M46 disposition |
| --- | --- |
| Four scene-capsule codec/status/external-evidence tests | Retire the private scene-capsule format and legacy glue in M50. Canonical document persistence remains owned by sketch M14 tests; typed external evidence remains owned by sketch M43 tests; M47's separate test/UAT-only finding capture has a direct `workbench::evidence` checksum/content test. |
| Two spatial-demo tests | Retain solver, branch, scale, continuation, rollback and report semantics in existing linkage M20/M23 tests plus proposed `crates/geosolve-linkage/tests/m49_legacy_consumer.rs`; retire the legacy browser spatial consumer and its storage/rendering claims. |
| Atomic full-palette creation, staged preview, cancel/invalid completion and generated-control deletion | Retain transaction/dependency semantics in existing sketch M13/M14 tests and proposed `crates/geosolve-sketch/tests/m49_advanced_geometry.rs`; retain progression/preview/cancel effects in the M40 editor corpus and proposed M48 adapter tests; retire legacy palette completeness and pointer delivery. |
| Broad selection/session/history/rectangle workflow | Retain selection/effect policy in the M40 editor corpus, rectangle/history/dependency semantics in sketch M11/M14, and persistence/identity rendering in named M48 workbench tests; retire the old end-to-end lab flow. |
| Provisional inference | Retain the headless inference/confirmation contract in a proposed `geosolve-constraint-editor` M49 regression; retire legacy pointer/DOM presentation. |
| Click-without-motion and polyline multiselect | Retain no-op/history and ordered-selection semantics in the editor corpus and named M48 selection test; retire legacy pointer/modifier delivery. |
| Complete constraint-button and dimension-palette tests | Retain relation/dimension applicability and executable semantics in sketch M37/M38 plus named M48 editor/workbench DTO tests; retire legacy full-palette controls and DOM display. |

The nine M14 E2E E-groups are likewise resolved by the final direct-owner/retirement matrix
in `docs/M46_DIRECT_TEST_REPLACEMENT.md`: durable domain, editor, persistence and adapter
claims move to the named M47-M49 owners; old authoring UI, mobile/profile presentation,
browser timing and file delivery retire. M50 removes the legacy runtime only after those
replacement tests pass. Placeholder M107X owns any later stable release-surface decision.

## Out of scope

- Running the costly deferred M14 or preserved M40 browser suites during this
  investigation.
- Recording M45 human approval, changing acceptance thresholds, or deleting/modifying
  implementation, test, configuration, or generated files.
