use mikomai_adapters::device::JsonDeviceRegistry;
use mikomai_adapters::headless::{EchoToolExecutor, JsonTaskRepository, StdoutReporter};
use mikomai_adapters::knowledge::{KnowledgePlanner, KnowledgeStore};
use mikomai_core::application::ChatService;
use mikomai_core::port::SearchPort;
use mikomai_core::TaskManager;
use std::path::PathBuf;

mod logging;
mod debug_trace;

fn main() {
    // Embedded RocksDB cannot be shared with the desktop process. Keep the
    // CLI's rebuildable RAG index separate; an explicit override still wins.
    if let Some(path) = cli_graph_path(std::env::var_os("MIKOMAI_GRAPH_DB_PATH"), std::env::var_os("HOME")) {
        std::env::set_var("MIKOMAI_GRAPH_DB_PATH", path);
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let json = args.iter().any(|arg| arg == "--json" || arg == "-j");
    match run(args, json) {
        Ok(output) => {
            if !output.is_empty() {
                println!("{output}");
            }
        }
        Err(error) => {
            eprintln!("mikomai-cli: {error}");
            std::process::exit(1);
        }
    }
}

fn cli_graph_path(explicit: Option<std::ffi::OsString>, home: Option<std::ffi::OsString>) -> Option<PathBuf> {
    explicit.map(PathBuf::from).or_else(|| home.map(|home| {
        PathBuf::from(home).join("Library/Application Support/MikomaiCLI/surrealdb")
    }))
}

#[cfg(test)]
mod graph_path_tests {
    use super::*;

    #[test]
    fn cli_does_not_claim_the_desktop_database() {
        let home = PathBuf::from("/tmp/mikomai-path-test");
        let cli = cli_graph_path(None, Some(home.clone().into_os_string())).unwrap();
        assert_ne!(cli, home.join("Library/Application Support/com.mikomai.agent/surrealdb"));
        assert_ne!(cli, home.join("Library/Application Support/MikomaiDesktopMac/surrealdb"));
        assert_eq!(cli, home.join("Library/Application Support/MikomaiCLI/surrealdb"));
    }

    #[test]
    fn explicit_database_override_is_preserved_without_home() {
        let path = PathBuf::from("/tmp/explicit-graph");
        assert_eq!(cli_graph_path(Some(path.clone().into_os_string()), None), Some(path));
        assert_eq!(cli_graph_path(None, None), None);
    }

    #[tokio::test]
    async fn cli_can_open_its_index_while_desktop_holds_a_lock() {
        use mikomai_adapters::portable_graph::PortableGraph;
        let home = std::env::temp_dir().join(format!("mikomai-cli-lock-test-{}", std::process::id()));
        let desktop_path = home.join("Library/Application Support/com.mikomai.agent/surrealdb");
        let _desktop = PortableGraph::initialize_at(&desktop_path).await.unwrap();
        // Reproduce the original conflict before checking the separated path.
        let duplicate = PortableGraph::initialize_at(&desktop_path).await;
        assert!(matches!(duplicate, Err(ref error) if error.contains("lock") || error.contains("LOCK")));
        let cli_path = cli_graph_path(None, Some(home.into_os_string())).unwrap();
        let _cli = PortableGraph::initialize_at(&cli_path).await.unwrap();
    }
}

pub fn run(mut args: Vec<String>, json: bool) -> Result<String, String> {
    logging::configure(args.iter().any(|arg| matches!(arg.as_str(), "--debug" | "-d")));
    let debug_jsonl = args.iter().any(|arg| arg == "--debug-jsonl");
    args.retain(|arg| !matches!(arg.as_str(), "--json" | "-j" | "--debug" | "-d" | "--debug-jsonl"));
    if debug_jsonl && json {
        return Err("--debug-jsonl cannot be combined with --json/-j".into());
    }
    if debug_jsonl && args.first().map(String::as_str) != Some("chat") {
        return Err("--debug-jsonl is only supported for chat".into());
    }
    match args.first().map(String::as_str) {
        Some("chat") => chat(args.into_iter().skip(1).collect::<Vec<_>>().join(" "), json, debug_jsonl),
        Some("rag-ingest") => { let path = args.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("nw-docs")); let store = knowledge_store(); let chunks = store.ingest(path)?; Ok(if json { serde_json::json!({"ok": true, "data": {"chunks": chunks}}).to_string() } else { format!("Ingested {chunks} knowledge documents.") }) }
        Some("rag-search") => { let query = args.get(1..).unwrap_or(&[]).join(" "); if query.trim().is_empty() { return Err("rag-search query is required".into()); } let hits = futures_lite::future::block_on(knowledge_store().search(&query, 8))?; Ok(if json { serde_json::json!({"ok": true, "data": hits}).to_string() } else { hits.into_iter().map(|hit| format!("## {}\n{}", hit.title, hit.content)).collect::<Vec<_>>().join("\n\n") }) }
        Some("devices") => {
            let devices = JsonDeviceRegistry::from_env().list()?;
            Ok(if json {
                serde_json::json!({"ok": true, "data": devices}).to_string()
            } else if devices.is_empty() {
                "No devices are registered.".into()
            } else {
                devices
                    .into_iter()
                    .map(|device| {
                        format!(
                            "{}\t{}\t{}",
                            device.hostname,
                            device.ip.unwrap_or_default(),
                            device.connection_type.unwrap_or_default()
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
        Some("resources") => { let resources = ["interfaces", "routes", "arp", "ndp", "mac-table", "config", "system"]; Ok(if json { serde_json::json!({"ok": true, "data": resources}).to_string() } else { resources.join("\n") }) }
        Some("get-state") => { let device = args.get(1).cloned().ok_or("get-state device is required")?; let resource = args.get(2).cloned().ok_or("get-state resource is required")?; let output = serde_json::json!({"device": device, "resource": resource, "success": false, "error": "No device transport is configured in the standalone CLI"}); if json { Ok(serde_json::json!({"ok": false, "data": output}).to_string()) } else { Err(output["error"].as_str().unwrap_or("get-state failed").into()) } }
        _ => Err("usage: mikomai-cli [--json] [--debug|-d] [--debug-jsonl] <chat|rag-ingest|rag-search|devices|resources|get-state> ...".into()),
    }
}

fn chat(goal: String, json: bool, debug_jsonl: bool) -> Result<String, String> {
    if goal.trim().is_empty() {
        return Err("chat message is required".into());
    }
    let docs = std::env::var_os("MIKOMAI_DOCS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("nw-docs"));
    let port_check = mikomai_core::dispatch::fast_route(&goal)
        .filter(|route| route.tool.as_deref() == Some("self_network_test_connection"));
    let next_hop = mikomai_core::dispatch::local_next_hop_shortcut(&goal);
    let local_route = next_hop.clone().or_else(|| mikomai_core::dispatch::local_route_shortcut(&goal));
    let local_ndp = mikomai_core::network::ndp::is_local_request(&goal);
    let local_mac = mikomai_core::dispatch::local_arp_mac_target(&goal);
    let interface_check = mikomai_core::network::interface_check::request(&goal);
    let greeting_reply = mikomai_core::dispatch::legacy_shortcut(&goal).and_then(|s| s.reply);
    let model_path = configured_model_path();
    let knowledge = std::env::var_os("MIKOMAI_KNOWLEDGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mikomai-knowledge"));
    let mut trace = debug_trace::Trace::new(std::io::stdout(), debug_jsonl);
    trace.emit(
        "cli_request",
        serde_json::json!({
            "query": goal, "history": "", "attachments": "", "devices_json": "[]",
            "mode": if local_ndp { if mikomai_core::network::ndp::is_command(&goal) { "fast_router" } else { "agent" } } else if interface_check.is_some() { "agent" } else if next_hop.is_some() { "agent" } else if local_route.is_some() || port_check.is_some() { "fast_router" } else if local_mac.is_some() { "agent" } else { "worker" }, "documents": docs, "knowledge": knowledge,
            "backend": if greeting_reply.is_some() { "deterministic_reply" } else if local_ndp { "local_ndp" } else if interface_check.is_some() { "native_device_transport_unavailable" } else if port_check.is_some() { "local_tcp" } else if local_route.is_some() { "local_route" } else if local_mac.is_some() { "local_arp" } else if model_path.is_some() { "local_model" } else { "markdown" }
        }),
    );
    let result = (|| {
        let answer = if let Some(reply) = greeting_reply {
            trace.emit("deterministic_reply", serde_json::json!({"reason":"greeting", "rag":false}));
            trace.stream(&reply, true);
            reply
        } else if local_ndp {
            let args = serde_json::json!({"device":"localhost","resource":"ndp"});
            trace.emit("agent_event", serde_json::json!({"event_type":"tool_call", "tool":"get_state", "target":"localhost", "args":args, "command":"/usr/sbin/ndp -a"}));
            #[cfg(target_os = "macos")]
            let output = std::process::Command::new("/usr/sbin/ndp").arg("-a").output();
            #[cfg(not(target_os = "macos"))]
            let output: std::io::Result<std::process::Output> = Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "NDP observation is supported only on macOS"));
            let output = output.map_err(|error| format!("自機のNDP取得に失敗しました: {error}"))?;
            let raw = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            trace.emit("agent_event", serde_json::json!({"event_type":"observation", "tool":"get_state", "target":"localhost", "args":args, "command":"/usr/sbin/ndp -a", "success":output.status.success(), "exit_code":output.status.code(), "output":raw, "stderr":stderr}));
            if !output.status.success() { return Err(format!("自機のNDP取得に失敗しました: {stderr}")); }
            let answer = mikomai_core::network::ndp::local_answer(&raw)?;
            trace.stream(&answer, true);
            answer
        } else if interface_check.is_some() {
            // Standalone CLI has no native Keychain/device callback. Do not
            // manufacture a live observation using the text-only Worker.
            trace.emit("agent_event", serde_json::json!({"event_type":"awaiting_input","reason":"native_device_transport_required","intent":"interface_up","request":interface_check}));
            let answer = "このCLIには登録済み機器への接続経路がありません。Mikomaiアプリのチャットで同じ依頼を実行してください。実機のup/downはまだ確認していません。".to_string();
            trace.stream(&answer, true);
            answer
        } else if let Some(check) = port_check {
            trace.emit("fast_route", serde_json::json!({"event_type":"tool_call","tool":check.tool,"target":"localhost","args":check.args}));
            let result = mikomai_app::test_tcp_connection_core(check.args["host"].as_str().unwrap(), check.args["port"].as_u64().unwrap() as u16, 3000);
            let success = result.is_ok();
            let answer = result.unwrap_or_else(|error| format!("TCP接続を確認できませんでした。{error}\n接続失敗だけではポート閉鎖と経路・フィルタによる遮断を区別できません。"));
            trace.emit("fast_route_result", serde_json::json!({"event_type":"observation","tool":check.tool,"target":"localhost","success":success,"output":answer}));
            trace.stream(&answer, true);
            answer
        } else if let Some(route) = local_route {
            trace.emit(if next_hop.is_some() { "agent_event" } else { "fast_route" }, serde_json::json!({"event_type":"tool_call", "tool":route.tool, "target":"localhost", "args":route.args}));
            let table = route.args["scope"] == "table";
            #[cfg(target_os = "macos")]
            let output = if let Some(destination) = route.args["destination"].as_str() {
                let mut command = std::process::Command::new("/sbin/route");
                command.args(["-n", "get"]);
                if destination.contains(':') { command.arg("-inet6"); }
                command.arg(destination).output()
            } else if table {
                std::process::Command::new("/usr/sbin/netstat").args(["-rn"]).output()
            } else {
                std::process::Command::new("/sbin/route").args(["-n", "get", "default"]).output()
            };
            #[cfg(target_os = "linux")]
            let output = if let Some(destination) = route.args["destination"].as_str() {
                std::process::Command::new("ip").args(["route", "get", destination]).output()
            } else if table {
                std::process::Command::new("ip").args(["route", "show", "table", "all"]).output()
            } else {
                std::process::Command::new("ip").args(["route", "show", "default"]).output()
            };
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            let output: std::io::Result<std::process::Output> = Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported, "Local routing is supported on macOS and Linux",
            ));
            let output = output.map_err(|error| format!("自機の経路の取得に失敗しました: {error}"))?;
            let raw = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr);
            trace.emit(if next_hop.is_some() { "agent_event" } else { "fast_route_result" }, serde_json::json!({"event_type":"observation", "target":"localhost", "tool":route.tool, "success":output.status.success(), "output":raw, "stderr":stderr}));
            if !output.status.success() {
                return Err(format!("自機の経路の取得に失敗しました: {stderr}"));
            }
            if raw.is_empty() {
                return Err("自機の経路の取得結果が空です。".into());
            }
            let answer = mikomai_core::network::route::local_route_answer(
                route.args["destination"].as_str().or_else(|| route.args["scope"].as_str()).unwrap_or("default"), &raw,
            );
            trace.stream(&answer, true);
            answer
        } else if let Some(mac) = local_mac {
            let args = serde_json::json!({"device":"localhost", "resource":"arp", "mac":mac});
            trace.emit("agent_event", serde_json::json!({"event_type":"tool_call", "tool":"get_state", "target":"localhost", "args":args}));
            if let Some(path) = model_path.as_deref() { mikomai_app::load_local_model(path)?; }
            let table = mikomai_app::local_arp_state()?;
            trace.emit("agent_event", serde_json::json!({"event_type":"observation", "tool":"get_state", "target":"localhost", "success":true, "output":table}));
            let answer = mikomai_core::network::arp::mac_lookup_answer("localhost", &mac, &table.to_string());
            trace.stream(&answer, true);
            answer
        } else if let Some(path) = model_path {
            mikomai_app::load_local_model(&path)?;
            if debug_jsonl {
                mikomai_app::local_model_chat_with_callback(
                    &goal,
                    "",
                    &docs.to_string_lossy(),
                    &knowledge.to_string_lossy(),
                    |text, done| trace.stream(text, done),
                )?
            } else {
                mikomai_app::local_model_chat(
                    &goal,
                    "",
                    &docs.to_string_lossy(),
                    &knowledge.to_string_lossy(),
                )?
            }
        } else if !mikomai_core::reference_context::needs_selected_references(&goal) {
            trace.emit("reference_context", serde_json::json!({"prefetch":false,"characters":0,"policy":"without_references"}));
            let answer = "会話を続けるには、設定画面で言語モデルを選択してください。".to_string();
            trace.stream(&answer, true);
            answer
        } else {
            // Keep a deterministic, explicitly non-generative fallback for headless
            // installations without a local model configured.
            let manager = TaskManager::new(JsonTaskRepository::default());
            let task = manager
                .start(goal.clone())
                .map_err(|error| error.to_string())?;
            let store = knowledge_store();
            if docs.exists() {
                store.ingest(&docs)?;
            }
            let planner = KnowledgePlanner::new(&store);
            let executor = EchoToolExecutor;
            let reporter = StdoutReporter::default();
            let service = ChatService::new(&planner, &executor, &reporter);
            let answer = futures_lite::future::block_on(manager.run_chat(&service, task));
            if debug_jsonl {
                if let Ok(events) = reporter.events.lock() {
                    for event in events.iter() {
                        trace.report(event);
                    }
                }
            }
            let answer = answer?;
            trace.stream(&answer, true);
            answer
        };
        Ok::<_, String>(answer)
    })();
    match &result {
        Ok(answer) => trace.emit(
            "core_response",
            serde_json::json!({"status": 0, "text": answer}),
        ),
        Err(error) => trace.emit(
            "core_response",
            serde_json::json!({"status": 1, "text": error}),
        ),
    }
    trace.finish()?;
    let answer = result?;
    Ok(if debug_jsonl {
        String::new()
    } else if json {
        serde_json::json!({"ok": true, "data": {"response": answer}}).to_string()
    } else {
        answer
    })
}

fn configured_model_path() -> Option<String> {
    if let Some(path) = std::env::var_os("MIKOMAI_MODEL_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path.to_string_lossy().into_owned());
        }
    }
    let home = std::env::var_os("HOME").map(PathBuf::from)?;
    let candidates = [
        std::env::var_os("MIKOMAI_SETTINGS_PATH").map(PathBuf::from),
        Some(home.join("Library/Application Support/MikomaiDesktopMac/settings.json")),
        Some(home.join("Library/Application Support/com.mikomai.agent/settings.json")),
        Some(home.join("Library/Application Support/mikomai/settings.json")),
        Some(home.join(".config/mikomai/settings.json")),
    ];
    for path in candidates.into_iter().flatten() {
        let Ok(contents) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(settings) = serde_json::from_str::<serde_json::Value>(&contents) else {
            continue;
        };
        let Some(model) = settings
            .get("modelPath")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let path = PathBuf::from(model.replace("~", &home.to_string_lossy()));
        if path.is_file() {
            return Some(path.to_string_lossy().into_owned());
        }
    }
    None
}

fn knowledge_store() -> KnowledgeStore {
    let root = std::env::var_os("MIKOMAI_KNOWLEDGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mikomai-knowledge"));
    KnowledgeStore::at(root)
}
