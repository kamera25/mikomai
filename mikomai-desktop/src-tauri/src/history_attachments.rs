use super::history_types::{
    Attachment, AttachmentPreparation, AttachmentRejection, AttachmentSource, AttachmentType,
};
use crate::error::TauriError;
use base64::{engine::general_purpose, Engine as _};
use std::fs;
use std::path::PathBuf;

pub const MAX_TEXT_ATTACHMENT_SIZE: u64 =
    mikomai_core::attachment_policy::MAX_TEXT_ATTACHMENT_BYTES;
pub const MAX_INLINE_ATTACHMENT_SIZE: usize =
    mikomai_core::attachment_policy::MAX_INLINE_ATTACHMENT_BYTES;
pub const MAX_IMAGE_ATTACHMENT_SIZE: u64 =
    mikomai_core::attachment_policy::MAX_IMAGE_ATTACHMENT_BYTES;

fn attachment_type(kind: mikomai_core::attachment_policy::AttachmentKind) -> AttachmentType {
    match kind {
        mikomai_core::attachment_policy::AttachmentKind::Text => AttachmentType::Text,
        mikomai_core::attachment_policy::AttachmentKind::Image => AttachmentType::Image,
        mikomai_core::attachment_policy::AttachmentKind::File => AttachmentType::File,
    }
}

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

    let file_size = fs::metadata(&path)
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let text = if !mikomai_core::attachment_policy::is_image_extension(&extension)
        && !extension.eq_ignore_ascii_case("pdf")
        && file_size <= MAX_TEXT_ATTACHMENT_SIZE
    {
        fs::read_to_string(&path).ok()
    } else {
        None
    };
    let kind =
        mikomai_core::attachment_policy::classify_path(&file_name, file_size, text.is_some())
            .map_err(|error| AttachmentRejection {
                name: file_name.clone(),
                reason: error.to_string(),
            })?;

    match kind {
        mikomai_core::attachment_policy::AttachmentKind::Image => {
            let bytes = fs::read(&path).map_err(|_| AttachmentRejection {
                name: file_name.clone(),
                reason: "画像ファイルを読み込めません".to_string(),
            })?;
            let mime =
                mikomai_core::attachment_policy::image_mime_type(&extension).unwrap_or("image/png");
            let b64 = general_purpose::STANDARD.encode(&bytes);
            Ok(Attachment {
                name: file_name,
                mime_type: AttachmentType::Image,
                content: format!("data:{mime};base64,{b64}"),
                path: Some(PathBuf::from(path_str)),
            })
        }
        mikomai_core::attachment_policy::AttachmentKind::Text => Ok(Attachment {
            name: file_name,
            mime_type: AttachmentType::Text,
            content: text.unwrap_or_default(),
            path: Some(PathBuf::from(path_str)),
        }),
        mikomai_core::attachment_policy::AttachmentKind::File => {
            let size_desc = if file_size < 1024 {
                format!("{} B", file_size)
            } else if file_size < 1024 * 1024 {
                format!("{:.1} KB", file_size as f64 / 1024.0)
            } else {
                format!("{:.1} MB", file_size as f64 / (1024.0 * 1024.0))
            };
            Ok(Attachment {
                name: file_name.clone(),
                mime_type: AttachmentType::File,
                content: format!("[ファイル: {} (サイズ: {})]", file_name, size_desc),
                path: Some(PathBuf::from(path_str)),
            })
        }
    }
}

pub fn attachment_from_inline(
    name: String,
    content: String,
    media_type: Option<String>,
) -> Result<Attachment, AttachmentRejection> {
    let kind =
        mikomai_core::attachment_policy::classify_inline(&name, &content, media_type.as_deref())
            .map_err(|error| AttachmentRejection {
                name: name.clone(),
                reason: error.to_string(),
            })?;
    let content = if kind == mikomai_core::attachment_policy::AttachmentKind::File {
        let size = content.len();
        let size_desc = if size < 1024 {
            format!("{} B", size)
        } else {
            format!("{:.1} KB", size as f64 / 1024.0)
        };
        format!("[ファイル: {} (PDF未抽出, サイズ: {})]", name, size_desc)
    } else {
        content
    };
    Ok(Attachment {
        name,
        mime_type: attachment_type(kind),
        content,
        path: None,
    })
}

