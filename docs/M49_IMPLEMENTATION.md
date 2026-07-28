<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# M49 implementation ledger: legacy semantic extraction

## Status

Complete (2026-07-28). The sketch/linkage, editor and focused workbench owners pass their
direct commands, all final non-browser gates pass, independent read-only verification passes,
and this ledger has zero unowned assertions. It authorizes no M14 E2E, playground, route, CSS,
serving, release-gate or legacy inline-test deletion; M50 subsequently completed that purge.

## Requirements and fixed boundary

- Preserve only durable domain, transaction, branch, history, sampling, profile, persistence
  and focused presentation semantics outside the legacy runtime.
- Confirm class-C duplicates against their exact native tests rather than copying them.
- Give every class-A claim a passing direct Rust owner in sketch, linkage, editor,
  persistence, or focused workbench presentation.
- Record explicit retirement rationales for class-B/D delivery/layout/demo claims and the
  class-E legacy capabilities resolved for retirement by M46.
- Retain canonical document/external-input codecs at their domain owners; retire private
  scene capsules and browser file-picker/download delivery.
- Do not serve the application, run `e2e/m14.mjs`, launch Chromium/CDP, scrape a DOM, use
  screenshots/timing/retries, or replace policy review with source-substring scans.

## M14 browser-group ledger

### Requirements

- This is the complete normal-flow inventory from M45: paired desktop/mobile suites are one
  group, and `m32DesktopSuite`/`m32BrowserPerformanceSuite` are one M32 group.  Thus there are
  **13** groups (A 1, B 2, D 1, E 9), not a claim that the browser suite passed.
- “Retained” below means a domain/editor/codec assertion, not its legacy-lab route, pointer,
  DOM/SVG, device-emulation, local-storage, file-picker, download, layout, or timing delivery.
  The M46 product-scope retirement decisions are fixed.

### Evidence and source pointers

