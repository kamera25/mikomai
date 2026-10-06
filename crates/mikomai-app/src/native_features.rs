//! Portable policies formerly owned by Swift. JSON is a temporary ABI envelope;
//! inputs and outputs retain UI DTO fields until the UniFFI migration.
use serde_json::{json, Value};

pub fn query(request: &Value) -> Result<Value, String> {
    let text = |key: &str| request[key].as_str().unwrap_or("");
    Ok(match text("op") {
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
    if !safe(host) && !(host.len() <= 255 && host.parse::<std::net::Ipv6Addr>().is_ok()) {
        return Some("ホストは IP アドレスまたは文字・数字と . - _ で入力してください。".into());
    }
    let port = field("port");
    if !port.is_empty() && !port.parse::<u16>().is_ok_and(|p| p > 0) {
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
