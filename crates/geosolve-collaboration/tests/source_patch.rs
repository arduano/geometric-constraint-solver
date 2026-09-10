// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_collaboration::{
    source_patch::{
        AnchoredSourcePatch, SourceEdit, SourcePatch, SourcePatchError, SourceReconciliation,
        SourceRegion, source_digest,
    },
    text::{SharedTextDocument, SharedTextLimits, utf8_to_utf16},
};

fn setup() -> (SharedTextDocument, SourcePatch, Vec<SourceRegion>) {
    let source = "// 🚰 water\nconst width = 12;\nconst unfinished = 1;\n";
    let candidate = source.replacen("12", "14", 1);
    let mut document = SharedTextDocument::new(b"server", SharedTextLimits::default()).unwrap();
    document.create_file("main.ts", source).unwrap();
    let start = utf8_to_utf16(source, source.find("12").unwrap()).unwrap();
    let owner_start = utf8_to_utf16(source, source.find("const width").unwrap()).unwrap();
    let patch = SourcePatch {
        base_source_digest: source_digest(source),
        candidate_source_digest: source_digest(&candidate),
        edits: vec![SourceEdit {
            start,
            end: start + 2,
            expected: "12".into(),
            replacement: "14".into(),
        }],
    };
    (
        document,
        patch,
        vec![SourceRegion {
            start: owner_start,
            end: start + 3,
        }],
    )
}

#[test]
fn unrelated_invalid_text_survives_anchored_canvas_source_edit() {
    let (accepted, patch, owners) = setup();
    let anchor = AnchoredSourcePatch::capture(&accepted, "main.ts", &patch, &owners).unwrap();
    let mut working = accepted.fork(b"editor").unwrap();
    let length = working
        .capture()
        .text("main.ts")
        .unwrap()
        .encode_utf16()
        .count();
    working.splice("main.ts", length, 0, "import {").unwrap();
    working.splice("main.ts", 0, 0, "// 漢字\n").unwrap();
    let before = working.capture().text("main.ts").unwrap().to_string();
    let SourceReconciliation::Applied { patch: merged, .. } =
        anchor.reconcile(&mut working).unwrap()
    else {
        panic!("unrelated invalid syntax must not block intact owner")
    };
    let expected = before.replacen("12", "14", 1);
    assert_eq!(working.capture().text("main.ts"), Some(expected.as_str()));
    assert_eq!(merged.apply(&before).unwrap(), expected);
}

#[test]
fn changed_ownership_or_competing_literal_preserves_entire_draft() {
    for replacement in ["const other = 12;", "const width = 18;", "const width ="] {
        let (accepted, patch, owners) = setup();
        let anchor = AnchoredSourcePatch::capture(&accepted, "main.ts", &patch, &owners).unwrap();
        let mut working = accepted.fork(b"editor").unwrap();
        working
            .splice(
                "main.ts",
                owners[0].start,
                owners[0].end - owners[0].start,
                replacement,
            )
            .unwrap();
        let before = working.save();
        assert!(matches!(
            anchor.reconcile(&mut working).unwrap(),
            SourceReconciliation::Pending { .. }
        ));
        assert_eq!(working.save(), before);
    }
}

#[test]
fn forged_patch_and_unowned_edit_cannot_acquire_anchors() {
    let (accepted, mut patch, owners) = setup();
    assert!(matches!(
        AnchoredSourcePatch::capture(&accepted, "main.ts", &patch, &[]),
        Err(SourcePatchError::Ownership)
    ));
    patch.edits[0].expected = "18".into();
    assert!(AnchoredSourcePatch::capture(&accepted, "main.ts", &patch, &owners).is_err());
    let (accepted, mut patch, _) = setup();
    patch.base_source_digest = "forged".into();
    assert_eq!(
        patch.apply(accepted.capture().text("main.ts").unwrap()),
        Err(SourcePatchError::Digest)
    );
}

#[test]
fn replacing_a_file_with_identical_text_invalidates_old_anchors() {
    let (accepted, patch, owners) = setup();
    let anchor = AnchoredSourcePatch::capture(&accepted, "main.ts", &patch, &owners).unwrap();
    let source = accepted.capture().text("main.ts").unwrap().to_string();
    let mut working = accepted.fork(b"editor").unwrap();
    working.remove_file("main.ts").unwrap();
    working.create_file("main.ts", &source).unwrap();
    let before = working.save();
    assert!(matches!(
        anchor.reconcile(&mut working).unwrap(),
        SourceReconciliation::Pending { .. }
    ));
    assert_eq!(working.save(), before);
}

#[test]
fn file_rename_follows_stable_identity_and_retyped_identical_owner_is_pending() {
    let (accepted, patch, owners) = setup();
    let anchor = AnchoredSourcePatch::capture(&accepted, "main.ts", &patch, &owners).unwrap();
    let mut working = accepted.fork(b"editor").unwrap();
    working.rename_file("main.ts", "renamed.ts").unwrap();
    assert!(matches!(
        anchor.reconcile(&mut working).unwrap(),
        SourceReconciliation::Applied { .. }
    ));
    assert!(
        working
            .capture()
            .text("renamed.ts")
            .unwrap()
            .contains("width = 14")
    );
    let mut working = accepted.fork(b"editor-two").unwrap();
    working
        .splice(
            "main.ts",
            owners[0].start,
            owners[0].end - owners[0].start,
            "",
        )
        .unwrap();
    working
        .splice("main.ts", owners[0].start, 0, "const width = 12;")
        .unwrap();
    let before = working.save();
    assert!(matches!(
        anchor.reconcile(&mut working).unwrap(),
        SourceReconciliation::Pending { .. }
    ));
    assert_eq!(working.save(), before);
}
