use mikomai_adapters::headless::{EchoToolExecutor, JsonTaskRepository, StdoutReporter};
use mikomai_adapters::knowledge::{KnowledgePlanner, KnowledgeStore};
use mikomai_core::application::ChatService;
use mikomai_core::port::SearchPort;
use mikomai_core::TaskManager;
use std::path::PathBuf;

mod logging;
mod debug_trace;

fn main() {
    // GUI and CLI use the same canonical path; an explicit override still wins.
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
        PathBuf::from(home).join("Library/Application Support/MikomaiDesktopMac/surrealdb")
    }))
}

#[cfg(test)]
mod graph_path_tests {
    use super::*;

    #[test]
    fn cli_uses_the_canonical_database() {
        let home = PathBuf::from("/tmp/mikomai-path-test");
        let cli = cli_graph_path(None, Some(home.clone().into_os_string())).unwrap();
        assert_ne!(cli, home.join("Library/Application Support/com.mikomai.agent/surrealdb"));
        assert_eq!(cli, home.join("Library/Application Support/MikomaiDesktopMac/surrealdb"));
    }

    #[test]
    fn explicit_database_override_is_preserved_without_home() {
        let path = PathBuf::from("/tmp/explicit-graph");
        assert_eq!(cli_graph_path(Some(path.clone().into_os_string()), None), Some(path));
        assert_eq!(cli_graph_path(None, None), None);
    }

