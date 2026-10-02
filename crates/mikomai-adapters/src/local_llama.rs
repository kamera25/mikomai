//! Local text and multimodal inference. Model lifetime is infrastructure,
//! independent of the FFI transport and the Core's response policy.
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::model::{AddBos, LlamaModel};
use llama_cpp_2::sampling::LlamaSampler;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

struct LoadedModel {
    backend: Arc<LlamaBackend>,
    model: Arc<LlamaModel>,
    path: PathBuf,
    gpu_layers: u32,
}

static MODEL: OnceLock<Mutex<Option<LoadedModel>>> = OnceLock::new();
pub static CANCEL_INFERENCE: AtomicBool = AtomicBool::new(false);
struct InferenceConfig {
    temperature: f32,
    repetition_penalty: f32,
    n_ctx: u32,
    max_new_tokens: u32,
}

static INFERENCE_CONFIG: OnceLock<Mutex<InferenceConfig>> = OnceLock::new();

fn inference_config_slot() -> &'static Mutex<InferenceConfig> {
    INFERENCE_CONFIG.get_or_init(|| {
        Mutex::new(InferenceConfig {
            temperature: 0.2,
            repetition_penalty: 1.1,
            n_ctx: 8192,
            max_new_tokens: 2048,
        })
    })
}

