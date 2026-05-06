use llama_cpp_2::llama_backend::LlamaBackend;
use llama_cpp_2::model::LlamaModel;
use llama_cpp_2::context::params::LlamaContextParams;
use llama_cpp_2::llama_batch::LlamaBatch;
use llama_cpp_2::model::AddBos;
use llama_cpp_2::sampling::LlamaSampler;
use std::sync::Mutex;
use std::num::NonZeroU32;
use tauri::Emitter;
use tauri::Manager;
use crate::llm::llm_manager::SharedModel;
use std::sync::Arc;

#[derive(serde::Serialize)]
pub enum ModelState {
    NotLoaded,
    Loading,
    Loaded,
    Error(String),
}

pub struct LlamaState {
    pub shared: Mutex<Option<SharedModel>>,
    pub status: Mutex<ModelState>,
    pub inference_lock: Mutex<()>,
    pub backend: Arc<LlamaBackend>,
}

impl LlamaState {
    pub fn new() -> Result<Self, String> {
        let backend = LlamaBackend::init().map_err(|e| e.to_string())?;
        Ok(Self {
            shared: Mutex::new(None),
            status: Mutex::new(ModelState::NotLoaded),
            inference_lock: Mutex::new(()),
            backend: Arc::new(backend),
        })
    }
}



#[tauri::command]
pub fn get_model_status(state: tauri::State<'_, LlamaState>) -> ModelState {
    let status_lock = match state.status.lock() {
        Ok(lock) => lock,
        Err(_) => return ModelState::Error("Mutex lock poisoned".to_string()),
    };
    match &*status_lock {
        ModelState::NotLoaded => ModelState::NotLoaded,
        ModelState::Loading => ModelState::Loading,
        ModelState::Loaded => ModelState::Loaded,
        ModelState::Error(e) => ModelState::Error(e.clone()),
    }
}

const SYSTEM_PROMPT: &str = r##"あなたは「MIKOMAI (Managed Infrastructure Knowledge Operator ML Agent Interface)」です。
ネットワークインフラを支えるプロフェッショナルなAIアシスタントとして、熟練のネットワークエンジニアの診断、運用、トラブルシューティングを最高精度で支援します。

回答を生成する際は、以下の厳格なルールに従ってください。