| M45 normal-flow group | Durable semantic assertions that must survive | Existing direct owner evidence | Smallest still-missing direct target | Reviewed retirement rationale |
| --- | --- | --- | --- | --- |
| `layoutPrioritySuite` | None. | — | None. | D: canvas/inspector ordering and sizes, badge placement, overflow, responsive breakpoints, and CSS geometry are legacy page layout. |
| `scenarioSuite` / `scaleWorkflow` / `historySuite` / `creationSuite` / `stressExampleSuite` / `reportedRegressionSuite` | Accepted solves have independently validated residuals; A1/A2 edits and projected drag; A3/A4 explicit contact/branch state and branch-escape rejection; A5 continuity/regularity; scale-invariant IDs, source order, branches, reports, and canonical round trips; conflict/invalid-import retention; accepted-only undo/redo, suppression, and dependency deletion; stress linkage motion/rank/diagnostics.  Ordinary draft completion/cancel remains editor-owned. | `crates/geosolve-sketch/tests/m14.rs::alpha_fixtures_solve_with_scale_invariant_ids_and_explicit_branches`, `::a1_dimension_edits_and_a2_projected_drag_match_the_canonical_workflows`, `::a3_a4_contacts_retain_explicit_state_and_reject_branch_escape`, `::a5_and_a8_round_trip_preserve_geometry_ids_and_branches`, `::a6_a7_a9_conflict_history_and_import_failures_are_atomic`, `::compass_stress_example_exposes_and_locks_rotational_mobility`, `::bridge_stress_example_exposes_mobility_and_rejects_degeneracy`, `::cam_motion_projects_one_roller_while_stabilizing_the_other`, `::tangent_orbit_projected_drag_traverses_all_quadrants`, `::compound_constraint_mechanisms_follow_their_emergent_motion`, `::advanced_linkage_examples_propagate_one_driver_through_every_bar`, `::advanced_diagnostic_examples_expose_rank_bounds_and_redundancy`; `crates/geosolve-sketch/tests/m11.rs::accepted_only_history_round_trips_create_edit_suppress_and_delete`, `::batch_delete_cascades_from_selected_rectangle_geometry`; `crates/geosolve-constraint-editor/src/lib.rs::every_core_draft_has_exact_completion_and_cancellation`, `::finish_commits_then_clears_the_polyline_preview`. | None for these retained primitives. | E: scenario menus, pointer/keyboard/touch paths, SVG/DOM coordinate parity, zoom/pan/resize, object-list/audit text, and full legacy authoring flow retire. |
| `conicCreationSuite` | Conic families retain typed finite/bounded scalars, explicit arc sweep/hyperbola branch, accepted projection, atomic failed edits, history, canonical persistence, and trim semantics. | `crates/geosolve-sketch/tests/m19.rs::persistent_families_validate_query_round_trip_lower_and_solve`, `::persistent_conic_commands_history_and_failed_edits_are_atomic`, `::native_conic_trim_projection_preserves_reversed_trims_and_hyperbola_branches`, `::trim_projection_reports_typed_failures_and_leaves_degenerate_edits_transactional`; combined family lifecycle: `m49_advanced_geometry.rs::five_conic_lifecycle_and_legacy_signatures_preserve_typed_state_and_validation`. | None. | E: tool palette/options, hover preview/handles, viewport homogeneous text, click sequencing, and desktop delivery retire. |
| `newDomainExampleSuite` | Canonical planar conic/trim document semantics survive; spatial solver validity, rank/mobility, branch/continuation, rollback, and reports survive independently of the browser consumer. | Planar: `crates/geosolve-sketch/tests/m19.rs::public_conic_examples_validate_lower_round_trip_solve_and_render_at_all_scales`, `m28.rs::reusable_trimmed_fillet_alpha_scenario_is_accepted_and_scale_invariant`, and `m49_advanced_geometry.rs::five_conic_lifecycle_and_legacy_signatures_preserve_typed_state_and_validation`. Spatial foundations: `crates/geosolve-linkage/tests/m20.rs::m20_joints_report_exact_floating_and_grounded_rows_rank_and_mobility`, `::shaft_bearing_driver_stage_matrix_reports_internal_two_one_one_zero`, `::block_base_three_target_transaction_commits_once_and_failures_roll_back_all_state`; `m23_continuation.rs::shaft_bearing_hinge_continuation_retains_winding_and_translation`; direct legacy signatures: `m49_legacy_consumer.rs`. | None. | E: release-contract prose/links, selector listing, spatial read-only page, hidden controls/pan/zoom, browser storage isolation, and spatial rendering retire. |
| `m28VisibleTrimSuite` (desktop/mobile) | Visible intervals are derived from trim ownership; generic fillet creates two trims; deleting the association explodes to fixed trims while retaining the output arc; ownership/dependency deletion and canonical v4 round trip are atomic. | `crates/geosolve-sketch/tests/m28.rs::reusable_trimmed_fillet_alpha_scenario_is_accepted_and_scale_invariant`, `::generic_command_creates_two_visible_trims_and_projects_parent_edits`, `::suppression_explode_fixed_views_and_output_ownership_are_preserved`, `::v4_round_trip_and_frozen_v1_v3_languages_are_strict`. | None. | A: SVG spans/markers/handles, hit testing/box selection, mobile pointer delivery, reload/localStorage, and rendered status/object text retire. |
| `m30DesktopSuite` / `m30MobileSmokeSuite` | Construction offsets/mirror, directed-angle branch/orientation, generic-fillet branch/radius, NURBS weight/gauge/knot/span state, projected drags, exact documented DOF, and atomic invalid edits survive. | `crates/geosolve-sketch/tests/m30.rs::m30_scenarios_start_accepted_with_exact_documented_dof`, `::offsets_and_mirror_accept_projected_drags_move_associated_geometry_and_keep_history`, `::directed_angle_crosses_cut_then_target_mode_and_orientation_edit_transactionally`, `::line_and_generic_fillet_drags_move_contacts_output_and_trim_state`, `::fillet_branch_radius_history_and_invalid_edit_are_atomic`, `::nurbs_weight_gauge_insertion_transition_and_differential_controls_are_transactional`, `::construction_creation_commands_use_public_side_orientation_target_and_mode_state`. | None. | E: example selector/UAT copy, inspector controls, desktop drag and mobile touch/overflow behavior retire. |
| `m31DesktopSuite` / `m31MobileSmokeSuite` | Certified profile status (complete/truncated/skipped), family/face/intersection evidence, bounded work counters, self-intersection/topology, and read-only analysis survive. | `crates/geosolve-sketch/tests/m31.rs::reusable_profile_scenarios_publish_exact_metadata_and_evidence`, `::analysis_is_read_only_and_reports_consumed_budgets`, `::every_unordered_family_pair_isolated_by_named_roles`, `::every_eligible_family_has_truthful_self_intersection_evidence`, `::every_public_work_budget_fails_closed_with_consumed_evidence`; budget/panic boundary: `m32_release.rs::all_m31_profile_families_and_options_are_panic_safe_and_bounded`. | None; the existing scenario matrix is already exact. | E: panel labels, overlay SVG/fill/pointer-events, inspector edits, scene capsule, mobile overflow, and diagnostic layout retire. |
| `m32DesktopSuite` / `m32BrowserPerformanceSuite` | Retain non-timing offset/fillet/NURBS/profile transaction and rejected-attempt retention semantics only. | `crates/geosolve-sketch/tests/m30.rs::fillet_branch_radius_history_and_invalid_edit_are_atomic`, `::nurbs_weight_gauge_insertion_transition_and_differential_controls_are_transactional`; `m32_release.rs::malformed_and_extreme_m30_commands_are_panic_safe_and_transactional`; profile owners in the preceding M31 row. | No browser-performance replacement.  The two focused M46-profile and advanced-geometry slices above cover any remaining exact non-timing fixture assertion. | E: reset controls, inspector/marker/SVG comparisons, private scene-capsule import/limits, DOM diagnostics, render counts, warmups, median/p95, and wall-clock budgets retire. |
| `fileSuite` (desktop/mobile) | Canonical document JSON must round trip strictly and preserve IDs/branch state. | `crates/geosolve-sketch/tests/m14.rs::a5_and_a8_round_trip_preserve_geometry_ids_and_branches`; `crates/geosolve-sketch/tests/m11.rs::canonical_json_round_trip_preserves_ids_and_branch_state`; focused workbench codec: `crates/geosolve-demo-web/src/workbench/persistence.rs::tests::checkpoint_codec_round_trips_design_accepted_and_revisions`, `::m49_checkpoint_codec_round_trips_accepted_a4_contact_state`. | None. The production snapshot codec stores design/accepted JSON and revision high-water, not command history/cursor. | E: download interception, filename/path, upload input, blob/file-picker, and browser error delivery retire. |
| `recoverySuite` | Rejected edits/imports leave accepted document, revision/history/redo and accepted evidence unchanged; malformed persisted snapshots select a deterministic fallback. | `crates/geosolve-sketch/tests/m14.rs::a6_a7_a9_conflict_history_and_import_failures_are_atomic`; `crates/geosolve-sketch/tests/m11.rs::malformed_imports_and_invalid_edits_leave_session_exactly_unchanged`; `workbench/persistence.rs::tests::codec_rejects_malformed_unknown_version_and_unknown_fields`, `::malformed_snapshot_selects_the_pure_fallback_path`. | None; no local-storage retry or command-history snapshot claim is retained. | B: localStorage quota/backup retry, primary/backup keys, reload recovery, status text, SVG/audit equality, and browser delivery retire. |
| `branchHistoryRecoverySuite` | Explicit contact winding is accepted state, native command history remains undoable/redoable, and canonical persistence preserves branch state. | Branch/contact and canonical persistence: `crates/geosolve-sketch/tests/m14.rs::a3_a4_contacts_retain_explicit_state_and_reject_branch_escape`, `::a5_and_a8_round_trip_preserve_geometry_ids_and_branches`; native history/high-water: `crates/geosolve-sketch/tests/m11.rs::accepted_only_history_round_trips_create_edit_suppress_and_delete`, `::canonical_json_round_trip_preserves_ids_and_branch_state`; adapter accepted-state codec: `workbench/persistence.rs::tests::m49_checkpoint_codec_round_trips_accepted_a4_contact_state`. | None. `WorkspaceSnapshot` deliberately does not encode command history/cursor, so native and adapter ownership remain separate. | B: autosave key mutation, reload and backup-recovery mechanics, and browser storage text retire. |
| `renderBudgets` | None; no native timing contract has been selected. | — | None; a future native benchmark needs a separately approved contract. | E: browser `performance.now()` zoom p95 and render budget gates retire. |
| `mobileConicSuite` | The underlying ellipse/hyperbola branch, scalar, trim, history, and accepted-state semantics are the same conic contracts in `conicCreationSuite`. | `crates/geosolve-sketch/tests/m19.rs::hyperbola_branches_and_reversed_trims_are_explicit_and_retained`, `::persistent_conic_commands_history_and_failed_edits_are_atomic`, `::native_conic_trim_projection_preserves_reversed_trims_and_hyperbola_branches`. | No separate mobile test; the optional `m46_advanced_geometry.rs` combined corpus is shared with desktop conics. | E: touch delivery, narrow control sizing/layout, draft-status text, handle dragging, and mobile autosave retire. |

