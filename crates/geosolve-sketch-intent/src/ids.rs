// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Maximum byte length of a semantic schema, field, alias, or presentation key.
pub const MAX_INTENT_KEY_BYTES: usize = 256;

/// Failure to parse an opaque fixed-width intent identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum IntentIdParseError {
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
            type Err = IntentIdParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                if value.len() != $width
                    || !value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                {
                    return Err(IntentIdParseError::InvalidEncoding {
                        kind: $kind,
                        value: value.to_owned(),
                    });
                }
                <$inner>::from_str_radix(value, 16).map(Self).map_err(|_| {
                    IntentIdParseError::InvalidEncoding {
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
    IntentSessionId,
    u128,
    32,
    "intent session ID",
    "Stable identity and namespace of one design-intent session."
);
hex_id!(
    NodeId,
    u64,
    16,
    "intent node ID",
    "Stable never-reused identity of one intent declaration."
);
hex_id!(
    PortId,
    u64,
    16,
    "intent port ID",
    "Stable never-reused identity of one typed node output."
);
hex_id!(
    ReservationId,
    u64,
    16,
    "intent reservation ID",
    "Stable never-reused reservation for one native materialized identity."
);
hex_id!(
    ChildId,
    u64,
    16,
    "intent child ID",
    "Stable never-reused identity of one variable-cardinality child."
);
hex_id!(
    CellId,
    u64,
    16,
    "intent cell ID",
    "Stable never-reused presentation-cell identity."
);
hex_id!(
    Revision,
    u64,
    16,
    "intent revision",
    "Opaque monotonic exact-CAS revision."
);
hex_id!(
    ExternalInputRevision,
    u64,
    16,
    "external input revision",
    "Opaque host-owned external-input revision."
);

impl Default for Revision {
    fn default() -> Self {
        Self::from_raw(0)
    }
}

/// Canonical deterministic 256-bit content digest.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ContentDigest([u8; 32]);

impl ContentDigest {
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

    /// Zero is used only as a temporary wire placeholder before digesting.
    #[must_use]
    pub const fn zero() -> Self {
        Self([0; 32])
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl FromStr for ContentDigest {
    type Err = IntentIdParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(IntentIdParseError::InvalidEncoding {
                kind: "intent digest",
                value: value.to_owned(),
            });
        }
        let mut bytes = [0_u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| {
                IntentIdParseError::InvalidEncoding {
                    kind: "intent digest",
                    value: value.to_owned(),
                }
            })?;
        }
        Ok(Self(bytes))
    }
}

impl Serialize for ContentDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ContentDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Exact identity of one independently revisioned session component.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentIdentity {
    pub revision: Revision,
    pub digest: ContentDigest,
}

/// Opaque bounded token authenticating one exact planned candidate.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PlanToken(ContentDigest);

impl PlanToken {
    #[must_use]
    pub(crate) const fn new(digest: ContentDigest) -> Self {
        Self(digest)
    }

    #[must_use]
    pub const fn digest(self) -> ContentDigest {
        self.0
    }
}

/// Invalid semantic key.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[non_exhaustive]
pub enum IntentKeyError {
    #[error("intent key must not be empty")]
    Empty,
    #[error("intent key exceeds {limit} bytes")]
    TooLong { limit: usize },
    #[error("intent key must not have leading/trailing whitespace or control characters")]
    InvalidCharacters,
}

/// Validated schema, field, alias, label, or native identity key.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct IntentKey(String);

impl IntentKey {
    /// Validates and constructs a key.
    ///
    /// # Errors
    ///
    /// Returns a typed error for an empty, excessive, padded, or control-
    /// character-containing key.
    pub fn new(value: impl Into<String>) -> Result<Self, IntentKeyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(IntentKeyError::Empty);
        }
        if value.len() > MAX_INTENT_KEY_BYTES {
            return Err(IntentKeyError::TooLong {
                limit: MAX_INTENT_KEY_BYTES,
            });
        }
        if value.trim() != value || value.chars().any(char::is_control) {
            return Err(IntentKeyError::InvalidCharacters);
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for IntentKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for IntentKey {
    type Err = IntentKeyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

impl Serialize for IntentKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for IntentKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

pub(crate) fn digest_bytes(bytes: &[u8]) -> ContentDigest {
    // Four stable independent FNV-1a-derived lanes avoid platform hashing and
    // keep this crate dependency-light. This is an integrity fingerprint, not
    // a security primitive.
    const OFFSETS: [u64; 4] = [
        0xcbf2_9ce4_8422_2325,
        0x8422_2325_cbf2_9ce4,
        0x9e37_79b9_7f4a_7c15,
        0x6a09_e667_f3bc_c909,
    ];
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut digest = [0_u8; 32];
    for (lane_index, offset) in OFFSETS.into_iter().enumerate() {
        let mut lane = offset ^ u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        for byte in bytes {
            lane ^= u64::from(*byte).wrapping_add((lane_index as u64) << 8);
            lane = lane.wrapping_mul(PRIME);
            lane ^= lane.rotate_right(17);
        }
        digest[lane_index * 8..lane_index * 8 + 8].copy_from_slice(&lane.to_be_bytes());
    }
    ContentDigest::from_bytes(digest)
}
