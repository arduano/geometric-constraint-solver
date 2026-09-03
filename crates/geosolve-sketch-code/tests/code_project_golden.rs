// SPDX-License-Identifier: GPL-3.0-or-later

use geosolve_sketch_code::bundled_code_project_demos;

const HEADER: &str = "# SPDX-License-Identifier: GPL-3.0-or-later\ndemo\tmanaged_sha256\tartifact_sha256\tcustom_sha256\tdeclarations\tgenerated_members\ttyped_outputs\n";
const REVIEWED: &str = include_str!("golden/m90_code_project_ledger.tsv");

#[test]
fn m90_code_project_ledger_is_deterministic_and_reviewed_separately() {
    let mut actual = String::from(HEADER);
    for demo in bundled_code_project_demos() {
        actual.push_str(&demo.ledger_row());
        actual.push('\n');
    }
    assert_eq!(actual, REVIEWED);
}
