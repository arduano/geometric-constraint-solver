// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicU64, Ordering},
};
use std::thread;

use geosolve_headless::{
    HEADLESS_EDIT_BATCH_LIMIT, HEADLESS_REPORT_VERSION, HeadlessError, HeadlessInput,
    HeadlessPreparedEdit, bundled_demo_keys, inspect, prepare_edit, publish_render, render,
    resolve_edit,
};
use geosolve_sketch_code::{
    CodeProject, CompiledManagedSource, KeyedReconcileState, ManagedControlEdit,
    ManagedControlEditBatch, ManagedValue, PreparedManagedMutationReceipt, ProjectKey,
    SketchCodeSession, UnitLiteral, expand_code_project, managed_control_manifest,
    required_generated_members,
};
use geosolve_sketch_intent::{IntentSession, IntentSessionId};

static NEXT_OUTPUT: AtomicU64 = AtomicU64::new(1);

fn typed_panel() -> HeadlessInput {
    HeadlessInput::BundledDemo("typed-panel".into())
}

fn typed_panel_radius(
    inspection: &geosolve_headless::HeadlessInspection,
) -> &geosolve_sketch_code::ManagedControl {
    inspection
        .controls
        .editable()
        .find(|control| {
            matches!(
                &control.value,
                ManagedValue::Unit(UnitLiteral { unit, value })
                    if unit == "mm" && value.to_bits() == 4.0_f64.to_bits()
            ) && control.consumers.len() == 2
        })
        .expect("Typed Panel shared radius control")
}

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

fn edit_with_pinned_deno(
    input: &HeadlessInput,
    batch: &ManagedControlEditBatch,
) -> Result<geosolve_headless::HeadlessRender, HeadlessError> {
    let prepared = prepare_edit(input, batch)?;
    let receipt = run_pinned_deno_mutation(&prepared);
    resolve_edit(input, &prepared, receipt)
}

fn run_cli_prepare_edit(
    binary: &str,
    input_flag: &str,
    input: &OsStr,
    batch: &Path,
) -> std::process::Output {
    Command::new(binary)
        .arg("prepare-edit")
        .arg(input_flag)
        .arg(input)
        .arg("--edit")
        .arg(batch)
        .output()
        .expect("run prepare-edit command")
}

fn write_compiler_exchange(
    root: &Path,
    stem: &str,
    prepared: &HeadlessPreparedEdit,
) -> (PathBuf, PathBuf) {
    let prepared_path = root.join(format!("{stem}-prepared.json"));
    let receipt_path = root.join(format!("{stem}-receipt.json"));
    fs::write(
        &prepared_path,
        serde_json::to_vec(prepared).expect("encode prepared compiler exchange"),
    )
    .expect("write prepared compiler exchange");
    fs::write(
        &receipt_path,
        serde_json::to_vec(&run_pinned_deno_mutation(prepared))
            .expect("encode compiler mutation receipt"),
    )
    .expect("write compiler mutation receipt");
    (prepared_path, receipt_path)
}

fn run_cli_two_phase_edit(
    binary: &str,
    input_flag: &str,
    input: &OsStr,
    batch: &Path,
    output: &Path,
    exchange_root: &Path,
    stem: &str,
) -> std::process::Output {
    let prepared_output = run_cli_prepare_edit(binary, input_flag, input, batch);
    if !prepared_output.status.success() {
        return prepared_output;
    }
    let prepared: HeadlessPreparedEdit = serde_json::from_slice(&prepared_output.stdout)
        .expect("decode CLI prepared compiler exchange");
    let (prepared_path, receipt_path) = write_compiler_exchange(exchange_root, stem, &prepared);
    Command::new(binary)
        .arg("resolve-edit")
        .arg(input_flag)
        .arg(input)
        .arg("--prepared")
        .arg(prepared_path)
        .arg("--receipt")
        .arg(receipt_path)
        .arg("--out")
        .arg(output)
        .output()
        .expect("run resolve-edit command")
}

