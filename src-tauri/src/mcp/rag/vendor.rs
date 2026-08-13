use crate::mcp::brands;
use regex::Regex;

pub struct ProcessedQuery {
    pub query: String,
    pub brand_filter: Option<String>,
}

pub fn check_registered_device(query: &str, app: &tauri::AppHandle) -> Option<String> {
    crate::mcp::devices::get_registered_device_info(query, app)
}

pub fn parse_vendor_context(query: &str) -> ProcessedQuery {
    let mut brand_filter: Option<String> = None;
    let mut processed_query = query.to_string();

    // Regex to match [Context: BrandName]
    // Matches something like [Context: Cisco] or [Context: Cisco OS=1.0]
    if let Ok(context_re) = Regex::new(r"\[Context:\s*([^\]\s]+)[^\]]*\]") {
        if let Some(caps) = context_re.captures(query) {
            let brand_candidate = caps.get(1).map(|m| m.as_str()).unwrap_or("");
            if let Some(matched_brand) = brands::get_brand(brand_candidate) {
                brand_filter = Some(format!("brand = '{0}' OR brand = '{1}'", matched_brand, brand_candidate));
                // Remove the context tag from the query for embedding
                processed_query = context_re.replace_all(query, "").to_string().trim().to_string();
            }
        }

        if brand_filter.is_none() {
            // Fallback: check if any known brand alias defined in brands.yaml is mentioned in the query string
            if let Some((matched_brand, matched_alias)) = brands::detect_brand_in_text(query) {
                brand_filter = Some(format!("brand = '{0}' OR brand = '{1}'", matched_brand, matched_alias));
            }
        }

        // If query is now empty (e.g. LLM sent ONLY the context tag), 
        // fallback to using the extracted brand name as the query.
        if processed_query.is_empty() && brand_filter.is_some() {
            if let Some(caps) = context_re.captures(query) {
                processed_query = caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            }
        }
    }

    ProcessedQuery {
        query: processed_query,
        brand_filter,
    }
}

pub fn get_vector_search_instruction() -> &'static str {
    "ネットワーク機器の操作マニュアルから、関連する設定コマンドや手順を検索します。"
}