fn model_slot() -> &'static Mutex<Option<LoadedModel>> {
    MODEL.get_or_init(|| Mutex::new(None))
}
pub fn load(path: &Path) -> Result<String, String> {
    if !path.is_file() {
        return Err(format!("model file does not exist: {}", path.display()));
    }
    if path.extension().and_then(|v| v.to_str()) != Some("gguf") {
        return Err("model must be a .gguf file".into());
    }
    let backend = Arc::new(
        LlamaBackend::init()
            .map_err(|e| format!("llama.cpp backend initialization failed: {e}"))?,
    );
    let layers = std::env::var("MIKOMAI_N_GPU_LAYERS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut params = LlamaModelParams::default().with_n_gpu_layers(layers);
    if layers == 0 {
        // Exclude GPU devices explicitly: n_gpu_layers=0 alone still lets the
        // runtime initialize Metal for context scheduling on macOS.
        params = params.with_devices(&[]).map_err(|e| e.to_string())?;
    }
    let params = std::pin::pin!(params);
    let loaded = LlamaModel::load_from_file(&backend, &path, &params)
        .map_err(|e| format!("model load failed: {e}"))?;
    *model_slot()
        .lock()
        .map_err(|_| "model state is unavailable".to_string())? = Some(LoadedModel {
        backend,
        model: Arc::new(loaded),
        path: path.to_path_buf(),
        gpu_layers: layers,
    });
    CANCEL_INFERENCE.store(false, Ordering::Relaxed);
    Ok("モデルを読み込みました".into())
}
pub fn status() -> Result<String, String> {
    let model = model_slot()
        .lock()
        .map_err(|_| "model state is unavailable".to_string())?;
    Ok(model
        .as_ref()
        .map(|m| m.path.to_string_lossy().into_owned())
        .unwrap_or_default())
}
pub fn set_params(
    temperature: f32,
    repetition_penalty: f32,
    n_ctx: u32,
    max_new_tokens: u32,
) -> Result<String, String> {
    let mut config = inference_config_slot()
        .lock()
        .map_err(|_| "inference config state unavailable".to_string())?;
    if (0.0..=2.0).contains(&temperature) {
        config.temperature = temperature;
    }
    if (0.5..=2.0).contains(&repetition_penalty) {
        config.repetition_penalty = repetition_penalty;
    }
    if (512..=32768).contains(&n_ctx) {
        config.n_ctx = n_ctx;
    }
    if (1..=8192).contains(&max_new_tokens) {
        config.max_new_tokens = max_new_tokens;
    }
    Ok("推論パラメータを更新しました".into())
}
pub fn infer(prompt: &str) -> Result<String, String> {
    infer_streaming(prompt, |_, _| {})
}

pub fn infer_streaming<F: FnMut(&str, bool)>(prompt: &str, on_token: F) -> Result<String, String> {
    infer_inner(prompt, None, on_token)
}
fn infer_inner<F: FnMut(&str, bool)>(
    prompt: &str,
    vision: Option<&mikomai_core::vision::VisionRequest>,
    mut on_token: F,
) -> Result<String, String> {
    let guard = model_slot()
        .lock()
        .map_err(|_| "model state is unavailable".to_string())?;
    let loaded = guard.as_ref().ok_or_else(|| {
        "モデルが未ロードです。設定から GGUF モデルを読み込んでください。".to_string()
    })?;
    let (temperature, repetition_penalty, n_ctx, max_new) = {
        let config = inference_config_slot()
            .lock()
            .map(|g| {
                (
                    g.temperature,
                    g.repetition_penalty,
                    g.n_ctx,
                    g.max_new_tokens as usize,
                )
            })
            .unwrap_or((0.2, 1.1, 8192, 2048));
        config
    };
    let system = mikomai_core::response::SYSTEM_PROMPT;
    let formatted = format!("<|turn>system\n{system}<turn|>\n");
    let mut tokens = loaded
        .model
        .str_to_token(&formatted, AddBos::Always)
        .map_err(|e| format!("system prompt tokenization failed: {e}"))?;
    let mut user_tokens = loaded
        .model
        .str_to_token(
            &format!("<|turn>user\n{prompt}<turn|>\n<|turn>model\n"),
            AddBos::Never,
        )
        .map_err(|e| format!("chat prompt tokenization failed: {e}"))?;

    let n_ctx_val = (n_ctx as usize).max(512);
    // Reserve minimum generation room: at least 64 tokens, up to 1/4 context
    let min_generation_room = 64.max(16).min(n_ctx_val / 4);
    let max_prompt_budget = n_ctx_val.saturating_sub(min_generation_room);

    // If total prompt tokens exceed prompt budget, truncate gracefully instead of hard failing
    if tokens.len() + user_tokens.len() > max_prompt_budget {
        let max_sys = max_prompt_budget / 3;
        if tokens.len() > max_sys {
            tokens.truncate(max_sys);
        }
        let remaining_for_user = max_prompt_budget.saturating_sub(tokens.len());
        if user_tokens.len() > remaining_for_user {
            let skip = user_tokens.len() - remaining_for_user;
            user_tokens = user_tokens[skip..].to_vec();
        }
    }
    tokens.extend(user_tokens);

    let mut params = LlamaContextParams::default();
    params = params
        .with_n_ctx(std::num::NonZeroU32::new(n_ctx))
        .with_n_batch(512)
        .with_n_ubatch(256)
        .with_flash_attention_policy(1)
        .with_offload_kqv(loaded.gpu_layers > 0)
        .with_op_offload(loaded.gpu_layers > 0);
    params = params
        .with_type_k(llama_cpp_2::context::params::KvCacheType::Q4_0)
        .with_type_v(llama_cpp_2::context::params::KvCacheType::Q4_0);
    let backend = loaded.backend.as_ref();
    let model = loaded.model.as_ref();
    let mut ctx = match model.new_context(backend, params) {
        Ok(c) => c,
        Err(_) => {
            let fallback_params = LlamaContextParams::default()
                .with_n_ctx(std::num::NonZeroU32::new(n_ctx))
                .with_n_batch(512)
                .with_offload_kqv(loaded.gpu_layers > 0)
                .with_op_offload(loaded.gpu_layers > 0);
            model
                .new_context(backend, fallback_params)
                .map_err(|e| format!("inference context creation failed: {e}"))?
        }
    };
    let mut batch = LlamaBatch::new(512, 1);
    let mut pos = if let Some(request) = vision {
        use llama_cpp_2::mtmd::{
            mtmd_default_marker, MtmdBitmap, MtmdContext, MtmdContextParams, MtmdInputText,
        };
        request.validate()?;
        let projector = vision_config()
            .lock()
            .map_err(|_| "Vision設定を読み込めません".to_string())?
            .clone()
            .ok_or("Visionを有効にし、対応するmmprojファイルを設定してください")?;
        let params = MtmdContextParams {
            use_gpu: loaded.gpu_layers > 0,
            print_timings: false,
            ..Default::default()
        };
        let mtmd = MtmdContext::init_from_file(&projector.to_string_lossy(), model, &params)
            .map_err(|e| format!("Vision projector load failed: {e}"))?;
        if !mtmd.support_vision() {
            return Err("このモデルとmmprojの組み合わせは画像入力に対応していません".into());
        }
        let bitmaps = request
            .images
            .iter()
            .map(|image| {
                MtmdBitmap::from_buffer(&mtmd, &image.data, false)
                    .map_err(|e| format!("画像 {} を読み込めません: {e}", image.name))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if bitmaps.iter().any(MtmdBitmap::is_audio) {
            return Err("画像以外の入力は使用できません".into());
        }
        let media = request
            .images
            .iter()
            .map(|i| format!("{}\n{}", i.name, mtmd_default_marker()))
            .collect::<Vec<_>>()
            .join("\n");
        let user = format!("{prompt}\n{media}");
        let template = model
            .chat_template(None)
            .map_err(|e| format!("Vision model chat template is unavailable: {e}"))?;
        let formatted =
            format_vision_chat(template.to_str().map_err(|e| e.to_string())?, system, &user)?;
        let chunks = mtmd
            .tokenize(
                MtmdInputText {
                    text: formatted,
                    add_special: true,
                    parse_special: true,
                },
                &bitmaps.iter().collect::<Vec<_>>(),
            )
            .map_err(|e| format!("Vision tokenization failed: {e}"))?;
        let positions = chunks.total_positions();
        if positions <= 0 || positions as usize > max_prompt_budget {
            return Err("画像と文章がコンテキスト長を超えています。画像を縮小するかコンテキスト長を増やしてください".into());
        }
        if CANCEL_INFERENCE.load(Ordering::Relaxed) {
            return Err("画像解析を停止しました".into());
        }
        chunks
            .eval_chunks(&mtmd, &ctx, 0, 0, 512, true)
            .map_err(|e| format!("Vision evaluation failed: {e}"))?
    } else {
        for (chunk_index, chunk) in tokens.chunks(256).enumerate() {
            batch.clear();
            let base = chunk_index * 256;
            for (offset, token) in chunk.iter().copied().enumerate() {
                let absolute = base + offset;
                batch
                    .add(token, absolute as i32, &[0], absolute + 1 == tokens.len())
                    .map_err(|e| format!("prompt decode setup failed: {e}"))?;
            }
            ctx.decode(&mut batch)
                .map_err(|e| format!("prompt evaluation failed: {e}"))?;
        }
        tokens.len() as i32
    };
    let actual_max_new = max_new.min(n_ctx_val.saturating_sub(pos as usize)).max(1);
    let mut sampler = LlamaSampler::chain_simple(vec![
        LlamaSampler::penalties(model.n_vocab(), 64, repetition_penalty, 0.0, 0.0),
        LlamaSampler::temp(temperature),
        LlamaSampler::dist(42),
    ]);
    let end = model
        .str_to_token("<turn|>", AddBos::Never)
        .ok()
        .and_then(|v| v.first().copied());
    let mut out = String::new();
    let mut pending_utf8 = Vec::new();
    for _ in 0..actual_max_new {
        if CANCEL_INFERENCE.load(Ordering::Relaxed) {
            if out.trim().is_empty() {
                let msg = "生成を停止しました。";
                on_token(msg, true);
                return Ok(msg.into());
            }
            let notice = "\n\n(生成を停止しました)";
            out.push_str(notice);
            on_token(notice, true);
            break;
        }
        let token = sampler.sample(&ctx, -1);
        if model.is_eog_token(token) || Some(token) == end {
            on_token("", true);
            break;
        }
        pending_utf8.extend(
            model
                .token_to_piece_bytes(token, 256, false, None)
                .unwrap_or_default(),
        );
        match std::str::from_utf8(&pending_utf8) {
            Ok(valid) => {
                out.push_str(valid);
                on_token(valid, false);
                pending_utf8.clear();
            }
            Err(error) => {
                let valid_len = error.valid_up_to();
                if valid_len > 0 {
                    let valid_str =
                        std::str::from_utf8(&pending_utf8[..valid_len]).expect("valid prefix");
                    out.push_str(valid_str);
                    on_token(valid_str, false);
                    pending_utf8.drain(..valid_len);
                }
                if error.error_len().is_some() {
                    return Err("model generated invalid UTF-8 text".into());
                }
            }
        }
        batch.clear();
        batch
            .add(token, pos, &[0], true)
            .map_err(|e| format!("decode batch failed: {e}"))?;
        pos += 1;
        ctx.decode(&mut batch)
            .map_err(|e| format!("token generation failed: {e}"))?;
    }
    if !pending_utf8.is_empty() {
        let remaining = std::str::from_utf8(&pending_utf8)
            .map_err(|_| "model response ended mid-character".to_string())?;
        out.push_str(remaining);
        on_token(remaining, false);
    }
    on_token("", true);
    if out.trim().is_empty() {
        return Err("model returned an empty response".into());
    }
    Ok(out.trim().to_string())
}

static VISION_CONFIG: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
fn vision_config() -> &'static Mutex<Option<PathBuf>> {
    VISION_CONFIG.get_or_init(|| Mutex::new(None))
}
pub fn configure_vision(enabled: bool, projector: Option<&Path>) -> Result<String, String> {
    let mut config = vision_config()
        .lock()
        .map_err(|_| "Vision設定を更新できません".to_string())?;
    *config = None;
    let path = if enabled {
        let path = projector
            .filter(|p| p.is_file())
            .ok_or("対応するmmproj GGUFファイルを指定してください")?;
        if path.extension().and_then(|s| s.to_str()) != Some("gguf") {
            return Err("mmprojはGGUFファイルを指定してください".into());
        }
        Some(path.to_path_buf())
    } else {
        None
    };
    *config = path;
    Ok("Vision設定を更新しました".into())
}

pub struct LocalInference;
impl mikomai_core::port::InferencePort for LocalInference {
    fn complete<'a>(&'a self, prompt: &'a str) -> mikomai_core::port::PortFuture<'a, String> {
        Box::pin(async move { infer(prompt) })
    }
}
impl mikomai_core::port::StreamingInferencePort for LocalInference {
    fn complete_streaming(
        &self,
        prompt: &str,
        on_chunk: &mut dyn FnMut(&str, bool),
    ) -> Result<String, String> {
        infer_streaming(prompt, on_chunk)
    }
}
pub struct LocalVision;
impl mikomai_core::port::VisionPort for LocalVision {
    fn analyze<'a>(
        &'a self,
        request: &'a mikomai_core::vision::VisionRequest,
    ) -> mikomai_core::port::PortFuture<'a, String> {
        Box::pin(async move {
            if CANCEL_INFERENCE.load(Ordering::Relaxed) {
                return Err("画像解析を停止しました".into());
            }
            let answer = infer_inner(&request.prompt, Some(request), |_, _| {})?;
            if CANCEL_INFERENCE.load(Ordering::Relaxed) {
                return Err("画像解析を停止しました".into());
            }
            Ok(answer)
        })
    }
}

// Avoid the native template detector's C++ exception path for Gemma 4, whose
// <|turn> format is not recognized by this llama.cpp template renderer.
fn format_vision_chat(template: &str, system: &str, user: &str) -> Result<String, String> {
    if template.contains("<|turn>") {
        Ok(format!(
            "<|turn>system\n{system}<turn|>\n<|turn>user\n{user}<turn|>\n<|turn>model\n"
        ))
    } else {
        Err("現在のローカルVision推論はGemma 4のチャット形式に対応しています。対応するモデルとmmprojを選択してください".into())
    }
}
