// SPDX-License-Identifier: GPL-3.0-or-later

//! Hermetic native SVG rasterization.

use std::{
    error::Error,
    fmt,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use base64::prelude::*;
use resvg::{tiny_skia, usvg};

use crate::{PNG_EXPORT_HEIGHT, PNG_EXPORT_WIDTH, standalone_static_export_svg_with_size};

const BUNDLED_FONT_FAMILY: &str = "Share Tech Mono";
const BUNDLED_FONT_BASE64: &str = include_str!("../assets/ShareTechMono-Regular.ttf.base64");

/// Largest accepted width or height for one native raster.
pub const MAX_NATIVE_RASTER_DIMENSION: u32 = 16_384;
/// Largest accepted pixel count for one native raster.
pub const MAX_NATIVE_RASTER_PIXELS: u64 = 67_108_864;
/// Largest accepted self-contained SVG input.
pub const MAX_NATIVE_SVG_BYTES: usize = 32 * 1024 * 1024;

/// Typed failure from hermetic native PNG rasterization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativePngError {
    /// One or both requested dimensions are zero or exceed the dimension bound.
    InvalidDimensions { width: u32, height: u32 },
    /// The requested output exceeds the total pixel bound.
    RasterPixelLimit { pixels: u64, maximum: u64 },
    /// The SVG input exceeds the parser resource bound.
    SvgByteLimit { bytes: usize, maximum: usize },
    /// The pinned bundled font could not be decoded or parsed.
    BundledFontUnavailable,
    /// An image reference was present; native rendering accepts none.
    ImageResourceDenied,
    /// A non-image external, embedded, or system resource route was present.
    ExternalResourceDenied,
    /// The self-contained SVG was not valid.
    InvalidSvg(String),
    /// The SVG declares dimensions other than the requested raster dimensions.
    SvgDimensionMismatch {
        requested: [u32; 2],
        declared: [u32; 2],
    },
    /// The bounded pixel buffer still could not be allocated.
    RasterAllocationFailed { width: u32, height: u32 },
    /// The rendered pixels could not be encoded as PNG.
    PngEncoding(String),
}

impl fmt::Display for NativePngError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => write!(
                formatter,
                "invalid native raster dimensions {width} x {height}; each must be within 1..={MAX_NATIVE_RASTER_DIMENSION}"
            ),
            Self::RasterPixelLimit { pixels, maximum } => write!(
                formatter,
                "native raster has {pixels} pixels, exceeding the {maximum}-pixel limit"
            ),
            Self::SvgByteLimit { bytes, maximum } => write!(
                formatter,
                "native SVG has {bytes} bytes, exceeding the {maximum}-byte limit"
            ),
            Self::BundledFontUnavailable => {
                formatter.write_str("pinned Share Tech Mono font is unavailable")
            }
            Self::ImageResourceDenied => formatter.write_str(
                "native SVG image resources are denied; only composed vector geometry and the bundled font are allowed",
            ),
            Self::ExternalResourceDenied => formatter.write_str(
                "native SVG external, embedded, and system resource routes are denied",
            ),
            Self::InvalidSvg(message) => write!(formatter, "invalid native SVG: {message}"),
            Self::SvgDimensionMismatch {
                requested,
                declared,
            } => write!(
                formatter,
                "native SVG declares {} x {}, not the requested {} x {}",
                declared[0], declared[1], requested[0], requested[1]
            ),
            Self::RasterAllocationFailed { width, height } => {
                write!(formatter, "could not allocate {width} x {height} native raster")
            }
            Self::PngEncoding(message) => write!(formatter, "PNG encoding failed: {message}"),
        }
    }
}

impl Error for NativePngError {}

