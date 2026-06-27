mod llm;
pub(crate) mod mcp;
mod network;
pub(crate) mod snapshot;
mod history;
mod connections;
pub(crate) mod scheduled_tasks;
pub(crate) mod settings;
pub(crate) mod crypto;
pub(crate) mod schema;
mod logger;
pub(crate) mod error;


use tauri::Manager;


#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logger::init().expect("Failed to initialize logger");
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
        .manage(mcp::config_helper::ChoiceManager::new())
        .manage(mcp::config_helper::InterfaceChoiceManager::new())
        .invoke_handler(tauri::generate_handler![
            llm::download_model,
            llm::open_model_dir,
            llm::load_model,
            llm::ask_llm_initial,
            llm::analyze_tool_output,
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
            mcp::hosts::require_host_registered,
            mcp::ip_info::network_get_ip_info,
            mcp::console::network_list_serial_ports,
            mcp::console::network_send_console_message,
            mcp::fetch::fetch_config::fetch_config,
            mcp::fetch::fetch_routing::fetch_routing,
            mcp::fetch::fetch_arp::fetch_arp,
            history::load_history,
            history::save_history,
            history::load_summaries,
            history::save_summary,
            connections::load_connections,
            connections::get_mcp_hosts,
            connections::save_connections,
            connections::get_device_types,
            scheduled_tasks::load_scheduled_tasks,
            scheduled_tasks::save_scheduled_tasks,
            scheduled_tasks::add_scheduled_task,
            scheduled_tasks::update_scheduled_task,
            scheduled_tasks::delete_scheduled_task,
            scheduled_tasks::execute_task,
            settings::load_settings,
            settings::save_settings,
            network::dns::resolve_ip,
            mcp::arp::self_network_arp,
            mcp::route::self_network_route,
            mcp::executor::execute_mcp_tool,
            mcp::executor::handle_mcp_message,
            mcp::nwdiag::self_network_nwdiag,
            mcp::config_helper::validate_cisco_config,
            mcp::config_helper::convert_cisco_config,
            mcp::config_helper::submit_user_choice,
            mcp::config_helper::ask_user_choice,
            mcp::config_helper::submit_interface_choice,
            mcp::config_helper::ask_interface_choice
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::ExitRequested { .. } => {
                let state = app_handle.state::<llm::LlamaState>();
                let status = state.status.blocking_lock();
                if let llm::ModelState::Loading = *status {
                    log::info!("Exiting while model is loading; using fast exit to prevent crash.");
                    #[cfg(unix)]
                    unsafe {
                        extern "C" {
                            fn _exit(status: std::os::raw::c_int) -> !;
                        }
                        _exit(0);
                    }
                    #[cfg(windows)]
                    unsafe {
                        extern "system" {
                            fn ExitProcess(uExitCode: u32) -> !;
                        }
                        ExitProcess(0);
                    }
                } else {
                    let mut shared = state.shared.blocking_lock();
                    *shared = None;
                    log::info!("Llama model cleared on exit.");
                }
            }
            _ => {}
        });
}
