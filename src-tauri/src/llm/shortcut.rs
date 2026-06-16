use regex::Regex;
use serde_json::Value;

pub fn parse_ping_command(input: &str) -> Option<Value> {
    let lower_input = input.to_lowercase();
    
    let re_ping1 = Regex::new(r"(?:ping|ピン|ピング)\s+([a-zA-Z0-9.:-]+)").expect("Invalid ping regex 1");
    let re_ping2 = Regex::new(r"([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:ping|ピン|ピング)").expect("Invalid ping regex 2");
    
    let host = if let Some(caps) = re_ping2.captures(&lower_input) {
        caps.get(1).map(|m| m.as_str().to_string())
    } else if let Some(caps) = re_ping1.captures(&lower_input) {
        let h = caps.get(1).map(|m| m.as_str().to_string());
        if let Some(ref h_str) = h {
            if h_str == "size" || h_str == "count" || h_str == "df" {
                None
            } else {
                h
            }
        } else {
            None
        }
    } else {
        None
    };
    
    let host = host?;
    
    let mut args = serde_json::Map::new();
    args.insert("host".to_string(), Value::String(host));
    
    let re_size = Regex::new(r"(?:size|サイズ)\s*(\d+)").expect("Invalid size regex");
    if let Some(caps) = re_size.captures(&lower_input) {
        if let Some(val_str) = caps.get(1) {
            if let Ok(val) = val_str.as_str().parse::<i64>() {
                args.insert("size".to_string(), Value::Number(serde_json::Number::from(val)));
            }
        }
    }
    
    let re_count1 = Regex::new(r"(?:count|回数|回)\s*(\d+)").expect("Invalid count regex 1");
    let re_count2 = Regex::new(r"(\d+)\s*回(?:実行)?").expect("Invalid count regex 2");
    if let Some(caps) = re_count1.captures(&lower_input) {
        if let Some(val_str) = caps.get(1) {
            if let Ok(val) = val_str.as_str().parse::<i64>() {
                args.insert("count".to_string(), Value::Number(serde_json::Number::from(val)));
            }
        }
    } else if let Some(caps) = re_count2.captures(&lower_input) {
        if let Some(val_str) = caps.get(1) {
            if let Ok(val) = val_str.as_str().parse::<i64>() {
                args.insert("count".to_string(), Value::Number(serde_json::Number::from(val)));
            }
        }
    }
    
    if lower_input.contains("df") || lower_input.contains("フラグメント禁止") || lower_input.contains("断片化禁止") {
        args.insert("df".to_string(), Value::Bool(true));
    }
    
    Some(Value::Object(args))
}

