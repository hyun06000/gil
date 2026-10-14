//! Bounded static PNG validation. No source paths, URLs, scope authority or HTML execution.
//! Original encoded bytes are retained; decoding is validation, not recompression.
use super::registry::{ImageKind, NewVisualAsset, SourceKind};
use crate::artifact::{ObjectStore, digest_from_raw};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read, Write};
use std::num::{NonZeroU32, NonZeroU64};

pub const MAX_ENCODED_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DIMENSION: u32 = 8192;
pub const MAX_PIXELS: u64 = 16 * 1024 * 1024;
const MAX_DECODED_BYTES: usize = 128 * 1024 * 1024;
const MAX_DECODER_BYTES: usize = 32 * 1024 * 1024;
const MAX_CHUNKS: usize = 4096;

/// Explicit Project-relative path and author-supplied provenance; never inferred from Report text.
pub struct VisualAttachment {
    pub path: String,
    pub source_kind: SourceKind,
    pub source_description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    TooLarge,
    Unsupported,
    Animated,
    Malformed,
    Dimensions,
    Allocation,
    Read,
    Object,
    MetadataMismatch,
    BlankSource,
}
impl std::fmt::Display for ImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::TooLarge => "시각 자료의 용량 한도를 넘었다",
            Self::Unsupported => "현재 정적 PNG만 시각 자료로 받을 수 있다",
            Self::Animated => "움직이는 PNG는 아직 지원하지 않는다",
            Self::Malformed => "PNG 전체를 정상적으로 디코딩하지 못했다",
            Self::Dimensions => "이미지 크기 또는 픽셀 수가 허용 범위 밖이다",
            Self::Allocation => "이미지 검증 메모리를 확보하지 못했다",
            Self::Read => "시각 자료 바이트를 읽지 못했다",
            Self::Object => "보존된 시각 자료 객체를 쓰거나 검증하지 못했다",
            Self::MetadataMismatch => "시각 자료의 기록과 실제 바이트가 다르다",
            Self::BlankSource => "시각 자료의 출처 설명이 비어 있다",
        })
    }
}
impl std::error::Error for ImageError {}

/// Only full, bounded decoding can construct this proof. Metadata is not caller supplied.
pub struct VerifiedImage {
    bytes: Vec<u8>,
    digest: [u8; 32],
    width: NonZeroU32,
    height: NonZeroU32,
}
impl std::fmt::Debug for VerifiedImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifiedImage")
            .field("byte_len", &self.bytes.len())
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}
impl VerifiedImage {
    /// Reads at most the encoded limit plus one byte. The caller owns path/permission checks.
    pub fn read(reader: impl Read) -> Result<Self, ImageError> {
        let mut bytes = Vec::new();
        reader
            .take(MAX_ENCODED_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| ImageError::Read)?;
        Self::decode(bytes)
    }

