use sha2::{Digest, Sha256};

/// Generis SHA256 hash function.
pub fn hash(data: impl AsRef<[u8]>) -> [u8; 32] {
    let mut hasher = Sha256::new();

    // write input message
    hasher.update(data);

    // read hash digest and consume hasher
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hex_literal::hex;

    const MESSAGE: &[u8] = "hello world".as_bytes();

    #[test]
    fn test_hashing() {
        assert_eq!(
            hash(MESSAGE),
            hex!("b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9").as_slice()
        );
    }
}
