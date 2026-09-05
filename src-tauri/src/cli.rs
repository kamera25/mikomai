//! Command-line adapter for the Mikomai runtime.
//!
//! This module intentionally contains no device implementation. It translates
//! CLI arguments into the same intent-oriented application API used by Tauri.

use crate::mcp::fetch::state_resource::StateResource;
use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[derive(Debug, Parser)]
#[command(name = "mikomai", version, about = "Mikomai network runtime CLI")]
pub struct Cli {
    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Rebuild the SurrealDB knowledge base from Markdown source documents.
    RagIngest {
        /// Markdown file or directory. Defaults to ./nw-docs.
        #[arg(default_value = "nw-docs")]
        path: PathBuf,
    },
    /// Search the SurrealDB knowledge base.
    RagSearch {
        /// Natural-language or command query.
        query: String,
    },
    /// Send a message through the same chat pipeline as the desktop app.
    Chat {
        /// Message for Mikomai's network assistant.
        message: String,
    },
    /// List devices registered by the Mikomai desktop app.
    Devices,
    /// List state resources supported by the runtime.
    Resources,
    /// Observe one kind of state on a registered device.
    GetState {
        /// Registered hostname or IP address.
        device: String,
        /// Semantic resource name, such as arp, routes, or interfaces.
        resource: String,
        /// Optional context retained as evidence for resolution/canonicalization.
        #[arg(long)]
        message: Option<String>,
        /// Output only the raw device response (ignored with --json).
        #[arg(long, value_enum, default_value_t = Output::Pretty)]
        output: Output,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum, PartialEq, Eq)]
enum Output {
    Pretty,
    Raw,
}

#[derive(Serialize)]
struct CliResult<T> {
    ok: bool,
    data: T,
}

pub fn run() -> Result<(), String> {
    run_from(Cli::parse())
}

/// `App::run()` executes Tauri's setup hook, whereas CLI commands only build
/// an app handle. Initialise the embedded store explicitly before a CLI RAG
/// command asks the handle for its managed graph state.
fn ensure_graph_state(handle: &tauri::AppHandle) -> Result<(), String> {
    if handle.try_state::<crate::graph::SurrealDbState>().is_some() {
        return Ok(());
    }
    let state = tauri::async_runtime::block_on(crate::graph::SurrealDbState::initialize(handle))?;
    if handle.manage(state) {
        Ok(())
    } else {
        Err("Failed to register SurrealDB state for CLI RAG command".to_string())
    }
}

fn configured_model_path(settings: &crate::settings::AppSettings) -> Result<String, String> {
    settings
        .model_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .map(str::to_owned)
        .ok_or_else(|| {
            "No model is configured. Set a model path in the Mikomai desktop app before using `mikomai-cli chat`."
                .to_string()
        })
}

async fn ensure_cli_model_loaded(
    app: &tauri::AppHandle,
    settings: &crate::settings::AppSettings,
) -> Result<(), String> {
    let state = app.state::<crate::llm::llm::LlamaState>();
    if state.shared.lock().await.is_some() {
        return Ok(());
    }

    let path = configured_model_path(settings)?;
    crate::llm::loader::load_model_internal(app.clone(), path, &state, false)
        .await
        .map(|_| ())
        .map_err(|error| format!("Failed to load the configured model for CLI chat: {error}"))
}

/// Chat still emits GUI timeline events as it runs. Unlike the other CLI
/// commands, it therefore needs Tauri's event loop to initialize that window.
/// The window is immediately hidden; the final response remains stdout-only.
fn run_chat(message: String) -> Result<String, String> {
    let app = crate::build_app().map_err(|error| error.to_string())?;
    let result = Arc::new(Mutex::new(None));
    let result_for_event = result.clone();

    let _ = app.run_return(move |handle, event| match event {
        tauri::RunEvent::Ready => {
            let result = result_for_event.clone();
            let message = message.clone();
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                let response = async {
                    let window = handle.get_window("main").ok_or_else(|| {
                        "The Mikomai chat window was not initialized by the Tauri runtime"
                            .to_string()
                    })?;
                    let _ = window.hide();
                    let settings =
                        crate::settings::load_settings(handle.clone()).unwrap_or_default();
                    ensure_cli_model_loaded(&handle, &settings).await?;
                    let summaries =
                        crate::history::load_summaries(handle.clone()).unwrap_or_default();
                    let llama_state = handle.state::<crate::llm::llm::LlamaState>();
                    let chat_res = crate::mcp::executor::handle_chat_request(
                        handle.clone(),
                        window,
                        &llama_state,
                        crate::mcp::protocol::ChatRequest {
                            user_message: message,
                            summaries,
                            recent_ips: settings.recent_ips,
                            history_limit: settings.history_limit,
                            mcp_timeout: settings.mcp_timeout.unwrap_or(30),
                            attachments: None,
                        },
                    )
                    .await;

                    // Explicitly unload model and contexts so all Metal GPU buffers
                    // are deallocated before the process exits.
                    {
                        let mut shared = llama_state.shared.lock().await;
                        *shared = None;
                        let mut status = llama_state.status.lock().await;
                        *status = crate::llm::llm::ModelState::NotLoaded;
                    }

                    chat_res
                }
                .await;
                *result.lock().expect("CLI chat result lock poisoned") = Some(response);
                handle.exit(0);
            });
        }
        tauri::RunEvent::ExitRequested { .. } => {
            if let Some(state) = handle.try_state::<crate::llm::llm::LlamaState>() {
                if let Ok(mut lock) = state.shared.try_lock() {
                    *lock = None;
                };
            }
        }
        _ => {}
    });

    let response = result
        .lock()
        .expect("CLI chat result lock poisoned")
        .take()
        .ok_or_else(|| "The Tauri event loop exited before the chat command completed".to_string())?;
    response
}

