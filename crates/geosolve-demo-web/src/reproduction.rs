// SPDX-License-Identifier: GPL-3.0-or-later

//! Versioned, bounded text transport for workbench reproduction checkpoints.
//!
//! This module transports opaque workspace JSON bytes. It does not validate or
//! publish sketch state; the workbench must still decode the ordinary workspace
//! envelope and construct a fully validated coordinator before replacing live
//! state.

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use miniz_oxide::deflate::compress_to_vec_zlib;
use miniz_oxide::inflate::stream::{InflateState, inflate};
use miniz_oxide::{DataFormat, MZFlush, MZStatus};

/// Stable prefix for the first reproduction-payload transport.
pub const REPRODUCTION_PAYLOAD_HEADER: &str = "GEOSOLVE_REPRO_V1";

const REPRODUCTION_PAYLOAD_CODEC: &str = "zlib-base64url";
const DEFAULT_COMPRESSION_LEVEL: u8 = 6;

/// Maximum accepted complete text payload, before trimming or parsing.
pub const MAX_REPRODUCTION_TEXT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum accepted compressed body after strict base64url decoding.
pub const MAX_REPRODUCTION_COMPRESSED_BYTES: usize = 12 * 1024 * 1024;
/// Maximum accepted decompressed workspace JSON.
pub const MAX_REPRODUCTION_WORKSPACE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug)]
struct CodecLimits {
    text: usize,
    compressed: usize,
    workspace: usize,
}

impl CodecLimits {
    const DEFAULT: Self = Self {
        text: MAX_REPRODUCTION_TEXT_BYTES,
        compressed: MAX_REPRODUCTION_COMPRESSED_BYTES,
        workspace: MAX_REPRODUCTION_WORKSPACE_BYTES,
    };
}

/// Typed failures returned before workspace JSON is entrusted to persistence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReproductionPayloadError {
    /// The raw text exceeds the transport's defensive bound.
    TextTooLarge { actual: usize, maximum: usize },
    /// The envelope does not contain exactly the five V1 fields.
    InvalidEnvelope,
    /// The transport prefix is unknown.
    UnsupportedVersion,
    /// The compression/text codec is unknown.
    UnsupportedCodec,
    /// The declared workspace length is not canonical unsigned decimal.
    InvalidWorkspaceLength,
    /// The declared or encoded workspace exceeds the defensive bound.
    WorkspaceTooLarge { actual: usize, maximum: usize },
    /// The checksum is not exactly sixteen lowercase hexadecimal digits.
    InvalidChecksum,
    /// The body is not strict unpadded URL-safe base64.
    InvalidBase64Url,
    /// The decoded compressed body exceeds the defensive bound.
    CompressedBodyTooLarge { actual: usize, maximum: usize },
    /// The zlib stream is corrupt, truncated, over-expanding or has trailing data.
    InvalidCompressedBody,
    /// The zlib output length disagrees with the declared workspace length.
    WorkspaceLengthMismatch { declared: usize, actual: usize },
    /// The independently recomputed corruption checksum disagrees.
    ChecksumMismatch,
    /// The decompressed workspace is not UTF-8 JSON text.
    InvalidUtf8,
}

impl fmt::Display for ReproductionPayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TextTooLarge { actual, maximum } => write!(
                formatter,
                "reproduction payload is {actual} bytes; the limit is {maximum} bytes"
            ),
            Self::InvalidEnvelope => formatter.write_str(
                "reproduction payload must contain exactly version, codec, length, checksum and body",
            ),
            Self::UnsupportedVersion => formatter.write_str("unsupported reproduction payload version"),
            Self::UnsupportedCodec => formatter.write_str("unsupported reproduction payload codec"),
            Self::InvalidWorkspaceLength => formatter.write_str(
                "reproduction payload workspace length must be canonical unsigned decimal",
            ),
            Self::WorkspaceTooLarge { actual, maximum } => write!(
                formatter,
                "reproduction workspace is {actual} bytes; the limit is {maximum} bytes"
            ),
            Self::InvalidChecksum => formatter.write_str(
                "reproduction payload checksum must be sixteen lowercase hexadecimal digits",
            ),
            Self::InvalidBase64Url => formatter.write_str(
                "reproduction payload body is not strict unpadded URL-safe base64",
            ),
            Self::CompressedBodyTooLarge { actual, maximum } => write!(
                formatter,
                "compressed reproduction body is {actual} bytes; the limit is {maximum} bytes"
            ),
            Self::InvalidCompressedBody => formatter.write_str(
                "reproduction payload zlib body is corrupt, truncated, oversized or has trailing data",
            ),
            Self::WorkspaceLengthMismatch { declared, actual } => write!(
                formatter,
                "reproduction payload declares {declared} workspace bytes but decoded {actual}"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("reproduction payload checksum mismatch")
            }
            Self::InvalidUtf8 => {
                formatter.write_str("reproduction workspace is not valid UTF-8")
            }
        }
    }
}

