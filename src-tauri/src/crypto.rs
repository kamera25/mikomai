use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce, Key
};
use base64::{engine::general_purpose::STANDARD, Engine};

const KEY_STORE_FILE: &str = "key.bin";

pub fn get_or_create_key(app: &tauri::AppHandle) -> Result<Key<Aes256Gcm>, String> {
    let path = tauri::Manager::path(app).app_data_dir().expect("Failed to get app data dir");
    if !path.exists() {
        let _ = std::fs::create_dir_all(&path);
    }
    let key_path = path.join(KEY_STORE_FILE);

    if key_path.exists() {
        let key_bytes = std::fs::read(&key_path).map_err(|e| e.to_string())?;
        if key_bytes.len() != 32 {
            return Err("Invalid key length".to_string());
        }
        Ok(*Key::<Aes256Gcm>::from_slice(&key_bytes))
    } else {
        let key = Aes256Gcm::generate_key(OsRng);
        std::fs::write(&key_path, key.as_slice()).map_err(|e| e.to_string())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&key_path).map_err(|e| e.to_string())?.permissions();
            perms.set_mode(0o400); // Read-only for user
            std::fs::set_permissions(&key_path, perms).map_err(|e| e.to_string())?;
        }

        Ok(key)
    }
}

pub fn encrypt_with_key(key: &Key<Aes256Gcm>, data: &str) -> Result<String, String> {
    if data.is_empty() {
        return Ok("".to_string());
    }

    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message

    let ciphertext = cipher.encrypt(&nonce, data.as_bytes())
        .map_err(|e| format!("Encryption failed: {:?}", e))?;

    let mut result = nonce.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(result))
}

pub fn decrypt_with_key(key: &Key<Aes256Gcm>, encrypted_data: &str) -> Result<String, String> {
    if encrypted_data.is_empty() {
        return Ok("".to_string());
    }

    let cipher = Aes256Gcm::new(key);

    let decoded = STANDARD.decode(encrypted_data)
        .map_err(|e| format!("Base64 decoding failed: {:?}", e))?;

    if decoded.len() < 12 {
        return Err("Encrypted data is too short".to_string());
    }

    let (nonce_bytes, ciphertext) = decoded.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher.decrypt(nonce, ciphertext)
        .map_err(|e| format!("Decryption failed: {:?}", e))?;

    String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8: {:?}", e))
}

pub fn encrypt(app: &tauri::AppHandle, data: &str) -> Result<String, String> {
    if data.is_empty() {
        return Ok("".to_string());
    }

    let key = get_or_create_key(app)?;
    encrypt_with_key(&key, data)
}

pub fn decrypt(app: &tauri::AppHandle, encrypted_data: &str) -> Result<String, String> {
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
        assert_eq!(result, Ok("".to_string()));
    }

    #[test]
    fn test_decrypt_invalid_base64() {
        let key = get_test_key();
        let result = decrypt_with_key(&key, "invalid_base64!!!");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Base64 decoding failed"));
    }

    #[test]
    fn test_decrypt_too_short() {
        let key = get_test_key();
        // A valid base64 string that decodes to less than 12 bytes
        let short_data = base64::engine::general_purpose::STANDARD.encode(b"short");
        let result = decrypt_with_key(&key, &short_data);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Encrypted data is too short");
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
        assert!(result.unwrap_err().contains("Decryption failed"));
    }
}
