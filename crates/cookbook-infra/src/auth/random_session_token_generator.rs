use base64::{Engine as _, engine::general_purpose::URL_SAFE};
use sha2::{Digest, Sha256};

use crate::auth::{
    models::{GeneratedSessionToken, SessionTokenHash},
    ports::SessionTokenGenerator,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct RandomSessionTokenGenerator;

impl RandomSessionTokenGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl SessionTokenGenerator for RandomSessionTokenGenerator {
    fn generate(&self) -> GeneratedSessionToken {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).expect("the session token randomness source failed");

        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let digest = hasher.finalize();

        GeneratedSessionToken::new(URL_SAFE.encode(bytes), SessionTokenHash::new(digest.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_distinct_tokens_with_matching_hashes() {
        let generator = RandomSessionTokenGenerator::new();

        let first = generator.generate();
        let second = generator.generate();

        assert_ne!(first.token(), second.token());

        let bytes = URL_SAFE.decode(first.token()).unwrap();
        assert_eq!(bytes.len(), 32);

        let digest = Sha256::digest(&bytes);
        assert_eq!(first.token_hash().as_bytes(), &digest.0);
    }
}
