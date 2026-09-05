// SPDX-License-Identifier: GPL-3.0-or-later

//! Canonical manifest-driven bundled-sample registry.

use std::{collections::BTreeMap, sync::OnceLock};

use geosolve_sketch_intent::intent_content_digest;
use miniz_oxide::{
    DataFormat, MZFlush, MZStatus,
    inflate::stream::{InflateState, inflate},
};
use serde::{Deserialize, Serialize};

use crate::{
    CodeProject, CodeProjectFile, CompiledManagedSource, MANAGED_WIRE_LIMIT, PatchModuleArtifact,
    ProjectKey,
};

/// Stable user-facing section of the bundled sample catalog.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleCategory {
    Mechanism,
    ProductFabrication,
    ReferenceLab,
    ScaleStudy,
}

impl SampleCategory {
    /// Stable label shared by the frontend and other catalog consumers.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Mechanism => "Mechanisms",
            Self::ProductFabrication => "Products & fabrication",
            Self::ReferenceLab => "Reference labs",
            Self::ScaleStudy => "Scale studies",
        }
    }
}

/// Reviewed mobility expected from one independently validated accepted solve.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SampleExpected {
    pub raw_dof: usize,
    pub effective_dof: usize,
}

impl SampleExpected {
    const fn new(raw_dof: usize, effective_dof: usize) -> Self {
        Self {
            raw_dof,
            effective_dof,
        }
    }

    /// Expected numerical equality-system right nullity.
    #[must_use]
    pub const fn numerical_right_nullity(self) -> usize {
        self.raw_dof
    }

    /// Expected equality degrees of freedom.
    #[must_use]
    pub const fn equality_degrees_of_freedom(self) -> usize {
        self.raw_dof
    }

    /// Expected bidirectional mobility after active-bound classification.
    #[must_use]
    pub const fn bidirectional_bounded_degrees_of_freedom(self) -> usize {
        self.effective_dof
    }
}

/// How one provenance record relates to a bundled 2D study.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleProvenanceRelationship {
    Original,
    DimensionsOnly,
    Adapted,
}

/// Immutable attribution and scope for one source that informed a sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SampleProvenance {
    pub relationship: SampleProvenanceRelationship,
    pub name: &'static str,
    pub url: Option<&'static str>,
    pub revision: Option<&'static str>,
    pub path: Option<&'static str>,
    pub licence: &'static str,
    pub scope: &'static str,
    pub notice_required: bool,
}

impl SampleProvenance {
    #[allow(
        clippy::too_many_arguments,
        reason = "generated immutable provenance mirrors the deliberately explicit manifest record"
    )]
    const fn new(
        relationship: SampleProvenanceRelationship,
        name: &'static str,
        url: Option<&'static str>,
        revision: Option<&'static str>,
        path: Option<&'static str>,
        licence: &'static str,
        scope: &'static str,
        notice_required: bool,
    ) -> Self {
        Self {
            relationship,
            name,
            url,
            revision,
            path,
            licence,
            scope,
            notice_required,
        }
    }
}

#[derive(Debug)]
struct BundledCompilerEnvelope {
    compressed: &'static [u8],
    decompressed_len: usize,
    text: OnceLock<String>,
}

#[derive(Clone, Copy, Debug)]
struct BundledSamplePatch {
    path: &'static str,
    source: &'static str,
    artifact_json: &'static str,
}

impl BundledSamplePatch {
    #[allow(
        dead_code,
        reason = "generated registries without custom patch samples do not call this constructor"
    )]
    const fn new(path: &'static str, source: &'static str, artifact_json: &'static str) -> Self {
        Self {
            path,
            source,
            artifact_json,
        }
    }
}

impl BundledCompilerEnvelope {
    const fn new(compressed: &'static [u8], decompressed_len: usize) -> Self {
        Self {
            compressed,
            decompressed_len,
            text: OnceLock::new(),
        }
    }