#[test]
#[ignore = "release gate builds the pinned TypeScript sidecar before running this exact test"]
fn inspect_edit_solve_and_static_render_share_one_exact_control_authority() {
    let before = inspect(&typed_panel()).expect("headless Typed Panel inspection");
    assert_eq!(before.report.version, HEADLESS_REPORT_VERSION);
    assert!(before.report.validation.hard_residuals_validated);
    assert!(before.report.validation.all_active_features_current);
    assert!(
        before
            .report
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    let radius = typed_panel_radius(&before);
    let edited = edit_with_pinned_deno(
        &typed_panel(),
        &ManagedControlEditBatch::new([ManagedControlEdit {
            token: radius.token().expect("editable radius").clone(),
            value: ManagedValue::Unit(UnitLiteral {
                unit: "mm".into(),
                value: 2.0,
            }),
        }]),
    )
    .expect("exact-CAS radius edit");
    assert_eq!(
        edited
            .project()
            .managed
            .source
            .matches("radius: mm(2)")
            .count(),
        1
    );
    let edited_radius = edited
        .controls()
        .editable()
        .find(|control| {
            matches!(
                &control.value,
                ManagedValue::Unit(UnitLiteral { unit, value })
                    if unit == "mm" && value.to_bits() == 2.0_f64.to_bits()
            ) && control.consumers.len() == 2
        })
        .expect("edited shared radius control");
    assert_ne!(edited_radius.token().unwrap(), radius.token().unwrap());
    assert!(
        edited
            .svg()
            .starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\"")
    );
    assert!(edited.scene_markup().contains("wb-computed-fillet"));
    let png = edited.png();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(u32::from_be_bytes(png[16..20].try_into().unwrap()), 2_000);
    assert_eq!(u32::from_be_bytes(png[20..24].try_into().unwrap()), 1_400);
    assert_ne!(edited.report().source_digest, before.report.source_digest);
    assert_ne!(edited.report().svg_sha256, before.report.svg_sha256);
}

#[test]
fn project_json_and_bundled_inputs_are_all_browser_free() {
    let bundled = render(&typed_panel()).expect("bundled demo render");
    assert!(bundled.report().validation.hard_residuals_validated);

    let canonical = bundled.project().to_canonical_json().unwrap();
    let mut unknown: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    unknown["unexpectedAuthority"] = serde_json::json!(true);
    let error = render(&HeadlessInput::CodeProjectJson(
        serde_json::to_string(&unknown).unwrap(),
    ))
    .expect_err("unknown project authority must reject");
    assert!(error.to_string().contains("unknown field"));
}

