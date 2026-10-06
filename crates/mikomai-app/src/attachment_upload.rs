//! Opaque FTP/TFTP attachments bypass vision, RAG, and model planning.
use super::{debug_trace, portable_app_data_dir, MikomaiPlanCallback, SwiftPlanTransport};
use mikomai_core::port::OperationProposalPort;
use mikomai_core::OperationPlan;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;

pub(super) enum Outcome {
    Ask(String),
    Plan(OperationPlan, String),
}

pub(super) fn prepare(
    goal: &str,
    payload: &str,
    callback: Option<MikomaiPlanCallback>,
    context: usize,
) -> Result<Option<Outcome>, String> {
    let Some(protocol) = mikomai_core::attachment_transfer::upload_protocol(goal) else {
        return Ok(None);
    };
    let label = protocol.label();
    let port = protocol.port();
    debug_trace::emit(
        "attachment_route",
        serde_json::json!({"purpose":protocol.tool(), "vision":false, "planner":false}),
    );
    let Some(target) = mikomai_core::attachment_transfer::upload_target(goal) else {
        return Ok(Some(Outcome::Ask(
            format!("### ❓ 確認要求\n{label}の送信先を1つ指定してください。添付画像の解析は行っていません。"),
        )));
    };
    let (text, images) = mikomai_adapters::attachments::decode(payload)?;
    let mut files: Vec<(String, Vec<u8>)> = images
        .into_iter()
        .map(|image| (image.name, image.data))
        .collect();
    if !text.is_empty() {
        // Legacy UTF-8 attachment framing is unambiguous for a single file.
        if !text.starts_with("[添付ファイル ") || text.matches("[添付ファイル ").count() != 1
        {
            return Ok(Some(Outcome::Ask(
                "### ❓ 確認要求\n転送対象のファイルを1つだけ添付して再送してください。".into(),
            )));
        }
        let (header, body) = text
            .split_once('\n')
            .ok_or("添付ファイルの形式が不正です")?;
        let name = header
            .split_once(": ")
            .and_then(|(_, name)| name.strip_suffix(']'))
            .ok_or("添付ファイル名が不正です")?;
        files.push((name.into(), body.as_bytes().to_vec()));
    }
    let selected = if files.len() == 1 {
        Some(0)
    } else {
        let matches: Vec<_> = files
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| goal.contains(name.as_str()))
            .map(|(index, _)| index)
            .collect();
        (matches.len() == 1).then(|| matches[0])
    };
    let Some(index) = selected else {
        return Ok(Some(Outcome::Ask("### ❓ 確認要求\nアップロードする添付ファイルを1つ指定してください。画像の解析は行っていません。".into())));
    };
    let (name, bytes) = files.swap_remove(index);
    if name.is_empty()
        || name.len() > 1024
        || name.contains(['/', '\\'])
        || name.chars().any(char::is_control)
    {
        return Err("転送対象の添付ファイル名が不正です".into());
    }
    let dir = portable_app_data_dir()?.join("artifacts/uploads");
    std::fs::create_dir_all(&dir).map_err(|e| format!("転送用の添付保存に失敗しました: {e}"))?;
    let path = dir.join(format!("{}.upload", uuid::Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let staged = (|| -> Result<OperationPlan, String> {
        let mut file = options
            .open(&path)
            .map_err(|e| format!("添付保存に失敗しました: {e}"))?;
        file.write_all(&bytes)
            .map_err(|e| format!("添付保存に失敗しました: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("添付保存に失敗しました: {e}"))?;
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let args = serde_json::json!({"local_path":path, "remote_path":name, "port":port, "mode":"octet", "attachment_sha256":digest, "attachment_bytes":bytes.len()});
        debug_trace::emit(
            "attachment_transfer",
            serde_json::json!({"target":target, "filename":name, "bytes":bytes.len(), "sha256":digest, "vision":false}),
        );
        let transport = SwiftPlanTransport { callback, context };
        futures_lite::future::block_on(transport.propose(
            &target,
            protocol.tool(),
            &args,
            &format!("添付ファイルを元のバイト列のまま{label}サーバーへアップロード"),
        ))
    })();
    let plan = match staged {
        Ok(plan) => plan,
        Err(error) => {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
    };
    let answer = format!("### ✅ 承認待ち\n添付ファイル「{name}」（{} bytes）を {target}:{port} へ{label}でアップロードする計画を作成しました。画像の解析は行わず、元のファイルをそのまま送信します。承認後に転送を開始します。", bytes.len());
    Ok(Some(Outcome::Plan(plan, answer)))
}

/// Read once and pin verified bytes in the worker, closing the approval/I/O gap.
pub(super) fn read_verified(path: &str, expected: &str) -> Result<Vec<u8>, String> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return Err("添付の転送パスが不正です".into());
    }
    let file = std::fs::File::open(path).map_err(|e| format!("転送対象を読み込めません: {e}"))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("転送対象は通常ファイルにしてください".into());
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 || format!("{:x}", Sha256::digest(&bytes)) != expected {
        return Err(
            "承認計画の作成後に添付ファイルが変わりました。計画を作り直してください。".into(),
        );
    }
    Ok(bytes)
}
