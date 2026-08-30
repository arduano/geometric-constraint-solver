// SPDX-License-Identifier: GPL-3.0-or-later

use std::cell::Cell;

/// Actual presentation and semantic work crossed by one browser workbench.
///
/// This is deliberately a monotonic count ledger rather than a timing oracle.
/// Native tests can therefore prove that a camera-only callback did not cross
/// a solver, materialization, persistence, or durable-render boundary, while a
/// focused browser trace remains responsible for wall-clock budgets.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct PresentationWorkSnapshot {
    counts: [u64; PresentationWork::COUNT],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum PresentationWork {
    CameraPresentation,
    ExactReprojection,
    HoverPresentation,
    SceneComposition,
    SvgSerialization,
    ViewportReplacement,
    SolverPreview,
    IntentMaterialization,
    ComputedEvaluation,
    NativeHistoryPublication,
    CodeParse,
    CodeExpansion,
    CodePublication,
    PersistenceWrite,
    DurablePanelRebuild,
}

impl PresentationWork {
    const COUNT: usize = 15;

    const ALL: [Self; Self::COUNT] = [
        Self::CameraPresentation,
        Self::ExactReprojection,
        Self::HoverPresentation,
        Self::SceneComposition,
        Self::SvgSerialization,
        Self::ViewportReplacement,
        Self::SolverPreview,
        Self::IntentMaterialization,
        Self::ComputedEvaluation,
        Self::NativeHistoryPublication,
        Self::CodeParse,
        Self::CodeExpansion,
        Self::CodePublication,
        Self::PersistenceWrite,
        Self::DurablePanelRebuild,
    ];

