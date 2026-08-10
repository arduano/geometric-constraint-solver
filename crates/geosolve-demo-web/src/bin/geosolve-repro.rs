// SPDX-License-Identifier: GPL-3.0-or-later

//! Decode a workbench reproduction payload from standard input.

use std::io::{self, Read as _, Write as _};
use std::process::ExitCode;

fn main() -> ExitCode {
    match decode_from_standard_io() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr().lock(), "geosolve-repro: {error}");
            ExitCode::FAILURE
        }
    }
}

fn decode_from_standard_io() -> Result<(), Box<dyn std::error::Error>> {
    decode_from_io(
        io::stdin().lock(),
        io::stdout().lock(),
        geosolve_demo_web::reproduction::MAX_REPRODUCTION_TEXT_BYTES,
    )
}

fn decode_from_io(
    reader: impl io::Read,
    mut writer: impl io::Write,
    maximum_text_bytes: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut payload = String::new();
    reader
        .take(maximum_text_bytes.saturating_add(1) as u64)
        .read_to_string(&mut payload)?;
    if payload.len() > maximum_text_bytes {
        return Err(Box::new(
            geosolve_demo_web::reproduction::ReproductionPayloadError::TextTooLarge {
                actual: payload.len(),
                maximum: maximum_text_bytes,
            },
        ));
    }
    let workspace = geosolve_demo_web::reproduction::decode_workspace(&payload)?;
    writer.write_all(workspace.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::decode_from_io;

    #[test]
    fn diagnostic_decoder_bounds_input_and_writes_only_valid_workspace_bytes() {
        let payload =
            geosolve_demo_web::reproduction::encode_workspace("{\"version\":5}").expect("payload");
        let mut output = Vec::new();
        decode_from_io(Cursor::new(payload), &mut output, 1_024).expect("decode payload");
        assert_eq!(output, b"{\"version\":5}");

        let mut rejected_output = Vec::new();
        let error = decode_from_io(Cursor::new("123456789"), &mut rejected_output, 8)
            .expect_err("bounded input");
        assert!(error.to_string().contains("9 bytes; the limit is 8 bytes"));
        assert!(rejected_output.is_empty());
    }
}
