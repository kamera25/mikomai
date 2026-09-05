pub use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};

const KEY_STORE_FILE: &str = "key.bin";

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Keyring use_native_store failed: {0}")]
    KeyringStoreInit(String),
    #[error("Keyring entry creation failed: {0}")]
    KeyringEntry(String),
    #[error("Keyring get_password failed: {0}")]
    KeyringGet(String),
    #[error("Keyring set_password failed: {0}")]
    KeyringSet(String),
    #[error("Base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("Invalid key length in keyring")]
    InvalidKeyringLength,
    #[error("Invalid key length in fallback file")]
    InvalidFileLength,
    #[error("File I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Encryption failed: {0}")]
    Encryption(String),
    #[error("Decryption failed: {0}")]
    Decryption(String),
    #[error("Encrypted data is too short")]
    TooShort,
    #[error("Invalid UTF-8 data: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

fn get_key_from_keyring() -> Result<Option<Key<Aes256Gcm>>, CryptoError> {
    keyring::use_native_store(false).map_err(|e| CryptoError::KeyringStoreInit(e.to_string()))?;
    let entry = keyring_core::Entry::new("com.mikomai.agent", "crypto-key")
        .map_err(|e| CryptoError::KeyringEntry(e.to_string()))?;
    match entry.get_password() {
        Ok(password_str) => {
            let key_bytes = STANDARD.decode(password_str)?;
            if key_bytes.len() != 32 {
                return Err(CryptoError::InvalidKeyringLength);
            }
            Ok(Some(*Key::<Aes256Gcm>::from_slice(&key_bytes)))
        }
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(e) => Err(CryptoError::KeyringGet(e.to_string())),
    }
}

fn save_key_to_keyring(key: &Key<Aes256Gcm>) -> Result<(), CryptoError> {
    keyring::use_native_store(false).map_err(|e| CryptoError::KeyringStoreInit(e.to_string()))?;
    let entry = keyring_core::Entry::new("com.mikomai.agent", "crypto-key")
        .map_err(|e| CryptoError::KeyringEntry(e.to_string()))?;
    let encoded = STANDARD.encode(key.as_slice());
    entry
        .set_password(&encoded)
        .map_err(|e| CryptoError::KeyringSet(e.to_string()))?;
    Ok(())
}

fn save_key_to_file(key_path: &std::path::Path, key: &Key<Aes256Gcm>) -> Result<(), CryptoError> {
    std::fs::write(key_path, key.as_slice())?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(key_path)?.permissions();
        perms.set_mode(0o400); // Read-only for user
        std::fs::set_permissions(key_path, perms)?;
    }
    Ok(())
}

use std::sync::Mutex;
use tauri::Emitter;

static IN_MEMORY_KEY: Mutex<Option<Key<Aes256Gcm>>> = Mutex::new(None);

struct KeyringEventGuard<'a, R: tauri::Runtime> {
    app: &'a tauri::AppHandle<R>,
}

impl<'a, R: tauri::Runtime> KeyringEventGuard<'a, R> {
    fn new(app: &'a tauri::AppHandle<R>) -> Self {
        let _ = app.emit("keyring-access-start", ());
        std::thread::sleep(std::time::Duration::from_millis(50));
        Self { app }
    }
}

impl<'a, R: tauri::Runtime> Drop for KeyringEventGuard<'a, R> {
    fn drop(&mut self) {
        let _ = self.app.emit("keyring-access-end", ());
    }
}

fn get_or_create_key_internal<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Key<Aes256Gcm>, CryptoError> {
    let path = tauri::Manager::path(app)
        .app_data_dir()
        .expect("Failed to get app data dir");
    if !path.exists() {
        let _ = std::fs::create_dir_all(&path);
    }
    let key_path = path.join(KEY_STORE_FILE);

    // Try to get key from keyring first
    match get_key_from_keyring() {
        Ok(Some(key)) => Ok(key),
        Ok(None) => {
            // Key not in keyring. Check if we have a fallback file key.bin
            if key_path.exists() {
                if let Ok(key_bytes) = std::fs::read(&key_path) {
                    if key_bytes.len() == 32 {
                        let key = *Key::<Aes256Gcm>::from_slice(&key_bytes);
                        // Try to migrate this key to the keyring so future calls use keyring
                        if let Err(e) = save_key_to_keyring(&key) {
                            log::warn!("Failed to migrate key to keyring: {}", e);
                        }
                        return Ok(key);
                    }
                }
            }
            // Neither exists. Generate a new key and try to store in keyring
            let new_key = Aes256Gcm::generate_key(OsRng);
            match save_key_to_keyring(&new_key) {
                Ok(_) => Ok(new_key),
                Err(e) => {
                    log::warn!(
                        "Failed to save new key to keyring, falling back to file: {}",
                        e
                    );
                    // Fallback to saving to file
                    save_key_to_file(&key_path, &new_key)?;
                    Ok(new_key)
                }
            }
        }
        Err(e) => {
            log::warn!(
                "Keyring failed or not available, falling back to file: {}",
                e
            );
            // Keyring failed (e.g. not supported, locked). Fallback to file-based storage.
            if key_path.exists() {
                let key_bytes = std::fs::read(&key_path)?;
                if key_bytes.len() != 32 {
                    return Err(CryptoError::InvalidFileLength);
                }
                Ok(*Key::<Aes256Gcm>::from_slice(&key_bytes))
            } else {
                let new_key = Aes256Gcm::generate_key(OsRng);
                save_key_to_file(&key_path, &new_key)?;
                Ok(new_key)
            }
        }
    }
}

