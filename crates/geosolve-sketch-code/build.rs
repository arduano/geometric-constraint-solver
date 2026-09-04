// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use miniz_oxide::{deflate::compress_to_vec_zlib, inflate::decompress_to_vec_zlib};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const COMPRESSION_LEVEL: u8 = 10;
const MANIFEST_FORMAT: &str = "geosolve-bundled-sample-v1";
const WITNESSES_FORMAT: &str = "geosolve-sample-witnesses-v1";
const SAMPLE_COUNT: usize = 20;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum ManifestCategory {
    Mechanism,
    ProductFabrication,
    ReferenceLab,
    ScaleStudy,
}

impl ManifestCategory {
    const fn rust_variant(self) -> &'static str {
        match self {
            Self::Mechanism => "SampleCategory::Mechanism",
            Self::ProductFabrication => "SampleCategory::ProductFabrication",
            Self::ReferenceLab => "SampleCategory::ReferenceLab",
            Self::ScaleStudy => "SampleCategory::ScaleStudy",
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Mechanism => "Mechanisms",
            Self::ProductFabrication => "Products & fabrication",
            Self::ReferenceLab => "Reference labs",
            Self::ScaleStudy => "Scale studies",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
enum ManifestProvenanceRelationship {
    Original,
    DimensionsOnly,
    Adapted,
}

impl ManifestProvenanceRelationship {
    const fn rust_variant(self) -> &'static str {
        match self {
            Self::Original => "SampleProvenanceRelationship::Original",
            Self::DimensionsOnly => "SampleProvenanceRelationship::DimensionsOnly",
            Self::Adapted => "SampleProvenanceRelationship::Adapted",
        }
    }

    const fn requires_external_source(self) -> bool {
        !matches!(self, Self::Original)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestExpected {
    raw_dof: usize,
    effective_dof: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestProvenance {
    relationship: ManifestProvenanceRelationship,
    name: String,
    url: Option<String>,
    revision: Option<String>,
    path: Option<String>,
    licence: String,
    scope: String,
    #[serde(default)]
    notice_required: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SampleManifest {
    format: String,
    ordinal: usize,
    key: String,
    title: String,
    summary: String,
    category: ManifestCategory,
    expected: ManifestExpected,
    groups: Vec<String>,
    provenance: Vec<ManifestProvenance>,
}

struct ValidatedSample {
    manifest: SampleManifest,
    directory: PathBuf,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrontendSample<'a> {
    ordinal: usize,
    stable_id: String,
    key: &'a str,
    title: &'a str,
    category: ManifestCategory,
    group: &'static str,
    summary: &'a str,
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo provides CARGO_MANIFEST_DIR"),
    );
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"))
        .join("bundled-compiler-envelopes");

    // These two M91 asset families remain only while their consumers are
    // clean-broken in the integration branch. They do not feed the M92
    // registry generated below.
    for category in ["samples", "demos"] {
        compress_flat_category(&manifest_dir, &output_dir, category);
    }

    let samples = validate_bundled_samples(&manifest_dir);
    compress_bundled_samples(&samples, &output_dir);
    generate_registry(&samples, &output_dir);
    generate_frontend_manifest(&samples, &output_dir);
}

fn compress_flat_category(manifest_dir: &Path, output_dir: &Path, category: &str) {
    let source_dir = manifest_dir.join("assets").join(category);
    let destination_dir = output_dir.join(category);
    println!("cargo:rerun-if-changed={}", source_dir.display());
    recreate_directory(&destination_dir);

    let mut sources = directory_paths(&source_dir)
        .into_iter()
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".compiled.json"))
        })
        .collect::<Vec<_>>();
    sources.sort();
    for source in sources {
        println!("cargo:rerun-if-changed={}", source.display());
        compress_envelope(&source, &destination_dir, None);
    }
}