impl std::error::Error for ReproductionPayloadError {}

/// Compress complete workspace JSON into deterministic, single-line exchange text.
///
/// # Errors
///
/// Returns a bounded-size error when the workspace or its compressed transport
/// cannot fit within the public reproduction-payload limits.
pub fn encode_workspace(workspace_json: &str) -> Result<String, ReproductionPayloadError> {
    encode_workspace_bytes(workspace_json.as_bytes(), CodecLimits::DEFAULT)
}

/// Decode and integrity-check exchange text into opaque workspace JSON.
///
/// Callers must still pass the result through the ordinary workspace decoder and
/// accepted-state restoration path before publishing it.
///
/// # Errors
///
/// Returns a typed transport error when the envelope, encoding, compressed
/// stream, declared size, checksum or UTF-8 workspace bytes are invalid.
pub fn decode_workspace(payload: &str) -> Result<String, ReproductionPayloadError> {
    let bytes = decode_workspace_bytes(payload, CodecLimits::DEFAULT)?;
    String::from_utf8(bytes).map_err(|_| ReproductionPayloadError::InvalidUtf8)
}

fn encode_workspace_bytes(
    workspace: &[u8],
    limits: CodecLimits,
) -> Result<String, ReproductionPayloadError> {
    if workspace.len() > limits.workspace {
        return Err(ReproductionPayloadError::WorkspaceTooLarge {
            actual: workspace.len(),
            maximum: limits.workspace,
        });
    }
    let compressed = compress_to_vec_zlib(workspace, DEFAULT_COMPRESSION_LEVEL);
    if compressed.len() > limits.compressed {
        return Err(ReproductionPayloadError::CompressedBodyTooLarge {
            actual: compressed.len(),
            maximum: limits.compressed,
        });
    }
    let body = URL_SAFE_NO_PAD.encode(compressed);
    let payload = format!(
        "{REPRODUCTION_PAYLOAD_HEADER}:{REPRODUCTION_PAYLOAD_CODEC}:{}:{:016x}:{body}",
        workspace.len(),
        corruption_checksum(workspace),
    );
    if payload.len() > limits.text {
        return Err(ReproductionPayloadError::TextTooLarge {
            actual: payload.len(),
            maximum: limits.text,
        });
    }
    Ok(payload)
}

