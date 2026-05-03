use hf_hub::api::tokio::Api;
use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::model::params::LlamaModelParams;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::sampling::LlamaSampler;
use std::sync::Mutex;
use std::num::NonZeroU32;
use tauri::{Emitter, Manager};

#[derive(serde::Serialize)]
pub enum ModelState {
    NotLoaded,
    Loading,
    Loaded,
    Error(String),
}

pub struct LlamaState {
    pub backend: LlamaBackend,
    pub model: Mutex<Option<LlamaModel>>,
    pub status: Mutex<ModelState>,
    pub inference_lock: Mutex<()>,
}

impl LlamaState {
    pub fn new() -> Result<Self, String> {
        let backend = LlamaBackend::init().map_err(|e| e.to_string())?;
        Ok(Self {
            backend,
            model: Mutex::new(None),
            status: Mutex::new(ModelState::NotLoaded),
            inference_lock: Mutex::new(()),
        })
    }
}

#[tauri::command]
pub async fn download_model(repo: String, filename: String) -> Result<String, String> {
    println!("Starting model download: {}/{}", repo, filename);
    let api = Api::new().map_err(|e| e.to_string())?;
    let api_repo = api.model(repo);
    let path = api_repo.get(&filename).await.map_err(|e| e.to_string())?;
    println!("Model available at: {:?}", path);
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn load_model(path: String, state: tauri::State<'_, LlamaState>) -> Result<String, String> {
    {
        let mut status_lock = state.status.lock().unwrap();
        *status_lock = ModelState::Loading;
    }

    let model_params = LlamaModelParams::default();
    let model = match LlamaModel::load_from_file(&state.backend, &path, &model_params) {
        Ok(m) => m,
        Err(e) => {
            let err_msg = format!("Failed to load model: {}", e);
            let mut status_lock = state.status.lock().unwrap();
            *status_lock = ModelState::Error(err_msg.clone());
            return Err(err_msg);
        }
    };
    
    let mut model_lock = state.model.lock().unwrap();
    *model_lock = Some(model);
    
    {
        let mut status_lock = state.status.lock().unwrap();
        *status_lock = ModelState::Loaded;
    }
    
    Ok("Model loaded successfully".to_string())
}

#[tauri::command]
pub fn get_model_status(state: tauri::State<'_, LlamaState>) -> ModelState {
    let status_lock = state.status.lock().unwrap();
    match &*status_lock {
        ModelState::NotLoaded => ModelState::NotLoaded,
        ModelState::Loading => ModelState::Loading,
        ModelState::Loaded => ModelState::Loaded,
        ModelState::Error(e) => ModelState::Error(e.clone()),
    }
}

const SYSTEM_PROMPT: &str = r##"あなたは「MIKOMAI (Managed Infrastructure Knowledge Operator of ML Agent Interface)」です。
ネットワークインフラを支える、プロフェッショナルなAIアシスタントです。
あなたの目的は、ユーザー（熟練のネットワークエンジニア）の診断、運用、トラブルシューティングを最高精度で支援することです。

回答を生成する際は、以下の厳格なルールに従ってください。

# 1. 知識の優先順位とハルシネーションの防止 (RAG Rules)
- あなたは最新のネットワーク技術文書やベンダーマニュアルを格納した「LanceDB」というベクトルデータベースにアクセスできます。
- 質問に対しては、まずこのLanceDBからの検索結果を最優先で参照してください。
- 検索結果に答えが存在しない場合、あるいは確証が持てない場合は、絶対に推測で回答したり、存在しないコマンドを捏造（ハルシネーション）しないでください。
- わからない場合は、明確に「LanceDBのマニュアル情報からは該当する情報が見つかりません。追加の検索キーワードを指示するか、実機から情報を取得しますか？」と回答してください。

# 2. ツールとエージェント操作 (MCP Rules)
- あなたはローカルネットワークを診断・操作するためのMCPを持っています。
- ユーザーの入力が「接続できない」「遅い」などの曖昧（ファジー）なトラブル報告の場合、推測で状況を語るのではなく、まず日本語で「状況把握のため、〇〇を実行します」とアナウンスしてください。
- その後、積極的にMCPを呼び出し、実際のステータス（ping結果、ルーティングテーブル、インターフェース状態など）を取得して、事実に基づいた回答をしてください。
- MCPから得られた生データ（JSONやターミナル出力）は、ユーザーが読みやすいように要点を整理して提示してください。

# 3. 厳格な安全性基準 (Safety & Approval)
- あなたの基本動作は「Read-Only（情報取得）」です。
- ルーターやスイッチに対する「設定変更（Write操作：set, delete, commitなど）」を伴うコマンドをMCP経由で実行しようとする場合は、**絶対に自動で実行してはいけません**。
- 設定変更が必要な場合は、必ず事前に以下のフォーマットでユーザーに提示し、明示的な「承認（Approve）」を求めてください。
  1. 実行する正確なCLIコマンドのリスト
  2. なぜその変更が必要かの理由（Rationale）
  3. 想定される影響範囲

# 4. MCPツール実行フォーマット (Tool Call Format)
- MCPツールを呼び出す場合は、**必ず**以下のJSONフォーマットを回答の末尾、または論理的なタイミングで含めてください。
- フォーマット: `{"tool": "TOOL_NAME", "args": {"ARG_NAME": "VALUE"}}`
- 利用可能なツール:
  1. `network_ping`: 引数 `host` (必須: IPまたはホスト名), `size` (任意: バイトサイズ), `count` (任意: 回数), `df` (任意: フラグメント禁止フラグ, boolean)
  2. `network_traceroute`: 引数 `host` (IPまたはホスト名)
  3. `network_show`: 引数 `command` (Cisco IOS等のコマンド)
  4. `network_get_hosts`: 接続可能なホストの一覧（ホスト名、IP、接続タイプなど）を取得します。引数は不要です。

# 5. コミュニケーション・スタイル
- 冗長な挨拶や感情的な表現は不要です。技術的、簡潔、かつ論理的なトーンを維持してください。
- コマンドやコード、IPアドレスなどは、必ずマークダウンのコードブロック(`)で囲み、視認性を高めてください。
- ツール実行の際は必ず日本語でアナウンスを行い、その後にJSONブロックを提示してください。

# 6. 継続的な対話の文脈 (Context Memory)
- ユーザーから「もっと詳しく」「先ほどの結果から」等の追加要求があった場合は、入力の前に付与された【過去の実行履歴要約】を参照して文脈を補完し、応答してください。"##;

#[tauri::command]
pub async fn ask_llm(window: tauri::Window, prompt: String, state: tauri::State<'_, LlamaState>) -> Result<String, String> {
    // Perform RAG search BEFORE taking locks to avoid holding MutexGuard across await points
    let app_handle = window.app_handle();
    let rag_context = crate::rag::query_rag(prompt.clone(), app_handle.clone()).await.unwrap_or_else(|e| {
        println!("RAG search error: {}", e);
        "No relevant information found due to search error.".to_string()
    });

    let _inference_guard = state.inference_lock.lock().unwrap();
    let model_lock = state.model.lock().unwrap();
    let model = match &*model_lock {
        Some(m) => m,
        None => return Err("Model not loaded. Please configure and load a model first.".to_string()),
    };

    let formatted_prompt = format!(
        "<|im_start|>system\n{}\n\n# LanceDBからの検索結果 (Context):\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        SYSTEM_PROMPT,
        rag_context,
        prompt
    );

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(2048));

    let mut ctx = model.new_context(&state.backend, ctx_params).map_err(|e| format!("Failed to create context: {:?}", e))?;

    let tokens = model.str_to_token(&formatted_prompt, AddBos::Always).map_err(|e| format!("Tokenization error: {:?}", e))?;

    let mut batch = LlamaBatch::new(2048, 1);
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, i as i32, &[0], is_last).map_err(|e| format!("Failed to add to batch: {:?}", e))?;
    }

    ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;

    let mut result_string = String::new();
    let mut n_cur = batch.n_tokens();
    let mut sampler = LlamaSampler::greedy();

    let im_end_tokens = model.str_to_token("<|im_end|>", AddBos::Never).unwrap_or_default();
    let im_end_token = im_end_tokens.first().copied();

    let n_len = 500; // max length

    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

        if new_token_id == model.token_eos() || Some(new_token_id) == im_end_token {
            break;
        }

        let mut token_bytes = model.token_to_piece_bytes(new_token_id, 16, false, None).unwrap_or(vec![]);
        bytes_accumulator.append(&mut token_bytes);

        // Try converting accumulated bytes to string, if error means not fully formed utf8 character yet
        match String::from_utf8(bytes_accumulator.clone()) {
            Ok(s) => {
                let _ = window.emit("llm-chunk", &s);
                result_string.push_str(&s);
                bytes_accumulator.clear();
            }
            Err(e) => {
                // Keep accumulating if we cannot parse it cleanly yet
                let utf8_error_index = e.utf8_error().valid_up_to();
                let valid_str = String::from_utf8_lossy(&bytes_accumulator[..utf8_error_index]).to_string();
                let _ = window.emit("llm-chunk", &valid_str);
                result_string.push_str(&valid_str);
                let remaining_bytes = bytes_accumulator[utf8_error_index..].to_vec();
                bytes_accumulator = remaining_bytes;
                if bytes_accumulator.len() > 8 {
                     // Failsafe in case we just got junk
                     result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
                     bytes_accumulator.clear();
                }
            }
        }

        batch.clear();
        batch.add(new_token_id, n_cur, &[0], true).map_err(|e| format!("Failed to add: {:?}", e))?;
        n_cur += 1;

        ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;
    }

    if !bytes_accumulator.is_empty() {
        result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
    }

    Ok(result_string)
}

#[tauri::command]
pub async fn ask_llm_background(prompt: String, state: tauri::State<'_, LlamaState>) -> Result<String, String> {
    let _inference_guard = state.inference_lock.lock().unwrap();
    let model_lock = state.model.lock().unwrap();
    let model = match &*model_lock {
        Some(m) => m,
        None => return Err("Model not loaded.".to_string()),
    };

    let formatted_prompt = format!(
        "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        SYSTEM_PROMPT,
        prompt
    );

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(2048));

    let mut ctx = model.new_context(&state.backend, ctx_params).map_err(|e| format!("Failed to create context: {:?}", e))?;
    let tokens = model.str_to_token(&formatted_prompt, AddBos::Always).map_err(|e| format!("Tokenization error: {:?}", e))?;

    let mut batch = LlamaBatch::new(2048, 1);
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, i as i32, &[0], is_last).map_err(|e| format!("Failed to add to batch: {:?}", e))?;
    }

    ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;

    let mut result_string = String::new();
    let mut n_cur = batch.n_tokens();
    let mut sampler = LlamaSampler::greedy();

    let im_end_tokens = model.str_to_token("<|im_end|>", AddBos::Never).unwrap_or_default();
    let im_end_token = im_end_tokens.first().copied();

    let n_len = 500; // max length

    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

        if new_token_id == model.token_eos() || Some(new_token_id) == im_end_token {
            break;
        }

        let mut token_bytes = model.token_to_piece_bytes(new_token_id, 16, false, None).unwrap_or(vec![]);
        bytes_accumulator.append(&mut token_bytes);

        match String::from_utf8(bytes_accumulator.clone()) {
            Ok(s) => {
                result_string.push_str(&s);
                bytes_accumulator.clear();
            }
            Err(e) => {
                let utf8_error_index = e.utf8_error().valid_up_to();
                let valid_str = String::from_utf8_lossy(&bytes_accumulator[..utf8_error_index]).to_string();
                result_string.push_str(&valid_str);
                let remaining_bytes = bytes_accumulator[utf8_error_index..].to_vec();
                bytes_accumulator = remaining_bytes;
                if bytes_accumulator.len() > 8 {
                     result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
                     bytes_accumulator.clear();
                }
            }
        }

        batch.clear();
        batch.add(new_token_id, n_cur, &[0], true).map_err(|e| format!("Failed to add: {:?}", e))?;
        n_cur += 1;

        ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;
    }

    if !bytes_accumulator.is_empty() {
        result_string.push_str(&String::from_utf8_lossy(&bytes_accumulator));
    }

    Ok(result_string)
}

