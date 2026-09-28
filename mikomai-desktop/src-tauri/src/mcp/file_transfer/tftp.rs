use serde::{Deserialize, Serialize};
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::time::Instant;

use super::FileTransferResult;
use crate::connections::{IpAddress, Port};
use crate::snapshot::SnapshotManager;

const TFTP_DEFAULT_PORT: u16 = 69;
const TFTP_BLOCK_SIZE: usize = 512;
const TFTP_DEFAULT_TIMEOUT_SECS: u64 = 3;
const TFTP_MAX_RETRIES: usize = 5;

// TFTP OpCodes (RFC 1350)
const OP_RRQ: u16 = 1;
const OP_WRQ: u16 = 2;
const OP_DATA: u16 = 3;
const OP_ACK: u16 = 4;
const OP_ERROR: u16 = 5;

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TftpDownloadParams {
    pub host: Option<String>,
    pub device: Option<String>,
    pub device_name: Option<String>,
    pub ip: Option<IpAddress>,
    pub port: Option<Port>,
    pub remote_file: Option<String>,
    pub filename: Option<String>,
    pub local_path: Option<String>,
    pub mode: Option<String>,
    pub timeout_secs: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct TftpUploadParams {
    pub host: Option<String>,
    pub device: Option<String>,
    pub device_name: Option<String>,
    pub ip: Option<IpAddress>,
    pub port: Option<Port>,
    pub local_file: Option<String>,
    pub remote_file: Option<String>,
    pub filename: Option<String>,
    pub content: Option<String>,
    pub mode: Option<String>,
    pub timeout_secs: Option<u64>,
}

/// Builds an RRQ or WRQ packet.
fn build_rq_packet(op: u16, filename: &str, mode: &str) -> Vec<u8> {
    let mut packet = Vec::with_capacity(2 + filename.len() + 1 + mode.len() + 1);
    packet.extend_from_slice(&op.to_be_bytes());
    packet.extend_from_slice(filename.as_bytes());
    packet.push(0);
    packet.extend_from_slice(mode.as_bytes());
    packet.push(0);
    packet
}

/// Builds an ACK packet.
fn build_ack_packet(block: u16) -> Vec<u8> {
    let mut packet = Vec::with_capacity(4);
    packet.extend_from_slice(&OP_ACK.to_be_bytes());
    packet.extend_from_slice(&block.to_be_bytes());
    packet
}

/// Builds a DATA packet.
fn build_data_packet(block: u16, data: &[u8]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(4 + data.len());
    packet.extend_from_slice(&OP_DATA.to_be_bytes());
    packet.extend_from_slice(&block.to_be_bytes());
    packet.extend_from_slice(data);
    packet
}

/// Parses an error packet and extracts the error message.
fn parse_error_packet(data: &[u8]) -> String {
    if data.len() >= 4 {
        let err_code = u16::from_be_bytes([data[2], data[3]]);
        let msg = if data.len() > 4 {
            let msg_bytes = &data[4..];
            let end = msg_bytes
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(msg_bytes.len());
            String::from_utf8_lossy(&msg_bytes[..end]).to_string()
        } else {
            String::new()
        };
        format!("TFTP Error {}: {}", err_code, msg)
    } else {
        "Malformed TFTP Error packet".to_string()
    }
}

/// Executes a TFTP download (RRQ) from `server_addr` for `remote_file`.
pub async fn tftp_download_core(
    server_addr: SocketAddr,
    remote_file: &str,
    mode: &str,
    timeout_duration: Duration,
) -> Result<(Vec<u8>, u64), String> {
    let bind_addr = if server_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };

    let socket = UdpSocket::bind(bind_addr)
        .await
        .map_err(|e| format!("Failed to bind local UDP socket: {}", e))?;

    let rrq = build_rq_packet(OP_RRQ, remote_file, mode);
    socket
        .send_to(&rrq, server_addr)
        .await
        .map_err(|e| format!("Failed to send RRQ to {}: {}", server_addr, e))?;

    let start_time = Instant::now();
    let mut file_data = Vec::new();
    let mut expected_block: u16 = 1;
    let mut transfer_tid: Option<SocketAddr> = None;
    let mut buf = [0u8; 1024];

    loop {
        let mut retries = 0;
        let (len, from_addr) = loop {
            match tokio::time::timeout(timeout_duration, socket.recv_from(&mut buf)).await {
                Ok(Ok((len, addr))) => {
                    // Check TID (the server uses a dynamic port after initial RRQ)
                    if let Some(tid) = transfer_tid {
                        if addr != tid {
                            // Packet from unexpected source, ignore
                            continue;
                        }
                    } else {
                        transfer_tid = Some(addr);
                    }
                    break (len, addr);
                }
                Ok(Err(e)) => return Err(format!("Socket receive error: {}", e)),
                Err(_) => {
                    retries += 1;
                    if retries >= TFTP_MAX_RETRIES {
                        return Err(format!(
                            "TFTP transfer timed out after {} retries (waiting for block {})",
                            TFTP_MAX_RETRIES, expected_block
                        ));
                    }
                    // Re-send previous ACK (or RRQ if waiting for block 1)
                    if expected_block == 1 {
                        let _ = socket.send_to(&rrq, server_addr).await;
                    } else if let Some(tid) = transfer_tid {
                        let ack = build_ack_packet(expected_block.wrapping_sub(1));
                        let _ = socket.send_to(&ack, tid).await;
                    }
                }
            }
        };

        if len < 4 {
            return Err("Received packet too small".to_string());
        }

        let opcode = u16::from_be_bytes([buf[0], buf[1]]);
        match opcode {
            OP_DATA => {
                let block = u16::from_be_bytes([buf[2], buf[3]]);
                if block == expected_block {
                    let chunk = &buf[4..len];
                    file_data.extend_from_slice(chunk);

                    // Send ACK
                    let ack = build_ack_packet(block);
                    socket
                        .send_to(&ack, from_addr)
                        .await
                        .map_err(|e| format!("Failed to send ACK: {}", e))?;

                    expected_block = expected_block.wrapping_add(1);

                    // If DATA packet is less than 512 bytes, transfer is complete
                    if chunk.len() < TFTP_BLOCK_SIZE {
                        break;
                    }
                } else if block == expected_block.wrapping_sub(1) {
                    // Duplicate DATA packet, re-send ACK
                    let ack = build_ack_packet(block);
                    let _ = socket.send_to(&ack, from_addr).await;
                }
            }
            OP_ERROR => {
                let err_msg = parse_error_packet(&buf[..len]);
                return Err(err_msg);
            }
            other => {
                return Err(format!("Unexpected TFTP opcode: {}", other));
            }
        }
    }

    let elapsed = start_time.elapsed().as_millis() as u64;
    Ok((file_data, elapsed))
}

