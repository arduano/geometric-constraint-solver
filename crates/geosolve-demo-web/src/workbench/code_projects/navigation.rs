// SPDX-License-Identifier: GPL-3.0-or-later

//! Read-only ownership between accepted managed statements and native nodes.
//! This index is independent of scalar edit lenses and mutation authority.

use super::*;

#[cfg(test)]
use geosolve_sketch_code::ManagedNavigationEntry;
pub(crate) use geosolve_sketch_code::ManagedNavigationIndex;

impl CodeProjectWorkbench {
    /// Builds a disposable projection from accepted provenance only. In
    /// particular, a retained failed source may have different statement
    /// offsets and must never supply navigation for the retained canvas.
    #[allow(
        clippy::too_many_lines,
        reason = "accepted source, declaration provenance and exact generated ownership form one coherent projection"
    )]
    pub(crate) fn navigation_index(
        &self,
        editor: &ProjectionalEditorSession,
    ) -> ManagedNavigationIndex {
        let snapshot = self.session.source_session().snapshot();
        let managed = snapshot
            .accepted_code_project
            .as_ref()
            .map_or(&snapshot.managed, |project| &project.managed);
        geosolve_sketch_code::managed_browsing_navigation_index(
            snapshot,
            editor,
            Some(
                self.session
                    .accepted_materialization()
                    .editor
                    .coordinator()
                    .intent()
                    .identity(),
            ),
            self.is_dirty(),
            self.managed_draft != managed.source,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry<'a>(index: &'a ManagedNavigationIndex, id: &str) -> &'a ManagedNavigationEntry {
        index.entries.iter().find(|entry| entry.id == id).unwrap()
    }

    fn compiled_fixture(bytes: &str) -> CompiledManagedSource {
        CompiledManagedSource::from_json(bytes).expect("checked compiler fixture")
    }

    #[test]
    fn navigation_index_preserves_complete_declarations_and_exact_host_children() {
        let (workbench, editor) = CodeProjectWorkbench::open_key("typed-panel").unwrap();
        let session_before = workbench
            .session
            .source_session()
            .to_canonical_json()
            .unwrap();
        let intent_before = editor.coordinator().intent().identity();
        let index = workbench.navigation_index(&editor);
        assert!(index.blocked_reason.is_none());
        let panel = entry(&index, "managed:panel");
        let fillets = entry(&index, "managed:cornerFillets");
        let expansion = workbench
            .session
            .source_session()
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let graph = editor.coordinator().intent().graph();
        let expected_panel = expansion
            .declaration_provenance
            .iter()
            .filter(|(_, owner)| owner.0 == "panel")
            .filter_map(|(alias, _)| graph.node_by_symbol(alias).map(|node| node.id))
            .collect::<BTreeSet<_>>();
        assert!(
            editor
                .navigation_selection_items(expected_panel.iter().copied())
                .len()
                > 1,
            "polyline navigation owns every point and span"
        );
        assert_eq!(
            panel.nodes.iter().copied().collect::<BTreeSet<_>>(),
            expected_panel
        );
        let mut expected_fillets = BTreeSet::new();
        for child in &expansion.generated_children {
            let CodeOwnerAddress::GeneratedMember { address } = &child.address.owner.address else {
                continue;
            };
            if address.invocation != "cornerFillets" {
                continue;
            }
            let node = graph.node_by_symbol(&child.alias).unwrap().id;
            expected_fillets.insert(node);
            let row = entry(&index, &generated_panel_row_id(address));
            assert_eq!(row.nodes, vec![node]);
            assert_eq!(
                row.exact_bindings.as_deref(),
                Some(
                    editor
                        .coordinator()
                        .accepted_materialization()
                        .unwrap()
                        .ownership
                        .node(node)
                        .unwrap()
                        .owned
                        .as_slice()
                ),
            );
            assert_eq!(
                (row.source_start, row.source_end),
                (fillets.source_start, fillets.source_end)
            );
            assert!(
                !expected_panel.contains(&node),
                "Fillet selection cannot select its source corner"
            );
        }
        assert_eq!(expected_fillets.len(), 2);
        assert!(expected_fillets.is_subset(&fillets.nodes.iter().copied().collect()));
        assert!(
            index.source[fillets.source_start..fillets.source_end]
                .contains("$.use(\"cornerFillets\"")
        );
        assert_eq!(
            workbench
                .session
                .source_session()
                .to_canonical_json()
                .unwrap(),
            session_before
        );
        assert_eq!(editor.coordinator().intent().identity(), intent_before);
    }

    #[test]
    fn navigation_channel_wall_members_include_native_spans_and_computed_bends() {
        let (workbench, editor) = CodeProjectWorkbench::open_key("pc-water-manifold").unwrap();
        assert_channel_wall_navigation(
            &workbench,
            &editor,
            &[
                ("upperChannel", 3),
                ("middleChannel", 3),
                ("lowerChannel", 3),
                ("stairChannel", 3),
                ("commonSealGroove", 4),
            ],
        );
    }

    #[test]
    fn navigation_open_channel_keeps_native_wall_spans_alongside_generated_fillets() {
        let (workbench, editor) =
            CodeProjectWorkbench::import_canonical_project_json(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../geosolve-sketch-code/tests/fixtures/m96/channel-zigzag.project.json"
            )))
            .unwrap();
        assert_channel_wall_navigation(&workbench, &editor, &[("wet", 3)]);
    }

    fn assert_channel_wall_navigation(
        workbench: &CodeProjectWorkbench,
        editor: &ProjectionalEditorSession,
        invocations: &[(&str, usize)],
    ) {
        let history = workbench.session.source_session();
        let before = history.to_canonical_json().unwrap();
        let index = workbench.navigation_index(editor);
        assert!(index.blocked_reason.is_none());
        let expansion = history.snapshot().accepted_expansion.as_ref().unwrap();
        let graph = editor.coordinator().intent().graph();
        let accepted = editor.coordinator().accepted_materialization().unwrap();
        let mut walls = 0;
        for (address, provenance) in &expansion.generated_provenance {
            let Some((_, span_count)) = invocations
                .iter()
                .find(|(name, _)| *name == address.invocation)
            else {
                continue;
            };
            let ExpandedSemanticTarget::Declaration { alias, .. } = &provenance.target else {
                continue;
            };
            let node = graph.node_by_symbol(alias).unwrap();
            let aggregates = accepted
                .ownership
                .aggregates
                .iter()
                .filter(|aggregate| aggregate.port.node == node.id)
                .collect::<Vec<_>>();
            if aggregates.is_empty() {
                continue;
            }
            assert_eq!(
                aggregates.len(),
                1,
                "one wall aggregate per generated member"
            );
            let spans = &aggregates[0].spans;
            assert_eq!(spans.len(), *span_count);
            let row = entry(&index, &generated_panel_row_id(address));
            let bindings = row
                .exact_bindings
                .as_ref()
                .unwrap()
                .iter()
                .copied()
                .collect::<BTreeSet<_>>();
            let mut expected = BTreeSet::new();
            for span in spans {
                let owner = accepted
                    .ownership
                    .exact_owner(IntentNativeBinding::Curve(span.curve))
                    .unwrap();
                assert_eq!(
                    expansion.declaration_provenance[&graph.node(owner).unwrap().symbol].0,
                    address.invocation
                );
                assert!(
                    row.nodes.contains(&owner),
                    "wall member retains its native straight-span owner"
                );
                expected.insert(IntentNativeBinding::CurveSpan(*span));
            }
            let mut bend_nodes = 0;
            for child in &expansion.generated_children {
                let CodeOwnerAddress::GeneratedMember {
                    address: child_address,
                } = &child.address.owner.address
                else {
                    continue;
                };
                if child_address != address {
                    continue;
                }
                let node = graph.node_by_symbol(&child.alias).unwrap().id;
                assert!(
                    row.nodes.contains(&node),
                    "wall member retains its computed bend owner"
                );
                expected.extend(accepted.ownership.node(node).unwrap().owned.iter().copied());
                bend_nodes += 1;
            }
            assert_eq!(bend_nodes, if *span_count == 4 { 4 } else { 2 });
            assert_eq!(
                bindings, expected,
                "wall selection is exactly its straight spans and bends"
            );
            let declaration = entry(&index, &format!("managed:{}", address.invocation));
            assert_eq!(
                (row.source_start, row.source_end),
                (declaration.source_start, declaration.source_end)
            );
            walls += 1;
        }
        assert_eq!(
            walls,
            invocations.len() * 2,
            "both walls of every requested passage and seal"
        );
        assert_eq!(history.to_canonical_json().unwrap(), before);
    }

    #[test]
    fn navigation_index_generated_polyline_members_keep_exact_point_and_span_bindings() {
        let compiled = compiled_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-polyline.json"
        )));
        let (workbench, editor) = CodeProjectWorkbench::open_managed_test_compiled(
            "m95-polyline-member-navigation",
            compiled,
        )
        .unwrap();
        let index = workbench.navigation_index(&editor);
        let declaration = entry(&index, "managed:geometry1");
        assert!(declaration.exact_bindings.is_none());
        assert_eq!(
            declaration.nodes.len(),
            1,
            "all members share one Polyline node"
        );
        let expansion = workbench
            .session
            .source_session()
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap();
        let graph = editor.coordinator().intent().graph();
        let ownership = &editor
            .coordinator()
            .accepted_materialization()
            .unwrap()
            .ownership;
        let mut exact_points = BTreeSet::new();
        let mut exact_spans = BTreeSet::new();
        for (address, provenance) in &expansion.generated_provenance {
            if address.invocation != "geometry1" {
                continue;
            }
            let ExpandedSemanticTarget::Port { port } = &provenance.target else {
                continue;
            };
            let node = graph.node_by_symbol(&port.alias).unwrap();
            let expected = ownership
                .port(
                    node.port_by_selector(port.selector)
                        .unwrap()
                        .as_ref(node.id),
                )
                .unwrap();
            let row = entry(&index, &generated_panel_row_id(address));
            assert_eq!(row.nodes, declaration.nodes);
            assert_eq!(row.exact_bindings, Some(vec![expected]));
            match expected {
                IntentNativeBinding::Point(point) => {
                    exact_points.insert(point);
                }
                IntentNativeBinding::CurveSpan(span) => {
                    exact_spans.insert(span);
                }
                other => panic!("unexpected Polyline member binding {other:?}"),
            }
        }
        assert_eq!(exact_points.len(), 3);
        assert_eq!(exact_spans.len(), 2);
        assert_eq!(
            editor
                .navigation_selection_items(declaration.nodes.iter().copied())
                .len(),
            5
        );
    }

    #[test]
    fn navigation_index_preserves_helper_rows_and_accepted_source_under_dirty_text() {
        let compiled = compiled_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-base.json"
        )));
        let (mut workbench, editor) =
            CodeProjectWorkbench::open_managed_test_compiled("m95-helper-navigation", compiled)
                .unwrap();
        let before = workbench.navigation_index(&editor);
        let helper = entry(&before, "managed:offsetChain11");
        let root = entry(&before, "managed:profileOffset12");
        assert!(!helper.nodes.is_empty());
        assert!(!root.nodes.is_empty());
        assert!(
            before.source[helper.source_start..helper.source_end].contains("$.aggregate.openChain")
        );
        assert!(
            before.source[root.source_start..root.source_end].contains("$.operation.profileOffset")
        );
        let panel = workbench.declaration_panel_projection(&editor);
        let root_row = panel
            .declarations
            .iter()
            .find(|row| row.id == root.id)
            .unwrap();
        assert_eq!(root_row.closure_helpers[0].id, helper.id);
        workbench.set_managed_draft(format!(
            "// λ multibyte draft shifts every span\n{}",
            before.source
        ));
        let dirty = workbench.navigation_index(&editor);
        assert!(dirty.blocked_reason.is_some());
        assert_eq!(dirty.source, before.source);
        assert_eq!(dirty.source_digest, before.source_digest);
        assert_eq!(dirty.entries, before.entries);
        assert!(workbench.revert_managed_draft());
        assert_eq!(workbench.navigation_index(&editor), before);
    }

    #[test]
    fn navigation_index_keeps_suppressed_source_owners_without_selecting_parent_geometry() {
        let compiled = compiled_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-lifecycle-both-suppressed.json"
        )));
        let (workbench, editor) =
            CodeProjectWorkbench::open_managed_test_compiled_with_sample_pins(
                "m95-suppressed-navigation",
                "typed-panel",
                compiled,
            )
            .unwrap();
        let index = workbench.navigation_index(&editor);
        assert!(index.blocked_reason.is_none());
        let guide = entry(&index, "managed:guide");
        assert!(index.source[guide.source_start..guide.source_end].contains("geometry.segment"));
        assert!(
            editor
                .navigation_selection_items(guide.nodes.iter().copied())
                .is_empty()
        );
        let child = workbench
            .session
            .source_session()
            .snapshot()
            .accepted_expansion
            .as_ref()
            .unwrap()
            .generated_children
            .iter()
            .find(|child| child.suppressed)
            .unwrap();
        let CodeOwnerAddress::GeneratedMember { address } = &child.address.owner.address else {
            panic!("the fixture suppresses one generated Fillet member")
        };
        let row = entry(&index, &generated_panel_row_id(address));
        assert!(
            editor
                .navigation_selection_items(row.nodes.iter().copied())
                .is_empty()
        );
        assert!(index.source[row.source_start..row.source_end].contains("$.use(\"cornerFillets\""));
    }

    #[test]
    fn navigation_index_and_explorer_retain_accepted_spans_after_failed_authority() {
        let base = compiled_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-infeasible-base.json"
        )));
        let candidate = compiled_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-contact-range-infeasible-limited.json"
        )));
        let (mut workbench, editor) =
            CodeProjectWorkbench::open_managed_test_compiled("m95-failed-navigation", base)
                .unwrap();
        let before = workbench.navigation_index(&editor);
        let panel_before = workbench.declaration_panel_projection(&editor);
        let candidate_project =
            CodeProject::managed(workbench.project.project.clone(), candidate).unwrap();
        assert!(matches!(
            workbench
                .apply_candidate_project(candidate_project)
                .unwrap(),
            CodeApplyOutcome::RetainedFailure { .. }
        ));
        assert!(
            !workbench.is_dirty(),
            "retained failed source is a distinct authority state"
        );
        assert_ne!(workbench.managed_source(), before.source);
        let retained = workbench.navigation_index(&editor);
        assert!(retained.blocked_reason.is_some());
        assert_eq!(retained.source, before.source);
        assert_eq!(retained.entries, before.entries);
        let panel_after = workbench.declaration_panel_projection(&editor);
        assert_eq!(panel_after.source_digest, panel_before.source_digest);
        let navigation_rows = |panel: &ManagedDeclarationPanelProjection| {
            panel
                .declarations
                .iter()
                .map(|row| {
                    (
                        row.id.clone(),
                        row.selection_node,
                        row.source_start,
                        row.source_end,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            navigation_rows(&panel_after),
            navigation_rows(&panel_before)
        );
        let wire = workbench.to_persistence_json().unwrap();
        let restored = CodeProjectWorkbench::from_persistence_json(&wire).unwrap();
        let restored_editor = restored.restore_accepted_editor().unwrap();
        assert_eq!(restored.navigation_index(&restored_editor), retained);
        assert_eq!(restored.to_persistence_json().unwrap(), wire);
        let undo = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.navigation_index(&undo.editor), before);
        let redo = workbench.step_history(false).unwrap().unwrap();
        assert_eq!(workbench.navigation_index(&redo.editor), retained);
    }

    #[test]
    fn navigation_index_tracks_deletion_undo_and_project_replacement_without_stale_nodes() {
        let base = compiled_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-base.json"
        )));
        let deleted = compiled_fixture(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../packages/geosolve-sketch-code/test/fixtures/managed-profile-offset-closure-deleted.json"
        )));
        let (mut workbench, editor) =
            CodeProjectWorkbench::open_managed_test_compiled("m95-navigation-lifecycle", base)
                .unwrap();
        let before = workbench.navigation_index(&editor);
        let candidate = CodeProject::managed(workbench.project.project.clone(), deleted).unwrap();
        let CodeApplyOutcome::Accepted(publication) =
            workbench.apply_candidate_project(candidate).unwrap()
        else {
            panic!("the reviewed closure deletion must materialize")
        };
        let after = workbench.navigation_index(&publication.editor);
        assert!(after.blocked_reason.is_none());
        assert!(after.entries.iter().all(|row| row.id != "managed:offsetChain11" && row.id != "managed:profileOffset12"));
        assert!(
            workbench.navigation_index(&editor).entries.is_empty(),
            "old editor cannot lend identity to newly accepted source"
        );
        let undo = workbench.step_history(true).unwrap().unwrap();
        assert_eq!(workbench.navigation_index(&undo.editor), before);
        let redo = workbench.step_history(false).unwrap().unwrap();
        assert_eq!(workbench.navigation_index(&redo.editor), after);
        let (_, unrelated_editor) = CodeProjectWorkbench::open_key("typed-panel").unwrap();
        let unrelated = workbench.navigation_index(&unrelated_editor);
        assert!(unrelated.entries.is_empty());
        assert!(unrelated.blocked_reason.is_some());
    }
}
