// SPDX-License-Identifier: GPL-3.0-or-later

use std::{env, fs, path::PathBuf, process::ExitCode};

const GENERATED: &str = include_str!(env!("GEOSOLVE_BUNDLED_SAMPLE_FRONTEND_MANIFEST"));

fn main() -> ExitCode {
    let mut arguments = env::args_os().skip(1);
    let Some(mode) = arguments.next() else {
        eprintln!("usage: generate_frontend_samples (--check|--write) <samples.json>");
        return ExitCode::FAILURE;
    };
    let Some(path) = arguments.next().map(PathBuf::from) else {
        eprintln!("missing samples.json path");
        return ExitCode::FAILURE;
    };
    if arguments.next().is_some() {
        eprintln!("unexpected extra argument");
        return ExitCode::FAILURE;
    }

    if mode == "--write" {
        if let Err(error) = fs::write(&path, GENERATED) {
            eprintln!("write `{}`: {error}", path.display());
            return ExitCode::FAILURE;
        }
        return ExitCode::SUCCESS;
    }
    if mode == "--check" {
        match fs::read_to_string(&path) {
            Ok(actual) if actual == GENERATED => return ExitCode::SUCCESS,
            Ok(_) => eprintln!(
                "`{}` differs from the canonical bundled-sample registry",
                path.display()
            ),
            Err(error) => eprintln!("read `{}`: {error}", path.display()),
        }
        return ExitCode::FAILURE;
    }

    eprintln!(
        "unsupported mode `{}`; expected --check or --write",
        mode.to_string_lossy()
    );
    ExitCode::FAILURE
}
