use sha2::{Digest, Sha256};

pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
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
}
