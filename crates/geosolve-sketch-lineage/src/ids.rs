// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Maximum byte length of a durable developer key or semantic port key.
pub const MAX_LINEAGE_KEY_BYTES: usize = 256;
/// Maximum byte length of an opaque host/materialized identity.
pub const MAX_LINEAGE_OPAQUE_ID_BYTES: usize = 512;

/// Failure to parse an opaque fixed-width lineage identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum LineageIdParseError {
    #[error("invalid {kind} encoding `{value}`")]
    InvalidEncoding { kind: &'static str, value: String },
}

macro_rules! hex_id {
    ($name:ident, $inner:ty, $width:expr, $kind:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name($inner);

        impl $name {
            /// Constructs an identity from its lossless Rust representation.
            #[must_use]
            pub const fn from_raw(value: $inner) -> Self {
                Self(value)
            }

            /// Returns the lossless Rust representation.
            #[must_use]
            pub const fn raw(self) -> $inner {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, concat!("{:0", $width, "x}"), self.0)
            }
        }

        impl FromStr for $name {
            type Err = LineageIdParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value.len() != $width
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(LineageIdParseError::InvalidEncoding {
                        kind: $kind,
                        value: value.to_owned(),
                    });
                }
                <$inner>::from_str_radix(value, 16).map(Self).map_err(|_| {
                    LineageIdParseError::InvalidEncoding {
                        kind: $kind,
                        value: value.to_owned(),
                    }
                })
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                String::deserialize(deserializer)?
                    .parse()
                    .map_err(serde::de::Error::custom)
            }
        }
    };
}

hex_id!(
    LineageDocumentId,
    u128,
    32,
    "lineage document ID",
    "Stable identity and namespace of one lineage document."
);
hex_id!(
    LineageStepId,
    u64,
    16,
    "lineage step ID",
    "Stable never-reused identity of one lineage step."
);
hex_id!(
    LineageOutputId,
    u64,
    16,
    "lineage output ID",
    "Stable never-reused identity of one semantic output port."
);
hex_id!(
    LineageReservationId,
    u64,
    16,
    "lineage reservation ID",
    "Stable never-reused identity of one materialization reservation."
);
hex_id!(
    LineageRevision,
    u64,
    16,
    "lineage revision",
    "Opaque monotonic revision of lineage intent."
);
hex_id!(
    LineageAuxiliaryHighWater,
    u64,
    16,
    "lineage auxiliary high-water",
    "Opaque monotonic cursor owned by a lineage session rather than one history position."
);

impl Default for LineageRevision {
    fn default() -> Self {
        Self::from_raw(0)
    }
}

/// Canonical deterministic 256-bit content digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LineageDigest([u8; 32]);

impl LineageDigest {
    /// Constructs a digest from exact bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Returns the exact digest bytes.
    #[must_use]
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for LineageDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for LineageDigest {
    type Err = LineageIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(LineageIdParseError::InvalidEncoding {
                kind: "lineage digest",
                value: value.to_owned(),
            });
        }
        let mut bytes = [0_u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| {
                LineageIdParseError::InvalidEncoding {
                    kind: "lineage digest",
                    value: value.to_owned(),
                }
            })?;
        }
        Ok(Self(bytes))
    }
}

impl Serialize for LineageDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for LineageDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Exact identity used by every compare-and-swap lineage mutation.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LineageDocumentIdentity {
    pub document: LineageDocumentId,
    pub revision: LineageRevision,
    pub digest: LineageDigest,
}

/// Invalid developer, port, input, schema, or opaque host key.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum LineageKeyError {
    #[error("{kind} must not be empty")]
    Empty { kind: &'static str },
    #[error("{kind} exceeds {limit} bytes")]
    TooLong { kind: &'static str, limit: usize },
    #[error("{kind} must not have leading/trailing whitespace or control characters")]
    InvalidCharacters { kind: &'static str },
}

macro_rules! string_key {
    ($name:ident, $kind:literal, $limit:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Validates and constructs the key.
            ///
            /// # Errors
            ///
            /// Returns a typed error for an empty, oversized, whitespace-padded,
            /// or control-character-containing key.
            pub fn new(value: impl Into<String>) -> Result<Self, LineageKeyError> {
                let value = value.into();
                validate_key(&value, $kind, $limit)?;
                Ok(Self(value))
            }

            /// Borrows the exact persisted key.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consumes the wrapper and returns the exact persisted key.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = LineageKeyError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

string_key!(
    LineageDeveloperKey,
    "lineage developer key",
    MAX_LINEAGE_KEY_BYTES,
    "Unique durable host/developer key of a lineage step."
);
string_key!(
    LineageSemanticKey,
    "lineage semantic key",
    MAX_LINEAGE_KEY_BYTES,
    "Stable semantic name of an input, output, reservation, or action schema."
);
string_key!(
    LineageOpaqueId,
    "opaque materialized identity",
    MAX_LINEAGE_OPAQUE_ID_BYTES,
    "Lossless string identity owned by an external materialization domain."
);

fn validate_key(value: &str, kind: &'static str, limit: usize) -> Result<(), LineageKeyError> {
    if value.is_empty() {
        return Err(LineageKeyError::Empty { kind });
    }
    if value.len() > limit {
        return Err(LineageKeyError::TooLong { kind, limit });
    }
    if value.trim() != value || value.chars().any(char::is_control) {
        return Err(LineageKeyError::InvalidCharacters { kind });
    }
    Ok(())
}
