use tauri::Emitter;

pub fn is_greeting(query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return false;
    }
    
    let greetings = [
        "こんにちは",
        "こんにちわ",
        "はじめまして",
        "おはよう",
        "おはようございます",
        "こんばんは",
        "お疲れ様です",
        "おつかれさま",
        "お疲れ様",
        "ハロー",
        "はろー",
        "自己紹介",
        "じこしょうかい",
        "hello",
        "hi",
        "hey",
        "who are you",
        "あなたは",
        "あなたは？",
        "あなたは誰",
        "あなたはだれ",
        "お名前は",
        "なまえは",
        "名前は",
        "おなまえは",
    ];

    for g in greetings {
        if q.contains(g) && q.chars().count() <= 20 {
            return true;
        }
    }
    
    let self_intro_keywords = [
        "自己紹介して",
        "自己紹介してください",
        "自己紹介をおねがいします",
        "自己紹介をお願いします",
        "何ができますか",
        "なにができますか",
    ];
    for kw in self_intro_keywords {
        if q.contains(kw) {
            return true;
        }
    }

    false
}

pub async fn stream_self_introduction(window: &tauri::Window) -> String {
    let intro = "はじめまして！私は「MIKOMAI (Managed Infrastructure Knowledge Operator ML Agent Interface)」です。\n\
                 ネットワークインフラの診断、運用、トラブルシューティングを最高精度で支援するプロフェッショナルAIアシスタントです。\n\n\
                 以下のような操作や調査をお手伝いできます：\n\
                 - **ネットワーク機器のステータス確認** (例: `show ip int brief` など)\n\
                 - **疎通確認・調査** (PingやTracerouteの実行)\n\
                 - **ホスト一覧やARPテーブルの取得**\n\
                 - **ネットワーク技術データベース (NW-DB) の検索と解説** (Cisco機器の設定手順など)\n\
                 - **ログの分析とトラブルシューティング**\n\n\
                 何かお手伝いできることはありますか？お気軽に話しかけてください！";

    let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::AgentSelected("MIKOMAI (アシスタント)".to_string()));

    let chars: Vec<char> = intro.chars().collect();
    let chunk_size = 5;
    for chunk in chars.chunks(chunk_size) {
        let chunk_str: String = chunk.iter().collect();
        let _ = window.emit("chat-event", crate::mcp::protocol::ChatEvent::LlmChunk(chunk_str));
        tokio::time::sleep(std::time::Duration::from_millis(15)).await;
    }

    intro.to_string()
}
