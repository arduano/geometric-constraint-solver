// SPDX-License-Identifier: GPL-3.0-or-later

/// Optional code-layer work crossed by one adjacent composition call.
///
/// Native interaction receipts deliberately do not contain these counters:
/// merely having a code project attached performs no code work.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CodeWorkReceipt {
    managed_parse_attempts: u64,
    expansion_attempts: u64,
    accepted_publications: u64,
}

impl CodeWorkReceipt {
    #[must_use]
    pub const fn managed_parse_attempts(self) -> u64 {
        self.managed_parse_attempts
    }

    #[must_use]
    pub const fn expansion_attempts(self) -> u64 {
        self.expansion_attempts
    }

    #[must_use]
    pub const fn accepted_publications(self) -> u64 {
        self.accepted_publications
    }

    pub fn merge(&mut self, other: Self) {
        self.managed_parse_attempts = self
            .managed_parse_attempts
            .saturating_add(other.managed_parse_attempts);
        self.expansion_attempts = self
            .expansion_attempts
            .saturating_add(other.expansion_attempts);
        self.accepted_publications = self
            .accepted_publications
            .saturating_add(other.accepted_publications);
    }

    /// Records entry into one managed-source parse boundary.
    pub fn record_managed_parse_attempt(&mut self) {
        self.managed_parse_attempts = self.managed_parse_attempts.saturating_add(1);
    }

    /// Records entry into one deterministic project-expansion boundary.
    pub fn record_expansion_attempt(&mut self) {
        self.expansion_attempts = self.expansion_attempts.saturating_add(1);
    }

    pub(crate) fn record_accepted_publication(&mut self) {
        self.accepted_publications = self.accepted_publications.saturating_add(1);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditedCodeWork<T> {
    pub outcome: T,
    pub work: CodeWorkReceipt,
}

impl<T> AuditedCodeWork<T> {
    pub(crate) const fn new(outcome: T, work: CodeWorkReceipt) -> Self {
        Self { outcome, work }
    }

    pub fn into_outcome(self) -> T {
        self.outcome
    }
}

#[cfg(test)]
mod tests {
    use super::CodeWorkReceipt;

    #[test]
    fn code_work_is_opt_in_and_saturating() {
        let mut work = CodeWorkReceipt::default();
        assert_eq!(work.expansion_attempts(), 0);
        work.record_managed_parse_attempt();
        work.record_expansion_attempt();
        work.record_accepted_publication();
        assert_eq!(work.managed_parse_attempts(), 1);
        assert_eq!(work.expansion_attempts(), 1);
        assert_eq!(work.accepted_publications(), 1);

        let mut saturated = CodeWorkReceipt {
            managed_parse_attempts: u64::MAX,
            expansion_attempts: u64::MAX,
            accepted_publications: u64::MAX,
        };
        saturated.merge(work);
        assert_eq!(saturated.managed_parse_attempts(), u64::MAX);
        assert_eq!(saturated.expansion_attempts(), u64::MAX);
        assert_eq!(saturated.accepted_publications(), u64::MAX);
    }
}
