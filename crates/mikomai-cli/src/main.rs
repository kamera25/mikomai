use mikomai_adapters::device::JsonDeviceRegistry;
use mikomai_adapters::headless::{EchoToolExecutor, JsonTaskRepository, StdoutReporter};
use mikomai_adapters::knowledge::{KnowledgePlanner, KnowledgeStore};
use mikomai_core::application::ChatService;
use mikomai_core::port::SearchPort;
use mikomai_core::TaskManager;
use std::path::PathBuf;

fn main() {
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

pub fn run(mut args: Vec<String>, json: bool) -> Result<String, String> {
    args.retain(|arg| !matches!(arg.as_str(), "--json" | "-j" | "--debug" | "-d"));
    match args.first().map(String::as_str) {
        Some("chat") => chat(args.into_iter().skip(1).collect::<Vec<_>>().join(" "), json),
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
        Some("resources") => { let resources = ["interfaces", "routes", "arp", "mac-table", "config", "system"]; Ok(if json { serde_json::json!({"ok": true, "data": resources}).to_string() } else { resources.join("\n") }) }
        Some("get-state") => { let device = args.get(1).cloned().ok_or("get-state device is required")?; let resource = args.get(2).cloned().ok_or("get-state resource is required")?; let output = serde_json::json!({"device": device, "resource": resource, "success": false, "error": "No device transport is configured in the standalone CLI"}); if json { Ok(serde_json::json!({"ok": false, "data": output}).to_string()) } else { Err(output["error"].as_str().unwrap_or("get-state failed").into()) } }
        _ => Err("usage: mikomai-cli [--json] <chat|rag-ingest|rag-search|devices|resources|get-state> ...".into()),
    }
}

fn chat(goal: String, json: bool) -> Result<String, String> {
    if goal.trim().is_empty() {
        return Err("chat message is required".into());
    }
    let manager = TaskManager::new(JsonTaskRepository::default());
    let store = knowledge_store();
    let default_documents = PathBuf::from("nw-docs");
    if default_documents.exists() {
        store.ingest(&default_documents)?;
    }
    let planner = KnowledgePlanner::new(&store);
    let executor = EchoToolExecutor;
    let reporter = StdoutReporter::default();
    let task = manager.start(goal).map_err(|error| error.to_string())?;
    let service = ChatService::new(&planner, &executor, &reporter);
    let answer = futures_lite::future::block_on(manager.run_chat(&service, task))?;
    Ok(if json {
        serde_json::json!({"ok": true, "data": {"response": answer}}).to_string()
    } else {
        answer
    })
}

fn knowledge_store() -> KnowledgeStore {
    let root = std::env::var_os("MIKOMAI_KNOWLEDGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mikomai-knowledge"));
    KnowledgeStore::at(root)
}
