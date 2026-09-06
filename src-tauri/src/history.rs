use std::fs;
use std::path::PathBuf;
use tauri::Manager;
use crate::graph::SurrealDbState;
use crate::error::TauriError;
use crate::history_store;
#[allow(unused_imports)]
pub use crate::history_attachments::{
    attachment_from_inline, attachment_from_path, prepare_attachments, prepare_attachments_for_sources,
    read_files_as_attachments, MAX_IMAGE_ATTACHMENT_SIZE,
    MAX_INLINE_ATTACHMENT_SIZE, MAX_TEXT_ATTACHMENT_SIZE,
};
#[allow(unused_imports)]
pub use crate::history_types::{
    Attachment, AttachmentPreparation, AttachmentRejection, AttachmentSource, AttachmentType,
    BaseMessage, ChatSession, ExecutionStatus, Folder, HistoryError, HistoryItem, HistoryMutation,
    HistorySnapshot, Message, MessageRole, SummaryItem,
};

pub fn sanitize_history_items(items: &mut Vec<HistoryItem>) -> bool {
    let mut modified = false;
    for item in items.iter_mut() {
        match item {
            HistoryItem::Session(ref mut session) => {
                for msg in session.messages.iter_mut() {
                    match msg {
                        Message::ToolExecution {
                            ref mut status,
                            ref mut base,
                            ref mut summary_text,
                            ref mut raw_data,
                            ref action_name,
                            ref tool_id,
                            ..
                        } => {
                            if *status == ExecutionStatus::Running
                                || base.is_tool_loading == Some(true)
                            {
                                *status = ExecutionStatus::Failed;
                                base.is_tool_loading = Some(false);
                                let label =
                                    action_name.as_deref().unwrap_or_else(|| tool_id.label());
                                *summary_text = format!("{} 失敗", label);
                                if raw_data.is_none()
                                    || raw_data.as_deref().unwrap_or("").trim().is_empty()
                                {
                                    *raw_data = Some(
                                        "アプリケーションが終了したため、MCPの実行が失敗しました。"
                                            .to_string(),
                                    );
                                }
                                modified = true;
                            }
                        }
                        Message::AgentResponse { ref mut base, .. }
                        | Message::UserInput { ref mut base, .. }
                        | Message::SystemMessage { ref mut base } => {
                            if base.is_tool_loading == Some(true) {
                                base.is_tool_loading = Some(false);
                                modified = true;
                            }
                        }
                    }
                }
            }
            HistoryItem::Folder(ref mut folder) => {
                if sanitize_history_items(&mut folder.items) {
                    modified = true;
                }
            }
        }
    }
    modified
}

async fn save_history_to_store(
    db: &SurrealDbState,
    history: &[HistoryItem],
) -> Result<(), TauriError> {
    let value = serde_json::to_value(history)?;
    history_store::save(db, value)
        .await
        .map_err(HistoryError::Database)?;
    Ok(())
}

async fn load_history_from_store(
    db: &SurrealDbState,
) -> Result<Vec<HistoryItem>, TauriError> {
    let stored = history_store::load(db)
        .await
        .map_err(HistoryError::Database)?;
    let mut history = match stored {
        Some(value) => serde_json::from_value(value)?,
        None => vec![],
    };
    if sanitize_history_items(&mut history) {
        save_history_to_store(db, &history).await?;
    }
    Ok(history)
}