### Decisions / inferred constraints

- There are **no class-C browser groups** in the M45 M14 group inventory (its classes are A, B,
  D, and E only).  Consequently, none of the three class-C inline-test duplicate relationships
  is represented by, or needs to be copied from, a browser group: **0/3 browser-to-inline class-C
  duplicates**.  The three class-C removals remain governed by M45's inline ledger and their exact
  cited M13/M14 native replacements, not this table.
- Every retained browser-group semantic bundle above has an exact passing Rust test owner.
  Existing tests are not evidence that browser rendering or interaction delivery is supported.

### Open questions

None. Existing M31 tests exercise the exact retained status/family/face matrix, so no separate
profile consolidation is needed. Native tests own command history/high-water while the focused
workbench codec owns accepted explicit-contact state; no browser-group implementation gap remains.

### Out of scope

Browser/UI/mobile/layout/timing/storage/file/download claims listed as retired above, all legacy
route/CDP execution, and any reopening of M46's E-scope decisions.

**Counts:** 13/13 normal-flow groups reviewed; retained semantic bundles with an existing or
named direct owner: **13**; browser-only retirement bundles explicitly reviewed: **13**; class-C
browser duplicates: **0**; unowned browser-group claims: **0**; concrete non-optional missing
implementation slices: **0**; all four focused slices pass, and the two conditional fixture
consolidations were resolved by the focused M49 sketch test and exact existing M31 owners.

## Legacy inline-test ledger

### Requirements

- This is the complete M45 inline inventory: **92 = 40 A + 29 B + 3 C + 4 D +
  16 E** (51 `playground.rs`, 41 legacy `lib.rs`).  “Owner” below means the direct
  Rust boundary for the retained semantic, never legacy markup, pointer delivery, or a
  browser route.
- `existing` names a current direct test. `target` names the smallest missing direct
  regression slice. A retirement code is an explicit reviewed disposition, not an
  unowned claim.

### Evidence and source pointers

M45 is the exhaustive name/class inventory (`M45_TEST_FIXTURE_CLEANUP_INVESTIGATION.md:185-254`)
and records the final E disposition at `:289-301`; M46 fixes the direct-owner boundary and
M14 decisions (`M46_DIRECT_TEST_REPLACEMENT.md:100-119,142-159`). Test-body review confirms
the retained portions described below; the cited native tests are the authoritative owners.

#### A — retained semantic and direct owner (40)

