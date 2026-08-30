// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::process::Command;
use std::sync::{
    Arc, Barrier,
    atomic::{AtomicU64, Ordering},
};
use std::thread;

use geosolve_headless::{
    HEADLESS_EDIT_BATCH_LIMIT, HEADLESS_REPORT_VERSION, HeadlessError, HeadlessInput,
    bundled_demo_keys, edit, inspect, publish_render, render,
};
use geosolve_sketch_code::{
    KeyedReconcileState, ManagedControlEdit, ManagedControlEditBatch, ManagedValue, ProjectKey,
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

#[test]
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
    let edited = edit(
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
fn managed_source_project_json_and_bundled_inputs_are_all_browser_free() {
    const SOURCE: &str = r#""use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const circle = $.geometry.circle("circle", {
    center: [0, 0],
    radius: mm(5),
  });
  return $.outputs({ circle });
});
"#;
    let managed = HeadlessInput::ManagedSource {
        project: ProjectKey("m87-headless-managed".into()),
        source: SOURCE.into(),
    };
    let first = render(&managed).expect("managed source render");
    let canonical = first.project().to_canonical_json().unwrap();
    let restored = render(&HeadlessInput::CodeProjectJson(canonical.clone()))
        .expect("canonical project render");
    assert_eq!(
        first.report().project_digest,
        restored.report().project_digest
    );
    assert_eq!(
        first.report().intent_session,
        restored.report().intent_session
    );
    assert_eq!(first.report().document, restored.report().document);
    assert_eq!(first.svg(), restored.svg());

    let bundled = render(&typed_panel()).expect("bundled demo render");
    assert!(bundled.report().validation.hard_residuals_validated);

    let mut unknown: serde_json::Value = serde_json::from_str(&canonical).unwrap();
    unknown["unexpectedAuthority"] = serde_json::json!(true);
    let error = render(&HeadlessInput::CodeProjectJson(
        serde_json::to_string(&unknown).unwrap(),
    ))
    .expect_err("unknown project authority must reject");
    assert!(error.to_string().contains("unknown field"));
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
    assert_eq!(first.report().validation.curve_count, 176);
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
        ("cnc-joinery-fit-coupon", 29, 47, 36, 33, 10, 23),
        ("gridfinity-1x1x3-section", 31, 36, 31, 18, 4, 11),
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
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end CLI regression keeps inspect, render, edit, stale-CAS, and non-overwrite publication in the same browser-free process contract"
)]
fn cli_inspect_render_and_edit_are_browser_free_and_never_overwrite_outputs() {
    const SOURCE: &str = r#""use geosolve managed-v1";
import { sketch, mm } from "@geosolve/sketch-code";

export default sketch(($) => {
  const circle = $.geometry.circle("circle", {
    center: [0, 0],
    radius: mm(5),
  });
  return $.outputs({ circle });
});
"#;
    let ordinal = NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "geosolve-m87-headless-cli-test-{}-{ordinal}",
        std::process::id()
    ));
    assert!(!root.exists(), "CLI test namespace must be fresh");
    fs::create_dir(&root).expect("create owned CLI test root");
    let source = root.join("sketch.ts");
    fs::write(&source, SOURCE).expect("write managed CLI input");
    let binary = env!("CARGO_BIN_EXE_geosolve-headless");

    let demos = Command::new(binary)
        .arg("demos")
        .output()
        .expect("run demos command");
    assert!(demos.status.success());
    assert_eq!(String::from_utf8(demos.stdout).unwrap().lines().count(), 12);

    let inspected = Command::new(binary)
        .args(["inspect", "--managed"])
        .arg(&source)
        .args(["--project-key", "m87-headless-cli"])
        .output()
        .expect("run inspect command");
    assert!(
        inspected.status.success(),
        "inspect stderr: {}",
        String::from_utf8_lossy(&inspected.stderr)
    );
    let inspected: geosolve_headless::HeadlessInspection =
        serde_json::from_slice(&inspected.stdout).expect("decode CLI inspection");
    let radius = inspected
        .controls
        .editable()
        .find(|control| control.source.declaration.0 == "circle")
        .expect("CLI circle radius control");
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
    let edited = Command::new(binary)
        .args(["edit", "--managed"])
        .arg(&source)
        .args(["--project-key", "m87-headless-cli", "--edit"])
        .arg(&batch_path)
        .arg("--out")
        .arg(&edited_output)
        .output()
        .expect("run edit command");
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
        .find(|control| control.source.declaration.0 == "circle")
        .expect("edited CLI circle radius control");
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
    let next = Command::new(binary)
        .args(["edit", "--project"])
        .arg(edited_output.join("project.json"))
        .arg("--edit")
        .arg(&next_batch_path)
        .arg("--out")
        .arg(&next_output)
        .output()
        .expect("edit emitted project with its fresh token");
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
    let stale = Command::new(binary)
        .args(["edit", "--project"])
        .arg(edited_output.join("project.json"))
        .arg("--edit")
        .arg(&batch_path)
        .arg("--out")
        .arg(&stale_output)
        .output()
        .expect("reject stale source token against emitted project");
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("stale or foreign"));
    assert!(!stale_output.exists());

    let rendered_output = root.join("rendered");
    let rendered = Command::new(binary)
        .args(["render", "--managed"])
        .arg(&source)
        .args(["--project-key", "m87-headless-cli", "--out"])
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
        .args(["render", "--managed"])
        .arg(&source)
        .args(["--project-key", "m87-headless-cli", "--out"])
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
    let rejected = Command::new(binary)
        .args(["edit", "--managed"])
        .arg(&source)
        .args(["--project-key", "m87-headless-cli", "--edit"])
        .arg(&malformed_batch)
        .arg("--out")
        .arg(&rejected_output)
        .output()
        .expect("run rejected edit command");
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
        edit(&typed_panel(), &failed_batch),
        Err(HeadlessError::Materialization(_))
    ));
    let failed_batch_path = root.join("failed-acceptance-batch.json");
    fs::write(
        &failed_batch_path,
        serde_json::to_vec(&failed_batch).unwrap(),
    )
    .unwrap();
    let failed_acceptance_output = root.join("failed-acceptance-output");
    let failed_acceptance = Command::new(binary)
        .args(["edit", "--demo", "typed-panel", "--edit"])
        .arg(&failed_batch_path)
        .arg("--out")
        .arg(&failed_acceptance_output)
        .output()
        .expect("run representable failed-acceptance edit");
    assert!(!failed_acceptance.status.success());
    assert!(
        String::from_utf8_lossy(&failed_acceptance.stderr)
            .contains("native materialization failed")
    );
    assert!(
        !failed_acceptance_output.exists(),
        "a representable edit which fails native acceptance must publish no directory",
    );

    let invalid_source = root.join("invalid-sketch.ts");
    fs::write(&invalid_source, b"not managed-v1 source").unwrap();
    let invalid_source_output = root.join("invalid-source-output");
    let rejected_source = Command::new(binary)
        .args(["render", "--managed"])
        .arg(&invalid_source)
        .args(["--project-key", "invalid-source", "--out"])
        .arg(&invalid_source_output)
        .output()
        .expect("run invalid-source render");
    assert!(!rejected_source.status.success());
    assert!(!invalid_source_output.exists());

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
fn cli_requires_explicit_managed_key_and_rejects_inputs_at_streaming_bounds() {
    const SOURCE: &str = r#""use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => $.outputs({}));
"#;
    let ordinal = NEXT_OUTPUT.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "geosolve-m87-headless-cli-bounds-{}-{ordinal}",
        std::process::id()
    ));
    fs::create_dir(&root).unwrap();
    let source = root.join("sketch.ts");
    fs::write(&source, SOURCE).unwrap();
    let binary = env!("CARGO_BIN_EXE_geosolve-headless");

    let missing_key = Command::new(binary)
        .args(["inspect", "--managed"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(!missing_key.status.success());
    assert!(String::from_utf8_lossy(&missing_key.stderr).contains("requires --project-key"));

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
    assert!(String::from_utf8_lossy(&irrelevant_key.stderr).contains("only valid with --managed"));

    let oversized_source = root.join("oversized-sketch.ts");
    File::create(&oversized_source)
        .unwrap()
        .set_len((geosolve_sketch_code::MANAGED_SOURCE_LIMIT + 1) as u64)
        .unwrap();
    let source_rejection = Command::new(binary)
        .args(["inspect", "--managed"])
        .arg(&oversized_source)
        .args(["--project-key", "bounded-source"])
        .output()
        .unwrap();
    assert!(!source_rejection.status.success());
    assert!(String::from_utf8_lossy(&source_rejection.stderr).contains("managed source exceeds"));

    let invalid_utf8_source = root.join("invalid-utf8-sketch.ts");
    fs::write(&invalid_utf8_source, [0xff]).unwrap();
    let utf8_rejection = Command::new(binary)
        .args(["inspect", "--managed"])
        .arg(&invalid_utf8_source)
        .args(["--project-key", "bounded-source"])
        .output()
        .unwrap();
    assert!(!utf8_rejection.status.success());
    assert!(String::from_utf8_lossy(&utf8_rejection.stderr).contains("not valid UTF-8"));

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

    let oversized_batch = root.join("oversized-batch.json");
    File::create(&oversized_batch)
        .unwrap()
        .set_len((HEADLESS_EDIT_BATCH_LIMIT + 1) as u64)
        .unwrap();
    let batch_rejection = Command::new(binary)
        .args(["edit", "--managed"])
        .arg(&source)
        .args(["--project-key", "bounded-batch", "--edit"])
        .arg(&oversized_batch)
        .arg("--out")
        .arg(root.join("oversized-batch-output"))
        .output()
        .unwrap();
    assert!(!batch_rejection.status.success());
    assert!(String::from_utf8_lossy(&batch_rejection.stderr).contains("edit batch exceeds"));
    assert!(!root.join("oversized-batch-output").exists());

    fs::remove_dir_all(root).unwrap();
}