#[test]
fn compiled_managed_project_reload_inspect_and_render_are_byte_deterministic() {
    const COMPILED: &str = include_str!(
        "../../../packages/geosolve-sketch-code/test/fixtures/managed-compiler-envelope.json"
    );
    let compiled = CompiledManagedSource::from_json(COMPILED)
        .expect("validated browser/Deno compiler fixture");
    let project = CodeProject::managed(ProjectKey("m89-headless-managed".into()), compiled)
        .expect("managed CodeProject");
    let canonical = project
        .to_canonical_json()
        .expect("canonical managed CodeProject JSON");
    let input = HeadlessInput::CodeProjectJson(canonical.clone());

    let first_inspection = inspect(&input).expect("first managed inspection");
    let second_inspection = inspect(&input).expect("second managed inspection");
    assert_eq!(
        serde_json::to_vec_pretty(&first_inspection.report).unwrap(),
        serde_json::to_vec_pretty(&second_inspection.report).unwrap(),
        "managed inspect report bytes",
    );
    assert_eq!(
        serde_json::to_vec_pretty(&first_inspection.controls).unwrap(),
        serde_json::to_vec_pretty(&second_inspection.controls).unwrap(),
        "managed inspect control bytes",
    );

    let first_render = render(&input).expect("first managed render");
    let second_render = render(&input).expect("second managed render");
    assert_eq!(&first_inspection.report, first_render.report());
    assert_eq!(&first_inspection.controls, first_render.controls());
    assert_eq!(
        serde_json::to_vec_pretty(first_render.report()).unwrap(),
        serde_json::to_vec_pretty(second_render.report()).unwrap(),
        "managed render report bytes",
    );
    assert_eq!(
        serde_json::to_vec_pretty(first_render.controls()).unwrap(),
        serde_json::to_vec_pretty(second_render.controls()).unwrap(),
        "managed render control bytes",
    );
    assert_eq!(
        first_render.scene_markup().as_bytes(),
        second_render.scene_markup().as_bytes(),
        "managed logical scene bytes",
    );
    assert_eq!(
        first_render.svg().as_bytes(),
        second_render.svg().as_bytes(),
        "managed standalone SVG bytes",
    );
    assert_eq!(first_render.png(), second_render.png(), "managed PNG bytes");
    assert_eq!(
        first_render.project().to_canonical_json().unwrap(),
        canonical,
        "headless reload must preserve canonical compiled project authority",
    );

    let report = first_render.report();
    assert!(report.validation.hard_residuals_validated);
    assert!(report.validation.all_active_features_current);
    assert!(
        report
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    assert!(report.camera.model_center.into_iter().all(f64::is_finite));
    assert!(report.camera.pixels_per_model_unit.is_finite());
    assert!(report.camera.pixels_per_model_unit > 0.0);
    assert!(report.camera.fit_margin_pixels.is_finite());
    assert!(report.camera.chord_tolerance_pixels.is_finite());
    assert!(!first_render.scene_markup().is_empty());
    assert!(
        first_render
            .svg()
            .starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\"")
    );
    let png = first_render.png();
    assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
    assert_eq!(
        [
            u32::from_be_bytes(png[16..20].try_into().unwrap()),
            u32::from_be_bytes(png[20..24].try_into().unwrap()),
        ],
        report.png_dimensions,
    );
    assert!(
        report
            .png_dimensions
            .into_iter()
            .all(|dimension| dimension > 0)
    );
}

#[test]
fn repeated_render_is_byte_deterministic_and_publication_is_new_directory_only() {
    let first_inspection = inspect(&typed_panel()).expect("first inspection");
    let second_inspection = inspect(&typed_panel()).expect("second inspection");
    assert_eq!(
        serde_json::to_vec_pretty(&first_inspection.report).unwrap(),
        serde_json::to_vec_pretty(&second_inspection.report).unwrap(),
        "repeated inspect reports must have identical encoded bytes",
    );
    assert_eq!(
        serde_json::to_vec_pretty(&first_inspection.controls).unwrap(),
        serde_json::to_vec_pretty(&second_inspection.controls).unwrap(),
        "repeated inspect controls must have identical encoded bytes",
    );

    let first = render(&typed_panel()).expect("first render");
    let second = render(&typed_panel()).expect("second render");
    assert_eq!(
        serde_json::to_vec_pretty(first.report()).unwrap(),
        serde_json::to_vec_pretty(second.report()).unwrap(),
        "repeated render reports must have identical encoded bytes",
    );
    assert_eq!(
        serde_json::to_vec_pretty(first.controls()).unwrap(),
        serde_json::to_vec_pretty(second.controls()).unwrap(),
        "repeated render controls must have identical encoded bytes",
    );
    assert_eq!(&first_inspection.report, first.report());
    assert_eq!(&first_inspection.controls, first.controls());
    assert_eq!(first.scene_markup(), second.scene_markup());
    assert_eq!(
        first.svg().as_bytes(),
        second.svg().as_bytes(),
        "repeated standalone SVG must have identical bytes",
    );
    assert_eq!(first.png(), second.png());

    let ordinal = NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed);
    let output = std::env::temp_dir().join(format!(
        "geosolve-m87-headless-test-{}-{ordinal}",
        std::process::id()
    ));
    assert!(!output.exists(), "test output namespace must be fresh");
    publish_render(&first, &output).expect("atomic output publication");
    let inventory = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        inventory,
        [
            "controls.json",
            "project.json",
            "report.json",
            "scene.png",
            "scene.svg",
            "sketch.ts",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        "one published generation contains exactly the six admitted products",
    );
    assert_eq!(
        fs::read_to_string(output.join("scene.svg")).unwrap(),
        first.svg()
    );
    assert_eq!(
        fs::read(output.join("scene.png")).unwrap().as_slice(),
        first.png()
    );
    let error = publish_render(&first, &output).expect_err("existing output must reject");
    assert!(error.to_string().contains("already exists"));
    fs::remove_dir_all(&output).expect("remove owned test output");
}