fn validate_bundled_samples(manifest_dir: &Path) -> Vec<ValidatedSample> {
    let root = manifest_dir.join("assets/bundled-samples");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut samples = directory_paths(&root)
        .into_iter()
        .filter(|path| path.is_dir())
        .map(validate_sample)
        .collect::<Vec<_>>();
    samples.sort_by_key(|sample| sample.manifest.ordinal);

    assert_eq!(
        samples.len(),
        SAMPLE_COUNT,
        "bundled-sample registry must contain exactly {SAMPLE_COUNT} directories"
    );

    let mut keys = BTreeSet::new();
    let mut titles = BTreeSet::new();
    let mut categories = BTreeMap::<ManifestCategory, usize>::new();
    for (index, sample) in samples.iter().enumerate() {
        let manifest = &sample.manifest;
        assert_eq!(
            manifest.ordinal,
            index + 1,
            "bundled-sample ordinals must be exactly 1..={SAMPLE_COUNT}"
        );
        assert!(
            keys.insert(manifest.key.as_str()),
            "duplicate bundled-sample key `{}`",
            manifest.key
        );
        assert!(
            titles.insert(manifest.title.as_str()),
            "duplicate bundled-sample title `{}`",
            manifest.title
        );
        *categories.entry(manifest.category).or_default() += 1;
    }
    assert_eq!(categories.get(&ManifestCategory::Mechanism), Some(&5));
    assert_eq!(
        categories.get(&ManifestCategory::ProductFabrication),
        Some(&11)
    );
    assert_eq!(categories.get(&ManifestCategory::ReferenceLab), Some(&2));
    assert_eq!(categories.get(&ManifestCategory::ScaleStudy), Some(&2));

    samples
}

fn validate_sample(directory: PathBuf) -> ValidatedSample {
    let directory_name = directory
        .file_name()
        .and_then(|name| name.to_str())
        .expect("bundled-sample directory name is UTF-8");
    let manifest_path = directory.join("manifest.json");
    let source_path = directory.join("sketch.ts");
    let compiled_path = directory.join("sketch.compiled.json");
    let witnesses_path = directory.join("witnesses.json");
    for path in [
        &manifest_path,
        &source_path,
        &compiled_path,
        &witnesses_path,
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
        assert!(
            path.is_file(),
            "required bundled-sample asset `{}` is missing",
            path.display()
        );
    }

    let manifest: SampleManifest = read_json(&manifest_path, "bundled-sample manifest");
    assert_eq!(
        manifest.format, MANIFEST_FORMAT,
        "unsupported manifest format"
    );
    assert_eq!(
        manifest.key, directory_name,
        "manifest key must equal its directory name"
    );
    assert_valid_key(&manifest.key);
    assert_meaningful(&manifest.title, "sample title");
    assert_meaningful(&manifest.summary, "sample summary");
    validate_expected(&manifest);
    validate_provenance(&manifest, &directory);

    let source = fs::read_to_string(&source_path).unwrap_or_else(|error| {
        panic!(
            "read bundled-sample source `{}`: {error}",
            source_path.display()
        )
    });
    let compiled: Value = read_json(&compiled_path, "bundled-sample compiler envelope");
    validate_compiler_envelope(&manifest, &source, &compiled);

    let witnesses: Value = read_json(&witnesses_path, "bundled-sample witnesses");
    assert_eq!(
        witnesses.get("format").and_then(Value::as_str),
        Some(WITNESSES_FORMAT),
        "{} witnesses must declare `{WITNESSES_FORMAT}`",
        manifest.key
    );
    assert!(
        witnesses.get("representative_edit").is_some(),
        "{} witnesses must declare a representative edit",
        manifest.key
    );
    assert!(
        witnesses.get("drags").and_then(Value::as_array).is_some(),
        "{} witnesses must declare a drag array",
        manifest.key
    );

    ValidatedSample {
        manifest,
        directory,
    }
}

fn validate_expected(manifest: &SampleManifest) {
    assert!(
        manifest.expected.effective_dof <= manifest.expected.raw_dof,
        "{} effective DOF cannot exceed raw DOF",
        manifest.key
    );
    assert!(
        manifest.expected.raw_dof <= 1_000_000,
        "{} expected mobility exceeds the registry resource bound",
        manifest.key
    );
}

