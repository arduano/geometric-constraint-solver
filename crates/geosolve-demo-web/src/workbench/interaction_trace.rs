// SPDX-License-Identifier: GPL-3.0-or-later
//! Bounded, memory-only interaction evidence for human UAT.
//!
//! This trace is deliberately separate from retained workspace authority,
//! reproduction payloads and browser persistence. It records only the latest
//! projectional canvas gesture and the publication decisions caused by that
//! gesture, so a user can copy a small diagnostic without exporting the full
//! project.

use std::collections::VecDeque;
use std::fmt::Write as _;

const TRACE_SCHEMA: &str = "GEOSOLVE_INTERACTION_TRACE_V1";
const MAX_TRACE_EVENTS: usize = 192;
pub(crate) const MAX_TRACE_EXPORT_BYTES: usize = 128 * 1024;
const MAX_TRACE_STAGE_BYTES: usize = 64;
const MAX_TRACE_DETAIL_BYTES: usize = 512;
const MAX_TRACE_CONTEXT_BYTES: usize = 2_048;
const PROTECTED_TRACE_EVENT_SLOTS: usize = 5;

#[derive(Clone, Debug, Eq, PartialEq)]
struct InteractionTraceEntry {
    sequence: u64,
    stage: String,
    detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InteractionTraceExportRow {
    stage: String,
    text: String,
}

/// One bounded trace of the latest projectional canvas gesture.
///
/// Sequence numbers remain monotonic across gestures so copied evidence can
/// reveal that the buffer was reset by a later pointer-down. When a very long
/// gesture exceeds the row bound, the first row (normally pointer-down), newest
/// row and newest terminal/rejection/rollback evidence are preserved. The
/// export reports the exact number of rows omitted by either bound.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct InteractionTrace {
    gesture: u64,
    next_sequence: u64,
    dropped_events: u64,
    events: VecDeque<InteractionTraceEntry>,
}

impl InteractionTrace {
    pub(crate) fn begin_gesture(&mut self, stage: &str, detail: impl AsRef<str>) {
        self.gesture = self.gesture.saturating_add(1);
        self.dropped_events = 0;
        self.events.clear();
        self.record(stage, detail);
    }

    pub(crate) fn record(&mut self, stage: &str, detail: impl AsRef<str>) {
        self.next_sequence = self.next_sequence.saturating_add(1);
        let entry = InteractionTraceEntry {
            sequence: self.next_sequence,
            stage: bounded_single_line(stage, MAX_TRACE_STAGE_BYTES),
            detail: bounded_single_line(detail.as_ref(), MAX_TRACE_DETAIL_BYTES),
        };
        self.events.push_back(entry);
        while self.events.len() > MAX_TRACE_EVENTS {
            // A long gesture may generate enough coalescing/preview rows to
            // outlive its terminal diagnosis. Preserve the opening row, newest
            // row and newest terminal, rejection and rollback rows separately.
            let protected =
                protected_trace_event_indices(self.events.len(), |index| &self.events[index].stage);
            let remove_at = (1..self.events.len())
                .find(|index| !protected.contains(&Some(*index)))
                .expect("the trace bound exceeds its protected evidence classes");
            let _ = self.events.remove(remove_at);
            self.dropped_events = self.dropped_events.saturating_add(1);
        }
    }

