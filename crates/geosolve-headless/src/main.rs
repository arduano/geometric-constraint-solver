// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]

use std::env;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use geosolve_headless::{
    HEADLESS_EDIT_BATCH_LIMIT, HeadlessInput, bundled_demo_keys, edit, inspect, publish_render,
    render,
};
use geosolve_sketch_code::{
    CODE_PROJECT_LIMIT, MANAGED_SOURCE_LIMIT, ManagedControlEditBatch, ProjectKey,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("geosolve-headless: {error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut arguments = env::args().skip(1);
    let command = arguments.next().ok_or_else(usage)?;
    if command == "demos" {
        if arguments.next().is_some() {
            return Err(usage());
        }
        println!("{}", bundled_demo_keys().join("\n"));
        return Ok(());
    }
    let mut demo = None;
    let mut managed = None;
    let mut project_json = None;
    let mut project_key = None;
    let mut edit_request = None;
    let mut output = None;
    while let Some(argument) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value after `{argument}`\n{}", usage()))?;
        match argument.as_str() {
            "--demo" => demo = Some(value),
            "--managed" => managed = Some(PathBuf::from(value)),
            "--project" => project_json = Some(PathBuf::from(value)),
            "--project-key" => project_key = Some(ProjectKey(value)),
            "--edit" => edit_request = Some(PathBuf::from(value)),
            "--out" => output = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option `{argument}`\n{}", usage())),
        }
    }
    let input_count = usize::from(demo.is_some())
        + usize::from(managed.is_some())
        + usize::from(project_json.is_some());
    if input_count != 1 {
        return Err(format!("select exactly one input\n{}", usage()));
    }
    if managed.is_some() && project_key.is_none() {
        return Err("managed input requires --project-key".into());
    }
    if managed.is_none() && project_key.is_some() {
        return Err("--project-key is only valid with --managed".into());
    }
    let input = if let Some(key) = demo {
        HeadlessInput::BundledDemo(key)
    } else if let Some(path) = managed {
        HeadlessInput::ManagedSource {
            project: project_key.expect("managed key checked"),
            source: read_bounded_utf8(&path, MANAGED_SOURCE_LIMIT, "managed source")?,
        }
    } else {
        let path = project_json.expect("one input checked");
        HeadlessInput::CodeProjectJson(read_bounded_utf8(
            &path,
            CODE_PROJECT_LIMIT,
            "project JSON",
        )?)
    };
    match command.as_str() {
        "inspect" => {
            if edit_request.is_some() || output.is_some() {
                return Err(
                    "inspect writes canonical JSON to stdout and accepts no --edit/--out".into(),
                );
            }
            let result = inspect(&input).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?
            );
        }
        "render" => {
            if edit_request.is_some() {
                return Err("render does not accept --edit".into());
            }
            let output = output.ok_or_else(|| "render requires --out".to_owned())?;
            let result = render(&input).map_err(|error| error.to_string())?;
            publish_render(&result, &output).map_err(|error| error.to_string())?;
            println!("{}", output.display());
        }
        "edit" => {
            let request = edit_request.ok_or_else(|| "edit requires --edit".to_owned())?;
            let output = output.ok_or_else(|| "edit requires --out".to_owned())?;
            let json = read_bounded_utf8(&request, HEADLESS_EDIT_BATCH_LIMIT, "edit batch")?;
            let batch: ManagedControlEditBatch =
                serde_json::from_str(&json).map_err(|error| error.to_string())?;
            let result = edit(&input, &batch).map_err(|error| error.to_string())?;
            publish_render(&result, &output).map_err(|error| error.to_string())?;
            println!("{}", output.display());
        }
        _ => return Err(format!("unknown command `{command}`\n{}", usage())),
    }
    Ok(())
}

fn read_bounded_utf8(path: &Path, limit: usize, label: &str) -> Result<String, String> {
    let mut file = File::open(path)
        .map_err(|error| format!("failed to open {label} `{}`: {error}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("failed to inspect {label} `{}`: {error}", path.display()))?;
    if metadata.is_file() && metadata.len() > limit as u64 {
        return Err(format!("{label} exceeds the {limit}-byte limit"));
    }

    let mut bytes = Vec::new();
    file.by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("failed to read {label} `{}`: {error}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!("{label} exceeds the {limit}-byte limit"));
    }
    String::from_utf8(bytes).map_err(|error| format!("{label} is not valid UTF-8: {error}"))
}

fn usage() -> String {
    concat!(
        "usage:\n",
        "  geosolve-headless demos\n",
        "  geosolve-headless inspect (--demo KEY | --managed FILE --project-key KEY | --project FILE)\n",
        "  geosolve-headless render (--demo KEY | --managed FILE --project-key KEY | --project FILE) --out NEW_DIR\n",
        "  geosolve-headless edit (--demo KEY | --managed FILE --project-key KEY | --project FILE) --edit BATCH.json --out NEW_DIR"
    )
    .into()
}