| Legacy test | Retained semantic | Exact owner / smallest missing direct target |
| --- | --- | --- |
| `exact_nurbs_control_authoring_refreshes_certified_self_root_evidence` | Accepted NURBS-control edit refreshes certified self-intersection evidence. | **existing** `crates/geosolve-sketch/tests/m49_advanced_geometry.rs::accepted_nurbs_edit_refreshes_profile_roots_and_directed_edges_keep_parameters`. |
| `m19_conic_examples_are_editable_accepted_documents_at_all_scales` | Conic-gallery/tangency/circle-limit documents solve, retain rank/branch/scalars and scale invariance. | **existing** `crates/geosolve-sketch/tests/m19.rs::conic_gallery_and_tangency_examples_retain_scalar_and_contact_semantics`, `::conic_circle_limit_example_exposes_unobservable_full_axis_and_directed_arc_rank`. |
| `every_conic_tool_creates_exact_persistent_state_atomically_and_cascade_deletes` | Five conic definitions use typed scalars/branches and create/delete/history atomically. | **existing** `m19.rs::persistent_families_validate_query_round_trip_lower_and_solve`, `::persistent_conic_commands_history_and_failed_edits_are_atomic`. |
| `post_draw_trim_and_homogeneous_handles_drag_as_one_transaction` | Accepted conic trim/weighted-middle projection is one history transaction. | **existing** `m19.rs::accepted_projection_updates_persistent_conic_shape_and_contact_state`, `::native_conic_trim_projection_preserves_reversed_trims_and_hyperbola_branches`. |
| `invalid_configuration_handle_targets_retain_document_and_history` | Invalid trim/weight projection is transactional. | **existing** `m19.rs::trim_projection_reports_typed_failures_and_leaves_degenerate_edits_transactional`. |
| `associative_line_fillet_arc_has_no_direct_trim_handles` | Associative fillet owns derived trim state; its output is not independently trimmed. | **existing** `m27.rs::creation_branch_history_explode_and_failed_escape_are_atomic`. |
| `conic_failures_retain_all_accepted_state_and_full_drafts_retry_without_extra_clicks` | Invalid conic command retains accepted document/history. | **existing** `m19.rs::persistent_conic_commands_history_and_failed_edits_are_atomic`. |
| `straight_curves_use_only_their_exact_endpoints` | Public line/polyline intervals expose their exact ordered endpoint supports. | **existing** `m49_advanced_geometry.rs::public_intervals_cover_ellipse_period_spline_spans_and_failed_nurbs_has_no_connector`; private renderer subdivision/count policy is retired. |
| `imported_full_ellipse_samples_its_complete_period` | Full ellipse public support closes exactly over one period. | **existing** `m49_advanced_geometry.rs::public_intervals_cover_ellipse_period_spline_spans_and_failed_nurbs_has_no_connector`. |
| `imported_bspline_samples_every_public_semantic_span` | Public intervals preserve each B-spline span identity and boundary continuity. | **existing** `m49_advanced_geometry.rs::public_intervals_cover_ellipse_period_spline_spans_and_failed_nurbs_has_no_connector`. |
| `m28_visible_intervals_drive_every_curve_consumer_and_explode_cleanly` | Visible intervals, contact admissibility, association deletion/explode and canonical persistence are domain-owned. | **existing** `m28.rs::reusable_trimmed_fillet_alpha_scenario_is_accepted_and_scale_invariant`, `::generic_command_creates_two_visible_trims_and_projects_parent_edits`, `::suppression_explode_fixed_views_and_output_ownership_are_preserved`, `::v4_round_trip_and_frozen_v1_v3_languages_are_strict`. |
| `failed_nurbs_sampling_is_not_connected_and_is_reported` | Failed public NURBS evaluation yields no fabricated connector and a typed failure. | **existing** `m49_advanced_geometry.rs::public_intervals_cover_ellipse_period_spline_spans_and_failed_nurbs_has_no_connector`; browser connector omission is retired presentation. |
| `free_line_drag_crosses_its_inactive_branch` | A free line may cross an inactive direction; enforced branch state is explicit. | **existing** `m11.rs::free_line_drag_preserves_inactive_branch_but_enforced_rectangle_branch_does_not_flip`. |
| `conflict_attempt_is_mapped_separately_from_retained_accepted_view` | Rejected conflicting dimension leaves accepted document distinct from attempted diagnostics. | **existing** `m11.rs::conflicting_command_retains_document_and_maps_both_persistent_sources`; `workbench/scene.rs::m47_lifecycle_attempt_and_accepted_identity_never_leak_attempt_into_scene`. |
| `explicit_arc_branch_reference_measurement_and_imported_labels_render_truthfully` | Arc sweep and reference dimension are explicit persistent state. | **existing** `m19.rs::circle_limit_arc_trim_makes_directed_orientation_observable`; `m38.rs::signed_coordinate_spacing_angle_and_arc_dimensions_share_reference_values`. |
| `deleting_a_contact_constraint_removes_its_owned_hidden_state` | Contact-owned latent state cascades on constraint deletion. | **existing** `m13.rs::public_contact_creation_and_owned_state_deletion_keep_browser_logic_domain_free`. |
| `paired_contacts_keep_independent_neighborhoods_and_touch_selection` | Paired contacts retain independent neighborhood/branch state. | **existing** `m14.rs::a3_a4_contacts_retain_explicit_state_and_reject_branch_escape`. |
| `curved_profile_edges_have_adaptive_interior_points_in_directed_order` | Certified profile edges retain directed source parameters and endpoint geometry. | **existing** `m49_advanced_geometry.rs::accepted_nurbs_edit_refreshes_profile_roots_and_directed_edges_keep_parameters`; private adaptive renderer subdivision/interior-point policy is retired because topology is certified by profile analysis, not sampling. |
| `reverse_directed_profile_parameters_are_not_reordered` | Profile parameters retain reverse directed order. | **existing** `m49_advanced_geometry.rs::accepted_nurbs_edit_refreshes_profile_roots_and_directed_edges_keep_parameters`. |
| `s3_action_switches_explicit_modes_on_the_positive_branch_transactionally` | Circle tangency mode and direction branch switch atomically. | **existing** `m7.rs::s3_external_and_internal_modes_are_explicit_transactional_and_scale_invariant`. |
| `arc_drag_updates_committed_span_and_rejects_escape_without_republishing` | Bounded arc contact commits span and rejects escape without publication. | **existing** `m7.rs::bounded_line_and_arc_contacts_accept_interior_and_endpoints_but_reject_escape`. |
| `auto_radius_scene_starts_accepted_with_two_dof_and_no_circle_driver` | Circle-arc auto-radius has solved radius/contact and two DOF without a circle driver. | **existing** `m7_circle_arc_tangency.rs::perturbed_radius_and_contacts_recover_without_a_circle_radius_equation`. |
| `auto_radius_two_dimensional_drags_solve_distinct_radii_contacts_and_release` | Temporary center drives solve radius/contact and release preserves accepted state. | **existing** `m7_circle_arc_tangency.rs::temporary_center_drags_solve_radius_and_contacts_then_release_with_two_dof`. |
| `auto_radius_invalid_span_side_and_zero_radius_requests_retain_all_published_state` | Invalid circle-arc requests are typed and retain accepted state. | **existing** `m10.rs::circle_arc_branch_failure_is_transactional_through_session`; `m7_circle_arc_tangency.rs::tiny_radius_tangent_row_is_dimensionless_and_zero_arc_derivative_is_invalid`. |
| `tangent_glide_updates_contacts_and_rejects_supporting_line_escape` | Line-circle contact state is projected and bounded transactionally. | **existing** `m7.rs::line_circle_tangency_preserves_domain_side_and_contact_transactionally`. |
| `rejection_wording_uses_typed_classification_in_banner_and_curve_hud` | Rejections retain typed classification (not legacy wording). | **existing** `m10.rs::branch_and_secondary_failures_retain_the_accepted_session_revision`; `m7.rs::zero_segments_bad_branches_nonfinite_state_and_stale_curve_ids_are_explicit`. |
| `ambiguous_auto_radius_scale_has_truthful_typed_retention_ui` | Ambiguous tangency scale is typed and accepted state is retained. | **existing** `m7_circle_arc_tangency.rs::tiny_radius_tangent_row_is_dimensionless_and_zero_arc_derivative_is_invalid`; `m10.rs::branch_and_secondary_failures_retain_the_accepted_session_revision`. |
| `rebuilding_an_m7_scene_resets_geometry_branch_and_contact_state` | Fresh fixture construction has canonical explicit branch/contact state. | **existing** `m7.rs::s3_external_and_internal_modes_are_explicit_transactional_and_scale_invariant`, `::bounded_line_and_arc_contacts_accept_interior_and_endpoints_but_reject_escape`. |
| `s2_initializes_from_expected_rejection_with_only_typed_width_conflicts` | Conflict source mapping is typed; no rejected solve is convergence. | **existing** `m11.rs::conflicting_command_retains_document_and_maps_both_persistent_sources`. |
| `horizontal_rail_drag_projects_to_one_dof_and_release_preserves_position` | Projected drag and release preserve accepted one-DOF state. | **existing** `m14.rs::a1_dimension_edits_and_a2_projected_drag_match_the_canonical_workflows`. |
| `coincident_pair_drag_moves_both_points_and_release_preserves_common_position` | Coincidence projects paired geometry and commits/release retains it. | **existing** `m26.rs::active_coincidence_constraints_weld_independent_line_endpoints`. |
| `rejected_attempt_renders_retained_geometry_and_display_audit` | Accepted/attempt evidence remains separate. | **existing** `m11.rs::conflicting_command_retains_document_and_maps_both_persistent_sources`; `workbench/scene.rs::m47_lifecycle_attempt_and_accepted_identity_never_leak_attempt_into_scene`. |
| `api_error_keeps_display_and_diagnostics_without_a_stale_attempt_report` | Focused presentation must not publish stale attempt diagnostics. | **existing** `workbench/panels.rs::tests::m47_parameter_batch_proposal_stamps_are_atomic_and_recover` now proves a pre-attempt API error leaves the prior attempt identity/report availability unchanged while accepted diagnostic provenance remains separate. |
| `retained_diagnostics_can_fall_back_to_audit_and_hide_invalid_rank` | Focused presentation distinguishes retained audit from invalid rank. | **existing** `workbench/panels.rs::tests::accepted_diagnostic_renderer_hides_invalid_rank_and_keeps_incomplete_empty_truthful`. |
| `incomplete_empty_diagnostics_are_never_rendered_as_none` | Focused presentation represents incomplete diagnostics truthfully. | **existing** `workbench/panels.rs::tests::accepted_diagnostic_renderer_hides_invalid_rank_and_keeps_incomplete_empty_truthful`. |
| `radius_cue_uses_the_public_distance_dimension_target` | Radius cue derives from the public dimension target. | **existing** `m7_circle_arc_tangency.rs::perturbed_radius_and_contacts_recover_without_a_circle_radius_equation`. |
| `l1_l2_l3_states_start_accepted_with_explicit_branches_and_valid_velocity` | L1/L2/L3 accepted assemblies retain branch and validated velocity. | **existing** `crates/geosolve-linkage/tests/m6.rs::l1_l2_initial_solutions_and_safe_sweeps_preserve_opposite_assembly_signs`, `::l3_initial_and_full_safe_sweep_validate_revolute_guide_and_positive_x_branch`, `::l1_l2_l3_velocities_match_central_position_continuation_oracles`. |
| `linkage_state_drives_low_mid_high_with_bounded_validated_continuation` | Linkage continuation is bounded and independently validated. | **existing** `m6.rs::linear_driver_continuation_and_velocity_are_physical_and_validated`. |
| `exact_toggle_failure_keeps_retained_linkage_display_and_diagnostics` | Ambiguous branch toggle rolls back accepted linkage state. | **existing** `m6.rs::known_near_toggle_warns_and_exact_toggle_rolls_back_on_branch_ambiguity`. |
| `accepted_position_with_forced_velocity_failure_rolls_back_atomically` | Failed velocity continuation retains accepted position/evidence. | **existing** `m6.rs::failed_continuation_retains_geometry_target_and_display_audit`. |