/// Executes a TFTP upload (WRQ) to `server_addr` for `remote_file` with `data`.
pub async fn tftp_upload_core(
    server_addr: SocketAddr,
    remote_file: &str,
    data: &[u8],
    mode: &str,
    timeout_duration: Duration,
) -> Result<u64, String> {
    let bind_addr = if server_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };

    let socket = UdpSocket::bind(bind_addr)
        .await
        .map_err(|e| format!("Failed to bind local UDP socket: {}", e))?;

    let wrq = build_rq_packet(OP_WRQ, remote_file, mode);
    socket
        .send_to(&wrq, server_addr)
        .await
        .map_err(|e| format!("Failed to send WRQ to {}: {}", server_addr, e))?;

    let start_time = Instant::now();
    let mut buf = [0u8; 1024];
    let mut retries = 0;
    let tid: SocketAddr = loop {
        match tokio::time::timeout(timeout_duration, socket.recv_from(&mut buf)).await {
            Ok(Ok((len, addr))) => {
                if len >= 4 {
                    let opcode = u16::from_be_bytes([buf[0], buf[1]]);
                    if opcode == OP_ACK {
                        let block = u16::from_be_bytes([buf[2], buf[3]]);
                        if block == 0 {
                            break addr;
                        }
                    } else if opcode == OP_ERROR {
                        return Err(parse_error_packet(&buf[..len]));
                    }
                }
            }
            Ok(Err(e)) => return Err(format!("Socket receive error: {}", e)),
            Err(_) => {
                retries += 1;
                if retries >= TFTP_MAX_RETRIES {
                    return Err(format!(
                        "TFTP upload WRQ timed out after {} retries waiting for ACK 0",
                        TFTP_MAX_RETRIES
                    ));
                }
                let _ = socket.send_to(&wrq, server_addr).await;
            }
        }
    };

    // 2. Send DATA blocks
    let chunks: Vec<&[u8]> = data.chunks(TFTP_BLOCK_SIZE).collect();

    let mut block_num: u16 = 1;
    let mut chunk_idx = 0;

    while chunk_idx < chunks.len()
        || (chunk_idx == chunks.len() && data.len() % TFTP_BLOCK_SIZE == 0)
    {
        let chunk_data = if chunk_idx < chunks.len() {
            chunks[chunk_idx]
        } else {
            &[]
        };

        let data_packet = build_data_packet(block_num, chunk_data);
        socket
            .send_to(&data_packet, tid)
            .await
            .map_err(|e| format!("Failed to send DATA block {}: {}", block_num, e))?;

        // Wait for ACK(block_num)
        let mut block_retries = 0;
        loop {
            match tokio::time::timeout(timeout_duration, socket.recv_from(&mut buf)).await {
                Ok(Ok((len, addr))) => {
                    if addr == tid && len >= 4 {
                        let opcode = u16::from_be_bytes([buf[0], buf[1]]);
                        if opcode == OP_ACK {
                            let ack_block = u16::from_be_bytes([buf[2], buf[3]]);
                            if ack_block == block_num {
                                break;
                            }
                        } else if opcode == OP_ERROR {
                            return Err(parse_error_packet(&buf[..len]));
                        }
                    }
                }
                Ok(Err(e)) => return Err(format!("Socket receive error: {}", e)),
                Err(_) => {
                    block_retries += 1;
                    if block_retries >= TFTP_MAX_RETRIES {
                        return Err(format!(
                            "TFTP upload timed out after {} retries waiting for ACK {}",
                            TFTP_MAX_RETRIES, block_num
                        ));
                    }
                    let _ = socket.send_to(&data_packet, tid).await;
                }
            }
        }

        block_num = block_num.wrapping_add(1);
        chunk_idx += 1;
    }

    let elapsed = start_time.elapsed().as_millis() as u64;
    Ok(elapsed)
}

