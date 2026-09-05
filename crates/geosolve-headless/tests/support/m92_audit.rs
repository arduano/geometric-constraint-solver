// SPDX-License-Identifier: GPL-3.0-or-later
#![allow(
    dead_code,
    reason = "shared support is compiled separately for each audit test target"
)]

use geosolve_headless::{
    HeadlessError, HeadlessInput, HeadlessPreparedEdit, HeadlessRender, inspect, prepare_edit,
    resolve_edit,
};
use geosolve_sketch::{DocumentId, PersistentId, SketchDocument};
use geosolve_sketch_code::{
    CodeProject, KeyedReconcileState, ManagedControlEdit, ManagedControlEditBatch,
    ManagedPathSegment, ManagedValue, PreparedManagedMutationReceipt, SketchCodeSession,
    UnitLiteral, materialize_code_project_cold, required_generated_members,
};
use geosolve_sketch_intent::IntentSessionId;
use std::{
    collections::BTreeSet,
    io::Write as _,
    path::PathBuf,
    process::{Command, Stdio},
};

fn run_pinned_deno_mutation(prepared: &HeadlessPreparedEdit) -> PreparedManagedMutationReceipt {
    let package_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../packages/geosolve-sketch-code");
    assert!(
        package_root.join("dist/src/managed.js").is_file(),
        "build @geosolve/sketch-code before running the pinned-Deno headless edit gate",
    );
    let mut child = Command::new("deno")
        .args([
            "run",
            "--no-config",
            "--no-lock",
            "--no-prompt",
            "--cached-only",
            "--no-remote",
            "--node-modules-dir=manual",
            "--ignore-env",
            "scripts/mutate-managed-deno.mjs",
        ])
        .current_dir(package_root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch pinned Deno mutation sidecar");
    child
        .stdin
        .take()
        .expect("piped Deno stdin")
        .write_all(&serde_json::to_vec(prepared).expect("encode prepared headless edit"))
        .expect("write prepared edit to Deno");
    let output = child.wait_with_output().expect("wait for Deno mutation");
    assert!(
        output.status.success(),
        "pinned Deno mutation failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    assert!(output.stderr.is_empty(), "successful sidecar must be quiet");
    serde_json::from_slice(&output.stdout).expect("decode pinned Deno mutation receipt")
}

pub fn edit_with_pinned_deno(
    input: &HeadlessInput,
    batch: &ManagedControlEditBatch,
) -> Result<geosolve_headless::HeadlessRender, HeadlessError> {
    let prepared = prepare_edit(input, batch)?;
    let receipt = run_pinned_deno_mutation(&prepared);
    resolve_edit(input, &prepared, receipt)
}

pub fn generated(project: &CodeProject) -> KeyedReconcileState {
    KeyedReconcileState::empty()
        .plan(
            required_generated_members(project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged()
}

pub fn accepted_document(project: &CodeProject) -> SketchDocument {
    let materialized = materialize_code_project_cold(
        project,
        &generated(project),
        IntentSessionId::from_raw(0x92_b0),
        DocumentId(PersistentId::from_u128(0x92_b0)),
        1.0,
    )
    .unwrap();
    let accepted = materialized
        .editor
        .coordinator()
        .accepted_materialization()
        .unwrap();
    assert!(accepted.validation.hard_residuals_validated);
    assert!(accepted.validation.all_active_features_current);
    assert!(
        accepted
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|v| v.is_finite() && v <= 1e-9)
    );
    let state = accepted.session.accepted_state_for_current_input().unwrap();
    assert!(
        state
            .document()
            .points()
            .iter()
            .flat_map(|p| p.position)
            .all(f64::is_finite)
    );
    assert!(
        state
            .document()
            .scalars()
            .iter()
            .all(|s| s.value.is_finite())
    );
    state.document().clone()
}

pub fn edit_mm(
    input: &HeadlessInput,
    declaration: &str,
    path: &[&str],
    replacement: f64,
) -> HeadlessRender {
    let inspection = inspect(input).unwrap();
    let fields = path
        .iter()
        .map(|p| ManagedPathSegment::Field((*p).into()))
        .collect::<Vec<_>>();
    let control = inspection
        .controls
        .editable()
        .find(|c| c.source.declaration.0 == declaration && c.source.path.0 == fields)
        .unwrap_or_else(|| panic!("no editable control {declaration}.{path:?}"));
    edit_with_pinned_deno(
        input,
        &ManagedControlEditBatch::new([ManagedControlEdit {
            token: control.token().unwrap().clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: replacement,
            }),
        }]),
    )
    .unwrap()
}

pub fn assert_valid(render: &HeadlessRender) {
    let validation = &render.report().validation;
    assert!(validation.hard_residuals_validated);
    assert!(validation.all_active_features_current);
    assert!(
        validation
            .maximum_normalized_hard_residual
            .is_none_or(|v| v.is_finite() && v <= 1e-9)
    );
}

pub fn assert_history(base: &CodeProject, edited: &CodeProject) {
    let state = generated(base);
    let materialized = materialize_code_project_cold(
        base,
        &state,
        IntentSessionId::from_raw(0x92_b1),
        DocumentId(PersistentId::from_u128(0x92_b1)),
        1.0,
    )
    .unwrap();
    let mut session = SketchCodeSession::new_project(
        base.clone(),
        state,
        materialized.expansion,
        serde_json::json!({"stage":"base"}),
    )
    .unwrap();
    let before = session.snapshot().clone();
    let plan = session
        .plan_structural_reconciliation(
            session.identity(),
            required_generated_members(edited).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap();
    let materialized = materialize_code_project_cold(
        edited,
        plan.staged(),
        IntentSessionId::from_raw(0x92_b2),
        DocumentId(PersistentId::from_u128(0x92_b2)),
        1.0,
    )
    .unwrap();
    let prepared = session
        .prepare_project_edit_from_plan(
            session.identity(),
            edited.clone(),
            plan,
            materialized.expansion,
            serde_json::json!({"stage":"edited"}),
            "Apply measured M92 design edit",
        )
        .unwrap();
    session.apply_prepared(prepared).unwrap();
    let after = session.snapshot().clone();
    session.undo().unwrap().unwrap();
    assert_eq!(
        session.snapshot(),
        &before,
        "exact source/project/expansion Undo"
    );
    session.redo().unwrap().unwrap();
    assert_eq!(
        session.snapshot(),
        &after,
        "exact source/project/expansion Redo"
    );
    let wire = session.to_canonical_json().unwrap();
    let restored = SketchCodeSession::from_json(&wire).unwrap();
    assert_eq!(
        restored.to_canonical_json().unwrap(),
        wire,
        "exact persisted code history"
    );
    assert_eq!(restored.snapshot(), &after);
    let project_wire = edited.to_canonical_json().unwrap();
    assert_eq!(CodeProject::from_json(&project_wire).unwrap(), *edited);
    assert_eq!(
        accepted_document(&CodeProject::from_json(&project_wire).unwrap()),
        accepted_document(edited),
        "accepted geometry reload"
    );
}

pub fn preserve(render: &HeadlessRender, key: &str, stage: &str) {
    if let Some(root) = std::env::var_os("M92_AUDIT_OUTPUT") {
        let parent = PathBuf::from(root).join(key);
        std::fs::create_dir_all(&parent).unwrap();
        let target = parent.join(stage);
        geosolve_headless::publish_render(render, &target).unwrap();
    }
}
