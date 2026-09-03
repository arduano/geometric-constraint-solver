// SPDX-License-Identifier: GPL-3.0-or-later
#![forbid(unsafe_code)]

use std::env;
use std::fs::File;
use std::io::Read as _;
use std::path::{Path, PathBuf};

use geosolve_headless::{
    HEADLESS_EDIT_BATCH_LIMIT, HEADLESS_PREPARED_EDIT_LIMIT, HeadlessInput, HeadlessPreparedEdit,
    bundled_demo_keys, inspect, prepare_edit, publish_render, render, resolve_edit,
};
use geosolve_sketch_code::{
    CODE_PROJECT_LIMIT, ManagedControlEditBatch, PreparedManagedMutationReceipt,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("geosolve-headless: {error}");
        std::process::exit(2);
    }
}

#[derive(Debug, Default)]
struct CommandInputs {
    edit: Option<PathBuf>,
    prepared: Option<PathBuf>,
    receipt: Option<PathBuf>,
    output: Option<PathBuf>,
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
    let mut project_json = None;
    let mut inputs = CommandInputs::default();
    while let Some(argument) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value after `{argument}`\n{}", usage()))?;
        match argument.as_str() {
            "--demo" => demo = Some(value),
            "--project" => project_json = Some(PathBuf::from(value)),
            "--edit" => inputs.edit = Some(PathBuf::from(value)),
            "--prepared" => inputs.prepared = Some(PathBuf::from(value)),
            "--receipt" => inputs.receipt = Some(PathBuf::from(value)),
            "--out" => inputs.output = Some(PathBuf::from(value)),
            _ => return Err(format!("unknown option `{argument}`\n{}", usage())),
        }
    }
    let input_count = usize::from(demo.is_some()) + usize::from(project_json.is_some());
    if input_count != 1 {
        return Err(format!("select exactly one input\n{}", usage()));
    }
    let input = if let Some(key) = demo {
        HeadlessInput::BundledDemo(key)
    } else {
        let path = project_json.expect("one input checked");
        HeadlessInput::CodeProjectJson(read_bounded_utf8(
            &path,
            CODE_PROJECT_LIMIT,
            "project JSON",
        )?)
    };
    dispatch(&command, &input, inputs)
}

fn dispatch(command: &str, input: &HeadlessInput, inputs: CommandInputs) -> Result<(), String> {
    match command {
        "inspect" => {
            if inputs.edit.is_some()
                || inputs.prepared.is_some()
                || inputs.receipt.is_some()
                || inputs.output.is_some()
            {
                return Err(
                    "inspect writes canonical JSON to stdout and accepts no --edit/--out".into(),
                );
            }
            let result = inspect(input).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&result).map_err(|error| error.to_string())?
            );
        }
        "render" => {
            if inputs.edit.is_some() || inputs.prepared.is_some() || inputs.receipt.is_some() {
                return Err("render does not accept mutation inputs".into());
            }
            let output = inputs
                .output
                .ok_or_else(|| "render requires --out".to_owned())?;
            let result = render(input).map_err(|error| error.to_string())?;
            publish_render(&result, &output).map_err(|error| error.to_string())?;
            println!("{}", output.display());
        }
        "prepare-edit" => {
            if inputs.prepared.is_some() || inputs.receipt.is_some() || inputs.output.is_some() {
                return Err(
                    "prepare-edit writes its compiler request to stdout and accepts no --prepared/--receipt/--out"
                        .into(),
                );
            }
            let request = inputs
                .edit
                .ok_or_else(|| "prepare-edit requires --edit".to_owned())?;
            let json = read_bounded_utf8(&request, HEADLESS_EDIT_BATCH_LIMIT, "edit batch")?;
            let batch: ManagedControlEditBatch =
                serde_json::from_str(&json).map_err(|error| error.to_string())?;
            let prepared = prepare_edit(input, &batch).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string(&prepared).map_err(|error| error.to_string())?
            );
        }
        "resolve-edit" => {
            if inputs.edit.is_some() {
                return Err("resolve-edit does not accept --edit".into());
            }
            let prepared = inputs
                .prepared
                .ok_or_else(|| "resolve-edit requires --prepared".to_owned())?;
            let receipt = inputs
                .receipt
                .ok_or_else(|| "resolve-edit requires --receipt".to_owned())?;
            let output = inputs
                .output
                .ok_or_else(|| "resolve-edit requires --out".to_owned())?;
            let prepared: HeadlessPreparedEdit = serde_json::from_str(&read_bounded_utf8(
                &prepared,
                HEADLESS_PREPARED_EDIT_LIMIT,
                "prepared edit",
            )?)
            .map_err(|error| error.to_string())?;
            let receipt: PreparedManagedMutationReceipt = serde_json::from_str(&read_bounded_utf8(
                &receipt,
                HEADLESS_PREPARED_EDIT_LIMIT,
                "mutation receipt",
            )?)
            .map_err(|error| error.to_string())?;
            let result =
                resolve_edit(input, &prepared, receipt).map_err(|error| error.to_string())?;
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
        "  geosolve-headless inspect (--demo KEY | --project FILE)\n",
        "  geosolve-headless render (--demo KEY | --project FILE) --out NEW_DIR\n",
        "  geosolve-headless prepare-edit (--demo KEY | --project FILE) --edit BATCH.json\n",
        "  geosolve-headless resolve-edit (--demo KEY | --project FILE) --prepared PREPARED.json --receipt RECEIPT.json --out NEW_DIR"
    )
    .into()
}
