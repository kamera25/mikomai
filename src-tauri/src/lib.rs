mod llm;
mod rag;
mod network;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let llama_state = llm::LlamaState::new().expect("Failed to initialize Llama backend");
    let rag_state = rag::RagState::new();

    let mcp_state = network::McpState {
        process: std::sync::Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(llama_state)
        .manage(rag_state)
        .manage(mcp_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            llm::download_model,
            llm::load_model,
            rag::connect_db,
            rag::ingest_document,
            rag::query_rag,
            network::network_show,
            network::network_config,
            network::start_ns_mcp_server,
            network::send_mcp_message
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
