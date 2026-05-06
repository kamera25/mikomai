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

pub fn encrypt(app: &tauri::AppHandle, data: &str) -> Result<String, String> {
    if data.is_empty() {
        return Ok("".to_string());
    }

    let key = get_or_create_key(app)?;
    let cipher = Aes256Gcm::new(&key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message

    let ciphertext = cipher.encrypt(&nonce, data.as_bytes())
        .map_err(|e| format!("Encryption failed: {:?}", e))?;

    let mut result = nonce.to_vec();
    result.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(result))
}

pub fn decrypt(app: &tauri::AppHandle, encrypted_data: &str) -> Result<String, String> {
    if encrypted_data.is_empty() {
        return Ok("".to_string());
    }

    let key = get_or_create_key(app)?;
    let cipher = Aes256Gcm::new(&key);

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
