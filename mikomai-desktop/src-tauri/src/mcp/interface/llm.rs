use crate::mcp::interface::canonical::{
    evidence, extract, prompt_contract, reconstruct_and_validate,
    InterfaceCanonicalizationEvidence, InterfaceSelection,
};

#[derive(Debug, Clone)]
pub struct CanonicalInterfaceResult {
    pub yaml: String,
    pub evidence: InterfaceCanonicalizationEvidence,
}

fn clean_yaml_output(output: &str) -> String {
    let trimmed = output
        .trim()
        .trim_start_matches("```yaml")
        .trim_start_matches("```yml")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    crate::mcp::canonicalization::normalize_yaml_indentation(trimmed)
}

pub async fn convert_raw_to_yaml(
    app: &tauri::AppHandle,
    state: &crate::llm::llm::LlamaState,
    raw_output: &str,
    device_name: &str,
    os_type: &str,
) -> Result<CanonicalInterfaceResult, String> {
    let extracted = extract(raw_output);
    if extracted.evidence.is_empty() {
        return Err("interface candidate extraction found no interface lines".to_string());
    }
    let system_prompt = "You map pre-extracted network CLI candidates. Never invent values: return only YAML index selections that follow the supplied contract. No prose or code fences.";
    let contract = prompt_contract(&extracted, raw_output);
    let generated_at = chrono::Utc::now();
    let mut prompt = contract.clone();
    for attempt in 0..=3 {
        let output = crate::llm::llm::ask_llm_internal(&prompt, system_prompt, app, state)
            .await
            .map_err(|e| format!("LLM inference failed: {e}"))?;
        let result = serde_yaml::from_str::<InterfaceSelection>(&clean_yaml_output(&output))
            .map_err(|e| format!("invalid constrained interface selection: {e}"))
            .and_then(|selection| {
                reconstruct_and_validate(selection, &extracted, device_name, os_type, generated_at)
            });
        match result {
            Ok(table) => {
                return Ok(CanonicalInterfaceResult {
                    yaml: serde_yaml::to_string(&table).map_err(|e| e.to_string())?,
                    evidence: evidence(&extracted),
                })
            }
            Err(error) if attempt < 3 => {
                prompt = format!("The prior index selection was rejected: {error}. Return the complete corrected YAML selection only.\n\n{contract}");
            }
            Err(error) => {
                return Err(format!(
                    "interface canonicalization failed after 4 attempts: {error}"
                ))
            }
        }
    }
    unreachable!()
}