#### C — exact native duplicates (3)

| Legacy duplicate | Exact cited native owner | Disposition |
| --- | --- | --- |
| `advanced_constraint_stress_examples_render_valid_public_documents` | `crates/geosolve-sketch/tests/m14.rs::compass_stress_example_exposes_and_locks_rotational_mobility`, `::bridge_stress_example_exposes_mobility_and_rejects_degeneracy`, `::cam_motion_projects_one_roller_while_stabilizing_the_other`, `::tangent_orbit_projected_drag_traverses_all_quadrants`, `::compound_constraint_mechanisms_follow_their_emergent_motion`, `::advanced_linkage_examples_propagate_one_driver_through_every_bar`, `::advanced_diagnostic_examples_expose_rank_bounds_and_redundancy` | Duplicate domain/rank behavior; retire labels/SVG. |
| `a5_line_endpoint_drag_stabilizes_the_opposite_bezier_handle` | `m14.rs::a5_and_a8_round_trip_preserve_geometry_ids_and_branches` | Duplicate A5 stability/branch contract; retire pointer/SVG delivery. |
| `endpoint_tangency_and_persisted_branch_edits_use_explicit_state` | `m14.rs::a3_a4_contacts_retain_explicit_state_and_reject_branch_escape` | Duplicate explicit contact branch-state contract; retire UI delivery. |

