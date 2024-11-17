use aes_gcm::{
    aead::{rand_core::RngCore, Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};

const NONCE_SIZE: usize = 12;

pub fn get_key() -> [u8; 32] {
    let key = std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY not set");
    assert!(key.len() == 64, "ENCRYPTION_KEY must be 64 characters long");
    let mut key_bytes = [0; 32];
    hex::decode_to_slice(key, &mut key_bytes).expect("invalid ENCRYPTION_KEY");
    key_bytes
}

pub fn encrypt(key: &[u8; 32], plaintext: &str) -> (String, String) {
    let cipher = Aes256Gcm::new(key.into());
    let mut nonce_bytes = [0; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);

    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .expect("encryption failed");

    (hex::encode(ciphertext), hex::encode(nonce_bytes))
}

pub fn decrypt(key: &[u8; 32], ciphertext_hex: &str, nonce_hex: &str) -> String {
    let cipher = Aes256Gcm::new(key.into());

    let ciphertext = hex::decode(ciphertext_hex).expect("invalid ciphertext hex");
    let nonce_bytes = hex::decode(nonce_hex).expect("invalid nonce hex");
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext_bytes = cipher
        .decrypt(nonce, ciphertext.as_ref())
        .expect("decryption failure!");
    String::from_utf8(plaintext_bytes).expect("invalid UTF-8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption() {
        let key = [0; 32];
        let plaintext = "Hello, world!";
        let (ciphertext, nonce) = encrypt(&key, plaintext);

        let decrypted = decrypt(&key, &ciphertext, &nonce);
        assert_eq!(decrypted, plaintext);
    }
}