fn validate_provenance(manifest: &SampleManifest, directory: &Path) {
    let mut identities = BTreeSet::new();
    let mut notice_required = false;
    for provenance in &manifest.provenance {
        assert_meaningful(&provenance.name, "provenance name");
        assert_meaningful(&provenance.licence, "provenance licence");
        assert_meaningful(&provenance.scope, "provenance scope");
        assert!(
            identities.insert((provenance.relationship, provenance.name.as_str())),
            "{} repeats provenance `{}`",
            manifest.key,
            provenance.name
        );
        if provenance.relationship.requires_external_source() {
            let url = required_optional_field(manifest, "url", provenance.url.as_deref());
            assert!(
                url.starts_with("https://"),
                "{} provenance URL must use HTTPS",
                manifest.key
            );
            let revision =
                required_optional_field(manifest, "revision", provenance.revision.as_deref());
            assert!(
                !matches!(revision, "main" | "master" | "latest" | "HEAD"),
                "{} provenance revision must be immutable",
                manifest.key
            );
            required_optional_field(manifest, "path", provenance.path.as_deref());
        } else {
            assert!(
                provenance.url.is_none()
                    && provenance.revision.is_none()
                    && provenance.path.is_none(),
                "{} original provenance must not invent an external source",
                manifest.key
            );
        }
        notice_required |= provenance.notice_required;
    }

    let notice_path = directory.join("NOTICE.md");
    println!("cargo:rerun-if-changed={}", notice_path.display());
    if notice_required {
        let notice = fs::read_to_string(&notice_path).unwrap_or_else(|error| {
            panic!(
                "{} provenance requires adjacent `{}`: {error}",
                manifest.key,
                notice_path.display()
            )
        });
        assert!(
            !notice.trim().is_empty(),
            "{} provenance notice must be nonempty",
            manifest.key
        );
        assert!(
            notice
                .lines()
                .any(|line| line.contains("SPDX-License-Identifier:")),
            "{} notice must preserve SPDX metadata",
            manifest.key
        );
    }
}

fn validate_compiler_envelope(manifest: &SampleManifest, source: &str, compiled: &Value) {
    let normalized_source = required_string(compiled, "normalizedSource", &manifest.key);
    assert_eq!(
        normalized_source, source,
        "{} source and compiler-normalized source must match byte-for-byte",
        manifest.key
    );
    let source_digest = sha256_hex(source.as_bytes());
    for pointer in [
        "/inputSourceDigest",
        "/ir/source_digest",
        "/artifact/source_digest",
    ] {
        assert_eq!(
            compiled.pointer(pointer).and_then(Value::as_str),
            Some(source_digest.as_str()),
            "{} compiler envelope has stale source digest at `{pointer}`",
            manifest.key
        );
    }
    assert_eq!(
        compiled.pointer("/ir/format").and_then(Value::as_str),
        Some("geosolve-managed-sketch-ir-v3"),
        "{} must own V3 managed IR",
        manifest.key
    );
    assert_eq!(
        compiled.pointer("/artifact/format").and_then(Value::as_str),
        Some("geosolve-executed-sketch-artifact-v3"),
        "{} must own a V3 executed artifact",
        manifest.key
    );
    assert_eq!(
        compiled.pointer("/artifact/ir_digest"),
        compiled.pointer("/ir/ir_digest"),
        "{} executed artifact must authenticate its IR",
        manifest.key
    );
    validate_canonical_projection(compiled, "ir", "canonicalIrJson", &manifest.key);
    validate_canonical_projection(compiled, "artifact", "canonicalArtifactJson", &manifest.key);
    validate_functional_groups(manifest, compiled);
}

fn validate_canonical_projection(compiled: &Value, field: &str, canonical: &str, key: &str) {
    assert!(
        compiled.get(field).is_some(),
        "{key} compiler envelope lacks `{field}`"
    );
    let bytes = required_string(compiled, canonical, key);
    let _: Value = serde_json::from_str(bytes)
        .unwrap_or_else(|error| panic!("{key} `{canonical}` is invalid JSON: {error}"));
}