#[tauri::command]
pub async fn cleanup_running_history_on_exit(app: tauri::AppHandle) -> Result<(), TauriError> {
    let db = app.state::<SurrealDbState>();
    let mut history = load_history_from_store(&db).await?;
    if sanitize_history_items(&mut history) {
        save_history_to_store(&db, &history).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn load_history(
    db: tauri::State<'_, SurrealDbState>,
) -> Result<Vec<HistoryItem>, TauriError> {
    load_history_from_store(&db).await
}

#[tauri::command]
pub async fn save_history(
    db: tauri::State<'_, SurrealDbState>,
    history: Vec<HistoryItem>,
) -> Result<(), TauriError> {
    save_history_to_store(&db, &history).await
}

fn first_session_id(items: &[HistoryItem]) -> Option<uuid::Uuid> {
    for item in items {
        match item {
            HistoryItem::Session(session) => return Some(session.id),
            HistoryItem::Folder(folder) => {
                if let Some(id) = first_session_id(&folder.items) {
                    return Some(id);
                }
            }
        }
    }
    None
}

fn mutate_items(items: &mut Vec<HistoryItem>, mutation: &HistoryMutation) -> bool {
    match mutation {
        HistoryMutation::RenameSession { session_id, title } => {
            items.iter_mut().any(|item| match item {
                HistoryItem::Session(session) if session.id == *session_id => {
                    session.title = title.trim().to_string();
                    true
                }
                HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
                _ => false,
            })
        }
        HistoryMutation::DeleteSession { session_id } => {
            let before = items.len();
            items.retain(
                |item| !matches!(item, HistoryItem::Session(session) if session.id == *session_id),
            );
            if items.len() != before {
                return true;
            }
            items.iter_mut().any(|item| match item {
                HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
                _ => false,
            })
        }
        HistoryMutation::ToggleFolder { folder_id } => items.iter_mut().any(|item| match item {
            HistoryItem::Folder(folder) if folder.id == *folder_id => {
                folder.is_open = !folder.is_open;
                true
            }
            HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
            _ => false,
        }),
        HistoryMutation::UpdateSessionMessages {
            session_id,
            messages,
        } => items.iter_mut().any(|item| match item {
            HistoryItem::Session(session) if session.id == *session_id => {
                session.messages = messages.clone();
                true
            }
            HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
            _ => false,
        }),
        HistoryMutation::UpdateSessionRecentIps {
            session_id,
            recent_ips,
        } => items.iter_mut().any(|item| match item {
            HistoryItem::Session(session) if session.id == *session_id => {
                session.recent_ips = Some(recent_ips.clone());
                true
            }
            HistoryItem::Folder(folder) => mutate_items(&mut folder.items, mutation),
            _ => false,
        }),
        HistoryMutation::CreateSession { .. } | HistoryMutation::CreateFolder { .. } => false,
    }
}

/// Applies a single tree mutation and saves only a valid history snapshot.  The UI
/// still owns selection and rendering, while identifiers and persisted tree
/// invariants are owned by the backend.
#[tauri::command]
pub async fn mutate_history(
    db: tauri::State<'_, SurrealDbState>,
    mutation: HistoryMutation,
) -> Result<HistorySnapshot, TauriError> {
    let mut history = load_history_from_store(&db).await?;
    match &mutation {
        HistoryMutation::CreateSession { title } => {
            history.push(HistoryItem::Session(ChatSession {
                id: uuid::Uuid::new_v4(),
                title: title
                    .clone()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "New Session".to_string()),
                messages: vec![],
                recent_ips: None,
            }))
        }
        HistoryMutation::CreateFolder { name } => history.push(HistoryItem::Folder(Folder {
            id: uuid::Uuid::new_v4(),
            name: name
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "New Folder".to_string()),
            items: vec![],
            is_open: true,
        })),
        _ => {
            mutate_items(&mut history, &mutation);
        }
    }
    let active_session_id = first_session_id(&history)
        .map(|id| id.to_string())
        .unwrap_or_default();
    save_history_to_store(&db, &history).await?;
    Ok(HistorySnapshot {
        history,
        active_session_id,
    })
}

