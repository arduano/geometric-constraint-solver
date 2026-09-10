// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(not(target_arch = "wasm32"))]

use geosolve_collaboration::{
    authority::DocumentAuthority,
    journal::DurableJournal,
    protocol::{Command, CommandKind, Limits, Principal, Role, Submit},
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
#[derive(Debug)]
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "geosolve-collab-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn durable_admission_reopens_without_overwriting_and_partial_tail_is_preserved() {
    let scratch = Scratch::new();
    let path = scratch.0.join("operations.journal");
    let mut journal = DurableJournal::create(&path, 8192, 65536).unwrap();
    assert!(DurableJournal::create(&path, 8192, 65536).is_err());
    let mut state = DocumentAuthority::new(
        "document".into(),
        "epoch".into(),
        "server".into(),
        "initial".into(),
        Limits::default(),
    )
    .unwrap();
    let connection = state
        .connect(
            Principal {
                user_id: "alice".into(),
                role: Role::Editor,
            },
            "tab".into(),
            "session".into(),
        )
        .unwrap();
    state
        .admit(
            &Submit {
                connection,
                request_id: "one".into(),
                command: Command {
                    kind: CommandKind::Apply,
                    basis_revision: 0,
                    payload: serde_json::json!({ "capturedRevision": "draft-one" }),
                },
            },
            |record| journal.append(record).map_err(|error| error.to_string()),
        )
        .unwrap();
    drop(journal);
    let (reopened, records) = DurableJournal::open(&path, 8192, 65536).unwrap();
    let restored = DocumentAuthority::restore(
        "document".into(),
        "epoch".into(),
        "new-server".into(),
        "initial".into(),
        Limits::default(),
        &records,
    )
    .unwrap();
    assert_eq!(restored.pending_count(), 1);
    drop(reopened);
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&[5, 0])
        .unwrap();
    let corrupt = fs::read(&path).unwrap();
    assert!(DurableJournal::open(&path, 8192, 65536).is_err());
    assert_eq!(fs::read(&path).unwrap(), corrupt);
}

#[test]
fn oversized_frame_rejects_before_allocating_its_declared_size() {
    let scratch = Scratch::new();
    let path = scratch.0.join("operations.journal");
    drop(DurableJournal::create(&path, 8192, 65536).unwrap());
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&u32::MAX.to_le_bytes())
        .unwrap();
    assert!(DurableJournal::open(&path, 8192, 65536).is_err());
}