    const fn key(self) -> &'static str {
        match self {
            Self::CameraPresentation => "camera-presentations",
            Self::ExactReprojection => "exact-reprojections",
            Self::HoverPresentation => "hover-presentations",
            Self::SceneComposition => "scene-compositions",
            Self::SvgSerialization => "svg-serializations",
            Self::ViewportReplacement => "viewport-replacements",
            Self::SolverPreview => "solver-previews",
            Self::IntentMaterialization => "intent-materializations",
            Self::ComputedEvaluation => "computed-evaluations",
            Self::NativeHistoryPublication => "native-history-publications",
            Self::CodeParse => "code-parses",
            Self::CodeExpansion => "code-expansions",
            Self::CodePublication => "code-publications",
            Self::PersistenceWrite => "persistence-writes",
            Self::DurablePanelRebuild => "durable-panel-rebuilds",
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct PresentationWorkLedger {
    snapshot: Cell<PresentationWorkSnapshot>,
}

impl PresentationWorkLedger {
    pub(crate) fn record(&self, work: PresentationWork) {
        self.record_count(work, 1);
    }

    pub(crate) fn record_count(&self, work: PresentationWork, count: u64) {
        let mut snapshot = self.snapshot.get();
        snapshot.counts[work as usize] = snapshot.counts[work as usize].saturating_add(count);
        self.snapshot.set(snapshot);
    }

    pub(crate) fn snapshot(&self) -> PresentationWorkSnapshot {
        self.snapshot.get()
    }
}

impl PresentationWorkSnapshot {
    #[must_use]
    pub(crate) fn delta_since(self, prior: Self) -> Self {
        let mut counts = [0; PresentationWork::COUNT];
        for (index, count) in counts.iter_mut().enumerate() {
            *count = self.counts[index].saturating_sub(prior.counts[index]);
        }
        Self { counts }
    }

    /// A raw camera RAF may update retained paint only. Exact reprojection is
    /// a separately authenticated reconciliation boundary and must remain zero
    /// in this predicate.
    #[must_use]
    pub(crate) fn is_camera_frame_only(self) -> bool {
        self.count(PresentationWork::CameraPresentation) == 1
            && PresentationWork::ALL
                .into_iter()
                .filter(|work| *work != PresentationWork::CameraPresentation)
                .all(|work| self.count(work) == 0)
    }

    /// A retained hover frame may update only stable SVG presentation state.
    /// In particular it cannot compose or serialize a scene, replace the
    /// viewport, evaluate semantics, persist, or rebuild durable panels.
    #[must_use]
    pub(crate) fn is_hover_frame_only(self) -> bool {
        self.count(PresentationWork::HoverPresentation) == 1
            && PresentationWork::ALL
                .into_iter()
                .filter(|work| *work != PresentationWork::HoverPresentation)
                .all(|work| self.count(work) == 0)
    }

    /// A semantic pointer preview may solve, materialize or evaluate its
    /// disposable native preview and repaint the canvas, but it cannot cross
    /// any durable or code-authoring boundary. Scene composition is explicit
    /// because the browser currently projects the newest accepted preview
    /// into a fresh screen-space DTO before serializing it.
    #[must_use]
    pub(crate) fn is_transient_preview_only(self) -> bool {
        self.count(PresentationWork::SceneComposition) >= 1
            && self.count(PresentationWork::SvgSerialization) >= 1
            && self.count(PresentationWork::ViewportReplacement) >= 1
            && [
                PresentationWork::CameraPresentation,
                PresentationWork::ExactReprojection,
                PresentationWork::HoverPresentation,
                PresentationWork::NativeHistoryPublication,
                PresentationWork::CodeParse,
                PresentationWork::CodeExpansion,
                PresentationWork::CodePublication,
                PresentationWork::PersistenceWrite,
                PresentationWork::DurablePanelRebuild,
            ]
            .into_iter()
            .all(|work| self.count(work) == 0)
    }

    /// One accepted pointer terminal may run native validation, code
    /// rematerialization/evaluation, one persistence write and one durable
    /// rebuild. Counts are deliberately exact for the publication side
    /// effects while semantic work may contain several independently audited
    /// stages inside that single transaction.
    #[must_use]
    pub(crate) fn is_single_terminal_publication(self) -> bool {
        self.count(PresentationWork::PersistenceWrite) == 1
            && self.count(PresentationWork::DurablePanelRebuild) == 1
            && self.count(PresentationWork::NativeHistoryPublication) == 1
            && self.count(PresentationWork::CodePublication) <= 1
            && self.count(PresentationWork::SceneComposition) >= 1
            && self.count(PresentationWork::SvgSerialization) >= 1
            && self.count(PresentationWork::ViewportReplacement) >= 1
            && self.count(PresentationWork::CameraPresentation) == 0
            && self.count(PresentationWork::HoverPresentation) == 0
    }

    /// Presentation admitted after an outer code/source transaction has
    /// already published. Persistence and one durable rebuild are required;
    /// a nested native-history or delegated code-checkpoint publication at
    /// this boundary is specifically forbidden.
    #[must_use]
    pub(crate) fn is_code_source_terminal_presentation(self) -> bool {
        self.count(PresentationWork::PersistenceWrite) == 1
            && self.count(PresentationWork::DurablePanelRebuild) == 1
            && self.count(PresentationWork::NativeHistoryPublication) == 0
            && self.count(PresentationWork::CodePublication) == 0
            && self.count(PresentationWork::CodeParse) == 0
            && self.count(PresentationWork::CodeExpansion) == 0
            && self.count(PresentationWork::SceneComposition) >= 1
            && self.count(PresentationWork::SvgSerialization) >= 1
            && self.count(PresentationWork::ViewportReplacement) >= 1
            && self.count(PresentationWork::CameraPresentation) == 0
            && self.count(PresentationWork::HoverPresentation) == 0
    }

    pub(crate) const fn count(self, work: PresentationWork) -> u64 {
        self.counts[work as usize]
    }

    /// Compact deterministic evidence suitable for a DOM data attribute and
    /// focused candidate-only browser trace.
    pub(crate) fn compact(self) -> String {
        PresentationWork::ALL
            .into_iter()
            .map(|work| format!("{}={}", work.key(), self.count(work)))
            .collect::<Vec<_>>()
            .join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::{PresentationWork, PresentationWorkLedger};

    #[test]
    fn camera_frame_ledger_rejects_every_forbidden_work_family() {
        let ledger = PresentationWorkLedger::default();
        let before = ledger.snapshot();
        ledger.record(PresentationWork::CameraPresentation);
        assert!(ledger.snapshot().delta_since(before).is_camera_frame_only());

        for forbidden in [
            PresentationWork::ExactReprojection,
            PresentationWork::HoverPresentation,
            PresentationWork::SceneComposition,
            PresentationWork::SvgSerialization,
            PresentationWork::ViewportReplacement,
            PresentationWork::SolverPreview,
            PresentationWork::IntentMaterialization,
            PresentationWork::ComputedEvaluation,
            PresentationWork::NativeHistoryPublication,
            PresentationWork::CodeParse,
            PresentationWork::CodeExpansion,
            PresentationWork::CodePublication,
            PresentationWork::PersistenceWrite,
            PresentationWork::DurablePanelRebuild,
        ] {
            let ledger = PresentationWorkLedger::default();
            let before = ledger.snapshot();
            ledger.record(PresentationWork::CameraPresentation);
            ledger.record(forbidden);
            assert!(
                !ledger.snapshot().delta_since(before).is_camera_frame_only(),
                "forbidden work: {forbidden:?}"
            );
        }
    }

    #[test]
    fn camera_frame_ledger_requires_exactly_one_camera_presentation() {
        let ledger = PresentationWorkLedger::default();
        let before = ledger.snapshot();
        assert!(
            !ledger.snapshot().delta_since(before).is_camera_frame_only(),
            "an inert or stale callback is not a painted camera frame"
        );

        ledger.record(PresentationWork::CameraPresentation);
        ledger.record(PresentationWork::CameraPresentation);
        assert!(
            !ledger.snapshot().delta_since(before).is_camera_frame_only(),
            "two retained paints in one admitted frame must not satisfy the one-paint contract"
        );
    }

    #[test]
    fn successful_retained_hover_ledger_admits_exactly_one_presentation() {
        let ledger = PresentationWorkLedger::default();
        let before = ledger.snapshot();
        ledger.record(PresentationWork::HoverPresentation);
        let delta = ledger.snapshot().delta_since(before);

        assert!(delta.is_hover_frame_only());
        assert_eq!(delta.count(PresentationWork::HoverPresentation), 1);
        for forbidden in [
            PresentationWork::CameraPresentation,
            PresentationWork::ExactReprojection,
            PresentationWork::SceneComposition,
            PresentationWork::SvgSerialization,
            PresentationWork::ViewportReplacement,
            PresentationWork::SolverPreview,
            PresentationWork::IntentMaterialization,
            PresentationWork::ComputedEvaluation,
            PresentationWork::NativeHistoryPublication,
            PresentationWork::CodeParse,
            PresentationWork::CodeExpansion,
            PresentationWork::CodePublication,
            PresentationWork::PersistenceWrite,
            PresentationWork::DurablePanelRebuild,
        ] {
            assert_eq!(delta.count(forbidden), 0, "forbidden work: {forbidden:?}");
        }
    }

    #[test]
    fn retained_hover_ledger_rejects_inert_duplicate_and_forbidden_work() {
        let ledger = PresentationWorkLedger::default();
        let before = ledger.snapshot();
        assert!(!ledger.snapshot().delta_since(before).is_hover_frame_only());

        ledger.record(PresentationWork::HoverPresentation);
        ledger.record(PresentationWork::HoverPresentation);
        assert!(!ledger.snapshot().delta_since(before).is_hover_frame_only());

        let ledger = PresentationWorkLedger::default();
        let before = ledger.snapshot();
        ledger.record(PresentationWork::HoverPresentation);
        ledger.record(PresentationWork::SvgSerialization);
        assert!(!ledger.snapshot().delta_since(before).is_hover_frame_only());
    }

    #[test]
    fn ledger_delta_is_monotonic_and_has_stable_browser_keys() {
        let ledger = PresentationWorkLedger::default();
        ledger.record(PresentationWork::SceneComposition);
        let before = ledger.snapshot();
        ledger.record(PresentationWork::CameraPresentation);
        ledger.record(PresentationWork::CameraPresentation);
        ledger.record(PresentationWork::HoverPresentation);

        let delta = ledger.snapshot().delta_since(before);
        assert_eq!(delta.count(PresentationWork::CameraPresentation), 2);
        assert_eq!(delta.count(PresentationWork::HoverPresentation), 1);
        assert_eq!(delta.count(PresentationWork::SceneComposition), 0);
        assert_eq!(
            delta.compact(),
            "camera-presentations=2,exact-reprojections=0,hover-presentations=1,scene-compositions=0,svg-serializations=0,viewport-replacements=0,solver-previews=0,intent-materializations=0,computed-evaluations=0,native-history-publications=0,code-parses=0,code-expansions=0,code-publications=0,persistence-writes=0,durable-panel-rebuilds=0"
        );
    }

    #[test]
    fn transient_preview_contract_rejects_code_persistence_and_panels() {
        let ledger = PresentationWorkLedger::default();
        let before = ledger.snapshot();
        for work in [
            PresentationWork::SolverPreview,
            PresentationWork::IntentMaterialization,
            PresentationWork::ComputedEvaluation,
            PresentationWork::SceneComposition,
            PresentationWork::SvgSerialization,
            PresentationWork::ViewportReplacement,
        ] {
            ledger.record(work);
        }
        assert!(
            ledger
                .snapshot()
                .delta_since(before)
                .is_transient_preview_only()
        );

        for forbidden in [
            PresentationWork::NativeHistoryPublication,
            PresentationWork::CodeParse,
            PresentationWork::CodeExpansion,
            PresentationWork::CodePublication,
            PresentationWork::PersistenceWrite,
            PresentationWork::DurablePanelRebuild,
        ] {
            let ledger = PresentationWorkLedger::default();
            let before = ledger.snapshot();
            for work in [
                PresentationWork::SolverPreview,
                PresentationWork::SceneComposition,
                PresentationWork::SvgSerialization,
                PresentationWork::ViewportReplacement,
                forbidden,
            ] {
                ledger.record(work);
            }
            assert!(
                !ledger
                    .snapshot()
                    .delta_since(before)
                    .is_transient_preview_only(),
                "forbidden transient work: {forbidden:?}"
            );
        }
    }

    #[test]
    fn terminal_contract_requires_exactly_one_save_and_panel_rebuild() {
        let ledger = PresentationWorkLedger::default();
        let before = ledger.snapshot();
        for work in [
            PresentationWork::SolverPreview,
            PresentationWork::IntentMaterialization,
            PresentationWork::ComputedEvaluation,
            PresentationWork::NativeHistoryPublication,
            PresentationWork::CodeExpansion,
            PresentationWork::CodePublication,
            PresentationWork::PersistenceWrite,
            PresentationWork::DurablePanelRebuild,
            PresentationWork::SceneComposition,
            PresentationWork::SvgSerialization,
            PresentationWork::ViewportReplacement,
        ] {
            ledger.record(work);
        }
        assert!(
            ledger
                .snapshot()
                .delta_since(before)
                .is_single_terminal_publication()
        );

        ledger.record(PresentationWork::PersistenceWrite);
        assert!(
            !ledger
                .snapshot()
                .delta_since(before)
                .is_single_terminal_publication()
        );
    }

    #[test]
    fn code_source_terminal_persists_without_a_second_checkpoint_publication() {
        let ledger = PresentationWorkLedger::default();
        let before = ledger.snapshot();
        for work in [
            PresentationWork::PersistenceWrite,
            PresentationWork::DurablePanelRebuild,
            PresentationWork::SceneComposition,
            PresentationWork::SvgSerialization,
            PresentationWork::ViewportReplacement,
        ] {
            ledger.record(work);
        }
        assert!(
            ledger
                .snapshot()
                .delta_since(before)
                .is_code_source_terminal_presentation()
        );

        for forbidden in [
            PresentationWork::NativeHistoryPublication,
            PresentationWork::CodeParse,
            PresentationWork::CodeExpansion,
            PresentationWork::CodePublication,
        ] {
            let ledger = PresentationWorkLedger::default();
            let before = ledger.snapshot();
            for work in [
                PresentationWork::PersistenceWrite,
                PresentationWork::DurablePanelRebuild,
                PresentationWork::SceneComposition,
                PresentationWork::SvgSerialization,
                PresentationWork::ViewportReplacement,
                forbidden,
            ] {
                ledger.record(work);
            }
            assert!(
                !ledger
                    .snapshot()
                    .delta_since(before)
                    .is_code_source_terminal_presentation(),
                "duplicate source-terminal work: {forbidden:?}",
            );
        }
    }
}
