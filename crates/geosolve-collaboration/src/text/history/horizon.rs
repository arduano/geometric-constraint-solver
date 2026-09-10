// SPDX-License-Identifier: GPL-3.0-or-later
//! Capacity shortens a validated personal Undo suffix, never stops raw typing.
use super::{
    HistoryAction, MAX_CONTRIBUTIONS, MAX_EVENTS, OperationId, SharedTextDocument, SharedTextError,
    TextContributionHistory, TextEdit, TextRevision,
};
use crate::protocol::MAX_REVISION;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextHistoryHorizon {
    pub generation: u64,
    pub discarded_events: u64,
    pub oldest_revision: Option<TextRevision>,
}
impl TextHistoryHorizon {
    pub fn is_empty(&self) -> bool {
        self.generation == 0 && self.discarded_events == 0 && self.oldest_revision.is_none()
    }
}
impl TextContributionHistory {
    pub fn horizon(&self) -> TextHistoryHorizon {
        let mut horizon = self.horizon.clone();
        if let Some(event) = self.events.first() {
            horizon.oldest_revision = Some(event.before.clone());
        }
        horizon
    }
    pub fn checkpoint_horizon(&self) -> &TextHistoryHorizon {
        &self.horizon
    }
    /// Restore the retained suffix and its explicitly declared history horizon.
    /// The surrounding source envelope still requires authenticated durable storage.
    /// # Errors
    /// Rejects invalid frontier/counters, suffix ancestry and replay discrepancies.
    pub fn restore_with_horizon(
        events: Vec<super::TextHistoryEvent>,
        horizon: TextHistoryHorizon,
        working: &SharedTextDocument,
    ) -> Result<Self, SharedTextError> {
        if horizon.generation > MAX_REVISION
            || horizon.discarded_events > MAX_REVISION
            || horizon.generation > horizon.discarded_events
            || (horizon.generation == 0 && !horizon.is_empty())
            || (horizon.generation > 0 && horizon.oldest_revision.is_none())
        {
            return Err(SharedTextError::UndoUnavailable);
        }
        if let Some(frontier) = &horizon.oldest_revision {
            working.validate_revision(frontier)?;
            if let Some(first) = events.first() {
                super::historical(working, &first.before, &first.actor)?
                    .validate_revision(frontier)?;
            }
        }
        let mut history = Self::restore(events, working)?;
        history.horizon = horizon;
        Ok(history)
    }
    #[allow(clippy::needless_pass_by_value)] // Own admission metadata across bounded retry composition.
    pub(super) fn retain_new(
        &mut self,
        operation: OperationId,
        before: &SharedTextDocument,
        after: &SharedTextDocument,
        actor: Vec<u8>,
        files: bool,
        edits: Option<&[TextEdit]>,
    ) -> Result<(), SharedTextError> {
        self.check_operation(&operation)?;
        let record = |candidate: &mut Self| {
            if let Some(edits) = edits {
                candidate.record_file_edits_exact(operation.clone(), before, after, edits)
            } else {
                candidate.record_exact(operation.clone(), before, after, actor.clone(), files)
            }
        };
        let mut candidate = self.clone();
        match record(&mut candidate) {
            Ok(()) => {}
            Err(error) if history_limit(&error) => {
                candidate = self.prune_recent(before)?;
                match record(&mut candidate) {
                    Ok(()) => {}
                    Err(error) if history_limit(&error) => {
                        // The one contribution alone exceeds retained Undo size.
                        // Preserve its valid raw text, drop crossing inverse plans,
                        // and announce an explicit history boundary.
                        candidate = self.clear_boundary(after, 1)?;
                    }
                    Err(error) => return Err(error),
                }
            }
            Err(error) => return Err(error),
        }
        *self = candidate;
        Ok(())
    }
    pub(super) fn needs_pruning(&self) -> bool {
        self.events.len() > MAX_EVENTS
            || self.contributions.len() > MAX_CONTRIBUTIONS
            || self.check_size().is_err()
    }
    pub(super) fn prune_recent(
        &self,
        working: &SharedTextDocument,
    ) -> Result<Self, SharedTextError> {
        if self.events.is_empty() {
            return self.clear_boundary(working, 0);
        }
        let mut cut = self.events.len() / 2;
        if cut == 0 {
            cut = 1;
        }
        // Every inverse in the retained suffix must still have its own original
        // contribution. Move the cut forward past an orphaned inverse, never
        // fabricate its token or silently reinterpret it as another edit.
        loop {
            let mut originals = BTreeSet::new();
            let mut next = None;
            for (index, event) in self.events.iter().enumerate().skip(cut) {
                match &event.action {
                    HistoryAction::Text | HistoryAction::Files => {
                        originals.insert(&event.operation);
                    }
                    HistoryAction::Undo { contribution } | HistoryAction::Redo { contribution } => {
                        if !originals.contains(contribution) {
                            next = Some(index + 1);
                            break;
                        }
                    }
                }
            }
            match next {
                Some(next) => cut = next,
                None => break,
            }
        }
        if cut >= self.events.len() {
            return self.clear_boundary(working, 0);
        }
        let retained = self.events[cut..].to_vec();
        let reconstructed = self
            .ordinary_suffix(cut)
            .map_or_else(|| Self::restore(retained, working), Ok);
        let mut candidate = match reconstructed {
            Ok(candidate) if !candidate.needs_pruning() => candidate,
            // A remaining inverse may rely on pre-horizon restoration lineage.
            // Keep at least the newest ordinary contribution when it can be
            // independently reconstructed, otherwise move to the current frontier.
            _ => {
                let last = self.events.last().ok_or(SharedTextError::UndoUnavailable)?;
                if matches!(last.action, HistoryAction::Text | HistoryAction::Files) {
                    match Self::restore(vec![last.clone()], working) {
                        Ok(candidate) => {
                            cut = self.events.len() - 1;
                            candidate
                        }
                        Err(_) => return self.clear_boundary(working, 0),
                    }
                } else {
                    return self.clear_boundary(working, 0);
                }
            }
        };
        candidate.horizon = self.advance_horizon(
            cut as u64,
            candidate
                .events
                .first()
                .map_or_else(|| working.revision(), |event| event.before.clone()),
        )?;
        Ok(candidate)
    }
    /// Ordinary contributions already contain native-validated deltas against
    /// their own before/after heads. If none has since been inverted, their
    /// actual character/file IDs need no earlier restoration aliases. Selecting
    /// that trusted suffix is equivalent to replay, without synchronously
    /// rebuilding hundreds of native historical documents during typing.
    /// Checkpoint restoration still independently reconstructs every event.
    fn ordinary_suffix(&self, cut: usize) -> Option<Self> {
        let events = &self.events[cut..];
        if events
            .iter()
            .any(|event| !matches!(event.action, HistoryAction::Text | HistoryAction::Files))
        {
            return None;
        }
        let operations = events
            .iter()
            .map(|event| event.operation.clone())
            .collect::<BTreeSet<_>>();
        let contributions = self
            .contributions
            .iter()
            .filter(|entry| operations.contains(&entry.operation))
            .cloned()
            .collect::<Vec<_>>();
        if contributions.len() != events.len() || contributions.iter().any(|entry| !entry.active) {
            return None;
        }
        Some(Self {
            events: events.to_vec(),
            contributions,
            operations,
            ..Self::default()
        })
    }
    fn clear_boundary(
        &self,
        working: &SharedTextDocument,
        extra_events: u64,
    ) -> Result<Self, SharedTextError> {
        let discarded = (self.events.len() as u64)
            .checked_add(extra_events)
            .ok_or(SharedTextError::ResourceLimit("text history horizon"))?;
        if discarded == 0 {
            return Ok(self.clone());
        }
        Ok(Self {
            horizon: self.advance_horizon(discarded, working.revision())?,
            ..Self::default()
        })
    }
    fn advance_horizon(
        &self,
        discarded: u64,
        revision: TextRevision,
    ) -> Result<TextHistoryHorizon, SharedTextError> {
        let increment = |value: u64, amount: u64| {
            value
                .checked_add(amount)
                .filter(|value| *value <= MAX_REVISION)
                .ok_or(SharedTextError::ResourceLimit("text history horizon"))
        };
        Ok(TextHistoryHorizon {
            generation: increment(self.horizon.generation, 1)?,
            discarded_events: increment(self.horizon.discarded_events, discarded)?,
            oldest_revision: Some(revision),
        })
    }
}
fn history_limit(error: &SharedTextError) -> bool {
    matches!(
        error,
        SharedTextError::ResourceLimit(
            "text contributions"
                | "text history events"
                | "personal text history bytes"
                | "personal text span scalars"
                | "personal text contribution bytes"
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SharedTextLimits;
    fn operation(user: &str, index: usize) -> OperationId {
        OperationId {
            user_id: user.into(),
            client_id: format!("tab-{user}"),
            request_id: format!("edit-{index}"),
        }
    }
    fn record(
        history: &mut TextContributionHistory,
        working: &mut SharedTextDocument,
        user: &str,
        index: usize,
        offset: usize,
        insert: &str,
    ) {
        let before = working.clone();
        let mut client = working.fork(user.as_bytes()).unwrap();
        client.splice("main.ts", offset, 1, insert).unwrap();
        working.merge(&client).unwrap();
        history
            .record(
                operation(user, index),
                &before,
                working,
                user.as_bytes().to_vec(),
                false,
            )
            .unwrap();
    }
    #[test]
    fn pruning_an_orphaned_inverse_advances_horizon_without_forging_history() {
        let mut working = SharedTextDocument::new(b"server", SharedTextLimits::default()).unwrap();
        working.create_file("main.ts", "a0b0").unwrap();
        let mut history = TextContributionHistory::default();
        record(&mut history, &mut working, "bob", 0, 1, "1");
        for index in 0..8 {
            record(
                &mut history,
                &mut working,
                "alice",
                index,
                3,
                if index % 2 == 0 { "1" } else { "0" },
            );
        }
        history
            .inverse(operation("bob", 1), &mut working, false)
            .unwrap();
        let before = working.capture();
        let pruned = history.prune_recent(&working).unwrap();
        assert_eq!(working.capture(), before);
        assert!(pruned.events.is_empty());
        assert_eq!(pruned.horizon.generation, 1);
        let restored = TextContributionHistory::restore_with_horizon(
            pruned.events.clone(),
            pruned.horizon.clone(),
            &working,
        )
        .unwrap();
        assert!(!restored.user_history("bob", &working).can_redo);
    }
    #[test]
    fn retained_suffix_preserves_foreign_same_value_barriers_and_lineage() {
        let mut working = SharedTextDocument::new(b"server", SharedTextLimits::default()).unwrap();
        working.create_file("main.ts", "a0b0").unwrap();
        let mut history = TextContributionHistory::default();
        for index in 0..8 {
            record(
                &mut history,
                &mut working,
                "carol",
                index,
                3,
                if index % 2 == 0 { "1" } else { "0" },
            );
        }
        record(&mut history, &mut working, "alice", 0, 1, "1");
        record(&mut history, &mut working, "bob", 0, 1, "1");
        let mut pruned = history.prune_recent(&working).unwrap();
        assert!(!pruned.user_history("alice", &working).can_undo);
        assert!(pruned.user_history("bob", &working).can_undo);
        pruned
            .inverse(operation("bob", 1), &mut working, false)
            .unwrap();
        assert!(pruned.user_history("alice", &working).can_undo);
        let mut restored = TextContributionHistory::restore_with_horizon(
            pruned.events.clone(),
            pruned.horizon.clone(),
            &working,
        )
        .unwrap();
        restored
            .inverse(operation("alice", 1), &mut working, false)
            .unwrap();
        assert_eq!(working.capture().text("main.ts"), Some("a0b0"));
    }
    #[test]
    fn ordinary_suffix_after_restoration_matches_independent_replay_for_every_inverse() {
        let mut working = SharedTextDocument::new(b"server", SharedTextLimits::default()).unwrap();
        working.create_file("main.ts", "a0b0").unwrap();
        let mut history = TextContributionHistory::default();
        record(&mut history, &mut working, "alice", 0, 1, "1");
        history
            .inverse(operation("alice", 1), &mut working, false)
            .unwrap();
        // Retained ordinary deltas reference the restored native characters,
        // even though the older history still contains their restoration alias.
        assert!(!history.lineage.atoms.is_empty());
        for index in 2..12 {
            record(
                &mut history,
                &mut working,
                "alice",
                index,
                1,
                if index % 2 == 0 { "1" } else { "0" },
            );
        }
        let mut pruned = history.prune_recent(&working).unwrap();
        assert!(pruned.lineage.atoms.is_empty());
        let mut replayed = TextContributionHistory::restore_with_horizon(
            pruned.events.clone(),
            pruned.horizon.clone(),
            &working,
        )
        .unwrap();
        let mut replayed_working = working.clone();
        let retained = pruned.contributions.len();
        for index in 0..retained {
            let inverse = operation("alice", index + 20);
            pruned
                .inverse(inverse.clone(), &mut working, false)
                .unwrap();
            replayed
                .inverse(inverse, &mut replayed_working, false)
                .unwrap();
            assert_eq!(working.capture(), replayed_working.capture());
            assert_eq!(working.revision(), replayed_working.revision());
        }
        assert!(!pruned.user_history("alice", &working).can_undo);
        assert_eq!(pruned.events, replayed.events);
    }
}
