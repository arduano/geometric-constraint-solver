// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch_code::{CodeProject, CodeProjectFile, ProjectKey, bundled_sample};
use geosolve_sketch_engine::{ManagedProjectInput, compile_project_json};
use geosolve_sketch_intent::intent_content_digest;

#[test]
fn complete_managed_project_construction_preserves_exact_source_and_pins() {
    let project = bundled_sample("pc-water-manifold").unwrap().project();
    let input = ManagedProjectInput {
        project: project.project.0.clone(),
        compiled: *project.managed.compiled.clone().unwrap(),
        custom_files: project.custom_files.clone(),
        artifacts: project.artifacts.clone(),
        lock: project.lock.clone(),
    };
    let canonical = compile_project_json(&serde_json::to_string(&input).unwrap()).unwrap();
    assert_eq!(CodeProject::from_json(&canonical).unwrap(), project);
    let mut invalid = input;
    invalid
        .custom_files
        .values_mut()
        .next()
        .unwrap()
        .contents
        .push_str("// external edit");
    assert!(compile_project_json(&serde_json::to_string(&invalid).unwrap()).is_err());
}

#[test]
fn generic_local_dependency_paths_are_bounded_without_relaxing_containment() {
    let mut project = CodeProject::empty(ProjectKey("local-module-admission".into())).unwrap();
    for path in [
        "helpers.ts",
        "lib/values.js",
        "shared/size.mts",
        "shapes/panel.patch.ts",
    ] {
        let contents = "// source provenance only\nexport const width = 12;\n".to_owned();
        project.custom_files.insert(
            path.into(),
            CodeProjectFile {
                path: path.into(),
                source_digest: intent_content_digest(contents.as_bytes()).to_string(),
                contents,
                managed: false,
            },
        );
    }
    project.validate().unwrap();
    for path in [
        "../escape.ts",
        "/absolute.ts",
        "dir//empty.ts",
        "dir/../escape.ts",
        "dir/./alias.ts",
        "dir\\windows.ts",
        "C:/absolute.ts",
        "unsupported.json",
        "dir\ncontrol.ts",
    ] {
        let mut invalid = project.clone();
        let (_, mut file) = invalid.custom_files.pop_first().unwrap();
        file.path = path.into();
        invalid.custom_files.insert(path.into(), file);
        assert!(invalid.validate().is_err(), "{path}");
    }
}
