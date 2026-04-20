use crypto_bigint::U256;
use ed25519_dalek::VerifyingKey;

pub fn generate_entry_nonce(verifying_key: VerifyingKey, leading_zeroes: usize) -> U256 {
    unimplemented!("will be used for POW when i support it");
}

pub fn validate_entry_nonce() {
    unimplemented!("will be used for POW when i support it");
}