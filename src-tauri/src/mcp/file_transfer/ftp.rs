use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;
use suppaftp::tokio::AsyncFtpStream;
use tokio::time::Instant;

use super::FileTransferResult;
use crate::connections::{IpAddress, Port};
use crate::crypto::decrypt;
use crate::snapshot::SnapshotManager;

const FTP_DEFAULT_PORT: u16 = 21;
const FTP_DEFAULT_TIMEOUT_SECS: u64 = 15;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FtpDownloadParams
{
    pub host: Option<String>,
    pub device: Option<String>,
    pub device_name: Option<String>,
    pub ip: Option<IpAddress>,
    pub port: Option<Port>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub remote_file: Option<String>,
    pub filename: Option<String>,
    pub local_path: Option<String>,
    pub timeout_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct FtpUploadParams
{
    pub host: Option<String>,
    pub device: Option<String>,
    pub device_name: Option<String>,
    pub ip: Option<IpAddress>,
    pub port: Option<Port>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub local_file: Option<String>,
    pub remote_file: Option<String>,
    pub filename: Option<String>,
    pub content: Option<String>,
    pub timeout_secs: Option<u64>,
}

/// Helper to resolve credentials from connections if not explicitly provided.
fn resolve_credentials(
    app: &tauri::AppHandle,
    target_host: &str,
    param_user: Option<String>,
    param_pass: Option<String>,
) -> (String, String)
{
    if let (Some(u), Some(p)) = (param_user.clone(), param_pass.clone())
    {
        if !u.is_empty()
        {
            return (u, p);
        }
    }

    if let Ok(connections) = crate::connections::load_connections_raw(app)
    {
        if let Some(conn) = connections.iter().find(|c| c.matches_host_or_ip(target_host))
        {
            let user = param_user
                .filter(|u| !u.trim().is_empty())
                .or_else(|| conn.username.as_ref().map(|u| u.to_string()))
                .unwrap_or_else(|| "anonymous".to_string());

            let pass = param_pass.unwrap_or_else(|| {
                conn.password
                    .as_ref()
                    .and_then(|p| decrypt(app, p.as_str()).ok())
                    .unwrap_or_else(|| "anonymous@".to_string())
            });

            return (user, pass);
        }
    }

    let user = param_user
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "anonymous".to_string());
    let pass = param_pass.unwrap_or_else(|| "anonymous@".to_string());

    (user, pass)
}

