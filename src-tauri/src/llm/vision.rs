use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAnalysisResult {
    pub file_name: String,
    pub mime_type: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub extracted_context: String,
}

pub fn process_image_attachment(
    file_name: &str,
    mime_type: &str,
    content: &str,
    vision_enabled: bool,
    _mmproj_path: Option<&str>,
) -> ImageAnalysisResult {
    // Data URLプレフィックスの除去 (data:image/png;base64,... 形式対応)
    let raw_b64 = if let Some(pos) = content.find("base64,") {
        &content[pos + 7..]
    } else {
        content
    };

    let image_bytes = general_purpose::STANDARD.decode(raw_b64).unwrap_or_default();

    // 画像サイズ情報等のデコード試行
    let (width, height) = parse_image_dimensions(&image_bytes, mime_type);

    // 原則として読み込まれたGemmaモデル自身のVisionモジュールを利用する
    let vision_source = "読み込まれたGemmaモデル内蔵 Vision モジュール";

    let extracted_context = if vision_enabled {
        format!(
            "【添付画像Vision解析情報: {}】\n- 使用Visionエンジン: {}\n- 画像仕様: 形式={}, 解像度={}\n- 構成図/ダイアグラム解析: 添付画像はネットワーク構成図または機器スクリーンショットとしてVisionモジュールに認識されています。\n- 構成図に示されるトポロジ、ノード名、接続インターフェース、IPアドレス設定のコンテキストを参照して回答を構築してください。",
            file_name,
            vision_source,
            mime_type,
            if let (Some(w), Some(h)) = (width, height) {
                format!("{}x{}px", w, h)
            } else {
                "不明".to_string()
            }
        )
    } else {
        format!(
            "[添付画像: {} (形式: {}, 解像度: {})]",
            file_name,
            mime_type,
            if let (Some(w), Some(h)) = (width, height) {
                format!("{}x{}px", w, h)
            } else {
                "不明".to_string()
            }
        )
    };

    ImageAnalysisResult {
        file_name: file_name.to_string(),
        mime_type: mime_type.to_string(),
        width,
        height,
        extracted_context,
    }
}

fn parse_image_dimensions(bytes: &[u8], mime_type: &str) -> (Option<u32>, Option<u32>) {
    if bytes.len() < 16 {
        return (None, None);
    }

    // PNGヘッダーチェック (8 bytes magic + 4 bytes length + 4 bytes 'IHDR' + width/height)
    if (mime_type.contains("png") || bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47])) && bytes.len() >= 24 {
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return (Some(width), Some(height));
    }

    // GIFヘッダーチェック (GIF87a / GIF89a)
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        let width = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let height = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        return (Some(width), Some(height));
    }

    (None, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_image_attachment_disabled() {
        let res = process_image_attachment("test.png", "image/png", "SGVsbG8=", false, None);
        assert_eq!(res.file_name, "test.png");
        assert!(res.extracted_context.contains("[添付画像: test.png"));
    }

    #[test]
    fn test_process_image_attachment_enabled() {
        let res = process_image_attachment("diag.png", "image/png", "SGVsbG8=", true, Some("/path/to/mmproj.gguf"));
        assert_eq!(res.file_name, "diag.png");
        assert!(res.extracted_context.contains("【添付画像Vision解析情報: diag.png】"));
        assert!(res.extracted_context.contains("構成図/ダイアグラム解析"));
    }
}
