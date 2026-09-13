// SPDX-License-Identifier: GPL-3.0-or-later
//! Existing source-workspace transport shared by standalone and filesystem hosts.
#![allow(
    clippy::missing_errors_doc,
    reason = "codec errors identify the rejected source, history, diagnostic or transport authority"
)]
use crate::{
    CodeProject, KeyedReconcileState, ManagedDiagnostic, ManagedSpan, SketchCodeSession,
    required_generated_members,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CODE_WORKBENCH_WIRE_VERSION: &str = "geosolve-code-workbench-v5";
pub const LEGACY_CODE_WORKBENCH_WIRE_VERSION: &str = "geosolve-code-workbench-v4";
const MAX_MANAGED_DRAFT_DIAGNOSTIC_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SourceWorkspaceOrigin {
    Bundled { sample: String },
    Authored,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CodeProjectWorkbenchWire {
    version: String,
    origin: SourceWorkspaceOrigin,
    project: String,
    // V4 stores canonical session JSON directly. V5 transports those exact
    // bytes through the bounded reproduction codec; the project/source remain
    // readable without decompressing history or delegated editor checkpoints.
    session: String,
    selected_file: String,
    managed_draft: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    draft_diagnostic: Option<ManagedDiagnostic>,
}

fn decode_code_workbench_session(version: &str, session: String) -> Result<String, String> {
    match version {
        LEGACY_CODE_WORKBENCH_WIRE_VERSION => Ok(session),
        CODE_WORKBENCH_WIRE_VERSION => {
            geosolve_constraint_editor::reproduction::decode_workspace(&session)
                .map_err(|error| format!("invalid compressed code session: {error}"))
        }
        _ => Err("unsupported code-workbench version".into()),
    }
}

/// Personal source state carried beside source/native history in the existing v4/v5 wire.
/// None of these fields grants native edit or accepted-source authority.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceWorkspacePresentation {
    pub origin: SourceWorkspaceOrigin,
    pub selected_file: String,
    pub managed_draft: String,
    pub draft_diagnostic: Option<ManagedDiagnostic>,
}

/// Validated source/history plus host-owned draft, origin and file selection.
#[derive(Debug)]
pub struct SourceWorkspace {
    pub origin: SourceWorkspaceOrigin,
    pub project: CodeProject,
    pub session: crate::editor_checkpoint::ValidatedSourceHistory,
    pub selected_file: String,
    pub managed_draft: String,
    pub draft_diagnostic: Option<ManagedDiagnostic>,
}

/// Encodes the unchanged v5 wire, preserving every native history checkpoint.
pub fn encode_source_workspace(
    origin: &SourceWorkspaceOrigin,
    project: &CodeProject,
    session: &SketchCodeSession,
    selected_file: &str,
    managed_draft: &str,
    draft_diagnostic: Option<&ManagedDiagnostic>,
) -> Result<String, String> {
    validate_managed_draft_bound(managed_draft)?;
    let session_json = session
        .to_canonical_json()
        .map_err(|error| error.to_string())?;
    let session = geosolve_constraint_editor::reproduction::encode_workspace(&session_json)
        .map_err(|error| format!("cannot encode code session: {error}"))?;
    let wire = CodeProjectWorkbenchWire {
        version: CODE_WORKBENCH_WIRE_VERSION.into(),
        origin: origin.clone(),
        project: project
            .to_canonical_json()
            .map_err(|error| error.to_string())?,
        session,
        selected_file: selected_file.into(),
        managed_draft: managed_draft.into(),
        draft_diagnostic: draft_diagnostic.cloned(),
    };
    let json = serde_json::to_string(&wire).map_err(|error| error.to_string())?;
    validate_workspace_bound(&json)?;
    Ok(json)
}

/// Decodes v4/v5 through complete source and independent native checkpoint validation.
pub fn decode_source_workspace(json: &str) -> Result<SourceWorkspace, String> {
    validate_workspace_bound(json)?;
    let wire: CodeProjectWorkbenchWire =
        serde_json::from_str(json).map_err(|error| error.to_string())?;
    let session_json = decode_code_workbench_session(&wire.version, wire.session)?;
    validate_managed_draft_bound(&wire.managed_draft)?;
    let project = CodeProject::from_json(&wire.project).map_err(|error| error.to_string())?;
    let session = crate::editor_checkpoint::ValidatedSourceHistory::decode(&session_json)?;
    validate_source_workspace_history(&project, session.history())?;
    validate_source_workspace_presentation(
        &project,
        &SourceWorkspacePresentation {
            origin: wire.origin.clone(),
            selected_file: wire.selected_file.clone(),
            managed_draft: wire.managed_draft.clone(),
            draft_diagnostic: wire.draft_diagnostic.clone(),
        },
    )?;
    let draft_diagnostic = wire.draft_diagnostic;
    Ok(SourceWorkspace {
        origin: wire.origin,
        project,
        session,
        selected_file: wire.selected_file,
        managed_draft: wire.managed_draft,
        draft_diagnostic,
    })
}

fn validate_workspace_bound(json: &str) -> Result<(), String> {
    if json.len() > crate::CODE_PROJECT_LIMIT {
        return Err(format!(
            "code workbench is {} bytes; the limit is {}",
            json.len(),
            crate::CODE_PROJECT_LIMIT
        ));
    }
    Ok(())
}

/// Authenticates current/accepted source and generated ownership against unified history.
pub fn validate_source_workspace_history(
    project: &CodeProject,
    session: &SketchCodeSession,
) -> Result<(), String> {
    if session.snapshot().project != project.project
        || session.snapshot().managed != project.managed
        || session.snapshot().code_project.as_ref() != Some(project)
        || session.snapshot().artifact_digests != artifact_digests(project)?
    {
        return Err("code project and unified session checkpoints disagree".into());
    }
    let snapshot = session.snapshot();
    let accepted_project = snapshot
        .accepted_code_project
        .as_ref()
        .ok_or_else(|| "code session has no accepted project authority".to_owned())?;
    let accepted_generated = snapshot
        .accepted_generated
        .as_ref()
        .ok_or_else(|| "code session has no accepted generated authority".to_owned())?;
    validate_generated_members(accepted_project, accepted_generated, "accepted")?;
    match required_generated_members(project) {
        Ok(desired) => {
            let actual = snapshot
                .generated
                .ordered_members()
                .into_iter()
                .map(|member| member.address)
                .collect::<Vec<_>>();
            if actual != desired {
                return Err(
                    "current code-session generated provenance does not match managed source"
                        .into(),
                );
            }
        }
        Err(error) if snapshot.failure.is_some() && snapshot.expansion.is_none() => {
            if snapshot
                .failure
                .as_ref()
                .is_none_or(|failure| failure.diagnostic != error.to_string())
            {
                return Err(
                    "retained structural diagnostic does not authenticate managed source".into(),
                );
            }
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok(())
}

pub fn validate_managed_draft_bound(draft: &str) -> Result<(), String> {
    if draft.len() > crate::MANAGED_SOURCE_LIMIT {
        Err(format!(
            "managed source draft is {} bytes; the limit is {}",
            draft.len(),
            crate::MANAGED_SOURCE_LIMIT,
        ))
    } else {
        Ok(())
    }
}

pub fn validate_managed_draft_diagnostic(
    source: &str,
    diagnostic: &str,
    span: ManagedSpan,
) -> Result<(), String> {
    if diagnostic.is_empty() {
        return Err("managed compiler diagnostic is empty".into());
    }
    if diagnostic.len() > MAX_MANAGED_DRAFT_DIAGNOSTIC_BYTES {
        return Err(format!(
            "managed compiler diagnostic is {} bytes; the limit is {MAX_MANAGED_DRAFT_DIAGNOSTIC_BYTES}",
            diagnostic.len(),
        ));
    }
    if span.start > span.end || span.end > source.len() {
        return Err("managed compiler diagnostic span is outside its candidate source".into());
    }
    if !source.is_char_boundary(span.start) || !source.is_char_boundary(span.end) {
        return Err("managed compiler diagnostic span splits a UTF-8 code point".into());
    }
    Ok(())
}

pub fn managed_diagnostic_line_column(source: &str, offset: usize) -> (usize, usize) {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.chars().count(), |(_, tail)| tail.chars().count())
        + 1;
    (line, column)
}

fn artifact_digests(project: &CodeProject) -> Result<BTreeMap<String, String>, String> {
    let modules = project
        .lock
        .get("modules")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| "code-project lock has no module pins".to_owned())?;
    modules
        .iter()
        .map(|(module, pin)| {
            pin.get("artifact")
                .and_then(serde_json::Value::as_str)
                .map(|digest| (module.clone(), digest.to_owned()))
                .ok_or_else(|| format!("code-project module `{module}` has no artifact digest"))
        })
        .collect()
}

fn validate_generated_members(
    project: &CodeProject,
    generated: &KeyedReconcileState,
    authority: &str,
) -> Result<(), String> {
    let desired = required_generated_members(project).map_err(|error| error.to_string())?;
    let actual = generated
        .ordered_members()
        .into_iter()
        .map(|member| member.address)
        .collect::<Vec<_>>();
    if actual == desired {
        Ok(())
    } else {
        Err(format!(
            "{authority} code-session generated provenance does not match managed source"
        ))
    }
}

pub fn validate_source_workspace_presentation(
    project: &CodeProject,
    value: &SourceWorkspacePresentation,
) -> Result<(), String> {
    if value.selected_file != "sketch.ts"
        && !project.custom_files.contains_key(&value.selected_file)
    {
        return Err(format!(
            "code-project file `{}` is unavailable",
            value.selected_file
        ));
    }
    if let SourceWorkspaceOrigin::Bundled { sample } = &value.origin
        && crate::bundled_sample(sample).is_none()
    {
        return Err(format!("unknown bundled sample `{sample}`"));
    }
    validate_managed_draft_bound(&value.managed_draft)?;
    if let Some(diagnostic) = &value.draft_diagnostic {
        validate_managed_draft_diagnostic(
            &value.managed_draft,
            &diagnostic.message,
            diagnostic.span,
        )?;
        let (line, column) =
            managed_diagnostic_line_column(&value.managed_draft, diagnostic.span.start);
        if (diagnostic.line, diagnostic.column) != (line, column) {
            return Err("persisted managed draft diagnostic has stale line or column".into());
        }
    }
    Ok(())
}
