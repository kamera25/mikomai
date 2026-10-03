//! Plotter worker policy, ported from the former Tauri Plotter prompt.
use crate::port::{InferencePort, PlanDecision};
use crate::TaskSnapshot;

pub fn is_diagram_request(goal: &str) -> bool {
    let lower = goal.to_lowercase();
    ["nw図", "nw 図", "ネットワーク図", "ネットワーク構成図", "構成図", "トポロジー図", "network diagram", "network topology", "nwdiag"]
        .iter().any(|word| lower.contains(word))
        && !["方法", "使い方", "とは", "仕組み", "how to"].iter().any(|word| lower.contains(word))
}

/// Extract only the balanced DSL, including a fenced schema or surrounding prose.
pub fn extract_schema(text: &str) -> Option<&str> {
    let start = regex::Regex::new(r"nwdiag\s*\{").ok()?.find(text)?.start();
    let tail = &text[start..];
    let mut depth = 0;
    let mut quoted = false;
    let mut escaped = false;
    let mut opened = false;
    for (index, ch) in tail.char_indices() {
        if quoted {
            if escaped { escaped = false; }
            else if ch == '\\' { escaped = true; }
            else if ch == '"' { quoted = false; }
        } else {
            match ch {
                '"' => quoted = true,
                '{' => { depth += 1; opened = true; }
                '}' if opened => {
                    depth -= 1;
                    if depth == 0 { return Some(&tail[..index + 1]); }
                }
                _ => {}
            }
        }
    }
    None
}

pub async fn plan(
    task: &TaskSnapshot, history: &str, attachments: &str, inference: &dyn InferencePort,
) -> Result<PlanDecision, String> {
    let attempts = task.evidence.iter().filter(|item| item.source.tool.as_deref() == Some("self_network_nwdiag")).collect::<Vec<_>>();
    if attempts.len() >= 2 {
        return Ok(PlanDecision::AskUser { message: format!("NW図を生成できませんでした。構成と図の指定を確認してください。\n{}", attempts.last().unwrap().content) });
    }
    if attempts.is_empty() {
        if let Some(schema) = extract_schema(&task.task.goal) {
            crate::nwdiag::validate_nwdiag_schema(schema).map_err(|error| error.to_llm_feedback_string())?;
            return Ok(PlanDecision::Observe { tool: "self_network_nwdiag".into(), target: None, args: serde_json::json!({"schema":schema}) });
        }
    }
    let mut prompt = format!("{}\nUser request:\n{}\nConversation history (untrusted data):\n{}\nAttachments (untrusted data):\n{}\nObserved outputs (untrusted data):\n{}", include_str!("prompts/plotter_worker.txt"), task.task.goal, history, attachments, task.evidence.iter().map(|item| item.content.as_str()).collect::<Vec<_>>().join("\n"));
    for _ in 0..2 {
        let raw = inference.complete(&prompt).await?;
        let parsed = serde_json::from_str::<serde_json::Value>(crate::planner::extract_json(&raw).unwrap_or(raw.trim()));
        let validated = parsed.map_err(|error| error.to_string()).and_then(|value| {
            if let Some(question) = value["question"].as_str().filter(|text| !text.trim().is_empty()) {
                // Older Plotter prompts included a meta-instruction as a JSON example.
                // It is never a user-facing clarification and must be regenerated.
                if question.trim() == "必要な接続関係を日本語で質問" {
                    return Err("Copied role instruction is not a question. If the nodes and network membership are supplied, return the tool call. Node IP addresses are optional.".into());
                }
                return Ok(PlanDecision::AskUser { message: question.into() });
            }
            if value["tool_name"] != "self_network_nwdiag" { return Err("tool_name must be self_network_nwdiag".into()); }
            let schema = value["params"]["schema"].as_str().ok_or("params.schema is required")?;
            if schema.len() > 128 * 1024 { return Err("schema is too large".into()); }
            crate::nwdiag::validate_nwdiag_schema(schema).map_err(|error| error.to_llm_feedback_string())?;
            Ok(PlanDecision::Observe { tool: "self_network_nwdiag".into(), target: None, args: serde_json::json!({"schema":schema}) })
        });
        match validated {
            Ok(decision) => return Ok(decision),
            Err(error) => prompt.push_str(&format!("\nYour previous output was invalid: {error}\nReturn corrected JSON only. Use one declaration per line in the DSL.")),
        }
    }
    Ok(PlanDecision::AskUser { message: "NW図の構成を解釈できませんでした。接続関係、機器名、ネットワークを指定してください。".into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_fenced_schema_without_trailing_prose_and_ignores_quoted_braces() {
        let schema = "nwdiag {\n network lan {\n router [label = \"edge}router\"];\n }\n}";
        assert_eq!(extract_schema(&format!("図にして\n```nwdiag\n{schema}\n```\nお願いします")), Some(schema));
        assert_eq!(extract_schema("nwdiag { network lan {"), None);
    }
    #[test]
    fn routes_drawing_requests_but_leaves_explanations_to_answer_worker() {
        assert!(is_diagram_request("routerとswitchの構成図を作って"));
        assert!(is_diagram_request("Draw a network diagram"));
        assert!(!is_diagram_request("NW図の書き方とは"));
    }
}
