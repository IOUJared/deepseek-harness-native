//! Local-only PNG/JPEG thumbnails. Run on an admitted blocking worker, never the UI thread.
//! This is a disposable snapshot, not an upload, inode lease or content-validation receipt.
//! Codec allocation limits are best-effort, not a process-RSS or execution-time sandbox.

use crate::worker::file_intake::{Failure, SelectedFile};
use image::{DynamicImage, GenericImageView, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::{
    cell::Cell,
    fmt,
    io::{self, BufRead, Cursor, Read, Seek, SeekFrom},
    path::PathBuf,
};

const MAX_COMPRESSED_BYTES: usize = 4 * 1024 * 1024;
const MAX_DIMENSION: u32 = 4096;
const MAX_PIXELS: u64 = 4_000_000;
const MAX_DECODE_BYTES: u64 = 32 * 1024 * 1024;
const THUMB_WIDTH: u32 = 160;
const THUMB_HEIGHT: u32 = 120;
const READ_CHUNK: usize = 64 * 1024;

/// Closed failures retain no path, filename, decoder diagnostic or compressed content.
#[derive(Clone, Eq, PartialEq)]
pub enum Outcome {
    Ready {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    /// Unsupported/non-image content; only PNG and JPEG are previewed.
    NotImage,
    /// Unreadable, malformed or over-budget content.
    Rejected,
    Canceled,
}

impl fmt::Debug for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready { width, height, .. } => f
                .debug_struct("Ready")
                .field("width", width)
                .field("height", height)
                .finish_non_exhaustive(),
            Self::NotImage => f.write_str("NotImage"),
            Self::Rejected => f.write_str("Rejected"),
            Self::Canceled => f.write_str("Canceled"),
        }
    }
}

/// Uses the same bounded, final-component-NOFOLLOW intake as upload, without uploading.
/// Parent symlinks remain permitted; cancellation cannot interrupt blocking kernel I/O.
pub fn load(path: PathBuf, canceled: impl Fn() -> bool) -> Outcome {
    let cancel = Cancellation::new(&canceled);
    if cancel.check() {
        return Outcome::Canceled;
    }
    let result = SelectedFile::new(path).and_then(|file| file.read_preview(|| cancel.check()));
    match result {
        Ok(bytes) => decode_inner(&bytes, &cancel),
        Err(Failure::Canceled) => Outcome::Canceled,
        Err(_) if cancel.check() => Outcome::Canceled,
        Err(_) => Outcome::Rejected,
    }
}

/// Byte-level verifier used by tests and local callers; no extension-based format trust.
/// Cancellation is sticky and checked at codec I/O boundaries and each thumbnail row.
/// A codec's CPU-only decode interval is not preemptible; callers must drain owned workers.
pub fn decode(bytes: &[u8], canceled: impl Fn() -> bool) -> Outcome {
    decode_inner(bytes, &Cancellation::new(&canceled))
}

fn decode_inner<F: Fn() -> bool>(bytes: &[u8], cancel: &Cancellation<'_, F>) -> Outcome {
    if cancel.check() {
        return Outcome::Canceled;
    }
    if bytes.len() > MAX_COMPRESSED_BYTES {
        return Outcome::Rejected;
    }
    // Explicit allowlist avoids hooks, extension trust, or accidentally enabled new decoders.
    let format = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        ImageFormat::Png
    } else if bytes.starts_with(b"\xff\xd8") {
        ImageFormat::Jpeg
    } else {
        return Outcome::NotImage;
    };
    let result = decode_image(bytes, format, cancel);
    if cancel.check() {
        return Outcome::Canceled;
    }
    match result {
        Ok(image) => thumbnail(image, cancel),
        Err(_) => Outcome::Rejected,
    }
}

fn decode_image<F: Fn() -> bool>(
    bytes: &[u8],
    format: ImageFormat,
    cancel: &Cancellation<'_, F>,
) -> image::ImageResult<DynamicImage> {
    let source = CancelReader {
        inner: Cursor::new(bytes),
        cancel,
    };
    let mut reader = ImageReader::with_format(source, format);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    if width == 0
        || height == 0
        || u64::from(width) * u64::from(height) > MAX_PIXELS
        || decoder.total_bytes() > MAX_DECODE_BYTES
    {
        return Err(image::ImageError::Limits(
            image::error::LimitError::from_kind(image::error::LimitErrorKind::DimensionError),
        ));
    }
    // Explicitly reserve the caller-owned output before pixel decode. JPEG's internal
    // scratch allocations do not uphold max_alloc; dimensions/pixels still cap workload.
    limits.reserve(decoder.total_bytes())?;
    decoder.set_limits(limits)?;
    cancel.io_check()?;
    DynamicImage::from_decoder(decoder)
}