fn validate_functional_groups(manifest: &SampleManifest, compiled: &Value) {
    assert!(
        !manifest.groups.is_empty(),
        "{} must declare functional groups",
        manifest.key
    );
    let mut manifest_names = BTreeSet::new();
    for group in &manifest.groups {
        assert_meaningful(group, "functional-group name");
        assert!(
            manifest_names.insert(group.as_str()),
            "{} repeats functional group `{group}`",
            manifest.key
        );
    }

    let artifact = compiled
        .get("artifact")
        .expect("validated compiler envelope has artifact");
    let declarations = artifact
        .get("declarations")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} artifact declarations are missing", manifest.key));
    let declaration_names = declarations
        .iter()
        .map(|declaration| {
            declaration
                .get("declaration")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{} artifact declaration has no name", manifest.key))
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        declaration_names.len(),
        declarations.len(),
        "{} artifact declaration names must be unique",
        manifest.key
    );

    let artifact_groups = group_projection(
        artifact
            .get("groups")
            .and_then(Value::as_array)
            .unwrap_or_else(|| panic!("{} artifact groups are missing", manifest.key)),
        &manifest.key,
    );
    assert_eq!(
        artifact_groups
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        manifest
            .groups
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        "{} manifest groups must exactly match executed order",
        manifest.key
    );

    let mut ownership = BTreeMap::<&str, usize>::new();
    for (_, members) in &artifact_groups {
        assert!(
            !members.is_empty(),
            "{} functional groups must not be empty",
            manifest.key
        );
        for member in members {
            assert!(
                declaration_names.contains(member.as_str()),
                "{} group refers to unknown declaration `{member}`",
                manifest.key
            );
            *ownership.entry(member).or_default() += 1;
        }
    }
    for declaration in &declaration_names {
        assert_eq!(
            ownership.get(declaration).copied(),
            Some(1),
            "{} declaration `{declaration}` must belong to exactly one functional group",
            manifest.key
        );
    }

    let ir_statements = compiled
        .pointer("/ir/statements")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("{} IR statements are missing", manifest.key));
    let ir_groups = ir_statements
        .iter()
        .filter(|statement| statement.get("statement").and_then(Value::as_str) == Some("group"))
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        group_projection(&ir_groups, &manifest.key),
        artifact_groups,
        "{} IR and executed functional groups must agree",
        manifest.key
    );
}

fn group_projection(groups: &[Value], key: &str) -> Vec<(String, Vec<String>)> {
    groups
        .iter()
        .map(|group| {
            let name = group
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{key} functional group has no name"))
                .to_owned();
            let declarations = group
                .get("declarations")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("{key} functional group `{name}` has no declarations"))
                .iter()
                .map(|reference| {
                    assert!(
                        reference
                            .get("path")
                            .and_then(Value::as_array)
                            .is_some_and(Vec::is_empty),
                        "{key} functional group `{name}` must contain flat declaration roots"
                    );
                    reference
                        .get("declaration")
                        .and_then(Value::as_str)
                        .unwrap_or_else(|| {
                            panic!("{key} functional group `{name}` has an unnamed declaration")
                        })
                        .to_owned()
                })
                .collect();
            (name, declarations)
        })
        .collect()
}

fn compress_bundled_samples(samples: &[ValidatedSample], output_dir: &Path) {
    let destination = output_dir.join("bundled-samples");
    recreate_directory(&destination);
    for sample in samples {
        compress_envelope(
            &sample.directory.join("sketch.compiled.json"),
            &destination,
            Some(&sample.manifest.key),
        );
    }
}

