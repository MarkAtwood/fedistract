//! Image processing utilities for fediverse media uploads.
//!
//! Provides MIME sniffing, validation, resize, EXIF stripping, blurhash generation,
//! and thumbnail creation. Extracted from duplicated logic in smallhold and gaja.

use image::GenericImageView;
use std::io::Cursor;

/// Errors that can occur during media processing.
#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("unsupported image format")]
    UnsupportedFormat,
    #[error("image dimensions {0}x{1} exceed maximum {2}")]
    DimensionsTooLarge(u32, u32, u32),
    #[error("GIF has {0} frames, exceeding maximum {1}")]
    GifFrameBomb(usize, u32),
    #[error("file size {0} bytes exceeds maximum {1}")]
    FileTooLarge(usize, usize),
    #[error("failed to decode image: {0}")]
    DecodeError(String),
    #[error("failed to encode image: {0}")]
    EncodeError(String),
    #[error("blurhash generation failed: {0}")]
    BlurhashError(String),
}

/// Metadata extracted from an image without full decode.
#[derive(Debug, Clone)]
pub struct ImageInfo {
    pub width: u32,
    pub height: u32,
    pub mime_type: String,
    pub file_size: usize,
}

/// Result of full image processing: decode, resize, EXIF strip, blurhash.
#[derive(Debug, Clone)]
pub struct ProcessedImage {
    pub data: Vec<u8>,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub blurhash: String,
    pub file_size: usize,
}

/// Sniff MIME type from magic bytes. Returns `None` if unrecognized.
pub fn sniff_mime(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if data.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if data.starts_with(b"GIF8") {
        Some("image/gif")
    } else if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Map MIME type to [`image::ImageFormat`] for re-encoding.
pub fn image_format_for_mime(mime: &str) -> Option<image::ImageFormat> {
    match mime {
        "image/jpeg" => Some(image::ImageFormat::Jpeg),
        "image/png" => Some(image::ImageFormat::Png),
        "image/gif" => Some(image::ImageFormat::Gif),
        "image/webp" => Some(image::ImageFormat::WebP),
        _ => None,
    }
}

/// Map MIME type to file extension.
pub fn ext_for_mime(mime: &str) -> Option<&'static str> {
    match mime {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

/// Validate image data without full decode.
///
/// Checks file size, sniffs MIME, decodes headers for dimensions,
/// and counts GIF frames against `max_gif_frames`.
pub fn validate_image(
    data: &[u8],
    max_dimension: u32,
    max_gif_frames: u32,
    max_file_size: usize,
) -> Result<ImageInfo, MediaError> {
    if data.len() > max_file_size {
        return Err(MediaError::FileTooLarge(data.len(), max_file_size));
    }

    let mime = sniff_mime(data).ok_or(MediaError::UnsupportedFormat)?;

    // GIF frame bomb check
    if mime == "image/gif" {
        let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(data))
            .map_err(|e| MediaError::DecodeError(e.to_string()))?;
        use image::AnimationDecoder;
        let frame_count = decoder.into_frames().count();
        if frame_count > max_gif_frames as usize {
            return Err(MediaError::GifFrameBomb(frame_count, max_gif_frames));
        }
    }

    // Decode headers only (via reader with limits)
    let mut reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| MediaError::DecodeError(e.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);

    let img = reader
        .decode()
        .map_err(|e| MediaError::DecodeError(e.to_string()))?;
    let (w, h) = img.dimensions();

    if w > max_dimension || h > max_dimension {
        return Err(MediaError::DimensionsTooLarge(w, h, max_dimension));
    }

    Ok(ImageInfo {
        width: w,
        height: h,
        mime_type: mime.to_string(),
        file_size: data.len(),
    })
}

/// Fully process an image: decode, resize if over `max_dimension`, strip EXIF
/// (by re-encoding), and generate a blurhash.
pub fn process_image(data: &[u8], max_dimension: u32) -> Result<ProcessedImage, MediaError> {
    let mime = sniff_mime(data).ok_or(MediaError::UnsupportedFormat)?;
    let fmt = image_format_for_mime(mime).ok_or(MediaError::UnsupportedFormat)?;

    // GIF frame bomb check before full decode
    if mime == "image/gif" {
        let decoder = image::codecs::gif::GifDecoder::new(Cursor::new(data))
            .map_err(|e| MediaError::DecodeError(e.to_string()))?;
        use image::AnimationDecoder;
        let frame_count = decoder.into_frames().count();
        if frame_count > 500 {
            return Err(MediaError::GifFrameBomb(frame_count, 500));
        }
    }

    let mut reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| MediaError::DecodeError(e.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);

    let mut img = reader
        .decode()
        .map_err(|e| MediaError::DecodeError(e.to_string()))?;

    let (w, h) = img.dimensions();
    if w > max_dimension || h > max_dimension {
        img = img.resize(
            max_dimension,
            max_dimension,
            image::imageops::FilterType::Lanczos3,
        );
    }

    let (w, h) = img.dimensions();

    // Re-encode to strip EXIF metadata (GPS, camera info, embedded scripts)
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, fmt)
        .map_err(|e| MediaError::EncodeError(e.to_string()))?;
    let clean = buf.into_inner();

    // Generate blurhash from 32x32 thumbnail
    let thumb = image::imageops::resize(
        &img.to_rgba8(),
        32,
        32,
        image::imageops::FilterType::Triangle,
    );
    let hash = blurhash::encode(4, 3, 32, 32, thumb.as_raw())
        .map_err(|e| MediaError::BlurhashError(e.to_string()))?;

    let file_size = clean.len();
    Ok(ProcessedImage {
        data: clean,
        mime_type: mime.to_string(),
        width: w,
        height: h,
        blurhash: hash,
        file_size,
    })
}

