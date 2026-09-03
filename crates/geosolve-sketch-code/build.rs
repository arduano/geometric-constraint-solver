// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    env, fs,
    path::{Path, PathBuf},
};

use miniz_oxide::{deflate::compress_to_vec_zlib, inflate::decompress_to_vec_zlib};

const COMPRESSION_LEVEL: u8 = 10;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo provides CARGO_MANIFEST_DIR"),
    );
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo provides OUT_DIR"))
        .join("bundled-compiler-envelopes");

    for category in ["samples", "demos"] {
        compress_category(&manifest_dir, &output_dir, category);
    }
}

fn compress_category(manifest_dir: &Path, output_dir: &Path, category: &str) {
    let source_dir = manifest_dir.join("assets").join(category);
    let destination_dir = output_dir.join(category);
    println!("cargo:rerun-if-changed={}", source_dir.display());
    if let Err(error) = fs::remove_dir_all(&destination_dir) {
        assert_eq!(
            error.kind(),
            std::io::ErrorKind::NotFound,
            "clear bundled compiler-envelope output directory `{}`: {error}",
            destination_dir.display()
        );
    }
    fs::create_dir_all(&destination_dir).unwrap_or_else(|error| {
        panic!(
            "create bundled compiler-envelope output directory `{}`: {error}",
            destination_dir.display()
        )
    });

    let mut sources = fs::read_dir(&source_dir)
        .unwrap_or_else(|error| {
            panic!(
                "read bundled compiler-envelope directory `{}`: {error}",
                source_dir.display()
            )
        })
        .map(|entry| {
            entry
                .expect("read bundled compiler-envelope directory entry")
                .path()
        })
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".compiled.json"))
        })
        .collect::<Vec<_>>();
    sources.sort();

    for source in sources {
        println!("cargo:rerun-if-changed={}", source.display());
        compress_envelope(&source, &destination_dir);
    }
}

fn compress_envelope(source: &Path, destination_dir: &Path) {
    let original = fs::read(source).unwrap_or_else(|error| {
        panic!(
            "read bundled compiler envelope `{}`: {error}",
            source.display()
        )
    });
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

    let file_name = source
        .file_name()
        .expect("bundled compiler envelope has a file name");
    let compressed_path = destination_dir.join(file_name).with_extension("json.zlib");
    fs::write(&compressed_path, compressed).unwrap_or_else(|error| {
        panic!(
            "write compressed bundled compiler envelope `{}`: {error}",
            compressed_path.display()
        )
    });

    let length_path = destination_dir
        .join(file_name)
        .with_extension("json.len.rs");
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
