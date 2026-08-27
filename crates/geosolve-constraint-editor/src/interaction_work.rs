// SPDX-License-Identifier: GPL-3.0-or-later

/// Presentation-independent evidence of semantic work crossed by one
/// interaction call.
///
/// Counts describe attempted owner boundaries, including attempts which later
/// reject. They deliberately exclude scene/SVG/DOM, persistence and code-layer
/// work, which belong to adjacent composition owners.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InteractionWorkReceipt {
    native_preview_attempts: u64,
    intent_materialization_attempts: u64,
    computed_evaluation_attempts: u64,
    history_publications: u64,
}

impl InteractionWorkReceipt {
    /// Number of native retained-preview or exact-release solve boundaries
    /// entered by the call.
    #[must_use]
    pub const fn native_preview_attempts(self) -> u64 {
        self.native_preview_attempts
    }

    /// Number of cold Intent materialization boundaries entered by the call.
    #[must_use]
    pub const fn intent_materialization_attempts(self) -> u64 {
        self.intent_materialization_attempts
    }

    /// Number of computed-feature evaluation boundaries entered by the call.
    #[must_use]
    pub const fn computed_evaluation_attempts(self) -> u64 {
        self.computed_evaluation_attempts
    }

    /// Number of durable interaction-history publications completed by the
    /// call.
    #[must_use]
    pub const fn history_publications(self) -> u64 {
        self.history_publications
    }

    /// Saturating component-wise composition for adjacent audited owners.
    pub fn merge(&mut self, other: Self) {
        self.native_preview_attempts = self
            .native_preview_attempts
            .saturating_add(other.native_preview_attempts);
        self.intent_materialization_attempts = self
            .intent_materialization_attempts
            .saturating_add(other.intent_materialization_attempts);
        self.computed_evaluation_attempts = self
            .computed_evaluation_attempts
            .saturating_add(other.computed_evaluation_attempts);
        self.history_publications = self
            .history_publications
            .saturating_add(other.history_publications);
    }

    pub(crate) fn record_native_preview_attempt(&mut self) {
        self.native_preview_attempts = self.native_preview_attempts.saturating_add(1);
    }

    pub(crate) fn record_intent_materialization_attempt(&mut self) {
        self.intent_materialization_attempts =
            self.intent_materialization_attempts.saturating_add(1);
    }

    pub(crate) fn record_computed_evaluation_attempt(&mut self) {
        self.computed_evaluation_attempts = self.computed_evaluation_attempts.saturating_add(1);
    }

    pub(crate) fn record_history_publication(&mut self) {
        self.history_publications = self.history_publications.saturating_add(1);
    }

    #[cfg(test)]
    pub(crate) const fn saturated() -> Self {
        Self {
            native_preview_attempts: u64::MAX,
            intent_materialization_attempts: u64::MAX,
            computed_evaluation_attempts: u64::MAX,
            history_publications: u64::MAX,
        }
    }
}

/// One ordinary outcome paired with exact semantic-work evidence.
///
/// `outcome` intentionally contains the `Result`: failures must retain work
/// already attempted before rejection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditedInteraction<T> {
    pub outcome: T,
    pub work: InteractionWorkReceipt,
}

impl<T> AuditedInteraction<T> {
    pub(crate) const fn new(outcome: T, work: InteractionWorkReceipt) -> Self {
        Self { outcome, work }
    }

    /// Drops the receipt while preserving the existing API outcome.
    pub fn into_outcome(self) -> T {
        self.outcome
    }
}

#[cfg(test)]
mod tests {
    use super::{AuditedInteraction, InteractionWorkReceipt};

    #[test]
    fn receipt_default_merge_and_saturation_are_deterministic() {
        let mut receipt = InteractionWorkReceipt::default();
        assert_eq!(receipt.native_preview_attempts(), 0);
        receipt.record_native_preview_attempt();
        receipt.record_intent_materialization_attempt();
        receipt.record_computed_evaluation_attempt();
        receipt.record_history_publication();

        let mut saturated = InteractionWorkReceipt::saturated();
        saturated.merge(receipt);
        assert_eq!(saturated, InteractionWorkReceipt::saturated());
    }

    #[test]
    fn failed_outcome_retains_crossed_work() {
        let mut work = InteractionWorkReceipt::default();
        work.record_native_preview_attempt();
        let audited: AuditedInteraction<Result<(), &'static str>> =
            AuditedInteraction::new(Err("rejected"), work);
        assert_eq!(audited.outcome, Err("rejected"));
        assert_eq!(audited.work.native_preview_attempts(), 1);
    }
}