#[test]
fn robotic_routing_board_report_controls_scene_svg_and_png_are_byte_deterministic() {
    let input = HeadlessInput::BundledDemo("robotic-routing-board".into());
    let first = render(&input).expect("first routing-board render");
    let second = render(&input).expect("second routing-board render");

    assert_eq!(first.report().validation.point_count, 104);
    assert_eq!(first.report().validation.curve_count, 112);
    assert_eq!(first.report().validation.constraint_count, 41);
    assert_eq!(first.report().validation.dimension_count, 2);
    assert_eq!(first.report().validation.feature_count, 64);
    assert_eq!(first.report().validation.computed_edge_count, 136);
    assert!(first.report().validation.hard_residuals_validated);
    assert!(first.report().validation.all_active_features_current);
    assert!(
        first
            .report()
            .validation
            .maximum_normalized_hard_residual
            .is_none_or(|value| value.is_finite() && value <= 1.0e-9)
    );
    assert_eq!(
        serde_json::to_vec_pretty(first.report()).unwrap(),
        serde_json::to_vec_pretty(second.report()).unwrap(),
        "routing-board report bytes must be deterministic",
    );
    assert_eq!(
        serde_json::to_vec_pretty(first.controls()).unwrap(),
        serde_json::to_vec_pretty(second.controls()).unwrap(),
        "routing-board control bytes must be deterministic",
    );
    assert_eq!(
        first.scene_markup().as_bytes(),
        second.scene_markup().as_bytes(),
        "routing-board logical scene bytes must be deterministic",
    );
    assert_eq!(
        first.svg().as_bytes(),
        second.svg().as_bytes(),
        "routing-board standalone SVG bytes must be deterministic",
    );
    assert_eq!(
        first.png(),
        second.png(),
        "routing-board PNG bytes must be deterministic for the pinned build",
    );
}

#[test]
fn manufacturing_sketch_reports_controls_scenes_svg_and_png_are_byte_deterministic() {
    let cases = [
        ("cnc-joinery-fit-coupon", 29, 26, 20, 33, 10, 23),
        ("gridfinity-1x1x3-section", 31, 11, 31, 18, 4, 11),
    ];

    for (key, points, curves, constraints, dimensions, features, edges) in cases {
        let input = HeadlessInput::BundledDemo(key.into());
        let first = render(&input).unwrap_or_else(|error| panic!("first {key} render: {error}"));
        let second = render(&input).unwrap_or_else(|error| panic!("second {key} render: {error}"));

        assert_eq!(
            first.report().validation.point_count,
            points,
            "{key} points"
        );
        assert_eq!(
            first.report().validation.curve_count,
            curves,
            "{key} curves"
        );
        assert_eq!(
            first.report().validation.constraint_count,
            constraints,
            "{key} constraints",
        );
        assert_eq!(
            first.report().validation.dimension_count,
            dimensions,
            "{key} dimensions",
        );
        assert_eq!(
            first.report().validation.feature_count,
            features,
            "{key} features",
        );
        assert_eq!(
            first.report().validation.computed_edge_count,
            edges,
            "{key} computed edges",
        );
        assert!(
            first.report().validation.hard_residuals_validated,
            "{key} hard-residual authority",
        );
        assert!(
            first.report().validation.all_active_features_current,
            "{key} Current features",
        );
        assert!(
            first
                .report()
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
            "{key} independent residual bound",
        );
        assert_eq!(
            serde_json::to_vec_pretty(first.report()).unwrap(),
            serde_json::to_vec_pretty(second.report()).unwrap(),
            "{key} report bytes",
        );
        assert_eq!(
            serde_json::to_vec_pretty(first.controls()).unwrap(),
            serde_json::to_vec_pretty(second.controls()).unwrap(),
            "{key} control bytes",
        );
        assert_eq!(
            first.scene_markup().as_bytes(),
            second.scene_markup().as_bytes(),
            "{key} logical scene bytes",
        );
        assert_eq!(
            first.svg().as_bytes(),
            second.svg().as_bytes(),
            "{key} standalone SVG bytes",
        );
        assert_eq!(first.png(), second.png(), "{key} pinned-build PNG bytes");
    }
}