    fn text(&'static self) -> &'static str {
        self.text
            .get_or_init(|| {
                decompress_bundled_compiler_envelope(self.compressed, self.decompressed_len)
                    .unwrap_or_else(|error| {
                        panic!("build-validated bundled compiler envelope is valid: {error:?}")
                    })
            })
            .as_str()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BundledEnvelopeError {
    CompressedLimitExceeded { actual: usize, limit: usize },
    DecompressedLimitExceeded { declared: usize, limit: usize },
    DecompressionFailed,
    TrailingInput { consumed: usize, available: usize },
    LengthMismatch { declared: usize, actual: usize },
    InvalidUtf8,
}

fn decompress_bundled_compiler_envelope(
    compressed: &[u8],
    decompressed_len: usize,
) -> Result<String, BundledEnvelopeError> {
    if compressed.len() > MANAGED_WIRE_LIMIT {
        return Err(BundledEnvelopeError::CompressedLimitExceeded {
            actual: compressed.len(),
            limit: MANAGED_WIRE_LIMIT,
        });
    }
    if decompressed_len > MANAGED_WIRE_LIMIT {
        return Err(BundledEnvelopeError::DecompressedLimitExceeded {
            declared: decompressed_len,
            limit: MANAGED_WIRE_LIMIT,
        });
    }

    let mut output = vec![0; decompressed_len];
    let mut state = InflateState::new_boxed(DataFormat::Zlib);
    let result = inflate(&mut state, compressed, &mut output, MZFlush::Finish);
    if result.status != Ok(MZStatus::StreamEnd) {
        return Err(BundledEnvelopeError::DecompressionFailed);
    }
    if result.bytes_consumed != compressed.len() {
        return Err(BundledEnvelopeError::TrailingInput {
            consumed: result.bytes_consumed,
            available: compressed.len(),
        });
    }
    if result.bytes_written != decompressed_len {
        return Err(BundledEnvelopeError::LengthMismatch {
            declared: decompressed_len,
            actual: result.bytes_written,
        });
    }
    String::from_utf8(output).map_err(|_| BundledEnvelopeError::InvalidUtf8)
}

/// Complete immutable authority for one bundled source-defined sample.
#[derive(Debug)]
pub struct BundledSampleSpec {
    pub ordinal: usize,
    pub key: &'static str,
    pub title: &'static str,
    pub category: SampleCategory,
    pub summary: &'static str,
    pub expected: SampleExpected,
    pub functional_groups: &'static [&'static str],
    pub provenance: &'static [SampleProvenance],
    manifest_json: &'static str,
    managed_source: &'static str,
    compiled: &'static BundledCompilerEnvelope,
    witnesses_json: &'static str,
    notice: Option<&'static str>,
    patches: &'static [BundledSamplePatch],
}

impl BundledSampleSpec {
    #[allow(
        clippy::too_many_arguments,
        reason = "only generated registry code constructs this complete immutable sample authority"
    )]
    const fn new(
        ordinal: usize,
        key: &'static str,
        title: &'static str,
        category: SampleCategory,
        summary: &'static str,
        expected: SampleExpected,
        functional_groups: &'static [&'static str],
        provenance: &'static [SampleProvenance],
        manifest_json: &'static str,
        managed_source: &'static str,
        compiled: &'static BundledCompilerEnvelope,
        witnesses_json: &'static str,
        notice: Option<&'static str>,
        patches: &'static [BundledSamplePatch],
    ) -> Self {
        Self {
            ordinal,
            key,
            title,
            category,
            summary,
            expected,
            functional_groups,
            provenance,
            manifest_json,
            managed_source,
            compiled,
            witnesses_json,
            notice,
            patches,
        }
    }

    /// Exact checked-in manifest bytes validated by the build.
    #[must_use]
    pub const fn manifest_json(&self) -> &'static str {
        self.manifest_json
    }

    /// Exact TypeScript source authenticated by the compiled envelope.
    #[must_use]
    pub const fn managed_source(&self) -> &'static str {
        self.managed_source
    }

    /// Lazily reconstructs the exact bounded compiler-envelope bytes.
    #[must_use]
    pub fn compiled_source(&'static self) -> &'static str {
        self.compiled.text()
    }

    /// Exact checked-in edit and drag witness bytes.
    #[must_use]
    pub const fn witnesses_json(&self) -> &'static str {
        self.witnesses_json
    }

    /// Adjacent attribution notice when required by provenance.
    #[must_use]
    pub const fn notice(&self) -> Option<&'static str> {
        self.notice
    }

    /// Builds the ordinary bounded code project authenticated by this sample.
    ///
    /// # Panics
    ///
    /// Panics only if source-controlled bytes that passed the build validator
    /// no longer satisfy the public compiler/project contract.
    #[must_use]
    pub fn project(&'static self) -> CodeProject {
        let compiled =
            CompiledManagedSource::from_json(self.compiled_source()).unwrap_or_else(|error| {
                panic!(
                    "bundled sample `{}` compiler authority is valid: {error:?}",
                    self.key
                )
            });
        assert_eq!(
            compiled.normalized_source, self.managed_source,
            "bundled source must match its authenticated compiler envelope"
        );
        let managed = compiled.into_managed_document().unwrap_or_else(|error| {
            panic!(
                "bundled sample `{}` projects to managed authority: {error:?}",
                self.key
            )
        });
        let mut custom_files = BTreeMap::new();
        let mut artifacts = BTreeMap::new();
        let mut pins = BTreeMap::new();
        for patch in self.patches {
            let source_digest = intent_content_digest(patch.source.as_bytes()).to_string();
            custom_files.insert(
                patch.path.to_owned(),
                CodeProjectFile {
                    path: patch.path.to_owned(),
                    source_digest,
                    contents: patch.source.to_owned(),
                    managed: false,
                },
            );
            let definition: PatchModuleArtifact = serde_json::from_str(patch.artifact_json)
                .unwrap_or_else(|error| {
                    panic!(
                        "bundled sample `{}` patch `{}` is valid JSON: {error}",
                        self.key, patch.path
                    )
                });
            let validated = definition.clone().validate().unwrap_or_else(|error| {
                panic!(
                    "bundled sample `{}` patch `{}` is valid: {error:?}",
                    self.key, patch.path
                )
            });
            let value = serde_json::from_str(validated.canonical_json())
                .expect("validated canonical patch artifact is JSON");
            pins.insert(
                definition.module_specifier.clone(),
                serde_json::json!({
                    "artifact": validated.digest(),
                    "source": definition.source_digest,
                    "interface": definition.interface_digest,
                    "sdk_abi": definition.sdk_abi,
                }),
            );
            artifacts.insert(validated.digest().to_owned(), value);
        }
        let project = CodeProject {
            project: ProjectKey(format!("geosolve-sample-{}", self.key)),
            managed,
            custom_files,
            artifacts,
            lock: serde_json::json!({
                "format": "geosolve-lock-v1",
                "modules": pins,
            }),
        };
        project
            .validate()
            .unwrap_or_else(|error| panic!("bundled sample `{}` is valid: {error:?}", self.key));
        project
    }
}

