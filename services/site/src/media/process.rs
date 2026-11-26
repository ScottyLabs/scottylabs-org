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
/// Most pixels decoded, such as 8000 by 5000. A small, highly compressed file can describe a far larger image, which would take seconds and gigabytes to decode and resize; no page needs one.
pub const MAX_PIXELS: u64 = 40_000_000;
/// Most bytes of decoded pixels: 40 megapixels at 4 bytes each. Checked from the header with the pixel count, so an image of 16 bits per channel, which decodes to twice the bytes, is refused above 20 megapixels instead of taking 320 MB or more.
pub const MAX_DECODED_BYTES: u64 = MAX_PIXELS * 4;
/// What the decoder may allocate in all: the pixels plus its own working buffers.
const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;
/// Resampling with a filter first builds a floating point copy of (source width x new height) pixels, 16 bytes each. Past this size the cheaper box filter is used.
const RESAMPLE_BUDGET_BYTES: u64 = 160 * 1024 * 1024;
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
    /// Within the pixel limit, but too many bytes per pixel, such as 16 bits per channel.
    #[error("the image holds too much data to decode")]
    TooMuchData,
    /// Too many uploads are already being processed or waiting.
    #[error("too many uploads are being processed")]
    Busy,
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
    let (decoded, orientation) = decode(bytes, format)?;
    // Every variant is stored with 8 bits per channel, so a 16-bit original is narrowed first: each copy made from here on takes half the memory.
    let mut current = eight_bits(decoded);
    // A quarter or three quarter turn swaps width and height, and the widths are for the image as it is shown.
    let turned = matches!(
        orientation,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    );
    let (shown_width, shown_height) = if turned {
        (current.height(), current.width())
    } else {
        (current.width(), current.height())
    };
    let widths = target_widths(shown_width);

    // Shrink to the widest version first and turn that, so the turn never copies the full-size original.
    let widest = widths[widths.len() - 1];
    if widest != shown_width {
        let shown_height = scaled_height(shown_width, shown_height, widest);
        current = if turned {
            shrink_to(current, shown_height, widest)
        } else {
            shrink_to(current, widest, shown_height)
        };
    }
    current.apply_orientation(orientation);

    // Resize from the widest down, each step from the previous one, which is much faster than resizing the original every time and looks the same. Each version is encoded as soon as it exists, so only one is held at a time.
    let mut output = None;
    let mut variants = Vec::with_capacity(widths.len());
    for width in widths.iter().rev() {
        if current.width() != *width {
            current = shrink(current, *width);
        }
        // The widest version decides the format for all of them.
        let format = *output.get_or_insert_with(|| {
            if has_transparency(&current) {
                OutputFormat::Png
            } else {
                OutputFormat::Jpeg
            }
        });
        variants.push(Variant {
            width: current.width(),
            height: current.height(),
            bytes: encode(&current, format)?,
        });
    }
    variants.reverse();
    Ok(Processed {
        format: output.unwrap_or(OutputFormat::Jpeg),
        variants,
    })
}

/// The image with 8 bits per channel, keeping its channels; 8-bit images are returned as they are. The original is freed once the copy exists.
fn eight_bits(image: DynamicImage) -> DynamicImage {
    match image {
        DynamicImage::ImageLuma8(_)
        | DynamicImage::ImageLumaA8(_)
        | DynamicImage::ImageRgb8(_)
        | DynamicImage::ImageRgba8(_) => image,
        wide if wide.color().has_alpha() => DynamicImage::ImageRgba8(wide.into_rgba8()),
        wide => DynamicImage::ImageRgb8(wide.into_rgb8()),
    }
}

/// `image` scaled down to `width`, keeping its proportions.
fn shrink(image: DynamicImage, width: u32) -> DynamicImage {
    let height = scaled_height(image.width(), image.height(), width);
    shrink_to(image, width, height)
}

/// `image` scaled down to `width` by `height`. Takes the image so that a large original is freed before the resampling filter builds its copy: at most two full-size buffers exist at once.
fn shrink_to(image: DynamicImage, width: u32, height: u32) -> DynamicImage {
    // Most of a large original is first averaged down to twice the size with the fast integer box filter, which looks the same at that ratio and needs no floating point copy of the original.
    let source = if image.width() > 2 * width {
        let halfway = image.thumbnail_exact(2 * width, 2 * height);
        drop(image);
        halfway
    } else {
        image
    };
    let resample_bytes = u64::from(source.width()) * u64::from(height) * 16;
    if resample_bytes > RESAMPLE_BUDGET_BYTES {
        // Very tall images: the filter's copy would be too large.
        source.thumbnail_exact(width, height)
    } else {
        source.resize_exact(width, height, FilterType::CatmullRom)
    }
}

