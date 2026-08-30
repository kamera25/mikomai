use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NwdiagValidationError {
    pub line: Option<usize>,
    pub message: String,
    pub suggestion: Option<String>,
}

impl fmt::Display for NwdiagValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(line) = self.line {
            write!(f, "Line {}: {}", line, self.message)?;
        } else {
            write!(f, "{}", self.message)?;
        }
        if let Some(sug) = &self.suggestion {
            write!(f, "\nSuggestion: {}", sug)?;
        }
        Ok(())
    }
}

impl NwdiagValidationError {
    pub fn new(
        line: Option<usize>,
        message: impl Into<String>,
        suggestion: Option<impl Into<String>>,
    ) -> Self {
        Self {
            line,
            message: message.into(),
            suggestion: suggestion.map(Into::into),
        }
    }

    pub fn to_llm_feedback_string(&self) -> String {
        let mut msg = format!("nwdiag DSL validation failed: {}", self.message);
        if let Some(line) = self.line {
            msg.push_str(&format!(" (around line {})", line));
        }
        if let Some(sug) = &self.suggestion {
            msg.push_str(&format!("\nSuggested fix: {}", sug));
        }
        msg.push_str("\n\nPlease regenerate the nwdiag schema ensuring valid syntax. Example:\nnwdiag {\n  network dmz {\n    address = \"192.168.1.0/24\";\n    web01 [address = \"192.168.1.10\"];\n    web02 [address = \"192.168.1.11\"];\n  }\n}");
        msg
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Scope {
    TopLevel,
    InNwdiag,
    InNetwork,
    InGroup,
}

/// Validates nwdiag schema DSL string against grammar rules
pub fn validate_nwdiag_schema(schema: &str) -> Result<(), NwdiagValidationError> {
    let trimmed = schema.trim();
    if trimmed.is_empty() {
        return Err(NwdiagValidationError::new(
            None,
            "Schema cannot be empty",
            Some("Provide a valid nwdiag DSL schema enclosed in 'nwdiag { ... }'"),
        ));
    }

    // Fast check for braces balance and quotes across entire content
    validate_balanced_tokens(schema)?;

    let lines: Vec<&str> = schema.lines().collect();
    let mut scope_stack = Vec::new();
    let mut network_count = 0;
    let mut peer_count = 0;
    let mut current_network_nodes = 0;

    let mut in_nwdiag_block = false;

    for (idx, original_line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let line = strip_comments(original_line).trim().to_string();

        if line.is_empty() {
            continue;
        }

        // Check for opening nwdiag block
        if !in_nwdiag_block {
            if line.starts_with("nwdiag") {
                if !line.contains('{') && idx + 1 < lines.len() && !lines[idx + 1].contains('{') {
                    return Err(NwdiagValidationError::new(
                        Some(line_num),
                        "Expected '{' after 'nwdiag'",
                        Some("Use 'nwdiag {' at the start of your schema"),
                    ));
                }
                in_nwdiag_block = true;
                scope_stack.push((Scope::InNwdiag, line_num));
                continue;
            } else {
                return Err(NwdiagValidationError::new(
                    Some(line_num),
                    format!("Schema must start with 'nwdiag {{', but found '{}'", line),
                    Some("Enclose all definitions inside 'nwdiag { ... }'"),
                ));
            }
        }

        // Handle line with braces and statements
        let tokens: Vec<char> = line.chars().collect();
        let mut i = 0;

        while i < tokens.len() {
            let ch = tokens[i];

            if ch == '{' {
                let before_brace = line[..i].trim();
                let current_scope = scope_stack.last().map(|s| s.0).unwrap_or(Scope::TopLevel);

                if before_brace.starts_with("network") || before_brace.contains("network") {
                    network_count += 1;
                    current_network_nodes = 0;
                    scope_stack.push((Scope::InNetwork, line_num));
                } else if before_brace.starts_with("group") || before_brace.contains("group") {
                    scope_stack.push((Scope::InGroup, line_num));
                } else if current_scope == Scope::InNwdiag {
                    network_count += 1;
                    current_network_nodes = 0;
                    scope_stack.push((Scope::InNetwork, line_num));
                } else {
                    scope_stack.push((Scope::InGroup, line_num));
                }
            } else if ch == '}' {
                if let Some((popped_scope, _)) = scope_stack.pop() {
                    if popped_scope == Scope::InNetwork && current_network_nodes == 0 {
                        return Err(NwdiagValidationError::new(
                            Some(line_num),
                            "Network block cannot be empty. At least one node or address must be defined.",
                            Some("Add nodes like 'web01;' or 'web01 [address = \"192.168.1.1\"];' inside the network block"),
                        ));
                    }
                }
            }
            i += 1;
        }

        // Inspect non-brace statements
        let trimmed_stmt = line.trim_matches(|c| c == '{' || c == '}').trim();
        if !trimmed_stmt.is_empty() {
            validate_statement(
                trimmed_stmt,
                line_num,
                scope_stack.last().map(|s| s.0).unwrap_or(Scope::TopLevel),
                &mut peer_count,
                &mut current_network_nodes,
            )?;
        }
    }

    if network_count == 0 && peer_count == 0 {
        return Err(NwdiagValidationError::new(
            None,
            "No 'network' block or peer connection found in nwdiag schema",
            Some("Define at least one network, e.g., 'network lan { server01; }'"),
        ));
    }

    Ok(())
}

fn strip_comments(line: &str) -> String {
    let mut result = String::new();
    let mut in_quote = false;
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '"' {
            in_quote = !in_quote;
            result.push(chars[i]);
        } else if !in_quote && i + 1 < chars.len() && chars[i] == '/' && chars[i + 1] == '/' {
            break;
        } else if !in_quote && chars[i] == '#' {
            break;
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }

    result
}

fn validate_balanced_tokens(schema: &str) -> Result<(), NwdiagValidationError> {
    let mut brace_count = 0;
    let mut bracket_count = 0;
    let mut in_quote = false;
    let mut quote_start_line = 1;
    let mut current_line = 1;

    let chars: Vec<char> = schema.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '\n' {
            current_line += 1;
        }

        if ch == '"' {
            if in_quote {
                in_quote = false;
            } else {
                in_quote = true;
                quote_start_line = current_line;
            }
        } else if !in_quote {
            if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '\n' {
                    current_line += 1;
                }
                i += 1;
                continue;
            }

            if ch == '{' {
                brace_count += 1;
            } else if ch == '}' {
                brace_count -= 1;
                if brace_count < 0 {
                    return Err(NwdiagValidationError::new(
                        Some(current_line),
                        "Unexpected closing brace '}'",
                        Some("Check for unmatched braces"),
                    ));
                }
            } else if ch == '[' {
                bracket_count += 1;
            } else if ch == ']' {
                bracket_count -= 1;
                if bracket_count < 0 {
                    return Err(NwdiagValidationError::new(
                        Some(current_line),
                        "Unexpected closing bracket ']'",
                        Some("Check for unmatched brackets around node attributes"),
                    ));
                }
            }
        }
        i += 1;
    }