#[test]
fn managed_capabilities_are_transient_from_project_and_session_persistence() {
    let rendered = render(&typed_panel()).expect("render exact Typed Panel authority");
    let project = rendered.project().clone();
    let project_wire = project.to_canonical_json().unwrap();
    let generated = KeyedReconcileState::empty()
        .plan(
            required_generated_members(&project).unwrap(),
            &BTreeSet::new(),
        )
        .unwrap()
        .into_staged();
    let intent = IntentSession::with_id(IntentSessionId::from_raw(0x87_c8)).unwrap();
    let expansion = expand_code_project(&project, &generated, intent.identity()).unwrap();
    let session = SketchCodeSession::new_project(
        project,
        generated,
        expansion,
        serde_json::json!({ "accepted": "opaque-headless-checkpoint" }),
    )
    .unwrap();
    let session_wire = session.to_canonical_json().unwrap();
    let manifest = managed_control_manifest(
        session.snapshot().code_project.as_ref().unwrap(),
        session.snapshot().expansion.as_ref().unwrap(),
    )
    .unwrap();

    assert_eq!(session.to_canonical_json().unwrap(), session_wire);
    assert_eq!(
        session
            .snapshot()
            .code_project
            .as_ref()
            .unwrap()
            .to_canonical_json()
            .unwrap(),
        project_wire,
    );
    for token in manifest.editable().map(|control| control.token().unwrap()) {
        assert!(!project_wire.contains(&token.authentication));
        assert!(!session_wire.contains(&token.authentication));
    }

    let restored = SketchCodeSession::from_json(&session_wire).unwrap();
    assert_eq!(restored.to_canonical_json().unwrap(), session_wire);
    assert_eq!(
        managed_control_manifest(
            restored.snapshot().code_project.as_ref().unwrap(),
            restored.snapshot().expansion.as_ref().unwrap(),
        )
        .unwrap(),
        manifest,
        "reload must deterministically rederive transient controls and capabilities",
    );
}

#[test]
fn concurrent_publication_is_atomic_no_clobber_and_io_failure_leaves_no_generation() {
    let rendered = Arc::new(render(&typed_panel()).expect("render publication candidate"));
    let ordinal = NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "geosolve-m87-headless-publication-race-{}-{ordinal}",
        std::process::id()
    ));
    fs::create_dir(&root).expect("create owned publication test root");
    let output = root.join("generation");
    let barrier = Arc::new(Barrier::new(4));
    let publishers = (0..4)
        .map(|_| {
            let rendered = Arc::clone(&rendered);
            let output = output.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                publish_render(rendered.as_ref(), &output)
            })
        })
        .collect::<Vec<_>>();
    let outcomes = publishers
        .into_iter()
        .map(|publisher| publisher.join().expect("publisher thread must not panic"))
        .collect::<Vec<_>>();
    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert!(outcomes.iter().filter_map(|outcome| outcome.as_ref().err()).all(
        |error| matches!(error, HeadlessError::OutputExists(path) if path == &output.display().to_string())
    ));
    assert_eq!(
        fs::read_dir(&root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>(),
        [output.file_name().unwrap().to_owned()],
        "losing publishers must remove only their owned staging directories",
    );

    let non_directory_parent = root.join("regular-file-parent");
    fs::write(&non_directory_parent, b"not a directory").unwrap();
    let failed_output = non_directory_parent.join("generation");
    let error = publish_render(rendered.as_ref(), &failed_output)
        .expect_err("an invalid destination parent must fail publication");
    assert!(matches!(error, HeadlessError::Io(_)));
    assert!(!failed_output.exists());

    fs::remove_dir_all(root).expect("remove owned publication test root");
}

