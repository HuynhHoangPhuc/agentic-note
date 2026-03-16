use zenon_core::Result;
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

/// Hex-encoded SHA-256 digest used as content-addressable object identifier.
pub type ObjectId = String;

/// Hash arbitrary bytes with SHA-256, returning a lowercase hex string.
pub fn hash_bytes(data: &[u8]) -> ObjectId {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Hash a file by streaming its contents, returning a lowercase hex SHA-256.
pub fn hash_file(path: &Path) -> Result<ObjectId> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_bytes_hash_is_deterministic() {
        let a = hash_bytes(b"");
        let b = hash_bytes(b"");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
    }

    #[test]
    fn known_sha256() {
        // echo -n "hello" | sha256sum
        let h = hash_bytes(b"hello");
        assert_eq!(
            h,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn different_inputs_differ() {
        assert_ne!(hash_bytes(b"foo"), hash_bytes(b"bar"));
    }
}
