// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

use std::fmt::Write as _;

use geosolve_constraint_editor::{LifecycleStatus, SelectionItem};
use geosolve_sketch::{
    DocumentMeasurementProvenance, DocumentParameterTarget, ExternalSnapshotInputError,
    GeometryRole, InactivityReason, OperationControl, OperationOutcome, ParameterValue,
    RetainedSketchDocumentSession, SketchAcceptedDocumentRedundancy, SketchDiagnosticSearch,
    SketchDiagnosticSearchStatus, SketchDiagnosticSnapshot, SketchDocument,
};
use geosolve_sketch_topology::{TopologyCompleteness, TopologyRequest, TopologySnapshot};

pub(crate) fn production_topology_markup(session: &RetainedSketchDocumentSession) -> String {
    let snapshot = match TopologySnapshot::capture(session) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            return format!(
                "<section class=\"wb-topology-card\" data-topology-status=\"unavailable\"><h3>Production topology</h3><p>Unavailable: {}</p><p class=\"wb-topology-scope\">Only current independently accepted geometry is consumable.</p></section>",
                escape(&error.to_string())
            );
        }
    };
    let outcome = match snapshot
        .prepare(TopologyRequest::default())
        .execute(OperationControl::default())
    {
        Ok(outcome) => outcome,
        Err(error) => {
            return format!(
                "<section class=\"wb-topology-card\" data-topology-status=\"error\"><h3>Production topology</h3><p>Query error: {}</p></section>",
                escape(&error.to_string())
            );
        }
    };
    let OperationOutcome::Completed { value, .. } = outcome else {
        let status = if matches!(outcome, OperationOutcome::Cancelled { .. }) {
            "cancelled"
        } else {
            "work-exhausted"
        };
        return format!(
            "<section class=\"wb-topology-card\" data-topology-status=\"{status}\"><h3>Production topology</h3><p>{status}</p></section>"
        );
    };
    let mut output = format!(
        "<section class=\"wb-topology-card\" data-topology-status=\"{}\" data-topology-eligible=\"{}\" data-topology-issues=\"{}\"><h3>Production topology</h3>",
        match value.completeness {
            TopologyCompleteness::Complete => "complete",
            TopologyCompleteness::Truncated => "truncated",
            TopologyCompleteness::Skipped => "skipped",
        },
        value.scope.eligible_sources.len(),
        value.issues.len(),
    );
    if let Some(profile) = value.production_profile {
        let _ = write!(
            output,
            "<p><strong>Complete</strong>: {} wire usages, {} bounded regions, exact accepted revision {}.</p><ul class=\"wb-host-list\" data-production-regions=\"true\">",
            profile.wires().len(),
            profile.regions().len(),
            profile.accepted_state_identity().revision().get(),
        );
        for region in profile.regions() {
            let _ = write!(
                output,
                "<li data-topology-region=\"{}\" data-topology-outer=\"{}\" data-topology-holes=\"{}\" data-topology-area=\"{}\">Region {} · area {} · {} hole(s)</li>",
                region.id.0,
                region.outer.0,
                region.holes.len(),
                region.area,
                region.id.0,
                region.area,
                region.holes.len(),
            );
        }
        output.push_str("</ul>");
    } else {
        let _ = write!(
            output,
            "<p><strong>{:?}</strong>: no production profile is consumable.</p><ul class=\"wb-host-list\" data-topology-issue-list=\"true\">",
            value.completeness
        );
        for issue in value.issues {
            let _ = write!(
                output,
                "<li data-topology-issue=\"{:?}\" data-topology-source-count=\"{}\">{:?} · {} source(s)</li>",
                issue.kind,
                issue.affected_sources.len(),
                issue.kind,
                issue.affected_sources.len(),
            );
        }
        output.push_str("</ul>");
    }
    output.push_str("<p class=\"wb-topology-scope\">Candidate arrangement evidence is independently checked for complete source coverage, provenance, closure and orientation.</p></section>");
    output
}

pub(crate) fn accepted_redundancy_markup(
    redundancy: Option<&SketchAcceptedDocumentRedundancy>,
) -> String {
    let Some(redundancy) = redundancy else {
        return "<p class=\"wb-empty\">Published accepted redundancy is unavailable because there is no accepted state.</p>".into();
    };
    let accepted = redundancy.accepted_state_identity();
    let design = redundancy.design_identity();
    let mut fully = redundancy.fully_redundant_sources().to_vec();
    let mut containing = redundancy.sources_containing_redundant_rows().to_vec();
    fully.sort_unstable();
    containing.sort_unstable();
    let mut output = format!(
        "<section class=\"wb-redundancy-publication\" aria-label=\"Published accepted redundancy\" data-accepted-document=\"{}\" data-accepted-revision=\"{}\" data-design-document=\"{}\" data-design-revision=\"{}\"><p><strong>Published accepted redundancy</strong> from accepted state <code>{}@{}</code> and solved design <code>{}@{}</code>.</p>",
        accepted.document(),
        accepted.revision().get(),
        design.document(),
        design.revision().get(),
        accepted.document(),
        accepted.revision().get(),
        design.document(),
        design.revision().get(),
    );
    source_list(
        &mut output,
        "fully-redundant",
        "Fully redundant sources",
        &fully,
    );
    source_list(
        &mut output,
        "contains-redundant-rows",
        "Sources containing redundant rows",
        &containing,
    );
    output.push_str("<p class=\"wb-redundancy-scope\">An empty published DTO does not prove global diagnostic completeness.</p></section>");
    output
}

pub(super) const fn lifecycle_presentation(
    status: LifecycleStatus,
) -> (&'static str, &'static str) {
    match status {
        LifecycleStatus::Accepted => ("accepted", "Accepted"),
        LifecycleStatus::DesignUnsolved => ("design-unsolved", "Design unsolved"),
        LifecycleStatus::RejectedAttempt => ("rejected-attempt", "Rejected attempt"),
        LifecycleStatus::SolvedPreview => ("solved-preview", "Solved preview"),
        LifecycleStatus::Solving => ("solving", "Solving"),
    }
}

pub(crate) fn problem_markup(problem: &str) -> String {
    format!(
        "<span class=\"wb-problem\" aria-label=\"Current solver problem\" role=\"status\">{}</span>",
        escape(problem)
    )
}

fn source_list(
    output: &mut String,
    kind: &str,
    label: &str,
    sources: &[geosolve_sketch::DocumentSourceId],
) {
    let _ = write!(
        output,
        "<div><span>{label}</span><ul data-redundancy-list=\"{kind}\">"
    );
    if sources.is_empty() {
        output.push_str("<li data-redundancy-empty=\"true\">No sources published</li>");
    } else {
        for source in sources {
            let _ = write!(
                output,
                "<li data-redundancy-kind=\"{kind}\" data-source-id=\"{source}\"><code>{source}</code></li>"
            );
        }
    }
    output.push_str("</ul></div>");
}

