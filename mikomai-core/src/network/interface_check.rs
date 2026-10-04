//! Vendor-independent interface-up intent and verification of graph-backed canonical data.
use crate::{agent::RegisteredDevice, port::PlanDecision, TaskSnapshot};
use regex::Regex;
use serde_json::{json, Value};

/// Single interface request only. Compound goals remain with the regular planner.
pub fn request(goal: &str) -> Option<(String, String)> {
    let re = Regex::new(r"(?i)^\s*(.+?)\s*の\s*([a-z][a-z0-9/_.:-]*[0-9][a-z0-9/_.:-]*)\s*(?:が|は|の)?\s*(?:up(?:している|してる|か|状態)?|アップ(?:している|してる)?|リンク(?:アップ|状態)|状態)\s*(?:か|どうか)?\s*(?:を)?\s*(?:確認(?:して(?:ください)?)?|調べて(?:ください)?|チェック(?:して)?)\s*[？?。！!]*\s*$").ok()?;
    let c = re.captures(goal)?;
    let target = c[1].trim();
    if target.contains(['、', ',', ';', '\n']) || target.contains("と") {
        return None;
    }
    Some((target.into(), c[2].to_ascii_lowercase()))
}

pub fn decision(task: &TaskSnapshot, inventory: &[RegisteredDevice]) -> Option<PlanDecision> {
    let (target, interface) = request(&task.task.goal)?;
    let matches = inventory
        .iter()
        .filter(|d| {
            d.hostname.eq_ignore_ascii_case(&target)
                || d.ip.as_deref() == Some(&target)
                || d.id.as_deref() == Some(&target)
                || (matches!(
                    target.as_str(),
                    "ヤマハルータ" | "ヤマハルーター" | "Yamahaルータ"
                ) && d
                    .device_type
                    .as_deref()
                    .is_some_and(|kind| kind.to_lowercase().contains("yamaha")))
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Some(PlanDecision::AskUser { message: "対象を1台に特定できません。登録機器名とインターフェース名（例: LAN1）を指定してください。".into() });
    }
    let device = matches[0];
    let args = json!({"device":device.hostname,"resource":"interfaces","interface":interface,"refresh":true});
    if let Some(evidence) =
        crate::agent::observed_request(task, "get_state", Some(&device.hostname), &args, inventory)
    {
        let answer = if evidence.source.success == Some(false) {
            format!(
                "{} の{}は確認できませんでした。取得失敗のためup/downは判定していません。\n{}",
                device.hostname,
                interface.to_uppercase(),
                evidence.content
            )
        } else {
            answer(&device.hostname, &interface, &evidence.content)
        };
        return Some(PlanDecision::Complete { brief: answer });
    }
    Some(PlanDecision::Observe {
        tool: "get_state".into(),
        target: Some(device.hostname.clone()),
        args,
    })
}

pub fn answer(device: &str, interface: &str, raw: &str) -> String {
    let result = serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| crate::network::interface::validate_canonical_table(&value, device).ok());
    let Some(table) = result else {
        return format!(
            "{device} の{interface}は確認不能です。Graph由来の有効なCanonical観測がありません。"
        );
    };
    let found = table
        .interfaces
        .iter()
        .filter(|entry| entry.name.eq_ignore_ascii_case(interface))
        .collect::<Vec<_>>();
    if found.len() != 1 {
        return format!("{device} の{interface}は確認不能です。対象インターフェースの観測を一意に取得できません。");
    }
    let result = match found[0].status {
        crate::schema::interface::InterfaceStatus::Up => "upです。",
        crate::schema::interface::InterfaceStatus::Down => "downです。",
        crate::schema::interface::InterfaceStatus::Unknown => {
            "確認不能です（Canonical観測の状態がunknown）。"
        }
    };
    format!("{device} の{}は{result}\n観測時刻: {}\n根拠: Graphに保存・再取得したCanonicalインターフェース状態（status）。\n管理上の有効/無効（admin_state）と、IP疎通はこの確認では判定していません。", found[0].name, table.metadata.generated_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn single_intent_and_no_duplicate_execution() {
        let inventory = vec![RegisteredDevice {
            id: None,
            hostname: "gw".into(),
            ip: None,
            device_type: Some("yamaha".into()),
        }];
        let mut task = TaskSnapshot::new("gwのLAN1がupしているか確認して");
        let Some(PlanDecision::Observe { tool, target, args }) = decision(&task, &inventory) else {
            panic!("observe")
        };
        let v = json!({"version":"1.0","metadata":{"source_device":"gw","os_type":"test","generated_at":chrono::Utc::now().to_rfc3339()},"interfaces":[{"name":"LAN1","status":"up","ipv4_addresses":[],"prefix_len":null}]});
        let mut evidence = crate::Evidence::from_tool(v.to_string(), target, Some(tool));
        evidence.source.request = Some(args.to_string());
        evidence.source.success = Some(true);
        task.evidence.push(evidence);
        assert!(
            matches!(decision(&task,&inventory),Some(PlanDecision::Complete{brief}) if brief.contains("upです"))
        );
        task.evidence[0].source.success = Some(false);
        assert!(
            matches!(decision(&task,&inventory),Some(PlanDecision::Complete{brief}) if brief.contains("取得失敗"))
        );
        assert!(request("gwのLAN1がupしているか確認して、その後pingして").is_none());
        assert!(request("gwのLAN1をupに変更して").is_none());
        assert!(matches!(
            decision(&task, &[]),
            Some(PlanDecision::AskUser { .. })
        ));
        assert!(request("gwのGigabitEthernet1/0/1がupしているか確認して").is_some());
        assert!(!answer("other", "LAN1", &v.to_string()).contains("upです"));
    }
}