/// Download a file via FTP.
pub async fn network_ftp_download_with_params(
    app: tauri::AppHandle,
    params: FtpDownloadParams,
) -> Result<FileTransferResult, String>
{
    let host_args = crate::mcp::args::HostArgs {
        host: params.host,
        device: params.device,
        device_name: params.device_name,
        ip: params.ip,
    };
    let (target_host, ip_addr) = crate::mcp::args::resolve_host_args(&app, &host_args).await?;

    let port = params.port.map(|p| *p).unwrap_or(FTP_DEFAULT_PORT);
    let (username, password) =
        resolve_credentials(&app, &target_host, params.username, params.password);

    let remote_file = params
        .remote_file
        .or(params.filename)
        .ok_or_else(|| "Error: remote_file or filename is required for FTP download".to_string())?;

    let timeout_secs = params.timeout_secs.unwrap_or(FTP_DEFAULT_TIMEOUT_SECS);
    let server_addr_str = format!("{}:{}", ip_addr, port);

    let start_time = Instant::now();

    let download_future = async {
        let mut ftp_stream = AsyncFtpStream::connect(&server_addr_str)
            .await
            .map_err(|e| format!("Failed to connect to FTP server {}: {}", server_addr_str, e))?;

        ftp_stream
            .login(&username, &password)
            .await
            .map_err(|e| format!("FTP login failed for user '{}': {}", username, e))?;

        use tokio::io::AsyncReadExt;

        let mut data_stream = ftp_stream
            .retr_as_stream(&remote_file)
            .await
            .map_err(|e| format!("FTP download failed for '{}': {}", remote_file, e))?;

        let mut file_data = Vec::new();
        data_stream.read_to_end(&mut file_data).await.map_err(|e| {
            format!(
                "Failed to read FTP data stream for '{}': {}",
                remote_file, e
            )
        })?;

        let _ = ftp_stream.finalize_retr_stream(data_stream).await;
        let _ = ftp_stream.quit().await;
        Ok::<Vec<u8>, String>(file_data)
    };

    let file_data = tokio::time::timeout(Duration::from_secs(timeout_secs), download_future)
        .await
        .map_err(|_| format!("FTP download timed out after {} seconds", timeout_secs))??;

    let duration_ms = start_time.elapsed().as_millis() as u64;

    let saved_path = if let Some(local_path) = params.local_path
    {
        let path = PathBuf::from(&local_path);
        if let Some(parent) = path.parent()
        {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&path, &file_data).map_err(|e| {
            format!(
                "Failed to write downloaded FTP file to {}: {}",
                local_path, e
            )
        })?;
        path
    }
    else
    {
        let mut manager = SnapshotManager::new(&app)
            .map_err(|e| format!("Failed to initialize SnapshotManager: {}", e))?;
        let clean_filename = Path::new(&remote_file)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("ftp_download.bin");

        let path = manager
            .save_artifact(
                &target_host,
                clean_filename,
                &String::from_utf8_lossy(&file_data),
            )
            .map_err(|e| format!("Failed to save artifact: {}", e))?;

        let _ = manager.update_current_link(path.parent().unwrap());
        path
    };

    let bytes_len = file_data.len();
    let output = format!(
        "FTP download successful:\n- Host: {}\n- Port: {}\n- User: {}\n- Remote file: {}\n- Saved to: {}\n- Size: {} bytes\n- Time: {} ms",
        ip_addr,
        port,
        username,
        remote_file,
        saved_path.display(),
        bytes_len,
        duration_ms
    );

    Ok(FileTransferResult {
        success: true,
        output,
        file_path: Some(saved_path),
        bytes_transferred: Some(bytes_len),
        duration_ms: Some(duration_ms),
    })
}