pub(crate) fn tree_markup(document: &SketchDocument, selection: &[SelectionItem]) -> String {
    let mut output = String::new();
    for point in document.points() {
        row(
            &mut output,
            "point",
            &point.id.to_string(),
            None,
            &point.label,
            selection.contains(&SelectionItem::Point(point.id)),
            "",
        );
    }
    for curve in document.curves() {
        if let Ok(spans) = document.curve_spans(curve.id) {
            for span in spans {
                row(
                    &mut output,
                    "curve",
                    &span.curve.to_string(),
                    Some(span.segment),
                    &curve.label,
                    selection.contains(&SelectionItem::Curve(span)),
                    match document.geometry_role(curve.id) {
                        Some(GeometryRole::Construction) => " data-role=\"construction\"",
                        _ => " data-role=\"profile\"",
                    },
                );
            }
        }
    }
    for constraint in document.constraints() {
        row(
            &mut output,
            "constraint",
            &constraint.id.to_string(),
            None,
            &constraint.label,
            selection.contains(&SelectionItem::Constraint(constraint.id)),
            "",
        );
    }
    for dimension in document.dimensions() {
        row(
            &mut output,
            "dimension",
            &dimension.id.to_string(),
            None,
            &dimension.label,
            selection.contains(&SelectionItem::Dimension(dimension.id)),
            match dimension.mode {
                geosolve_sketch::DocumentDimensionMode::Driving => {
                    " data-dimension-mode=\"driving\""
                }
                geosolve_sketch::DocumentDimensionMode::Reference => {
                    " data-dimension-mode=\"reference\""
                }
            },
        );
    }
    for binding in document.external_bindings() {
        let topology = binding
            .expected_topology
            .map_or_else(|| "none".to_owned(), |value| short_digest(value.bytes()));
        let _ = write!(
            output,
            "<div class=\"wb-tree-row wb-tree-external\" role=\"treeitem\" data-external-binding=\"{}\" data-external-kind=\"{:?}\" data-external-topology=\"{}\"><span class=\"wb-tree-icon\"></span>{}</div>",
            binding.id,
            binding.expected_kind,
            topology,
            escape(&binding.label),
        );
    }
    if output.is_empty() {
        output.push_str("<p class=\"wb-empty\">No sketch objects</p>");
    }
    output
}

fn row(
    output: &mut String,
    kind: &str,
    id: &str,
    segment: Option<u32>,
    label: &str,
    selected: bool,
    extra: &str,
) {
    let label = escape(label);
    let segment = segment.map_or_else(String::new, |value| {
        format!(" data-editor-segment=\"{value}\"")
    });
    let _ = write!(
        output,
        "<button class=\"wb-tree-row{}\" role=\"treeitem\" aria-selected=\"{}\" data-editor-item=\"{kind}\" data-persistent-id=\"{id}\"{segment}{extra}><span class=\"wb-tree-icon\"></span>{label}</button>",
        if selected { " selected" } else { "" },
        if selected { "true" } else { "false" },
    );
}

