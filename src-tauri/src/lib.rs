mod llm;
mod rag;
mod network;
mod mcp_network;
mod history;
mod connections;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let llama_state = llm::LlamaState::new().expect("Failed to initialize Llama backend");
    let rag_state = rag::RagState::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .manage(llama_state)
        .manage(rag_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            llm::download_model,
            llm::load_model,
            llm::ask_llm,
            llm::ask_llm_background,
            llm::get_model_status,
            rag::connect_db,
            rag::ingest_document,
            rag::query_rag,
            network::network_show,
            network::network_config,
            mcp_network::network_ping,
            mcp_network::network_traceroute,
            mcp_network::network_get_hosts,
            history::load_history,
            history::save_history,
            history::load_summaries,
            history::save_summary,
            connections::load_connections,
            connections::save_connections,
            connections::get_mcp_hosts
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