fn decode_workspace_bytes(
    payload: &str,
    limits: CodecLimits,
) -> Result<Vec<u8>, ReproductionPayloadError> {
    if payload.len() > limits.text {
        return Err(ReproductionPayloadError::TextTooLarge {
            actual: payload.len(),
            maximum: limits.text,
        });
    }
    let payload = payload.trim();
    let mut fields = payload.splitn(6, ':');
    let version = fields
        .next()
        .ok_or(ReproductionPayloadError::InvalidEnvelope)?;
    let codec = fields
        .next()
        .ok_or(ReproductionPayloadError::InvalidEnvelope)?;
    let workspace_len = fields
        .next()
        .ok_or(ReproductionPayloadError::InvalidEnvelope)?;
    let checksum = fields
        .next()
        .ok_or(ReproductionPayloadError::InvalidEnvelope)?;
    let body = fields
        .next()
        .ok_or(ReproductionPayloadError::InvalidEnvelope)?;
    if fields.next().is_some() || body.is_empty() {
        return Err(ReproductionPayloadError::InvalidEnvelope);
    }
    if version != REPRODUCTION_PAYLOAD_HEADER {
        return Err(ReproductionPayloadError::UnsupportedVersion);
    }
    if codec != REPRODUCTION_PAYLOAD_CODEC {
        return Err(ReproductionPayloadError::UnsupportedCodec);
    }
    let workspace_len = parse_workspace_length(workspace_len)?;
    if workspace_len > limits.workspace {
        return Err(ReproductionPayloadError::WorkspaceTooLarge {
            actual: workspace_len,
            maximum: limits.workspace,
        });
    }
    let expected_checksum = parse_checksum(checksum)?;
    let compressed = URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|_| ReproductionPayloadError::InvalidBase64Url)?;
    if URL_SAFE_NO_PAD.encode(&compressed) != body {
        return Err(ReproductionPayloadError::InvalidBase64Url);
    }
    if compressed.len() > limits.compressed {
        return Err(ReproductionPayloadError::CompressedBodyTooLarge {
            actual: compressed.len(),
            maximum: limits.compressed,
        });
    }
    let workspace = decompress_exact(&compressed, workspace_len)?;
    if corruption_checksum(&workspace) != expected_checksum {
        return Err(ReproductionPayloadError::ChecksumMismatch);
    }
    Ok(workspace)
}

fn parse_workspace_length(value: &str) -> Result<usize, ReproductionPayloadError> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ReproductionPayloadError::InvalidWorkspaceLength);
    }
    value
        .parse::<usize>()
        .map_err(|_| ReproductionPayloadError::InvalidWorkspaceLength)
}

fn parse_checksum(value: &str) -> Result<u64, ReproductionPayloadError> {
    if value.len() != 16
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ReproductionPayloadError::InvalidChecksum);
    }
    u64::from_str_radix(value, 16).map_err(|_| ReproductionPayloadError::InvalidChecksum)
}

fn decompress_exact(
    compressed: &[u8],
    expected_len: usize,
) -> Result<Vec<u8>, ReproductionPayloadError> {
    // One byte lets the streaming API validate a legitimate empty stream while
    // still detecting any unexpected output against the declared zero length.
    let mut workspace = vec![0; expected_len.max(1)];
    let mut state = InflateState::new_boxed(DataFormat::Zlib);
    let result = inflate(&mut state, compressed, &mut workspace, MZFlush::Finish);
    if result.status != Ok(MZStatus::StreamEnd) || result.bytes_consumed != compressed.len() {
        return Err(ReproductionPayloadError::InvalidCompressedBody);
    }
    if result.bytes_written != expected_len {
        return Err(ReproductionPayloadError::WorkspaceLengthMismatch {
            declared: expected_len,
            actual: result.bytes_written,
        });
    }
    workspace.truncate(expected_len);
    Ok(workspace)
}