fn thumbnail<F: Fn() -> bool>(image: DynamicImage, cancel: &Cancellation<'_, F>) -> Outcome {
    let (source_width, source_height) = image.dimensions();
    let (width, height) = thumbnail_dimensions(source_width, source_height);
    let mut rgba = Vec::new();
    if rgba
        .try_reserve_exact((width * height * 4) as usize)
        .is_err()
    {
        return Outcome::Rejected;
    }
    // Nearest-neighbor sampling intentionally avoids another full-size RGBA conversion.
    // The preview is not color-managed and does not apply EXIF orientation or animate APNG.
    for y in 0..height {
        if cancel.check() {
            return Outcome::Canceled;
        }
        let source_y = (u64::from(y) * u64::from(source_height) / u64::from(height)) as u32;
        for x in 0..width {
            let source_x = (u64::from(x) * u64::from(source_width) / u64::from(width)) as u32;
            rgba.extend_from_slice(&image.get_pixel(source_x, source_y).0);
        }
    }
    if cancel.check() {
        Outcome::Canceled
    } else {
        Outcome::Ready {
            width,
            height,
            rgba,
        }
    }
}

fn thumbnail_dimensions(width: u32, height: u32) -> (u32, u32) {
    if width <= THUMB_WIDTH && height <= THUMB_HEIGHT {
        return (width, height);
    }
    if u64::from(width) * u64::from(THUMB_HEIGHT) >= u64::from(height) * u64::from(THUMB_WIDTH) {
        (
            THUMB_WIDTH,
            (u64::from(height) * u64::from(THUMB_WIDTH) / u64::from(width)).max(1) as u32,
        )
    } else {
        (
            (u64::from(width) * u64::from(THUMB_HEIGHT) / u64::from(height)).max(1) as u32,
            THUMB_HEIGHT,
        )
    }
}

struct Cancellation<'a, F> {
    callback: &'a F,
    seen: Cell<bool>,
}
impl<'a, F: Fn() -> bool> Cancellation<'a, F> {
    fn new(callback: &'a F) -> Self {
        Self {
            callback,
            seen: Cell::new(false),
        }
    }
    fn check(&self) -> bool {
        if !self.seen.get() && (self.callback)() {
            self.seen.set(true);
        }
        self.seen.get()
    }
    fn io_check(&self) -> io::Result<()> {
        if self.check() {
            // Not Interrupted: codecs may retry Interrupted indefinitely.
            Err(io::Error::other("preview canceled"))
        } else {
            Ok(())
        }
    }
}

