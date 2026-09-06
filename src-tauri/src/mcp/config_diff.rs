use serde::{Deserialize, Serialize};

/// A single normalized line in a configuration diff.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// Serialized as the stable UI contract: `normal`, `insert`, or `delete`.
    pub r#type: String,
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub content: String,
}

pub fn normalize_config_for_diff(config: &str) -> String {
    const IGNORABLE_KEYWORDS: [&str; 11] = [
        "info:", "building configuration", "current configuration",
        "nvram config last updated", "! last edit", "! last refresh", "! last save",
        "! time:", "! current time:", "! last modified", "show running",
    ];

    let lines: Vec<&str> = config.lines().map(str::trim_end).filter(|line| {
        let lower = line.trim().to_lowercase();
        !IGNORABLE_KEYWORDS.iter().any(|keyword| lower.starts_with(keyword) || lower.contains(keyword))
            && !lower.starts_with("show config")
            && !lower.starts_with("show run")
    }).collect();

    let start = lines.iter().position(|line| !line.trim().is_empty()).unwrap_or(lines.len());
    let end = lines.iter().rposition(|line| !line.trim().is_empty()).map(|i| i + 1).unwrap_or(start);
    lines.get(start..end).unwrap_or_default().join("\n")
}

pub fn compute_line_diff(old_text: &str, new_text: &str) -> (Vec<DiffLine>, usize, usize) {
    let normalized_old = normalize_config_for_diff(old_text);
    let normalized_new = normalize_config_for_diff(new_text);
    let old_lines: Vec<&str> = normalized_old.lines().collect();
    let new_lines: Vec<&str> = normalized_new.lines().collect();
    let (n, m) = (old_lines.len(), new_lines.len());
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if old_lines[i] == new_lines[j] { dp[i + 1][j + 1] + 1 } else { dp[i + 1][j].max(dp[i][j + 1]) };
        }
    }

    let mut result = Vec::new();
    let (mut i, mut j, mut old_no, mut new_no, mut additions, mut deletions) = (0, 0, 1, 1, 0, 0);
    while i < n && j < m {
        if old_lines[i] == new_lines[j] {
            result.push(DiffLine { r#type: "normal".to_string(), old_line: Some(old_no), new_line: Some(new_no), content: old_lines[i].to_string() });
            i += 1; j += 1; old_no += 1; new_no += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            result.push(DiffLine { r#type: "delete".to_string(), old_line: Some(old_no), new_line: None, content: old_lines[i].to_string() });
            i += 1; old_no += 1; deletions += 1;
        } else {
            result.push(DiffLine { r#type: "insert".to_string(), old_line: None, new_line: Some(new_no), content: new_lines[j].to_string() });
            j += 1; new_no += 1; additions += 1;
        }
    }
    while i < n {
        result.push(DiffLine { r#type: "delete".to_string(), old_line: Some(old_no), new_line: None, content: old_lines[i].to_string() });
        i += 1; old_no += 1; deletions += 1;
    }
    while j < m {
        result.push(DiffLine { r#type: "insert".to_string(), old_line: None, new_line: Some(new_no), content: new_lines[j].to_string() });
        j += 1; new_no += 1; additions += 1;
    }
    (result, additions, deletions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_removes_cli_noise_and_outer_blank_lines() {
        let input = "\nInfo: generated\nshow run\ninterface Gi0/1  \n description uplink\n\n";
        assert_eq!(normalize_config_for_diff(input), "interface Gi0/1\n description uplink");
    }

    #[test]
    fn diff_tracks_insertions_and_deletions() {
        let (diff, additions, deletions) = compute_line_diff("a\nb", "a\nc");
        assert_eq!((additions, deletions), (1, 1));
        assert_eq!(diff.iter().filter(|line| line.r#type == "insert").count(), 1);
        assert_eq!(diff.iter().filter(|line| line.r#type == "delete").count(), 1);
    }
}
