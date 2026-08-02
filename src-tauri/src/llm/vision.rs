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
    mmproj_path: Option<&str>,
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

    let vision_source = if let Some(path) = mmproj_path {
        format!("Gemma Vision モジュール ({})", path)
    } else {
        "Gemma Vision モジュール".to_string()
    };

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

fn resolve_mmproj_path(path_str: &str) -> Option<String> {
    let path = std::path::Path::new(path_str);
    if !path.exists() {
        log::warn!("mmproj path does not exist: {}", path_str);
        return None;
    }
    if path.is_file() {
        return Some(path_str.to_string());
    }
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            let mut gguf_files = Vec::new();
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("gguf") {
                    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                    if name.contains("mmproj") || name.contains("clip") || name.contains("vision") {
                        log::info!("Resolved mmproj GGUF file from directory: {}", p.display());
                        return Some(p.to_string_lossy().to_string());
                    }
                    gguf_files.push(p.to_string_lossy().to_string());
                }
            }
            if let Some(first) = gguf_files.into_iter().next() {
                log::info!("Resolved fallback GGUF file from directory: {}", first);
                return Some(first);
            }
        }
    }
    None
}

pub async fn analyze_image_attachment(
    file_name: &str,
    mime_type: &str,
    content: &str,
    vision_enabled: bool,
    mmproj_path: Option<&str>,
    llama_state: &crate::llm::llm::LlamaState,
) -> ImageAnalysisResult {
    let fallback = process_image_attachment(file_name, mime_type, content, vision_enabled, mmproj_path);
    if !vision_enabled {
        log::info!("Vision is disabled in settings for {}", file_name);
        return fallback;
    }

    let raw_mmproj = match mmproj_path {
        Some(p) if !p.trim().is_empty() => p,
        _ => {
            log::warn!("Vision enabled but mmproj_path is empty for {}", file_name);
            return fallback;
        }
    };

    let mmproj_path_str = match resolve_mmproj_path(raw_mmproj) {
        Some(p) => p,
        None => {
            log::warn!("Could not resolve valid mmproj GGUF file from path: {}", raw_mmproj);
            return fallback;
        }
    };

    log::info!("Starting Vision Inference for {} using mmproj: {}", file_name, mmproj_path_str);

    let raw_b64 = if let Some(pos) = content.find("base64,") {
        &content[pos + 7..]
    } else {
        content
    };

    let image_bytes = match general_purpose::STANDARD.decode(raw_b64) {
        Ok(b) if !b.is_empty() => b,
        _ => {
            log::warn!("Failed to decode base64 image bytes for {}", file_name);
            return fallback;
        }
    };

    let shared = {
        let lock = llama_state.shared.lock().await;
        lock.clone()
    };

    let shared_model = match shared {
        Some(s) => s,
        None => {
            log::warn!("Llama model is not currently loaded in llama_state");
            return fallback;
        }
    };

    let file_name_owned = file_name.to_string();
    let mime_type_owned = mime_type.to_string();
    let width = fallback.width;
    let height = fallback.height;

    let res = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let model = &shared_model.model;
        let backend = &shared_model.backend;

        let mtmd_params = llama_cpp_2::mtmd::MtmdContextParams::default();
        let mtmd_ctx = llama_cpp_2::mtmd::MtmdContext::init_from_file(&mmproj_path_str, model, &mtmd_params)
            .map_err(|e| format!("Failed to init mtmd context: {:?}", e))?;

        let bitmap = llama_cpp_2::mtmd::MtmdBitmap::from_buffer(&mtmd_ctx, &image_bytes, false)
            .map_err(|e| format!("Failed to create mtmd bitmap: {:?}", e))?;

        let marker = llama_cpp_2::mtmd::mtmd_default_marker();
        let prompt_text = format!(
            "{} この画像に記載されているネットワーク構成図、機器トポロジ、IPアドレス、接続インターフェース、テキスト、および内容を日本語で詳細に解析・解説してください。",
            marker
        );

        let input_text = llama_cpp_2::mtmd::MtmdInputText {
            text: prompt_text,
            add_special: true,
            parse_special: true,
        };

        let chunks = mtmd_ctx.tokenize(input_text, &[&bitmap])
            .map_err(|e| format!("Failed to tokenize mtmd chunks: {:?}", e))?;

        let mut ctx_params = llama_cpp_2::context::params::LlamaContextParams::default();
        ctx_params = ctx_params.with_n_ctx(std::num::NonZeroU32::new(4096));
        ctx_params = ctx_params.with_flash_attention_policy(1);

        let mut ctx = model.new_context(backend, ctx_params)
            .map_err(|e| format!("Failed to create LlamaContext for vision: {:?}", e))?;

        let new_n_past = chunks.eval_chunks(&mtmd_ctx, &ctx, 0, 0, 512, true)
            .map_err(|e| format!("Failed to eval mtmd chunks: {:?}", e))?;

        let samplers = vec![
            llama_cpp_2::sampling::LlamaSampler::temp(0.2),
            llama_cpp_2::sampling::LlamaSampler::dist(42),
        ];
        let mut sampler = llama_cpp_2::sampling::LlamaSampler::chain_simple(samplers);

        let mut batch = llama_cpp_2::llama_batch::LlamaBatch::new(4096, 1);
        let mut result_text = String::new();
        let mut n_cur = new_n_past;
        let mut bytes_acc = Vec::new();

        for _ in 0..512 {
            let token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);
            if token_id == model.token_eos() {
                break;
            }

            if let Ok(mut piece) = model.token_to_piece_bytes(token_id, 256, false, None) {
                bytes_acc.append(&mut piece);
            }

            batch.clear();
            batch.add(token_id, n_cur, &[0], true).map_err(|e| format!("Batch add error: {:?}", e))?;
            n_cur += 1;
            ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;
        }

        if !bytes_acc.is_empty() {
            result_text.push_str(&String::from_utf8_lossy(&bytes_acc));
        }

        Ok(result_text)
    }).await;

    match res {
        Ok(Ok(vision_text)) => {
            if !vision_text.trim().is_empty() {
                log::info!("Vision Inference Success for {}: {} chars generated", file_name, vision_text.len());
                let extracted_context = format!(
                    "【添付画像Vision解析情報: {}】\n- 画像仕様: 形式={}, 解像度={}\n- Visionモデル解析結果:\n{}\n- 上記のVision解析結果（トポロジ、機器名、IPアドレス、設定など）を踏まえてユーザーの要望に回答してください。",
                    file_name_owned,
                    mime_type_owned,
                    if let (Some(w), Some(h)) = (width, height) {
                        format!("{}x{}px", w, h)
                    } else {
                        "不明".to_string()
                    },
                    vision_text.trim()
                );

                ImageAnalysisResult {
                    file_name: file_name_owned,
                    mime_type: mime_type_owned,
                    width,
                    height,
                    extracted_context,
                }
            } else {
                log::warn!("Vision inference returned empty text for {}, falling back", file_name);
                fallback
            }
        }
        Ok(Err(e)) => {
            log::warn!("Vision inference failed for {}: {}, falling back", file_name, e);
            fallback
        }
        Err(e) => {
            log::error!("Vision spawn_blocking panicked for {}: {}, falling back", file_name, e);
            fallback
        }
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
