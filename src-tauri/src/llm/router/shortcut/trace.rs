use super::{calculate_host_confidence, extract_first_capture, ShortcutRulesConfig};
use serde_json::Value;

pub fn detect_traceroute_shortcut(
    input: &str,
    lower_input: &str,
    config: &ShortcutRulesConfig,
) -> Option<(String, Value, String, f64)>
{
    let trace_cfg = &config.fastroute.traceroute;
    let trace_host = extract_first_capture(lower_input, &trace_cfg.patterns)?;

    let mut params = serde_json::Map::new();
    params.insert("host".to_string(), Value::String(trace_host.clone()));
    let confidence = calculate_host_confidence(input, &trace_host, config);
    Some((
        trace_cfg.action.clone(),
        Value::Object(params),
        trace_cfg.message.clone(),
        confidence,
    ))
}
