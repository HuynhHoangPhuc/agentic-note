//! Zstd compression/decompression wrappers for sync blob transfer.
//!
//! Provides simple compress/decompress functions wrapping the `zstd` crate.
//! Used during sync to reduce bandwidth for blob transfers.

use zenon_core::{AgenticError, Result};
use std::io::Cursor;

/// Compress data using zstd at the given compression level (1-22).
pub fn compress(data: &[u8], level: i32) -> Result<Vec<u8>> {
    let level = level.clamp(1, 22);
    zstd::encode_all(Cursor::new(data), level)
        .map_err(|e| AgenticError::Sync(format!("zstd compress failed: {e}")))
}

/// Decompress zstd-compressed data.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>> {
    zstd::decode_all(Cursor::new(data))
        .map_err(|e| AgenticError::Sync(format!("zstd decompress failed: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        let original = b"Hello, this is a test of zstd compression for sync blobs. \
                         It should compress well since markdown text is repetitive.";
        let compressed = compress(original, 3).expect("compress data");
        let decompressed = decompress(&compressed).expect("decompress data");
        assert_eq!(decompressed, original);
    }

    #[test]
    fn test_compress_empty_data() {
        let compressed = compress(b"", 3).expect("compress empty");
        let decompressed = decompress(&compressed).expect("decompress empty");
        assert!(decompressed.is_empty());
    }

    #[test]
    fn test_compression_reduces_size_for_text() {
        let text = "This is a markdown note with lots of repeated text. ".repeat(100);
        let compressed = compress(text.as_bytes(), 3).expect("compress text");
        assert!(
            compressed.len() < text.len(),
            "compressed ({}) should be smaller than original ({})",
            compressed.len(),
            text.len()
        );
    }

    #[test]
    fn test_compress_clamps_level() {
        // Level 0 should be clamped to 1, level 30 to 22
        let data = b"test data";
        let _ = compress(data, 0).expect("compress level low");
        let _ = compress(data, 30).expect("compress level high");
    }

    #[test]
    fn test_decompress_invalid_data_returns_error() {
        let result = decompress(b"not valid zstd data");
        assert!(result.is_err());
    }
}