fn run_from(cli: Cli) -> Result<(), String> {
    crate::logger::init().map_err(|error| error.to_string())?;

    match cli.command {
        Command::RagIngest { path } => {
            let app = crate::build_app().map_err(|error| error.to_string())?;
            let handle = app.handle().clone();
            ensure_graph_state(&handle)?;
            let rag = handle.state::<crate::mcp::rag::RagState>();
            let graph = handle.state::<crate::graph::SurrealDbState>();
            let chunks = tauri::async_runtime::block_on(crate::mcp::rag::ingest_path(
                &path, &rag, &graph,
            ))?;
            if cli.json {
                print_json(&CliResult { ok: true, data: serde_json::json!({ "chunks": chunks }) })
            } else {
                println!("Ingested {chunks} knowledge chunks into SurrealDB.");
                Ok(())
            }
        }
        Command::RagSearch { query } => {
            let app = crate::build_app().map_err(|error| error.to_string())?;
            let handle = app.handle().clone();
            ensure_graph_state(&handle)?;
            let state_handle = handle.clone();
            let rag = state_handle.state::<crate::mcp::rag::RagState>();
            let result = tauri::async_runtime::block_on(crate::mcp::rag::query_nw_db(
                query, None, rag, handle,
            ))?;
            if cli.json {
                print_json(&CliResult { ok: result.success, data: result })
            } else {
                print!("{}", result.output);
                Ok(())
            }
        }
        Command::Chat { message } => {
            let response = run_chat(message)?;
            if cli.json {
                print_json(&CliResult {
                    ok: true,
                    data: serde_json::json!({ "response": response }),
                })
            } else {
                print!("{response}");
                if !response.ends_with('\n') {
                    println!();
                }
                Ok(())
            }
        }
        Command::Devices => {
            let app = crate::build_app().map_err(|error| error.to_string())?;
            let handle = app.handle().clone();
            let devices =
                crate::connections::load_connections(handle).map_err(|error| error.to_string())?;
            if cli.json {
                print_json(&CliResult {
                    ok: true,
                    data: devices,
                })
            } else if devices.is_empty() {
                println!("No devices are registered.");
                Ok(())
            } else {
                for device in devices {
                    println!(
                        "{}\t{}\t{}",
                        device.hostname,
                        device.ip_string(),
                        device.conn_type
                    );
                }
                Ok(())
            }
        }
        Command::Resources => {
            let resources = StateResource::valid_resources();
            if cli.json {
                print_json(&CliResult {
                    ok: true,
                    data: resources,
                })
            } else {
                println!("{}", resources.join("\n"));
                Ok(())
            }
        }
        Command::GetState {
            device,
            resource,
            message,
            output,
        } => {
            let app = crate::build_app().map_err(|error| error.to_string())?;
            let handle = app.handle().clone();
            let resource = StateResource::from_str(&resource)?;
            let result =
                tauri::async_runtime::block_on(crate::mcp::fetch::get_state::dispatch_get_state(
                    &handle, &device, resource, message,
                ))?;
            if cli.json {
                print_json(&CliResult {
                    ok: result.success,
                    data: result,
                })
            } else if output == Output::Raw {
                print!("{}", result.output);
                Ok(())
            } else {
                if !result.success {
                    return Err(result.output);
                }
                println!("device: {device}");
                println!("resource: {resource}");
                println!("---");
                print!("{}", result.output);
                if !result.output.ends_with('\n') {
                    println!();
                }
                Ok(())
            }
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<(), String> {
    let output = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
    println!("{output}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_intent_oriented_get_state() {
        let cli = Cli::try_parse_from([
            "mikomai",
            "--json",
            "get-state",
            "edge-01",
            "mac-table",
            "--message",
            "uplink only",
        ])
        .unwrap();
        assert!(cli.json);
        assert!(matches!(
            cli.command,
            Command::GetState { device, resource, .. }
                if device == "edge-01" && resource == "mac-table"
        ));
    }

    #[test]
    fn parses_chat_message() {
        let cli = Cli::try_parse_from(["mikomai", "chat", "show edge-01 interfaces"]).unwrap();
        assert!(matches!(
            cli.command,
            Command::Chat { message } if message == "show edge-01 interfaces"
        ));
    }

    #[test]
    fn requires_a_configured_model_for_chat() {
        let error = configured_model_path(&crate::settings::AppSettings::default()).unwrap_err();
        assert!(error.contains("No model is configured"));
    }

    #[test]
    fn uses_the_configured_model_path_for_chat() {
        let mut settings = crate::settings::AppSettings::default();
        settings.model_path = Some("/models/mikomai.gguf".to_string());
        assert_eq!(
            configured_model_path(&settings).unwrap(),
            "/models/mikomai.gguf"
        );
    }

}