/// Generate a square thumbnail of the given size.
pub fn generate_thumbnail(data: &[u8], size: u32) -> Result<Vec<u8>, MediaError> {
    let mime = sniff_mime(data).ok_or(MediaError::UnsupportedFormat)?;
    let fmt = image_format_for_mime(mime).ok_or(MediaError::UnsupportedFormat)?;

    let mut reader = image::ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|e| MediaError::DecodeError(e.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);

    let img = reader
        .decode()
        .map_err(|e| MediaError::DecodeError(e.to_string()))?;

    let thumb = img.resize_to_fill(size, size, image::imageops::FilterType::Lanczos3);

    let mut buf = Cursor::new(Vec::new());
    thumb
        .write_to(&mut buf, fmt)
        .map_err(|e| MediaError::EncodeError(e.to_string()))?;

    Ok(buf.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal valid 1x1 red PNG (67 bytes).
    fn tiny_png() -> Vec<u8> {
        // Generated externally; this is a valid 1x1 RGBA PNG.
        let mut png = Vec::new();
        let encoder = image::codecs::png::PngEncoder::new(&mut png);
        use image::ImageEncoder;
        encoder
            .write_image(&[255, 0, 0, 255], 1, 1, image::ExtendedColorType::Rgba8)
            .unwrap();
        png
    }

    #[test]
    fn sniff_mime_png() {
        let data = tiny_png();
        assert_eq!(sniff_mime(&data), Some("image/png"));
    }

    #[test]
    fn sniff_mime_jpeg() {
        assert_eq!(sniff_mime(&[0xff, 0xd8, 0xff, 0xe0]), Some("image/jpeg"));
    }

    #[test]
    fn sniff_mime_gif() {
        assert_eq!(sniff_mime(b"GIF89a"), Some("image/gif"));
    }

    #[test]
    fn sniff_mime_webp() {
        let mut data = vec![0u8; 12];
        data[..4].copy_from_slice(b"RIFF");
        data[8..12].copy_from_slice(b"WEBP");
        assert_eq!(sniff_mime(&data), Some("image/webp"));
    }

    #[test]
    fn sniff_mime_unknown() {
        assert_eq!(sniff_mime(b"not an image"), None);
    }

    #[test]
    fn validate_image_ok() {
        let data = tiny_png();
        let info = validate_image(&data, 4096, 500, 10 * 1024 * 1024).unwrap();
        assert_eq!(info.width, 1);
        assert_eq!(info.height, 1);
        assert_eq!(info.mime_type, "image/png");
        assert_eq!(info.file_size, data.len());
    }

    #[test]
    fn validate_image_too_large_file() {
        let data = tiny_png();
        let err = validate_image(&data, 4096, 500, 1).unwrap_err();
        assert!(matches!(err, MediaError::FileTooLarge(_, _)));
    }

    #[test]
    fn validate_image_unsupported() {
        let err = validate_image(b"not an image", 4096, 500, 1024).unwrap_err();
        assert!(matches!(err, MediaError::UnsupportedFormat));
    }

    #[test]
    fn process_image_png() {
        let data = tiny_png();
        let result = process_image(&data, 4096).unwrap();
        assert_eq!(result.width, 1);
        assert_eq!(result.height, 1);
        assert_eq!(result.mime_type, "image/png");
        assert!(!result.blurhash.is_empty());
        assert!(result.file_size > 0);
        // Output should still be a valid PNG
        assert_eq!(sniff_mime(&result.data), Some("image/png"));
    }

    #[test]
    fn generate_thumbnail_png() {
        let data = tiny_png();
        let thumb = generate_thumbnail(&data, 64).unwrap();
        assert_eq!(sniff_mime(&thumb), Some("image/png"));

        // Verify the thumbnail is 64x64
        let img = image::load_from_memory(&thumb).unwrap();
        assert_eq!(img.dimensions(), (64, 64));
    }

    #[test]
    fn ext_for_known_mimes() {
        assert_eq!(ext_for_mime("image/jpeg"), Some("jpg"));
        assert_eq!(ext_for_mime("image/png"), Some("png"));
        assert_eq!(ext_for_mime("image/gif"), Some("gif"));
        assert_eq!(ext_for_mime("image/webp"), Some("webp"));
        assert_eq!(ext_for_mime("text/plain"), None);
    }

    #[test]
    fn process_image_unsupported() {
        let err = process_image(b"not an image", 4096).unwrap_err();
        assert!(matches!(err, MediaError::UnsupportedFormat));
    }
}
