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

    // Empty string handling
    let empty_encrypted = encrypt_with_key(&key, "").expect("empty encryption should succeed");
    assert_eq!(empty_encrypted, "");
    let empty_decrypted = decrypt_with_key(&key, "").expect("empty decryption should succeed");
    assert_eq!(empty_decrypted, "");
}