#[test]
fn all_twelve_bundled_demos_cold_materialize_with_independent_native_acceptance() {
    let keys = bundled_demo_keys();
    assert_eq!(keys.len(), 12);
    for key in keys {
        let rendered = render(&HeadlessInput::BundledDemo(key.into()))
            .unwrap_or_else(|error| panic!("{key} failed headless render: {error}"));
        assert!(
            rendered.report().validation.hard_residuals_validated,
            "{key}"
        );
        assert!(
            rendered.report().validation.all_active_features_current,
            "{key}"
        );
        assert!(
            rendered
                .report()
                .validation
                .maximum_normalized_hard_residual
                .is_none_or(|value| value.is_finite() && value <= 1.0e-9),
            "{key}"
        );
        assert!(!rendered.svg().is_empty(), "{key}");
    }
}

#[test]
#[ignore = "release gate builds and invokes the pinned Deno mutation sidecar"]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end CLI regression keeps inspect, render, edit, stale-CAS, and non-overwrite publication in the same browser-free process contract"
)]
fn cli_inspect_render_and_edit_are_browser_free_and_never_overwrite_outputs() {
    let ordinal = NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "geosolve-m87-headless-cli-test-{}-{ordinal}",
        std::process::id()
    ));
    assert!(!root.exists(), "CLI test namespace must be fresh");
    fs::create_dir(&root).expect("create owned CLI test root");
    let project = root.join("project.json");
    let canonical_project = render(&typed_panel())
        .expect("load bundled compiled project authority")
        .project()
        .to_canonical_json()
        .expect("encode bundled compiled project authority");
    fs::write(&project, canonical_project).expect("write compiled CLI project input");
    let binary = env!("CARGO_BIN_EXE_geosolve-headless");

    let demos = Command::new(binary)
        .arg("demos")
        .output()
        .expect("run demos command");
    assert!(demos.status.success());
    assert_eq!(String::from_utf8(demos.stdout).unwrap().lines().count(), 12);

    let inspected = Command::new(binary)
        .args(["inspect", "--project"])
        .arg(&project)
        .output()
        .expect("run inspect command");
    assert!(
        inspected.status.success(),
        "inspect stderr: {}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let inspected: geosolve_headless::HeadlessInspection =
        serde_json::from_slice(&inspected.stdout).expect("decode CLI inspection");
    let radius = typed_panel_radius(&inspected);
    let batch = ManagedControlEditBatch::new([ManagedControlEdit {
        token: radius.token().expect("editable CLI radius").clone(),
        value: ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 3.0,
        }),
    }]);
    let batch_path = root.join("batch.json");
    fs::write(&batch_path, serde_json::to_vec(&batch).unwrap()).expect("write exact-CAS batch");

    let edited_output = root.join("edited");
    let edited = run_cli_two_phase_edit(
        binary,
        "--project",
        project.as_os_str(),
        &batch_path,
        &edited_output,
        &root,
        "edited",
    );
    assert!(
        edited.status.success(),
        "edit stderr: {}",
        String::from_utf8_lossy(&edited.stderr)
    );
    assert!(
        fs::read_to_string(edited_output.join("sketch.ts"))
            .unwrap()
            .contains("radius: mm(3)")
    );

    let inspected_edited = Command::new(binary)
        .args(["inspect", "--project"])
        .arg(edited_output.join("project.json"))
        .output()
        .expect("inspect emitted edited project");
    assert!(
        inspected_edited.status.success(),
        "edited-project inspect stderr: {}",
        String::from_utf8_lossy(&inspected_edited.stderr)
    );
    let inspected_edited: geosolve_headless::HeadlessInspection =
        serde_json::from_slice(&inspected_edited.stdout).expect("decode edited project inspection");
    let edited_radius = inspected_edited
        .controls
        .editable()
        .find(|control| {
            matches!(
                &control.value,
                ManagedValue::Unit(UnitLiteral { unit, value })
                    if unit == "mm" && value.to_bits() == 3.0_f64.to_bits()
            ) && control.consumers.len() == 2
        })
        .expect("edited Typed Panel shared radius control");
    assert_ne!(edited_radius.token().unwrap(), radius.token().unwrap());

    let rendered_edited_output = root.join("rendered-edited-project");
    let rendered_edited = Command::new(binary)
        .args(["render", "--project"])
        .arg(edited_output.join("project.json"))
        .arg("--out")
        .arg(&rendered_edited_output)
        .output()
        .expect("render emitted edited project");
    assert!(
        rendered_edited.status.success(),
        "emitted-project render stderr: {}",
        String::from_utf8_lossy(&rendered_edited.stderr)
    );
    for product in [
        "controls.json",
        "project.json",
        "scene.png",
        "scene.svg",
        "sketch.ts",
    ] {
        assert_eq!(
            fs::read(edited_output.join(product)).unwrap(),
            fs::read(rendered_edited_output.join(product)).unwrap(),
            "rendering emitted project.json changed `{product}` authority",
        );
    }
    let mut edited_report: serde_json::Value =
        serde_json::from_slice(&fs::read(edited_output.join("report.json")).unwrap()).unwrap();
    let mut rendered_edited_report: serde_json::Value =
        serde_json::from_slice(&fs::read(rendered_edited_output.join("report.json")).unwrap())
            .unwrap();
    edited_report.as_object_mut().unwrap().remove("input");
    rendered_edited_report
        .as_object_mut()
        .unwrap()
        .remove("input");
    assert_eq!(
        rendered_edited_report, edited_report,
        "only the admitted input-form descriptor may differ after rendering emitted project.json",
    );

    let next_batch = ManagedControlEditBatch::new([ManagedControlEdit {
        token: edited_radius
            .token()
            .expect("fresh emitted-project radius")
            .clone(),
        value: ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 2.0,
        }),
    }]);
    let next_batch_path = root.join("next-batch.json");
    fs::write(&next_batch_path, serde_json::to_vec(&next_batch).unwrap()).unwrap();
    let next_output = root.join("next");
    let edited_project = edited_output.join("project.json");
    let next = run_cli_two_phase_edit(
        binary,
        "--project",
        edited_project.as_os_str(),
        &next_batch_path,
        &next_output,
        &root,
        "next",
    );
    assert!(
        next.status.success(),
        "next edit stderr: {}",
        String::from_utf8_lossy(&next.stderr)
    );
    assert!(
        fs::read_to_string(next_output.join("sketch.ts"))
            .unwrap()
            .contains("radius: mm(2)")
    );

    let stale_output = root.join("stale");
    let stale = run_cli_prepare_edit(binary, "--project", edited_project.as_os_str(), &batch_path);
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("stale or foreign"));
    assert!(!stale_output.exists());

    let rendered_output = root.join("rendered");
    let rendered = Command::new(binary)
        .args(["render", "--project"])
        .arg(&project)
        .arg("--out")
        .arg(&rendered_output)
        .output()
        .expect("run render command");
    assert!(
        rendered.status.success(),
        "render stderr: {}",
        String::from_utf8_lossy(&rendered.stderr)
    );
    let original_report = fs::read(rendered_output.join("report.json")).unwrap();
    let duplicate = Command::new(binary)
        .args(["render", "--project"])
        .arg(&project)
        .arg("--out")
        .arg(&rendered_output)
        .output()
        .expect("run duplicate render command");
    assert!(!duplicate.status.success());
    assert_eq!(
        fs::read(rendered_output.join("report.json")).unwrap(),
        original_report,
        "an existing generation must remain byte-identical"
    );

    let malformed_batch = root.join("malformed-batch.json");
    fs::write(&malformed_batch, b"not json").unwrap();
    let rejected_output = root.join("rejected");
    let rejected = run_cli_prepare_edit(binary, "--project", project.as_os_str(), &malformed_batch);
    assert!(!rejected.status.success());
    assert!(!rejected_output.exists());

    let failed_inspection = inspect(&typed_panel()).expect("inspect failed-acceptance fixture");
    let failed_batch = ManagedControlEditBatch::new([ManagedControlEdit {
        token: typed_panel_radius(&failed_inspection)
            .token()
            .unwrap()
            .clone(),
        value: ManagedValue::Unit(UnitLiteral {
            unit: "mm".into(),
            value: 400.0,
        }),
    }]);
    assert!(matches!(
        edit_with_pinned_deno(&typed_panel(), &failed_batch),
        Err(HeadlessError::Materialization(_))
    ));
    let failed_batch_path = root.join("failed-acceptance-batch.json");
    fs::write(
        &failed_batch_path,
        serde_json::to_vec(&failed_batch).unwrap(),
    )
    .unwrap();
    let failed_acceptance_output = root.join("failed-acceptance-output");
    let failed_acceptance = run_cli_two_phase_edit(
        binary,
        "--demo",
        OsStr::new("typed-panel"),
        &failed_batch_path,
        &failed_acceptance_output,
        &root,
        "failed-acceptance",
    );
    assert!(!failed_acceptance.status.success());
    assert!(
        String::from_utf8_lossy(&failed_acceptance.stderr)
            .contains("native materialization failed")
    );
    assert!(
        !failed_acceptance_output.exists(),
        "a representable edit which fails native acceptance must publish no directory",
    );

    let invalid_project = root.join("invalid-project.json");
    fs::write(&invalid_project, b"{}").unwrap();
    let invalid_project_output = root.join("invalid-project-output");
    let rejected_project = Command::new(binary)
        .args(["render", "--project"])
        .arg(&invalid_project)
        .arg("--out")
        .arg(&invalid_project_output)
        .output()
        .expect("run invalid-project render");
    assert!(!rejected_project.status.success());
    assert!(!invalid_project_output.exists());

    fs::remove_dir_all(&root).expect("remove owned CLI test root");
}

