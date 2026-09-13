// SPDX-License-Identifier: GPL-3.0-or-later
//! Existing outer workspace presentation wire. Inner source/native authority has its own codec.
use crate::{DimensionDisplayMode, DimensionPresentationState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const WORKBENCH_PERSISTENCE_FORMAT: &str = "geosolve-workbench-presentation-v1";

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    #[default]
    Focused,
    All,
    Hidden,
}

impl From<DisplayMode> for DimensionDisplayMode {
    fn from(mode: DisplayMode) -> Self {
        match mode {
            DisplayMode::Focused => Self::Focused,
            DisplayMode::All => Self::All,
            DisplayMode::Hidden => Self::Hidden,
        }
    }
}

impl From<DimensionDisplayMode> for DisplayMode {
    fn from(mode: DimensionDisplayMode) -> Self {
        match mode {
            DimensionDisplayMode::Focused => Self::Focused,
            DimensionDisplayMode::All => Self::All,
            DimensionDisplayMode::Hidden => Self::Hidden,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DimensionPersistence {
    #[serde(default)]
    pub mode: DisplayMode,
    /// Exact native document/source/item identity; never resolved by a label or ordinal.
    #[serde(default)]
    pub pins: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchPersistenceEnvelope {
    pub format: String,
    pub project: String,
    pub presentation: WorkbenchPresentationPersistence,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkbenchPresentationPersistence {
    pub hidden_rows: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub isolate_restore: Option<Vec<String>>,
    pub construction_visible: bool,
    #[serde(default)]
    pub dimensions: DimensionPersistence,
}

impl DimensionPersistence {
    /// Exact persisted annotation identity shared by editing and detached views.
    #[must_use]
    pub fn pin_identity(key: crate::AnnotationLayoutKey) -> String {
        format!(
            "{}:{}:{:?}:{:?}:{:?}",
            key.document, key.source, key.item, key.kind, key.marker_index
        )
    }

    #[must_use]
    pub fn from_state(state: &DimensionPresentationState) -> Self {
        Self {
            mode: state.mode.into(),
            pins: state.pins.iter().copied().map(Self::pin_identity).collect(),
        }
    }

    /// # Errors
    /// Rejects excessive, duplicate or oversized persisted pin identities.
    pub fn validate(&self) -> Result<(), String> {
        if self.pins.len() > DimensionPresentationState::MAX_PINS
            || self.pins.iter().any(|pin| pin.len() > 1024)
            || self.pins.iter().collect::<BTreeSet<_>>().len() != self.pins.len()
        {
            return Err("dimension preferences contain invalid or excessive pins".into());
        }
        Ok(())
    }
}
impl WorkbenchPresentationPersistence {
    /// # Errors
    /// Rejects duplicate, oversized or excessive personal row/pin identities.
    pub fn validate(&self) -> Result<(), String> {
        validate_rows(&self.hidden_rows)?;
        if let Some(rows) = &self.isolate_restore {
            validate_rows(rows)?;
        }
        self.dimensions.validate()
    }
}
fn validate_rows(rows: &[String]) -> Result<(), String> {
    if rows.len() > 4096
        || rows.iter().any(|row| row.is_empty() || row.len() > 1024)
        || rows.iter().collect::<BTreeSet<_>>().len() != rows.len()
    {
        return Err(
            "Explorer visibility contains invalid, duplicate or excessive row identities".into(),
        );
    }
    Ok(())
}
impl WorkbenchPersistenceEnvelope {
    /// Strictly decodes the outer wire only. The caller must admit its inner authority.
    /// # Errors
    /// Rejects duplicate/unknown fields, invalid format, oversized transport or personal state.
    pub fn decode(json: &str) -> Result<Self, String> {
        if json.len() > 96 * 1024 * 1024 {
            return Err("workspace presentation exceeds its byte limit".into());
        }
        let envelope: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        if envelope.format != WORKBENCH_PERSISTENCE_FORMAT {
            return Err("unsupported workbench presentation format".into());
        }
        envelope.presentation.validate()?;
        Ok(envelope)
    }
}
