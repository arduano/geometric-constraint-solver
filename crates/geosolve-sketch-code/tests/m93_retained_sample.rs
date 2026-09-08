// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "support/retained_sample.rs"]
mod retained_sample;

use geosolve_sketch_code::{CodeProject, bundled_sample};

#[test]
fn retained_bondtech_fixture_preserves_original_source_project_and_provenance() {
    let archived = retained_sample::TestSample::RetainedBondtech;
    let original = archived.project();
    let canonical = original.to_canonical_json().unwrap();
    assert_eq!(CodeProject::from_json(&canonical).unwrap(), original);
    assert_eq!(original.project.0, "geosolve-sample-bondtech-indx-link");
    assert_eq!(original.managed.source, archived.source());
    assert_eq!(archived.manifest()["key"], "bondtech-indx-link");
    assert_eq!(archived.manifest()["ordinal"], 12);
    assert!(
        archived
            .notice()
            .unwrap()
            .contains("9c44c7ad9efb9b1d2f20c9e5d26951902685345b")
    );
    // Before retirement, establish exact parity with the actual accepted catalog
    // project, including compiler IR, groups, witnesses, provenance and edit data.
    // After explicit retirement, all decoder/authority assertions above still run.
    match retained_sample::resolve("bondtech-indx-link") {
        retained_sample::TestSample::Catalog(live) => {
            let project = live.project();
            let metadata = project
                .managed
                .compiled
                .as_deref()
                .unwrap()
                .authored_metadata()
                .unwrap();
            assert_eq!(metadata.document.title.as_deref(), Some(live.title));
            assert_eq!(metadata.document.description.as_deref(), Some(live.summary));
            assert_eq!(
                metadata
                    .document
                    .dimensions
                    .unwrap()
                    .are_key_constraints_by_default,
                Some(true)
            );
            // Only the explicit source document options were added to this live sample.
            // Preserve an exact comparison of every original declaration/comment/body byte.
            let (prefix, _) = live
                .managed_source()
                .split_once("export default sketch({")
                .unwrap();
            let (_, body) = live.managed_source().split_once("}, ($) => {").unwrap();
            assert_eq!(
                format!("{prefix}export default sketch(($) => {{{body}"),
                archived.source()
            );
            let mut live_manifest = retained_sample::TestSample::Catalog(live).manifest();
            let mut original_manifest = archived.manifest();
            // Catalog order can change without changing the retained original.
            live_manifest.as_object_mut().unwrap().remove("ordinal");
            original_manifest.as_object_mut().unwrap().remove("ordinal");
            // Live design metadata is source-owned; the archived manifest is immutable.
            for field in ["title", "summary", "groups"] {
                assert!(live_manifest.get(field).is_none());
                original_manifest.as_object_mut().unwrap().remove(field);
            }
            assert!(live_manifest.get("dimension_presentation").is_none());
            assert!(original_manifest.get("dimension_presentation").is_none());
            assert_eq!(live_manifest, original_manifest);
            assert_eq!(live.witnesses_json(), archived.witnesses());
            assert_eq!(live.notice(), archived.notice());
            assert_eq!(
                retained_sample::TestSample::Catalog(live).edits(),
                archived.edits()
            );
        }
        retained_sample::TestSample::RetainedBondtech => {
            assert!(bundled_sample("bondtech-indx-link").is_none());
        }
    }
}

#[test]
fn an_unknown_dedicated_sample_cannot_implicitly_use_an_archive() {
    let panic = std::panic::catch_unwind(|| retained_sample::resolve("missing-dedicated-sample"))
        .expect_err("unknown dedicated sample must fail closed");
    let message = panic
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic.downcast_ref::<&str>().copied())
        .expect("assertion panic message");
    assert!(message.contains("no archived fixture exists"));
}