#[allow(clippy::too_many_lines)]
pub(crate) fn host_state_markup(
    session: &geosolve_sketch::RetainedSketchDocumentSession,
) -> String {
    let design = session.design_identity();
    let attempt = session.last_attempt();
    let attempt_input = attempt.input();
    let failure = attempt.failure();
    let accepted = session.accepted_state();
    let accepted_identity = accepted.map(geosolve_sketch::SketchAcceptedDocumentState::identity);
    let accepted_input = accepted.map(geosolve_sketch::SketchAcceptedDocumentState::input);
    let mut output = format!(
        "<section class=\"wb-host-card\" data-design-document=\"{}\" data-design-revision=\"{}\" data-attempt-document=\"{}\" data-attempt-revision=\"{}\" data-accepted-document=\"{}\" data-accepted-revision=\"{}\"><h3>Retained lifecycle</h3><dl><div><dt>Design</dt><dd><code>{}@{}</code></dd></div><div><dt>Latest attempt</dt><dd><code>{}@{}</code></dd></div><div><dt>Accepted</dt><dd><code>{}</code></dd></div></dl>",
        design.document(),
        design.revision().get(),
        attempt.identity().document(),
        attempt.identity().revision().get(),
        accepted_identity.map_or_else(String::new, |value| value.document().to_string()),
        accepted_identity.map_or(0, |value| value.revision().get()),
        design.document(),
        design.revision().get(),
        attempt.identity().document(),
        attempt.identity().revision().get(),
        accepted_identity.map_or_else(
            || "none".to_owned(),
            |value| format!("{}@{}", value.document(), value.revision().get())
        ),
    );
    let attempt_status = failure.map_or_else(
        || "accepted".to_owned(),
        |value| format!("{:?}: {}", value.kind(), escape(value.message())),
    );
    let _ = write!(
        output,
        "<p class=\"wb-host-status\" data-attempt-status=\"{}\">{attempt_status}</p><div class=\"wb-stamp-grid\"><span>Attempt parameters <code data-attempt-parameter-revision=\"{}\" data-attempt-parameter-digest=\"{}\">r{} · {}</code></span><span>Attempt external <code data-attempt-external-revision=\"{}\" data-attempt-external-digest=\"{}\">r{} · {}</code></span><span>Attempt activation <code data-attempt-activation-revision=\"{}\" data-attempt-activation-digest=\"{}\">r{} · {}</code></span>",
        if failure.is_some() {
            "failed"
        } else {
            "accepted"
        },
        attempt_input.parameter_revision(),
        short_digest(attempt_input.parameter_digest().bytes()),
        attempt_input.parameter_revision(),
        short_digest(attempt_input.parameter_digest().bytes()),
        attempt_input.external_snapshot_set_revision(),
        short_digest(attempt_input.external_snapshot_set_digest().bytes()),
        attempt_input.external_snapshot_set_revision(),
        short_digest(attempt_input.external_snapshot_set_digest().bytes()),
        attempt_input.effective_activation_revision(),
        short_digest(attempt_input.activation_digest().bytes()),
        attempt_input.effective_activation_revision(),
        short_digest(attempt_input.activation_digest().bytes()),
    );
    if let Some(input) = accepted_input {
        let _ = write!(
            output,
            "<span>Accepted parameters <code data-accepted-parameter-revision=\"{}\" data-accepted-parameter-digest=\"{}\">r{} · {}</code></span><span>Accepted external <code data-accepted-external-revision=\"{}\" data-accepted-external-digest=\"{}\">r{} · {}</code></span><span>Accepted activation <code data-accepted-activation-revision=\"{}\" data-accepted-activation-digest=\"{}\">r{} · {}</code></span>",
            input.parameter_revision(),
            short_digest(input.parameter_digest().bytes()),
            input.parameter_revision(),
            short_digest(input.parameter_digest().bytes()),
            input.external_snapshot_set_revision(),
            short_digest(input.external_snapshot_set_digest().bytes()),
            input.external_snapshot_set_revision(),
            short_digest(input.external_snapshot_set_digest().bytes()),
            input.effective_activation_revision(),
            short_digest(input.activation_digest().bytes()),
            input.effective_activation_revision(),
            short_digest(input.activation_digest().bytes()),
        );
    }
    output.push_str("</div></section>");
    output.push_str(&diagnostic_evidence_markup(accepted, attempt));

    let document = session.design_document();
    output.push_str(
        "<section class=\"wb-host-card\"><h3>Declared parameters</h3><ul class=\"wb-host-list\">",
    );
    for parameter in document.parameters() {
        let batch_value = session
            .parameter_batch()
            .entries()
            .iter()
            .find(|entry| entry.parameter == parameter.id)
            .map(|entry| parameter_value(entry.value));
        let (value, unit) = batch_value.unwrap_or(("unavailable".to_owned(), "none"));
        let _ = write!(
            output,
            "<li data-parameter-id=\"{}\" data-parameter-kind=\"{:?}\" data-parameter-value=\"{}\" data-parameter-unit=\"{}\"><code>{}</code> {} <span>{:?} · {} {}</span></li>",
            parameter.id,
            parameter.kind,
            value,
            unit,
            parameter.id,
            escape(&parameter.label),
            parameter.kind,
            value,
            unit,
        );
    }
    output.push_str("</ul><h3>Declared bindings</h3><ul class=\"wb-host-list\">");
    for binding in document.parameter_bindings() {
        let (target_type, target_id) = parameter_target(binding.target);
        let _ = write!(
            output,
            "<li data-binding-parameter=\"{}\" data-binding-target-type=\"{}\" data-binding-target-id=\"{}\"><code>{}</code> → <code>{}:{}</code></li>",
            binding.parameter, target_type, target_id, binding.parameter, target_type, target_id,
        );
    }
    output.push_str("</ul><h3>Accepted output proposals</h3><ul class=\"wb-host-list\" data-accepted-proposals=\"true\">");
    if let Some(accepted) = accepted {
        for proposal in accepted.parameter_output_proposals() {
            let (provenance_kind, provenance_revision) = match proposal.provenance {
                DocumentMeasurementProvenance::AcceptedDocument { revision } => {
                    ("accepted-document", revision)
                }
                DocumentMeasurementProvenance::RetainedDesign { revision } => {
                    ("retained-design", revision)
                }
            };
            let _ = write!(
                output,
                "<li data-proposal-parameter=\"{}\" data-proposal-dimension=\"{}\" data-proposal-source=\"{}\" data-proposal-unit=\"{:?}\" data-proposal-value=\"{}\" data-proposal-design-document=\"{}\" data-proposal-design-revision=\"{}\" data-proposal-attempt-document=\"{}\" data-proposal-attempt-revision=\"{}\" data-proposal-accepted-document=\"{}\" data-proposal-accepted-revision=\"{}\" data-proposal-parameter-revision=\"{}\" data-proposal-parameter-digest=\"{}\" data-proposal-provenance-kind=\"{}\" data-proposal-provenance-revision=\"{}\"><code>{}</code> = {} <span>{:?} · {}@{}</span></li>",
                proposal.parameter,
                proposal.dimension,
                proposal.source,
                proposal.unit,
                proposal.value,
                proposal.design.document(),
                proposal.design.revision().get(),
                proposal.attempt.document(),
                proposal.attempt.revision().get(),
                proposal.accepted.document(),
                proposal.accepted.revision().get(),
                proposal.parameter_revision,
                short_digest(proposal.parameter_digest.bytes()),
                provenance_kind,
                provenance_revision,
                proposal.parameter,
                proposal.value,
                proposal.unit,
                provenance_kind,
                provenance_revision,
            );
        }
    }
    output.push_str("</ul></section>");

    activity_markup(&mut output, session);
    external_markup(&mut output, session);
    if let Some(accepted) = accepted {
        let document = accepted.document();
        output.push_str(
            "<section class=\"wb-host-card\" data-accepted-geometry-roles=\"true\"><h3>Accepted geometry roles</h3><p>Declared profile participation only. Consumable regions are reported separately by Production topology.</p><ul class=\"wb-host-list\">",
        );
        for curve in document
            .curves()
            .iter()
            .filter(|curve| document.geometry_role(curve.id) != Some(GeometryRole::Construction))
        {
            if let Ok(spans) = document.curve_spans(curve.id) {
                for span in spans {
                    let _ = write!(
                        output,
                        "<li data-profile-span=\"true\" data-profile-curve=\"{}\" data-profile-segment=\"{}\"><code>{}:{}</code> {}</li>",
                        span.curve,
                        span.segment,
                        span.curve,
                        span.segment,
                        escape(&curve.label),
                    );
                }
            }
        }
        output.push_str("</ul></section>");
    }
    output
}

/// Renders the small diagnostic surface from separately provenanced retained evidence.
///
/// The attempt is identified only as the latest persisted attempt: a coordinator API error
/// can leave it unchanged, so it must never be presented as a newly created report. Numerical
/// diagnostics always come from the independently accepted state, never from attempt evidence.
fn diagnostic_evidence_markup(
    accepted: Option<&geosolve_sketch::SketchAcceptedDocumentState>,
    attempt: &geosolve_sketch::SketchDocumentAttempt,
) -> String {
    let attempt_identity = attempt.identity();
    let attempt_report = if attempt.solve_result().is_some() {
        "available"
    } else {
        "not-reported"
    };
    let mut output = format!(
        "<section class=\"wb-host-card wb-diagnostic-evidence\" data-latest-attempt-document=\"{}\" data-latest-attempt-revision=\"{}\" data-latest-attempt-report=\"{}\"><h3>Diagnostics</h3><p>Latest persisted attempt <code>{}@{}</code>: {}</p>",
        attempt_identity.document(),
        attempt_identity.revision().get(),
        attempt_report,
        attempt_identity.document(),
        attempt_identity.revision().get(),
        attempt_report,
    );
    match accepted {
        Some(accepted) => {
            let identity = accepted.identity();
            let diagnostics = accepted.diagnostics();
            let _ = write!(
                output,
                "<div data-accepted-diagnostic-provenance=\"accepted\" data-accepted-diagnostic-document=\"{}\" data-accepted-diagnostic-revision=\"{}\"><p>Accepted diagnostics from accepted state <code>{}@{}</code>.</p>{}</div>",
                identity.document(),
                identity.revision().get(),
                identity.document(),
                identity.revision().get(),
                accepted_report_markup(&diagnostics),
            );
        }
        None => output.push_str("<p data-accepted-diagnostic-provenance=\"none\">Accepted diagnostics unavailable because there is no accepted state.</p>"),
    }
    output.push_str("</section>");
    output
}

