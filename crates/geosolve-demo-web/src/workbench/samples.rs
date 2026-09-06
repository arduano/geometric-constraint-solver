// SPDX-License-Identifier: GPL-3.0-or-later
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]

/// Browser-session selection state for the one canonical bundled-sample
/// registry. Sample construction remains owned by `geosolve-sketch-code`;
/// this adapter stores only the selected stable key.
#[derive(Default)]
pub(crate) struct SampleCatalogState {
    selected: Option<String>,
}

impl SampleCatalogState {
    pub(crate) fn selected_key(&self) -> Option<&str> {
        self.selected.as_deref()
    }

    pub(crate) fn select_key(&mut self, key: &str) -> Result<String, String> {
        let key = geosolve_sketch_code::bundled_sample(key)
            .map(|sample| sample.key.to_owned())
            .ok_or_else(|| format!("bundled sample `{key}` is unavailable"))?;
        self.selected = Some(key.clone());
        Ok(key)
    }

    pub(crate) fn menu_markup(&self) -> String {
        super::code_projects::sample_group_markup(self.selected.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::SampleCatalogState;

    #[test]
    fn selection_resolves_only_canonical_sample_keys() {
        let mut catalog = SampleCatalogState::default();
        assert_eq!(catalog.selected_key(), None);
        assert_eq!(
            catalog.select_key("theo-jansen-leg").unwrap(),
            "theo-jansen-leg"
        );
        assert_eq!(catalog.selected_key(), Some("theo-jansen-leg"));
        let reviewed: serde_json::Value = serde_json::from_str(include_str!(
            "../../../geosolve-sketch-code/assets/bundled-sample-catalog.json"
        ))
        .expect("reviewed sample catalog contract");
        assert_eq!(reviewed["schema"], 1);
        for retired in reviewed["retired_keys"].as_array().unwrap() {
            assert!(catalog.select_key(retired.as_str().unwrap()).is_err());
            assert_eq!(catalog.selected_key(), Some("theo-jansen-leg"));
        }
    }

    #[test]
    fn menu_contains_each_canonical_sample_once_in_four_categories() {
        let markup = SampleCatalogState::default().menu_markup();
        let reviewed: serde_json::Value = serde_json::from_str(include_str!(
            "../../../geosolve-sketch-code/assets/bundled-sample-catalog.json"
        ))
        .expect("reviewed sample catalog contract");
        assert_eq!(reviewed["schema"], 1);
        let expected = reviewed["samples"].as_array().unwrap();
        let categories = expected
            .iter()
            .map(|sample| sample["category"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            markup.matches("data-sample-group-trigger").count(),
            categories.len()
        );
        assert_eq!(markup.matches("data-sample-id=").count(), expected.len());
        assert!(!markup.contains("data-code-sample-id="));
        let mut previous = 0;
        for sample in expected {
            let key = sample["key"].as_str().unwrap();
            let identity = format!("data-sample-id=\"{key}\"");
            assert_eq!(markup.matches(&identity).count(), 1, "{key}");
            let position = markup.find(&identity).unwrap();
            assert!(position >= previous, "{key} reviewed menu order");
            previous = position;
        }
        for retired in reviewed["retired_keys"].as_array().unwrap() {
            assert!(!markup.contains(&format!("data-sample-id=\"{}\"", retired.as_str().unwrap())));
        }
    }
}
