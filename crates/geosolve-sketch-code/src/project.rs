// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use geosolve_sketch_intent::intent_content_digest;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    ArtifactValidationError, CODE_PROJECT_LIMIT, CodeProject, PatchModuleArtifact, ProjectKey,
    parse_managed_source,
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
    Managed(#[from] crate::ManagedParseError),
}

impl CodeProject {
    /// Creates one artifact-free project whose complete authored authority is
    /// the bounded managed TypeScript source.
    ///
    /// This is the smallest code-only entry point: no ordinary GUI scene,
    /// custom patch file or precompiled artifact is required. The resulting
    /// project still expands through the ordinary intent/materialization path
    /// and therefore acquires no solver authority merely by parsing.
    ///
    /// # Errors
    ///
    /// Returns a managed-source or project-validation error before the
    /// candidate may be materialized.
    pub fn managed_only(project: ProjectKey, source: &str) -> Result<Self, CodeProjectError> {
        let managed = parse_managed_source(source)?;
        if managed
            .imports
            .iter()
            .any(|import| import.module != "@geosolve/sketch-code")
        {
            return Err(CodeProjectError::InvalidProject(
                "managed-only source cannot import a custom patch without its pinned artifact"
                    .into(),
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
        let reparsed = parse_managed_source(&self.managed.source)?;
        if reparsed != self.managed {
            return Err(CodeProjectError::InvalidProject(
                "managed document is not its source's canonical parse".into(),
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

    const CODE_ONLY_SOURCE: &str = r#""use geosolve managed-v1";
import { sketch } from "@geosolve/sketch-code";

export default sketch(($) => {
  const frame = $.geometry.rectangle("frame", {
    lowerLeft: [0, 0],
    upperRight: [60, 35],
  });
  const diagonal = $.geometry.line("diagonal", {
    start: frame.corners.lowerLeft,
    end: frame.corners.upperRight,
  });
  return $.outputs({ frame, diagonal });
});
"#;

    #[test]
    fn managed_only_project_is_valid_artifact_free_code_authority() {
        let project =
            CodeProject::managed_only(ProjectKey("code-only-test".into()), CODE_ONLY_SOURCE)
                .unwrap();
        assert!(project.custom_files.is_empty());
        assert!(project.artifacts.is_empty());
        assert_eq!(project.managed.program.declarations.len(), 2);
        assert_eq!(project.managed.source, CODE_ONLY_SOURCE);
        assert_eq!(project.lock["format"], "geosolve-lock-v1");
        assert_eq!(project.lock["modules"], serde_json::json!({}));
        assert!(project.validate().is_ok());
    }

    #[test]
    fn managed_only_project_rejects_invalid_source_and_project_brand() {
        assert!(matches!(
            CodeProject::managed_only(ProjectKey("code-only-test".into()), "not managed code"),
            Err(CodeProjectError::Managed(_))
        ));
        assert!(matches!(
            CodeProject::managed_only(ProjectKey(String::new()), CODE_ONLY_SOURCE),
            Err(CodeProjectError::InvalidProject(_))
        ));
        let custom_import = CODE_ONLY_SOURCE.replacen(
            "import { sketch } from \"@geosolve/sketch-code\";",
            concat!(
                "import { sketch } from \"@geosolve/sketch-code\";\n",
                "import { custom } from \"./patches/custom.patch.ts\";"
            ),
            1,
        );
        assert!(matches!(
            CodeProject::managed_only(ProjectKey("code-only-test".into()), &custom_import),
            Err(CodeProjectError::InvalidProject(message))
                if message.contains("cannot import a custom patch")
        ));
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