pub fn get_or_create_key<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<Key<Aes256Gcm>, CryptoError> {
    let mut lock = IN_MEMORY_KEY.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(key) = *lock {
        return Ok(key);
    }

    let _guard = KeyringEventGuard::new(app);
    let key = get_or_create_key_internal(app)?;
    *lock = Some(key);
    Ok(key)
}

#[cfg(test)]
#[allow(dead_code)]
pub fn clear_key_cache_for_testing() {
    let mut lock = IN_MEMORY_KEY.lock().unwrap_or_else(|e| e.into_inner());
    *lock = None;
}

pub fn generate_key() -> Key<Aes256Gcm> {
    Aes256Gcm::generate_key(OsRng)
}

pub fn encrypt_with_key(key: &Key<Aes256Gcm>, data: &str) -> Result<String, CryptoError> {
    if data.is_empty() {
        return Ok("".to_string());
    }

    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message

    let ciphertext = cipher
        .encrypt(&nonce, data.as_bytes())
        .map_err(|e| CryptoError::Encryption(format!("{:?}", e)))?;

    let mut result = nonce.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(result))
}

pub fn decrypt_with_key(key: &Key<Aes256Gcm>, encrypted_data: &str) -> Result<String, CryptoError> {
    if encrypted_data.is_empty() {
        return Ok("".to_string());
    }

    let cipher = Aes256Gcm::new(key);

    let decoded = STANDARD.decode(encrypted_data)?;

    if decoded.len() < 12 {
        return Err(CryptoError::TooShort);
    }

    let (nonce_bytes, ciphertext) = decoded.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| CryptoError::Decryption(format!("{:?}", e)))?;

    Ok(String::from_utf8(plaintext)?)
}

pub fn encrypt<R: tauri::Runtime>(app: &tauri::AppHandle<R>, data: &str) -> Result<String, CryptoError> {
    if data.is_empty() {
        return Ok("".to_string());
    }

    let key = get_or_create_key(app)?;
    encrypt_with_key(&key, data)
}

pub fn decrypt<R: tauri::Runtime>(app: &tauri::AppHandle<R>, encrypted_data: &str) -> Result<String, CryptoError> {
    if encrypted_data.is_empty() {
        return Ok("".to_string());
    }

    let key = get_or_create_key(app)?;
    decrypt_with_key(&key, encrypted_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes_gcm::aead::OsRng;
    use aes_gcm::Aes256Gcm;

    fn get_test_key() -> Key<Aes256Gcm> {
        Aes256Gcm::generate_key(OsRng)
    }

    #[test]
    fn test_encrypt_decrypt() {
        let key = get_test_key();
        let plain_text = "Hello, World! This is a secret message.";

        let encrypted = encrypt_with_key(&key, plain_text).unwrap();
        let decrypted = decrypt_with_key(&key, &encrypted).unwrap();

        assert_eq!(plain_text, decrypted);
    }

    #[test]
    fn test_decrypt_empty_string() {
        let key = get_test_key();
        let result = decrypt_with_key(&key, "");
        assert_eq!(result.unwrap(), "".to_string());
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        let key = get_test_key();
        let result = decrypt_with_key(&key, "invalid_base64!!!");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CryptoError::Base64(_)));
    }

    #[test]
    fn test_decrypt_too_short() {
        let key = get_test_key();
        // A valid base64 string that decodes to less than 12 bytes
        let short_data = base64::engine::general_purpose::STANDARD.encode(b"short");
        let result = decrypt_with_key(&key, &short_data);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CryptoError::TooShort));
    }

    #[test]
    fn test_decrypt_invalid_ciphertext() {
        let key = get_test_key();
        // Create valid base64 but invalid ciphertext/nonce combo
        let mut invalid_data = vec![0u8; 32]; // 12 bytes nonce + 20 bytes random data
        invalid_data[0] = 1; // Just to have some data
        let encoded = base64::engine::general_purpose::STANDARD.encode(invalid_data);

        let result = decrypt_with_key(&key, &encoded);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CryptoError::Decryption(_)));
    }
}
