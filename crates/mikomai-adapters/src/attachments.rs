//! Versioned native attachment wire format. Image policy belongs to Core.
use base64::{engine::general_purpose::STANDARD, Engine};
use mikomai_core::vision::{ImageInput, MAX_IMAGES, MAX_IMAGE_BYTES, MAX_TOTAL_IMAGE_BYTES};
use serde::Deserialize;

pub const WIRE_PREFIX: &str = "__MIKOMAI_ATTACHMENTS_V1__";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireAttachments {
    text: String,
    images: Vec<WireImage>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireImage {
    name: String,
    mime_type: String,
    base64: String,
}

pub fn decode(payload: &str) -> Result<(String, Vec<ImageInput>), String> {
    let Some(json) = payload.strip_prefix(WIRE_PREFIX) else {
        mikomai_core::attachment_policy::validate_native_text_payload(payload)
            .map_err(|e| e.to_string())?;
        return Ok((payload.into(), Vec::new()));
    };
    if json.len() > (MAX_TOTAL_IMAGE_BYTES * 4 / 3) + 256 * 1024 {
        return Err("添付データが上限を超えています".into());
    }
    let wire: WireAttachments =
        serde_json::from_str(json).map_err(|e| format!("添付データが不正です: {e}"))?;
    mikomai_core::attachment_policy::validate_native_text_payload(&wire.text)
        .map_err(|e| e.to_string())?;
    if wire.images.len() > MAX_IMAGES {
        return Err("画像は4ファイル以下にしてください".into());
    }
    let mut total = 0;
    let mut images = Vec::new();
    for image in wire.images {
        if image.base64.len() > MAX_IMAGE_BYTES.div_ceil(3) * 4 {
            return Err("画像は1ファイル8 MiB以下にしてください".into());
        }
        let data = STANDARD
            .decode(image.base64)
            .map_err(|e| format!("画像データが不正です: {e}"))?;
        total += data.len();
        if total > MAX_TOTAL_IMAGE_BYTES {
            return Err("画像の合計は16 MiB以下にしてください".into());
        }
        let image = ImageInput {
            name: image.name,
            mime_type: image.mime_type,
            data,
        };
        image.validate()?;
        images.push(image);
    }
    Ok((wire.text, images))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structured_images_decode_without_losing_bytes_or_text() {
        let png = include_bytes!("../../mikomai-app/tests/fixtures/red-square.png");
        let payload = format!(
            "{WIRE_PREFIX}{}",
            serde_json::json!({"text":"[添付ファイル 1: note.md]\n参考", "images":[{"name":"red.png","mimeType":"image/png","base64":STANDARD.encode(png)}]})
        );
        let (text, images) = decode(&payload).unwrap();
        assert!(text.contains("参考"));
        assert_eq!(images[0].data, png);
    }
    #[test]
    fn malformed_wire_and_fake_images_are_rejected() {
        assert!(decode(&format!("{WIRE_PREFIX}invalid json")).is_err());
        let payload = format!(
            "{WIRE_PREFIX}{}",
            serde_json::json!({"text":"", "images":[{"name":"red.png","mimeType":"image/png","base64":STANDARD.encode(b"fake") }]})
        );
        assert!(decode(&payload).is_err());
        let (text, images) = decode("[添付ファイル 1: note.md]\nhello").unwrap();
        assert!(text.contains("hello"));
        assert!(images.is_empty());
    }
}