#### B/D — reviewed legacy-only retirement (33)

Retirement codes: **B1** viewport/coordinate/hit/gesture adapter; **B2** browser storage,
reload, file or retry delivery; **B3** SVG/HTML/CSS/accessibility/diagnostic presentation;
**D1** legacy selector/page/control inventory.  None asserts a durable domain capability.

| Test | Retirement |
| --- | --- |
| `viewport_transform_zoom_and_hit_geometry_round_trip` | B1: legacy finite-grid/SVG coordinate and hit adapter. |
| `alpha_scale_extremes_fit_inside_the_editable_canvas` | B1: canvas fit presentation; domain scale is M14. |
| `conic_previews_use_clone_only_persistent_sampling_and_omit_invalid_candidates` | B3: draft preview/render omission; persistent document remains unchanged by design. |
| `autosave_payload_retries_until_browser_confirms_storage` | B2: browser storage acknowledgement retry. |
| `visual_profile_overlay_is_read_only_and_has_no_interaction_identity` | B3: overlay interaction/render adapter. |
| `nested_profile_holes_share_one_even_odd_overlay_path` | B3: SVG composition. |
| `native_budget_scene_never_gains_a_web_overlay` | B3: web-overlay absence. |
| `web_budget_failure_omits_whole_face_without_changing_native_status` | B3: renderer budget policy. |
| `sampled_profile_gap_omits_whole_face_instead_of_drawing_connector` | B3: renderer safety presentation. |
| `box_selection_and_pan_gestures_are_web_only_and_deterministic` | B1: web gesture delivery. |
| `live_s1_view_comes_from_an_accepted_sketch_result` | B3: old live-demo markup/audit formatting. |
| `s1_has_no_static_audit_or_handwritten_equation_templates` | B3: source-text scan is retired. |
| `auto_radius_svg_title_uses_rank_valid_report_mobility` | B3: SVG title formatting. |
| `generic_scene_action_is_visible_only_for_s3_and_uses_a_native_button` | B3: old button/HTML behavior. |
| `s2_render_uses_retained_geometry_display_audit_and_expected_conflict_status` | B3: legacy scene markup. |
| `every_live_scene_renders_only_its_evaluated_display_audit_rows` | B3: legacy SVG/audit rendering. |
| `model_svg_and_client_view_box_transforms_round_trip` | B1: legacy viewport transform. |
| `m7_arc_and_tangent_client_model_transforms_follow_the_responsive_viewport` | B1: responsive viewport adapter. |
| `auto_radius_mobile_transform_and_center_hit_target_remain_usable` | B1: mobile hit target. |
| `arc_ccw_240_svg_path_has_exact_large_arc_and_screen_sweep_flags` | B3: SVG path encoding. |
| `auto_radius_ccw_300_svg_path_has_exact_large_arc_and_screen_sweep_flags` | B3: SVG path encoding. |
| `viewport_drag_state_and_tangent_endpoint_styles_are_explicit` | B1: drag state/CSS. |
| `outside_rail_and_coincident_drags_retain_fully_visible_handles` | B1: viewport handle layout. |
| `pointer_start_requires_one_primary_pointer_and_left_mouse_button` | B1: browser pointer policy. |
| `interaction_does_not_advertise_an_inaccessible_svg_button` | B3: legacy accessibility markup/delivery. |
| `viewport_css_preserves_exact_ratio_and_hit_target_is_large_enough_when_narrow` | B1: CSS layout. |
| `linkage_rendering_uses_display_geometry_audit_and_driver_source_identity` | B3: old linkage SVG/audit rendering. |
| `linkage_degree_controls_and_scene_transforms_are_pure_and_accessible` | B1: control/transform/accessibility delivery. |
| `dynamic_audit_strings_are_html_escaped` | B3: old HTML renderer; no legacy renderer survives. |
| `public_domain_example_selector_keys_are_visible` | D1: selector/HTML inventory. |
| `conic_tool_ui_is_complete_and_spatially_hidden` | D1: tool palette/CSS inventory. |
| `page_exposes_document_tools_mobile_input_and_accepted_diagnostics` | D1: legacy page/mobile control inventory. |
| `all_eleven_selectors_and_names_map_to_fresh_domain_scene_kinds` | D1: old demo selector inventory. |

#### E — final M46 direct-owner/retirement disposition (16)

