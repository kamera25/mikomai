use validator::Validate;

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

    cleaned.trim().to_string()
}

pub async fn convert_raw_to_yaml(
    app: &tauri::AppHandle,
    state: &crate::llm::llm::LlamaState,
    raw_output: &str,
    device_name: &str,
    os_type: &str,
) -> Result<String, String> {
    let schema_json = include_str!("../../schema/route-table-schema.json");
    let generated_at = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    let system_prompt = format!(
        include_str!("../config/route_system_prompt.txt"),
        schema_json, generated_at, device_name, os_type
    );

    let mut current_user_prompt = format!(
        "Convert this raw routing table command output to YAML:\n\n{}",
        raw_output
    );

    let mut retry_count = 0;
    let max_retries = 3;

    while retry_count <= max_retries {
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

        // Validate YAML
        match serde_yaml::from_str::<crate::schema::route::UniversalRouteTable>(&clean_yaml) {
            Ok(parsed) => match parsed.validate() {
                Ok(_) => {
                    log::info!("Route table successfully parsed and validated!");
                    return Ok(clean_yaml);
                }
                Err(validation_errors) => {
                    log::warn!("Validation failed: {:?}", validation_errors);
                    if retry_count < max_retries {
                        retry_count += 1;
                        current_user_prompt = format!(
                                "The previous output failed validation with the following errors:\n{:?}\n\nPlease fix the errors and output the complete corrected YAML. DO NOT output any comments, just output the corrected YAML.",
                                validation_errors
                            );
                    } else {
                        return Err(format!(
                            "Route table validation failed after {} retries. Errors: {:?}",
                            max_retries, validation_errors
                        ));
                    }
                }
            },
            Err(parse_err) => {
                log::warn!("YAML parsing failed: {}", parse_err);
                if retry_count < max_retries {
                    retry_count += 1;
                    current_user_prompt = format!(
                        "The previous output was not valid YAML. Parsing error:\n{}\n\nPlease fix the syntax and output the complete corrected YAML. DO NOT output any comments, just output the corrected YAML.",
                        parse_err
                    );
                } else {
                    return Err(format!(
                        "Route table parsing failed after {} retries. Error: {}",
                        max_retries, parse_err
                    ));
                }
            }
        }
    }

    Err("Unknown validation state reached".to_string())
}