struct CancelReader<'a, 'c, F> {
    inner: Cursor<&'a [u8]>,
    cancel: &'c Cancellation<'c, F>,
}
impl<F: Fn() -> bool> Read for CancelReader<'_, '_, F> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.cancel.io_check()?;
        let len = output.len().min(READ_CHUNK);
        self.inner.read(&mut output[..len])
    }
}
impl<F: Fn() -> bool> BufRead for CancelReader<'_, '_, F> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.cancel.io_check()?;
        let bytes = self.inner.fill_buf()?;
        Ok(&bytes[..bytes.len().min(READ_CHUNK)])
    }
    fn consume(&mut self, amount: usize) {
        self.inner.consume(amount);
    }
}
impl<F: Fn() -> bool> Seek for CancelReader<'_, '_, F> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.cancel.io_check()?;
        self.inner.seek(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::hash_map::RandomState,
        fs::{self, DirBuilder, File},
        hash::{BuildHasher, Hasher},
        os::unix::fs::{DirBuilderExt, symlink},
        sync::atomic::{AtomicU64, Ordering},
    };

    const TEMP_PREFIX: &str = "dsh-native-preview-";
    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Temp {
        parent: PathBuf,
        root: PathBuf,
    }
    impl Temp {
        fn new() -> Self {
            let parent = std::env::temp_dir().canonicalize().unwrap();
            assert!(parent.is_absolute());
            loop {
                let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
                let mut hash = RandomState::new().build_hasher();
                hash.write_u64(serial);
                let root = parent.join(format!(
                    "{TEMP_PREFIX}{}-{:x}-{serial}",
                    std::process::id(),
                    hash.finish(),
                ));
                match DirBuilder::new().mode(0o700).create(&root) {
                    Ok(()) => return Self { parent, root },
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(error) => panic!("cannot create private preview fixture: {error}"),
                }
            }
        }
        fn path(&self, name: &str) -> PathBuf {
            assert!(!name.is_empty() && !name.contains('/') && name != "." && name != "..");
            self.root.join(name)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.path(name);
            fs::write(&path, bytes).unwrap();
            path
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            // Verify the exact owned, privately-created target before recursive cleanup.
            assert!(self.root.is_absolute());
            assert_eq!(self.root.parent(), Some(self.parent.as_path()));
            assert!(
                self.root
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with(TEMP_PREFIX)
            );
            let metadata = fs::symlink_metadata(&self.root).unwrap();
            assert!(metadata.is_dir() && !metadata.file_type().is_symlink());
            assert_eq!(self.root.canonicalize().unwrap(), self.root);
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    fn png_fixture(
        width: u32,
        height: u32,
        color: png::ColorType,
        depth: png::BitDepth,
        data: &[u8],
        palette: Option<&[u8]>,
        transparency: Option<&[u8]>,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut bytes, width, height);
            encoder.set_color(color);
            encoder.set_depth(depth);
            if let Some(palette) = palette {
                encoder.set_palette(palette.to_vec());
            }
            if let Some(transparency) = transparency {
                encoder.set_trns(transparency.to_vec());
            }
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(data).unwrap();
        }
        bytes
    }

    fn ready(outcome: Outcome) -> (u32, u32, Vec<u8>) {
        match outcome {
            Outcome::Ready {
                width,
                height,
                rgba,
            } => {
                assert!(width > 0 && width <= THUMB_WIDTH);
                assert!(height > 0 && height <= THUMB_HEIGHT);
                assert_eq!(rgba.len(), (width * height * 4) as usize);
                (width, height, rgba)
            }
            other => panic!("expected ready, got {other:?}"),
        }
    }

    #[test]
    fn palette_and_transparency_expand_to_rgba() {
        let bytes = png_fixture(
            2,
            1,
            png::ColorType::Indexed,
            png::BitDepth::Eight,
            &[0, 1],
            Some(&[255, 0, 0, 0, 255, 0]),
            Some(&[255, 64]),
        );
        assert_eq!(
            ready(decode(&bytes, || false)),
            (2, 1, vec![255, 0, 0, 255, 0, 255, 0, 64])
        );
    }

    #[test]
    fn low_bit_grayscale_and_gray_alpha_expand() {
        let bytes = png_fixture(
            2,
            1,
            png::ColorType::Grayscale,
            png::BitDepth::One,
            &[0b0100_0000],
            None,
            None,
        );
        assert_eq!(
            ready(decode(&bytes, || false)).2,
            vec![0, 0, 0, 255, 255, 255, 255, 255]
        );
        let bytes = png_fixture(
            1,
            1,
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            &[80, 40],
            None,
            None,
        );
        assert_eq!(ready(decode(&bytes, || false)).2, vec![80, 80, 80, 40]);
    }

    #[test]
    fn sixteen_bit_png_and_rgba_preserve_alpha() {
        let bytes = png_fixture(
            1,
            1,
            png::ColorType::Rgba,
            png::BitDepth::Sixteen,
            &[255, 255, 0, 0, 128, 128, 64, 64],
            None,
            None,
        );
        assert_eq!(ready(decode(&bytes, || false)).2, vec![255, 0, 128, 64]);
    }

    #[test]
    fn jpeg_is_supported_without_extension_trust() {
        let mut bytes = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut bytes, 95)
            .encode(
                &[255, 0, 0, 0, 255, 0],
                2,
                1,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        let (width, height, rgba) = ready(decode(&bytes, || false));
        assert_eq!((width, height), (2, 1));
        assert_eq!(rgba[3], 255);
        assert_eq!(rgba[7], 255);
    }

    #[test]
    fn thumbnail_is_bounded_without_upscaling() {
        let bytes = png_fixture(
            640,
            480,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &vec![100; 640 * 480],
            None,
            None,
        );
        assert_eq!(
            ready(decode(&bytes, || false)),
            (160, 120, vec![100, 100, 100, 255].repeat(160 * 120))
        );
        for (source, expected) in [
            ((1, 4096), (1, 120)),
            ((4096, 1), (160, 1)),
            ((320, 100), (160, 50)),
            ((100, 400), (30, 120)),
            ((7, 5), (7, 5)),
        ] {
            assert_eq!(thumbnail_dimensions(source.0, source.1), expected);
        }
    }

    #[test]
    fn malformed_and_unsupported_inputs_are_closed() {
        assert_eq!(decode(b"PUBLIC text", || false), Outcome::NotImage);
        assert_eq!(decode(b"GIF89a", || false), Outcome::NotImage);
        assert_eq!(decode(b"\xff\xd8", || false), Outcome::Rejected);
        assert_eq!(decode(b"\x89PNG\r\n\x1a\n", || false), Outcome::Rejected);
        let bytes = png_fixture(
            1,
            1,
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            &[1, 2, 3],
            None,
            None,
        );
        for cut in [8, 16, bytes.len() / 2] {
            assert_eq!(decode(&bytes[..cut], || false), Outcome::Rejected);
        }
        let mut bad_crc = bytes;
        bad_crc[29] ^= 1; // IHDR CRC, not dimension bytes.
        assert_eq!(decode(&bad_crc, || false), Outcome::Rejected);
    }

    #[test]
    fn compressed_dimension_and_pixel_limits_reject_before_pixel_decode() {
        assert_eq!(
            decode(&vec![0; MAX_COMPRESSED_BYTES + 1], || false),
            Outcome::Rejected
        );
        for (width, height) in [(4097, 1), (1, 4097), (2001, 2000)] {
            let bytes = png_fixture(
                width,
                height,
                png::ColorType::Grayscale,
                png::BitDepth::Eight,
                &vec![0; (width * height) as usize],
                None,
                None,
            );
            assert_eq!(decode(&bytes, || false), Outcome::Rejected);
        }
        let bytes = png_fixture(
            2000,
            2000,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &vec![0; 4_000_000],
            None,
            None,
        );
        assert_eq!(ready(decode(&bytes, || false)).0, 120);
    }

    #[test]
    fn cancellation_before_read_during_decode_and_during_thumbnail_is_sticky() {
        assert_eq!(
            load(PathBuf::from("/PUBLIC unavailable file"), || true),
            Outcome::Canceled
        );
        assert_eq!(load(PathBuf::from("relative"), || false), Outcome::Rejected);
        assert_eq!(decode(b"", || true), Outcome::Canceled);
        let bytes = png_fixture(
            2,
            1,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &[1, 2],
            None,
            None,
        );
        let calls = Cell::new(0);
        assert_eq!(
            decode(&bytes, || {
                calls.set(calls.get() + 1);
                calls.get() == 3
            }),
            Outcome::Canceled
        );
        let calls = Cell::new(0);
        let callback = || {
            calls.set(calls.get() + 1);
            calls.get() == 2
        };
        let cancel = Cancellation::new(&callback);
        let image = DynamicImage::ImageRgba8(image::RgbaImage::new(2, 2));
        assert_eq!(thumbnail(image, &cancel), Outcome::Canceled);
        assert!(cancel.check());
    }

    #[test]
    fn load_png_and_jpeg_match_byte_decode_and_shared_intake() {
        let temp = Temp::new();
        let png = png_fixture(
            2,
            1,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            &[10, 20],
            None,
            None,
        );
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 95)
            .encode(
                &[255, 0, 0, 0, 255, 0],
                2,
                1,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        // Misleading extensions intentionally exercise content detection, not path trust.
        for (name, bytes) in [("PUBLIC png.txt", png), ("PUBLIC jpeg.png", jpeg)] {
            let path = temp.file(name, &bytes);
            let expected = decode(&bytes, || false);
            assert!(matches!(expected, Outcome::Ready { .. }));
            assert_eq!(load(path.clone(), || false), expected);
            assert_eq!(
                SelectedFile::new(path.clone())
                    .unwrap()
                    .read_preview(|| false)
                    .unwrap(),
                bytes
            );
            assert_eq!(
                SelectedFile::new(path)
                    .unwrap()
                    .read(|| false)
                    .unwrap()
                    .len(),
                bytes.len()
            );
        }
        let text = temp.file("PUBLIC text.png", b"PUBLIC ordinary text");
        assert_eq!(load(text, || false), Outcome::NotImage);
    }

    #[test]
    fn preview_load_and_upload_intake_agree_on_forbidden_file_types_and_size() {
        let temp = Temp::new();
        let file = temp.file("PUBLIC target", b"PUBLIC fixture");
        let link = temp.path("PUBLIC final symlink");
        symlink(file, &link).unwrap();
        let fifo = temp.path("PUBLIC fifo");
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
        let directory = temp.path("PUBLIC directory");
        fs::create_dir(&directory).unwrap();
        let oversized = temp.path("PUBLIC oversized");
        File::create(&oversized)
            .unwrap()
            .set_len((MAX_COMPRESSED_BYTES + 1) as u64)
            .unwrap();
        let missing = temp.path("PUBLIC missing");
        for (path, failure) in [
            (link, Failure::Unavailable),
            (fifo, Failure::NotRegular),
            (directory, Failure::NotRegular),
            (oversized, Failure::TooLarge),
            (missing, Failure::Unavailable),
        ] {
            assert_eq!(load(path.clone(), || false), Outcome::Rejected);
            assert_eq!(
                SelectedFile::new(path.clone())
                    .unwrap()
                    .read_preview(|| false)
                    .unwrap_err(),
                failure
            );
            assert_eq!(
                SelectedFile::new(path).unwrap().read(|| false).unwrap_err(),
                failure
            );
        }
    }

    #[test]
    fn cancellation_precedes_selection_and_is_observed_by_local_reader() {
        let temp = Temp::new();
        let missing = temp.path("PUBLIC never created");
        let fifo = temp.path("PUBLIC canceled fifo");
        rustix::fs::mkfifoat(
            rustix::fs::CWD,
            &fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
        )
        .unwrap();
        // Invalid selection and nonexistent-path results establish that the initial
        // cancellation wins before validation/open; exact absence of syscalls is source-reviewed.
        for path in [PathBuf::from("relative"), missing.clone(), fifo] {
            let calls = Cell::new(0);
            assert_eq!(
                load(path, || {
                    calls.set(calls.get() + 1);
                    true
                }),
                Outcome::Canceled
            );
            assert_eq!(calls.get(), 1);
        }
        assert!(!missing.exists());
        let file = temp.file("PUBLIC canceled regular", &vec![0; READ_CHUNK * 2]);
        // load admission, pre-open, post-open, read admission, then first read-loop check.
        let calls = Cell::new(0);
        assert_eq!(
            load(file, || {
                calls.set(calls.get() + 1);
                calls.get() == 5
            }),
            Outcome::Canceled
        );
        assert_eq!(calls.get(), 5);
    }

    #[test]
    fn maximum_pixel_count_sixteen_bit_rgba_is_not_double_reserved() {
        // 4M pixels x 8 bytes = 32,000,000, below the 32MiB advertised-output budget.
        // Use compressible pixels so the file also fits the independent 4MiB intake cap.
        let bytes = png_fixture(
            2000,
            2000,
            png::ColorType::Rgba,
            png::BitDepth::Sixteen,
            &vec![0; 4_000_000 * 8],
            None,
            None,
        );
        assert!(bytes.len() <= MAX_COMPRESSED_BYTES);
        assert_eq!(
            ready(decode(&bytes, || false)),
            (120, 120, vec![0; 120 * 120 * 4])
        );
    }

    #[test]
    fn debug_redacts_pixel_contents() {
        let outcome = Outcome::Ready {
            width: 1,
            height: 1,
            rgba: b"PRIVATE".to_vec(),
        };
        let debug = format!("{outcome:?}");
        assert_eq!(debug, "Ready { width: 1, height: 1, .. }");
        assert!(!debug.contains("PRIVATE"));
    }
}