| Test | Final disposition |
| --- | --- |
| `scene_capsule_codec_and_profile_options_round_trip_deterministically` | Retire private capsule/codec/profile glue; canonical document is `m14.rs::a5_and_a8_round_trip_preserve_geometry_ids_and_branches`, typed capture is `workbench/evidence.rs::typed_host_capture_contains_inputs_attempt_and_accepted_evidence`. |
| `malformed_scene_capsules_retain_the_accepted_document_atomically` | Retire capsule import; canonical malformed-import retention is `m11.rs::malformed_imports_and_invalid_edits_leave_session_exactly_unchanged`. |
| `scene_capsule_status_is_non_authoritative` | Retire private non-authoritative capsule status; canonical accepted state remains sketch-owned. |
| `scene_capsule_decodes_exact_external_evidence_without_publishing_it` | Retire capsule transport; external evidence is `m43.rs::snapshot_set_is_canonical_strict_and_exactly_stamped` and workbench typed capture. |
| `m20_spatial_examples_render_accepted_features_and_physical_reports_at_all_scales` | Retain spatial solver/rank/scale/report semantics in `m20.rs::m20_joints_report_exact_floating_and_grounded_rows_rank_and_mobility`, `::shaft_bearing_driver_stage_matrix_reports_internal_two_one_one_zero`, `::block_base_three_target_transaction_commits_once_and_failures_roll_back_all_state`, and **existing** `crates/geosolve-linkage/tests/m49_legacy_consumer.rs` for the two legacy example signatures. Retire browser rendering. |
| `spatial_mode_rejects_hidden_sketch_edits_and_has_no_storage_payload` | Retain read-only spatial transaction isolation in `m20.rs::block_base_three_target_transaction_commits_once_and_failures_roll_back_all_state` and **existing** `m49_legacy_consumer.rs`. Retire mode/storage UI. |
| `every_alpha_draw_tool_creates_one_atomic_history_entry` | Retain transaction/history in `m13.rs::compound_geometry_transaction_solves_commits_and_undoes_once`; the uncovered multi-family lifecycle is **existing** `m49_advanced_geometry.rs::five_conic_lifecycle_and_legacy_signatures_preserve_typed_state_and_validation`. Retire palette UI. |
| `every_draw_tool_has_a_staged_primitive_preview` | Retain progression/preview lifecycle in `geosolve-constraint-editor/src/lib.rs::every_core_draft_has_exact_completion_and_cancellation` and `workbench/effect_adapter.rs::tests::m49_editor_cancel_and_invalid_completion_only_clear_or_retain_staged_preview`. Retire SVG preview. |
| `pointer_cancel_and_invalid_completion_retain_the_staged_draft` | Retain completion/cancel semantics in the same editor and M49 effect-adapter tests. Retire pointer wiring. |
| `deleting_each_new_shape_removes_its_generated_controls` | Retain dependency cleanup in `m11.rs::batch_delete_cascades_from_selected_rectangle_geometry`; uncovered conic-family owned-scalar cleanup/history is **existing** `m49_advanced_geometry.rs::five_conic_lifecycle_and_legacy_signatures_preserve_typed_state_and_validation`. Retire generated-control presentation. |
| `selection_constraints_dimensions_drag_history_and_json_use_document_session` | Retain selection/effect policy in `constraint-editor/src/lib.rs::ordered_mixed_selection_replaces_extends_and_toggles_by_persistent_identity`, `coordinator.rs::suppression_delete_and_selection_reconciliation_use_persistent_ids`, and history/persistence in `m11.rs::accepted_only_history_round_trips_create_edit_suppress_and_delete`. Retire lab flow. |
| `drawn_rectangle_has_free_size_and_full_geometry_delete_cascades` | Retain rectangle/history/delete in `m11.rs::rectangle_macro_expands_to_ordinary_geometry_and_solves`, `::batch_delete_cascades_from_selected_rectangle_geometry`; retire draw/drag UI. |
| `inference_is_provisional_until_confirmed` | **existing** `constraint-editor/src/coordinator.rs::staged_inference_is_non_authoritative_until_its_commit_effect_is_applied`; retire pointer/DOM presentation. |
| `click_without_motion_preserves_history_and_polyline_spans_multiselect` | Retain no-op/history in `m11.rs::solver_projected_no_op_point_edit_preserves_history_and_redo` and ordered span selection in `constraint-editor/src/lib.rs::ordered_mixed_selection_replaces_extends_and_toggles_by_persistent_identity`. Retire pointer/modifier delivery. |
| `all_constraint_buttons_create_their_public_document_definition` | Retain applicability/execution in `m37.rs::m37_high_level_contact_and_tangent_constructors_allocate_explicit_latents_atomically`, `constraint-editor/src/lib.rs::relation_applicability_matrix_builds_only_valid_public_edits`, and `coordinator.rs::relation_availability_and_edit_building_are_prospective_until_one_coordinator_apply`. Retire full palette controls. |
| `every_dimension_kind_supports_reference_display_and_driving_edit` | Retain dimension execution in `m38.rs::driving_coordinate_dimensions_lower_into_the_solver_and_commit_atomically`, `::driving_arc_conic_and_path_dimensions_publish_solver_owned_audit`, and coordinator dimension matrix/route tests. Retire full palette/display. |

### Decisions / inferred constraints

- All three C tests are confirmed against the exact native tests above; no duplicate direct
  test is needed. Every A retained claim has a passing direct owner. Every B/D claim and the
  legacy-only half of every E claim has a reviewed retirement.
- The sketch/profile, linkage, editor, persistence and focused presentation targets now pass.
  The deduplicated genuinely missing direct slices are **none**.

### Open questions

None: M46 fixed the product-scope dispositions and every retained target now has a direct owner.

### Out of scope

Deleting legacy tests/runtime, browser/CDP execution, and any legacy layout, DOM, CSS, mobile,
storage, file-picker, download, timing or selector behavior.

**Counts:** A **40**, B **29**, C **3**, D **4**, E **16** = **92**; `playground.rs` **51**
(19 A, 10 B, 3 C, 3 D, 16 E) and legacy `lib.rs` **41** (21 A, 19 B, 1 D). **Unowned
claims: 0.**