fn accepted_report_markup(diagnostics: &SketchDiagnosticSnapshot) -> String {
    let conflict = diagnostic_candidates_markup("conflict", &diagnostics.conflicts);
    let redundancy = diagnostic_candidates_markup("redundancy", &diagnostics.redundancy);
    let hard_residual = accepted_hard_residual_max(diagnostics).map_or_else(
        || "<p data-accepted-hard-residual-valid=\"false\">Validated hard residual not reported.</p>".into(),
        |value| {
            format!(
                "<p data-accepted-hard-residual=\"{value}\">Validated hard residual {value}</p>"
            )
        },
    );
    let numerical = diagnostics.rank.as_ref().map_or_else(
        || "<p data-accepted-rank-is-valid=\"false\">Numerical diagnostics not reported.</p>".into(),
        |rank| if rank.numerical_valid {
        format!(
            "<p data-accepted-rank=\"{}\" data-accepted-left-nullity=\"{}\" data-accepted-right-nullity=\"{}\" data-accepted-singularity=\"{}\">Numerical rank {} · left nullity {} · right nullity {} · singularity {}</p>",
            rank.numerical_rank.unwrap_or(0),
            rank.numerical_left_nullity.unwrap_or(0),
            rank.numerical_right_nullity.unwrap_or(0),
            rank.singular.unwrap_or(false),
            rank.numerical_rank.unwrap_or(0),
            rank.numerical_left_nullity.unwrap_or(0),
            rank.numerical_right_nullity.unwrap_or(0),
            rank.singular.unwrap_or(false),
        )
    } else {
        "<p data-accepted-rank-is-valid=\"false\">Numerical diagnostics not reported.</p>".into()
    });
    let structural = diagnostics.rank.as_ref().map_or_else(
        || "<p data-accepted-structural-rank-valid=\"false\">Structural diagnostics not reported.</p>".into(),
        |rank| format!(
            "<p data-accepted-structural-rank=\"{}\" data-accepted-structural-left-nullity=\"{}\" data-accepted-structural-right-nullity=\"{}\">Structural rank {} · left nullity {} · right nullity {} · {:?}</p>",
            rank.structural_rank,
            rank.structural_left_nullity,
            rank.structural_right_nullity,
            rank.structural_rank,
            rank.structural_left_nullity,
            rank.structural_right_nullity,
            rank.structural_classification,
        ),
    );
    let mobility = diagnostics.mobility.as_ref().map_or_else(
        || "<p data-accepted-mobility-valid=\"false\">Mobility diagnostics not reported.</p>".into(),
        |mobility| format!(
            "<p data-accepted-equality-dof=\"{}\" data-accepted-bidirectional-bounded-dof=\"{}\" data-accepted-one-sided-mobility=\"{:?}\">Equality DOF {} · bounded bidirectional DOF {} · one-sided {:?}</p>",
            mobility.equality_degrees_of_freedom.map_or_else(|| "unknown".into(), |value| value.to_string()),
            mobility.bidirectional_bounded_degrees_of_freedom.map_or_else(|| "unknown".into(), |value| value.to_string()),
            mobility.one_sided,
            mobility.equality_degrees_of_freedom.map_or_else(|| "unknown".into(), |value| value.to_string()),
            mobility.bidirectional_bounded_degrees_of_freedom.map_or_else(|| "unknown".into(), |value| value.to_string()),
            mobility.one_sided,
        ),
    );
    let repairs = format!(
        "<p data-accepted-repair-suggestions=\"{}\">Non-mutating repair suggestions: {}</p>",
        diagnostics.repair_suggestions.len(),
        diagnostics.repair_suggestions.len(),
    );
    format!("{hard_residual}{numerical}{structural}{mobility}{conflict}{redundancy}{repairs}")
}

fn accepted_hard_residual_max(diagnostics: &SketchDiagnosticSnapshot) -> Option<f64> {
    diagnostics
        .solve
        .and_then(|solve| solve.maximum_normalized_hard_residual)
        .filter(|value| value.is_finite())
}

fn diagnostic_candidates_markup(kind: &str, search: &SketchDiagnosticSearch) -> String {
    let candidates = search.candidates.len();
    let description = if candidates != 0 {
        format!("{candidates} reported ({:?})", search.status)
    } else if search.status == SketchDiagnosticSearchStatus::Complete {
        "none".into()
    } else {
        format!("not reported ({:?}/{:?})", search.status, search.reason)
    };
    format!(
        "<p data-accepted-{kind}-status=\"{:?}\" data-accepted-{kind}-candidates=\"{candidates}\">{kind}: {description}</p>",
        search.status,
    )
}

fn activity_markup(output: &mut String, session: &geosolve_sketch::RetainedSketchDocumentSession) {
    let fallback = session.design_document().effective_activity();
    let activity = session
        .last_attempt()
        .failure()
        .and_then(|failure| failure.effective_activity())
        .unwrap_or(&fallback);
    let _ = write!(
        output,
        "<section class=\"wb-host-card\" data-activation-revision=\"{}\" data-activation-digest=\"{}\"><h3>Effective activity</h3><ul class=\"wb-host-list\">",
        activity.activation_revision(),
        short_digest(activity.activation_digest().bytes()),
    );
    for entry in activity.elements() {
        let reason = activity_reason(entry.reason);
        let _ = write!(
            output,
            "<li data-activity-element=\"{}\" data-activity-state=\"{}\" data-activity-reason=\"{}\"><code>{}</code> {}</li>",
            entry.element.persistent_id(),
            if entry.is_active() {
                "active"
            } else {
                "inactive"
            },
            reason,
            entry.element.persistent_id(),
            reason,
        );
    }
    output.push_str("</ul></section>");
}

fn external_markup(output: &mut String, session: &geosolve_sketch::RetainedSketchDocumentSession) {
    let set = session.external_snapshot_set();
    let attempt = session.last_attempt();
    let attempted = attempt.input();
    let accepted = session
        .accepted_state()
        .map(geosolve_sketch::SketchAcceptedDocumentState::input);
    let external_error = attempt
        .failure()
        .and_then(|failure| failure.external_snapshot_error());
    let (failure_code, failure_binding) = external_error.map_or_else(
        || {
            if attempt.failure().is_some() {
                ("not-evaluated", None)
            } else {
                ("valid", None)
            }
        },
        external_failure,
    );
    let _ = write!(
        output,
        "<section class=\"wb-host-card\" data-attempted-external-revision=\"{}\" data-attempted-external-digest=\"{}\" data-attempted-external-status=\"{}\" data-attempted-external-failure-binding=\"{}\" data-retained-external-revision=\"{}\" data-retained-external-digest=\"{}\" data-accepted-external-revision=\"{}\" data-accepted-external-digest=\"{}\"><h3>External references</h3><p>Attempted <code>r{} · {}</code> ({})</p><p>Retained <code>r{} · {}</code></p><ul class=\"wb-host-list\">",
        attempted.external_snapshot_set_revision(),
        short_digest(attempted.external_snapshot_set_digest().bytes()),
        failure_code,
        failure_binding.map_or_else(String::new, |value| value.to_string()),
        set.revision(),
        short_digest(set.digest().bytes()),
        accepted.map_or(
            0,
            geosolve_sketch::SketchAttemptInput::external_snapshot_set_revision
        ),
        accepted.map_or_else(String::new, |input| short_digest(
            input.external_snapshot_set_digest().bytes()
        )),
        attempted.external_snapshot_set_revision(),
        short_digest(attempted.external_snapshot_set_digest().bytes()),
        failure_code,
        set.revision(),
        short_digest(set.digest().bytes()),
    );
    for binding in session.design_document().external_bindings() {
        let present = set
            .entries()
            .iter()
            .any(|entry| entry.binding == binding.id);
        let topology = binding
            .expected_topology
            .map_or_else(|| "none".to_owned(), |value| short_digest(value.bytes()));
        let _ = write!(
            output,
            "<li data-external-binding=\"{}\" data-retained-entry-status=\"{}\" data-attempted-entry-status=\"{}\" data-external-kind=\"{:?}\" data-external-topology=\"{}\"><code>{}</code> {} <span>{:?} · topology {}</span></li>",
            binding.id,
            if present { "present" } else { "missing" },
            if failure_binding == Some(binding.id) {
                failure_code
            } else if external_error.is_some() {
                "not-failing-binding"
            } else if attempt.failure().is_some() {
                "not-evaluated"
            } else {
                "valid"
            },
            binding.expected_kind,
            topology,
            binding.id,
            escape(&binding.label),
            binding.expected_kind,
            topology,
        );
    }
    output.push_str("</ul></section>");
}