#[tauri::command]
pub async fn initialize_history(
    db: tauri::State<'_, SurrealDbState>,
) -> Result<HistorySnapshot, TauriError> {
    let history = load_history_from_store(&db).await?;
    Ok(HistorySnapshot {
        active_session_id: first_session_id(&history)
            .map(|id| id.to_string())
            .unwrap_or_default(),
        history,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_serialization() {
        let msg = Message::UserInput {
            base: BaseMessage {
                role: MessageRole::User,
                content: "Hello".to_string(),
                timestamp: None,
                is_tool_loading: None,
                is_hidden: None,
                task_id: None,
            },
            status: None,
            attachments: None,
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains(r#""role":"user""#));
        assert!(serialized.contains(r#""content":"Hello""#));
        assert!(serialized.contains(r#""event_type":"UserInput""#));
    }

    #[test]
    fn test_history_item_session_serialization() {
        let session_id = uuid::Uuid::new_v4();
        let session = ChatSession {
            id: session_id,
            title: "Test Session".to_string(),
            messages: vec![Message::UserInput {
                base: BaseMessage {
                    role: MessageRole::User,
                    content: "Hi".to_string(),
                    timestamp: None,
                    is_tool_loading: None,
                    is_hidden: None,
                    task_id: None,
                },
                status: None,
                attachments: None,
            }],
            recent_ips: None,
        };
        let item = HistoryItem::Session(session);
        let serialized = serde_json::to_string(&item).unwrap();
        assert!(serialized.contains(r#""type":"session""#));
        assert!(serialized.contains(&format!(r#""id":"{}""#, session_id)));
    }

    #[test]
    fn test_history_item_folder_serialization() {
        let folder_id = uuid::Uuid::new_v4();
        let folder = Folder {
            id: folder_id,
            name: "Test Folder".to_string(),
            items: vec![],
            is_open: true,
        };
        let item = HistoryItem::Folder(folder);
        let serialized = serde_json::to_string(&item).unwrap();
        assert!(serialized.contains(r#""type":"folder""#));
        assert!(serialized.contains(r#""name":"Test Folder""#));
        assert!(serialized.contains(r#""isOpen":true"#));
        assert!(serialized.contains(&format!(r#""id":"{}""#, folder_id)));
    }

    #[test]
    fn test_summary_item_serialization() {
        let summary = SummaryItem {
            timestamp: "2023-10-27T10:00:00Z"
                .parse::<chrono::DateTime<chrono::Utc>>()
                .unwrap(),
            content: "Test summary".to_string(),
        };
        let serialized = serde_json::to_string(&summary).unwrap();
        assert!(serialized.contains(r#""timestamp":"2023-10-27T10:00:00Z""#));
        assert!(serialized.contains(r#""content":"Test summary""#));
    }

    #[test]
    fn test_sanitize_history_items_running_mcp() {
        let session_id = uuid::Uuid::new_v4();
        let mut history = vec![HistoryItem::Session(ChatSession {
            id: session_id,
            title: "Test Session".to_string(),
            messages: vec![Message::ToolExecution {
                base: BaseMessage {
                    role: MessageRole::Ai,
                    content: "".to_string(),
                    timestamp: None,
                    is_tool_loading: Some(true),
                    is_hidden: None,
                    task_id: Some(uuid::Uuid::new_v4()),
                },
                status: ExecutionStatus::Running,
                action_name: Some("インターフェース選択".to_string()),
                tool_id: crate::mcp::ToolKind::AskInterfaceChoice,
                summary_text: "ask_interface_choice を実行中...".to_string(),
                raw_data: None,
                args: None,
                saved_path: None,
                is_cached: None,
                cache_time: None,
                waiting_for_approval: None,
            }],
            recent_ips: None,
        })];

        let modified = sanitize_history_items(&mut history);
        assert!(modified);

        if let HistoryItem::Session(session) = &history[0] {
            if let Message::ToolExecution {
                status,
                base,
                summary_text,
                raw_data,
                ..
            } = &session.messages[0]
            {
                assert_eq!(*status, ExecutionStatus::Failed);
                assert_eq!(base.is_tool_loading, Some(false));
                assert_eq!(summary_text, "インターフェース選択 失敗");
                assert_eq!(
                    raw_data.as_deref(),
                    Some("アプリケーションが終了したため、MCPの実行が失敗しました。")
                );
            } else {
                panic!("Expected ToolExecution message");
            }
        } else {
            panic!("Expected Session item");
        }
    }

    #[test]
    fn test_attachment_serialization_with_path() {
        let att = Attachment {
            name: "firmware.bin".to_string(),
            mime_type: AttachmentType::File,
            content: "[ファイル: firmware.bin (サイズ: 1.2 MB)]".to_string(),
            path: Some(PathBuf::from("/tmp/firmware.bin")),
        };
        let json = serde_json::to_string(&att).unwrap();
        assert!(json.contains(r#""name":"firmware.bin""#));
        assert!(json.contains(r#""type":"file""#));
        assert!(json.contains(r#""path":"/tmp/firmware.bin""#));

        let deserialized: Attachment = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "firmware.bin");
        assert_eq!(deserialized.mime_type, AttachmentType::File);
        assert_eq!(deserialized.path, Some(PathBuf::from("/tmp/firmware.bin")));
    }

    #[test]
    fn test_read_files_as_attachments_text_and_binary() {
        use std::io::Write;
        let temp_dir = std::env::temp_dir();
        let text_path = temp_dir.join("test_text_file.txt");
        let bin_path = temp_dir.join("test_bin_file.bin");

        let mut f_text = fs::File::create(&text_path).unwrap();
        writeln!(f_text, "hostname Switch1\ninterface GigabitEthernet0/1").unwrap();

        let mut f_bin = fs::File::create(&bin_path).unwrap();
        // Write invalid utf-8 binary bytes
        f_bin
            .write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0xFF, 0xFE, 0x00, 0x01])
            .unwrap();

        let paths = vec![
            text_path.to_string_lossy().to_string(),
            bin_path.to_string_lossy().to_string(),
        ];

        let atts = prepare_attachments_for_sources(
            paths.into_iter().map(|path| AttachmentSource::Path { path }).collect(),
            true,
        ).attachments;
        assert_eq!(atts.len(), 2);

        let text_att = &atts[0];
        assert_eq!(text_att.name, "test_text_file.txt");
        assert_eq!(text_att.mime_type, AttachmentType::Text);
        assert!(text_att.content.contains("hostname Switch1"));
        assert_eq!(text_att.path, Some(text_path.clone()));

        let bin_att = &atts[1];
        assert_eq!(bin_att.name, "test_bin_file.bin");
        assert_eq!(bin_att.mime_type, AttachmentType::File);
        assert!(bin_att.content.contains("test_bin_file.bin"));
        assert_eq!(bin_att.path, Some(bin_path.clone()));

        let _ = fs::remove_file(text_path);
        let _ = fs::remove_file(bin_path);
    }

    #[test]
    fn prepares_inline_attachments_and_rejects_duplicates_and_large_content() {
        let result = prepare_attachments_for_sources(vec![
            AttachmentSource::Inline {
                name: "note.txt".to_string(),
                content: "router configuration".to_string(),
                media_type: Some("text/plain".to_string()),
            },
            AttachmentSource::Inline {
                name: "note.txt".to_string(),
                content: "duplicate".to_string(),
                media_type: Some("text/plain".to_string()),
            },
            AttachmentSource::Inline {
                name: "large.txt".to_string(),
                content: "x".repeat(MAX_INLINE_ATTACHMENT_SIZE + 1),
                media_type: Some("text/plain".to_string()),
            },
        ], true);
        assert_eq!(result.attachments.len(), 1);
        assert_eq!(result.attachments[0].mime_type, AttachmentType::Text);
        assert_eq!(result.rejected.len(), 2);
    }

    #[test]
    fn rejects_images_when_vision_is_not_ready() {
        let result = prepare_attachments_for_sources(
            vec![AttachmentSource::Inline {
                name: "diagram.png".to_string(),
                content: "data:image/png;base64,AA==".to_string(),
                media_type: Some("image/png".to_string()),
            }],
            false,
        );
        assert!(result.attachments.is_empty());
        assert_eq!(result.rejected.len(), 1);
        assert!(result.rejected[0].reason.contains("Vision"));
    }

    #[test]
    fn rejects_oversized_inline_attachments() {
        let result = prepare_attachments_for_sources(
            vec![AttachmentSource::Inline {
                name: "large.txt".to_string(),
                content: "x".repeat(MAX_INLINE_ATTACHMENT_SIZE + 1),
                media_type: Some("text/plain".to_string()),
            }],
            true,
        );
        assert!(result.attachments.is_empty());
        assert_eq!(result.rejected.len(), 1);
        assert!(result.rejected[0].reason.contains("512 KB"));
    }

    #[test]
    fn deleting_the_last_session_leaves_an_empty_history() {
        let id = uuid::Uuid::new_v4();
        let mut items = vec![HistoryItem::Session(ChatSession {
            id,
            title: "Session".to_string(),
            messages: vec![],
            recent_ips: None,
        })];
        assert!(mutate_items(&mut items, &HistoryMutation::DeleteSession { session_id: id }));
        assert!(items.is_empty());
        assert!(first_session_id(&items).is_none());
    }

    #[test]
    fn test_mutation_deserialization() {
        let json_data = r#"{
            "type": "updateSessionMessages",
            "sessionId": "a0000000-0000-0000-0000-000000000001",
            "messages": [
                {
                    "role": "user",
                    "content": "fitelnetのVLAN設定を教えて",
                    "event_type": "UserInput",
                    "task_id": "a0000000-0000-0000-0000-000000000002"
                },
                {
                    "role": "ai",
                    "content": "VLAN設定には、アクセスVLANとトランクVLANの2種類があります。",
                    "event_type": "AgentResponse",
                    "task_id": "a0000000-0000-0000-0000-000000000003",
                    "summary_text": "エージェントによる解析を開始"
                },
                {
                    "role": "ai",
                    "content": "",
                    "event_type": "ToolExecution",
                    "task_id": "a0000000-0000-0000-0000-000000000004",
                    "status": "Success",
                    "action_name": "設定取得",
                    "tool_id": "fetch_config",
                    "summary_text": "設定取得 完了",
                    "raw_data": "interface GigaEthernet 1/1",
                    "waitingForApproval": false
                }
            ]
        }"#;
        let mutation: Result<HistoryMutation, _> = serde_json::from_str(json_data);
        assert!(mutation.is_ok(), "Failed to deserialize mutation: {:?}", mutation.err());
    }

    #[tokio::test]
    async fn test_full_surrealdb_history_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!("mikomai-full-test-{}", uuid::Uuid::new_v4()));
        let state = SurrealDbState::initialize_at(&temp_dir).await.unwrap();
        history_store::initialize(&state).await.unwrap();

        // 1. Initial state is empty
        let initial = load_history_from_store(&state).await.unwrap();
        assert!(initial.is_empty());

        // 2. Create session and save messages
        let session_id = uuid::Uuid::new_v4();
        let user_task_id = uuid::Uuid::new_v4();
        let ai_task_id = uuid::Uuid::new_v4();
        let user_msg = Message::UserInput {
            base: BaseMessage {
                role: MessageRole::User,
                content: "fitelnetのVLAN設定を教えて".to_string(),
                timestamp: Some(chrono::Utc::now()),
                is_tool_loading: None,
                is_hidden: None,
                task_id: Some(user_task_id),
            },
            status: None,
            attachments: None,
        };
        let ai_msg = Message::AgentResponse {
            base: BaseMessage {
                role: MessageRole::Ai,
                content: "VLAN設定の手順です".to_string(),
                timestamp: Some(chrono::Utc::now()),
                is_tool_loading: Some(false),
                is_hidden: Some(false),
                task_id: Some(ai_task_id),
            },
            summary_text: Some("解析完了".to_string()),
        };

        let session = ChatSession {
            id: session_id,
            title: "Fitelnet VLAN設定".to_string(),
            messages: vec![user_msg.clone(), ai_msg.clone()],
            recent_ips: Some(vec!["192.168.1.1".to_string()]),
        };
        let history = vec![HistoryItem::Session(session)];

        save_history_to_store(&state, &history).await.unwrap();

        // 3. Load back from SurrealDB and verify contents
        let loaded = load_history_from_store(&state).await.unwrap();
        assert_eq!(loaded.len(), 1);
        if let HistoryItem::Session(s) = &loaded[0] {
            assert_eq!(s.id, session_id);
            assert_eq!(s.title, "Fitelnet VLAN設定");
            assert_eq!(s.recent_ips, Some(vec!["192.168.1.1".to_string()]));
            assert_eq!(s.messages.len(), 2);
            assert_eq!(s.messages[0], user_msg);
            assert_eq!(s.messages[1], ai_msg);
        } else {
            panic!("Expected session");
        }

        // 4. Test mutation serialization via JSON (matching frontend Tauri invoke)
        let rename_json = format!(r#"{{
            "type": "renameSession",
            "sessionId": "{session_id}",
            "title": "更新されたタイトル"
        }}"#);
        let mutation: HistoryMutation = serde_json::from_str(&rename_json).unwrap();
        let mut mutated_history = loaded;
        let mutated = mutate_items(&mut mutated_history, &mutation);
        assert!(mutated);
        save_history_to_store(&state, &mutated_history).await.unwrap();

        // 5. Simulate reopening SurrealDB (app restart)
        drop(state);
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        let reopened_state = SurrealDbState::initialize_at(&temp_dir).await.unwrap();
        let reloaded = load_history_from_store(&reopened_state).await.unwrap();
        assert_eq!(reloaded.len(), 1);
        if let HistoryItem::Session(s) = &reloaded[0] {
            assert_eq!(s.title, "更新されたタイトル");
            assert_eq!(s.messages.len(), 2);
        } else {
            panic!("Expected session");
        }

        drop(reopened_state);
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_invalid_task_id_fails_deserialization() {
        let json_data = r#"{
            "role": "user",
            "content": "test",
            "event_type": "UserInput",
            "task_id": "not-a-valid-uuid"
        }"#;
        let result: Result<Message, _> = serde_json::from_str(json_data);
        assert!(result.is_err(), "Expected error when task_id is not a valid UUID, but got: {:?}", result);
    }
}

fn get_summaries_path(app: &tauri::AppHandle) -> PathBuf {
    let path = app
        .path()
        .app_data_dir()
        .expect("Failed to get app data dir");
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path.join("summaries.json")
}

#[tauri::command]
pub fn load_summaries(app: tauri::AppHandle) -> Result<Vec<SummaryItem>, TauriError> {
    let path = get_summaries_path(&app);
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = fs::read_to_string(path)?;
    let summaries: Vec<SummaryItem> = serde_json::from_str(&data)?;
    Ok(summaries)
}

#[tauri::command]
pub fn save_summary(app: tauri::AppHandle, summary: SummaryItem) -> Result<(), TauriError> {
    let mut summaries = load_summaries(app.clone()).unwrap_or_default();
    summaries.push(summary);

    // Keep only the last 100 summaries to prevent the file from growing indefinitely
    if summaries.len() > 100 {
        let skip = summaries.len() - 100;
        summaries = summaries.into_iter().skip(skip).collect();
    }

    let path = get_summaries_path(&app);
    let data = serde_json::to_string_pretty(&summaries)?;
    fs::write(path, data)?;
    Ok(())
}