#[test]
fn cli_rejects_removed_raw_source_flags_and_inputs_at_streaming_bounds() {
    let ordinal = NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "geosolve-m87-headless-cli-bounds-{}-{ordinal}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let binary = env!("CARGO_BIN_EXE_geosolve-headless");

    let removed_managed = Command::new(binary)
        .args(["inspect", "--managed"])
        .arg(root.join("sketch.ts"))
        .output()
        .unwrap();
    assert!(!removed_managed.status.success());
    assert!(
        String::from_utf8_lossy(&removed_managed.stderr).contains("unknown option `--managed`")
    );

    let irrelevant_key = Command::new(binary)
        .args([
            "inspect",
            "--demo",
            "typed-panel",
            "--project-key",
            "foreign",
        ])
        .output()
        .unwrap();
    assert!(!irrelevant_key.status.success());
    assert!(
        String::from_utf8_lossy(&irrelevant_key.stderr).contains("unknown option `--project-key`")
    );

    let oversized_project = root.join("oversized-project.json");
    File::create(&oversized_project)
        .unwrap()
        .set_len((geosolve_sketch_code::CODE_PROJECT_LIMIT + 1) as u64)
        .unwrap();
    let project_rejection = Command::new(binary)
        .args(["inspect", "--project"])
        .arg(&oversized_project)
        .output()
        .unwrap();
    assert!(!project_rejection.status.success());
    assert!(String::from_utf8_lossy(&project_rejection.stderr).contains("project JSON exceeds"));

    let invalid_utf8_project = root.join("invalid-utf8-project.json");
    fs::write(&invalid_utf8_project, [0xff]).unwrap();
    let utf8_rejection = Command::new(binary)
        .args(["inspect", "--project"])
        .arg(&invalid_utf8_project)
        .output()
        .unwrap();
    assert!(!utf8_rejection.status.success());
    assert!(String::from_utf8_lossy(&utf8_rejection.stderr).contains("not valid UTF-8"));

    let oversized_batch = root.join("oversized-batch.json");
    File::create(&oversized_batch)
        .unwrap()
        .set_len((HEADLESS_EDIT_BATCH_LIMIT + 1) as u64)
        .unwrap();
    let batch_rejection = run_cli_prepare_edit(
        binary,
        "--demo",
        OsStr::new("typed-panel"),
        &oversized_batch,
    );
    assert!(!batch_rejection.status.success());
    assert!(String::from_utf8_lossy(&batch_rejection.stderr).contains("edit batch exceeds"));
    assert!(!root.join("oversized-batch-output").exists());

    fs::remove_dir_all(root).unwrap();
}