fn parameter_value(value: ParameterValue) -> (String, &'static str) {
    match value {
        ParameterValue::Length(value) => (value.to_string(), "length"),
        ParameterValue::Angle(value) => (value.to_string(), "radian"),
        ParameterValue::Dimensionless(value) => (value.to_string(), "dimensionless"),
        ParameterValue::Activation(value) => (value.to_string(), "boolean"),
    }
}

fn parameter_target(target: DocumentParameterTarget) -> (&'static str, String) {
    match target {
        DocumentParameterTarget::DrivingDimension(id) => ("driving-dimension", id.to_string()),
        DocumentParameterTarget::DimensionlessFixedScalar(property) => {
            ("dimensionless-fixed-scalar", property.scalar.to_string())
        }
        DocumentParameterTarget::Activation(element) => {
            ("activation", element.persistent_id().to_string())
        }
    }
}

fn external_failure(
    error: &ExternalSnapshotInputError,
) -> (
    &'static str,
    Option<geosolve_sketch::DocumentExternalBindingId>,
) {
    match error {
        ExternalSnapshotInputError::UnsupportedVersion { .. } => ("unsupported-version", None),
        ExternalSnapshotInputError::InvalidSetRevision => ("invalid-set-revision", None),
        ExternalSnapshotInputError::InvalidSourceRevision { binding } => {
            ("invalid-source-revision", Some(*binding))
        }
        ExternalSnapshotInputError::DuplicateBinding { binding } => {
            ("duplicate-binding", Some(*binding))
        }
        ExternalSnapshotInputError::ResourceLimit { .. } => ("resource-limit", None),
        ExternalSnapshotInputError::InvalidFeature { binding, .. } => {
            ("invalid-feature", Some(*binding))
        }
        ExternalSnapshotInputError::DigestMismatch => ("digest-mismatch", None),
        ExternalSnapshotInputError::UnknownBinding { binding } => {
            ("unknown-binding", Some(*binding))
        }
        ExternalSnapshotInputError::MissingBinding { binding } => {
            ("missing-binding", Some(*binding))
        }
        ExternalSnapshotInputError::WrongKind { binding, .. } => ("wrong-kind", Some(*binding)),
        ExternalSnapshotInputError::TopologyMismatch { binding } => {
            ("topology-mismatch", Some(*binding))
        }
        ExternalSnapshotInputError::Json(_) => ("json", None),
        _ => ("other", None),
    }
}

fn activity_reason(reason: Option<InactivityReason>) -> &'static str {
    match reason {
        None => "active",
        Some(InactivityReason::UserSuppressed) => "user-suppressed",
        Some(InactivityReason::HostConfigurationInactive) => "host-configuration-inactive",
        Some(InactivityReason::UnavailableDependency { .. }) => "unavailable-dependency",
        Some(InactivityReason::UnavailableExternalReference) => "unavailable-external-reference",
    }
}