/// Upload a file via FTP.
pub async fn network_ftp_upload_with_params(
    app: tauri::AppHandle,
    params: FtpUploadParams,
) -> Result<FileTransferResult, String>
{
    let host_args = crate::mcp::args::HostArgs {
        host: params.host,
        device: params.device,
        device_name: params.device_name,
        ip: params.ip,
    };
    let (target_host, ip_addr) = crate::mcp::args::resolve_host_args(&app, &host_args).await?;

    let port = params.port.map(|p| *p).unwrap_or(FTP_DEFAULT_PORT);
    let (username, password) =
        resolve_credentials(&app, &target_host, params.username, params.password);

    let (file_data, file_source_desc, default_remote_name) = if let Some(local_path) =
        &params.local_file
    {
        let path = PathBuf::from(local_path);
        let data = fs::read(&path)
            .map_err(|e| format!("Failed to read local file {}: {}", local_path, e))?;
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file.bin")
            .to_string();
        (data, local_path.clone(), name)
    }
    else if let Some(text_content) = params.content
    {
        (
            text_content.into_bytes(),
            "provided text content".to_string(),
            "upload.txt".to_string(),
        )
    }
    else
    {
        return Err("Error: local_file or content is required for FTP upload".to_string());
    };

    let remote_file = params
        .remote_file
        .or(params.filename)
        .unwrap_or(default_remote_name);

    let timeout_secs = params.timeout_secs.unwrap_or(FTP_DEFAULT_TIMEOUT_SECS);
    let server_addr_str = format!("{}:{}", ip_addr, port);

    let start_time = Instant::now();
    let bytes_len = file_data.len();

    let upload_future = async {
        let mut ftp_stream = AsyncFtpStream::connect(&server_addr_str)
            .await
            .map_err(|e| format!("Failed to connect to FTP server {}: {}", server_addr_str, e))?;

        ftp_stream
            .login(&username, &password)
            .await
            .map_err(|e| format!("FTP login failed for user '{}': {}", username, e))?;

        let mut reader = Cursor::new(file_data);
        ftp_stream
            .put_file(&remote_file, &mut reader)
            .await
            .map_err(|e| format!("FTP upload failed for '{}': {}", remote_file, e))?;

        let _ = ftp_stream.quit().await;
        Ok::<(), String>(())
    };

    tokio::time::timeout(Duration::from_secs(timeout_secs), upload_future)
        .await
        .map_err(|_| format!("FTP upload timed out after {} seconds", timeout_secs))??;

    let duration_ms = start_time.elapsed().as_millis() as u64;

    let output = format!(
        "FTP upload successful:\n- Host: {}\n- Port: {}\n- User: {}\n- Source: {}\n- Remote filename: {}\n- Size: {} bytes\n- Time: {} ms",
        ip_addr,
        port,
        username,
        file_source_desc,
        remote_file,
        bytes_len,
        duration_ms
    );

    Ok(FileTransferResult {
        success: true,
        output,
        file_path: params.local_file.map(PathBuf::from),
        bytes_transferred: Some(bytes_len),
        duration_ms: Some(duration_ms),
    })
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn network_ftp_download(
    app: tauri::AppHandle,
    host: Option<String>,
    device: Option<String>,
    deviceName: Option<String>,
    device_name: Option<String>,
    ip: Option<IpAddress>,
    port: Option<Port>,
    username: Option<String>,
    password: Option<String>,
    remoteFile: Option<String>,
    remote_file: Option<String>,
    filename: Option<String>,
    localPath: Option<String>,
    local_path: Option<String>,
    timeoutSecs: Option<u64>,
    timeout_secs: Option<u64>,
) -> Result<FileTransferResult, String>
{
    network_ftp_download_with_params(
        app,
        FtpDownloadParams {
            host,
            device,
            device_name: deviceName.or(device_name),
            ip,
            port,
            username,
            password,
            remote_file: remoteFile.or(remote_file),
            filename,
            local_path: localPath.or(local_path),
            timeout_secs: timeoutSecs.or(timeout_secs),
        },
    )
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn network_ftp_upload(
    app: tauri::AppHandle,
    host: Option<String>,
    device: Option<String>,
    deviceName: Option<String>,
    device_name: Option<String>,
    ip: Option<IpAddress>,
    port: Option<Port>,
    username: Option<String>,
    password: Option<String>,
    localFile: Option<String>,
    local_file: Option<String>,
    remoteFile: Option<String>,
    remote_file: Option<String>,
    filename: Option<String>,
    content: Option<String>,
    timeoutSecs: Option<u64>,
    timeout_secs: Option<u64>,
) -> Result<FileTransferResult, String>
{
    network_ftp_upload_with_params(
        app,
        FtpUploadParams {
            host,
            device,
            device_name: deviceName.or(device_name),
            ip,
            port,
            username,
            password,
            local_file: localFile.or(local_file),
            remote_file: remoteFile.or(remote_file),
            filename,
            content,
            timeout_secs: timeoutSecs.or(timeout_secs),
        },
    )
    .await
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_ftp_result_serialization()
    {
        let res = FileTransferResult {
            success: true,
            output: "FTP OK".to_string(),
            file_path: Some(PathBuf::from("/tmp/test.bin")),
            bytes_transferred: Some(1024),
            duration_ms: Some(150),
        };
        let serialized = serde_json::to_string(&res).unwrap();
        assert!(serialized.contains("FTP OK"));
        assert!(serialized.contains("/tmp/test.bin"));
    }
}
