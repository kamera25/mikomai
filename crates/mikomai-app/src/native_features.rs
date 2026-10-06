//! Portable policies formerly owned by Swift. JSON is a temporary ABI envelope;
//! inputs and outputs retain UI DTO fields until the UniFFI migration.
use serde_json::{json, Value};

pub fn query(request: &Value) -> Result<Value, String> {
    let text = |key: &str| request[key].as_str().unwrap_or("");
    Ok(match text("op") {
        "public_ip" => json!(public_ip(text("value"))),
        "cpu_command" => json!(crate::native_execution::cpu_command(
            &json!({"deviceType":canonical_device_id(text("value"))})
        )),
        "cpu_usage" => json!(crate::native_execution::cpu_usage(text("value")).ok()),
        "arp_command" => json!(crate::native_execution::arp_command(
            &json!({"deviceType":canonical_device_id(text("value"))})
        )),
        "interface_command" => json!(interface_command(
            text("deviceType"),
            request["interface"].as_str()
        )?),
        "ping_parse" => parse_ping(text("value")),
        "ping_arguments" => ping_arguments(&request["command"]),
        "network_error" => network_error(text("value")),
        "diagnostic_host" => match diagnostic_host(text("host"), &request["connections"]) {
            Ok(host) => json!({"host":host}),
            Err(error) => json!({"error":error}),
        },
        "credential_update" => {
            mikomai_adapters::secrets::update(
                text("id"),
                request["password"].as_str(),
                request["enablePassword"].as_str(),
            )?;
            Value::Null
        }
        "device_snapshot" => {
            crate::native_execution::snapshot(&crate::native_execution::connection(text("id"))?)?
        }
        "native_tool" => json!(crate::native_execution::execute_tool(
            text("tool"),
            &request["target"],
            &request["args"]
        )?),
        "operation_prepare" => {
            let connection = crate::native_execution::connection(text("id"))?;
            let snapshot =
                std::ffi::CString::new(crate::native_execution::snapshot(&connection)?.to_string())
                    .map_err(|e| e.to_string())?;
            let target =
                std::ffi::CString::new(connection["name"].as_str().ok_or("device name missing")?)
                    .map_err(|e| e.to_string())?;
            let commands: Vec<&str> = text("proposal")
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            let commands =
                std::ffi::CString::new(json!(commands).to_string()).map_err(|e| e.to_string())?;
            let rationale = std::ffi::CString::new(text("rationale")).map_err(|e| e.to_string())?;
            serde_json::from_str(&unsafe {
                crate::consume_result(crate::mikomai_operation_plan_create(
                    target.as_ptr(),
                    snapshot.as_ptr(),
                    commands.as_ptr(),
                    rationale.as_ptr(),
                ))?
            })
            .map_err(|e| e.to_string())?
        }
        "store_load" => crate::shared_service()
            .load_document(text("collection"))?
            .unwrap_or(Value::Null),
        "store_save" => {
            crate::shared_service().save_document(text("collection"), &request["value"])?;
            Value::Null
        }
        "device_catalog" => {
            serde_json::from_str(include_str!("device_catalog.json")).map_err(|e| e.to_string())?
        }
        "device_id" => json!(canonical_device_id(text("value"))),
        "model_presets" => {
            serde_json::from_str(include_str!("model_presets.json")).map_err(|e| e.to_string())?
        }
        "connection" => {
            let connection = &request["connection"];
            let kind = connection["connectionType"]
                .as_str()
                .unwrap_or("SSH")
                .to_lowercase();
            let default = if kind == "telnet" { "23" } else { "22" };
            let port = connection["port"].as_str().unwrap_or("");
            let base = text("base");
            let driver = if kind == "telnet" && !base.ends_with("_telnet") {
                format!("{}_telnet", base.strip_suffix("_ssh").unwrap_or(base))
            } else {
                base.to_owned()
            };
            json!({"error":validate_connection(connection), "defaultPort":default,
                "effectivePort":if port.is_empty() { default } else { port }, "driver":driver})
        }
        "connection_select_type" => {
            let mut connection = request["connection"].clone();
            let old_default = if connection["connectionType"]
                .as_str()
                .unwrap_or("SSH")
                .eq_ignore_ascii_case("telnet")
            {
                "23"
            } else {
                "22"
            };
            let port = connection["port"].as_str().unwrap_or("");
            let uses_default = port.is_empty() || port == old_default;
            connection["connectionType"] = json!(text("type"));
            if uses_default {
                connection["port"] = json!(if text("type").eq_ignore_ascii_case("telnet") {
                    "23"
                } else {
                    "22"
                });
            }
            connection
        }
        "credential_flags" => {
            let mut connection = request["connection"].clone();
            for (field, flag) in [
                ("hasPassword", "passwordPresent"),
                ("hasEnablePassword", "enablePresent"),
            ] {
                if let Some(value) = request[flag].as_bool() {
                    connection[field] = json!(value);
                }
            }
            connection
        }
        "legacy_locations" => {
            let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
            let paths = home
                .into_iter()
                .flat_map(|home| {
                    [
                        home.join("Library/Application Support/com.mikomai.agent"),
                        home.join("Library/Application Support/mikomai"),
                        home.join(".config/mikomai"),
                    ]
                })
                .filter(|p| p.is_dir())
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            json!(paths)
        }
        "connection_save" | "connection_remove" => {
            let mut connections = request["connections"]
                .as_array()
                .ok_or("connections must be an array")?
                .clone();
            if text("op") == "connection_remove" {
                connections.retain(|c| c["id"] != request["id"]);
            } else {
                let connection = &request["connection"];
                if let Some(error) = validate_connection(connection) {
                    return Err(error);
                }
                if let Some(index) = connections.iter().position(|c| c["id"] == connection["id"]) {
                    connections[index] = connection.clone();
                } else {
                    connections.push(connection.clone());
                }
            }
            json!(connections)
        }
        "dry_run" => json!(accepts_dry_run(
            request["processSucceeded"].as_bool().unwrap_or(false),
            text("output")
        )),
        "sessions" => update_sessions(request)?,
        _ => return Err("unknown native policy query".into()),
    })
}