# 1. コミュニケーション・スタイルとトンマナ
- 冗長な挨拶、感情的な表現、「提供された情報に基づくと」「次に実行すべきアクション案」などの前置きやメタ的な発言は一切禁止します。常に技術的、簡潔、かつ論理的なトーンを維持してください。
- コマンド、コード、IPアドレスなどは、必ずマークダウンのコードブロック(`)で囲み、視認性を高めてください。
- ツール実行時は、**いかなる提案や理由づけも行わず**、まず日本語で「確認のため、〇〇を実行します。」と1文のみで簡潔にアナウンスし、直後にツール呼び出しのJSONブロックを提示して出力を停止してください。

# 2. 厳格な安全性基準 (Safety & Approval)
- あなたの基本動作は「Read-Only（情報取得）」です。
- ルーターやスイッチに対する「設定変更（Write操作：set, delete, commitなど）」を伴うコマンドをMCP経由で実行しようとする場合は、**絶対に自動で実行せず、必ず事前に以下のフォーマットでユーザーに提示し、明示的な「承認（Approve）」を求めてください**。
  1. 実行する正確なCLIコマンドのリスト
  2. なぜその変更が必要かの理由（Rationale）
  3. 想定される影響範囲

# 3. ツールとエージェント操作 (MCP Rules)
- あなたの基本動作は「Read-Only（情報取得）」です。ステータス確認などのRead-Only操作において、ユーザーへの「提案」や「許可取り」は不要です。推測で語らず、直ちにツールを実行して事実を取得してください。
- ツールを呼び出す場合は、**絶対にリストにある「ツール名」と「引数名」を正確に使用してください。独自にツール名（例: ping等）や引数名（例: target_ip等）を創作することは厳禁です。**
- ツール呼び出しのJSONフォーマットは以下を厳守してください。これ以外の文字をJSONの後に含めないでください。
  `{"tool": "TOOL_NAME", "args": {"ARG_NAME": "VALUE"}}`
- Ping実行後には完結に状況を説明し、対話を終了してください。
- 利用可能なツール:
  1. `network_ping`: 引数 `host` (必須: IPまたはホスト名), `size` (任意: バイトサイズ), `count` (任意: 回数), `df` (任意: フラグメント禁止フラグ, boolean)
  2. `network_traceroute`: 引数 `host` (IPまたはホスト名)
  3. `network_show`: 引数 `command` (Cisco IOS等のコマンド)
  4. `network_get_hosts`: 接続可能なホストの一覧（ホスト名、IP、接続タイプなど）を取得（引数不要）
  5. `query_nw_db`: 特定のベンダー（Cisco等）のコマンド、設定手順、トラブルシューティング手順を回答する前に必ず呼び出すべき検索ツール。LLMの事前知識で回答できる場合でも、正確性担保のためにこのツールを優先して実行すること。**特定のメーカーが判明している場合は、必ず `query` 引数の冒頭に `[Context: メーカー名]` （例: `[Context: Cisco] show run`）を付与してください。この形式を使用する場合、ツール呼び出しの前に「状況把握のため〜」などの挨拶や説明文を一切出力せず、直ちにタグとJSONブロックのみを出力してください。**
  6. `network_arp`: ローカルシステムのARPテーブル（IPとMACアドレスの対応表）を取得します（引数不要）
  7. `network_get_ip_info`: 自分のIPアドレス、IPv6、MACアドレスなどの情報を取得します（引数 verbose: boolean でルーティングやDNSなど詳細情報を取得可能）

# 4. 知識の優先順位とハルシネーションの防止 (RAG Rules)

## 絶対順守事項：事前知識による回答の禁止
- Cisco等の著名なベンダーであっても、LLM自身の事前知識（内部メモリ）に依存した回答は固く禁じます。
- 具体的な機器の操作、コマンド実行手順、トラブルシューティング、仕様の確認に関する回答を行う際は、**必ず事前に `query_nw_db` ツールを実行**し、NW-DBの検索結果をベースに回答を組み立ててください。
- **検索ツール呼び出し時、メーカー名が既知であれば必ず `[Context: メーカー名]` をクエリの先頭に付与し、他の余計な文章を出力しないでください。**

## ツールの使用条件と除外条件
- [必須] 構成変更、機器のステータス確認コマンド、ベンダー特有の仕様確認、手順の立案
- [禁止] PingやTracerouteなどの単純な疎通確認処理、または一般的なネットワークの基礎用語（OSI参照モデルなど）の解説には `query_nw_db` を使用しないでください。

## 検索クエリの最適化（略語の展開）
- ユーザーの入力にネットワーク機器特有の省略形（例: sh, conf t, ter len, int など）が含まれている場合、ツールを実行する前に必ず正式なコマンド名（例: show, configure terminal, terminal length, interface）に脳内で展開してください。
- `query_nw_db` の検索クエリには、省略形ではなく**展開後の正式名称**を使用してください。

## 検索結果の厳格な評価と出力プロセス
LLMは、`query_nw_db` 実行後、以下の手順に従って厳格に情報を処理してください。

### 1. 判定と抽出（フィルタリング）
- ユーザーから機器メーカーやOSの指定があった場合、検索結果の冒頭にある `[Context: (メーカー名/OS名)]` が厳密に一致するか確認してください。
- ユーザーからの具体的な作業依頼（構成変更など）の場合は、上記に加え `[Context: ... operation]` タグが含まれるドキュメントのみを抽出対象としてください。
- 一致しないドキュメント（例: ユーザー指定がFitelnetで、結果が `[Context: Cisco]` の場合など）は**完全に破棄（無視）**してください。

### 2. 抽出結果が「空（ゼロ）」の場合の強制終了
- 上記のフィルタリングの結果、一致するドキュメントが1つも残らなかった場合、「指定された機器の情報はありませんが、代わりに〇〇の情報を〜」といった代替情報の提示や、推測による回答（おせっかい）を**絶対に行わないでください**。
- いかなる技術的な回答や補足もせず、直ちに以下の定型文のみを出力して処理を終了してください。
  定型文：「NW-DBには指定されたメーカー・機器（〇〇）に該当する情報が見つかりません。追加の検索キーワードを指示するか、実機から情報を取得しますか？」（※〇〇にはユーザーが指定した機器名を入れる）

### 3. 抽出結果が存在する場合の回答生成
- 抽出された正しいドキュメントのみをベースに回答を生成してください。
- トラブルシューティングや `[Context: ... operation]` に該当する手順が含まれている場合は、記載された手順に一切の省略を行わず、段階的に回答・実行を進めてください。

### 4. 内部記憶へのアクセス (Internal Memory)
- 入力の末尾に付与される `<memory> ... </memory>` ブロックは、あなた自身の「直近の対話記憶」です。ユーザーからの直接のメッセージではありません。
- 直近の対話記憶は新しい順に上から表示されています。
- ユーザーからの要求に対し、この記憶を暗黙の前提として文脈を補完し、一連の継続した対話として自然な日本語で応答してください。
- 【厳守事項】応答内に「提供された情報によると」「記憶によると」「8.8.8.8|Ping|OK」などのメタ的な前置きや、記憶の生の文字列をそのまま出力することは禁止します。すでに知っている事実として振る舞ってください。

# 5. 期待される対話プロセス（Few-Shot Examples）

User: 192.168.1.1 への疎通が取れない。
MIKOMAI: 確認のため、192.168.1.1へのPingを実行します。
{"tool": "network_ping", "args": {"host": "192.168.1.1"}}

"##;

const SUMMARIZATION_SYSTEM_PROMPT: &str = r##"あなたは「MIKOMAI」の要約担当ユニットです。
入力された対話や実行結果から、重要な「事実」と「実行結果」のみを抽出して要約してください。

# 厳格なルール:
1. **セパレータ（区切り文字）構文**で出力してください。助詞・句読点・自然言語の文法を完全に排除し、「|」や「>」で要素を直接連結します。
   - 基本フォーマット: `対象|操作|結果` または `対象>状態`
2. トークン消費を最小化するため、ステータスは可能な限り**英略語**を使用してください。
   - 推奨ステータス: 正常=`OK`, 異常=`ERR`, 警告=`WARN`, 設定=`CFG`, 追加・更新=`UPD`
   - 出力例: `SRX-01|Ping|OK` / `CoreSW|CPU>WARN` / `OSPF|CFG|UPD` / `Node-A|Routing>ERR`
3. 実行したMCPツールの詳細（ツール名、引数、JSONブロックなど）は要約に含めないでください。
4. 事実と結果以外の解釈、推測、挨拶は一切不要です。
5. 出力は必ず40トークン以内の極小サイズに収めてください。"##;

fn prepare_prompt_tokens(
    model: &LlamaModel,
    prompt: &str,
) -> Result<Vec<llama_cpp_2::token::LlamaToken>, String> {
    let mut tokens = model.str_to_token(prompt, AddBos::Always).map_err(|e| format!("Tokenization error: {:?}", e))?;

    let max_tokens = 2048 - 512;
    if tokens.len() > max_tokens {
        let to_remove = tokens.len() - max_tokens;
        let start_keep = 500;

        if tokens.len() > start_keep + to_remove {
            tokens.drain(start_keep..(start_keep + to_remove));
        } else {
            tokens.truncate(max_tokens);
        }
    }
    Ok(tokens)
}

fn process_token_bytes(
    bytes_accumulator: &mut Vec<u8>,
    result_string: &mut String,
    window: Option<&tauri::Window>,
) {
    match std::str::from_utf8(bytes_accumulator) {
        Ok(s) => {
            if let Some(w) = window {
                let _ = w.emit("llm-chunk", s);
            }
            result_string.push_str(s);
            bytes_accumulator.clear();
        }
        Err(e) => {
            let utf8_error_index = e.valid_up_to();
            let valid_str = String::from_utf8_lossy(&bytes_accumulator[..utf8_error_index]).to_string();
            if let Some(w) = window {
                let _ = w.emit("llm-chunk", &valid_str);
            }
            result_string.push_str(&valid_str);
            bytes_accumulator.drain(..utf8_error_index);
            if bytes_accumulator.len() > 8 {
                 result_string.push_str(&String::from_utf8_lossy(bytes_accumulator));
                 bytes_accumulator.clear();
            }
        }
    }
}

#[tauri::command]
pub async fn ask_llm(
    window: tauri::Window,
    prompt: String,
    llama_state: tauri::State<'_, LlamaState>,
) -> Result<String, String> {
    println!("Received prompt: {}", prompt);

    let _inference_guard = llama_state.inference_lock.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared_lock = llama_state.shared.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared = match &*shared_lock {
        Some(s) => s,
        None => return Err("Model not loaded. Please configure and load a model first.".to_string()),
    };

    let formatted_prompt = format!(
        "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
        SYSTEM_PROMPT,
        prompt
    );

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(2048));

    let mut ctx = shared.model.new_context(&llama_state.backend, ctx_params).map_err(|e| format!("Failed to create context: {:?}", e))?;

    let tokens = prepare_prompt_tokens(&shared.model, &formatted_prompt)?;
    println!("Total tokens in prompt: {}", tokens.len());

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

    let turn_end_tokens = shared.model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = 500; // max length

    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

        if new_token_id == shared.model.token_eos() || Some(new_token_id) == turn_end_token {
            break;
        }

        let mut token_bytes = shared.model.token_to_piece_bytes(new_token_id, 16, false, None).unwrap_or(vec![]);
        bytes_accumulator.append(&mut token_bytes);

        process_token_bytes(&mut bytes_accumulator, &mut result_string, Some(&window));

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
    let _inference_guard = state.inference_lock.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared_lock = state.shared.lock().map_err(|_| "Mutex lock poisoned".to_string())?;
    let shared = match &*shared_lock {
        Some(s) => s,
        None => return Err("Model not loaded.".to_string()),
    };

    let formatted_prompt = format!(
        "<|turn>system\n{}<turn|>\n<|turn>user\n{}<turn|>\n<|turn>model\n",
        SUMMARIZATION_SYSTEM_PROMPT,
        prompt
    );

    let mut ctx_params = LlamaContextParams::default();
    ctx_params = ctx_params.with_n_ctx(NonZeroU32::new(2048));

    let mut ctx = shared.model.new_context(&state.backend, ctx_params).map_err(|e| format!("Failed to create context: {:?}", e))?;

    let tokens = prepare_prompt_tokens(&shared.model, &formatted_prompt)?;

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

    let turn_end_tokens = shared.model.str_to_token("<turn|>", AddBos::Never).unwrap_or_default();
    let turn_end_token = turn_end_tokens.first().copied();

    let n_len = 500; // max length

    let mut bytes_accumulator = Vec::new();

    for _ in 0..n_len {
        let new_token_id = sampler.sample(&mut ctx, batch.n_tokens() - 1);

        if new_token_id == shared.model.token_eos() || Some(new_token_id) == turn_end_token {
            break;
        }

        let mut token_bytes = shared.model.token_to_piece_bytes(new_token_id, 16, false, None).unwrap_or(vec![]);
        bytes_accumulator.append(&mut token_bytes);

        process_token_bytes(&mut bytes_accumulator, &mut result_string, None);

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
