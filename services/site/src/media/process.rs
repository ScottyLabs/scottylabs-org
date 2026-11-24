//! Turns an uploaded file into clean, resized images. The format comes from the bytes, never the name or header. Decoding has hard limits, and re-encoding drops every bit of metadata after applying the EXIF orientation, so phone photos keep their rotation without their location.
//!
//! CPU heavy: call [`process`] from `spawn_blocking`.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageError, ImageFormat, ImageReader, Limits};

/// Largest file accepted.
pub const MAX_UPLOAD_BYTES: usize = 5 * 1024 * 1024;
/// Largest width or height decoded.
pub const MAX_DIMENSION: u32 = 12_000;
const MAX_DECODE_ALLOC: u64 = 512 * 1024 * 1024;
/// Widths made for every upload, never wider than the original.
pub const WIDTHS: [u32; 3] = [480, 960, 1920];
const JPEG_QUALITY: u8 = 82;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    Jpeg,
    Png,
}

impl OutputFormat {
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
        }
    }
}

pub struct Variant {
    pub width: u32,
    pub height: u32,
    pub bytes: Vec<u8>,
}

/// The stored versions of one upload, narrowest first.
pub struct Processed {
    pub format: OutputFormat,
    pub variants: Vec<Variant>,
}

impl Processed {
    /// The widest version, whose size is the image's size as far as pages are concerned.
    pub fn largest(&self) -> Option<&Variant> {
        self.variants.last()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    #[error("not a JPEG, PNG or WebP image")]
    Unsupported,
    #[error("the image is too large to decode")]
    TooLarge,
    #[error("the image could not be decoded: {0}")]
    Unreadable(String),
    #[error("the image could not be encoded: {0}")]
    Encode(String),
}

/// Recognizes JPEG, PNG and WebP by their magic bytes.
pub fn sniff(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageFormat::Jpeg)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageFormat::Png)
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageFormat::WebP)
    } else {
        None
    }
}

/// Widths to store for an image `original` pixels wide: each standard width below it, and the largest one capped at the original, so nothing is ever upscaled.
pub fn target_widths(original: u32) -> Vec<u32> {
    let mut widths: Vec<u32> = WIDTHS.iter().copied().filter(|w| *w < original).collect();
    widths.push(original.min(WIDTHS[WIDTHS.len() - 1]));
    widths.dedup();
    widths
}

pub fn process(bytes: &[u8]) -> Result<Processed, ProcessError> {
    let format = sniff(bytes).ok_or(ProcessError::Unsupported)?;
    let image = decode(bytes, format)?;
    let widths = target_widths(image.width());

    // Resize from the widest down, each step from the previous one, which is much faster than resizing the original every time and looks the same.
    let mut resized = Vec::with_capacity(widths.len());
    let mut current = image;
    for width in widths.iter().rev() {
        if current.width() != *width {
            let height = scaled_height(current.width(), current.height(), *width);
            current = current.resize_exact(*width, height, FilterType::CatmullRom);
        }
        resized.push(current.clone());
    }
    resized.reverse();

    let output = match resized.last() {
        Some(widest) if has_transparency(widest) => OutputFormat::Png,
        _ => OutputFormat::Jpeg,
    };
    let variants = resized
        .iter()
        .map(|image| {
            Ok(Variant {
                width: image.width(),
                height: image.height(),
                bytes: encode(image, output)?,
            })
        })
        .collect::<Result<Vec<_>, ProcessError>>()?;
    Ok(Processed {
        format: output,
        variants,
    })
}

fn decode(bytes: &[u8], format: ImageFormat) -> Result<DynamicImage, ProcessError> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    image.apply_orientation(orientation);
    if image.width() == 0 || image.height() == 0 {
        return Err(ProcessError::Unreadable("empty image".into()));
    }
    Ok(image)
}

fn decode_error(error: ImageError) -> ProcessError {
    match error {
        ImageError::Limits(_) => ProcessError::TooLarge,
        ImageError::Unsupported(_) => ProcessError::Unsupported,
        other => ProcessError::Unreadable(other.to_string()),
    }
}

fn scaled_height(width: u32, height: u32, new_width: u32) -> u32 {
    let scaled = u64::from(height) * u64::from(new_width) / u64::from(width.max(1));
    u32::try_from(scaled).unwrap_or(u32::MAX).max(1)
}

/// True when some pixel is not fully opaque. An alpha channel that is all opaque does not count.
fn has_transparency(image: &DynamicImage) -> bool {
    if !image.color().has_alpha() {
        return false;
    }
    image.to_rgba8().pixels().any(|pixel| pixel.0[3] < u8::MAX)
}

