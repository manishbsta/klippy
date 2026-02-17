use sha2::{Digest, Sha256};

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn sha256_hex_bytes(input: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_deterministic() {
        let a = sha256_hex("hello");
        let b = sha256_hex("hello");
        let c = sha256_hex("world");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn byte_hash_is_deterministic() {
        let a = sha256_hex_bytes(b"hello");
        let b = sha256_hex_bytes(b"hello");
        let c = sha256_hex_bytes(b"world");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