fn corruption_checksum(input: &[u8]) -> u64 {
    input.iter().fold(0xcbf2_9ce4_8422_2325, |checksum, byte| {
        (checksum ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    use super::{
        CodecLimits, REPRODUCTION_PAYLOAD_HEADER, ReproductionPayloadError, corruption_checksum,
        decode_workspace, decode_workspace_bytes, encode_workspace, encode_workspace_bytes,
    };

    const SMALL_LIMITS: CodecLimits = CodecLimits {
        text: 512,
        compressed: 128,
        workspace: 256,
    };

    #[test]
    fn payload_is_deterministic_compact_and_round_trips_exact_workspace_bytes() {
        let workspace = format!(
            "{{\"version\":5,\"design\":{}}}",
            "\"repeated-workspace-content\"".repeat(2_000)
        );
        let first = encode_workspace(&workspace).expect("encode workspace");
        let second = encode_workspace(&workspace).expect("repeat encode");
        assert_eq!(first, second);
        assert!(first.starts_with("GEOSOLVE_REPRO_V1:zlib-base64url:"));
        assert!(first.len() < workspace.len());
        assert_eq!(
            decode_workspace(&first).expect("decode workspace"),
            workspace
        );

        let empty = encode_workspace("").expect("encode empty workspace bytes");
        assert_eq!(
            decode_workspace(&empty).expect("decode empty workspace"),
            ""
        );
    }

    #[test]
    fn envelope_fields_are_strict_and_canonical() {
        let valid = encode_workspace_bytes(b"{}", SMALL_LIMITS).expect("small payload");
        let fields = valid.split(':').collect::<Vec<_>>();
        assert_eq!(fields.len(), 5);
        assert_eq!(fields[0], REPRODUCTION_PAYLOAD_HEADER);

        for (input, expected) in [
            ("", ReproductionPayloadError::InvalidEnvelope),
            (
                "GEOSOLVE_REPRO_V2:zlib-base64url:2:08f44b07b5901a25:e30",
                ReproductionPayloadError::UnsupportedVersion,
            ),
            (
                "GEOSOLVE_REPRO_V1:unknown:2:08f44b07b5901a25:e30",
                ReproductionPayloadError::UnsupportedCodec,
            ),
            (
                "GEOSOLVE_REPRO_V1:zlib-base64url:02:08f44b07b5901a25:e30",
                ReproductionPayloadError::InvalidWorkspaceLength,
            ),
            (
                "GEOSOLVE_REPRO_V1:zlib-base64url:2:08F44B07B5901A25:e30",
                ReproductionPayloadError::InvalidChecksum,
            ),
            (
                "GEOSOLVE_REPRO_V1:zlib-base64url:2:08f44b07b5901a25:e30:extra",
                ReproductionPayloadError::InvalidEnvelope,
            ),
        ] {
            assert_eq!(
                decode_workspace_bytes(input, SMALL_LIMITS).unwrap_err(),
                expected
            );
        }
    }

    #[test]
    fn transport_rejects_corruption_truncation_padding_and_trailing_bytes() {
        let workspace = b"{\"version\":5,\"value\":\"repeat repeat repeat\"}";
        let valid = encode_workspace_bytes(workspace, SMALL_LIMITS).expect("small payload");
        let mut fields = valid.split(':').map(str::to_owned).collect::<Vec<_>>();

        fields[3] = "0000000000000000".into();
        assert_eq!(
            decode_workspace_bytes(&fields.join(":"), SMALL_LIMITS).unwrap_err(),
            ReproductionPayloadError::ChecksumMismatch
        );

        let mut fields = valid.split(':').map(str::to_owned).collect::<Vec<_>>();
        let mut compressed = URL_SAFE_NO_PAD.decode(&fields[4]).expect("base64 body");
        let checksum_byte = compressed.last_mut().expect("zlib checksum byte");
        *checksum_byte ^= 1;
        fields[4] = URL_SAFE_NO_PAD.encode(compressed);
        assert_eq!(
            decode_workspace_bytes(&fields.join(":"), SMALL_LIMITS).unwrap_err(),
            ReproductionPayloadError::InvalidCompressedBody
        );

        let mut fields = valid.split(':').map(str::to_owned).collect::<Vec<_>>();
        fields[4].push('=');
        assert_eq!(
            decode_workspace_bytes(&fields.join(":"), SMALL_LIMITS).unwrap_err(),
            ReproductionPayloadError::InvalidBase64Url
        );

        let mut fields = valid.split(':').map(str::to_owned).collect::<Vec<_>>();
        fields[4].pop();
        assert!(matches!(
            decode_workspace_bytes(&fields.join(":"), SMALL_LIMITS),
            Err(ReproductionPayloadError::InvalidBase64Url
                | ReproductionPayloadError::InvalidCompressedBody)
        ));

        let mut fields = valid.split(':').map(str::to_owned).collect::<Vec<_>>();
        let mut compressed = URL_SAFE_NO_PAD.decode(&fields[4]).expect("base64 body");
        compressed.push(0);
        fields[4] = URL_SAFE_NO_PAD.encode(compressed);
        assert_eq!(
            decode_workspace_bytes(&fields.join(":"), SMALL_LIMITS).unwrap_err(),
            ReproductionPayloadError::InvalidCompressedBody
        );
    }

    #[test]
    fn limits_reject_before_unbounded_decode_or_inflate() {
        let workspace = vec![b'a'; 200];
        let valid = encode_workspace_bytes(&workspace, SMALL_LIMITS).expect("small payload");

        let exact_text_limits = CodecLimits {
            text: valid.len(),
            ..SMALL_LIMITS
        };
        assert_eq!(
            encode_workspace_bytes(&workspace, exact_text_limits)
                .expect("encode at exact text limit"),
            valid
        );
        assert_eq!(
            decode_workspace_bytes(&valid, exact_text_limits).expect("decode at exact text limit"),
            workspace
        );

        let compressed_len = URL_SAFE_NO_PAD
            .decode(valid.rsplit(':').next().expect("body"))
            .expect("compressed body")
            .len();
        let exact_compressed_limits = CodecLimits {
            compressed: compressed_len,
            ..SMALL_LIMITS
        };
        assert_eq!(
            encode_workspace_bytes(&workspace, exact_compressed_limits)
                .expect("encode at exact compressed limit"),
            valid
        );
        assert_eq!(
            decode_workspace_bytes(&valid, exact_compressed_limits)
                .expect("decode at exact compressed limit"),
            workspace
        );

        let exact_workspace_limit = vec![b'a'; SMALL_LIMITS.workspace];
        let exact_workspace_payload =
            encode_workspace_bytes(&exact_workspace_limit, SMALL_LIMITS).expect("exact limit");
        assert_eq!(
            decode_workspace_bytes(&exact_workspace_payload, SMALL_LIMITS)
                .expect("decode exact workspace limit"),
            exact_workspace_limit
        );

        assert_eq!(
            encode_workspace_bytes(&vec![b'a'; 257], SMALL_LIMITS).unwrap_err(),
            ReproductionPayloadError::WorkspaceTooLarge {
                actual: 257,
                maximum: 256,
            }
        );

        let encode_compressed_limits = CodecLimits {
            compressed: 1,
            ..SMALL_LIMITS
        };
        assert!(matches!(
            encode_workspace_bytes(&workspace, encode_compressed_limits),
            Err(ReproductionPayloadError::CompressedBodyTooLarge { .. })
        ));

        let encode_text_limits = CodecLimits {
            text: valid.len() - 1,
            ..SMALL_LIMITS
        };
        assert!(matches!(
            encode_workspace_bytes(&workspace, encode_text_limits),
            Err(ReproductionPayloadError::TextTooLarge { .. })
        ));

        let text_limits = CodecLimits {
            text: valid.len() - 1,
            ..SMALL_LIMITS
        };
        assert!(matches!(
            decode_workspace_bytes(&valid, text_limits),
            Err(ReproductionPayloadError::TextTooLarge { .. })
        ));

        let declared_oversize = valid.replacen(":200:", ":257:", 1);
        assert_eq!(
            decode_workspace_bytes(&declared_oversize, SMALL_LIMITS).unwrap_err(),
            ReproductionPayloadError::WorkspaceTooLarge {
                actual: 257,
                maximum: 256,
            }
        );

        let compressed_limits = CodecLimits {
            compressed: compressed_len - 1,
            ..SMALL_LIMITS
        };
        assert!(matches!(
            decode_workspace_bytes(&valid, compressed_limits),
            Err(ReproductionPayloadError::CompressedBodyTooLarge { .. })
        ));

        let bomb = valid.replacen(":200:", ":16:", 1);
        assert_eq!(
            decode_workspace_bytes(&bomb, SMALL_LIMITS).unwrap_err(),
            ReproductionPayloadError::InvalidCompressedBody
        );
    }

    #[test]
    fn decoded_length_and_utf8_are_independently_checked() {
        let valid = encode_workspace_bytes(b"abc", SMALL_LIMITS).expect("small payload");
        let longer = valid.replacen(":3:", ":4:", 1);
        assert_eq!(
            decode_workspace_bytes(&longer, SMALL_LIMITS).unwrap_err(),
            ReproductionPayloadError::WorkspaceLengthMismatch {
                declared: 4,
                actual: 3,
            }
        );

        let invalid_utf8 = encode_workspace_bytes(&[0xff], SMALL_LIMITS).expect("binary payload");
        assert_eq!(
            decode_workspace(&invalid_utf8).unwrap_err(),
            ReproductionPayloadError::InvalidUtf8
        );
    }

    #[test]
    fn checksum_convention_is_frozen() {
        assert_eq!(corruption_checksum(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(corruption_checksum(b"GeoSolve"), 0xa0d1_7ee8_63e1_c99d);
    }
}
