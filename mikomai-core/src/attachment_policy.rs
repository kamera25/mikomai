use std::path::Path;

pub const MAX_TEXT_ATTACHMENT_BYTES: u64 = 512 * 1024;
pub const MAX_INLINE_ATTACHMENT_BYTES: usize = 512 * 1024;
pub const MAX_IMAGE_ATTACHMENT_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_NATIVE_TEXT_ATTACHMENT_BYTES: usize = 128 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Text,
    Image,
    File,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentPolicyError {
    NameRequired,
    InlineTooLarge,
    ImageTooLarge,
    VisionUnavailable,
    NativePayloadTooLarge,
    NativeImageUnsupported,
    NativePDFUnsupported,
    NativeFileUnsupported,
}

impl AttachmentPolicyError {
    pub fn message(self) -> &'static str {
        match self {
            Self::NameRequired => "ファイル名が必要です",
            Self::InlineTooLarge => "添付内容が大きすぎます (最大 512 KB)",
            Self::ImageTooLarge => "画像ファイルが大きすぎます (最大 10 MB)",
            Self::VisionUnavailable => "画像添付には Vision モデルの設定が必要です",
            Self::NativePayloadTooLarge => "添付ファイルの合計サイズが128 KiBを超えています。",
            Self::NativeImageUnsupported => "画像添付は未対応です。画像推論は実行できません。",
            Self::NativePDFUnsupported => "PDFからテキストを抽出する機能は未対応です。",
            Self::NativeFileUnsupported => "FFI添付はUTF-8テキスト形式のみ対応しています。",
        }
    }
}

impl std::fmt::Display for AttachmentPolicyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

pub fn is_image_extension(extension: &str) -> bool {
    matches!(
        extension
            .trim_start_matches('.')
            .to_ascii_lowercase()
            .as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg"
    )
}

pub fn image_mime_type(extension: &str) -> Option<&'static str> {
    match extension
        .trim_start_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

pub fn classify_path(
    file_name: &str,
    byte_count: u64,
    is_utf8_text: bool,
) -> Result<AttachmentKind, AttachmentPolicyError> {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if is_image_extension(extension) {
        return if byte_count > MAX_IMAGE_ATTACHMENT_BYTES {
            Err(AttachmentPolicyError::ImageTooLarge)
        } else {
            Ok(AttachmentKind::Image)
        };
    }
    if extension.eq_ignore_ascii_case("pdf") {
        return Ok(AttachmentKind::File);
    }
    if byte_count <= MAX_TEXT_ATTACHMENT_BYTES && is_utf8_text {
        Ok(AttachmentKind::Text)
    } else {
        Ok(AttachmentKind::File)
    }
}

