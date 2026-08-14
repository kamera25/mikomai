use regex::Regex;
use serde::Deserialize;
use serde_json::Value;
use super::ShortcutRulesConfig;

fn default_nwdiag_action() -> String {
    "self_network_nwdiag".to_string()
}

fn default_nwdiag_message() -> String {
    "ネットワーク図(nwdiag)を生成します。".to_string()
}

fn default_nwdiag_pattern() -> String {
    r"(?i)nwdiag\s*\{".to_string()
}

#[derive(Debug, Deserialize, Clone)]
pub struct NwdiagRegexConfig {
    #[serde(default = "default_nwdiag_action")]
    pub action: String,
    #[serde(default = "default_nwdiag_message")]
    pub message: String,
    #[serde(default = "default_nwdiag_pattern")]
    pub pattern: String,
}

impl Default for NwdiagRegexConfig {
    fn default() -> Self {
        Self {
            action: default_nwdiag_action(),
            message: default_nwdiag_message(),
            pattern: default_nwdiag_pattern(),
        }
    }
}

pub fn detect_nwdiag_shortcut(input: &str, config: &ShortcutRulesConfig) -> Option<(String, Value, String, f64)> {
    let nwdiag_cfg = &config.fastroute.nwdiag;
    let lower_trimmed = input.trim();
    if lower_trimmed.contains('{') {
        if let Ok(re_nwdiag) = Regex::new(&nwdiag_cfg.pattern) {
            if let Some(mat) = re_nwdiag.find(lower_trimmed) {
                let start_idx = mat.start();
                if let Some(end_idx) = lower_trimmed.rfind('}') {
                    if end_idx > start_idx {
                        let schema = lower_trimmed[start_idx..=end_idx].to_string();
                        let mut params = serde_json::Map::new();
                        params.insert("schema".to_string(), Value::String(schema));
                        return Some((
                            nwdiag_cfg.action.clone(),
                            Value::Object(params),
                            nwdiag_cfg.message.clone(),
                            1.0,
                        ));
                    }
                }
            }
        }
    }
    None
}