/// Download a file via TFTP with parameter resolution and storage integration.
pub async fn network_tftp_download_with_params(
    app: tauri::AppHandle,
    params: TftpDownloadParams,
) -> Result<FileTransferResult, String> {
    let host_args = crate::mcp::args::HostArgs {
        host: params.host,
        device: params.device,
        device_name: params.device_name,
        ip: params.ip,
    };
    let (target_host, ip_addr) = crate::mcp::args::resolve_host_args(&app, &host_args).await?;

    let port = params.port.map(|p| *p).unwrap_or(TFTP_DEFAULT_PORT);
    let server_addr = SocketAddr::new(ip_addr, port);

    let remote_file = params.remote_file.or(params.filename).ok_or_else(|| {
        "Error: remote_file or filename is required for TFTP download".to_string()
    })?;

    let mode = params.mode.as_deref().unwrap_or("octet");
    let timeout_secs = params.timeout_secs.unwrap_or(TFTP_DEFAULT_TIMEOUT_SECS);
    let timeout_duration = Duration::from_secs(timeout_secs);

    let (file_data, duration_ms) =
        tftp_download_core(server_addr, &remote_file, mode, timeout_duration).await?;

    let saved_path = if let Some(local_path) = params.local_path {
        let path = PathBuf::from(&local_path);
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(&path, &file_data).map_err(|e| {
            format!(
                "Failed to write downloaded TFTP file to {}: {}",
                local_path, e
            )
        })?;
        path
    } else {
        let mut manager = SnapshotManager::new(&app)
            .map_err(|e| format!("Failed to initialize SnapshotManager: {}", e))?;
        let clean_filename = Path::new(&remote_file)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("tftp_download.bin");

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
        "TFTP download successful:\n- Host: {}\n- Port: {}\n- Remote file: {}\n- Mode: {}\n- Saved to: {}\n- Size: {} bytes\n- Time: {} ms",
        ip_addr,
        port,
        remote_file,
        mode,
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

/// Upload a file via TFTP.
pub async fn network_tftp_upload_with_params(
    app: tauri::AppHandle,
    params: TftpUploadParams,
) -> Result<FileTransferResult, String> {
    let host_args = crate::mcp::args::HostArgs {
        host: params.host,
        device: params.device,
        device_name: params.device_name,
        ip: params.ip,
    };
    let (_target_host, ip_addr) = crate::mcp::args::resolve_host_args(&app, &host_args).await?;

    let port = params.port.map(|p| *p).unwrap_or(TFTP_DEFAULT_PORT);
    let server_addr = SocketAddr::new(ip_addr, port);

    let (file_data, file_source_desc, default_remote_name) =
        if let Some(local_path) = &params.local_file {
            let path = PathBuf::from(local_path);
            let data = fs::read(&path)
                .map_err(|e| format!("Failed to read local file {}: {}", local_path, e))?;
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file.bin")
                .to_string();
            (data, local_path.clone(), name)
        } else if let Some(text_content) = params.content {
            (
                text_content.into_bytes(),
                "provided text content".to_string(),
                "upload.txt".to_string(),
            )
        } else {
            return Err("Error: local_file or content is required for TFTP upload".to_string());
        };

    let remote_file = params
        .remote_file
        .or(params.filename)
        .unwrap_or(default_remote_name);

    let mode = params.mode.as_deref().unwrap_or("octet");
    let timeout_secs = params.timeout_secs.unwrap_or(TFTP_DEFAULT_TIMEOUT_SECS);
    let timeout_duration = Duration::from_secs(timeout_secs);

    let bytes_len = file_data.len();
    let duration_ms = tftp_upload_core(
        server_addr,
        &remote_file,
        &file_data,
        mode,
        timeout_duration,
    )
    .await?;

    let output = format!(
        "TFTP upload successful:\n- Host: {}\n- Port: {}\n- Source: {}\n- Remote filename: {}\n- Size: {} bytes\n- Time: {} ms",
        server_addr.ip(),
        server_addr.port(),
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
pub async fn network_tftp_download(
    app: tauri::AppHandle,
    host: Option<String>,
    device: Option<String>,
    deviceName: Option<String>,
    device_name: Option<String>,
    ip: Option<IpAddress>,
    port: Option<Port>,
    remote_file: Option<String>,
    remoteFile: Option<String>,
    filename: Option<String>,
    local_path: Option<String>,
    localPath: Option<String>,
    mode: Option<String>,
    timeout_secs: Option<u64>,
    timeoutSecs: Option<u64>,
) -> Result<FileTransferResult, String> {
    network_tftp_download_with_params(
        app,
        TftpDownloadParams {
            host,
            device,
            device_name: deviceName.or(device_name),
            ip,
            port,
            remote_file: remoteFile.or(remote_file),
            filename,
            local_path: localPath.or(local_path),
            mode,
            timeout_secs: timeoutSecs.or(timeout_secs),
        },
    )
    .await
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn network_tftp_upload(
    app: tauri::AppHandle,
    host: Option<String>,
    device: Option<String>,
    deviceName: Option<String>,
    device_name: Option<String>,
    ip: Option<IpAddress>,
    port: Option<Port>,
    local_file: Option<String>,
    localFile: Option<String>,
    remote_file: Option<String>,
    remoteFile: Option<String>,
    filename: Option<String>,
    content: Option<String>,
    mode: Option<String>,
    timeout_secs: Option<u64>,
    timeoutSecs: Option<u64>,
) -> Result<FileTransferResult, String> {
    network_tftp_upload_with_params(
        app,
        TftpUploadParams {
            host,
            device,
            device_name: deviceName.or(device_name),
            ip,
            port,
            local_file: localFile.or(local_file),
            remote_file: remoteFile.or(remote_file),
            filename,
            content,
            mode,
            timeout_secs: timeoutSecs.or(timeout_secs),
        },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_rq_packet() {
        let rrq = build_rq_packet(OP_RRQ, "startup-config", "octet");
        assert_eq!(rrq[0..2], [0, 1]);
        assert_eq!(&rrq[2..16], b"startup-config");
        assert_eq!(rrq[16], 0);
        assert_eq!(&rrq[17..22], b"octet");
        assert_eq!(rrq[22], 0);
    }

    #[test]
    fn test_build_ack_packet() {
        let ack = build_ack_packet(42);
        assert_eq!(ack, vec![0, 4, 0, 42]);
    }

    #[test]
    fn test_build_data_packet() {
        let data = b"hello world";
        let packet = build_data_packet(1, data);
        assert_eq!(&packet[0..4], &[0, 3, 0, 1]);
        assert_eq!(&packet[4..], data);
    }

    #[test]
    fn test_parse_error_packet() {
        let mut err_pkt = vec![0, 5, 0, 1];
        err_pkt.extend_from_slice(b"File not found\0");
        let msg = parse_error_packet(&err_pkt);
        assert_eq!(msg, "TFTP Error 1: File not found");
    }

    #[tokio::test]
    #[ignore = "requires localhost UDP bind permission; run explicitly in an integration environment"]
    async fn test_local_tftp_mock_loopback() {
        // Spin up a mock TFTP server on UDP localhost
        let server_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let server_addr = server_socket.local_addr().unwrap();

        let server_handle = tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let (_len, client_addr) = server_socket.recv_from(&mut buf).await.unwrap();
            let op = u16::from_be_bytes([buf[0], buf[1]]);
            assert_eq!(op, OP_RRQ);

            // Send DATA block 1 (<512 bytes, so terminates)
            let test_payload = b"Sample TFTP Configuration File Data";
            let data_pkt = build_data_packet(1, test_payload);
            server_socket.send_to(&data_pkt, client_addr).await.unwrap();

            // Receive ACK 1
            let (ack_len, _) = server_socket.recv_from(&mut buf).await.unwrap();
            assert_eq!(ack_len, 4);
            assert_eq!(u16::from_be_bytes([buf[0], buf[1]]), OP_ACK);
            assert_eq!(u16::from_be_bytes([buf[2], buf[3]]), 1);
        });

        let (received_bytes, _) = tftp_download_core(
            server_addr,
            "test_config.cfg",
            "octet",
            Duration::from_secs(2),
        )
        .await
        .unwrap();

        assert_eq!(received_bytes, b"Sample TFTP Configuration File Data");
        server_handle.await.unwrap();
    }
}
