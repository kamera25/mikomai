use serde_json::Value;

pub fn get_str_arg(args: &Value, keys: &[&str]) -> Option<String>
{
    for &key in keys
    {
        if let Some(val) = args.get(key)
        {
            if let Some(s) = val.as_str()
            {
                if !s.trim().is_empty()
                {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

pub fn get_usize_arg(args: &Value, keys: &[&str]) -> Option<usize>
{
    for &key in keys
    {
        if let Some(val) = args.get(key)
        {
            if let Some(n) = val.as_u64()
            {
                return Some(n as usize);
            }
            if let Some(s) = val.as_str()
            {
                if let Ok(n) = s.parse::<usize>()
                {
                    return Some(n);
                }
            }
        }
    }
    None
}

pub fn get_u32_arg(args: &Value, keys: &[&str]) -> Option<u32>
{
    for &key in keys
    {
        if let Some(val) = args.get(key)
        {
            if let Some(n) = val.as_u64()
            {
                return Some(n as u32);
            }
            if let Some(s) = val.as_str()
            {
                if let Ok(n) = s.parse::<u32>()
                {
                    return Some(n);
                }
            }
        }
    }
    None
}

pub fn get_bool_arg(args: &Value, keys: &[&str]) -> Option<bool>
{
    for &key in keys
    {
        if let Some(val) = args.get(key)
        {
            if let Some(b) = val.as_bool()
            {
                return Some(b);
            }
            if let Some(s) = val.as_str()
            {
                if s.eq_ignore_ascii_case("true")
                {
                    return Some(true);
                }
                if s.eq_ignore_ascii_case("false")
                {
                    return Some(false);
                }
            }
        }
    }
    None
}

pub fn get_history_block_rust(items: &[crate::history::SummaryItem], limit: usize) -> String
{
    if limit == 0 || items.is_empty()
    {
        return "".to_string();
    }
    let mut recent: Vec<crate::history::SummaryItem> = items.to_vec();
    recent.reverse();
    let limit_len = std::cmp::min(limit, recent.len());
    let recent_slice = &recent[0..limit_len];

    let mut text = String::new();
    for (i, item) in recent_slice.iter().enumerate()
    {
        if i > 0
        {
            text.push('\n');
        }
        text.push_str(&format!("{}. {}", i + 1, item.content));
    }
    format!("\n\n<memory>\n{}\n</memory>", text)
}

pub fn extract_json_blocks(text: &str) -> Vec<String>
{
    let mut blocks = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len
    {
        if chars[i] == '{'
        {
            let mut success = false;
            for j in (i + 1..len).rev()
            {
                if chars[j] == '}'
                {
                    let candidate: String = chars[i..=j].iter().collect();
                    if serde_json::from_str::<Value>(&candidate).is_ok()
                    {
                        blocks.push(candidate);
                        i = j;
                        success = true;
                        break;
                    }
                }
            }
            if success
            {
                i += 1;
                continue;
            }
        }
        i += 1;
    }
    blocks
}

#[cfg(test)]
mod tests
{
    use super::*;
    use serde_json::json;

    #[test]
    fn test_extract_json_blocks()
    {
        let text =
            "Here is some text with { \"tool\": \"test\" } and another { \"abc\": 123 } block.";
        let blocks = extract_json_blocks(text);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0], "{ \"tool\": \"test\" }");
        assert_eq!(blocks[1], "{ \"abc\": 123 }");

        let text_no_json = "No JSON blocks here.";
        assert!(extract_json_blocks(text_no_json).is_empty());
    }

    #[test]
    fn test_get_str_arg()
    {
        let args = json!({
            "host": "192.168.1.1",
            "empty": "   "
        });
        assert_eq!(
            get_str_arg(&args, &["host"]),
            Some("192.168.1.1".to_string())
        );
        assert_eq!(
            get_str_arg(&args, &["empty", "host"]),
            Some("192.168.1.1".to_string())
        );
        assert_eq!(get_str_arg(&args, &["nonexistent"]), None);
    }

    #[test]
    fn test_get_usize_arg()
    {
        let args = json!({
            "size": 64,
            "size_str": "128"
        });
        assert_eq!(get_usize_arg(&args, &["size"]), Some(64));
        assert_eq!(get_usize_arg(&args, &["size_str"]), Some(128));
        assert_eq!(get_usize_arg(&args, &["nonexistent"]), None);
    }

    #[test]
    fn test_get_u32_arg()
    {
        let args = json!({
            "count": 5,
            "count_str": "10"
        });
        assert_eq!(get_u32_arg(&args, &["count"]), Some(5));
        assert_eq!(get_u32_arg(&args, &["count_str"]), Some(10));
        assert_eq!(get_u32_arg(&args, &["nonexistent"]), None);
    }

    #[test]
    fn test_get_bool_arg()
    {
        let args = json!({
            "df_bool": true,
            "df_str_true": "true",
            "df_str_false": "FALSE"
        });
        assert_eq!(get_bool_arg(&args, &["df_bool"]), Some(true));
        assert_eq!(get_bool_arg(&args, &["df_str_true"]), Some(true));
        assert_eq!(get_bool_arg(&args, &["df_str_false"]), Some(false));
        assert_eq!(get_bool_arg(&args, &["nonexistent"]), None);
    }

    #[test]
    fn test_get_history_block_rust()
    {
        let items = vec![
            crate::history::SummaryItem {
                timestamp: "2023-10-27".to_string(),
                content: "First summary".to_string(),
            },
            crate::history::SummaryItem {
                timestamp: "2023-10-28".to_string(),
                content: "Second summary".to_string(),
            },
        ];
        let block = get_history_block_rust(&items, 2);
        assert!(block.contains("1. Second summary"));
        assert!(block.contains("2. First summary"));

        let empty_block = get_history_block_rust(&items, 0);
        assert_eq!(empty_block, "");
    }
}