pub fn classify_inline(
    name: &str,
    content: &str,
    media_type: Option<&str>,
) -> Result<AttachmentKind, AttachmentPolicyError> {
    if name.trim().is_empty() {
        return Err(AttachmentPolicyError::NameRequired);
    }
    if content.len() > MAX_INLINE_ATTACHMENT_BYTES {
        return Err(AttachmentPolicyError::InlineTooLarge);
    }
    let extension = Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if extension.eq_ignore_ascii_case("pdf")
        || media_type.is_some_and(|value| value.eq_ignore_ascii_case("application/pdf"))
        || content
            .get(.."data:application/pdf".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:application/pdf"))
    {
        return Ok(AttachmentKind::File);
    }
    let is_image = media_type.is_some_and(|value| value.starts_with("image/"))
        || content.starts_with("data:image/");
    Ok(if is_image {
        AttachmentKind::Image
    } else {
        AttachmentKind::Text
    })
}

pub fn validate_vision(
    kind: AttachmentKind,
    vision_ready: bool,
) -> Result<(), AttachmentPolicyError> {
    if kind == AttachmentKind::Image && !vision_ready {
        Err(AttachmentPolicyError::VisionUnavailable)
    } else {
        Ok(())
    }
}

/// The current C ABI accepts a flattened text payload, not typed media. Reject
/// image/PDF and non-text file markers before the prompt reaches the model.
pub fn validate_native_text_payload(payload: &str) -> Result<(), AttachmentPolicyError> {
    let mut body_bytes = 0usize;
    let mut attachment_count = 0usize;
    for line in payload.split_inclusive('\n') {
        let marker_line = line.strip_suffix('\n').unwrap_or(line);
        let marker_line = marker_line.strip_suffix('\r').unwrap_or(marker_line);
        if let Some(name) = native_attachment_name(marker_line) {
            attachment_count += 1;
            let extension = Path::new(name)
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if is_image_extension(extension) {
                return Err(AttachmentPolicyError::NativeImageUnsupported);
            }
            if extension.eq_ignore_ascii_case("pdf") {
                return Err(AttachmentPolicyError::NativePDFUnsupported);
            }
            if !matches!(
                extension.to_ascii_lowercase().as_str(),
                "txt" | "md" | "csv" | "json" | "yaml" | "yml" | "xml" | "log"
            ) {
                return Err(AttachmentPolicyError::NativeFileUnsupported);
            }
        } else {
            body_bytes += line.len();
        }
    }
    // Native wrappers join attachment bodies with exactly two LF bytes.
    if attachment_count > 1 {
        body_bytes = body_bytes.saturating_sub((attachment_count - 1) * 2);
    }
    if body_bytes > MAX_NATIVE_TEXT_ATTACHMENT_BYTES {
        return Err(AttachmentPolicyError::NativePayloadTooLarge);
    }
    Ok(())
}

fn native_attachment_name(line: &str) -> Option<&str> {
    let header = line.strip_prefix("[添付ファイル ")?;
    let (index, label) = header.split_once(": ")?;
    if index.is_empty() || !index.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let name = label.strip_suffix(']')?;
    (!name.trim().is_empty()).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_file_vectors_cover_images_text_and_unextracted_pdf() {
        assert_eq!(
            classify_path("DIAGRAM.PNG", 10, false),
            Ok(AttachmentKind::Image)
        );
        assert_eq!(
            classify_path("notes.txt", 12, true),
            Ok(AttachmentKind::Text)
        );
        assert_eq!(
            classify_path("notes.pdf", 48, true),
            Ok(AttachmentKind::File)
        );
        assert_eq!(
            classify_path("large.png", MAX_IMAGE_ATTACHMENT_BYTES + 1, false),
            Err(AttachmentPolicyError::ImageTooLarge)
        );
        assert_eq!(
            classify_path("limit.png", MAX_IMAGE_ATTACHMENT_BYTES, false),
            Ok(AttachmentKind::Image)
        );
    }

    #[test]
    fn shared_inline_vectors_enforce_vision_and_duplicate_free_classification() {
        let image = classify_inline(
            "diagram.png",
            "data:image/png;base64,AA==",
            Some("image/png"),
        )
        .unwrap();
        assert_eq!(image, AttachmentKind::Image);
        assert_eq!(
            validate_vision(image, false),
            Err(AttachmentPolicyError::VisionUnavailable)
        );
        assert_eq!(validate_vision(image, true), Ok(()));
        assert_eq!(
            classify_inline("note.txt", "router config", Some("text/plain")),
            Ok(AttachmentKind::Text)
        );
        assert_eq!(
            classify_inline("notes.pdf", "%PDF-1.7", Some("application/pdf")),
            Ok(AttachmentKind::File)
        );
    }

    #[test]
    fn text_only_ffi_vectors_reject_image_pdf_and_unsupported_files() {
        assert_eq!(
            validate_native_text_payload("[添付ファイル 1: diagram.png]\ncontent"),
            Err(AttachmentPolicyError::NativeImageUnsupported)
        );
        assert_eq!(
            validate_native_text_payload("[添付ファイル 1: notes.pdf]\ncontent"),
            Err(AttachmentPolicyError::NativePDFUnsupported)
        );
        assert_eq!(
            validate_native_text_payload("[添付ファイル 1: notes.md]\nrouter config"),
            Ok(())
        );
    }

    #[test]
    fn native_text_limit_counts_content_not_wrapper_bytes() {
        let payload = format!(
            "[添付ファイル 1: notes.md]\n{}",
            "x".repeat(MAX_NATIVE_TEXT_ATTACHMENT_BYTES)
        );
        assert_eq!(validate_native_text_payload(&payload), Ok(()));

        let too_large = format!(
            "[添付ファイル 1: notes.md]\n{}",
            "x".repeat(MAX_NATIVE_TEXT_ATTACHMENT_BYTES + 1)
        );
        assert_eq!(
            validate_native_text_payload(&too_large),
            Err(AttachmentPolicyError::NativePayloadTooLarge)
        );

        let two_files = format!(
            "[添付ファイル 1: first.md]\n{}\n\n[添付ファイル 2: second.md]\n{}",
            "x".repeat(MAX_NATIVE_TEXT_ATTACHMENT_BYTES / 2),
            "y".repeat(MAX_NATIVE_TEXT_ATTACHMENT_BYTES / 2)
        );
        assert_eq!(validate_native_text_payload(&two_files), Ok(()));
    }

    #[test]
    fn ordinary_text_mentions_of_media_mime_are_not_typed_attachments() {
        assert_eq!(
            validate_native_text_payload(
                "画像形式は data:image/png です。PDF形式は data:application/pdf です。"
            ),
            Ok(())
        );
    }
}
