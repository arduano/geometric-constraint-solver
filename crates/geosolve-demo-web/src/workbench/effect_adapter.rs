// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstructionPreview, DraftAuthoringInput, DraftInferenceCandidateId, DraftInferenceInput,
    EditorEffect, Modifiers, RetainedEditorCoordinator, ScreenPoint,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ClientRect {
    pub(super) left: f64,
    pub(super) top: f64,
    pub(super) width: f64,
    pub(super) height: f64,
}

/// Maps CSS client coordinates into the editor's fixed screen coordinate system.
/// Device scale deliberately does not enter this conversion.
pub(super) fn normalize_client_point(
    rect: ClientRect,
    screen_size: [f64; 2],
    client: [f64; 2],
) -> Option<ScreenPoint> {
    normalize_client_point_inner(rect, screen_size, client, true)
}

/// Preserves the pre-M74 coordinate translation for an already captured
/// pointer. Capture owns move and terminal samples even when the pointer
/// crosses an SVG letterbox band or leaves the mapped sketch plane.
pub(super) fn normalize_captured_client_point(
    rect: ClientRect,
    screen_size: [f64; 2],
    client: [f64; 2],
) -> Option<ScreenPoint> {
    normalize_client_point_inner(rect, screen_size, client, false)
}

/// Lifecycle action for a browser sample that cannot be mapped into the
/// fitted sketch plane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum UnmappedCanvasPointerAction {
    /// An uncaptured pointer entered an SVG letterbox band. Its previous hover
    /// and stationary sample no longer describe the pointer's semantic owner.
    RevokePointerContext,
    /// A captured interaction retains ownership across the fitted-plane edge.
    PreserveCapturedGesture,
}

#[must_use]
pub(super) const fn unmapped_canvas_pointer_action(
    pointer_is_captured: bool,
) -> UnmappedCanvasPointerAction {
    if pointer_is_captured {
        UnmappedCanvasPointerAction::PreserveCapturedGesture
    } else {
        UnmappedCanvasPointerAction::RevokePointerContext
    }
}

fn normalize_client_point_inner(
    rect: ClientRect,
    screen_size: [f64; 2],
    client: [f64; 2],
    reject_letterbox: bool,
) -> Option<ScreenPoint> {
    let [screen_width, screen_height] = screen_size;
    if !rect.left.is_finite()
        || !rect.top.is_finite()
        || !rect.width.is_finite()
        || !rect.height.is_finite()
        || !screen_width.is_finite()
        || !screen_height.is_finite()
        || !client.into_iter().all(f64::is_finite)
        || rect.width <= 0.0
        || rect.height <= 0.0
        || screen_width <= 0.0
        || screen_height <= 0.0
    {
        return None;
    }
    let scale = (rect.width / screen_width).min(rect.height / screen_height);
    if !scale.is_finite() || scale <= 0.0 {
        return None;
    }
    let left = rect.left + (rect.width - screen_width * scale) * 0.5;
    let top = rect.top + (rect.height - screen_height * scale) * 0.5;
    let right = left + screen_width * scale;
    let bottom = top + screen_height * scale;
    if reject_letterbox
        && (client[0] < left || client[0] > right || client[1] < top || client[1] > bottom)
    {
        return None;
    }
    Some(ScreenPoint {
        x: (client[0] - left) / scale,
        y: (client[1] - top) / scale,
    })
}

/// Translates one browser-captured modifier sample into semantic headless
/// authoring input. Ctrl/Cmd suppress ambient inference while Shift remains an
/// independent recipe regularization request.
#[must_use]
pub(super) const fn draft_authoring_input(
    modifiers: Modifiers,
    preferred_candidate: Option<DraftInferenceCandidateId>,
) -> DraftAuthoringInput {
    draft_authoring_input_for_state(
        modifiers.control || modifiers.command,
        modifiers.shift,
        preferred_candidate,
    )
}

