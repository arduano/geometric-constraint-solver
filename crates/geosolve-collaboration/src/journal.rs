// SPDX-License-Identifier: GPL-3.0-or-later
//! Native reference durable append transport. The host must hold its existing
//! exclusive document lock. Corruption is reported without truncating user data.

use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use thiserror::Error;

use crate::protocol::JournalRecord;

const MAGIC: &[u8; 8] = b"GSCLJ01\n";

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("journal I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("journal format: {0}")]
    Format(String),
    #[error("journal needs recovery after an uncertain append")]
    NeedsRecovery,
}

/// Append handles never silently repair partial tails or discard outcomes. Open
/// returns the exact records for `DocumentAuthority::restore` chain validation.
#[derive(Debug)]
pub struct DurableJournal {
    file: File,
    max_record_bytes: usize,
    max_total_bytes: usize,
    total_bytes: usize,
    poisoned: bool,
}

impl DurableJournal {
    /// Creates a new journal without replacing existing data. Parent directory
    /// durability follows the same Linux file-workspace persistence contract.
    ///
    /// # Errors
    /// Rejects an existing path, unusable limits or any failed write/sync.
    pub fn create(
        path: &Path,
        max_record_bytes: usize,
        max_total_bytes: usize,
    ) -> Result<Self, JournalError> {
        validate_limits(max_record_bytes, max_total_bytes)?;
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create_new(true)
            .open(path)?;
        file.write_all(MAGIC)?;
        file.sync_all()?;
        File::open(
            path.parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new(".")),
        )?
        .sync_all()?;
        Ok(Self {
            file,
            max_record_bytes,
            max_total_bytes,
            total_bytes: MAGIC.len(),
            poisoned: false,
        })
    }

    /// Reads bounded framing without trusting checksums as geometry authority.
    /// Call `DocumentAuthority::restore` before appending or serving the document.
    ///
    /// # Errors
    /// Rejects partial tails, malformed framing/JSON, exceeded limits or I/O failure.
    /// No bytes are overwritten or truncated on any error.
    pub fn open(
        path: &Path,
        max_record_bytes: usize,
        max_total_bytes: usize,
    ) -> Result<(Self, Vec<JournalRecord>), JournalError> {
        validate_limits(max_record_bytes, max_total_bytes)?;
        let mut file = OpenOptions::new().read(true).append(true).open(path)?;
        let length = usize::try_from(file.metadata()?.len())
            .map_err(|_| JournalError::Format("file size".into()))?;
        if length > max_total_bytes {
            return Err(JournalError::Format("total byte limit".into()));
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(
                u64::try_from(max_total_bytes)
                    .unwrap_or(u64::MAX)
                    .saturating_add(1),
            )
            .read_to_end(&mut bytes)?;
        if bytes.len() > max_total_bytes || !bytes.starts_with(MAGIC) {
            return Err(JournalError::Format("header or total byte limit".into()));
        }
        let mut offset = MAGIC.len();
        let mut records = Vec::new();
        while offset < bytes.len() {
            let header = bytes
                .get(offset..offset + 4)
                .ok_or_else(|| JournalError::Format("partial record length".into()))?;
            let size = u32::from_le_bytes(
                header
                    .try_into()
                    .map_err(|_| JournalError::Format("partial record length".into()))?,
            ) as usize;
            if size == 0 || size > max_record_bytes {
                return Err(JournalError::Format("record byte limit".into()));
            }
            offset += 4;
            let end = offset
                .checked_add(size)
                .ok_or_else(|| JournalError::Format("record size overflow".into()))?;
            let record = bytes
                .get(offset..end)
                .ok_or_else(|| JournalError::Format("partial record body".into()))?;
            records.push(
                serde_json::from_slice(record)
                    .map_err(|error| JournalError::Format(error.to_string()))?,
            );
            offset = end;
        }
        Ok((
            Self {
                file,
                max_record_bytes,
                max_total_bytes,
                total_bytes: bytes.len(),
                poisoned: false,
            },
            records,
        ))
    }

    /// Appends and syncs before returning. After any I/O error, discard this handle
    /// and explicitly recover; success cannot be inferred from a later retry.
    ///
    /// # Errors
    /// Rejects oversized records and uncertain/failed durable publication.
    pub fn append(&mut self, record: &JournalRecord) -> Result<(), JournalError> {
        if self.poisoned {
            return Err(JournalError::NeedsRecovery);
        }
        let bytes =
            serde_json::to_vec(record).map_err(|error| JournalError::Format(error.to_string()))?;
        let size = u32::try_from(bytes.len())
            .map_err(|_| JournalError::Format("record byte limit".into()))?;
        let total = self
            .total_bytes
            .saturating_add(4)
            .saturating_add(bytes.len());
        if bytes.len() > self.max_record_bytes || total > self.max_total_bytes {
            return Err(JournalError::Format("journal byte limit".into()));
        }
        // Poison before the first byte can escape; clear only after durable success.
        self.poisoned = true;
        self.file.write_all(&size.to_le_bytes())?;
        self.file.write_all(&bytes)?;
        self.file.sync_all()?;
        self.total_bytes = total;
        self.poisoned = false;
        Ok(())
    }
}

fn validate_limits(record: usize, total: usize) -> Result<(), JournalError> {
    if record == 0 || total < MAGIC.len() + 4 || record > total || record > u32::MAX as usize {
        return Err(JournalError::Format("invalid journal limits".into()));
    }
    Ok(())
}
