use mikomai_lib::crypto::{decrypt_with_key, encrypt_with_key, generate_key};

#[test]
fn test_crypto_roundtrip() {
    let key = generate_key();
    let secret = "cisco_enable_secret_password_12345!";

    let encrypted = encrypt_with_key(&key, secret).expect("encryption should succeed");
    assert_ne!(secret, encrypted);
    assert!(!encrypted.is_empty());

    let decrypted = decrypt_with_key(&key, &encrypted).expect("decryption should succeed");
    assert_eq!(secret, decrypted);

    // Empty string handling (VULN-014: empty string is encrypted properly with AES-GCM)
    let empty_encrypted = encrypt_with_key(&key, "").expect("empty encryption should succeed");
    assert!(!empty_encrypted.is_empty(), "Empty string should produce valid ciphertext");
    let empty_decrypted = decrypt_with_key(&key, &empty_encrypted).expect("empty decryption should succeed");
    assert_eq!(empty_decrypted, "");

    // Legacy backward compatibility: unencrypted empty string
    let legacy_decrypted = decrypt_with_key(&key, "").expect("legacy empty decryption should succeed");
    assert_eq!(legacy_decrypted, "");
}
