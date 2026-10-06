//! OS credential ownership. Secrets are not exposed through queries or events.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, Zeroizing};
const SERVICE: &str = "com.mikomai.native.v1";
#[cfg(target_os = "macos")]
pub fn update(id: &str, password: Option<&str>, enable: Option<&str>) -> Result<(), String> {
    let id = uuid::Uuid::parse_str(id)
        .map_err(|_| "invalid credential reference")?
        .to_string();
    for (kind, value) in [("password", password), ("enable", enable)] {
        if let Some(value) = value {
            let entry = keyring::Entry::new(SERVICE, &format!("{id}:{kind}"))
                .map_err(|_| "credential store unavailable")?;
            if value.is_empty() {
                match entry.delete_credential() {
                    Ok(()) | Err(keyring::Error::NoEntry) => {}
                    Err(_) => return Err("credential deletion failed".into()),
                }
            } else {
                entry
                    .set_password(value)
                    .map_err(|_| "credential save failed")?;
            }
        }
    }
    Ok(())
}
#[cfg(target_os = "macos")]
pub fn load(id: &str) -> Result<Value, String> {
    let id = uuid::Uuid::parse_str(id)
        .map_err(|_| "invalid credential reference")?
        .to_string();
    let get = |kind: &str| -> Result<String, String> {
        match keyring::Entry::new(SERVICE, &format!("{id}:{kind}"))
            .map_err(|_| "credential store unavailable")?
            .get_password()
        {
            Ok(value) => Ok(value),
            Err(keyring::Error::NoEntry) => Ok(String::new()),
            Err(_) => Err("credential read failed".into()),
        }
    };
    Ok(json!({"password":get("password")?,"enablePassword":get("enable")?}))
}
pub fn fingerprint(id: &str) -> Result<String, String> {
    let mut secrets = load(id)?;
    let material = Zeroizing::new(format!(
        "{}\0{}",
        secrets["password"].as_str().unwrap_or(""),
        secrets["enablePassword"].as_str().unwrap_or("")
    ));
    let fingerprint = format!("{:x}", Sha256::digest(material.as_bytes()));
    wipe(&mut secrets);
    Ok(fingerprint)
}

#[cfg(not(target_os = "macos"))]
pub fn update(_: &str, _: Option<&str>, _: Option<&str>) -> Result<(), String> {
    Err("OS credential provider is not implemented on this platform".into())
}
#[cfg(not(target_os = "macos"))]
pub fn load(_: &str) -> Result<Value, String> {
    Err("OS credential provider is not implemented on this platform".into())
}

/// Best-effort erasure of transient secret-bearing JSON buffers.
pub fn wipe(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Object(values) => values.values_mut().for_each(wipe),
        Value::Array(values) => values.iter_mut().for_each(wipe),
        _ => {}
    }
}