    #[tokio::test]
    async fn cli_shares_the_canonical_store_while_desktop_owns_it() {
        use mikomai_adapters::portable_graph::PortableGraph;
        let home=std::env::temp_dir().join(format!("mikomai-cli-lock-test-{}",uuid::Uuid::new_v4()));
        let path=cli_graph_path(None,Some(home.into_os_string())).unwrap();
        let desktop=PortableGraph::initialize_at(&path).await.unwrap();
        desktop.save_app_document("settings",&serde_json::json!({"marker":"desktop"})).await.unwrap();
        let cli=PortableGraph::initialize_at(&path).await.unwrap();
        assert_eq!(cli.load_app_document("settings").await.unwrap().unwrap()["marker"],"desktop");
        cli.save_app_document("settings",&serde_json::json!({"marker":"cli"})).await.unwrap();
        assert_eq!(desktop.load_app_document("settings").await.unwrap().unwrap()["marker"],"cli");
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
        Some("contract") => {
            let fixture=std::fs::read_to_string(args.get(1).ok_or("contract fixture required")?).map_err(|e|e.to_string())?;
            let engine=mikomai_app::api::engine();let id=engine.submit(mikomai_app::api::Command::Contract{fixture_json:fixture})?;
            let start=std::time::Instant::now();
            loop {
                let snapshot=engine.query(&id)?;
                if snapshot.state=="failed" {return Err(snapshot.result);}
                if snapshot.state=="completed" {return Ok(serde_json::json!({"version":1,"kinds":snapshot.events.iter().map(|e|&e.kind).collect::<Vec<_>>(),"result":snapshot.result}).to_string());}
                if start.elapsed()>std::time::Duration::from_secs(20) {return Err("contract timeout".into());}
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        Some("task-query") => {
            let snapshot = mikomai_app::api::engine().query(args.get(1).ok_or("task ID required")?)?;
            serde_json::to_string(&snapshot).map_err(|e| e.to_string())
        }
        Some("device-register") => {
            if args.len()!=8{return Err("device-register <id> <name> <host/serial-path> <device-type> <transport> <port> <username>".into());}
            let record=serde_json::json!({"id":args[1],"name":args[2],"host":args[3],"deviceType":args[4],"connectionType":args[5],"port":args[6],"username":args[7]});
            mikomai_app::native_features::register_connection(&record)?;Ok("Device registered".into())
        }
        Some("operation-approve") => {
            mikomai_app::owned_bridge::invoke("mikomai_operation_plan_approve",&[args.get(1).ok_or("plan ID required")?.clone(),args.get(2).ok_or("plan hash required")?.clone()],None)
        }
        Some("operation-execute") => {
            let engine=mikomai_app::api::engine();let id=engine.submit(mikomai_app::api::Command::ExecuteApproved{plan_id:args.get(1).ok_or("plan ID required")?.clone(),plan_hash:args.get(2).ok_or("plan hash required")?.clone()})?;
            wait_task(engine,&id)
        }
        Some("device-show") => {
            let target=args.get(1).ok_or("registered target required")?.clone();let commands=vec![args.get(2).ok_or("show command required")?.clone()];
            let engine=mikomai_app::api::engine();let id=engine.submit(mikomai_app::api::Command::ReadDevice{target,commands,timeout_seconds:60})?;
            wait_task(engine,&id)
        }
        Some("credentials-stdin") => {
            // Secrets are read only from stdin and never accepted in argv/env.
            let mut input=zeroize::Zeroizing::new(String::new());use std::io::Read;
            std::io::stdin().take(64*1024).read_to_string(&mut input).map_err(|_|"credential stdin unavailable")?;
            let mut value:serde_json::Value=serde_json::from_str(&input).map_err(|_|"invalid credential JSON")?;
            let result = mikomai_adapters::secrets::update(args.get(1).ok_or("credential reference required")?,value["password"].as_str(),value["enablePassword"].as_str());
            mikomai_adapters::secrets::wipe(&mut value);
            result?;
            Ok("Credentials updated".into())
        }
        Some("native-query") => {
            let request:serde_json::Value=serde_json::from_str(args.get(1).ok_or("query JSON required")?).map_err(|e|e.to_string())?;
            if request["op"]=="store_save"&&request["collection"]=="connections" {return Err("bulk connection import is not supported; register devices individually".into());}
            if request["op"]=="credential_update" {return Err("use credentials-stdin; secrets must not appear in argv".into());}
            Ok(mikomai_app::native_features::query(&request)?.to_string())
        }
        Some("chat") => chat(args.into_iter().skip(1).collect::<Vec<_>>().join(" "), json, debug_jsonl),
        Some("rag-ingest") => { let path = args.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("nw-docs")); let store = knowledge_store(); let chunks = store.ingest(path)?; Ok(if json { serde_json::json!({"ok": true, "data": {"chunks": chunks}}).to_string() } else { format!("Ingested {chunks} knowledge documents.") }) }
        Some("rag-search") => { let query = args.get(1..).unwrap_or(&[]).join(" "); if query.trim().is_empty() { return Err("rag-search query is required".into()); } let hits = futures_lite::future::block_on(knowledge_store().search(&query, 8))?; Ok(if json { serde_json::json!({"ok": true, "data": hits}).to_string() } else { hits.into_iter().map(|hit| format!("## {}\n{}", hit.title, hit.content)).collect::<Vec<_>>().join("\n\n") }) }
        Some("devices") => {
            let records=mikomai_app::native_features::query(&serde_json::json!({"op":"store_load","collection":"connections"}))?;
            let devices=records.as_array().cloned().unwrap_or_default();
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
                            device["name"].as_str().unwrap_or(""),
                            device["host"].as_str().unwrap_or(""),
                            device["connectionType"].as_str().unwrap_or("")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
        }
        Some("resources") => { let resources = ["interfaces", "routes", "arp", "ndp", "mac-table", "config", "system"]; Ok(if json { serde_json::json!({"ok": true, "data": resources}).to_string() } else { resources.join("\n") }) }
        Some("query-state" | "diff-state") => {
            if args.len()!=2 {return Err("query-state/diff-state requires one JSON argument".into());}
            let input=serde_json::from_str(&args[1]).map_err(|e|e.to_string())?;
            let tool=if args[0]=="query-state" {"query_state"}else{"diff_state"};
            Ok(mikomai_app::native_execution::execute_stored_state(tool,&input)?.to_string())
        }
        Some("get-state") => {
            let device = args.get(1).ok_or("get-state device is required")?;
            let resource = args.get(2).ok_or("get-state resource is required")?;
            let output = mikomai_app::native_execution::execute_tool("get_state", &serde_json::json!({"id":device}), &serde_json::json!({"resource":resource}))?;
            Ok(output)
        }
        _ => Err("usage: mikomai-cli [--json] [--debug|-d] [--debug-jsonl] <chat|rag-ingest|rag-search|devices|resources|get-state|query-state|diff-state> ...".into()),
    }
}

fn wait_task(engine:&mikomai_app::api::Engine,id:&str)->Result<String,String> {
    loop {
        let snapshot=engine.query(id)?;
        match snapshot.state.as_str() {
            "completed"=>return Ok(snapshot.result),"failed"|"cancelled"|"unknown"=>return Err(snapshot.result),
            "awaiting_user"=>{
                eprintln!("{}\n待機を続ける場合は continue、中止する場合は cancel を入力してください。",snapshot.result);
                let mut answer=String::new();std::io::stdin().read_line(&mut answer).map_err(|e|e.to_string())?;
                if answer.trim()=="continue" {engine.resume(id)?;} else {engine.cancel(id)?;}
            },_=>{}
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
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
    let model_configuration = configured_model_path();
    let model_path = model_configuration.as_ref().ok().and_then(|value| value.clone());
    let knowledge = std::env::var_os("MIKOMAI_KNOWLEDGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mikomai-knowledge"));
    let mut trace = debug_trace::Trace::journaled(std::io::stdout(), debug_jsonl);
    trace.emit(
        "cli_request",
        serde_json::json!({
            "query": goal, "history": "", "attachments": "", "devices_json": "[]",
            "mode": if mikomai_core::dispatch::is_stored_state_request(&goal) { "agent" } else if local_ndp { if mikomai_core::network::ndp::is_command(&goal) { "fast_router" } else { "agent" } } else if interface_check.is_some() { "agent" } else if next_hop.is_some() { "agent" } else if local_route.is_some() || port_check.is_some() { "fast_router" } else if local_mac.is_some() { "agent" } else { "worker" }, "documents": docs, "knowledge": knowledge,
            "backend": if greeting_reply.is_some() { "deterministic_reply" } else if local_ndp { "local_ndp" } else if interface_check.is_some() { "native_device_transport" } else if port_check.is_some() { "local_tcp" } else if local_route.is_some() { "local_route" } else if local_mac.is_some() { "local_arp" } else if model_path.is_some() { "local_model" } else { "markdown" }
        }),
    );
    let result = (|| {
        trace.check()?;
        model_configuration?;
        let answer = if let Some(reply) = greeting_reply {
            trace.emit("deterministic_reply", serde_json::json!({"reason":"greeting", "rag":false}));
            trace.stream(&reply, true);
            reply
        } else if local_ndp {
            let args = serde_json::json!({"device":"localhost","resource":"ndp"});
            trace.emit("agent_event", serde_json::json!({"event_type":"tool_call", "tool":"get_state", "target":"localhost", "args":args, "command":"/usr/sbin/ndp -a"}));
            let raw = mikomai_app::native_execution::execute_tool("self_network_ndp", &serde_json::Value::Null, &args)?;
            trace.emit("agent_event", serde_json::json!({"event_type":"observation","tool":"get_state","target":"localhost","args":args,"success":true,"output":raw}));
            let answer = mikomai_core::network::ndp::local_answer(&raw)?;
            trace.stream(&answer, true);
            answer
        } else if let Some((device, interface)) = interface_check {
            let args = serde_json::json!({"resource":"interfaces","interface":interface,"refresh":true});
            trace.emit("agent_event", serde_json::json!({"event_type":"tool_call","tool":"get_state","target":device,"args":args}));
            let output = mikomai_app::native_execution::execute_tool("get_state", &serde_json::json!({"hostname":device}), &args)?;
            trace.emit("agent_event", serde_json::json!({"event_type":"observation","tool":"get_state","target":device,"success":true,"output":output}));
            let answer = mikomai_core::network::interface_check::answer(&device, &interface, &output);
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
            let raw = mikomai_app::native_execution::execute_tool("self_network_route", &serde_json::Value::Null, &route.args)?.trim().to_owned();
            trace.emit(if next_hop.is_some() { "agent_event" } else { "fast_route_result" }, serde_json::json!({"event_type":"observation","target":"localhost","tool":route.tool,"success":true,"output":raw}));
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
            let engine=mikomai_app::api::engine();
            let id=engine.submit(mikomai_app::api::Command::Chat{message:goal.clone(),history:String::new(),documents_dir:docs.to_string_lossy().into(),knowledge_dir:knowledge.to_string_lossy().into(),attachments:String::new(),devices_json:"[]".into(),agent:mikomai_core::dispatch::is_stored_state_request(&goal)})?;
            let mut seq=0;
            loop {
                let snapshot=engine.query(&id)?;
                for event in snapshot.events.iter().filter(|event|event.seq>seq).collect::<Vec<_>>() {
                    let payload:serde_json::Value=serde_json::from_str(&event.payload).unwrap_or(serde_json::Value::Null);
                    if let Some(text)=payload["text"].as_str(){trace.stream(text,payload["done"].as_bool().unwrap_or(false));}
                    else {trace.emit(&event.kind,payload);}
                    seq=event.seq;
                }
                if ["completed","failed","cancelled","unknown","awaiting_user","awaiting_approval"].contains(&snapshot.state.as_str()) {
                    if ["failed","cancelled","unknown"].contains(&snapshot.state.as_str()){return Err(snapshot.result);}
                    break snapshot.result;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
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

fn configured_model_path() -> Result<Option<String>, String> {
    let configured = if let Some(path) = std::env::var_os("MIKOMAI_MODEL_PATH") {
        Some(PathBuf::from(path))
    } else {
        let settings = mikomai_app::native_features::query(&serde_json::json!({
            "op": "store_load", "collection": "settings"
        }))?;
        settings.get("modelPath").and_then(serde_json::Value::as_str).map(PathBuf::from)
    };
    Ok(configured.and_then(|path| {
        let text = path.to_string_lossy();
        let expanded = if let Some(relative) = text.strip_prefix("~/") {
            PathBuf::from(std::env::var_os("HOME")?).join(relative)
        } else { path };
        expanded.is_file().then(|| expanded.to_string_lossy().into_owned())
    }))
}

fn knowledge_store() -> KnowledgeStore {
    let root = std::env::var_os("MIKOMAI_KNOWLEDGE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join("mikomai-knowledge"));
    KnowledgeStore::at(root)
}
