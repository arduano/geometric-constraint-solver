// SPDX-License-Identifier: GPL-3.0-or-later

pub fn ordinal(key: &str) -> usize {
    let contract: serde_json::Value =
        serde_json::from_str(include_str!("../../assets/bundled-sample-catalog.json"))
            .expect("reviewed sample catalog contract");
    assert_eq!(contract["schema"], 1);
    contract["samples"]
        .as_array()
        .unwrap()
        .iter()
        .position(|sample| sample["key"] == key)
        .unwrap_or_else(|| panic!("dedicated sample `{key}` is absent from the reviewed catalog"))
        + 1
}