    #[must_use]
    pub(crate) fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Exports line-oriented, bounded evidence intended for direct pasting in
    /// a defect report. `context` is caller-produced current authority identity;
    /// it must not contain source text or a reproduction payload.
    #[must_use]
    pub(crate) fn export(&self, context: &str, notice: &str) -> String {
        let context = bounded_single_line(context, MAX_TRACE_CONTEXT_BYTES);
        let notice = bounded_single_line(notice, MAX_TRACE_DETAIL_BYTES);
        // Re-sanitize at the export boundary as a defensive invariant. Normal
        // callers can only create already-bounded rows through `record`.
        let rows = self
            .events
            .iter()
            .map(|event| {
                let stage = bounded_single_line(&event.stage, MAX_TRACE_STAGE_BYTES);
                let detail = bounded_single_line(&event.detail, MAX_TRACE_DETAIL_BYTES);
                InteractionTraceExportRow {
                    text: format!("{}\t{stage}\t{detail}\n", event.sequence),
                    stage,
                }
            })
            .collect::<Vec<_>>();
        let protected = protected_trace_event_indices(rows.len(), |index| &rows[index].stage);
        let mut selected = vec![false; rows.len()];

        // Reserve the widest possible numeric header before admitting rows.
        // The final header can only be shorter, so the returned String has a
        // hard byte bound even if an internal caller violates the row-count
        // invariant in a future change.
        let reserved_header_bytes =
            trace_export_header(u64::MAX, usize::MAX, u64::MAX, &context, &notice).len();
        let mut selected_bytes = reserved_header_bytes;
        for &index in protected.iter().flatten() {
            if !selected[index] {
                selected_bytes = selected_bytes
                    .checked_add(rows[index].text.len())
                    .expect("bounded interaction trace byte count");
                assert!(
                    selected_bytes <= MAX_TRACE_EXPORT_BYTES,
                    "protected interaction evidence must fit the export bound"
                );
                selected[index] = true;
            }
        }
        // Fill remaining space newest-first, then serialize chronologically.
        for index in (0..rows.len()).rev() {
            if selected[index] {
                continue;
            }
            let Some(candidate_bytes) = selected_bytes.checked_add(rows[index].text.len()) else {
                continue;
            };
            if candidate_bytes <= MAX_TRACE_EXPORT_BYTES {
                selected[index] = true;
                selected_bytes = candidate_bytes;
            }
        }

        let selected_events = selected.iter().filter(|selected| **selected).count();
        let export_dropped = rows.len().saturating_sub(selected_events);
        let dropped_events = self
            .dropped_events
            .saturating_add(u64::try_from(export_dropped).unwrap_or(u64::MAX));
        let mut output = trace_export_header(
            self.gesture,
            selected_events,
            dropped_events,
            &context,
            &notice,
        );
        for (row, selected) in rows.iter().zip(selected) {
            if selected {
                output.push_str(&row.text);
            }
        }
        assert!(output.len() <= MAX_TRACE_EXPORT_BYTES);
        output
    }
}

fn trace_export_header(
    gesture: u64,
    events: usize,
    dropped_events: u64,
    context: &str,
    notice: &str,
) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "{TRACE_SCHEMA}");
    let _ = writeln!(output, "package={}", env!("CARGO_PKG_VERSION"));
    let _ = writeln!(output, "gesture={gesture}");
    let _ = writeln!(output, "events={events}");
    let _ = writeln!(output, "dropped_events={dropped_events}");
    let _ = writeln!(output, "context={context}");
    let _ = writeln!(output, "notice={notice}");
    output.push_str("sequence\tstage\tdetail\n");
    output
}

fn protected_trace_event_indices<'a>(
    event_count: usize,
    mut stage_at: impl FnMut(usize) -> &'a str,
) -> [Option<usize>; PROTECTED_TRACE_EVENT_SLOTS] {
    let opening = (event_count > 0).then_some(0);
    let newest = event_count.checked_sub(1);
    let newest_terminal = (0..event_count)
        .rev()
        .find(|index| is_terminal_trace_stage(stage_at(*index)));
    let newest_rejection = (0..event_count)
        .rev()
        .find(|index| is_rejection_trace_stage(stage_at(*index)));
    let newest_rollback = (0..event_count)
        .rev()
        .find(|index| is_rollback_trace_stage(stage_at(*index)));
    [
        opening,
        newest,
        newest_terminal,
        newest_rejection,
        newest_rollback,
    ]
}

fn is_terminal_trace_stage(stage: &str) -> bool {
    stage.contains("terminal") || stage.contains("pointerup") || stage.contains("pointercancel")
}

fn is_rejection_trace_stage(stage: &str) -> bool {
    stage.contains("reject") || stage.contains("abort")
}

fn is_rollback_trace_stage(stage: &str) -> bool {
    stage.contains("rollback") || stage.contains("restore") || stage.contains("cancel")
}

