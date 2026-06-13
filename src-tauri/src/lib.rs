mod llm;
pub mod mcp;
mod network;
pub mod snapshot;
mod history;
mod connections;
pub mod scheduled_tasks;
pub(crate) mod settings;
pub(crate) mod crypto;
pub mod schema;

use tauri::Manager;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let llama_state = llm::LlamaState::new().expect("Failed to initialize Llama backend");
    let rag_state = mcp::rag::RagState::new();

    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let sched_state = scheduled_tasks::init_scheduler(&app_handle).await;
                app_handle.manage(sched_state);
            });

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(llama_state)
        .manage(rag_state)
        .invoke_handler(tauri::generate_handler![
            greet,
            llm::download_model,
            llm::open_model_dir,
            llm::load_model,
            llm::ask_llm,
            llm::ask_llm_background,
            llm::get_model_status,
            mcp::rag::connect_db,
            mcp::rag::ingest_document,
            mcp::rag::query_nw_db,
            network::network_show,
            network::network_config,
            mcp::ping::self_network_ping,
            mcp::traceroute::self_network_traceroute,
            mcp::hosts::network_get_hosts,
            mcp::hosts::require_host_regsterd,
            mcp::ip_info::network_get_ip_info,
            mcp::console::network_list_serial_ports,
            mcp::console::network_send_console_message,
            mcp::fetch_config::fetch_config,
            mcp::fetch_routing::fetch_routing,
            mcp::fetch_arp::fetch_arp,
            history::load_history,
            history::save_history,
            history::load_summaries,
            history::save_summary,
            connections::load_connections,
            connections::save_connections,
            connections::get_mcp_hosts,
            scheduled_tasks::load_scheduled_tasks,
            scheduled_tasks::save_scheduled_tasks,
            scheduled_tasks::add_scheduled_task,
            scheduled_tasks::update_scheduled_task,
            scheduled_tasks::delete_scheduled_task,
            scheduled_tasks::execute_task,
            settings::load_settings,
            settings::save_settings,
            network::dns::resolve_ip,
            mcp::arp::self_network_arp
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::ExitRequested { .. } => {
                let state = app_handle.state::<llm::LlamaState>();
                let mut shared = state.shared.lock().unwrap();
                *shared = None;
                println!("Llama model cleared on exit.");
            }
            _ => {}
        });
}