fn encode(image: &DynamicImage, format: OutputFormat) -> Result<Vec<u8>, ProcessError> {
    let mut bytes = Vec::new();
    let result = match format {
        OutputFormat::Jpeg => DynamicImage::ImageRgb8(image.to_rgb8())
            .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, JPEG_QUALITY)),
        OutputFormat::Png => DynamicImage::ImageRgba8(image.to_rgba8())
            .write_with_encoder(PngEncoder::new(&mut bytes)),
    };
    result.map_err(|error| ProcessError::Encode(error.to_string()))?;
    Ok(bytes)
}

#[cfg(test)]
pub(crate) mod tests {
    use image::{Rgb, RgbImage, Rgba, RgbaImage};

    use super::*;

    pub fn jpeg(width: u32, height: u32) -> Vec<u8> {
        let image = RgbImage::from_fn(width, height, |x, y| {
            Rgb([(x % 256) as u8, (y % 256) as u8, 90])
        });
        let mut bytes = Vec::new();
        DynamicImage::ImageRgb8(image)
            .write_with_encoder(JpegEncoder::new_with_quality(&mut bytes, 90))
            .unwrap();
        bytes
    }

    pub fn png(width: u32, height: u32, alpha: u8) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, Rgba([200, 30, 40, alpha]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_with_encoder(PngEncoder::new(&mut bytes))
            .unwrap();
        bytes
    }

    pub fn webp(width: u32, height: u32) -> Vec<u8> {
        let image = RgbaImage::from_pixel(width, height, Rgba([10, 20, 30, 255]));
        let mut bytes = Vec::new();
        DynamicImage::ImageRgba8(image)
            .write_with_encoder(image::codecs::webp::WebPEncoder::new_lossless(&mut bytes))
            .unwrap();
        bytes
    }

    #[test]
    fn sniffs_only_allowed_formats() {
        assert_eq!(sniff(&jpeg(4, 4)), Some(ImageFormat::Jpeg));
        assert_eq!(sniff(&png(4, 4, 255)), Some(ImageFormat::Png));
        assert_eq!(sniff(&webp(4, 4)), Some(ImageFormat::WebP));
        assert_eq!(sniff(b"GIF89a\x01\x00\x01\x00"), None);
        assert_eq!(sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"), None);
        assert_eq!(sniff(b""), None);
    }

    #[test]
    fn widths_never_upscale() {
        assert_eq!(target_widths(4000), [480, 960, 1920]);
        assert_eq!(target_widths(1920), [480, 960, 1920]);
        assert_eq!(target_widths(1200), [480, 960, 1200]);
        assert_eq!(target_widths(960), [480, 960]);
        assert_eq!(target_widths(300), [300]);
    }

    #[test]
    fn opaque_images_become_jpeg_variants() {
        let processed = process(&jpeg(2000, 1000)).unwrap();
        assert_eq!(processed.format, OutputFormat::Jpeg);
        let sizes: Vec<(u32, u32)> = processed
            .variants
            .iter()
            .map(|v| (v.width, v.height))
            .collect();
        assert_eq!(sizes, [(480, 240), (960, 480), (1920, 960)]);
        for variant in &processed.variants {
            assert_eq!(sniff(&variant.bytes), Some(ImageFormat::Jpeg));
        }
    }

    #[test]
    fn transparency_keeps_png_and_opaque_png_becomes_jpeg() {
        let clear = process(&png(600, 300, 128)).unwrap();
        assert_eq!(clear.format, OutputFormat::Png);
        assert_eq!(sniff(&clear.variants[0].bytes), Some(ImageFormat::Png));
        let solid = process(&png(600, 300, 255)).unwrap();
        assert_eq!(solid.format, OutputFormat::Jpeg);
        assert_eq!(solid.largest().map(|v| v.width), Some(600));
    }

    #[test]
    fn webp_is_accepted_and_re_encoded() {
        let processed = process(&webp(100, 50)).unwrap();
        assert_eq!(processed.format, OutputFormat::Jpeg);
        assert_eq!(processed.variants.len(), 1);
        assert_eq!(
            (processed.variants[0].width, processed.variants[0].height),
            (100, 50)
        );
    }

    #[test]
    fn rejects_other_and_broken_files() {
        assert!(matches!(
            process(b"GIF89a....."),
            Err(ProcessError::Unsupported)
        ));
        assert!(matches!(process(b"<svg/>"), Err(ProcessError::Unsupported)));
        let mut broken = jpeg(50, 50);
        broken.truncate(40);
        assert!(matches!(process(&broken), Err(ProcessError::Unreadable(_))));
    }

    #[test]
    fn refuses_images_over_the_pixel_limit() {
        assert!(matches!(
            process(&png(MAX_DIMENSION + 1, 1, 255)),
            Err(ProcessError::TooLarge)
        ));
        assert!(matches!(
            process(&png(1, MAX_DIMENSION + 1, 255)),
            Err(ProcessError::TooLarge)
        ));
    }
}
