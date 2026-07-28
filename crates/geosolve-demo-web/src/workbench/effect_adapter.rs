// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_constraint_editor::{
    ConstructionPreview, EditorEffect, ProvisionalInferenceCandidate, ScreenPoint,
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
    let [screen_width, screen_height] = screen_size;
    if !rect.width.is_finite()
        || !rect.height.is_finite()
        || !screen_width.is_finite()
        || !screen_height.is_finite()
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
    Some(ScreenPoint {
        x: (client[0] - left) / scale,
        y: (client[1] - top) / scale,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ConstructionDispatch {
    NotConstruction,
    ApplyCommit,
    Handled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InferenceDispatch {
    NotInference,
    ApplyCommit,
    Handled,
}

pub(super) fn dispatch_inference_effect(
    preview: &mut Option<ProvisionalInferenceCandidate>,
    effect: &EditorEffect,
) -> InferenceDispatch {
    match effect {
        EditorEffect::PreviewInference(candidate) => {
            *preview = Some(candidate.clone());
            InferenceDispatch::Handled
        }
        EditorEffect::CommitInference(_) => InferenceDispatch::ApplyCommit,
        EditorEffect::ClearInferencePreview => {
            *preview = None;
            InferenceDispatch::Handled
        }
        _ => InferenceDispatch::NotInference,
    }
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
        EditorEffect, EditorScene, EditorTool, Modifiers, PointerInput,
        ProvisionalInferenceCandidate, ScreenPoint,
    };
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        CurveDefinition, DocumentConstraintDefinition, DocumentEdit, DocumentSolveRequest,
        RetainedSketchDocumentSession, SketchDocument,
    };

    use super::{
        ClientRect, ConstructionDispatch, InferenceDispatch, dispatch_construction_effect,
        dispatch_inference_effect, normalize_client_point,
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
                position: [1.0, 0.0],
            },
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
    fn inference_dispatch_stages_commits_and_clears_typed_preview_state() {
        let session = RetainedSketchDocumentSession::new(
            SketchDocument::new(1.0).expect("document"),
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .expect("session");
        let candidate = ProvisionalInferenceCandidate {
            expected: session.design_identity(),
            label: "coincident inference".into(),
            edit: DocumentEdit::CreatePoint {
                label: "inferred point".into(),
                position: [1.0, 2.0],
            },
        };
        let mut preview = None;

        assert_eq!(
            dispatch_inference_effect(
                &mut preview,
                &EditorEffect::PreviewInference(candidate.clone()),
            ),
            InferenceDispatch::Handled
        );
        assert_eq!(preview, Some(candidate.clone()));
        assert_eq!(
            dispatch_inference_effect(
                &mut preview,
                &EditorEffect::CommitInference(candidate.clone()),
            ),
            InferenceDispatch::ApplyCommit
        );
        assert_eq!(preview, Some(candidate));
        assert_eq!(
            dispatch_inference_effect(&mut preview, &EditorEffect::ClearInferencePreview),
            InferenceDispatch::Handled
        );
        assert!(preview.is_none());
    }

    #[test]
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
        let [EditorEffect::PreviewConstruction(preview)] = staged.as_slice() else {
            panic!("line route must stage a construction preview");
        };
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
        assert_eq!(effects, [EditorEffect::ClearConstructionPreview]);
        dispatch_construction_effect(&mut displayed, &effects[0], None, &mut failed_commit);
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