pub fn prepare_attachments_for_sources(
    sources: Vec<AttachmentSource>,
    vision_ready: bool,
) -> AttachmentPreparation {
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
            Ok(attachment)
                if mikomai_core::attachment_policy::validate_vision(
                    match attachment.mime_type {
                        AttachmentType::Text => {
                            mikomai_core::attachment_policy::AttachmentKind::Text
                        }
                        AttachmentType::Image => {
                            mikomai_core::attachment_policy::AttachmentKind::Image
                        }
                        AttachmentType::File => {
                            mikomai_core::attachment_policy::AttachmentKind::File
                        }
                    },
                    vision_ready,
                )
                .is_err() =>
            {
                rejected.push(AttachmentRejection {
                    name: attachment.name,
                    reason:
                        mikomai_core::attachment_policy::AttachmentPolicyError::VisionUnavailable
                            .to_string(),
                });
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
pub fn prepare_attachments(
    app: tauri::AppHandle,
    sources: Vec<AttachmentSource>,
) -> AttachmentPreparation {
    let settings = crate::settings::load_settings(app).unwrap_or_default();
    let vision_ready = settings.vision_enabled
        && settings
            .mmproj_path
            .as_deref()
            .is_some_and(|path| !path.trim().is_empty());
    prepare_attachments_for_sources(sources, vision_ready)
}

#[tauri::command]
pub fn read_files_as_attachments(
    app: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<Vec<Attachment>, TauriError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "mikomai-attachment-{}-{name}",
            uuid::Uuid::new_v4()
        ))
    }

    #[test]
    fn image_file_limit_accepts_exactly_ten_mib_and_rejects_one_byte_over() {
        let at_limit = temp_file("at-limit.png");
        let over_limit = temp_file("over-limit.png");
        std::fs::File::create(&at_limit)
            .unwrap()
            .set_len(MAX_IMAGE_ATTACHMENT_SIZE)
            .unwrap();
        std::fs::File::create(&over_limit)
            .unwrap()
            .set_len(MAX_IMAGE_ATTACHMENT_SIZE + 1)
            .unwrap();

        let accepted = attachment_from_path(at_limit.to_string_lossy().into_owned()).unwrap();
        assert_eq!(accepted.mime_type, AttachmentType::Image);
        let rejected = attachment_from_path(over_limit.to_string_lossy().into_owned()).unwrap_err();
        assert!(rejected.reason.contains("10 MB"));

        let _ = std::fs::remove_file(at_limit);
        let _ = std::fs::remove_file(over_limit);
    }

    #[test]
    fn pdf_is_retained_as_an_unextracted_file_not_raw_utf8_text() {
        let path = temp_file("notes.pdf");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\n%%EOF")
            .unwrap();

        let attachment = attachment_from_path(path.to_string_lossy().into_owned()).unwrap();

        assert_eq!(attachment.name, path.file_name().unwrap().to_string_lossy());
        assert_eq!(attachment.mime_type, AttachmentType::File);
        assert!(attachment.content.starts_with("[ファイル:"));
        assert!(!attachment.content.contains("/Catalog"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn inline_pdf_is_retained_as_a_file_placeholder_not_raw_payload() {
        let attachment = attachment_from_inline(
            "notes.pdf".to_string(),
            "%PDF-1.7\nraw document bytes".to_string(),
            Some("application/pdf".to_string()),
        )
        .unwrap();

        assert_eq!(attachment.mime_type, AttachmentType::File);
        assert!(attachment.content.starts_with("[ファイル:"));
        assert!(!attachment.content.contains("%PDF"));
    }

    #[test]
    fn vision_unready_images_are_rejected_and_same_name_ready_images_deduplicate() {
        let image = || AttachmentSource::Inline {
            name: "diagram.png".to_string(),
            content: "data:image/png;base64,AA==".to_string(),
            media_type: Some("image/png".to_string()),
        };
        let rejected = prepare_attachments_for_sources(vec![image()], false);
        assert!(rejected.attachments.is_empty());
        assert_eq!(rejected.rejected.len(), 1);
        assert!(rejected.rejected[0].reason.contains("Vision"));

        let prepared = prepare_attachments_for_sources(vec![image(), image()], true);
        assert_eq!(prepared.attachments.len(), 1);
        assert_eq!(prepared.rejected.len(), 1);
        assert!(prepared.rejected[0].reason.contains("同名"));
    }
}