## Direct implementation slices

The parent integrated both ledgers and the exact gap lookups into four disjoint slices. New
tests use M49 names; the pre-implementation `m46_*` proposed filenames in the frozen M46/M45
inventories are evidence pointers, not current milestone numbers.

| Slice | Exclusive implementation scope | Required direct contracts | Acceptance command |
| --- | --- | --- | --- |
| Sketch advanced/profile extraction | New `crates/geosolve-sketch/tests/m49_advanced_geometry.rs` only | accepted NURBS control edit refreshes certified self-root evidence; directed curved-profile source parameters and reverse order; exact public straight intervals, one-period full ellipse support, every public B-spline span, and failed-NURBS no-fabrication; combined five-family conic lifecycle and retained planar legacy signatures. Private adaptive renderer subdivision/count is retired. | `nix-shell shell.nix --run 'cargo test --locked -p geosolve-sketch --test m49_advanced_geometry'` |
| Linkage legacy-consumer extraction | New `crates/geosolve-linkage/tests/m49_legacy_consumer.rs` only | shaft-bearing and block-base scale/rank/report/continuation/rollback signatures with no browser consumer or storage/rendering dependency | `nix-shell shell.nix --run 'cargo test --locked -p geosolve-linkage --test m49_legacy_consumer'` |
| Editor interaction extraction | `crates/geosolve-constraint-editor/src/lib.rs` and `src/coordinator.rs` tests only | provisional inference remains non-persistent until explicit confirmation; ordered persistent selection toggle/multiselect; constraint applicability matrix; invalid completion/cancellation emits no accepted mutation | `nix-shell shell.nix --run 'cargo test --locked -p geosolve-constraint-editor'` |
| Workbench codec/presentation extraction | `crates/geosolve-demo-web/src/workbench/{effect_adapter,panels,persistence,scene}.rs` | accepted explicit-contact state survives the exact production snapshot codec; native history/high-water remains separately owned; failed/incomplete diagnostic presentation remains distinct from accepted state, falls back to accepted audit, and never invents rank; adapter cancellation preserves or clears preview according to typed effects | `nix-shell shell.nix --run 'cargo test --locked -p geosolve-demo-web --all-features workbench::'` |

The sketch and linkage slices are independent new integration-test files and may run in
parallel. The editor and workbench slices are a second wave because their exact DTO/effect
assertions must be reconciled against the domain fixtures produced by the first wave. No slice
may edit or invoke `playground.rs`, legacy `lib.rs` tests, `e2e/m14.mjs`, HTML/CSS, routes,
serving scripts, or release-gate browser commands.

First-wave verification record (2026-07-28): the sketch command passes 3 tests and the linkage
command passes 2 tests. Independent review passes after confirming public line/polyline
endpoint intervals without a self-authored sampler, accepted and independently validated
five-family deletion/undo/redo, ordinary validation of every continuation sample, and the
all-scale block/base branch/monitor/rollback matrix. The block/base fixture publicly exposes
parity, winding and plane-side monitors but no ordered-volume monitor ID, so no private monitor
is claimed.

Second-wave verification record (2026-07-28): the editor command passes 58 tests plus doc-tests.
The typed `ProvisionalInferenceCandidate` remains editor-only state through stage, cancellation
and confirmation and mutates retained design/history only when its explicit commit effect is
submitted to the coordinator; stale candidates are rejected without mutation. The focused
workbench command passes 23 tests. Existing host-state markup now keeps latest-attempt identity
separate from accepted diagnostics, falls back to finite accepted audit residuals, suppresses
invalid rank and distinguishes complete-empty from incomplete-empty diagnostics. The production
snapshot codec round-trips accepted A4 contact state; it does not claim to encode command history
or cursor. Formatting, diff, workspace, Clippy, WASM and Trunk gates pass. Independent review
passes and confirms the M49/M50 deletion boundary.

## Validation plan

The final direct commands include focused sketch/linkage/editor/persistence owners,
the complete locked all-feature workspace suite, formatting, diff, warnings-denied Clippy,
the all-feature WASM check, and the relevant release Trunk build. No browser E2E is an M49
qualification command.

## Completion record

Complete (2026-07-28).

1. **Files/APIs added:** direct M49 sketch and linkage integration suites; typed
   `ProvisionalInferenceCandidate` staging, clear and commit effects in
   `geosolve-constraint-editor`; production inference-effect dispatch and direct codec,
   diagnostic and presentation owners in `geosolve-demo-web::workbench`.
2. **Mathematical behavior:** accepted NURBS/profile/conic behavior and directed source
   parameters, five-family lifecycle validation, linkage rank/branch/continuation/rollback,
   explicit inference confirmation, accepted-only diagnostic fallback and explicit-contact
   snapshot semantics are preserved without private renderer policy or legacy controls.
3. **Commands and outcomes:** `cargo fmt --all -- --check`, `git diff --check`,
   `cargo test --locked --workspace --all-features`,
   `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`,
   `cargo check --locked -p geosolve-demo-web --all-features --target wasm32-unknown-unknown`
   and the release `trunk build --release` all pass through `nix-shell`; no browser E2E ran.
4. **Acceptance:** 13/13 M14 browser groups and 92/92 inline tests are classified with zero
   unowned claims; all retained claims have direct owners and all retired claims have reviewed
   rationales. Independent read-only verification passes.
5. **Resolved follow-up from M49 completion:** the old M14 E2E, playground, route, CSS,
   serving and release-gate infrastructure intentionally remained for M50. M50 subsequently
   removed that slice after its direct gates and independent review passed.