fn generate_registry(samples: &[ValidatedSample], output_dir: &Path) {
    let mut generated = String::from("// SPDX-License-Identifier: GPL-3.0-or-later\n\n");
    for sample in samples {
        let ordinal = sample.manifest.ordinal;
        let key = &sample.manifest.key;
        writeln!(
            generated,
            "static BUNDLED_SAMPLE_ENVELOPE_{ordinal:02}: BundledCompilerEnvelope = \
             BundledCompilerEnvelope::new(include_bytes!(concat!(env!(\"OUT_DIR\"), \
             \"/bundled-compiler-envelopes/bundled-samples/{key}.compiled.json.zlib\")), \
             include!(concat!(env!(\"OUT_DIR\"), \
             \"/bundled-compiler-envelopes/bundled-samples/{key}.compiled.json.len.rs\")));"
        )
        .expect("writing generated Rust to a String cannot fail");
    }
    write!(
        generated,
        "\npub(super) static BUNDLED_SAMPLES: [BundledSampleSpec; {SAMPLE_COUNT}] = [\n"
    )
    .expect("writing generated Rust to a String cannot fail");
    for sample in samples {
        append_generated_sample(&mut generated, sample);
    }
    generated.push_str("];\n");

    let path = output_dir.join("bundled-sample-registry.rs");
    fs::write(&path, generated).unwrap_or_else(|error| {
        panic!(
            "write generated bundled-sample registry `{}`: {error}",
            path.display()
        )
    });
}

fn append_generated_sample(generated: &mut String, sample: &ValidatedSample) {
    let manifest = &sample.manifest;
    let groups = manifest
        .groups
        .iter()
        .map(|group| RustString(group).to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let provenance = manifest
        .provenance
        .iter()
        .map(|entry| {
            format!(
                "SampleProvenance::new({}, {}, {}, {}, {}, {}, {}, {})",
                entry.relationship.rust_variant(),
                RustString(&entry.name),
                RustOptionString(entry.url.as_deref()),
                RustOptionString(entry.revision.as_deref()),
                RustOptionString(entry.path.as_deref()),
                RustString(&entry.licence),
                RustString(&entry.scope),
                entry.notice_required,
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let notice = if sample.directory.join("NOTICE.md").is_file() {
        format!(
            "Some(include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \
             \"/assets/bundled-samples/{}/NOTICE.md\")))",
            manifest.key
        )
    } else {
        "None".to_owned()
    };
    write!(
        generated,
        "    BundledSampleSpec::new(\n        {ordinal}, {key}, {title}, {category}, {summary},\n        \
         SampleExpected::new({raw}, {effective}),\n        &[{groups}],\n        &[{provenance}],\n        \
         include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \
         \"/assets/bundled-samples/{raw_key}/manifest.json\")),\n        \
         include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \
         \"/assets/bundled-samples/{raw_key}/sketch.ts\")),\n        \
         &BUNDLED_SAMPLE_ENVELOPE_{ordinal:02},\n        include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \
         \"/assets/bundled-samples/{raw_key}/witnesses.json\")),\n        {notice},\n    ),\n",
        ordinal = manifest.ordinal,
        key = RustString(&manifest.key),
        title = RustString(&manifest.title),
        category = manifest.category.rust_variant(),
        summary = RustString(&manifest.summary),
        raw = manifest.expected.raw_dof,
        effective = manifest.expected.effective_dof,
        raw_key = manifest.key,
    )
    .expect("writing generated Rust to a String cannot fail");
}

fn generate_frontend_manifest(samples: &[ValidatedSample], output_dir: &Path) {
    let frontend = samples
        .iter()
        .map(|sample| FrontendSample {
            ordinal: sample.manifest.ordinal,
            stable_id: format!("sample.{}", sample.manifest.key),
            key: &sample.manifest.key,
            title: &sample.manifest.title,
            category: sample.manifest.category,
            group: sample.manifest.category.label(),
            summary: &sample.manifest.summary,
        })
        .collect::<Vec<_>>();
    let mut json = serde_json::to_string_pretty(&frontend)
        .expect("validated bundled-sample frontend projection serializes");
    json.push('\n');
    let path = output_dir.join("frontend-samples.json");
    fs::write(&path, json).unwrap_or_else(|error| {
        panic!(
            "write generated frontend sample manifest `{}`: {error}",
            path.display()
        )
    });
    println!(
        "cargo:rustc-env=GEOSOLVE_BUNDLED_SAMPLE_FRONTEND_MANIFEST={}",
        path.display()
    );
}

fn compress_envelope(source: &Path, destination_dir: &Path, output_key: Option<&str>) {
    let original = read_file(source, "bundled compiler envelope");
    let compressed = compress_to_vec_zlib(&original, COMPRESSION_LEVEL);
    let reconstructed = decompress_to_vec_zlib(&compressed).unwrap_or_else(|error| {
        panic!(
            "verify compressed bundled compiler envelope `{}`: {error:?}",
            source.display()
        )
    });
    assert_eq!(
        reconstructed, original,
        "compressed bundled compiler envelope must reconstruct byte-for-byte"
    );

    let default_stem = source
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".json"))
        .expect("bundled compiler envelope has a .json file name");
    let stem = output_key.map_or_else(|| default_stem.to_owned(), |key| format!("{key}.compiled"));
    let compressed_path = destination_dir.join(format!("{stem}.json.zlib"));
    fs::write(&compressed_path, compressed).unwrap_or_else(|error| {
        panic!(
            "write compressed bundled compiler envelope `{}`: {error}",
            compressed_path.display()
        )
    });

    let length_path = destination_dir.join(format!("{stem}.json.len.rs"));
    fs::write(
        &length_path,
        format!(
            "// SPDX-License-Identifier: GPL-3.0-or-later\n{}usize\n",
            separated_decimal(original.len())
        ),
    )
    .unwrap_or_else(|error| {
        panic!(
            "write bundled compiler-envelope length `{}`: {error}",
            length_path.display()
        )
    });
}

fn recreate_directory(path: &Path) {
    if let Err(error) = fs::remove_dir_all(path) {
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::NotFound,
            "clear generated directory `{}`: {error}",
            path.display()
        );
    }
    fs::create_dir_all(path)
        .unwrap_or_else(|error| panic!("create generated directory `{}`: {error}", path.display()));
}

fn directory_paths(path: &Path) -> Vec<PathBuf> {
    fs::read_dir(path)
        .unwrap_or_else(|error| panic!("read directory `{}`: {error}", path.display()))
        .map(|entry| entry.expect("read directory entry").path())
        .collect()
}

fn read_file(path: &Path, description: &str) -> Vec<u8> {
    fs::read(path)
        .unwrap_or_else(|error| panic!("read {description} `{}`: {error}", path.display()))
}

fn read_json<T>(path: &Path, description: &str) -> T
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_slice(&read_file(path, description))
        .unwrap_or_else(|error| panic!("parse {description} `{}`: {error}", path.display()))
}