/// The decoded image as stored in the file, and the EXIF orientation it is shown in.
fn decode(bytes: &[u8], format: ImageFormat) -> Result<(DynamicImage, Orientation), ProcessError> {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(limits);
    let mut decoder = reader.into_decoder().map_err(decode_error)?;
    // Checked from the header, before any pixel is decoded.
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(ProcessError::TooLarge);
    }
    if decoder.total_bytes() > MAX_DECODED_BYTES {
        return Err(ProcessError::TooMuchData);
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let image = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    if image.width() == 0 || image.height() == 0 {
        return Err(ProcessError::Unreadable("empty image".into()));
    }
    Ok((image, orientation))
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

    /// A PNG that claims to be `width` by `height` but holds a single row of data. Its pixels are never needed: the size alone must stop it.
    fn png_header_only(width: u32, height: u32) -> Vec<u8> {
        // 8-bit grayscale.
        png_header(width, height, 8, 0)
    }

    /// [`png_header_only`] with a bit depth and PNG color type, such as 16 and 6 for 16-bit RGBA.
    fn png_header(width: u32, height: u32, depth: u8, color: u8) -> Vec<u8> {
        fn crc32(bytes: &[u8]) -> u32 {
            let mut crc = u32::MAX;
            for byte in bytes {
                crc ^= u32::from(*byte);
                for _ in 0..8 {
                    crc = if crc & 1 == 1 {
                        (crc >> 1) ^ 0xEDB8_8320
                    } else {
                        crc >> 1
                    };
                }
            }
            !crc
        }
        fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
            out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
            let mut body = kind.to_vec();
            body.extend_from_slice(data);
            out.extend_from_slice(&body);
            out.extend_from_slice(&crc32(&body).to_be_bytes());
        }
        let mut header = Vec::new();
        header.extend_from_slice(&width.to_be_bytes());
        header.extend_from_slice(&height.to_be_bytes());
        // No interlacing.
        header.extend_from_slice(&[depth, color, 0, 0, 0]);
        let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
        chunk(&mut out, b"IHDR", &header);
        // A stored zlib block of one empty row.
        chunk(
            &mut out,
            b"IDAT",
            &[
                0x78, 0x01, 0x01, 0x01, 0x00, 0xFE, 0xFF, 0x00, 0x00, 0x01, 0x00, 0x01,
            ],
        );
        chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    fn small_files_that_decode_to_huge_images_are_refused_from_the_header() {
        let started = std::time::Instant::now();
        for (width, height) in [(12_000, 12_000), (8_000, 5_001), (MAX_DIMENSION, 3_334)] {
            let bomb = png_header_only(width, height);
            assert!(bomb.len() < 100);
            assert!(
                matches!(process(&bomb), Err(ProcessError::TooLarge)),
                "{width}x{height}"
            );
        }
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        // At the budget the header passes and decoding the missing rows fails instead.
        assert!(matches!(
            process(&png_header_only(8_000, 5_000)),
            Err(ProcessError::Unreadable(_))
        ));
    }

    #[test]
    fn deep_images_are_refused_from_the_header_by_their_bytes() {
        // 30 megapixels is within the pixel limit, but at 16 bits per channel it decodes to 240 MB.
        assert!(matches!(
            process(&png_header(6_000, 5_000, 16, 6)),
            Err(ProcessError::TooMuchData)
        ));
        assert!(matches!(
            process(&png_header(8_000, 5_000, 16, 2)),
            Err(ProcessError::TooMuchData)
        ));
        // 16 megapixels at 16 bits per channel is 128 MB: the header passes, and the missing rows fail.
        assert!(matches!(
            process(&png_header(4_000, 4_000, 16, 6)),
            Err(ProcessError::Unreadable(_))
        ));
        // 40 megapixels of 8-bit RGBA is exactly the budget.
        assert!(matches!(
            process(&png_header(8_000, 5_000, 8, 6)),
            Err(ProcessError::Unreadable(_))
        ));
    }

    /// EXIF with only an orientation, as a big-endian TIFF block.
    fn exif_orientation(value: u16) -> Vec<u8> {
        let mut exif = b"MM\0\x2a\0\0\0\x08".to_vec();
        exif.extend_from_slice(&1u16.to_be_bytes());
        exif.extend_from_slice(&0x0112u16.to_be_bytes());
        // One SHORT.
        exif.extend_from_slice(&3u16.to_be_bytes());
        exif.extend_from_slice(&1u32.to_be_bytes());
        exif.extend_from_slice(&value.to_be_bytes());
        exif.extend_from_slice(&[0, 0]);
        // No next directory.
        exif.extend_from_slice(&0u32.to_be_bytes());
        exif
    }

    #[test]
    fn turned_photos_are_shrunk_before_they_are_turned() {
        use image::{ExtendedColorType, ImageEncoder};

        // Stored 1000 wide and 2400 tall, left half red and right half blue, shown turned a quarter clockwise.
        let (width, height) = (1_000, 2_400);
        let stored = RgbImage::from_fn(width, height, |x, _| {
            if x < width / 2 {
                Rgb([220, 20, 20])
            } else {
                Rgb([20, 20, 220])
            }
        });
        let mut bytes = Vec::new();
        let mut encoder = PngEncoder::new(&mut bytes);
        encoder.set_exif_metadata(exif_orientation(6)).unwrap();
        encoder
            .write_image(stored.as_raw(), width, height, ExtendedColorType::Rgb8)
            .unwrap();

        let processed = process(&bytes).unwrap();
        let sizes: Vec<(u32, u32)> = processed
            .variants
            .iter()
            .map(|v| (v.width, v.height))
            .collect();
        assert_eq!(sizes, [(480, 200), (960, 400), (1920, 800)]);
        // The stored left edge is shown on top.
        let shown = image::load_from_memory(&processed.variants[2].bytes)
            .unwrap()
            .to_rgb8();
        let top = shown.get_pixel(960, 100).0;
        let bottom = shown.get_pixel(960, 700).0;
        assert!(top[0] > 150 && top[2] < 100, "{top:?}");
        assert!(bottom[2] > 150 && bottom[0] < 100, "{bottom:?}");
    }

    #[test]
    fn wide_and_tall_images_shrink_to_every_width() {
        let wide = process(&png(4_000, 100, 255)).unwrap();
        let sizes: Vec<(u32, u32)> = wide.variants.iter().map(|v| (v.width, v.height)).collect();
        assert_eq!(sizes, [(480, 12), (960, 24), (1920, 48)]);
        // Too tall for the filter's floating point copy, so the box filter does it.
        let tall = shrink(DynamicImage::new_rgb8(1_921, 5_500), 1_920);
        assert_eq!((tall.width(), tall.height()), (1_920, 5_497));
    }

    #[test]
    fn sixteen_bit_images_are_narrowed_before_resizing() {
        let wide = DynamicImage::ImageRgba16(image::ImageBuffer::from_pixel(
            6,
            4,
            image::Rgba([65_535u16, 0, 0, 32_768]),
        ));
        let narrowed = eight_bits(wide);
        assert_eq!(narrowed.color(), image::ColorType::Rgba8);
        assert_eq!(narrowed.to_rgba8().get_pixel(0, 0).0, [255, 0, 0, 128]);
        let opaque = eight_bits(DynamicImage::ImageRgb16(image::ImageBuffer::new(2, 2)));
        assert_eq!(opaque.color(), image::ColorType::Rgb8);
        let gray = DynamicImage::new_luma8(2, 2);
        assert_eq!(eight_bits(gray).color(), image::ColorType::L8);

        let mut bytes = Vec::new();
        DynamicImage::ImageRgb16(image::ImageBuffer::from_pixel(
            2_400,
            1_000,
            image::Rgb([40_000u16, 20_000, 10_000]),
        ))
        .write_with_encoder(PngEncoder::new(&mut bytes))
        .unwrap();
        let processed = process(&bytes).unwrap();
        assert_eq!(processed.format, OutputFormat::Jpeg);
        assert_eq!(
            processed.largest().map(|v| (v.width, v.height)),
            Some((1_920, 800))
        );
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
