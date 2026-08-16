use super::{
    calculate_host_confidence, extract_first_capture, has_question_keywords, ShortcutRulesConfig,
};
use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

fn default_ping_action() -> String
{
    "self_network_ping".to_string()
}

fn default_ping_message() -> String
{
    "Pingを実行します。".to_string()
}

fn default_ping_patterns() -> Vec<String>
{
    vec![
        r"(?i)([a-zA-Z0-9.:-]+?)\s*(?:に|へ|で|を)?\s*(?:ping|ピン|ピング)".to_string(),
        r"(?i)(?:ping|ピン|ピング)\s*(?::|=|：)?\s*([a-zA-Z0-9.:-]+)".to_string(),
    ]
}

fn default_ping_size_pattern() -> String
{
    r"(?:size|サイズ)\s*(\d+)".to_string()
}

fn default_ping_count_patterns() -> Vec<String>
{
    vec![
        r"(?:count|回数|回)\s*(\d+)".to_string(),
        r"(\d+)\s*回(?:実行)?".to_string(),
    ]
}

#[derive(Debug, Deserialize, Clone)]
pub struct PingRegexConfig
{
    #[serde(default = "default_ping_action")]
    pub action: String,
    #[serde(default = "default_ping_message")]
    pub message: String,
    #[serde(default = "default_ping_patterns")]
    pub patterns: Vec<String>,
    #[serde(default = "default_ping_size_pattern")]
    pub size_pattern: String,
    #[serde(default = "default_ping_count_patterns")]
    pub count_patterns: Vec<String>,
}

impl Default for PingRegexConfig
{
    fn default() -> Self
    {
        Self {
            action: default_ping_action(),
            message: default_ping_message(),
            patterns: default_ping_patterns(),
            size_pattern: default_ping_size_pattern(),
            count_patterns: default_ping_count_patterns(),
        }
    }
}

#[allow(dead_code)]
pub fn parse_ping_command(input: &str) -> Option<Value>
{
    let config = ShortcutRulesConfig::load();
    parse_ping_command_with_config(input, &config)
}

pub fn parse_ping_command_with_config(input: &str, config: &ShortcutRulesConfig) -> Option<Value>
{
    let lower_input = input.to_lowercase();
    let ping_cfg = &config.fastroute.ping;

    let host = extract_first_capture(&lower_input, &ping_cfg.patterns)?;
    if host == "size" || host == "count" || host == "df"
    {
        return None;
    }

    let mut args = serde_json::Map::new();
    args.insert("host".to_string(), Value::String(host));

    if let Ok(re_size) = Regex::new(&ping_cfg.size_pattern)
    {
        if let Some(caps) = re_size.captures(&lower_input)
        {
            if let Some(val_str) = caps.get(1)
            {
                if let Ok(val) = val_str.as_str().parse::<i64>()
                {
                    args.insert(
                        "size".to_string(),
                        Value::Number(serde_json::Number::from(val)),
                    );
                }
            }
        }
    }

    for pattern in &ping_cfg.count_patterns
    {
        if let Ok(re_count) = Regex::new(pattern)
        {
            if let Some(caps) = re_count.captures(&lower_input)
            {
                if let Some(val_str) = caps.get(1)
                {
                    if let Ok(val) = val_str.as_str().parse::<i64>()
                    {
                        args.insert(
                            "count".to_string(),
                            Value::Number(serde_json::Number::from(val)),
                        );
                        break;
                    }
                }
            }
        }
    }

    if lower_input.contains("df")
        || lower_input.contains("フラグメント禁止")
        || lower_input.contains("断片化禁止")
        || lower_input.contains("フラグメントなし")
        || lower_input.contains("フラグメント無し")
    {
        args.insert("df".to_string(), Value::Bool(true));
    }

    Some(Value::Object(args))
}

pub fn detect_ping_shortcut(
    input: &str,
    config: &ShortcutRulesConfig,
) -> Option<(String, Value, String, f64)>
{
    let ping_cfg = &config.fastroute.ping;
    if let Some(ping_args) = parse_ping_command_with_config(input, config)
    {
        let confidence = if let Some(host) = ping_args.get("host").and_then(|v| v.as_str())
        {
            calculate_host_confidence(input, host, config)
        }
        else if has_question_keywords(input, config)
        {
            0.0
        }
        else
        {
            0.8
        };
        return Some((
            ping_cfg.action.clone(),
            ping_args,
            ping_cfg.message.clone(),
            confidence,
        ));
    }
    None
}