pub fn canonical_device_id(value: &str) -> String {
    let catalog: Value =
        serde_json::from_str(include_str!("device_catalog.json")).expect("bundled catalog");
    if catalog["deviceTypes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|id| id == value)
    {
        return value.to_owned();
    }
    if let Some((id, _)) = catalog["aliases"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(_, name)| name.as_str().unwrap().to_lowercase() == value.to_lowercase())
    {
        return id.clone();
    }
    match value.to_lowercase().as_str() {
        "other" => "generic".into(),
        "f220" | "fitelnet" | "furukawa fitelnet" => "furukawa_fitelnet".into(),
        _ => value.to_lowercase().replace(' ', "_"),
    }
}

pub fn validate_connection(connection: &Value) -> Option<String> {
    let field = |key: &str| connection[key].as_str().unwrap_or("");
    let name = field("name");
    let host = field("host");
    let safe = |s: &str| {
        !s.is_empty()
            && s.chars().count() <= 255
            && s.chars().all(|c| c.is_alphanumeric() || ".-_".contains(c))
    };
    if !safe(name) {
        return Some("名前は文字・数字と . - _ で入力してください。".into());
    }
    let serial = matches!(
        field("connectionType").to_ascii_lowercase().as_str(),
        "console" | "serial"
    );
    let serial_path = serial
        && host.starts_with("/dev/")
        && host.len() <= 255
        && !host.contains("..")
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c));
    if !serial_path
        && !safe(host)
        && !(host.len() <= 255 && host.parse::<std::net::Ipv6Addr>().is_ok())
    {
        return Some("ホストは IP アドレスまたは文字・数字と . - _ で入力してください。".into());
    }
    let port = field("port");
    if !serial && !port.is_empty() && !port.parse::<u16>().is_ok_and(|p| p > 0) {
        return Some("ポートは 1 から 65535 の数値で入力してください。".into());
    }
    let username = field("username");
    if username.chars().count() > 128 || username.chars().any(char::is_control) {
        return Some("ユーザー名が長すぎるか、使用できない文字を含んでいます。".into());
    }
    let driver = field("deviceType");
    if driver.is_empty() || driver.chars().count() > 128 || driver.chars().any(char::is_control) {
        return Some("機器タイプは 1 から 128 文字で入力してください。".into());
    }
    None
}

pub fn accepts_dry_run(process_succeeded: bool, output: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(output) else {
        return false;
    };
    process_succeeded
        && value["success"] == true
        && value["results"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty() && rows.iter().all(|r| r["ok"] == true))
}