fn short_digest(bytes: [u8; 32]) -> String {
    let mut output = String::new();
    for byte in &bytes[..6] {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

pub(crate) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use geosolve_constraint_editor::{LifecycleStatus, RetainedEditorCoordinator, SelectionItem};
    use geosolve_core::SolverConfig;
    use geosolve_sketch::{
        AlphaScenarioKind, CurveDefinition, CurveSpan, DocumentConstraintDefinition,
        DocumentDimensionDefinition, DocumentDimensionMode, DocumentDirectionSense,
        DocumentElementId, DocumentExternalLineSupportRef, DocumentLineSupportRef,
        DocumentParameterKind, DocumentParameterTarget, DocumentSolveRequest,
        ExternalFeatureKindV1, ExternalLineOrientationV1, ExternalSnapshotDigest,
        ExternalSnapshotEntry, ExternalSnapshotFeatureV1, ExternalSnapshotResourcesV1,
        ExternalSnapshotSet, ExternalTopologyDigest, GeometryRole, ParameterBatch,
        ParameterBatchEntry, ParameterValue, RetainedSketchDocumentSession, ScalarDomain,
        ScalarUnit, SketchDiagnosticIncompleteReason, SketchDiagnosticSearchStatus, SketchDocument,
        alpha_scenario,
    };

    use super::{
        accepted_hard_residual_max, accepted_redundancy_markup, accepted_report_markup,
        host_state_markup, lifecycle_presentation, problem_markup, production_topology_markup,
        tree_markup,
    };

    const TOPOLOGY_A: ExternalTopologyDigest = ExternalTopologyDigest::from_bytes([0x41; 32]);
    const TOPOLOGY_B: ExternalTopologyDigest = ExternalTopologyDigest::from_bytes([0x42; 32]);

    fn make_coordinator(
        document: SketchDocument,
        parameters: ParameterBatch,
        external: ExternalSnapshotSet,
    ) -> RetainedEditorCoordinator {
        RetainedEditorCoordinator::new(
            RetainedSketchDocumentSession::new_with_inputs(
                document,
                parameters,
                external,
                DocumentSolveRequest::default(),
                SolverConfig::default(),
            )
            .unwrap(),
        )
        .unwrap()
    }

    fn line_snapshot(
        revision: u64,
        binding: geosolve_sketch::DocumentExternalBindingId,
        topology: ExternalTopologyDigest,
    ) -> ExternalSnapshotSet {
        ExternalSnapshotSet::new(
            revision,
            vec![ExternalSnapshotEntry {
                binding,
                source_revision: revision,
                source_digest: ExternalSnapshotDigest::from_bytes([0x5a; 32]),
                feature: ExternalSnapshotFeatureV1::LineSegment {
                    start: [-1.0, 0.0],
                    end: [6.0, 0.0],
                    domain: [0.0, 1.0],
                    orientation: ExternalLineOrientationV1::StartToEnd,
                    scale: 1.0,
                    topology_digest: topology,
                    resources: ExternalSnapshotResourcesV1 {
                        point_count: 2,
                        control_count: 0,
                        span_count: 1,
                    },
                },
            }],
        )
        .unwrap()
    }

    fn external_line_document() -> (
        SketchDocument,
        geosolve_sketch::DocumentExternalBindingId,
        geosolve_sketch::DocumentExternalBindingId,
    ) {
        let mut document = SketchDocument::new(8.0).unwrap();
        let start = document.add_point("start", [0.0, 0.0]).unwrap();
        let end = document.add_point("end", [4.0, 0.0]).unwrap();
        let line = document
            .add_curve(
                "line",
                CurveDefinition::Line {
                    start,
                    end,
                    branch_direction: [1.0, 0.0],
                },
            )
            .unwrap();
        let binding = document
            .add_external_binding(
                "datum line",
                ExternalFeatureKindV1::LineSegment,
                Some(TOPOLOGY_A),
            )
            .unwrap();
        let spare = document
            .add_external_binding(
                "unused datum line",
                ExternalFeatureKindV1::LineSegment,
                Some(TOPOLOGY_A),
            )
            .unwrap();
        document
            .add_constraint(
                "datum collinearity",
                DocumentConstraintDefinition::ExternalLineCollinear {
                    line: DocumentLineSupportRef {
                        span: CurveSpan::line(line),
                        direction: DocumentDirectionSense::Forward,
                    },
                    external: DocumentExternalLineSupportRef {
                        binding,
                        direction: DocumentDirectionSense::Forward,
                    },
                },
            )
            .unwrap();
        (document, binding, spare)
    }

    #[test]
    fn tree_problem_and_lifecycle_markup_preserve_typed_semantics() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let point = document.add_point("A < origin", [0.0, 0.0]).unwrap();
        let markup = tree_markup(&document, &[SelectionItem::Point(point)]);
        assert!(markup.contains("role=\"treeitem\""));
        assert!(markup.contains("aria-selected=\"true\""));
        assert!(markup.contains(&format!("data-persistent-id=\"{point}\"")));
        assert!(markup.contains("A &lt; origin"));
        assert_eq!(
            lifecycle_presentation(LifecycleStatus::RejectedAttempt),
            ("rejected-attempt", "Rejected attempt")
        );
        let problem = problem_markup("bad < geometry");
        assert!(problem.contains("aria-label=\"Current solver problem\""));
        assert!(problem.contains("role=\"status\""));
        assert!(problem.contains("bad &lt; geometry"));
    }

    #[test]
    fn accepted_redundancy_markup_preserves_provenance_sorted_sources_and_scope() {
        let mut document = SketchDocument::new(4.0).unwrap();
        let first = document.add_point("first", [0.0, 0.0]).unwrap();
        let second = document.add_point("second", [2.0, 0.0]).unwrap();
        document
            .add_constraint(
                "fix first",
                DocumentConstraintDefinition::FixedPoint {
                    point: first,
                    target: [0.0, 0.0],
                },
            )
            .unwrap();
        for label in ["first distance", "duplicate distance", "third distance"] {
            let target = document
                .add_scalar(label, 2.0, ScalarUnit::Length, ScalarDomain::Positive)
                .unwrap();
            document
                .add_dimension(
                    label,
                    DocumentDimensionDefinition::PointDistance {
                        first,
                        second,
                        target,
                    },
                    DocumentDimensionMode::Driving,
                )
                .unwrap();
        }
        let coordinator = make_coordinator(
            document,
            ParameterBatch::default(),
            ExternalSnapshotSet::default(),
        );
        let accepted = coordinator.session().accepted_state().unwrap();
        let redundancy = coordinator.accepted_redundancy().unwrap();
        let markup = accepted_redundancy_markup(Some(redundancy));
        assert!(markup.contains(&format!(
            "data-accepted-document=\"{}\" data-accepted-revision=\"{}\"",
            accepted.identity().document(),
            accepted.identity().revision().get()
        )));
        assert!(markup.contains(&format!(
            "data-design-document=\"{}\" data-design-revision=\"{}\"",
            accepted.design_identity().document(),
            accepted.design_identity().revision().get()
        )));
        for sources in [
            redundancy.fully_redundant_sources(),
            redundancy.sources_containing_redundant_rows(),
        ] {
            assert!(!sources.is_empty());
            let mut rendered_offsets = sources
                .iter()
                .map(|source| {
                    markup
                        .find(&format!("data-source-id=\"{source}\""))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            let mut sorted_sources = sources.to_vec();
            sorted_sources.sort_unstable();
            let sorted_offsets = sorted_sources
                .iter()
                .map(|source| {
                    markup
                        .find(&format!("data-source-id=\"{source}\""))
                        .unwrap()
                })
                .collect::<Vec<_>>();
            rendered_offsets.sort_unstable();
            assert_eq!(sorted_offsets, rendered_offsets);
        }
        assert!(markup.contains("does not prove global diagnostic completeness"));
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn m47_role_profile_activity_keeps_geometry_active_and_reports_reasons() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let rectangle = document
            .add_rectangle("profile", [0.0, 0.0], 4.0, 3.0)
            .unwrap();
        let role_curve = rectangle.curves[2];
        let mode_dimension = rectangle.dimensions[1];
        let mut coordinator = make_coordinator(
            document,
            ParameterBatch::default(),
            ExternalSnapshotSet::default(),
        );
        let accepted_geometry = coordinator
            .session()
            .accepted_state()
            .unwrap()
            .solve_result()
            .geometry
            .clone();
        let profile = host_state_markup(coordinator.session());
        assert!(profile.contains("data-accepted-geometry-roles=\"true\""));
        assert!(profile.contains("Declared profile participation only"));
        assert!(!profile.contains("data-profile-status="));
        assert!(profile.contains(&format!("data-profile-curve=\"{role_curve}\"")));

        let expected = coordinator.session().design_identity();
        coordinator
            .set_geometry_role(expected, role_curve, GeometryRole::Construction)
            .unwrap();
        let construction = host_state_markup(coordinator.session());
        assert!(!construction.contains(&format!("data-profile-curve=\"{role_curve}\"")));
        assert!(construction.contains(&format!(
            "data-activity-element=\"{role_curve}\" data-activity-state=\"active\""
        )));
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .unwrap()
                .solve_result()
                .geometry,
            accepted_geometry
        );

        coordinator
            .editor_mut()
            .set_selection([SelectionItem::Dimension(mode_dimension)]);
        let expected = coordinator.session().design_identity();
        coordinator.set_selected_suppressed(expected, true).unwrap();
        assert!(host_state_markup(coordinator.session()).contains("user-suppressed"));
        let expected = coordinator.session().design_identity();
        coordinator
            .set_selected_suppressed(expected, false)
            .unwrap();
        let expected = coordinator.session().design_identity();
        coordinator
            .set_dimension_mode(expected, mode_dimension, DocumentDimensionMode::Reference)
            .unwrap();
        assert!(
            tree_markup(coordinator.session().design_document(), &[])
                .contains("data-dimension-mode=\"reference\"")
        );

        let mut activity_document = SketchDocument::new(4.0).unwrap();
        let mut constraints = Vec::new();
        for index in 0..3 {
            let point = activity_document
                .add_point(format!("point {index}"), [f64::from(index), 0.0])
                .unwrap();
            let binding = activity_document
                .add_external_binding(
                    format!("binding {index}"),
                    ExternalFeatureKindV1::Point,
                    None,
                )
                .unwrap();
            constraints.push(
                activity_document
                    .add_constraint(
                        format!("constraint {index}"),
                        DocumentConstraintDefinition::ExternalPointCoincident {
                            point,
                            external: geosolve_sketch::DocumentExternalPointRef { binding },
                        },
                    )
                    .unwrap(),
            );
        }
        activity_document
            .set_element_user_suppressed(constraints[0].into(), true)
            .unwrap();
        let activation = activity_document
            .add_parameter("host activation", DocumentParameterKind::Activation)
            .unwrap();
        activity_document
            .add_parameter_binding(
                activation,
                DocumentParameterTarget::Activation(DocumentElementId::Constraint(constraints[1])),
            )
            .unwrap();
        let parameters = ParameterBatch::new(
            3,
            vec![ParameterBatchEntry {
                parameter: activation,
                value: ParameterValue::Activation(false),
            }],
        )
        .unwrap();
        let activity = make_coordinator(
            activity_document,
            parameters,
            ExternalSnapshotSet::default(),
        );
        let markup = host_state_markup(activity.session());
        for reason in [
            "user-suppressed",
            "host-configuration-inactive",
            "unavailable-external-reference",
            "unavailable-dependency",
        ] {
            assert!(markup.contains(reason), "missing activity reason {reason}");
        }
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn m47_parameter_batch_proposal_stamps_are_atomic_and_recover() {
        let mut document = SketchDocument::new(8.0).unwrap();
        let rectangle = document
            .add_rectangle("parameterized", [0.0, 0.0], 4.0, 4.0)
            .unwrap();
        let input = document
            .add_parameter("shared size", DocumentParameterKind::Length)
            .unwrap();
        for dimension in rectangle.dimensions {
            document
                .add_parameter_binding(input, DocumentParameterTarget::DrivingDimension(dimension))
                .unwrap();
        }
        let target = document
            .add_scalar("reported", 1.0, ScalarUnit::Length, ScalarDomain::Finite)
            .unwrap();
        let reference = document
            .add_dimension(
                "reported size",
                DocumentDimensionDefinition::CurveLength {
                    curve: CurveSpan::line(rectangle.curves[2]),
                    target,
                },
                DocumentDimensionMode::Reference,
            )
            .unwrap();
        let output = document
            .add_parameter("output", DocumentParameterKind::Length)
            .unwrap();
        document.add_parameter_output(output, reference).unwrap();
        let batch = |revision, value| {
            ParameterBatch::new(
                revision,
                vec![ParameterBatchEntry {
                    parameter: input,
                    value,
                }],
            )
            .unwrap()
        };
        let mut coordinator = make_coordinator(
            document,
            batch(10, ParameterValue::Length(4.0)),
            ExternalSnapshotSet::default(),
        );
        let initial = coordinator.session().accepted_state().unwrap();
        let initial_identity = initial.identity();
        let initial_proposal = initial.parameter_output_proposals()[0];
        let markup = host_state_markup(coordinator.session());
        assert_eq!(
            markup
                .matches("data-binding-target-type=\"driving-dimension\"")
                .count(),
            2
        );
        assert!(markup.contains("data-proposal-source="));

        let expected = coordinator.session().design_identity();
        coordinator
            .replace_parameter_batch(
                expected,
                batch(11, ParameterValue::Angle(4.0)),
                DocumentSolveRequest::default(),
            )
            .unwrap();
        assert!(coordinator.session().last_attempt().failure().is_some());
        assert_eq!(
            coordinator.session().accepted_state().unwrap().identity(),
            initial_identity
        );
        assert_eq!(
            coordinator
                .session()
                .accepted_state()
                .unwrap()
                .parameter_output_proposals()[0],
            initial_proposal
        );
        let failed_attempt = coordinator.session().last_attempt().identity();
        let expected = coordinator.session().design_identity();
        assert!(
            coordinator
                .replace_parameter_batch(
                    expected,
                    batch(1, ParameterValue::Length(4.0)),
                    DocumentSolveRequest::default(),
                )
                .is_err()
        );
        assert_eq!(
            coordinator.session().last_attempt().identity(),
            failed_attempt
        );
        let stale_api_error = host_state_markup(coordinator.session());
        assert!(stale_api_error.contains(&format!(
            "data-latest-attempt-document=\"{}\" data-latest-attempt-revision=\"{}\"",
            failed_attempt.document(),
            failed_attempt.revision().get(),
        )));
        assert!(stale_api_error.contains("data-latest-attempt-report=\"not-reported\""));
        assert!(stale_api_error.contains(&format!(
            "data-accepted-diagnostic-document=\"{}\" data-accepted-diagnostic-revision=\"{}\"",
            initial_identity.document(),
            initial_identity.revision().get(),
        )));
        assert!(stale_api_error.contains("Accepted diagnostics from accepted state"));
        let redundancy = accepted_redundancy_markup(coordinator.accepted_redundancy());
        assert!(redundancy.contains(&format!(
            "data-accepted-document=\"{}\" data-accepted-revision=\"{}\"",
            initial_identity.document(),
            initial_identity.revision().get(),
        )));
        assert!(redundancy.contains("No sources published"));
        assert!(redundancy.contains("does not prove global diagnostic completeness"));

        let recovery = batch(12, ParameterValue::Length(5.0));
        let expected = coordinator.session().design_identity();
        coordinator
            .replace_parameter_batch(expected, recovery.clone(), DocumentSolveRequest::default())
            .unwrap();
        let accepted = coordinator.session().accepted_state().unwrap();
        assert_ne!(accepted.identity(), initial_identity);
        assert_eq!(accepted.input().parameter_revision(), 12);
        assert_eq!(accepted.input().parameter_digest(), recovery.digest());
        assert_eq!(
            accepted.parameter_output_proposals()[0].parameter_revision,
            12
        );
        assert_eq!(
            accepted.parameter_output_proposals()[0].parameter_digest,
            recovery.digest()
        );
    }

    #[test]
    fn accepted_diagnostic_renderer_hides_invalid_rank_and_keeps_incomplete_empty_truthful() {
        let mut document = SketchDocument::new(4.0).unwrap();
        let point = document.add_point("fixed", [0.0, 0.0]).unwrap();
        document
            .add_constraint(
                "fixed",
                DocumentConstraintDefinition::FixedPoint {
                    point,
                    target: [0.0, 0.0],
                },
            )
            .unwrap();
        let coordinator = make_coordinator(
            document,
            ParameterBatch::default(),
            ExternalSnapshotSet::default(),
        );
        let mut diagnostics = coordinator
            .session()
            .accepted_state()
            .unwrap()
            .diagnostics();
        diagnostics
            .solve
            .as_mut()
            .unwrap()
            .maximum_normalized_hard_residual = None;
        let rank = diagnostics.rank.as_mut().unwrap();
        rank.numerical_valid = false;
        rank.numerical_rank = None;
        rank.numerical_left_nullity = None;
        rank.numerical_right_nullity = None;
        rank.singular = None;
        rank.near_singular = None;
        diagnostics.conflicts.candidates.clear();
        diagnostics.redundancy.candidates.clear();
        diagnostics.conflicts.status = SketchDiagnosticSearchStatus::Truncated;
        diagnostics.conflicts.reason = Some(SketchDiagnosticIncompleteReason::TrialBudget);
        diagnostics.redundancy.status = SketchDiagnosticSearchStatus::Skipped;
        diagnostics.redundancy.reason = Some(SketchDiagnosticIncompleteReason::InvalidRank);

        assert!(accepted_hard_residual_max(&diagnostics).is_none());
        let markup = accepted_report_markup(&diagnostics);
        assert!(markup.contains("data-accepted-hard-residual-valid=\"false\""));
        assert!(markup.contains("data-accepted-rank-is-valid=\"false\""));
        assert!(!markup.contains("data-accepted-rank="));
        assert!(!markup.contains("data-accepted-singularity="));
        assert!(markup.contains("data-accepted-structural-rank="));
        assert!(markup.contains("data-accepted-equality-dof="));
        assert!(markup.contains("data-accepted-bidirectional-bounded-dof="));
        assert!(markup.contains("data-accepted-one-sided-mobility="));
        assert!(markup.contains("conflict: not reported (Truncated/Some(TrialBudget))"));
        assert!(markup.contains("redundancy: not reported (Skipped/Some(InvalidRank))"));

        diagnostics.conflicts.status = SketchDiagnosticSearchStatus::Complete;
        diagnostics.conflicts.reason = None;
        let complete = accepted_report_markup(&diagnostics);
        assert!(complete.contains("conflict: none"));
    }

    #[test]
    fn m47_external_snapshot_rebind_retains_then_advances_evidence() {
        let (document, binding, spare) = external_line_document();
        let accepted_set = line_snapshot(10, binding, TOPOLOGY_A);
        let mut coordinator =
            make_coordinator(document, ParameterBatch::default(), accepted_set.clone());
        let initial_identity = coordinator.session().accepted_state().unwrap().identity();

        let missing = line_snapshot(11, spare, TOPOLOGY_A);
        let expected = coordinator.session().design_identity();
        coordinator
            .replace_external_snapshot_set(expected, missing, DocumentSolveRequest::default())
            .unwrap();
        let markup = host_state_markup(coordinator.session());
        assert!(markup.contains("data-attempted-external-revision=\"11\""));
        assert!(markup.contains("data-retained-external-revision=\"10\""));
        assert!(markup.contains("data-accepted-external-revision=\"10\""));
        assert!(markup.contains("data-attempted-external-status=\"missing-binding\""));
        assert_eq!(coordinator.session().external_snapshot_set(), &accepted_set);

        let attempt = coordinator.session().last_attempt().identity();
        let expected = coordinator.session().design_identity();
        assert!(
            coordinator
                .replace_external_snapshot_set(
                    expected,
                    line_snapshot(1, binding, TOPOLOGY_A),
                    DocumentSolveRequest::default(),
                )
                .is_err()
        );
        assert_eq!(coordinator.session().last_attempt().identity(), attempt);

        let expected = coordinator.session().design_identity();
        coordinator
            .replace_external_snapshot_set(
                expected,
                line_snapshot(12, binding, TOPOLOGY_B),
                DocumentSolveRequest::default(),
            )
            .unwrap();
        assert!(
            host_state_markup(coordinator.session())
                .contains("data-attempted-external-status=\"topology-mismatch\"")
        );
        assert_eq!(
            coordinator.session().accepted_state().unwrap().identity(),
            initial_identity
        );

        let expected = coordinator.session().design_identity();
        coordinator
            .rebind_external_binding(
                expected,
                binding,
                ExternalFeatureKindV1::LineSegment,
                Some(TOPOLOGY_B),
            )
            .unwrap();
        assert_eq!(
            coordinator.session().accepted_state().unwrap().identity(),
            initial_identity
        );
        let expected = coordinator.session().design_identity();
        coordinator
            .replace_external_snapshot_set(
                expected,
                line_snapshot(13, binding, TOPOLOGY_B),
                DocumentSolveRequest::default(),
            )
            .unwrap();
        let accepted = coordinator.session().accepted_state().unwrap();
        assert_ne!(accepted.identity(), initial_identity);
        assert_eq!(accepted.input().external_snapshot_set_revision(), 13);
        let recovered = host_state_markup(coordinator.session());
        assert!(recovered.contains("data-attempted-external-status=\"valid\""));
        assert!(recovered.contains("data-accepted-external-revision=\"13\""));
    }

    #[test]
    fn production_topology_markup_distinguishes_complete_and_open_eligible_geometry() {
        let fixture = alpha_scenario(AlphaScenarioKind::ProfileCurvedTopology, 1.0).unwrap();
        let complete = RetainedSketchDocumentSession::new(
            fixture.document,
            fixture.request,
            SolverConfig::default(),
        )
        .unwrap();
        let complete_markup = production_topology_markup(&complete);
        assert!(complete_markup.contains("data-topology-status=\"complete\""));
        assert!(complete_markup.contains("data-production-regions=\"true\""));
        assert!(complete_markup.contains("exact accepted revision"));

        let mut open = SketchDocument::new(8.0).unwrap();
        let first = open.add_point("open first", [0.0, 0.0]).unwrap();
        let second = open.add_point("open second", [4.0, 0.0]).unwrap();
        open.add_curve(
            "open eligible line",
            CurveDefinition::Line {
                start: first,
                end: second,
                branch_direction: [1.0, 0.0],
            },
        )
        .unwrap();
        let open = RetainedSketchDocumentSession::new(
            open,
            DocumentSolveRequest::default(),
            SolverConfig::default(),
        )
        .unwrap();
        let open_markup = production_topology_markup(&open);
        assert!(open_markup.contains("data-topology-status=\"skipped\""));
        assert!(open_markup.contains("UncoveredEligibleSource"));
        assert!(!open_markup.contains("data-production-regions=\"true\""));
    }
}