fn bounded_single_line(value: &str, maximum_bytes: usize) -> String {
    let mut normalized = String::with_capacity(value.len().min(maximum_bytes));
    let mut truncated = false;
    for character in value.chars() {
        let character = if character.is_control()
            || matches!(character, '\t' | '\n' | '\r' | '\u{2028}' | '\u{2029}')
        {
            ' '
        } else {
            character
        };
        if normalized.len() + character.len_utf8() > maximum_bytes {
            truncated = true;
            break;
        }
        normalized.push(character);
    }
    if truncated && maximum_bytes >= '…'.len_utf8() {
        while normalized.len() + '…'.len_utf8() > maximum_bytes {
            let _ = normalized.pop();
        }
        normalized.push('…');
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::{
        InteractionTrace, InteractionTraceEntry, MAX_TRACE_CONTEXT_BYTES, MAX_TRACE_DETAIL_BYTES,
        MAX_TRACE_EVENTS, MAX_TRACE_EXPORT_BYTES, MAX_TRACE_STAGE_BYTES, bounded_single_line,
    };

    #[test]
    fn trace_keeps_pointer_down_and_newest_terminal_rejection_and_rollback_evidence() {
        let mut trace = InteractionTrace::default();
        trace.begin_gesture("browser.pointerdown", "pointer=83 model=[0,40]");
        trace.record("terminal.replay.accepted", "old-terminal");
        trace.record("parity.reject", "old-rejection");
        trace.record("code.publication.restore", "old-rollback");
        trace.record("terminal.replay.accepted", "new-terminal");
        trace.record("parity.reject", "new-rejection");
        trace.record("code.publication.restore", "new-rollback");
        for ordinal in 0..MAX_TRACE_EVENTS + 17 {
            trace.record("browser.pointermove", format!("ordinal={ordinal}"));
        }

        let exported = trace.export("project=typed-panel", "accepted authority restored");
        assert!(exported.contains("\tbrowser.pointerdown\tpointer=83 model=[0,40]"));
        assert!(exported.contains("\tterminal.replay.accepted\tnew-terminal"));
        assert!(exported.contains("\tparity.reject\tnew-rejection"));
        assert!(exported.contains("\tcode.publication.restore\tnew-rollback"));
        assert!(!exported.contains("old-terminal"));
        assert!(!exported.contains("old-rejection"));
        assert!(!exported.contains("old-rollback"));
        assert!(exported.contains("dropped_events=24"));
        assert_eq!(exported.lines().count(), MAX_TRACE_EVENTS + 8);

        let terminal = exported.find("new-terminal").unwrap();
        let rejection = exported.find("new-rejection").unwrap();
        let rollback = exported.find("new-rollback").unwrap();
        assert!(terminal < rejection && rejection < rollback);
    }

    #[test]
    fn trace_is_single_line_bounded_and_resets_only_on_the_next_gesture() {
        let mut trace = InteractionTrace::default();
        trace.begin_gesture("pointer\tdown", "first\nline");
        trace.record("terminal", "kept");
        let first = trace.export("context\nrow", "notice\trow");
        assert!(first.contains("pointer down\tfirst line"));
        assert!(first.contains("context=context row"));
        assert!(first.contains("notice=notice row"));

        trace.begin_gesture("browser.pointerdown", "second");
        let second = trace.export("project=typed-panel", "ready");
        assert!(!second.contains("\tterminal\tkept"));
        assert!(second.contains("gesture=2"));
        assert!(second.contains("\tbrowser.pointerdown\tsecond"));
    }

    #[test]
    fn maximum_public_inputs_and_large_sequences_stay_below_the_export_bound() {
        let mut trace = InteractionTrace {
            next_sequence: u64::MAX - 1_000,
            ..InteractionTrace::default()
        };
        let stage = "s".repeat(MAX_TRACE_STAGE_BYTES + 1);
        let detail = "d".repeat(MAX_TRACE_DETAIL_BYTES + 1);
        trace.begin_gesture(&stage, &detail);
        for _ in 1..MAX_TRACE_EVENTS * 2 {
            trace.record(&stage, &detail);
        }

        let exported = trace.export(
            &"c".repeat(MAX_TRACE_CONTEXT_BYTES + 1),
            &"n".repeat(MAX_TRACE_DETAIL_BYTES + 1),
        );
        assert!(exported.len() <= MAX_TRACE_EXPORT_BYTES);
        assert!(exported.contains(&format!("events={MAX_TRACE_EVENTS}")));
        assert!(exported.contains("184467440737095"));
        assert!(exported.is_char_boundary(exported.len()));
    }

    #[test]
    fn export_defensively_prunes_extra_internal_rows_and_counts_every_drop() {
        let mut trace = InteractionTrace {
            gesture: 9,
            dropped_events: 7,
            ..InteractionTrace::default()
        };
        let total_events = MAX_TRACE_EVENTS * 4;
        for sequence in 0..total_events {
            let (stage, marker) = match sequence {
                0 => ("browser.pointerdown", "opening"),
                1 => ("terminal.replay.accepted", "terminal-evidence"),
                2 => ("parity.reject", "rejection-evidence"),
                3 => ("code.publication.restore", "rollback-evidence"),
                _ => ("browser.pointermove", "ordinary"),
            };
            trace.events.push_back(InteractionTraceEntry {
                sequence: u64::try_from(sequence).unwrap(),
                stage: stage.into(),
                detail: format!("{marker}-{}", "x".repeat(MAX_TRACE_DETAIL_BYTES * 4)),
            });
        }

        let exported = trace.export("project=typed-panel", "current notice");
        let exported_events = header_number(&exported, "events");
        let dropped_events = header_number(&exported, "dropped_events");
        assert!(exported.len() <= MAX_TRACE_EXPORT_BYTES);
        assert!(exported_events < total_events as u64);
        assert_eq!(exported_events + dropped_events, total_events as u64 + 7);
        assert!(exported.contains("\tbrowser.pointerdown\topening-"));
        assert!(exported.contains("\tterminal.replay.accepted\tterminal-evidence-"));
        assert!(exported.contains("\tparity.reject\trejection-evidence-"));
        assert!(exported.contains("\tcode.publication.restore\trollback-evidence-"));
    }

    #[test]
    fn sanitization_removes_line_controls_and_truncates_utf8_on_char_boundaries() {
        let sanitized = bounded_single_line(
            "alpha\tbeta\ngamma\r\u{0}delta\u{2028}epsilon\u{2029}omega",
            128,
        );
        assert!(!sanitized.chars().any(char::is_control));
        assert!(!sanitized.contains(['\t', '\n', '\r', '\u{2028}', '\u{2029}']));

        let multibyte = bounded_single_line("ééééé", 8);
        assert_eq!(multibyte, "éé…");
        assert!(multibyte.len() <= 8);
        assert!(multibyte.is_char_boundary(multibyte.len()));
        assert_eq!(bounded_single_line("😀😀", 2), "");
        assert_eq!(bounded_single_line("\u{85}", 8), " ");
    }

    #[test]
    fn trace_is_memory_only_and_repeated_export_does_not_mutate_it() {
        let mut trace = InteractionTrace::default();
        assert!(trace.is_empty());
        let fresh = trace.clone();
        let empty_export = trace.export("project=typed-panel", "not yet recorded");
        assert_eq!(trace, fresh);
        assert!(trace.is_empty());
        assert_eq!(
            empty_export,
            trace.export("project=typed-panel", "not yet recorded")
        );

        trace.record("browser.pointerdown", "pointer=83");
        let recorded = trace.clone();
        let first = trace.export("project=typed-panel", "ready");
        let second = trace.export("project=typed-panel", "ready");
        assert_eq!(first, second);
        assert_eq!(trace, recorded);
        assert!(!trace.is_empty());
    }

    fn header_number(exported: &str, name: &str) -> u64 {
        exported
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{name}=")))
            .expect("trace header field")
            .parse()
            .expect("numeric trace header field")
    }
}