/// Wraps canonical scene markup and rasterizes it with hermetic native inputs.
///
/// This is the direct native equivalent of the browser's Canvas export path.
/// It loads no system font, path, URL, data image, or other external resource.
/// The caller receives bytes and retains sole ownership of file publication.
///
/// # Errors
///
/// Returns a typed resource-bound, font, SVG, allocation, or encoding failure.
pub fn render_scene_png(
    scene_markup: &str,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, NativePngError> {
    validate_dimensions(width, height)?;
    let svg = standalone_static_export_svg_with_size(scene_markup, width, height)
        .ok_or(NativePngError::InvalidDimensions { width, height })?;
    render_svg_png(&svg, width, height)
}

/// Rasterizes canonical scene markup at the default 2000 × 1400 export size.
///
/// # Errors
///
/// Returns a typed font, SVG, allocation, or encoding failure.
pub fn render_default_scene_png(scene_markup: &str) -> Result<Vec<u8>, NativePngError> {
    render_scene_png(scene_markup, PNG_EXPORT_WIDTH, PNG_EXPORT_HEIGHT)
}

/// Rasterizes one already self-contained SVG with explicit output dimensions.
///
/// The SVG must declare the same dimensions. Images, external references,
/// resource-bearing CSS, DTDs, entities, and non-XML processing instructions
/// are rejected, and text resolves only through the bundled OFL font.
///
/// # Errors
///
/// Returns a typed resource-bound, font, SVG, dimension, allocation, or
/// encoding failure.
pub fn render_svg_png(svg: &str, width: u32, height: u32) -> Result<Vec<u8>, NativePngError> {
    validate_dimensions(width, height)?;
    if svg.len() > MAX_NATIVE_SVG_BYTES {
        return Err(NativePngError::SvgByteLimit {
            bytes: svg.len(),
            maximum: MAX_NATIVE_SVG_BYTES,
        });
    }
    validate_svg_resource_policy(svg)?;

    let encoded_font = BUNDLED_FONT_BASE64
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();
    let font = BASE64_STANDARD
        .decode(encoded_font)
        .map_err(|_| NativePngError::BundledFontUnavailable)?;

    let image_resource_requested = Arc::new(AtomicBool::new(false));
    let data_flag = Arc::clone(&image_resource_requested);
    let string_flag = Arc::clone(&image_resource_requested);
    let mut options = usvg::Options {
        font_family: BUNDLED_FONT_FAMILY.to_owned(),
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(move |_, _, _| {
                data_flag.store(true, Ordering::Relaxed);
                None
            }),
            resolve_string: Box::new(move |_, _| {
                string_flag.store(true, Ordering::Relaxed);
                None
            }),
        },
        ..usvg::Options::default()
    };
    let fontdb = options.fontdb_mut();
    fontdb.load_font_data(font);
    if fontdb.faces().next().is_none() {
        return Err(NativePngError::BundledFontUnavailable);
    }
    fontdb.set_serif_family(BUNDLED_FONT_FAMILY);
    fontdb.set_sans_serif_family(BUNDLED_FONT_FAMILY);
    fontdb.set_cursive_family(BUNDLED_FONT_FAMILY);
    fontdb.set_fantasy_family(BUNDLED_FONT_FAMILY);
    fontdb.set_monospace_family(BUNDLED_FONT_FAMILY);

    let tree = usvg::Tree::from_str(svg, &options)
        .map_err(|error| NativePngError::InvalidSvg(error.to_string()))?;
    if image_resource_requested.load(Ordering::Relaxed) {
        return Err(NativePngError::ImageResourceDenied);
    }

    let declared = tree.size().to_int_size();
    if declared.width() != width || declared.height() != height {
        return Err(NativePngError::SvgDimensionMismatch {
            requested: [width, height],
            declared: [declared.width(), declared.height()],
        });
    }
    let mut pixmap = tiny_skia::Pixmap::new(width, height)
        .ok_or(NativePngError::RasterAllocationFailed { width, height })?;
    resvg::render(
        &tree,
        tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    pixmap
        .encode_png()
        .map_err(|error| NativePngError::PngEncoding(error.to_string()))
}

fn validate_svg_resource_policy(svg: &str) -> Result<(), NativePngError> {
    let lower = svg.to_ascii_lowercase();
    if lower.contains("<!doctype")
        || lower.contains("<!entity")
        || lower.contains("<?xml-stylesheet")
    {
        return Err(NativePngError::ExternalResourceDenied);
    }

    let mut cursor = 0;
    while let Some(relative_start) = lower[cursor..].find('<') {
        let start = cursor + relative_start;
        if lower[start..].starts_with("<!--") {
            let Some(relative_end) = lower[start + 4..].find("-->") else {
                return Err(NativePngError::InvalidSvg(
                    "unterminated XML comment".into(),
                ));
            };
            cursor = start + 4 + relative_end + 3;
            continue;
        }
        if lower[start..].starts_with("<![cdata[") {
            let Some(relative_end) = lower[start + 9..].find("]]>") else {
                return Err(NativePngError::InvalidSvg(
                    "unterminated CDATA section".into(),
                ));
            };
            cursor = start + 9 + relative_end + 3;
            continue;
        }
        let end = xml_tag_end(svg, start + 1).ok_or_else(|| {
            NativePngError::InvalidSvg("unterminated XML element or declaration".into())
        })?;
        let tag = &svg[start + 1..end];
        let trimmed = tag.trim_start();
        if trimmed.starts_with('?') {
            let declaration = trimmed
                .strip_prefix('?')
                .unwrap_or(trimmed)
                .trim_start()
                .to_ascii_lowercase();
            if !declaration.starts_with("xml ") && declaration != "xml?" {
                return Err(NativePngError::ExternalResourceDenied);
            }
            cursor = end + 1;
            continue;
        }
        if trimmed.starts_with('!') {
            return Err(NativePngError::ExternalResourceDenied);
        }
        if trimmed.starts_with('/') {
            cursor = end + 1;
            continue;
        }

        let (qualified_name, attributes) = split_xml_name(trimmed);
        let local_name = qualified_name
            .rsplit_once(':')
            .map_or(qualified_name, |(_, local)| local)
            .to_ascii_lowercase();
        if matches!(
            local_name.as_str(),
            "image"
                | "feimage"
                | "script"
                | "foreignobject"
                | "iframe"
                | "object"
                | "embed"
                | "audio"
                | "video"
                | "source"
                | "link"
                | "font"
                | "font-face"
                | "font-face-src"
                | "font-face-uri"
                | "cursor"
                | "include"
        ) {
            return Err(if local_name == "image" || local_name == "feimage" {
                NativePngError::ImageResourceDenied
            } else {
                NativePngError::ExternalResourceDenied
            });
        }
        validate_svg_attributes(attributes)?;

        if local_name == "style" {
            let content_start = end + 1;
            let Some(relative_close) = lower[content_start..].find("</style") else {
                return Err(NativePngError::InvalidSvg(
                    "unterminated SVG style element".into(),
                ));
            };
            validate_css_resource_policy(&svg[content_start..content_start + relative_close])?;
        }
        cursor = end + 1;
    }
    Ok(())
}

fn xml_tag_end(svg: &str, mut cursor: usize) -> Option<usize> {
    let bytes = svg.as_bytes();
    let mut quote = None;
    while cursor < bytes.len() {
        match (quote, bytes[cursor]) {
            (None, b'\'' | b'"') => quote = Some(bytes[cursor]),
            (Some(expected), current) if expected == current => quote = None,
            (None, b'>') => return Some(cursor),
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn split_xml_name(tag: &str) -> (&str, &str) {
    let end = tag
        .find(|character: char| character.is_ascii_whitespace() || character == '/')
        .unwrap_or(tag.len());
    (&tag[..end], &tag[end..])
}

fn validate_svg_attributes(mut attributes: &str) -> Result<(), NativePngError> {
    while !attributes.trim_start().is_empty() {
        attributes = attributes.trim_start();
        if attributes.starts_with('/') {
            break;
        }
        let name_end = attributes
            .find(|character: char| {
                character.is_ascii_whitespace() || character == '=' || character == '/'
            })
            .unwrap_or(attributes.len());
        if name_end == 0 {
            break;
        }
        let name = &attributes[..name_end];
        attributes = attributes[name_end..].trim_start();
        if !attributes.starts_with('=') {
            continue;
        }
        attributes = attributes[1..].trim_start();
        let Some(quote) = attributes.as_bytes().first().copied() else {
            break;
        };
        if quote != b'\'' && quote != b'"' {
            continue;
        }
        attributes = &attributes[1..];
        let Some(value_end) = attributes.as_bytes().iter().position(|byte| *byte == quote) else {
            return Err(NativePngError::InvalidSvg(
                "unterminated XML attribute".into(),
            ));
        };
        let value = &attributes[..value_end];
        attributes = &attributes[value_end + 1..];

        let lower_name = name.to_ascii_lowercase();
        let local_name = lower_name
            .rsplit_once(':')
            .map_or(lower_name.as_str(), |(_, local)| local);
        if lower_name == "xml:base"
            || matches!(
                local_name,
                "base"
                    | "src"
                    | "poster"
                    | "schemalocation"
                    | "nonamespaceschemalocation"
                    | "externalresourcesrequired"
            )
        {
            return Err(NativePngError::ExternalResourceDenied);
        }
        if local_name == "href" && !is_safe_local_fragment(value) {
            return Err(NativePngError::ExternalResourceDenied);
        }
        if local_name == "style" {
            validate_css_resource_policy(value)?;
        } else {
            validate_local_url_functions(value)?;
        }
    }
    Ok(())
}

fn validate_css_resource_policy(css: &str) -> Result<(), NativePngError> {
    let lower = css.to_ascii_lowercase();
    if lower.contains('@')
        || lower.contains("url")
        || lower.contains("src:")
        || lower.contains('\\')
        || lower.contains('&')
        || lower.contains("/*")
        || lower.contains("http:")
        || lower.contains("https:")
        || lower.contains("file:")
        || lower.contains("data:")
        || lower.contains("//")
    {
        return Err(NativePngError::ExternalResourceDenied);
    }
    Ok(())
}

fn validate_local_url_functions(value: &str) -> Result<(), NativePngError> {
    let lower = value.to_ascii_lowercase();
    let mut rest = lower.as_str();
    while let Some(url_start) = rest.find("url") {
        let after_name = rest[url_start + 3..].trim_start();
        let Some(argument) = after_name.strip_prefix('(') else {
            return Err(NativePngError::ExternalResourceDenied);
        };
        let Some(end) = argument.find(')') else {
            return Err(NativePngError::ExternalResourceDenied);
        };
        let reference = argument[..end].trim().trim_matches(['\'', '"']);
        if !is_safe_local_fragment(reference) {
            return Err(NativePngError::ExternalResourceDenied);
        }
        rest = &argument[end + 1..];
    }
    Ok(())
}

fn is_safe_local_fragment(reference: &str) -> bool {
    let Some(fragment) = reference.strip_prefix('#') else {
        return false;
    };
    !fragment.is_empty()
        && fragment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn validate_dimensions(width: u32, height: u32) -> Result<(), NativePngError> {
    if width == 0
        || height == 0
        || width > MAX_NATIVE_RASTER_DIMENSION
        || height > MAX_NATIVE_RASTER_DIMENSION
    {
        return Err(NativePngError::InvalidDimensions { width, height });
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > MAX_NATIVE_RASTER_PIXELS {
        return Err(NativePngError::RasterPixelLimit {
            pixels,
            maximum: MAX_NATIVE_RASTER_PIXELS,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::standalone_export_svg_with_size;

    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    const BACKGROUND_RGBA: &[u8] = &[0x12, 0x16, 0x17, 0xff];

    #[test]
    fn native_png_has_exact_dimensions_and_semantic_vector_pixels() {
        let markup = "<path class=\"wb-curve\" d=\"M 100 100 L 900 600\"/><circle class=\"wb-point\" cx=\"500\" cy=\"350\" r=\"8\"/>";
        let first = render_scene_png(markup, 320, 224).expect("native PNG");
        let second = render_scene_png(markup, 320, 224).expect("repeat native PNG");

        assert_eq!(first, second);
        assert_eq!(&first[..8], PNG_SIGNATURE);
        assert_eq!(u32::from_be_bytes(first[16..20].try_into().unwrap()), 320);
        assert_eq!(u32::from_be_bytes(first[20..24].try_into().unwrap()), 224);
        let decoded = tiny_skia::Pixmap::decode_png(&first).expect("decode rendered PNG");
        assert!(
            decoded
                .data()
                .chunks_exact(4)
                .any(|pixel| pixel != BACKGROUND_RGBA),
            "semantic curve pixels must differ from the fixed background"
        );
    }

    #[test]
    fn native_png_renders_text_from_the_bundled_font() {
        let png = render_scene_png(
            "<text class=\"wb-dimension\" x=\"500\" y=\"350\">42 mm</text>",
            320,
            224,
        )
        .expect("font-backed PNG");
        let decoded = tiny_skia::Pixmap::decode_png(&png).expect("decode rendered PNG");

        assert!(
            decoded
                .data()
                .chunks_exact(4)
                .any(|pixel| pixel != BACKGROUND_RGBA),
            "bundled font must produce visible semantic pixels"
        );
    }

    #[test]
    fn native_png_rejects_unbounded_dimensions_and_image_resources() {
        assert_eq!(
            render_scene_png("", 0, 1),
            Err(NativePngError::InvalidDimensions {
                width: 0,
                height: 1,
            })
        );
        assert!(matches!(
            render_scene_png("", MAX_NATIVE_RASTER_DIMENSION, MAX_NATIVE_RASTER_DIMENSION),
            Err(NativePngError::RasterPixelLimit { .. })
        ));
        assert_eq!(
            render_scene_png(
                "<image href=\"file:///tmp/unapproved.png\" x=\"0\" y=\"0\" width=\"10\" height=\"10\"/>",
                10,
                7,
            ),
            Err(NativePngError::ImageResourceDenied)
        );
    }

    #[test]
    fn native_png_rejects_every_external_image_reference_form() {
        for href in [
            "https://example.invalid/pixel.png",
            "file:///tmp/unapproved.png",
            "data:image/png;base64,AA==",
            "//example.invalid/pixel.png",
            "relative/pixel.png",
        ] {
            let svg = format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\"><image href=\"{href}\" width=\"1\" height=\"1\"/></svg>"
            );
            assert_eq!(
                render_svg_png(&svg, 10, 7),
                Err(NativePngError::ImageResourceDenied),
                "image resource must be denied: {href}",
            );
        }
    }

    #[test]
    fn native_png_rejects_fonts_stylesheets_dtds_entities_and_external_links() {
        let forbidden = [
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\"><style>@font-face{font-family:x;src:url(https://example.invalid/x.woff2)}</style></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\"><style>@import 'https://example.invalid/x.css';</style></svg>",
            "<?xml-stylesheet href=\"https://example.invalid/x.css\"?><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\"/>",
            "<!DOCTYPE svg SYSTEM \"file:///tmp/unapproved.dtd\"><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\"/>",
            "<!DOCTYPE svg [<!ENTITY unused SYSTEM \"file:///tmp/unapproved.txt\">]><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\"/>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\"><use href=\"https://example.invalid/shape.svg#x\"/></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\"><foreignObject width=\"1\" height=\"1\"/></svg>",
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\"><text style=\"font-family:x;src:url(file:///tmp/x.woff)\">x</text></svg>",
        ];
        for svg in forbidden {
            assert_eq!(
                render_svg_png(svg, 10, 7),
                Err(NativePngError::ExternalResourceDenied),
                "external route must be denied: {svg}",
            );
        }
    }

    #[test]
    fn native_png_accepts_safe_internal_fragment_references() {
        let svg = concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"7\">",
            "<defs><path id=\"safe-shape\" d=\"M1 1L9 6\"/></defs>",
            "<use href=\"#safe-shape\" stroke=\"white\"/></svg>"
        );
        let png = render_svg_png(svg, 10, 7).expect("safe internal reference");
        assert_eq!(&png[..8], PNG_SIGNATURE);
    }

    #[test]
    fn standalone_svg_must_match_the_requested_dimensions() {
        let svg = standalone_export_svg_with_size("", 20, 14).unwrap();
        assert_eq!(
            render_svg_png(&svg, 40, 28),
            Err(NativePngError::SvgDimensionMismatch {
                requested: [40, 28],
                declared: [20, 14],
            })
        );
    }
}
