use std::fs;
use std::path::PathBuf;
use crate::error::TauriError;
use base64::{engine::general_purpose, Engine as _};
use super::history_types::{Attachment, AttachmentPreparation, AttachmentRejection, AttachmentSource, AttachmentType};

pub const MAX_TEXT_ATTACHMENT_SIZE: u64 = 512 * 1024;
pub const MAX_INLINE_ATTACHMENT_SIZE: usize = 512 * 1024;
pub const MAX_IMAGE_ATTACHMENT_SIZE: u64 = 10 * 1024 * 1024;

pub fn attachment_from_path(path_str: String) -> Result<Attachment, AttachmentRejection> {
    let path = std::path::Path::new(&path_str);
    if !path.exists() || !path.is_file() {
        return Err(AttachmentRejection {
            name: path_str,
            reason: "ファイルが見つかりません".to_string(),
        });
    }

    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let is_image = matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg"
    );

    if is_image {
        let file_size = fs::metadata(&path).map(|metadata| metadata.len()).unwrap_or(0);
        if file_size > MAX_IMAGE_ATTACHMENT_SIZE {
            return Err(AttachmentRejection {
                name: file_name,
                reason: "画像ファイルが大きすぎます (最大 10 MB)".to_string(),
            });
        }
        if let Ok(bytes) = fs::read(&path) {
            let mime = match extension.as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "bmp" => "image/bmp",
                "svg" => "image/svg+xml",
                _ => "image/png",
            };
            let b64 = general_purpose::STANDARD.encode(&bytes);
            let data_url = format!("data:{};base64,{}", mime, b64);
            return Ok(Attachment {
                name: file_name,
                mime_type: AttachmentType::Image,
                content: data_url,
                path: Some(PathBuf::from(path_str)),
            });
        }
        return Err(AttachmentRejection {
            name: file_name,
            reason: "画像ファイルを読み込めません".to_string(),
        });
    } else {
        let file_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        if file_size <= MAX_TEXT_ATTACHMENT_SIZE {
            if let Ok(text) = fs::read_to_string(&path) {
                return Ok(Attachment {
                    name: file_name,
                    mime_type: AttachmentType::Text,
                    content: text,
                    path: Some(PathBuf::from(path_str)),
                });
            }
        }

        let size_desc = if file_size < 1024 {
            format!("{} B", file_size)
        } else if file_size < 1024 * 1024 {
            format!("{:.1} KB", file_size as f64 / 1024.0)
        } else {
            format!("{:.1} MB", file_size as f64 / (1024.0 * 1024.0))
        };

        return Ok(Attachment {
            name: file_name.clone(),
            mime_type: AttachmentType::File,
            content: format!("[ファイル: {} (サイズ: {})]", file_name, size_desc),
            path: Some(PathBuf::from(path_str)),
        });
    }
}

pub fn attachment_from_inline(
    name: String,
    content: String,
    media_type: Option<String>,
) -> Result<Attachment, AttachmentRejection> {
    if name.trim().is_empty() {
        return Err(AttachmentRejection {
            name,
            reason: "ファイル名が必要です".to_string(),
        });
    }
    if content.len() > MAX_INLINE_ATTACHMENT_SIZE {
        return Err(AttachmentRejection {
            name,
            reason: "添付内容が大きすぎます (最大 512 KB)".to_string(),
        });
    }
    let is_image = media_type
        .as_deref()
        .is_some_and(|value| value.starts_with("image/"))
        || content.starts_with("data:image/");
    Ok(Attachment {
        name,
        mime_type: if is_image {
            AttachmentType::Image
        } else {
            AttachmentType::Text
        },
        content,
        path: None,
    })
}

pub fn prepare_attachments_for_sources(sources: Vec<AttachmentSource>, vision_ready: bool) -> AttachmentPreparation {
    let mut attachments = Vec::new();
    let mut rejected = Vec::new();
    let mut names = std::collections::HashSet::new();
    for source in sources {
        let result = match source {
            AttachmentSource::Path { path } => attachment_from_path(path),
            AttachmentSource::Inline {
                name,
                content,
                media_type,
            } => attachment_from_inline(name, content, media_type),
        };
        match result {
            Ok(attachment) if attachment.mime_type == AttachmentType::Image && !vision_ready => {
                rejected.push(AttachmentRejection { name: attachment.name, reason: "画像添付には Vision モデルの設定が必要です".to_string() });
            }
            Ok(attachment) if names.insert(attachment.name.clone()) => attachments.push(attachment),
            Ok(attachment) => rejected.push(AttachmentRejection {
                name: attachment.name,
                reason: "同名の添付が既にあります".to_string(),
            }),
            Err(rejection) => rejected.push(rejection),
        }
    }
    AttachmentPreparation {
        attachments,
        rejected,
    }
}

#[tauri::command]
pub fn prepare_attachments(app: tauri::AppHandle, sources: Vec<AttachmentSource>) -> AttachmentPreparation {
    let settings = crate::settings::load_settings(app).unwrap_or_default();
    let vision_ready = settings.vision_enabled
        && settings.mmproj_path.as_deref().is_some_and(|path| !path.trim().is_empty());
    prepare_attachments_for_sources(sources, vision_ready)
}

#[tauri::command]
pub fn read_files_as_attachments(app: tauri::AppHandle, paths: Vec<String>) -> Result<Vec<Attachment>, TauriError> {
    let settings = crate::settings::load_settings(app).unwrap_or_default();
    Ok(prepare_attachments_for_sources(
        paths
            .into_iter()
            .map(|path| AttachmentSource::Path { path })
            .collect(),
        settings.vision_enabled
            && settings
                .mmproj_path
                .as_deref()
                .is_some_and(|path| !path.trim().is_empty()),
    )
    .attachments)
}