    pub fn decode(bytes: Vec<u8>) -> Result<Self, ImageError> {
        if bytes.len() > MAX_ENCODED_BYTES {
            return Err(ImageError::TooLarge);
        }
        check_envelope(&bytes)?;
        let mut options = png::DecodeOptions::default();
        options.set_ignore_checksums(false);
        options.set_skip_ancillary_crc_failures(false);
        let mut decoder = png::Decoder::new_with_options(Cursor::new(&bytes), options);
        decoder.set_limits(png::Limits {
            bytes: MAX_DECODER_BYTES,
        });
        let info = decoder
            .read_header_info()
            .map_err(|_| ImageError::Malformed)?;
        let (width, height) = (info.width, info.height);
        if width == 0
            || height == 0
            || width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || u64::from(width) * u64::from(height) > MAX_PIXELS
        {
            return Err(ImageError::Dimensions);
        }
        let mut reader = decoder.read_info().map_err(|_| ImageError::Malformed)?;
        if reader.info().animation_control.is_some() {
            return Err(ImageError::Animated);
        }
        let size = reader.output_buffer_size().ok_or(ImageError::Dimensions)?;
        if size > MAX_DECODED_BYTES {
            return Err(ImageError::Dimensions);
        }
        let mut decoded = Vec::new();
        decoded
            .try_reserve_exact(size)
            .map_err(|_| ImageError::Allocation)?;
        decoded.resize(size, 0);
        reader
            .next_frame(&mut decoded)
            .map_err(|_| ImageError::Malformed)?;
        reader.finish().map_err(|_| ImageError::Malformed)?;
        Ok(Self {
            digest: Sha256::digest(&bytes).into(),
            bytes,
            width: NonZeroU32::new(width).ok_or(ImageError::Dimensions)?,
            height: NonZeroU32::new(height).ok_or(ImageError::Dimensions)?,
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn metadata(
        &self,
        source_kind: SourceKind,
        source_description: String,
    ) -> Result<NewVisualAsset, ImageError> {
        if source_description.trim().is_empty() {
            return Err(ImageError::BlankSource);
        }
        Ok(NewVisualAsset {
            digest: self.digest,
            image_kind: ImageKind::Png,
            byte_len: NonZeroU64::new(self.bytes.len() as u64).expect("PNG is nonempty"),
            width: self.width,
            height: self.height,
            source_kind,
            source_description,
        })
    }
}

// Bound chunk traversal, require an actual final IEND, and refuse APNG before decoding pixels.
// CRC and compression validation belong to the PNG decoder, not this envelope scanner.
fn check_envelope(bytes: &[u8]) -> Result<(), ImageError> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(ImageError::Unsupported);
    }
    let mut offset = 8usize;
    for _ in 0..MAX_CHUNKS {
        let header = bytes.get(offset..offset + 8).ok_or(ImageError::Malformed)?;
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let kind = &header[4..8];
        let end = offset
            .checked_add(12)
            .and_then(|n| n.checked_add(length))
            .ok_or(ImageError::Malformed)?;
        if end > bytes.len() {
            return Err(ImageError::Malformed);
        }
        if matches!(kind, b"acTL" | b"fcTL" | b"fdAT") {
            return Err(ImageError::Animated);
        }
        if kind == b"IEND" {
            return if length == 0 && end == bytes.len() {
                Ok(())
            } else {
                Err(ImageError::Malformed)
            };
        }
        offset = end;
    }
    Err(ImageError::TooLarge)
}

/// Requires the Session lock. Unix ObjectStore uses descriptor-relative no-follow I/O;
/// this wrapper does not secure Session opening, recovery or all same-user concurrent mutation.
/// Not a public path-based image attachment endpoint. No new object namespace or GC is introduced.
#[allow(dead_code)]
impl ObjectStore {
    pub(crate) fn put_visual(&self, image: &VerifiedImage) -> Result<(), ImageError> {
        let digest = self
            .put_blob(image.bytes())
            .map_err(|_| ImageError::Object)?;
        if digest != digest_from_raw(image.digest) {
            return Err(ImageError::MetadataMismatch);
        }
        let mut output = LimitedBytes(Vec::new());
        self.read_blob(&digest, &mut output)
            .map_err(|_| ImageError::Object)?;
        if output.0 != image.bytes {
            return Err(ImageError::MetadataMismatch);
        }
        Ok(())
    }