pub fn detect_shortcut_tool(input: &str) -> Option<(String, Value, String)> {
    let lower_input = input.to_lowercase();
    
    // 1. Ping
    if let Some(ping_args) = parse_ping_command(input) {
        return Some((
            "self_network_ping".to_string(), 
            ping_args, 
            "Pingを実行します。".to_string()
        ));
    }
    
    // 2. Traceroute
    let re_trace1 = Regex::new(r"(?:trace(?:route)?|トレース|トレースルート)\s+([a-zA-Z0-9.:-]+)").expect("Invalid trace regex 1");
    let re_trace2 = Regex::new(r"([a-zA-Z0-9.:-]+)\s*(?:に|へ)?\s*(?:trace(?:route)?|トレース|トレースルート)").expect("Invalid trace regex 2");
    
    let trace_host = if let Some(caps) = re_trace2.captures(&lower_input) {
        caps.get(1).map(|m| m.as_str().to_string())
    } else if let Some(caps) = re_trace1.captures(&lower_input) {
        caps.get(1).map(|m| m.as_str().to_string())
    } else {
        None
    };
    if let Some(host) = trace_host {
        let mut params = serde_json::Map::new();
        params.insert("host".to_string(), Value::String(host));
        return Some((
            "self_network_traceroute".to_string(), 
            Value::Object(params),
            "Tracerouteを実行します。".to_string()
        ));
    }
    
    // 3. Host List
    let re_host_list1 = Regex::new(r"(?:host|ホスト|接続先|ターゲット).*(?:list|一覧|教え|見せ|確認)").expect("Invalid host list regex 1");
    let re_host_list2 = Regex::new(r"(?:list|一覧|教え|見せ|確認).*(?:host|ホスト|接続先|ターゲット)").expect("Invalid host list regex 2");
    if re_host_list1.is_match(&lower_input) || re_host_list2.is_match(&lower_input) {
        return Some((
            "network_get_hosts".to_string(),
            serde_json::json!({}),
            "登録機器の一覧を取得します。".to_string()
        ));
    }
    
    // 4. ARP
    if lower_input.contains("arp") && (lower_input.contains("ローカル") || lower_input.contains("自機") || lower_input.contains("このpc") || lower_input.contains("local")) {
        return Some((
            "self_network_arp".to_string(),
            serde_json::json!({}),
            "ローカルのARPテーブルを取得します。".to_string()
        ));
    }
    
    // 5. Route Table
    if (lower_input.contains("route") || lower_input.contains("ルーティング")) && (lower_input.contains("ローカル") || lower_input.contains("自機") || lower_input.contains("このpc") || lower_input.contains("local")) {
        return Some((
            "self_network_route".to_string(),
            serde_json::json!({}),
            "ローカルのルーティングテーブルを取得します。".to_string()
        ));
    }
    
    // 6. IP Info
    if lower_input.contains("ip") || lower_input.contains("ネットワーク情報") {
        return Some((
            "network_get_ip_info".to_string(),
            serde_json::json!({}),
            "IP情報を取得します。".to_string()
        ));
    }
    
    // 7. Serial Ports
    if lower_input.contains("console") || lower_input.contains("コンソール") || lower_input.contains("シリアル") {
        if lower_input.contains("list") || lower_input.contains("一覧") || lower_input.contains("ポート") || lower_input.contains("リスト") {
            return Some((
                "network_list_serial_ports".to_string(),
                serde_json::json!({}),
                "シリアルポートの一覧を取得します。".to_string()
            ));
        }
    }
    
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ping_command() {
        let val1 = parse_ping_command("ping 192.168.1.1").unwrap();
        assert_eq!(val1["host"], "192.168.1.1");
        assert!(val1.get("size").is_none());
        assert!(val1.get("count").is_none());

        let val2 = parse_ping_command("8.8.8.8 へピング size 1400 count 5 df").unwrap();
        assert_eq!(val2["host"], "8.8.8.8");
        assert_eq!(val2["size"], 1400);
        assert_eq!(val2["count"], 5);
        assert_eq!(val2["df"], true);
    }

    #[test]
    fn test_detect_shortcut_tool() {
        // Ping
        let res = detect_shortcut_tool("ping google.com").unwrap();
        assert_eq!(res.0, "self_network_ping");
        assert_eq!(res.1["host"], "google.com");

        // Traceroute
        let res = detect_shortcut_tool("traceroute 1.1.1.1").unwrap();
        assert_eq!(res.0, "self_network_traceroute");
        assert_eq!(res.1["host"], "1.1.1.1");

        // Host List
        let res = detect_shortcut_tool("接続先一覧を確認したい").unwrap();
        assert_eq!(res.0, "network_get_hosts");

        // Local ARP
        let res = detect_shortcut_tool("自機のarpテーブル").unwrap();
        assert_eq!(res.0, "self_network_arp");

        // Local Route
        let res = detect_shortcut_tool("ローカルのルーティングテーブル").unwrap();
        assert_eq!(res.0, "self_network_route");

        // IP Info
        let res = detect_shortcut_tool("このPC of IPアドレス、ネットワーク情報を教えて").unwrap(); // "このPC of IPアドレス" に "ip" が含まれる
        assert_eq!(res.0, "network_get_ip_info");

        // Serial Ports
        let res = detect_shortcut_tool("コンソールポート一覧").unwrap();
        assert_eq!(res.0, "network_list_serial_ports");

        // None
        assert!(detect_shortcut_tool("普通の質問: NTPって何？").is_none());
    }
}