    if in_quote {
        return Err(NwdiagValidationError::new(
            Some(quote_start_line),
            "Unclosed string literal",
            Some("Ensure all double quotes '\"' are properly closed"),
        ));
    }

    if brace_count != 0 {
        return Err(NwdiagValidationError::new(
            Some(current_line),
            format!("Unmatched curly braces: {} unclosed '{{'", brace_count),
            Some("Ensure every opening brace '{' has a corresponding closing brace '}'"),
        ));
    }

    if bracket_count != 0 {
        return Err(NwdiagValidationError::new(
            Some(current_line),
            format!("Unmatched square brackets: {} unclosed '['", bracket_count),
            Some("Ensure every opening bracket '[' has a corresponding closing bracket ']'"),
        ));
    }

    Ok(())
}

fn validate_statement(
    stmt: &str,
    line_num: usize,
    current_scope: Scope,
    peer_count: &mut usize,
    network_nodes: &mut usize,
) -> Result<(), NwdiagValidationError> {
    let s = stmt.trim();
    if s.is_empty()
        || s == "nwdiag"
        || s == "network"
        || s.starts_with("network ")
        || s == "group"
        || s.starts_with("group ")
    {
        return Ok(());
    }

    let ends_with_semicolon = s.ends_with(';');
    let content = if ends_with_semicolon {
        &s[..s.len() - 1]
    } else {
        s
    }
    .trim();

    // Check for peer connection: "node1 -- node2"
    if content.contains("--") {
        *peer_count += 1;
        let parts: Vec<&str> = content.split("--").collect();
        if parts.len() != 2 || parts[0].trim().is_empty() || parts[1].trim().is_empty() {
            return Err(NwdiagValidationError::new(
                Some(line_num),
                format!("Invalid peer connection syntax '{}'", content),
                Some("Peer connection must be in format 'nodeA -- nodeB;'"),
            ));
        }
        return Ok(());
    }

    // Check for attribute assignment: "address = ...", "color = ...", etc.
    if content.contains('=') && !content.contains('[') {
        let parts: Vec<&str> = content.splitn(2, '=').collect();
        let key = parts[0].trim();
        let val = parts[1].trim();

        let allowed_keys = [
            "address",
            "color",
            "textcolor",
            "shape",
            "label",
            "description",
            "fontsize",
            "width",
            "height",
            "stacked",
            "class",
        ];
        if !allowed_keys.contains(&key) {
            return Err(NwdiagValidationError::new(
                Some(line_num),
                format!("Unknown property '{}' in statement", key),
                Some(format!(
                    "Supported properties include: {}",
                    allowed_keys.join(", ")
                )),
            ));
        }

        if val.is_empty() {
            return Err(NwdiagValidationError::new(
                Some(line_num),
                format!("Empty value for property '{}'", key),
                Some(format!("Assign a valid value, e.g. {} = \"value\";", key)),
            ));
        }
        return Ok(());
    }

    // Check for node declaration with attributes: "node_name [attr = "val", ...]"
    if let Some(bracket_start) = content.find('[') {
        let bracket_end = content.rfind(']').ok_or_else(|| {
            NwdiagValidationError::new(
                Some(line_num),
                "Missing closing bracket ']' in node attribute definition",
                Some("Close attributes with ']'; e.g. web01 [address = \"192.168.1.1\"];"),
            )
        })?;

        let node_name = content[..bracket_start].trim();
        if node_name.is_empty() {
            return Err(NwdiagValidationError::new(
                Some(line_num),
                "Missing node name before attribute bracket '['",
                Some(
                    "Specify a node identifier before attributes, e.g. router01 [shape = router];",
                ),
            ));
        }

        let attr_str = &content[bracket_start + 1..bracket_end].trim();
        validate_attributes(attr_str, line_num)?;

        if current_scope == Scope::InNetwork || current_scope == Scope::InGroup {
            *network_nodes += 1;
        }
        return Ok(());
    }

    // Simple node or identifier: "node_name;"
    if is_valid_identifier(content) {
        if current_scope == Scope::InNetwork || current_scope == Scope::InGroup {
            *network_nodes += 1;
        }
        return Ok(());
    }

    if !ends_with_semicolon && !s.ends_with('{') && !s.ends_with('}') {
        return Err(NwdiagValidationError::new(
            Some(line_num),
            format!("Missing semicolon ';' at end of statement '{}'", s),
            Some(format!("Add a semicolon at the end: '{};'", s)),
        ));
    }

    Ok(())
}

