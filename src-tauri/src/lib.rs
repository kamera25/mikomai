pub mod audit;
pub(crate) mod background_work;
pub mod cli;
mod connections;
pub(crate) mod crypto;
pub(crate) mod error;
pub(crate) mod graph;
pub mod harness;
mod history;
mod history_store;
mod llm;
mod logger;
pub(crate) mod mcp;
mod network;
pub(crate) mod node_refresh;
pub mod operations;
pub mod planner;
pub(crate) mod schema;
pub(crate) mod settings;
pub(crate) mod snapshot;
pub mod state;
pub mod task_audit;
pub mod validator;
pub(crate) mod watch;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logger::init().expect("Failed to initialize logger");
    build_app()
        .expect("error while building tauri application")
        .run(|app_handle, event| match event {
            tauri::RunEvent::ExitRequested { .. } => {
                let history_app_handle = app_handle.clone();
                let _ = tauri::async_runtime::block_on(async move {
                    history::cleanup_running_history_on_exit(history_app_handle).await
                });
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

pub(crate) fn build_app() -> tauri::Result<tauri::App> {
    let llama_state = llm::LlamaState::new().expect("Failed to initialize Llama backend");
    let rag_state = mcp::rag::RagState::new();

    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle().clone();
            let operation_store = operations::OperationStore::load(&app_handle)
                .expect("Failed to initialize operation-plan storage");
            app_handle.manage(operation_store);
            tauri::async_runtime::block_on(async move {
                let watch_state = watch::init_watch_scheduler(&app_handle).await;
                app_handle.manage(watch_state);
                let graph_state = graph::SurrealDbState::initialize(&app_handle)
                    .await
                    .expect("Failed to initialize embedded SurrealDB");
                history_store::initialize(&graph_state)
                    .await
                    .expect("Failed to initialize chat history storage");
                app_handle.manage(graph_state);
            });

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(llama_state)
        .manage(background_work::BackgroundWorkState::default())
        .manage(rag_state)
        .manage(mcp::config_helper::ChoiceManager::new())
        .manage(mcp::config_helper::InterfaceChoiceManager::new())
        .manage(mcp::config_helper::IpAddressChoiceManager::new())
        .invoke_handler(tauri::generate_handler![
            llm::download_model,
            llm::check_model_exists,
            llm::open_model_dir,
            llm::open_path_in_file_manager,
            llm::copy_file_to_destination,
            llm::load_model,
            llm::ask_llm_initial,
            llm::analyze_tool_output,
            llm::ask_llm_background,
            llm::get_model_status,
            llm::stop_llm,
            mcp::rag::ingest_document,
            mcp::rag::query_nw_db,
            network::network_show,
            mcp::ping::self_network_ping,
            mcp::traceroute::self_network_traceroute,
            mcp::test_connection::self_network_test_connection,
            mcp::hosts::network_get_hosts,
            mcp::hosts::require_host_registered,
            mcp::ip_info::network_get_ip_info,
            mcp::console::network_list_serial_ports,
            mcp::console::network_send_console_message,
            mcp::fetch::fetch_config::fetch_config,
            mcp::fetch::fetch_routing::fetch_routing,
            mcp::fetch::fetch_arp::fetch_arp,
            mcp::fetch::get_state::get_state,
            graph::query_network_graph,
            node_refresh::start_node_db_bulk_refresh,
            history::load_history,
            history::save_history,
            history::mutate_history,
            history::initialize_history,
            history::load_summaries,
            history::save_summary,
            history::read_files_as_attachments,
            history::prepare_attachments,
            connections::load_connections,
            connections::get_mcp_hosts,
            connections::save_connections,
            connections::import_connections_csv,
            connections::export_connections_csv,
            connections::get_device_types,
            watch::create_watch,
            watch::list_watches,
            watch::get_watch,
            watch::update_watch,
            watch::delete_watch,
            watch::enable_watch,
            watch::disable_watch,
            watch::execute_watch_now,
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
            operations::create_operation_plan,
            operations::create_network_config_operation_plan,
            operations::get_operation_plan,
            operations::approve_operation_plan,
            operations::execute_approved_operation_plan,
            task_audit::list_agent_tasks,
            task_audit::get_agent_task_audit,
            task_audit::resume_agent_task,
            mcp::config_helper::ask_user_choice,
            mcp::config_helper::submit_interface_choice,
            mcp::config_helper::ask_interface_choice,
            mcp::config_helper::submit_ipaddress_choice,
            mcp::config_helper::ask_ipaddress_choice,
            mcp::ftp::network_ftp_download,
            mcp::ftp::network_ftp_upload,
            mcp::tftp::network_tftp_download,
            mcp::tftp::network_tftp_upload
        ])
        .build(tauri::generate_context!())
}
