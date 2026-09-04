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
        assert!(catalog.select_key("drafting-compass").is_err());
        assert_eq!(catalog.selected_key(), Some("theo-jansen-leg"));
    }

    #[test]
    fn menu_contains_each_canonical_sample_once_in_four_categories() {
        let markup = SampleCatalogState::default().menu_markup();
        assert_eq!(markup.matches("data-sample-group-trigger").count(), 4);
        assert_eq!(markup.matches("data-sample-id=").count(), 20);
        assert!(!markup.contains("data-code-sample-id="));
        for sample in geosolve_sketch_code::bundled_sample_catalog() {
            assert_eq!(
                markup
                    .matches(&format!("data-sample-id=\"{}\"", sample.key))
                    .count(),
                1,
                "{}",
                sample.key
            );
        }
    }
}