include!(concat!(
    env!("OUT_DIR"),
    "/bundled-compiler-envelopes/bundled-sample-registry.rs"
));

/// Exact canonical bundled-sample order generated from colocated manifests.
#[must_use]
pub fn bundled_sample_catalog() -> &'static [BundledSampleSpec] {
    &BUNDLED_SAMPLES
}

/// Resolves one exact canonical key without loading any compiler envelope.
#[must_use]
pub fn bundled_sample(key: &str) -> Option<&'static BundledSampleSpec> {
    BUNDLED_SAMPLES.iter().find(|sample| sample.key == key)
}

#[cfg(test)]
mod tests {
    use miniz_oxide::deflate::compress_to_vec_zlib;

    use super::*;

    #[test]
    fn bounded_decompression_rejects_corruption_and_trailing_input() {
        let payload = br#"{"format":"fixture","value":42}"#;
        let compressed = compress_to_vec_zlib(payload, 10);
        assert_eq!(
            decompress_bundled_compiler_envelope(&compressed, payload.len()).unwrap(),
            std::str::from_utf8(payload).unwrap()
        );
        assert!(matches!(
            decompress_bundled_compiler_envelope(&compressed, MANAGED_WIRE_LIMIT + 1),
            Err(BundledEnvelopeError::DecompressedLimitExceeded { .. })
        ));
        let mut trailing = compressed.clone();
        trailing.extend_from_slice(b"trailing");
        assert!(matches!(
            decompress_bundled_compiler_envelope(&trailing, payload.len()),
            Err(BundledEnvelopeError::TrailingInput { .. })
        ));
        let truncated = &compressed[..compressed.len() - 1];
        assert_eq!(
            decompress_bundled_compiler_envelope(truncated, payload.len()),
            Err(BundledEnvelopeError::DecompressionFailed)
        );
    }

    #[test]
    fn catalog_lookup_does_not_eagerly_expand_envelopes() {
        for sample in bundled_sample_catalog() {
            assert!(sample.compiled.text.get().is_none());
        }
        let selected = bundled_sample("peaucellier-linkage").expect("known sample");
        assert!(selected.compiled.text.get().is_none());
        assert!(!selected.compiled_source().is_empty());
        assert!(selected.compiled.text.get().is_some());
        for sample in bundled_sample_catalog() {
            if sample.key != selected.key {
                assert!(sample.compiled.text.get().is_none());
            }
        }
    }
}
