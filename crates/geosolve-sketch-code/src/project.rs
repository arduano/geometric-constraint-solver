// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use geosolve_sketch_intent::intent_content_digest;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    ArtifactValidationError, CODE_PROJECT_LIMIT, CodeProject, CompiledManagedSource,
    ManagedValidationError, PatchModuleArtifact, ProjectKey,
};

const MAX_PROJECT_FILES: usize = 65_536;
const MAX_PROJECT_PATH_BYTES: usize = 1_024;
const MAX_PROJECT_PATH_SEGMENT_BYTES: usize = 256;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeProjectLock {
    format: String,
    modules: BTreeMap<String, CodeProjectModulePin>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CodeProjectModulePin {
    artifact: String,
    source: String,
    interface: String,
    sdk_abi: String,
}

/// Rejected offline code-project envelope.
#[derive(Clone, Debug, Error, PartialEq)]
#[non_exhaustive]
pub enum CodeProjectError {
    #[error("code project is {actual} bytes; the limit is {limit}")]
    ResourceLimit { actual: usize, limit: usize },
    #[error("code project JSON is invalid: {0}")]
    InvalidJson(String),
    #[error("code project is invalid: {0}")]
    InvalidProject(String),
    #[error(transparent)]
    Artifact(#[from] ArtifactValidationError),
    #[error(transparent)]
    Managed(#[from] ManagedValidationError),
}

impl CodeProject {
    /// Creates one empty managed project from the checked-in, executed V3
    /// bootstrap envelope. The first canvas or source edit still uses the
    /// ordinary prepared compiler transaction; no raw-source parser is
    /// involved in project construction.
    ///
    /// # Errors
    ///
    /// Returns a managed authority or project-validation error before native
    /// materialization.
    pub fn empty(project: ProjectKey) -> Result<Self, CodeProjectError> {
        let compiled = CompiledManagedSource::from_json(include_str!(
            "../assets/demos/authored-empty.compiled.json"
        ))?;
        Self::managed(project, compiled)
    }

    /// Creates one artifact-free managed project from an independently
    /// validated browser or Deno compiler handoff.
    ///
    /// # Errors
    ///
    /// Returns a managed authority or project-validation error before native
    /// materialization.
    pub fn managed(
        project: ProjectKey,
        compiled: CompiledManagedSource,
    ) -> Result<Self, CodeProjectError> {
        let managed = compiled.into_managed_document()?;
        if managed
            .imports
            .iter()
            .any(|import| import.module != "@geosolve/sketch-code")
        {
            return Err(CodeProjectError::InvalidProject(
                "artifact-free managed source cannot import a custom patch".into(),
            ));
        }
        let project = Self {
            project,
            managed,
            custom_files: BTreeMap::new(),
            artifacts: BTreeMap::new(),
            lock: serde_json::json!({
                "format": "geosolve-lock-v1",
                "modules": {},
            }),
        };
        project.validate()?;
        Ok(project)
    }

    /// Validates every managed/custom file, precompiled artifact and exact lock
    /// pin without evaluating custom TypeScript.
    ///
    /// # Errors
    ///
    /// Returns a typed managed-source, artifact, lock, digest, path or resource
    /// error before the project can acquire authority.
    #[allow(
        clippy::too_many_lines,
        reason = "one audit pass authenticates the complete offline project cross-links"
    )]
    pub fn validate(&self) -> Result<(), CodeProjectError> {
        if self.project.0.is_empty()
            || self.project.0.len() > 256
            || self.project.0.chars().any(char::is_control)
        {
            return Err(CodeProjectError::InvalidProject(
                "invalid project brand".into(),
            ));
        }
        if let Some(compiled) = self.managed.compiled.as_deref() {
            compiled.validate()?;
            if compiled.normalized_source != self.managed.source {
                return Err(CodeProjectError::InvalidProject(
                    "managed source differs from its normalized compiler authority".into(),
                ));
            }
            let projected = compiled.projected_managed_document()?;
            let mut candidate = self.managed.clone();
            candidate.compiled = None;
            let mut projected = projected;
            projected.declaration_name_high_water = candidate.declaration_name_high_water;
            if projected != candidate {
                return Err(CodeProjectError::InvalidProject(
                    "managed equation-free projection differs from its executed authority".into(),
                ));
            }
        } else {
            return Err(CodeProjectError::InvalidProject(
                "managed project lacks compiled authority".into(),
            ));
        }
        if self.custom_files.len() > MAX_PROJECT_FILES || self.artifacts.len() > MAX_PROJECT_FILES {
            return Err(CodeProjectError::InvalidProject(format!(
                "code project file or artifact count exceeds {MAX_PROJECT_FILES}"
            )));
        }
        for (path, file) in &self.custom_files {
            if path != &file.path || file.managed || !valid_patch_path(path) {
                return Err(CodeProjectError::InvalidProject(format!(
                    "invalid custom patch path `{path}`"
                )));
            }
            let digest = intent_content_digest(file.contents.as_bytes()).to_string();
            if digest != file.source_digest {
                return Err(CodeProjectError::InvalidProject(format!(
                    "custom patch `{path}` digest mismatch"
                )));
            }
        }

        let lock: CodeProjectLock = serde_json::from_value(self.lock.clone())
            .map_err(|error| CodeProjectError::InvalidProject(format!("invalid lock: {error}")))?;
        if lock.format != "geosolve-lock-v1" {
            return Err(CodeProjectError::InvalidProject(
                "missing geosolve-lock-v1 format".into(),
            ));
        }
        let modules = lock.modules;
        for (digest, value) in &self.artifacts {
            let definition: PatchModuleArtifact = serde_json::from_value(value.clone())
                .map_err(|error| CodeProjectError::InvalidJson(error.to_string()))?;
            let artifact = definition.validate()?;
            if artifact.digest() != digest {
                return Err(CodeProjectError::InvalidProject(format!(
                    "artifact key `{digest}` does not match canonical bytes"
                )));
            }
            let definition = artifact.artifact();
            let source_path = definition
                .module_specifier
                .strip_prefix("./")
                .ok_or_else(|| {
                    CodeProjectError::InvalidProject(format!(
                        "artifact `{digest}` module specifier is not relative"
                    ))
                })?;
            let Some(custom) = self.custom_files.get(source_path) else {
                return Err(CodeProjectError::InvalidProject(format!(
                    "artifact `{digest}` has no custom source `{source_path}`"
                )));
            };
            if custom.source_digest != definition.source_digest {
                return Err(CodeProjectError::InvalidProject(format!(
                    "artifact `{digest}` source pin is stale"
                )));
            }
            let Some(pin) = modules.get(&definition.module_specifier) else {
                return Err(CodeProjectError::InvalidProject(format!(
                    "artifact `{digest}` is absent from the lock"
                )));
            };
            if pin.artifact != *digest
                || pin.source != definition.source_digest
                || pin.interface != definition.interface_digest
                || pin.sdk_abi != definition.sdk_abi
            {
                return Err(CodeProjectError::InvalidProject(format!(
                    "artifact `{digest}` lock pin is incomplete or stale"
                )));
            }
        }
        if modules.len() != self.artifacts.len() {
            return Err(CodeProjectError::InvalidProject(
                "lock and artifact sets differ".into(),
            ));
        }
        let bytes = serde_json::to_vec(self)
            .map_err(|error| CodeProjectError::InvalidJson(error.to_string()))?;
        if bytes.len() > CODE_PROJECT_LIMIT {
            return Err(CodeProjectError::ResourceLimit {
                actual: bytes.len(),
                limit: CODE_PROJECT_LIMIT,
            });
        }
        Ok(())
    }

    /// Encodes the fully validated project into deterministic JSON.
    ///
    /// # Errors
    ///
    /// Returns the same validation errors as [`Self::validate`] or a JSON
    /// encoding error.
    pub fn to_canonical_json(&self) -> Result<String, CodeProjectError> {
        self.validate()?;
        serde_json::to_string(self)
            .map_err(|error| CodeProjectError::InvalidJson(error.to_string()))
    }

    /// Decodes into a fresh candidate and validates all offline authority
    /// before returning it to the caller for atomic replacement.
    ///
    /// # Errors
    ///
    /// Returns a resource, JSON, managed-source, artifact or lock error.
    pub fn from_json(json: &str) -> Result<Self, CodeProjectError> {
        if json.len() > CODE_PROJECT_LIMIT {
            return Err(CodeProjectError::ResourceLimit {
                actual: json.len(),
                limit: CODE_PROJECT_LIMIT,
            });
        }
        let project: Self = serde_json::from_str(json)
            .map_err(|error| CodeProjectError::InvalidJson(error.to_string()))?;
        project.validate()?;
        Ok(project)
    }
}

fn valid_patch_path(path: &str) -> bool {
    if path.len() > MAX_PROJECT_PATH_BYTES
        || !path.starts_with("patches/")
        || !path.ends_with(".patch.ts")
        || path.contains('\\')
        || path.chars().any(char::is_control)
    {
        return false;
    }
    let mut segments = path.split('/');
    if segments.next() != Some("patches") {
        return false;
    }
    segments.all(|segment| {
        !segment.is_empty()
            && segment != "."
            && segment != ".."
            && segment.len() <= MAX_PROJECT_PATH_SEGMENT_BYTES
    })
}

#[cfg(test)]
mod tests {
    use crate::bundled_code_project_demos;

    use super::*;

    #[test]
    fn empty_project_uses_checked_in_executed_v3_authority() {
        let project = CodeProject::empty(ProjectKey("empty-project-test".into())).unwrap();
        assert!(project.custom_files.is_empty());
        assert!(project.artifacts.is_empty());
        assert!(project.managed.program.declarations.is_empty());
        assert_eq!(
            project.managed.source,
            include_str!("../assets/demos/authored-empty.sketch.ts")
        );
        assert!(project.validate().is_ok());
    }

    #[test]
    fn all_bundled_projects_round_trip_with_custom_source_byte_identity() {
        for demo in bundled_code_project_demos() {
            let project = demo.project();
            project.validate().unwrap();
            let json = project.to_canonical_json().unwrap();
            let restored = CodeProject::from_json(&json).unwrap();
            assert_eq!(restored, project);
            assert_eq!(restored.to_canonical_json().unwrap(), json);
            for (path, source) in demo.custom_files {
                assert_eq!(restored.custom_files[path].contents, source);
            }
        }
    }

    #[test]
    fn tampered_custom_source_artifact_and_lock_fail_closed() {
        let mut custom = bundled_code_project_demos().remove(0).project();
        custom
            .custom_files
            .values_mut()
            .next()
            .unwrap()
            .contents
            .push_str("// tampered");
        assert!(matches!(
            custom.validate(),
            Err(CodeProjectError::InvalidProject(_))
        ));

        let mut artifact = bundled_code_project_demos().remove(0).project();
        let value = artifact.artifacts.values_mut().next().unwrap();
        value["export_name"] = serde_json::json!("different");
        assert!(matches!(
            artifact.validate(),
            Err(CodeProjectError::Artifact(
                ArtifactValidationError::InterfaceDigestMismatch { .. }
            ))
        ));

        let mut lock = bundled_code_project_demos().remove(0).project();
        lock.lock["modules"] = serde_json::json!({});
        assert!(matches!(
            lock.validate(),
            Err(CodeProjectError::InvalidProject(_))
        ));
    }

    #[test]
    fn oversized_envelope_rejects_before_decode() {
        let oversized = " ".repeat(CODE_PROJECT_LIMIT + 1);
        assert!(matches!(
            CodeProject::from_json(&oversized),
            Err(CodeProjectError::ResourceLimit { .. })
        ));
    }

    #[test]
    fn project_json_rejects_unknown_top_level_authority() {
        let project = bundled_code_project_demos().remove(0).project();
        let mut value: serde_json::Value =
            serde_json::from_str(&project.to_canonical_json().unwrap()).unwrap();
        value["unexpectedAuthority"] = serde_json::json!(true);
        let error = CodeProject::from_json(&serde_json::to_string(&value).unwrap()).unwrap_err();
        assert!(
            matches!(error, CodeProjectError::InvalidJson(ref message) if message.contains("unknown field")),
            "unknown project authority must reject at the strict JSON boundary: {error}",
        );
    }

    #[test]
    fn lock_and_patch_paths_are_exact_closed_authority() {
        let mut extra_lock = bundled_code_project_demos().remove(0).project();
        extra_lock.lock["unexpected"] = serde_json::json!(true);
        assert!(matches!(
            extra_lock.validate(),
            Err(CodeProjectError::InvalidProject(_))
        ));

        let mut extra_pin = bundled_code_project_demos().remove(0).project();
        extra_pin.lock["modules"]
            .as_object_mut()
            .unwrap()
            .values_mut()
            .next()
            .unwrap()["unexpected"] = serde_json::json!(true);
        assert!(matches!(
            extra_pin.validate(),
            Err(CodeProjectError::InvalidProject(_))
        ));

        for invalid in [
            "patches//helper.patch.ts",
            "patches/./helper.patch.ts",
            "patches/nested/../helper.patch.ts",
            "patches\\helper.patch.ts",
        ] {
            let mut project = bundled_code_project_demos().remove(0).project();
            let (_, mut file) = project.custom_files.pop_first().unwrap();
            file.path = invalid.into();
            project.custom_files.insert(invalid.into(), file);
            assert!(matches!(
                project.validate(),
                Err(CodeProjectError::InvalidProject(_))
            ));
        }
    }
}
