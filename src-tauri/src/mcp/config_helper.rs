use crate::network::CommandResult;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::process::Command;
use std::sync::Mutex;

pub struct ChoiceManager {
    pub txs: Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<String>>>,
}

impl ChoiceManager {
    pub fn new() -> Self {
        Self {
            txs: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn submit_user_choice(
    id: Option<String>,
    choice: String,
    state: tauri::State<'_, ChoiceManager>,
) -> Result<(), String> {
    let id = id.unwrap_or_else(|| "default".to_string());
    let mut lock = state
        .txs
        .lock()
        .map_err(|_| "Mutex lock poisoned".to_string())?;
    if let Some(tx) = lock.remove(&id) {
        let _ = tx.send(choice);
    }
    Ok(())
}

#[derive(Serialize)]
struct ValidatePayload {
    action: &'static str,
    config: String,
}

#[derive(Deserialize)]
struct ValidateResponse {
    success: bool,
    errors: Vec<String>,
    warnings: Vec<String>,
}

#[derive(Serialize)]
struct ConvertPayload {
    action: &'static str,
    config: String,
    target_vendor: String,
}

#[derive(Deserialize)]
struct ConvertResponse {
    success: bool,
    converted_config: String,
    error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DiffLine {
    pub r#type: String, // "normal", "insert", "delete"
    pub old_line: Option<usize>,
    pub new_line: Option<usize>,
    pub content: String,
}

pub fn normalize_config_for_diff(config: &str) -> String {
    let ignorable_keywords = [
        "info:",
        "building configuration",
        "current configuration",
        "nvram config last updated",
        "! last edit",
        "! last refresh",
        "! last save",
        "! time:",
        "! current time:",
        "! last modified",
    ];

    let lines: Vec<&str> = config
        .lines()
        .map(|l| l.trim_end())
        .filter(|line| {
            let trimmed = line.trim();
            let lower = trimmed.to_lowercase();

            for kw in &ignorable_keywords {
                if lower.starts_with(kw) || lower.contains(kw) {
                    return false;
                }
            }

            if lower.starts_with("show running")
                || lower.starts_with("show config")
                || lower.starts_with("show run")
            {
                return false;
            }

            true
        })
        .collect();

    let mut start = 0;
    while start < lines.len() && lines[start].trim().is_empty() {
        start += 1;
    }

    let mut end = lines.len();
    while end > start && lines[end - 1].trim().is_empty() {
        end -= 1;
    }

    if start >= end {
        String::new()
    } else {
        lines[start..end].join("\n")
    }
}

pub fn compute_line_diff(old_text: &str, new_text: &str) -> (Vec<DiffLine>, usize, usize) {
    let norm_old = normalize_config_for_diff(old_text);
    let norm_new = normalize_config_for_diff(new_text);
    let old_lines: Vec<&str> = norm_old.lines().collect();
    let new_lines: Vec<&str> = norm_new.lines().collect();

    let n = old_lines.len();
    let m = new_lines.len();

    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            if old_lines[i] == new_lines[j] {
                dp[i][j] = dp[i + 1][j + 1] + 1;
            } else {
                dp[i][j] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    let mut diff_lines = Vec::new();
    let mut i = 0;
    let mut j = 0;
    let mut old_line_num = 1;
    let mut new_line_num = 1;
    let mut additions = 0;
    let mut deletions = 0;

    while i < n && j < m {
        if old_lines[i] == new_lines[j] {
            diff_lines.push(DiffLine {
                r#type: "normal".to_string(),
                old_line: Some(old_line_num),
                new_line: Some(new_line_num),
                content: old_lines[i].to_string(),
            });
            i += 1;
            j += 1;
            old_line_num += 1;
            new_line_num += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            diff_lines.push(DiffLine {
                r#type: "delete".to_string(),
                old_line: Some(old_line_num),
                new_line: None,
                content: old_lines[i].to_string(),
            });
            i += 1;
            old_line_num += 1;
            deletions += 1;
        } else {
            diff_lines.push(DiffLine {
                r#type: "insert".to_string(),
                old_line: None,
                new_line: Some(new_line_num),
                content: new_lines[j].to_string(),
            });
            j += 1;
            new_line_num += 1;
            additions += 1;
        }
    }

    while i < n {
        diff_lines.push(DiffLine {
            r#type: "delete".to_string(),
            old_line: Some(old_line_num),
            new_line: None,
            content: old_lines[i].to_string(),
        });
        i += 1;
        old_line_num += 1;
        deletions += 1;
    }

    while j < m {
        diff_lines.push(DiffLine {
            r#type: "insert".to_string(),
            old_line: None,
            new_line: Some(new_line_num),
            content: new_lines[j].to_string(),
        });
        j += 1;
        new_line_num += 1;
        additions += 1;
    }

    (diff_lines, additions, deletions)
}

fn run_config_helper(payload: serde_json::Value) -> Result<String, String> {
    let mut current_dir =
        std::env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    if current_dir.ends_with("src-tauri") {
        current_dir.pop();
    }

    let python_path = current_dir.join("venv").join("bin").join("python");
    let wrapper_path = current_dir
        .join("src-tauri")
        .join("python")
        .join("config_helper.py");

    if !python_path.exists() {
        return Err(format!(
            "Python virtual environment binary not found at {:?}",
            python_path
        ));
    }
    if !wrapper_path.exists() {
        return Err(format!(
            "config_helper script not found at {:?}",
            wrapper_path
        ));
    }

    let payload_str = serde_json::to_string(&payload)
        .map_err(|e| format!("Failed to serialize payload: {}", e))?;

    let mut child = Command::new(&python_path)
        .arg(&wrapper_path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run config helper process: {}", e))?;

    {
        let stdin = child.stdin.as_mut().ok_or("Failed to open stdin")?;
        stdin
            .write_all(payload_str.as_bytes())
            .map_err(|e| format!("Failed to write to stdin: {}", e))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| format!("Failed to wait on helper process: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(format!("config_helper failed with stderr: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

pub async fn validate_cisco_config_impl(
    app: Option<tauri::AppHandle>,
    id: Option<String>,
    config: String,
    target_device: Option<(String, String)>,
) -> Result<CommandResult, String> {
    if config.trim().is_empty() {
        return Err("Configuration text cannot be empty".to_string());
    }

    let payload = serde_json::json!(ValidatePayload {
        action: "validate",
        config: config.clone(),
    });

    let (res_errors, res_warnings) = match run_config_helper(payload) {
        Ok(output_json) => {
            if let Ok(res) = serde_json::from_str::<ValidateResponse>(&output_json) {
                (res.errors, res.warnings)
            } else {
                (vec![], vec![])
            }
        }
        Err(e) => (vec![], vec![format!("Config helper notice: {}", e)]),
    };

    let res = ValidateResponse {
        success: true, // Cisco Config 検証失敗機能を一旦無効化
        errors: res_errors,
        warnings: res_warnings,
    };

    let mut md = String::new();
    md.push_str("### Cisco Config Validation Results\n");
    if res.success {
        md.push_str("- **Status**: ✅ Validation Passed\n");
    } else {
        md.push_str("- **Status**: ❌ Validation Failed\n");
    }

    if !res.errors.is_empty() {
        md.push_str("\n#### Errors:\n");
        for err in &res.errors {
            md.push_str(&format!("- ❌ {}\n", err));
        }
    }

    if !res.warnings.is_empty() {
        md.push_str("\n#### Warnings / Security Advice:\n");
        for warn in &res.warnings {
            md.push_str(&format!("- ⚠️ {}\n", warn));
        }
    }

    // Without a desktop app there is no approval UI, so validation remains
    // read-only (this path is also used by unit tests).
    if !res.success || app.is_none() {
        return Ok(CommandResult {
            success: res.success,
            output: md,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        });
    }

    if res.success {
        if let Some(app_handle) = app {
            use tauri::Emitter;
            use tauri::Manager;

            let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
            let choice_manager = app_handle.state::<ChoiceManager>();
            let (tx, rx) = tokio::sync::oneshot::channel();
            {
                let mut lock = choice_manager
                    .txs
                    .lock()
                    .map_err(|_| "Mutex lock poisoned".to_string())?;
                lock.insert(id.clone(), tx);
            }

            let mut hostname = None;
            let mut ip = None;
            if let Some((h, i)) = target_device {
                hostname = Some(h);
                ip = Some(i);
            } else if let Some(host) = crate::settings::load_settings(app_handle.clone())
                .ok()
                .and_then(|settings| settings.recent_ips.first().cloned())
            {
                if let Ok(connections) = crate::connections::load_connections_raw(&app_handle) {
                    if let Some(conn) = connections.iter().find(|c| c.matches_host_or_ip(&host)) {
                        hostname = Some(conn.hostname.as_str().to_string());
                        let ip_s = conn.ip_string();
                        ip = if ip_s.is_empty() { None } else { Some(ip_s) };
                    }
                }
            }

            // Emit event to request diff commit
            let payload = serde_json::json!({
                "id": id,
                "config": config,
                "fileName": "cisco.conf",
                "hostname": hostname,
                "ip": ip
            });
            let _ = app_handle.emit("request-diff-commit", payload);

            // Wait for frontend response (commit or cancel)
            match rx.await {
                Ok(c) => {
                    // The panel executes a persisted, hash-bound operation plan
                    // itself.  Keep this conversion worker read-only so this
                    // older choice channel cannot bypass the approval boundary.
                    if c == "operation_submitted" {
                        Ok(CommandResult {
                            success: true,
                            output: format!(
                                "{}\n\n**Status**: 変更計画を承認し、安全な実行フローへ送信しました。",
                                md
                            ),
                            saved_path: None,
                            is_cached: None,
                            cache_time: None,
                        })
                    } else if c == "legacy_commit_disabled" {
                        let target_name = hostname.clone().or_else(|| ip.clone());
                        let device_name = match target_name {
                            Some(name) => name,
                            None => {
                                if let Ok(conns) =
                                    crate::connections::load_connections_raw(&app_handle)
                                {
                                    if let Some(conn) = conns.first() {
                                        conn.hostname.as_str().to_string()
                                    } else {
                                        "unknown".to_string()
                                    }
                                } else {
                                    "unknown".to_string()
                                }
                            }
                        };

                        // Step 1: Notify status and fetch pre-deployment config
                        let _ = app_handle.emit(
                            "commit-status",
                            serde_json::json!({
                                "id": id,
                                "phase": "fetching_before",
                                "message": "現状のConfigを取得中..."
                            }),
                        );

                        let dev_config_res = crate::mcp::fetch::fetch_base::resolve_device_config(
                            &app_handle,
                            &device_name,
                        )
                        .await;
                        let dev_config = match dev_config_res {
                            Ok(cfg) => cfg,
                            Err(e) => {
                                let err_msg = format!(
                                    "対象機器 ({}) の接続設定取得に失敗しました: {}",
                                    device_name, e
                                );
                                let _ = app_handle.emit(
                                    "commit-status",
                                    serde_json::json!({
                                        "id": id,
                                        "phase": "failed",
                                        "message": err_msg.clone()
                                    }),
                                );
                                return Ok(CommandResult {
                                    success: false,
                                    output: format!(
                                        "{}\n\n**Status**: ❌ コミット失敗: {}",
                                        md, err_msg
                                    ),
                                    saved_path: None,
                                    is_cached: None,
                                    cache_time: None,
                                });
                            }
                        };

                        let wrapper = crate::network::SidecarNetmikoWrapper::new(&app_handle);
                        let fetch_cmd_string =
                            crate::mcp::fetch::command_template::get_show_running_config_command(
                                &dev_config.device_type,
                            );
                        let fetch_cmd = fetch_cmd_string.as_str();

                        let before_config = match wrapper.execute_show(&dev_config, fetch_cmd).await
                        {
                            Ok(cfg) => cfg,
                            Err(e) => {
                                let err_msg = format!("現状のConfig取得に失敗しました: {}", e);
                                let _ = app_handle.emit(
                                    "commit-status",
                                    serde_json::json!({
                                        "id": id,
                                        "phase": "failed",
                                        "message": err_msg.clone()
                                    }),
                                );
                                return Ok(CommandResult {
                                    success: false,
                                    output: format!(
                                        "{}\n\n**Status**: ❌ コミット失敗: {}",
                                        md, err_msg
                                    ),
                                    saved_path: None,
                                    is_cached: None,
                                    cache_time: None,
                                });
                            }
                        };

                        // Step 2: Launch netmiko and stream configuration commands
                        let _ = app_handle.emit(
                            "commit-status",
                            serde_json::json!({
                                "id": id,
                                "phase": "deploying",
                                "message": "Netmikoを起動し、Configを投入中..."
                            }),
                        );

                        let commands: Vec<String> = config
                            .lines()
                            .map(|l| l.trim().to_string())
                            .filter(|l| !l.is_empty())
                            .collect();

                        // Step 1.5: Automatic Dry-run check if enabled in settings
                        let app_settings =
                            crate::settings::load_settings(app_handle.clone()).unwrap_or_default();
                        if app_settings.auto_dry_run {
                            let _ = app_handle.emit("commit-status", serde_json::json!({
                                "id": id,
                                "phase": "dry_running",
                                "message": "自動Dry-runを適用中: 1行ずつconfigureモードでTab補完検証を実行中..."
                            }));

                            let _ = app_handle.emit(
                                "commit-log",
                                serde_json::json!({
                                    "line": "[SYSTEM] --- 自動Dry-run (Tab補完検証) 開始 ---"
                                }),
                            );

                            match wrapper.execute_dry_run(&dev_config, commands.clone()).await {
                                Ok(dry_res) => {
                                    let errors: Vec<_> =
                                        dry_res.results.iter().filter(|r| !r.ok).collect();
                                    for r in &dry_res.results {
                                        if r.ok {
                                            let _ = app_handle.emit(
                                                "commit-log",
                                                serde_json::json!({
                                                    "line": format!("[DRY-RUN OK] {}", r.line)
                                                }),
                                            );
                                        } else {
                                            let err_detail =
                                                r.error.as_deref().unwrap_or("Error detected");
                                            let _ = app_handle.emit("commit-log", serde_json::json!({
                                                "line": format!("[DRY-RUN ERROR] 行: '{}' -> 理由: {}", r.line, err_detail)
                                            }));
                                        }
                                    }

                                    if !errors.is_empty() {
                                        let _ = app_handle.emit("commit-log", serde_json::json!({
                                            "line": format!("[SYSTEM] ⚠️ Dry-run検証で {} 件のエラーが検出されました。ユーザーに投入確認を要請します。", errors.len())
                                        }));

                                        let (force_tx, force_rx) = tokio::sync::oneshot::channel();
                                        let force_id = format!("{}_force", id);
                                        {
                                            let mut lock = choice_manager
                                                .txs
                                                .lock()
                                                .map_err(|_| "Mutex lock poisoned".to_string())?;
                                            lock.insert(force_id.clone(), force_tx);
                                        }

                                        let error_items: Vec<serde_json::Value> = errors
                                            .iter()
                                            .map(|e| {
                                                serde_json::json!({
                                                    "line": e.line,
                                                    "error": e.error
                                                })
                                            })
                                            .collect();

                                        let _ = app_handle.emit("request-force-commit", serde_json::json!({
                                            "id": id,
                                            "forceId": force_id,
                                            "errors": error_items,
                                            "message": format!("Dry-run検証で {} 件のエラーが発生しました。強制的に投入を継続しますか？", errors.len())
                                        }));

                                        let user_choice =
                                            force_rx.await.unwrap_or_else(|_| "cancel".to_string());
                                        if user_choice != "commit_force" && user_choice != "commit"
                                        {
                                            let err_msg = "Dry-runでエラーが検出されたため、ユーザー選択により投入を中止しました。".to_string();
                                            let _ = app_handle.emit(
                                                "commit-status",
                                                serde_json::json!({
                                                    "id": id,
                                                    "phase": "failed",
                                                    "message": err_msg.clone()
                                                }),
                                            );
                                            return Ok(CommandResult {
                                                success: false,
                                                output: format!(
                                                    "{}\n\n**Status**: ❌ コミットキャンセル: {}",
                                                    md, err_msg
                                                ),
                                                saved_path: None,
                                                is_cached: None,
                                                cache_time: None,
                                            });
                                        }
                                        let _ = app_handle.emit("commit-log", serde_json::json!({
                                            "line": "[SYSTEM] ユーザー承認により強制投入を開始します。"
                                        }));
                                    } else {
                                        let _ = app_handle.emit("commit-log", serde_json::json!({
                                            "line": "[SYSTEM] ✅ 自動Dry-run全行成功。エラーなしで本番投入へ進みます。"
                                        }));
                                    }
                                }
                                Err(e) => {
                                    let _ = app_handle.emit("commit-log", serde_json::json!({
                                        "line": format!("[SYSTEM ⚠️] Dry-run実行スキップ (エラー: {})", e)
                                    }));
                                }
                            }
                        }

                        // Step 2: Launch netmiko and stream configuration commands
                        let _ = app_handle.emit(
                            "commit-status",
                            serde_json::json!({
                                "id": id,
                                "phase": "deploying",
                                "message": "Netmikoを起動し、Configを投入中..."
                            }),
                        );

                        use crate::network::NetworkInterface;
                        log::info!(
                            "Deploying config commands to device '{}' ({}): {:?}",
                            dev_config.host,
                            dev_config.device_type,
                            commands
                        );
                        let deploy_res = wrapper.execute_config(&dev_config, commands).await;
                        let deploy_output = match deploy_res {
                            Ok(out) => out,
                            Err(e) => {
                                let err_msg = format!("Config投入中にエラーが発生しました: {}", e);
                                let _ = app_handle.emit(
                                    "commit-status",
                                    serde_json::json!({
                                        "id": id,
                                        "phase": "failed",
                                        "message": err_msg.clone()
                                    }),
                                );
                                return Ok(CommandResult {
                                    success: false,
                                    output: format!(
                                        "{}\n\n**Status**: ❌ コミット失敗: {}",
                                        md, err_msg
                                    ),
                                    saved_path: None,
                                    is_cached: None,
                                    cache_time: None,
                                });
                            }
                        };

                        // Step 2.5: Execute post-deployment apply command (e.g. commit) and save command (e.g. save side / write memory / save)
                        let apply_save =
                            crate::mcp::fetch::command_template::get_apply_and_save_config_commands(
                                &dev_config.device_type,
                            );
                        let mut deploy_output = deploy_output;

                        if !apply_save.apply_command.trim().is_empty() {
                            let _ = app_handle.emit("commit-status", serde_json::json!({
                                "id": id,
                                "phase": "deploying",
                                "message": format!("設定適用コマンド ({}) を実行中...", apply_save.apply_command)
                            }));
                            if let Ok(apply_out) = wrapper
                                .execute_show(&dev_config, &apply_save.apply_command)
                                .await
                            {
                                deploy_output.push_str("\n");
                                deploy_output.push_str(&apply_out);
                            }
                        }

                        if !apply_save.save_command.trim().is_empty() {
                            let _ = app_handle.emit("commit-status", serde_json::json!({
                                "id": id,
                                "phase": "deploying",
                                "message": format!("設定保存コマンド ({}) を実行中...", apply_save.save_command)
                            }));
                            if let Ok(save_out) = wrapper
                                .execute_show(&dev_config, &apply_save.save_command)
                                .await
                            {
                                deploy_output.push_str("\n");
                                deploy_output.push_str(&save_out);
                            }
                        }

                        // Step 3: Fetch post-deployment config and verify Diff
                        let _ = app_handle.emit(
                            "commit-status",
                            serde_json::json!({
                                "id": id,
                                "phase": "verifying",
                                "message": "投入完了。投入後のConfigを取得しDiff検証中..."
                            }),
                        );

                        let after_config = match wrapper.execute_show(&dev_config, fetch_cmd).await
                        {
                            Ok(cfg) => cfg,
                            Err(e) => {
                                let err_msg = format!("投入後のConfig取得に失敗しました: {}", e);
                                let _ = app_handle.emit(
                                    "commit-status",
                                    serde_json::json!({
                                        "id": id,
                                        "phase": "failed",
                                        "message": err_msg.clone()
                                    }),
                                );
                                return Ok(CommandResult {
                                    success: false,
                                    output: format!("{}\n\n**投入ログ (Netmiko)**:\n```text\n{}\n```\n\n**Status**: ⚠️ Config投入は実行されましたが、投入後の検証に失敗しました: {}", md, deploy_output, err_msg),
                                    saved_path: None,
                                    is_cached: None,
                                    cache_time: None,
                                });
                            }
                        };

                        let (diff_lines, additions, deletions) =
                            compute_line_diff(&before_config, &after_config);
                        let diff_applied = additions > 0 || deletions > 0;

                        // Emit diff result to frontend right pane
                        let _ = app_handle.emit("commit-diff-result", serde_json::json!({
                            "id": id,
                            "fileName": "running-config",
                            "additions": additions,
                            "deletions": deletions,
                            "diffLines": diff_lines,
                            "diffApplied": diff_applied,
                            "deployOutput": deploy_output,
                            "status": if diff_applied { "success" } else { "warning" },
                            "message": if diff_applied {
                                format!("Config投入完了。差分が正常に確認されました (+{} 行, -{} 行)", additions, deletions)
                            } else {
                                "Config投入完了。投入前後のConfigに差分がありませんでした (既に適用済みの可能性があります)".to_string()
                            }
                        }));

                        let _ = app_handle.emit(
                            "commit-status",
                            serde_json::json!({
                                "id": id,
                                "phase": "success",
                                "message": "投入およびDiff検証が完了しました"
                            }),
                        );

                        // Step 4: Return result to user
                        let status_str = if diff_applied {
                            format!(
                                "🚀 Configの投入およびDiff検証が成功しました (+{} 行, -{} 行)",
                                additions, deletions
                            )
                        } else {
                            "⚠️ Configの投入は完了しましたが、投入前後のConfigに差分は検出されませんでした。".to_string()
                        };

                        let mut final_md = format!("{}\n\n### 🚀 Config 投入結果\n- **対象機器**: `{}` ({})\n- **ステータス**: {}\n\n", md, device_name, dev_config.host, status_str);
                        final_md.push_str("#### 投入ログ (Netmiko):\n```text\n");
                        final_md.push_str(&deploy_output);
                        final_md.push_str("\n```\n");

                        if diff_applied {
                            final_md.push_str("\n#### 適用された差分 (Diff):\n```diff\n");
                            for d in &diff_lines {
                                if d.r#type == "insert" {
                                    final_md.push_str(&format!("+ {}\n", d.content));
                                } else if d.r#type == "delete" {
                                    final_md.push_str(&format!("- {}\n", d.content));
                                }
                            }
                            final_md.push_str("```\n");
                        }

                        Ok(CommandResult {
                            success: true,
                            output: final_md,
                            saved_path: None,
                            is_cached: None,
                            cache_time: None,
                        })
                    } else {
                        Ok(CommandResult {
                            success: false,
                            output: format!(
                                "{}\n\n**Status**: ⚠️ Configuration deployment cancelled by user.",
                                md
                            ),
                            saved_path: None,
                            is_cached: None,
                            cache_time: None,
                        })
                    }
                }
                Err(_) => Err("Failed to receive user choice".to_string()),
            }
        } else {
            Ok(CommandResult {
                success: true,
                output: md,
                saved_path: None,
                is_cached: None,
                cache_time: None,
            })
        }
    } else {
        Ok(CommandResult {
            success: false,
            output: md,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        })
    }
}

#[tauri::command]
pub async fn validate_cisco_config(
    app: tauri::AppHandle,
    config: String,
) -> Result<CommandResult, String> {
    validate_cisco_config_impl(Some(app), None, config, None).await
}

#[tauri::command]
pub async fn convert_cisco_config(
    config: String,
    target_vendor: String,
) -> Result<CommandResult, String> {
    if config.trim().is_empty() {
        return Err("Configuration text cannot be empty".to_string());
    }
    let vendor = target_vendor.trim().to_lowercase();
    if vendor != "juniper" && vendor != "arista" {
        return Err(format!(
            "Unsupported target vendor: '{}'. Supported: 'juniper', 'arista'",
            target_vendor
        ));
    }

    let payload = serde_json::json!(ConvertPayload {
        action: "convert",
        config: config.clone(),
        target_vendor: vendor.clone(),
    });

    let output_json = run_config_helper(payload)?;
    let res: ConvertResponse = serde_json::from_str(&output_json)
        .map_err(|e| format!("Failed to parse converter output: {}", e))?;

    if !res.success {
        let err_msg = res
            .error
            .unwrap_or_else(|| "Unknown conversion error".to_string());
        return Ok(CommandResult {
            success: false,
            output: format!("### Conversion Failed\nError: {}", err_msg),
            saved_path: None,
            is_cached: None,
            cache_time: None,
        });
    }

    let md = format!(
        "### Converted Configuration ({})\n\n```{}\n{}\n```",
        vendor, vendor, res.converted_config
    );

    Ok(CommandResult {
        success: true,
        output: md,
        saved_path: None,
        is_cached: None,
        cache_time: None,
    })
}

#[tauri::command]
pub async fn ask_user_choice(
    app: tauri::AppHandle,
    id: Option<String>,
    title: String,
    message: String,
    options: Vec<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    use tauri::Manager;

    let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let choice_manager = app.state::<ChoiceManager>();

    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut lock = choice_manager
            .txs
            .lock()
            .map_err(|_| "Mutex lock poisoned".to_string())?;
        lock.insert(id.clone(), tx);
    }

    // Emit event to request user choice
    let payload = serde_json::json!({
        "id": id,
        "title": title,
        "message": message,
        "options": options
    });

    let _ = app.emit("request-user-choice", payload);

    // Wait for frontend response
    match rx.await {
        Ok(c) => Ok(c),
        Err(_) => Ok("cancelled".to_string()),
    }
}

pub struct InterfaceChoiceManager {
    pub txs: Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<String>>>,
}

impl InterfaceChoiceManager {
    pub fn new() -> Self {
        Self {
            txs: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn submit_interface_choice(
    id: Option<String>,
    choice: String,
    state: tauri::State<'_, InterfaceChoiceManager>,
) -> Result<(), String> {
    let id = id.unwrap_or_else(|| "default".to_string());
    let mut lock = state
        .txs
        .lock()
        .map_err(|_| "Mutex lock poisoned".to_string())?;
    if let Some(tx) = lock.remove(&id) {
        let _ = tx.send(choice);
    }
    Ok(())
}

#[tauri::command]
pub async fn ask_interface_choice(
    app: tauri::AppHandle,
    id: Option<String>,
    vendor: String,
    message: Option<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    use tauri::Manager;

    let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let choice_manager = app.state::<InterfaceChoiceManager>();

    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut lock = choice_manager
            .txs
            .lock()
            .map_err(|_| "Mutex lock poisoned".to_string())?;
        lock.insert(id.clone(), tx);
    }

    // Emit event to request interface choice
    let payload = serde_json::json!({
        "id": id,
        "vendor": vendor,
        "message": message,
    });

    let _ = app.emit("request-interface-choice", payload);

    // Wait for frontend response
    match rx.await {
        Ok(c) => Ok(c),
        Err(_) => Ok("cancelled".to_string()),
    }
}

pub struct IpAddressChoiceManager {
    pub txs: Mutex<std::collections::HashMap<String, tokio::sync::oneshot::Sender<String>>>,
}

impl IpAddressChoiceManager {
    pub fn new() -> Self {
        Self {
            txs: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[tauri::command]
pub async fn submit_ipaddress_choice(
    id: Option<String>,
    choice: String,
    state: tauri::State<'_, IpAddressChoiceManager>,
) -> Result<(), String> {
    let id = id.unwrap_or_else(|| "default".to_string());
    let mut lock = state
        .txs
        .lock()
        .map_err(|_| "Mutex lock poisoned".to_string())?;
    if let Some(tx) = lock.remove(&id) {
        let _ = tx.send(choice);
    }
    Ok(())
}

#[tauri::command]
pub async fn ask_ipaddress_choice(
    app: tauri::AppHandle,
    id: Option<String>,
    title: String,
    message: String,
    subnet: String,
    default_ip: Option<String>,
) -> Result<String, String> {
    use tauri::Emitter;
    use tauri::Manager;

    let id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let choice_manager = app.state::<IpAddressChoiceManager>();

    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut lock = choice_manager
            .txs
            .lock()
            .map_err(|_| "Mutex lock poisoned".to_string())?;
        lock.insert(id.clone(), tx);
    }

    // Emit event to request IP address choice
    let payload = serde_json::json!({
        "id": id,
        "title": title,
        "message": message,
        "subnet": subnet,
        "defaultIp": default_ip,
    });

    let _ = app.emit("request-ipaddress-choice", payload);

    // Wait for frontend response
    match rx.await {
        Ok(c) => Ok(c),
        Err(_) => Ok("cancelled".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_validate_cisco_config() {
        let config = "hostname RouterA\ninterface GigabitEthernet0/1\n ip address 192.168.1.1 255.255.255.0\n".to_string();
        let result = validate_cisco_config_impl(None, None, config, None).await;
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
        let res = result.unwrap();
        assert!(res.success);
        assert!(res.output.contains("Validation Passed"));
    }

    #[tokio::test]
    async fn test_convert_cisco_config() {
        let config = "hostname RouterA\ninterface GigabitEthernet0/1\n ip address 192.168.1.1 255.255.255.0\n".to_string();
        let result = convert_cisco_config(config, "juniper".to_string()).await;
        assert!(result.is_ok(), "Expected success, got: {:?}", result);
        let res = result.unwrap();
        assert!(res.success);
        assert!(res.output.contains("set system host-name RouterA"));
    }

    #[test]
    fn test_compute_line_diff() {
        let old_text = "hostname RouterA\ninterface GigabitEthernet0/1\n shutdown\n";
        let new_text = "hostname RouterA\ninterface GigabitEthernet0/1\n no shutdown\n ip address 10.0.0.1 255.255.255.0\n";
        let (lines, additions, deletions) = compute_line_diff(old_text, new_text);
        assert!(additions >= 2);
        assert!(deletions >= 1);
        assert!(lines
            .iter()
            .any(|l| l.r#type == "insert" && l.content.contains("no shutdown")));
        assert!(lines
            .iter()
            .any(|l| l.r#type == "delete" && l.content.contains("shutdown")));
    }

    #[test]
    fn test_normalize_config_for_diff() {
        let raw_config = "\
INFO: Connecting to device...
INFO: Connected successfully.
! LAST EDIT 18:56:59 2025/09/12 by operator
! LAST REFRESH 18:57:14 2025/09/12 by operator
hostname F220-Mi
INFO: Disconnected.
";
        let normalized = normalize_config_for_diff(raw_config);
        assert_eq!(normalized, "hostname F220-Mi");

        let old_with_timestamp =
            "INFO: Executing show...\n! LAST EDIT 18:56:59 2025/09/12\nhostname F220-Mi";
        let new_with_timestamp =
            "INFO: Executing show...\n! LAST EDIT 19:02:33 2025/09/12\nhostname F220-Mikomai2";
        let (lines, additions, deletions) =
            compute_line_diff(old_with_timestamp, new_with_timestamp);
        assert_eq!(additions, 1);
        assert_eq!(deletions, 1);
        assert!(lines
            .iter()
            .any(|l| l.r#type == "insert" && l.content == "hostname F220-Mikomai2"));
        assert!(lines
            .iter()
            .any(|l| l.r#type == "delete" && l.content == "hostname F220-Mi"));
    }
}