    pub(crate) fn read_visual(
        &self,
        metadata: &NewVisualAsset,
    ) -> Result<VerifiedImage, ImageError> {
        if metadata.image_kind != ImageKind::Png {
            return Err(ImageError::Unsupported);
        }
        if metadata.byte_len.get() > MAX_ENCODED_BYTES as u64 {
            return Err(ImageError::TooLarge);
        }
        let mut output = LimitedBytes(Vec::new());
        self.read_blob(&digest_from_raw(metadata.digest), &mut output)
            .map_err(|_| ImageError::Object)?;
        let image = VerifiedImage::decode(output.0)?;
        if image.metadata(metadata.source_kind, metadata.source_description.clone())? != *metadata {
            return Err(ImageError::MetadataMismatch);
        }
        Ok(image)
    }
}
struct LimitedBytes(Vec<u8>);
impl Write for LimitedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > MAX_ENCODED_BYTES - self.0.len() {
            return Err(std::io::Error::other("visual object exceeds limit"));
        }
        self.0
            .try_reserve(bytes.len())
            .map_err(std::io::Error::other)?;
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn png_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255, 0, 0, 255, 0, 0, 255, 255])
            .unwrap();
        bytes
    }
    fn metadata(image: &VerifiedImage) -> NewVisualAsset {
        image
            .metadata(SourceKind::Generated, "합성 시험 그림".into())
            .unwrap()
    }
    fn temp() -> std::path::PathBuf {
        let root = std::env::temp_dir().join(format!("gil-visual-image-{}", uuid_free_id()));
        fs::create_dir(&root).unwrap();
        root
    }
    fn uuid_free_id() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        format!(
            "{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )
    }

    #[test]
    fn complete_decode_preserves_exact_original_bytes_and_derives_metadata() {
        let bytes = png_bytes();
        let image = VerifiedImage::read(&bytes[..]).unwrap();
        assert_eq!(image.bytes(), bytes);
        let meta = metadata(&image);
        assert_eq!((meta.width.get(), meta.height.get()), (2, 1));
        assert_eq!(meta.digest, <[u8; 32]>::from(Sha256::digest(&bytes)));
        assert!(image.metadata(SourceKind::Captured, " ".into()).is_err());
    }
    #[test]
    fn truncated_crc_corrupt_and_trailing_data_are_refused() {
        let bytes = png_bytes();
        for length in 0..bytes.len() {
            assert!(
                VerifiedImage::decode(bytes[..length].to_vec()).is_err(),
                "{length}"
            );
        }
        let mut bad = bytes.clone();
        bad[29] ^= 1;
        assert!(VerifiedImage::decode(bad).is_err());
        let mut bad = bytes.clone();
        bad.extend_from_slice(b"<script>bad</script>");
        assert!(VerifiedImage::decode(bad).is_err());
        let mut bad = bytes;
        let last = bad.len() - 1;
        bad[last] ^= 1;
        assert!(VerifiedImage::decode(bad).is_err());
    }
    #[test]
    fn unsupported_formats_and_animation_are_not_silently_converted() {
        for bytes in [
            b"<svg/>".as_slice(),
            b"\xff\xd8\xff\xd9",
            b"GIF89a",
            b"<html/>",
        ] {
            assert_eq!(
                VerifiedImage::decode(bytes.to_vec()).unwrap_err(),
                ImageError::Unsupported
            );
        }
        let mut bytes = png_bytes();
        // Envelope detection must refuse animation even before chunk CRC is considered.
        bytes.splice(33..33, [0, 0, 0, 0, b'a', b'c', b'T', b'L', 0, 0, 0, 0]);
        assert_eq!(
            VerifiedImage::decode(bytes).unwrap_err(),
            ImageError::Animated
        );
    }
    #[test]
    fn encoded_input_is_bounded_even_for_an_infinite_reader() {
        assert_eq!(
            VerifiedImage::read(std::io::repeat(0)).unwrap_err(),
            ImageError::TooLarge
        );
    }
    #[test]
    fn absurd_dimensions_are_refused_before_pixels_are_allocated() {
        let mut bytes = Vec::new();
        let writer = png::Encoder::new(&mut bytes, MAX_DIMENSION + 1, 1)
            .write_header()
            .unwrap();
        drop(writer);
        // Keep only signature + IHDR; writer drop may already append IEND.
        bytes.truncate(33);
        // Header-only image needs final IEND to reach the dimension preflight.
        bytes.extend_from_slice(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 174, 66, 96, 130]);
        assert_eq!(
            VerifiedImage::decode(bytes).unwrap_err(),
            ImageError::Dimensions
        );
    }
    #[test]
    fn stored_bytes_survive_source_replacement_deletion_and_store_reopen() {
        let root = temp();
        let source = root.join("screen.png");
        let original = png_bytes();
        fs::write(&source, &original).unwrap();
        let image = VerifiedImage::read(fs::File::open(&source).unwrap()).unwrap();
        let meta = metadata(&image);
        let store = ObjectStore::at(&root.join(".gil"));
        store.put_visual(&image).unwrap();
        store.put_visual(&image).unwrap();
        fs::write(&source, b"replacement").unwrap();
        fs::remove_file(&source).unwrap();
        let reloaded = ObjectStore::at(&root.join(".gil"))
            .read_visual(&meta)
            .unwrap();
        assert_eq!(reloaded.bytes(), original);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn missing_corrupt_and_mismatched_objects_never_become_a_successful_image() {
        let root = temp();
        let store = ObjectStore::at(&root.join(".gil"));
        let image = VerifiedImage::decode(png_bytes()).unwrap();
        let meta = metadata(&image);
        assert!(store.read_visual(&meta).is_err());
        store.put_visual(&image).unwrap();
        let mut wrong = meta.clone();
        wrong.width = NonZeroU32::new(3).unwrap();
        assert_eq!(
            store.read_visual(&wrong).unwrap_err(),
            ImageError::MetadataMismatch
        );
        let hex: String = meta
            .digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let path = store
            .root()
            .join("blobs/sha256")
            .join(&hex[..2])
            .join(&hex[2..]);
        fs::write(&path, b"corrupt").unwrap();
        assert!(store.read_visual(&meta).is_err());
        assert!(store.put_visual(&image).is_err());
        assert_eq!(
            fs::read(&path).unwrap(),
            b"corrupt",
            "do not silently repair immutable objects"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn object_read_rejects_growth_beyond_limit_without_unbounded_output() {
        let mut output = LimitedBytes(Vec::new());
        assert!(output.write_all(&vec![0; MAX_ENCODED_BYTES]).is_ok());
        assert!(output.write_all(&[0]).is_err());
        assert_eq!(output.0.len(), MAX_ENCODED_BYTES);
    }

    #[test]
    fn every_single_byte_corruption_of_fixture_is_detected() {
        let original = png_bytes();
        for index in 0..original.len() {
            let mut bytes = original.clone();
            bytes[index] ^= 1;
            assert!(VerifiedImage::decode(bytes).is_err(), "byte {index}");
        }
    }

    #[test]
    fn pixel_count_limit_is_separate_from_dimension_limit() {
        let mut bytes = Vec::new();
        drop(
            png::Encoder::new(&mut bytes, 8192, 4096)
                .write_header()
                .unwrap(),
        );
        bytes.truncate(33);
        bytes.extend_from_slice(&[0, 0, 0, 0, b'I', b'E', b'N', b'D', 174, 66, 96, 130]);
        assert_eq!(
            VerifiedImage::decode(bytes).unwrap_err(),
            ImageError::Dimensions
        );
    }

    #[test]
    fn chunk_count_is_bounded_even_when_each_chunk_is_tiny() {
        let mut bytes = png_bytes()[..33].to_vec();
        for _ in 0..MAX_CHUNKS {
            bytes.extend_from_slice(&[0, 0, 0, 0, b't', b'E', b'X', b't', 0, 0, 0, 0]);
        }
        assert_eq!(
            VerifiedImage::decode(bytes).unwrap_err(),
            ImageError::TooLarge
        );
    }

    #[test]
    fn caller_read_error_never_returns_a_partial_image() {
        struct Failing;
        impl Read for Failing {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("synthetic read failure"))
            }
        }
        assert_eq!(VerifiedImage::read(Failing).unwrap_err(), ImageError::Read);
    }

    #[test]
    fn ancillary_chunk_checksum_is_not_silently_ignored() {
        let mut bytes = Vec::new();
        let mut encoder = png::Encoder::new(&mut bytes, 1, 1);
        encoder
            .add_text_chunk("caption".into(), "synthetic".into())
            .unwrap();
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[127])
            .unwrap();
        assert!(VerifiedImage::decode(bytes.clone()).is_ok());
        let at = bytes.windows(4).position(|v| v == b"tEXt").unwrap();
        bytes[at + 4] ^= 1;
        assert!(VerifiedImage::decode(bytes).is_err());
    }
}