fn validate_attributes(attrs: &str, line_num: usize) -> Result<(), NwdiagValidationError> {
    if attrs.is_empty() {
        return Ok(());
    }

    let mut items = Vec::new();
    let mut in_quote = false;
    let mut current = String::new();

    for ch in attrs.chars() {
        if ch == '"' {
            in_quote = !in_quote;
            current.push(ch);
        } else if ch == ',' && !in_quote {
            items.push(current.trim().to_string());
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if !current.trim().is_empty() {
        items.push(current.trim().to_string());
    }

    let allowed_attr_keys = [
        "address",
        "color",
        "textcolor",
        "shape",
        "label",
        "description",
        "fontsize",
        "width",
        "height",
        "stacked",
        "class",
    ];

    let allowed_shapes = [
        "box",
        "square",
        "roundedbox",
        "dots",
        "circle",
        "ellipse",
        "diamond",
        "minidiamond",
        "note",
        "mail",
        "cloud",
        "actor",
        "beginpoint",
        "endpoint",
        "condition",
        "database",
    ];

    for item in items {
        if item.is_empty() {
            continue;
        }
        if !item.contains('=') {
            if item == "stacked" {
                continue;
            }
            return Err(NwdiagValidationError::new(
                Some(line_num),
                format!("Invalid attribute syntax '{}'", item),
                Some("Attributes should be in 'key = \"value\"' format (e.g. address = \"192.168.1.1\")"),
            ));
        }

        let parts: Vec<&str> = item.splitn(2, '=').collect();
        let k = parts[0].trim();
        let v = parts[1].trim();

        if !allowed_attr_keys.contains(&k) {
            return Err(NwdiagValidationError::new(
                Some(line_num),
                format!("Unknown attribute key '{}'", k),
                Some(format!(
                    "Supported attributes: {}",
                    allowed_attr_keys.join(", ")
                )),
            ));
        }

        if k == "shape" {
            let shape_val = v.trim_matches('"').trim();
            if !allowed_shapes.contains(&shape_val) {
                return Err(NwdiagValidationError::new(
                    Some(line_num),
                    format!("Unsupported shape '{}'", shape_val),
                    Some(format!("Available shapes: {}", allowed_shapes.join(", "))),
                ));
            }
        }
    }

    Ok(())
}

fn is_valid_identifier(ident: &str) -> bool {
    if ident.is_empty() {
        return false;
    }
    ident
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_simple_schema() {
        let schema = r#"
            nwdiag {
                network dmz {
                    address = "210.x.x.x/24";
                    web01 [address = "210.x.x.1"];
                    web02 [address = "210.x.x.2"];
                }
                network internal {
                    address = "172.x.x.x/24";
                    web01 [address = "172.x.x.1"];
                    web02 [address = "172.x.x.2"];
                    db01;
                    db02;
                }
            }
        "#;
        assert!(validate_nwdiag_schema(schema).is_ok());
    }

    #[test]
    fn test_valid_peer_and_shapes() {
        let schema = r#"
            nwdiag {
                inet [shape = cloud];
                inet -- router;

                network {
                    router;
                    web01;
                    web02;
                }
            }
        "#;
        assert!(validate_nwdiag_schema(schema).is_ok());
    }

    #[test]
    fn test_valid_groups() {
        let schema = r##"
            nwdiag {
                group {
                    color = "#FF7777";
                    web01;
                    web02;
                    db01;
                }
                network dmz {
                    web01;
                    web02;
                }
                network internal {
                    web01;
                    web02;
                    db01;
                }
            }
        "##;
        assert!(validate_nwdiag_schema(schema).is_ok());
    }

    #[test]
    fn test_empty_schema() {
        let res = validate_nwdiag_schema("");
        assert!(res.is_err());
        assert_eq!(res.unwrap_err().message, "Schema cannot be empty");
    }

    #[test]
    fn test_missing_nwdiag_wrapper() {
        let schema = r#"
            network dmz {
                web01;
            }
        "#;
        let res = validate_nwdiag_schema(schema);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .message
            .contains("Schema must start with 'nwdiag {'"));
    }

    #[test]
    fn test_unbalanced_braces() {
        let schema = r#"
            nwdiag {
                network dmz {
                    web01;
            }
        "#;
        let res = validate_nwdiag_schema(schema);
        assert!(res.is_err());
        assert!(res.unwrap_err().message.contains("Unmatched curly braces"));
    }

    #[test]
    fn test_empty_network_block() {
        let schema = r#"
            nwdiag {
                network dmz {
                }
            }
        "#;
        let res = validate_nwdiag_schema(schema);
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .message
            .contains("Network block cannot be empty"));
    }

    #[test]
    fn test_invalid_shape() {
        let schema = r#"
            nwdiag {
                router [shape = invalid_shape_type];
                network dmz {
                    router;
                }
            }
        "#;
        let res = validate_nwdiag_schema(schema);
        assert!(res.is_err());
        assert!(res.unwrap_err().message.contains("Unsupported shape"));
    }

    #[test]
    fn test_unclosed_quotes() {
        let schema = r#"
            nwdiag {
                network dmz {
                    address = "192.168.1.0/24;
                    web01;
                }
            }
        "#;
        let res = validate_nwdiag_schema(schema);
        assert!(res.is_err());
        assert!(res.unwrap_err().message.contains("Unclosed string literal"));
    }
}
