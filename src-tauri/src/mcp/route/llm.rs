use crate::mcp::route::canonical::{
    evidence, extract, prompt_contract, reconstruct_and_validate, RouteCanonicalizationEvidence,
    RouteSelection,
};

#[derive(Debug, Clone)]
pub struct CanonicalRouteResult {
    pub yaml: String,
    pub evidence: RouteCanonicalizationEvidence,
}

fn clean_yaml_output(output: &str) -> String {
    let mut cleaned = output.trim().to_string();

    if cleaned.starts_with("```yaml") {
        cleaned = cleaned["```yaml".len()..].trim_start().to_string();
    } else if cleaned.starts_with("```yml") {
        cleaned = cleaned["```yml".len()..].trim_start().to_string();
    } else if cleaned.starts_with("```") {
        cleaned = cleaned["```".len()..].trim_start().to_string();
    }

    if cleaned.ends_with("```") {
        cleaned = cleaned[..cleaned.len() - "```".len()]
            .trim_end()
            .to_string();
    }

    let trimmed = cleaned.trim();
    crate::mcp::canonicalization::normalize_yaml_indentation(trimmed)
}

pub async fn convert_raw_to_yaml(
    app: &tauri::AppHandle,
    state: &crate::llm::llm::LlamaState,
    raw_output: &str,
    device_name: &str,
    os_type: &str,
) -> Result<CanonicalRouteResult, String> {
    let extracted = extract(raw_output);
    if extracted.evidence.is_empty() {
        return Err("route candidate extraction found no route lines".to_string());
    }
    let system_prompt = "You map pre-extracted network CLI candidates. Never invent values: return only YAML index selections that follow the supplied contract. No prose or code fences.";
    let mut current_user_prompt = prompt_contract(&extracted, raw_output);
    let generated_at = chrono::Utc::now();
    let max_retries = 3;
    for retry_count in 0..=max_retries {
        log::info!(
            "Prompting LLM to convert route table (Attempt {})...",
            retry_count + 1
        );
        let llm_res = match crate::llm::llm::ask_llm_internal(
            &current_user_prompt,
            &system_prompt,
            app,
            state,
        )
        .await
        {
            Ok(res) => res,
            Err(e) => {
                log::error!("LLM call failed: {}", e);
                return Err(format!("LLM inference failed: {}", e));
            }
        };

        let clean_yaml = clean_yaml_output(&llm_res);

        match serde_yaml::from_str::<RouteSelection>(&clean_yaml)
            .map_err(|error| format!("invalid constrained route selection: {error}"))
            .and_then(|selection| {
                reconstruct_and_validate(selection, &extracted, device_name, os_type, generated_at)
            }) {
            Ok(table) => {
                return Ok(CanonicalRouteResult {
                    yaml: serde_yaml::to_string(&table).map_err(|error| error.to_string())?,
                    evidence: evidence(&extracted),
                })
            }
            Err(error) if retry_count < max_retries => {
                log::warn!("Constrained route selection validation failed: {error}");
                current_user_prompt = format!("The prior index selection was rejected: {error}. Return the complete corrected YAML selection only.\n\n{}", prompt_contract(&extracted, raw_output));
            }
            Err(error) => {
                return Err(format!(
                    "route canonicalization failed after {} attempts: {error}",
                    max_retries + 1
                ))
            }
        }
    }
    unreachable!("retry loop always returns")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_yaml_output_with_unindented_properties() {
        let raw_llm_output = "```yaml\nentries:\n- line_idx: 0\ndestination_idx: 0\ngateway_idx: 1\ninterface_idx: 2\nflags_idx: null\nmetric: null\n```";
        let cleaned = clean_yaml_output(raw_llm_output);
        let parsed: Result<RouteSelection, _> = serde_yaml::from_str(&cleaned);
        assert!(parsed.is_ok(), "Failed to parse cleaned YAML: {:?}", parsed.err());
        let selection = parsed.unwrap();
        assert_eq!(selection.entries.len(), 1);
        assert_eq!(selection.entries[0].line_idx, 0);
        assert_eq!(selection.entries[0].destination_idx, 0);
        assert_eq!(selection.entries[0].gateway_idx, 1);
    }
}

