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
use tauri::Emitter;
use tauri::Manager;

#[derive(serde::Serialize)]
pub enum ModelState {
    NotLoaded,
    Loading,
    Loaded,
    Error(String),
}

pub struct LlamaState {
    pub model: Mutex<Option<LlamaModel>>,
    pub status: Mutex<ModelState>,
    pub inference_lock: Mutex<()>,
    pub backend: LlamaBackend,
}

impl LlamaState {
    pub fn new() -> Result<Self, String> {
        let backend = LlamaBackend::init().map_err(|e| e.to_string())?;
        Ok(Self {
            model: Mutex::new(None),
            status: Mutex::new(ModelState::NotLoaded),
            inference_lock: Mutex::new(()),
            backend,
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
ネットワークインフラを支えるプロフェッショナルなAIアシスタントとして、熟練のネットワークエンジニアの診断、運用、トラブルシューティングを最高精度で支援します。

回答を生成する際は、以下の厳格なルールに従ってください。

# 1. コミュニケーション・スタイルとトンマナ
- 冗長な挨拶や感情的な表現は一切不要です。常に技術的、簡潔、かつ論理的なトーンを維持してください。
- コマンド、コード、IPアドレスなどは、必ずマークダウンのコードブロック(`)で囲み、視認性を高めてください。
- ツール実行時は、まず日本語で「状況把握のため、〇〇を実行します」と簡潔にアナウンスし、その後ツール呼び出しのJSONブロックを提示してください。
- 数式はそのまま表示せず、分かりやすくテキストで表現してください。

# 2. 厳格な安全性基準 (Safety & Approval)
- あなたの基本動作は「Read-Only（情報取得）」です。
- ルーターやスイッチに対する「設定変更（Write操作：set, delete, commitなど）」を伴うコマンドをMCP経由で実行しようとする場合は、**絶対に自動で実行せず、必ず事前に以下のフォーマットでユーザーに提示し、明示的な「承認（Approve）」を求めてください**。
  1. 実行する正確なCLIコマンドのリスト
  2. なぜその変更が必要かの理由（Rationale）
  3. 想定される影響範囲

# 3. ツールとエージェント操作 (MCP Rules)
- ローカルネットワークを診断・操作するためのMCPを持っています。曖昧なトラブル報告（「接続できない」「遅い」など）を受けた際は、推測で語るのではなく、積極的にMCPツールを呼び出し、実際のステータスを取得して事実に基づいた回答を行ってください。
- MCPから得られた生データ（JSONやターミナル出力）は、要点を整理して提示してください。
- ツールを呼び出す場合は、**必ず**以下のJSONフォーマットを回答の末尾、または論理的なタイミングで含めてください。
  `{"tool": "TOOL_NAME", "args": {"ARG_NAME": "VALUE"}}`
- Ping実行後には完結に状況を説明し、対話を終了してください。
- 利用可能なツール:
  1. `network_ping`: 引数 `host` (必須: IPまたはホスト名), `size` (任意: バイトサイズ), `count` (任意: 回数), `df` (任意: フラグメント禁止フラグ, boolean)
  2. `network_traceroute`: 引数 `host` (IPまたはホスト名)
  3. `network_show`: 引数 `command` (Cisco IOS等のコマンド)
  4. `network_get_hosts`: 接続可能なホストの一覧（ホスト名、IP、接続タイプなど）を取得（引数不要）
  5. `query_nw_db`: 技術文書データベース（NW-DB）を検索します。引数 `query` (検索クエリ)。PingやTraceなどの単純な処理ではなく、自身で解決できない場合のみ使用してください。
  6. `network_arp`: ローカルシステムのARPテーブル（IPとMACアドレスの対応表）を取得します（引数不要）
  7. `network_get_ip_info`: 自分のIPアドレス、サブネットマスク、デフォルトゲートウェイ、DNS設定などの情報を取得します（引数不要）

# 4. 知識の優先順位とハルシネーションの防止 (RAG Rules)
- ネットワーク技術文書を格納した「NW-DB」の検索結果を最優先で参照してください。
- LLMが必要だと判断した時のみ（自力で解決できない場合や特定のメーカー独自のコマンド等を確認する必要がある場合）、`query_nw_db` ツールを使用して検索を行ってください。PingやTracerouteなどの単純な処理ではRAGを利用しないでください。
- 検索結果に答えがない場合や確証がない場合は、推測や捏造（ハルシネーション）を絶対に避け、「NW-DBのマニュアルからは該当する情報が見つかりません。追加の検索キーワードを指示するか、実機から情報を取得しますか？」と回答してください。
- ユーザーが機器メーカーやOSを指定してRAGの検索結果を得た場合、その結果の冒頭にある `[Context: ...]` が一致するデータのみを使用し、一致しない資料は完全に無視してください。
- トラブルシューティングの手順が含まれている場合は、その手順に沿って段階的に診断・ツールの実行を進めてください。
- ユーザからの依頼については`[Context: ...]`に「operation」が含まれているもののみ検索対象とし、その実行手順に沿って回答を行ってください。

# 5. 継続的な対話の文脈 (Context Memory)
- ユーザーから追加要求があった場合は、入力の末尾に付与された【過去の実行履歴要約】を参照して文脈を補完し、応答してください。"##;

const SUMMARIZATION_SYSTEM_PROMPT: &str = r##"あなたは「MIKOMAI」の要約担当ユニットです。
入力された対話や実行結果から、重要な「事実」と「実行結果」のみを抽出して要約してください。

# 厳格なルール:
1. **原始人構文**で出力してください（助詞を極限まで省き、体言止めや簡潔な動詞のみを使用）。
   - 例: 「Ping 成功。通信 ヨシ。」「CPU 負荷 高い。調査 必要。」「設定 変更 完了。」
2. 実行したMCPツールの詳細（ツール名、引数、JSONブロックなど）は要約に含めないでください。
3. 事実と結果以外の解釈や挨拶は一切不要です。
4. 40文字以内で簡潔に出力してください。"##;

#[tauri::command]
pub async fn ask_llm(
    window: tauri::Window,
    prompt: String,
    llama_state: tauri::State<'_, LlamaState>,
) -> Result<String, String> {
    println!("Received prompt: {}", prompt);

    let _inference_guard = llama_state.inference_lock.lock().unwrap();
    let model_lock = llama_state.model.lock().unwrap();
    let model = match &*model_lock {
        Some(m) => m,
        None => return Err("Model not loaded. Please configure and load a model first.".to_string()),
    };

    let formatted_prompt = format!(
        "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
        SYSTEM_PROMPT,
        prompt
    );

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(2048));

    let mut ctx = model.new_context(&llama_state.backend, ctx_params).map_err(|e| format!("Failed to create context: {:?}", e))?;

    let mut tokens = model.str_to_token(&formatted_prompt, AddBos::Always).map_err(|e| format!("Tokenization error: {:?}", e))?;
    println!("Total tokens in prompt: {}", tokens.len());

    // Truncate if tokens exceed capacity (leaving room for response)
    let max_tokens = 2048 - 512; // Leave 512 for response
    if tokens.len() > max_tokens {
        println!("Prompt too long ({} tokens), truncating to {} tokens", tokens.len(), max_tokens);
        // Keep the start (system prompt) and end (user prompt) but truncate middle if possible?
        // For simplicity, just take the first max_tokens.
        tokens.truncate(max_tokens);
    }

    let mut batch = LlamaBatch::new(2048, 1);
    let last_index = tokens.len() - 1;
    for (i, token) in tokens.into_iter().enumerate() {
        let is_last = i == last_index;
        batch.add(token, i as i32, &[0], is_last).map_err(|e| format!("Failed to add to batch: {:?}", e))?;
    }

    ctx.decode(&mut batch).map_err(|e| format!("Decode error: {:?}", e))?;

    let mut result_string = String::new();
    let mut n_cur = batch.n_tokens();
    let settings = crate::settings::load_settings(window.app_handle().clone()).unwrap_or_default();
    println!("LLM Temperature setting: {}", settings.temperature);

    let sampler = if settings.temperature <= 0.0 {
        LlamaSampler::chain_simple([
            LlamaSampler::penalties(64, settings.repetition_penalty, 0.0, 0.0),
            LlamaSampler::greedy(),
        ])
    } else {
        LlamaSampler::chain_simple([
            LlamaSampler::penalties(64, settings.repetition_penalty, 0.0, 0.0),
            LlamaSampler::temp(settings.temperature),
            LlamaSampler::dist(42),
        ])
    };
    let mut sampler = sampler;

    let turn_end_tokens = model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = 500; // max length

    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

        if new_token_id == model.token_eos() || Some(new_token_id) == turn_end_token {
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
pub async fn ask_llm_background(
    prompt: String, 
    app: tauri::AppHandle,
    state: tauri::State<'_, LlamaState>
) -> Result<String, String> {
    let _inference_guard = state.inference_lock.lock().unwrap();
    let model_lock = state.model.lock().unwrap();
    let model = match &*model_lock {
        Some(m) => m,
        None => return Err("Model not loaded.".to_string()),
    };

    let formatted_prompt = format!(
        "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
        SUMMARIZATION_SYSTEM_PROMPT,
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
    let settings = crate::settings::load_settings(app).unwrap_or_default();
    let mut sampler = LlamaSampler::chain_simple([
        LlamaSampler::penalties(64, settings.repetition_penalty, 0.0, 0.0),
        LlamaSampler::greedy(),
    ]);

    let turn_end_tokens = model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = 500; // max length

    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

        if new_token_id == model.token_eos() || Some(new_token_id) == turn_end_token {
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