fn required_string<'a>(value: &'a Value, field: &str, key: &str) -> &'a str {
    value
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{key} compiler envelope lacks string `{field}`"))
}

fn required_optional_field<'a>(
    manifest: &SampleManifest,
    field: &str,
    value: Option<&'a str>,
) -> &'a str {
    let value =
        value.unwrap_or_else(|| panic!("{} external provenance lacks `{field}`", manifest.key));
    assert_meaningful(value, field);
    value
}

fn assert_valid_key(key: &str) {
    assert!(
        !key.is_empty()
            && !key.starts_with('-')
            && !key.ends_with('-')
            && key
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'),
        "invalid bundled-sample key `{key}`"
    );
}

fn assert_meaningful(value: &str, description: &str) {
    assert!(
        !value.trim().is_empty() && value == value.trim(),
        "{description} must be nonempty and already trimmed"
    );
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(encoded, "{byte:02x}").expect("writing a digest to a String cannot fail");
    }
    encoded
}

fn separated_decimal(value: usize) -> String {
    let digits = value.to_string();
    let mut separated = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            separated.push('_');
        }
        separated.push(digit);
    }
    separated
}

struct RustString<'a>(&'a str);

impl fmt::Display for RustString<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}", self.0)
    }
}

struct RustOptionString<'a>(Option<&'a str>);

impl fmt::Display for RustOptionString<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(value) => write!(formatter, "Some({value:?})"),
            None => formatter.write_str("None"),
        }
    }
}