/// Builds semantic authoring input for a keyboard-state transition that has no
/// newer pointer coordinates of its own.
#[must_use]
pub(super) const fn draft_authoring_input_for_state(
    suppressed: bool,
    regularized: bool,
    preferred_candidate: Option<DraftInferenceCandidateId>,
) -> DraftAuthoringInput {
    DraftAuthoringInput {
        inference: DraftInferenceInput {
            suppressed,
            preferred_candidate,
        },
        regularized,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConstructionDispatch {
    NotConstruction,
    ApplyCommit,
    Handled,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PlannedConstructionOutcome {
    pub(super) accepted: bool,
    pub(super) error: Option<String>,
    pub(super) acknowledgement: Vec<EditorEffect>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum PlannedConstructionDispatch {
    NotPlannedConstruction,
    Handled(PlannedConstructionOutcome),
}

/// Applies and acknowledges one tokenized inferred construction without any
/// browser or DOM state. Keeping this transition in the native adapter makes
/// accepted, rejected and stale-token behavior directly testable.
pub(super) fn dispatch_planned_construction_effect(
    coordinator: &mut RetainedEditorCoordinator,
    effect: &EditorEffect,
) -> PlannedConstructionDispatch {
    let EditorEffect::CommitConstructionPlan { token, .. } = effect else {
        return PlannedConstructionDispatch::NotPlannedConstruction;
    };
    let result = coordinator.apply_editor_effect(effect);
    let accepted = result.is_ok();
    let error = result.err().map(|error| error.to_string());
    let acknowledgement = coordinator.acknowledge_construction_commit(*token, accepted);
    PlannedConstructionDispatch::Handled(PlannedConstructionOutcome {
        accepted,
        error,
        acknowledgement,
    })
}

pub(super) fn dispatch_construction_effect(
    preview: &mut Option<ConstructionPreview>,
    effect: &EditorEffect,
    commit_succeeded: Option<bool>,
    failed_commit: &mut bool,
) -> ConstructionDispatch {
    match effect {
        EditorEffect::PreviewConstruction(next) => {
            *preview = Some(next.clone());
            *failed_commit = false;
            ConstructionDispatch::Handled
        }
        EditorEffect::CommitConstruction { .. } => match commit_succeeded {
            Some(succeeded) => {
                *failed_commit = !succeeded;
                ConstructionDispatch::Handled
            }
            None => ConstructionDispatch::ApplyCommit,
        },
        EditorEffect::ClearConstructionPreview => {
            if !*failed_commit {
                *preview = None;
            }
            *failed_commit = false;
            ConstructionDispatch::Handled
        }
        _ => ConstructionDispatch::NotConstruction,
    }
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{
        ConstraintEditor, ConstructionPoint, ConstructionPreview, ConstructionProposal,
        DraftCurveSlot, EditorEffect, EditorScene, EditorTool, InferredRelation, Modifiers,
        PointerInput, RetainedEditorCoordinator, ScreenPoint, Viewport,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        CurveDefinition, DocumentConstraintDefinition, DocumentSolveRequest, GeometryRole,
        RetainedSketchDocumentSession, ScalarDomain, ScalarUnit, SketchDocument,
    };

    use super::{
        ClientRect, ConstructionDispatch, PlannedConstructionDispatch, UnmappedCanvasPointerAction,
        dispatch_construction_effect, dispatch_planned_construction_effect, draft_authoring_input,
        draft_authoring_input_for_state, normalize_captured_client_point, normalize_client_point,
        unmapped_canvas_pointer_action,
    };

    fn input(pointer_id: u64, position: [f64; 2]) -> PointerInput {
        PointerInput {
            pointer_id,
            position: ScreenPoint {
                x: position[0],
                y: position[1],
            },
            modifiers: Modifiers::default(),
        }
    }

    fn inferred_plan_fixture() -> (RetainedEditorCoordinator, EditorScene, EditorEffect) {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(10.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        let effect = emit_inferred_plan(&mut coordinator, &scene, 91);
        (coordinator, scene, effect)
    }

    fn emit_inferred_plan(
        coordinator: &mut RetainedEditorCoordinator,
        scene: &EditorScene,
        pointer_id: u64,
    ) -> EditorEffect {
        coordinator.editor_mut().activate_tool(EditorTool::Line);
        for position in [[0.0, 0.0], [2.0, 0.01]] {
            let screen = scene.viewport.model_to_screen(position);
            let effects = coordinator.pointer_down(scene, input(pointer_id, [screen.x, screen.y]));
            if let Some(effect) = effects
                .into_iter()
                .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
            {
                return effect;
            }
        }
        panic!("inferred construction plan was not emitted");
    }

    #[test]
    fn client_normalization_rejects_non_positive_extents() {
        for rect in [
            ClientRect {
                left: 0.0,
                top: 0.0,
                width: 0.0,
                height: 100.0,
            },
            ClientRect {
                left: 0.0,
                top: 0.0,
                width: 100.0,
                height: -1.0,
            },
        ] {
            assert_eq!(
                normalize_client_point(rect, [1000.0, 700.0], [0.0, 0.0]),
                None
            );
        }
    }

    #[test]
    fn client_normalization_accounts_for_letterboxing_and_css_size_only() {
        let widescreen = ClientRect {
            left: 10.0,
            top: 20.0,
            width: 2000.0,
            height: 700.0,
        };
        assert_eq!(
            normalize_client_point(widescreen, [1000.0, 700.0], [510.0, 20.0]),
            Some(geosolve_constraint_editor::ScreenPoint { x: 0.0, y: 0.0 })
        );
        assert_eq!(
            normalize_client_point(widescreen, [1000.0, 700.0], [1510.0, 720.0]),
            Some(geosolve_constraint_editor::ScreenPoint {
                x: 1000.0,
                y: 700.0
            })
        );
        for point in [[509.999, 350.0], [1510.001, 350.0]] {
            assert_eq!(
                normalize_client_point(widescreen, [1000.0, 700.0], point),
                None,
                "horizontal letterbox band at {point:?} must not become canvas input"
            );
            assert!(
                normalize_captured_client_point(widescreen, [1000.0, 700.0], point).is_some(),
                "captured interaction must retain its historical translated sample at {point:?}"
            );
            assert_eq!(
                unmapped_canvas_pointer_action(false),
                UnmappedCanvasPointerAction::RevokePointerContext,
            );
            assert_eq!(
                unmapped_canvas_pointer_action(true),
                UnmappedCanvasPointerAction::PreserveCapturedGesture,
            );
        }
        let alternate_css = ClientRect {
            left: 100.0,
            top: 50.0,
            width: 500.0,
            height: 350.0,
        };
        assert_eq!(
            normalize_client_point(alternate_css, [1000.0, 700.0], [350.0, 225.0]),
            Some(geosolve_constraint_editor::ScreenPoint { x: 500.0, y: 350.0 })
        );

        let portrait = ClientRect {
            left: 40.0,
            top: 10.0,
            width: 500.0,
            height: 700.0,
        };
        // The fitted viewBox is 500x350 CSS pixels, vertically centred at y=185.
        for point in [[290.0, 184.999], [290.0, 535.001]] {
            assert_eq!(
                normalize_client_point(portrait, [1000.0, 700.0], point),
                None,
                "vertical letterbox band at {point:?} must not become canvas input"
            );
            assert!(
                normalize_captured_client_point(portrait, [1000.0, 700.0], point).is_some(),
                "captured interaction must retain its historical translated sample at {point:?}"
            );
        }
        assert_eq!(
            normalize_client_point(portrait, [1000.0, 700.0], [40.0, 185.0]),
            Some(geosolve_constraint_editor::ScreenPoint { x: 0.0, y: 0.0 })
        );
    }

    #[test]
    fn modifiers_keep_ambient_suppression_and_recipe_regularization_independent() {
        assert_eq!(
            draft_authoring_input(Modifiers::default(), None),
            geosolve_constraint_editor::DraftAuthoringInput::default()
        );
        assert_eq!(
            draft_authoring_input(
                Modifiers {
                    shift: true,
                    control: true,
                    command: true,
                },
                None
            ),
            geosolve_constraint_editor::DraftAuthoringInput {
                inference: geosolve_constraint_editor::DraftInferenceInput {
                    suppressed: true,
                    preferred_candidate: None,
                },
                regularized: true,
            }
        );
        assert_eq!(
            draft_authoring_input_for_state(false, true, None),
            geosolve_constraint_editor::DraftAuthoringInput {
                inference: geosolve_constraint_editor::DraftInferenceInput::default(),
                regularized: true,
            }
        );
    }

    #[test]
    fn planned_construction_dispatch_accepts_and_acknowledges_atomically() {
        let (mut coordinator, _, effect) = inferred_plan_fixture();
        let before = coordinator.session().design_identity();
        let PlannedConstructionDispatch::Handled(outcome) =
            dispatch_planned_construction_effect(&mut coordinator, &effect)
        else {
            panic!("planned effect was not handled");
        };
        assert!(outcome.accepted);
        assert!(outcome.error.is_none());
        assert!(
            outcome
                .acknowledgement
                .iter()
                .any(|effect| matches!(effect, EditorEffect::ClearConstructionPreview))
        );
        assert_ne!(coordinator.session().design_identity(), before);
        assert!(
            coordinator
                .editor()
                .pending_construction_commit_token()
                .is_none()
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one thin browser-adapter regression keeps default input, emitted plan, dispatch, and retained result together"
    )]
    fn ordinary_browser_centered_input_commits_concentric_over_colocated_point_identity() {
        let mut document = SketchDocument::new(10.0).expect("document");
        let stored_center = document
            .add_point("stored center", [0.0, 0.0])
            .expect("stored center");
        let radius = document
            .add_scalar(
                "reference radius",
                3.0,
                ScalarUnit::Length,
                ScalarDomain::Positive,
            )
            .expect("reference radius");
        let reference = document
            .add_curve(
                "reference circle",
                CurveDefinition::Circle {
                    center: stored_center,
                    radius,
                },
            )
            .expect("reference circle");
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted = session
            .accepted_state_for_current_input()
            .expect("accepted state");
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            accepted.design_identity(),
            accepted.document(),
            session.design_document(),
            Viewport::new([1000.0, 700.0], [0.0, 0.0], 50.0).expect("viewport"),
            0.5,
        )
        .expect("scene")
        .with_retained_session(&session)
        .expect("bound scene");
        let mut coordinator = RetainedEditorCoordinator::new(session).expect("coordinator");
        coordinator.editor_mut().activate_tool(EditorTool::Circle);

        let browser_input = draft_authoring_input(Modifiers::default(), None);
        assert_eq!(browser_input.inference.preferred_candidate, None);
        let center = scene.viewport.model_to_screen([0.0, 0.0]);
        coordinator.editor_mut().pointer_down_with_draft_authoring(
            &scene,
            input(71, [center.x, center.y]),
            browser_input,
        );
        let rim = scene.viewport.model_to_screen([1.5, 0.0]);
        let effect = coordinator
            .editor_mut()
            .pointer_down_with_draft_authoring(
                &scene,
                input(71, [rim.x, rim.y]),
                draft_authoring_input(Modifiers::default(), None),
            )
            .into_iter()
            .find(|effect| matches!(effect, EditorEffect::CommitConstructionPlan { .. }))
            .expect("ordinary browser input must emit an inferred circle plan");
        let EditorEffect::CommitConstructionPlan { plan, .. } = &effect else {
            unreachable!("filtered construction-plan effect")
        };
        assert!(matches!(
            plan.proposal,
            ConstructionProposal::Circle {
                center: ConstructionPoint::New(position),
                ..
            } if position == [0.0, 0.0]
        ));
        assert!(matches!(
            plan.relations.as_slice(),
            [InferredRelation::Concentric {
                first: DraftCurveSlot::Created { curve_index: 0 },
                second: DraftCurveSlot::Existing(existing),
            }] if *existing == reference
        ));

        let PlannedConstructionDispatch::Handled(outcome) =
            dispatch_planned_construction_effect(&mut coordinator, &effect)
        else {
            panic!("planned effect was not handled");
        };
        assert!(outcome.accepted, "{:?}", outcome.error);
        assert!(
            outcome
                .acknowledgement
                .contains(&EditorEffect::ClearConstructionPreview)
        );

        let retained = coordinator.session().design_document();
        let created = retained
            .curves()
            .iter()
            .find(|curve| curve.id != reference)
            .expect("created circle");
        assert!(matches!(
            created.definition,
            CurveDefinition::Circle { center, .. } if center != stored_center
        ));
        assert!(retained.constraints().iter().any(|constraint| matches!(
            constraint.definition,
            DocumentConstraintDefinition::Concentric { first, second }
                if first.curve == created.id && second.curve == reference
        )));
    }

    #[test]
    fn planned_construction_rejection_retains_preview_and_stale_ack_preserves_new_token() {
        let (mut coordinator, scene, original) = inferred_plan_fixture();
        let before = coordinator.session().design_identity();
        let mut substituted = original.clone();
        let EditorEffect::CommitConstructionPlan { plan, token, .. } = &mut substituted else {
            unreachable!("fixture returns a construction plan")
        };
        plan.role = match plan.role {
            GeometryRole::Profile => GeometryRole::Construction,
            GeometryRole::Construction => GeometryRole::Profile,
        };
        let rejected_token = *token;
        let PlannedConstructionDispatch::Handled(rejected) =
            dispatch_planned_construction_effect(&mut coordinator, &substituted)
        else {
            panic!("planned effect was not handled");
        };
        assert!(!rejected.accepted);
        assert!(rejected.error.is_some());
        assert!(rejected.acknowledgement.is_empty());
        assert_eq!(coordinator.session().design_identity(), before);

        let mut preview = Some(ConstructionPreview::Complete {
            proposal: ConstructionProposal::Point {
                point: ConstructionPoint::New([0.0, 0.0]),
            },
            geometry: geosolve_constraint_editor::ConstructionPreviewGeometry::Point {
                position: [0.0, 0.0],
            },
        });
        let mut failed = true;
        for effect in &rejected.acknowledgement {
            dispatch_construction_effect(&mut preview, effect, None, &mut failed);
        }
        assert!(
            preview.is_some(),
            "rejection must not clear the terminal preview"
        );

        let current = emit_inferred_plan(&mut coordinator, &scene, 92);
        let EditorEffect::CommitConstructionPlan {
            token: current_token,
            ..
        } = current
        else {
            unreachable!("helper returns a construction plan");
        };
        assert_ne!(current_token, rejected_token);
        let PlannedConstructionDispatch::Handled(stale) =
            dispatch_planned_construction_effect(&mut coordinator, &original)
        else {
            panic!("stale planned effect was not handled");
        };
        assert!(!stale.accepted);
        assert!(stale.acknowledgement.is_empty());
        assert_eq!(
            coordinator.editor().pending_construction_commit_token(),
            Some(current_token),
            "a stale acknowledgement must not consume the genuine pending token"
        );
    }

    #[test]
    fn failed_construction_commit_retains_preview_across_terminal_clear() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(10.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let proposal = ConstructionProposal::Line {
            start: ConstructionPoint::New([0.0, 0.0]),
            end: ConstructionPoint::New([1.0, 0.0]),
        };
        let mut preview = Some(ConstructionPreview::Anchor {
            position: [0.0, 0.0],
        });
        let commit = EditorEffect::CommitConstruction {
            expected: session.design_identity(),
            proposal,
            role: GeometryRole::Profile,
        };
        let clear = EditorEffect::ClearConstructionPreview;
        let mut failed_commit = false;

        assert_eq!(
            dispatch_construction_effect(&mut preview, &commit, None, &mut failed_commit),
            ConstructionDispatch::ApplyCommit
        );
        dispatch_construction_effect(&mut preview, &commit, Some(false), &mut failed_commit);
        dispatch_construction_effect(&mut preview, &clear, None, &mut failed_commit);

        assert!(preview.is_some());
    }

    #[test]
    fn successful_construction_commit_clears_preview_on_terminal_clear() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(10.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let commit = EditorEffect::CommitConstruction {
            expected: session.design_identity(),
            proposal: ConstructionProposal::Point {
                point: ConstructionPoint::New([1.0, 0.0]),
            },
            role: GeometryRole::Profile,
        };
        let mut preview = Some(ConstructionPreview::Anchor {
            position: [1.0, 0.0],
        });
        let mut failed_commit = false;

        dispatch_construction_effect(&mut preview, &commit, Some(true), &mut failed_commit);
        dispatch_construction_effect(
            &mut preview,
            &EditorEffect::ClearConstructionPreview,
            None,
            &mut failed_commit,
        );

        assert!(preview.is_none());
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one adapter regression keeps preview retention and inference publication on the same pointer lifecycle"
    )]
    fn m49_editor_cancel_and_invalid_completion_only_clear_or_retain_staged_preview() {
        // Match the accepted fixed-line scene and snapped pointer route covered by the
        // editor's construction tests. An empty document does not provide a valid accepted
        // scene for this adapter fixture.
        let mut document = SketchDocument::new(1.0).expect("document");
        let start = document.add_point("a", [0.0, 0.0]).expect("start");
        let end = document.add_point("b", [2.0, 0.0]).expect("end");
        document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .expect("line");
        for (label, point, target) in [
            ("fix start", start, [0.0, 0.0]),
            ("fix end", end, [2.0, 0.0]),
        ] {
            document
                .add_constraint(
                    label,
                    DocumentConstraintDefinition::FixedPoint { point, target },
                )
                .expect("fixed point");
        }
        let session = RetainedSketchDocumentSession::new(
            document,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let accepted_json = session
            .accepted_state()
            .unwrap()
            .document()
            .to_canonical_json()
            .unwrap();
        let design = session.design_identity();
        let accepted = session.accepted_state().unwrap();
        let scene = EditorScene::from_accepted_for_design(
            accepted.identity().revision().get(),
            design,
            accepted.document(),
            session.design_document(),
            crate::workbench::scene::viewport(),
            0.8,
        )
        .unwrap();
        let mut editor = ConstraintEditor::default();
        editor.activate_tool(EditorTool::Line);
        let start = scene.viewport.model_to_screen([0.0, 0.0]);
        assert!(
            editor
                .pointer_down(&scene, input(1, [start.x, start.y]))
                .is_empty()
        );
        let preview_end = scene.viewport.model_to_screen([1.0, 0.0]);
        let staged = editor.pointer_move(&scene, input(1, [preview_end.x, preview_end.y]));
        let preview = staged
            .iter()
            .find_map(|effect| match effect {
                EditorEffect::PreviewConstruction(preview) => Some(preview),
                _ => None,
            })
            .expect("line route must stage a construction preview");
        assert!(
            staged
                .iter()
                .any(|effect| matches!(effect, EditorEffect::DraftInferenceChanged(Some(_)))),
            "the same pointer sample should publish its headless inference DTO"
        );
        let mut displayed = None;
        let mut failed_commit = false;
        dispatch_construction_effect(
            &mut displayed,
            &EditorEffect::PreviewConstruction(preview.clone()),
            None,
            &mut failed_commit,
        );

        // A duplicate terminal point is an invalid line completion: the editor emits no
        // commit or clear and retains the already staged preview.
        assert!(
            editor
                .pointer_down(&scene, input(1, [start.x, start.y]))
                .is_empty()
        );
        assert_eq!(displayed, Some(preview.clone()));
        assert_eq!(
            session
                .accepted_state()
                .unwrap()
                .document()
                .to_canonical_json()
                .unwrap(),
            accepted_json
        );

        let effects = editor.cancel();
        assert!(effects.contains(&EditorEffect::ClearConstructionPreview));
        assert!(effects.contains(&EditorEffect::DraftInferenceChanged(None)));
        for effect in &effects {
            dispatch_construction_effect(&mut displayed, effect, None, &mut failed_commit);
        }
        assert!(displayed.is_none());
        assert_eq!(
            session
                .accepted_state()
                .unwrap()
                .document()
                .to_canonical_json()
                .unwrap(),
            accepted_json
        );
    }
}