fn update_sessions(request: &Value) -> Result<Value, String> {
    let mut sessions = request["state"]["sessions"]
        .as_array()
        .ok_or("sessions must be an array")?
        .clone();
    let mut active = request["state"]["activeSessionID"].clone();
    if !sessions.iter().any(|s| s["id"] == active) {
        active = sessions
            .first()
            .map(|s| s["id"].clone())
            .unwrap_or(Value::Null);
    }
    let id = &request["id"];
    let create = |sessions: &mut Vec<Value>, active: &mut Value| {
        let created = json!({"id":uuid::Uuid::new_v4().to_string().to_uppercase(),
            "title":request["title"].as_str().unwrap_or("新しい会話"), "messages":[], "updatedAt":request["now"]});
        *active = created["id"].clone();
        sessions.insert(0, created);
    };
    match request["action"].as_str().unwrap_or("normalize") {
        "normalize" => {}
        "create" => create(&mut sessions, &mut active),
        "select" => {
            if sessions.iter().any(|s| s["id"] == *id) {
                active = id.clone();
            }
        }
        "delete" => {
            sessions.retain(|s| s["id"] != *id);
            if active == *id {
                active = sessions
                    .first()
                    .map(|s| s["id"].clone())
                    .unwrap_or(Value::Null);
            }
            if sessions.is_empty() {
                create(&mut sessions, &mut active);
            }
        }
        "rename" => {
            let title = request["title"].as_str().unwrap_or("").trim();
            if !title.is_empty() {
                if let Some(session) = sessions.iter_mut().find(|s| s["id"] == *id) {
                    session["title"] = json!(title);
                }
            }
        }
        _ => return Err("unknown session action".into()),
    }
    Ok(json!({"sessions":sessions,"activeSessionID":active}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dry_run_cannot_authorize_empty_failed_or_malformed_results() {
        for output in [
            "{}",
            "{\"success\":true,\"results\":[]}",
            "{\"success\":true,\"results\":[{\"ok\":false}]}",
            "invalid",
        ] {
            assert!(!accepts_dry_run(true, output));
        }
        let good = r#"{"success":true,"results":[{"ok":true}]}"#;
        assert!(accepts_dry_run(true, good));
        assert!(!accepts_dry_run(false, good));
    }
    #[test]
    fn connection_rejects_shell_input_and_resolves_ipv6_and_telnet() {
        let mut c = json!({"name":"edge","host":"2001:db8::1","port":"23","username":"admin","deviceType":"cisco_xr","connectionType":"Telnet"});
        assert!(validate_connection(&c).is_none());
        assert_eq!(
            query(&json!({"op":"connection","connection":c,"base":"cisco_xr_ssh"})).unwrap()
                ["driver"],
            "cisco_xr_telnet"
        );
        c["host"] = json!("localhost;touch");
        assert!(validate_connection(&c).is_some());
        c["host"] = json!("localhost");
        c["port"] = json!("65536");
        assert!(validate_connection(&c).is_some());
    }
    #[test]
    fn deleting_last_session_creates_replacement_and_preserves_other_fields() {
        let state = json!({"sessions":[{"id":"first","title":"old","messages":[{"role":"user","text":"hello"}]}],"activeSessionID":"first"});
        let renamed = query(&json!({"op":"sessions","state":state,"action":"rename","id":"first","title":"  title  "})).unwrap();
        assert_eq!(
            renamed["sessions"][0]["messages"],
            state["sessions"][0]["messages"]
        );
        assert_eq!(renamed["sessions"][0]["title"], "title");
        let deleted = query(
            &json!({"op":"sessions","state":renamed,"action":"delete","id":"first","now":123}),
        )
        .unwrap();
        assert_eq!(deleted["sessions"].as_array().unwrap().len(), 1);
        assert_ne!(deleted["activeSessionID"], "first");
        assert_eq!(deleted["activeSessionID"], deleted["sessions"][0]["id"]);
    }
}

fn public_ip(value: &str) -> bool {
    match value.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(ip)) => {
            let [a, b, _, _] = ip.octets();
            !matches!(a, 0 | 10 | 127 | 224..=255)
                && !(a == 172 && (16..=31).contains(&b))
                && !(a == 192 && b == 168)
                && !(a == 169 && b == 254)
        }
        Ok(std::net::IpAddr::V6(ip)) => {
            !ip.is_loopback()
                && !ip.is_unspecified()
                && ip.segments()[0] & 0xffc0 != 0xfe80
                && ip.segments()[0] & 0xfe00 != 0xfc00
                && !ip.is_multicast()
        }
        Err(_) => false,
    }
}
pub(crate) fn ping_arguments(command: &Value) -> Value {
    let host = command["host"].as_str().unwrap_or("");
    if host.is_empty()
        || host.starts_with('-')
        || !host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._:%-".contains(c))
    {
        return Value::Null;
    }
    let mut args = vec![
        "-c".to_owned(),
        command["count"]
            .as_i64()
            .unwrap_or(4)
            .clamp(1, 10)
            .to_string(),
    ];
    if let Some(size) = command["size"].as_i64() {
        if !(1..=65500).contains(&size) {
            return Value::Null;
        }
        args.extend(["-s".into(), size.to_string()]);
    }
    if command["df"] == true {
        args.push("-D".into());
    }
    args.push(host.into());
    json!(args)
}
fn parse_ping(text: &str) -> Value {
    let text = text.to_lowercase();
    let capture = |pattern: &str| {
        regex::Regex::new(pattern)
            .ok()?
            .captures(&text)?
            .get(1)
            .map(|m| m.as_str().to_owned())
    };
    let Some(host) = capture(r"(?:ping|ピン|ピング)\s+([a-zA-Z0-9.:-]+)")
        .or_else(|| capture(r"([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:ping|ピン|ピング)"))
    else {
        return Value::Null;
    };
    let size = capture(r"(?:size|サイズ)\s*(\d+)").and_then(|n| n.parse::<i64>().ok());
    let count = capture(r"(?:count|回数|回)\s*(\d+)")
        .or_else(|| capture(r"(\d+)\s*回(?:実行)?"))
        .and_then(|n| n.parse::<i64>().ok());
    json!({"host":host,"size":size,"count":count,"df":if ["df","フラグメント禁止","断片化禁止"].iter().any(|word|text.contains(word)){Some(true)}else{None}})
}
fn network_error(text: &str) -> Value {
    let lower = text.to_lowercase();
    for (kind, marker) in [
        ("invalidInput", "% invalid input"),
        ("incompleteCommand", "% incomplete command"),
        ("ambiguousCommand", "% ambiguous command"),
        ("syntaxError", "syntax error"),
        ("netmikoError", "netmiko error:"),
        ("deviceError", "error: device"),
    ] {
        if lower.contains(marker) {
            return json!({"kind":kind,"detail":marker});
        }
    }
    Value::Null
}
pub(crate) fn diagnostic_host(host: &str, connections: &Value) -> Result<String, String> {
    let host = host.trim();
    let matches = connections
        .as_array()
        .ok_or("ambiguous")?
        .iter()
        .filter(|v| {
            v["name"]
                .as_str()
                .is_some_and(|s| s.trim().eq_ignore_ascii_case(host))
                || v["id"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case(host))
                || v["sourceID"].as_str() == Some(host)
        })
        .collect::<Vec<_>>();
    if matches.len() > 1 {
        return Err("ambiguous".into());
    }
    let Some(record) = matches.first() else {
        return Ok(host.into());
    };
    let address = record["host"].as_str().unwrap_or("").trim();
    if address.is_empty() {
        return Err("missingAddress".into());
    }
    if address.parse::<std::net::IpAddr>().is_err() {
        return Err("invalidAddress".into());
    }
    Ok(address.into())
}
pub(crate) fn interface_command(
    device_type: &str,
    interface: Option<&str>,
) -> Result<String, String> {
    if canonical_device_id(device_type) == "yamaha" {
        let name = interface.unwrap_or("").to_ascii_lowercase();
        if !regex::Regex::new(r"^lan[1-9][0-9]{0,2}$")
            .unwrap()
            .is_match(&name)
        {
            return Err("ヤマハルータの確認対象LAN名（例: LAN1）を1つ指定してください。".into());
        }
        Ok(format!("show status {name}"))
    } else {
        Ok("show interfaces".into())
    }
}

/// Individual connection registration; no bulk file import path.
pub fn register_connection(record: &Value) -> Result<(), String> {
    crate::service::validate_document("connections", &json!([record]))?;
    if let Some(error) = validate_connection(record) {
        return Err(error);
    }
    let id = record["id"]
        .as_str()
        .and_then(|id| uuid::Uuid::parse_str(id).ok())
        .ok_or("connection ID must be a UUID")?;
    crate::shared_service().update_internal("connections", |value| {
        if !value.is_array() {
            *value = json!([]);
        }
        let records = value.as_array_mut().unwrap();
        if let Some(existing) = records
            .iter_mut()
            .find(|v| v["id"].as_str().and_then(|s| uuid::Uuid::parse_str(s).ok()) == Some(id))
        {
            *existing = record.clone();
        } else {
            records.push(record.clone());
        }
        Ok(())
    })
}
