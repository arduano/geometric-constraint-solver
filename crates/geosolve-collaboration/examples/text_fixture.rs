// SPDX-License-Identifier: GPL-3.0-or-later
//! Deterministic native/WASM parity fixture; no accepted model authority.
use std::io::Read;

use geosolve_collaboration::{CursorDeletionBias, SharedTextDocument, SharedTextLimits};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let document = if std::env::args().nth(1).as_deref() == Some("load") {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        let bytes: Vec<u8> = serde_json::from_str(&input)?;
        SharedTextDocument::load(&bytes, b"native-verifier", SharedTextLimits::default())?
    } else {
        let mut document = SharedTextDocument::new(b"native-fixture", SharedTextLimits::default())?;
        document.create_file("main.ts", "const fish = '🐟';\n// e\u{301}𐐀\n")?;
        document.create_file("patches/channel.ts", "export const = (\n")?;
        document.rename_file("main.ts", "src/design.ts")?;
        document
    };
    let cursor = document.cursor("src/design.ts", 16, CursorDeletionBias::After)?;
    println!(
        "{}",
        serde_json::json!({"snapshot":document.capture(),"checkpoint":document.save(),"cursor":cursor,"location":document.resolve_cursor_location(&cursor)?,"file_id":document.file_id("src/design.ts")?})
    );
    Ok(())
}
