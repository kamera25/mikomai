//! Concrete serial-console and FTP/TFTP transfer adapters.
//!
//! Callers must invoke these functions only after the approved operation plan
//! has been claimed. Credentials are deliberately kept out of serialized
//! request types and all returned messages.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::net::{IpAddr, SocketAddr};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{lookup_host, TcpStream, UdpSocket};
use zeroize::{Zeroize, Zeroizing};

const MAX_TRANSFER_BYTES: usize = 16 * 1024 * 1024;
const MAX_TIMEOUT: Duration = Duration::from_secs(120);
const TFTP_BLOCK_SIZE: usize = 512;
const TFTP_RETRIES: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferProtocol {
    Ftp,
    Tftp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}

/// The host and paths are expected to have been bound to the approved plan.
/// `content` is accepted only for uploads and is capped at 16 MiB.
pub struct TransferRequest {
    pub host: String,
    pub port: u16,
    pub protocol: TransferProtocol,
    pub direction: TransferDirection,
    pub username: Option<String>,
    pub password: Option<String>,
    pub remote_path: String,
    pub local_path: String,
    pub content: Option<Vec<u8>>,
    pub mode: Option<String>,
    pub timeout_secs: u64,
}

impl Drop for TransferRequest {
    fn drop(&mut self) {
        if let Some(username) = &mut self.username {
            username.zeroize();
        }
        if let Some(password) = &mut self.password {
            password.zeroize();
        }
        if let Some(content) = &mut self.content {
            content.zeroize();
        }
    }
}

impl std::fmt::Debug for TransferRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TransferRequest")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("protocol", &self.protocol)
            .field("direction", &self.direction)
            .field("username", &self.username.as_ref().map(|_| "[REDACTED]"))
            .field("password", &self.password.as_ref().map(|_| "[REDACTED]"))
            .field("remote_path", &self.remote_path)
            .field("local_path", &self.local_path)
            .field(
                "content",
                &self.content.as_ref().map(|content| content.len()),
            )
            .field("mode", &self.mode)
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub output: String,
    pub file_path: Option<PathBuf>,
    pub bytes_transferred: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct SerialConsoleRequest {
    pub port_path: String,
    pub message: String,
    pub baud_rate: Option<u32>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerialConsoleResult {
    pub output: String,
    pub bytes_written: usize,
}

/// Execute one approved FTP or TFTP upload/download.
pub async fn execute_file_transfer(req: TransferRequest) -> Result<TransferResult, String> {
    validate_transfer_request(&req)?;
    let target = resolve_target(&req.host, req.port).await?;
    let started = Instant::now();
    let transfer = async {
        match (req.protocol, req.direction) {
            (TransferProtocol::Ftp, TransferDirection::Upload) => ftp_upload(&req, target).await,
            (TransferProtocol::Ftp, TransferDirection::Download) => {
                ftp_download(&req, target).await
            }
            (TransferProtocol::Tftp, TransferDirection::Upload) => tftp_upload(&req, target).await,
            (TransferProtocol::Tftp, TransferDirection::Download) => {
                tftp_download(&req, target).await
            }
        }
    };
    let total_timeout = match req.protocol {
        TransferProtocol::Ftp => Duration::from_secs(req.timeout_secs),
        TransferProtocol::Tftp => Duration::from_secs(
            req.timeout_secs
                .saturating_mul(TFTP_RETRIES as u64)
                .min(120),
        ),
    };
    let (bytes, count) = tokio::time::timeout(total_timeout, transfer)
        .await
        .map_err(|_| "File transfer timed out".to_string())??;
    let bytes = Zeroizing::new(bytes);
    if req.direction == TransferDirection::Download {
        save_download(&req.local_path, &bytes)?;
    }
    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let action = match req.direction {
        TransferDirection::Upload => "uploaded",
        TransferDirection::Download => "downloaded",
    };
    Ok(TransferResult {
        output: format!(
            "{} {} bytes via {:?} {}:{} to {}",
            action, count, req.protocol, req.host, req.port, req.remote_path
        ),
        file_path: match req.direction {
            TransferDirection::Download => Some(PathBuf::from(&req.local_path)),
            TransferDirection::Upload if req.content.is_none() => {
                Some(PathBuf::from(&req.local_path))
            }
            TransferDirection::Upload => None,
        },
        bytes_transferred: count,
        duration_ms,
    })
}

/// Send a single command to a serial console at 9600 baud and collect its response.
pub fn send_serial_console(req: SerialConsoleRequest) -> Result<SerialConsoleResult, String> {
    validate_serial_port(&req.port_path)?;
    validate_console_message(&req.message)?;
    let baud_rate = req.baud_rate.unwrap_or(9600);
    let timeout_ms = req.timeout_ms.unwrap_or(1000);
    if !(300..=4_000_000).contains(&baud_rate) || timeout_ms == 0 || timeout_ms > 120_000 {
        return Err("Serial baud rate or timeout is outside the supported range".into());
    }
    let mut port = serialport::new(&req.port_path, baud_rate)
        .timeout(Duration::from_millis(timeout_ms))
        .open()
        .map_err(|e| format!("Failed to open serial port: {e}"))?;
    let written = write_console_message(&mut port, &req.message)?;
    std::thread::sleep(Duration::from_millis(500));
    let mut output = Vec::new();
    let mut buffer = [0u8; 1024];
    while output.len() < 4096 {
        let remaining = (4096 - output.len()).min(buffer.len());
        match std::io::Read::read(&mut port, &mut buffer[..remaining]) {
            Ok(0) => break,
            Ok(n) => output.extend_from_slice(&buffer[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(format!("Failed to read serial response: {e}")),
        }
    }
    Ok(SerialConsoleResult {
        output: String::from_utf8_lossy(&output).into_owned(),
        bytes_written: written,
    })
}

fn write_console_message(writer: &mut impl Write, message: &str) -> Result<usize, String> {
    let mut bytes = message.as_bytes().to_vec();
    if !bytes.ends_with(b"\n") && !bytes.ends_with(b"\r") {
        bytes.push(b'\r');
    }
    writer
        .write_all(&bytes)
        .map_err(|e| format!("Failed to write serial command: {e}"))?;
    Ok(bytes.len())
}

fn validate_transfer_request(req: &TransferRequest) -> Result<(), String> {
    validate_host(&req.host)?;
    if req.port == 0 {
        return Err("Transfer port must be between 1 and 65535".into());
    }
    if req.timeout_secs == 0 || Duration::from_secs(req.timeout_secs) > MAX_TIMEOUT {
        return Err("Transfer timeout must be between 1 and 120 seconds".into());
    }
    validate_remote_path(&req.remote_path)?;
    if req.direction == TransferDirection::Download || req.content.is_none() {
        validate_local_path(&req.local_path, req.direction)?;
    }
    if let Some(content) = &req.content {
        if req.direction != TransferDirection::Upload {
            return Err("Inline content is only valid for uploads".into());
        }
        if content.len() > MAX_TRANSFER_BYTES {
            return Err("Transfer content exceeds the 16 MiB limit".into());
        }
    }
    if req.protocol == TransferProtocol::Tftp && (req.username.is_some() || req.password.is_some())
    {
        return Err("TFTP does not support username or password credentials".into());
    }
    if let Some(mode) = &req.mode {
        if !["octet", "netascii", "mail"]
            .iter()
            .any(|supported| mode.eq_ignore_ascii_case(supported))
        {
            return Err("TFTP mode must be octet, netascii, or mail".into());
        }
    }
    if req
        .username
        .as_ref()
        .is_some_and(|v| v.len() > 256 || v.contains('\0'))
        || req
            .password
            .as_ref()
            .is_some_and(|v| v.len() > 1024 || v.contains('\0'))
    {
        return Err("FTP credentials are invalid".into());
    }
    Ok(())
}

fn validate_host(host: &str) -> Result<(), String> {
    if host.is_empty() || host.len() > 253 || host.trim() != host {
        return Err("Transfer host is invalid".into());
    }
    if host.parse::<IpAddr>().is_ok() {
        return Ok(());
    }
    if !host
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
        || host.starts_with('.')
        || host.ends_with('.')
        || host.split('.').any(|label| {
            label.is_empty() || label.len() > 63 || label.starts_with('-') || label.ends_with('-')
        })
    {
        return Err("Transfer host must be an IP address or DNS name".into());
    }
    Ok(())
}

fn validate_remote_path(path: &str) -> Result<(), String> {
    if path.is_empty()
        || path.len() > 1024
        || path.contains('\0')
        || path.chars().any(char::is_control)
    {
        return Err("Remote path is invalid".into());
    }
    if Path::new(path)
        .components()
        .any(|part| matches!(part, Component::ParentDir))
        || path.split(['/', '\\']).any(|part| part == "..")
    {
        return Err("Remote path cannot contain parent-directory traversal".into());
    }
    Ok(())
}

fn validate_local_path(path: &str, direction: TransferDirection) -> Result<(), String> {
    let path = Path::new(path);
    if !path.is_absolute() || path.as_os_str().len() > 4096 {
        return Err("Local transfer path must be an absolute path of at most 4096 bytes".into());
    }
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("Local path cannot contain parent-directory traversal".into());
    }
    match direction {
        TransferDirection::Upload => {
            let meta = std::fs::symlink_metadata(path)
                .map_err(|_| "Upload source file is unavailable".to_string())?;
            if !meta.file_type().is_file()
                || meta.file_type().is_symlink()
                || meta.len() > MAX_TRANSFER_BYTES as u64
            {
                return Err("Upload source must be a regular file no larger than 16 MiB".into());
            }
        }
        TransferDirection::Download => {
            let parent = path
                .parent()
                .ok_or_else(|| "Download path has no parent directory".to_string())?;
            if parent.exists() {
                let canonical_parent = parent
                    .canonicalize()
                    .map_err(|_| "Download parent directory is unavailable".to_string())?;
                let name = path
                    .file_name()
                    .ok_or_else(|| "Download path must name a file".to_string())?;
                let candidate = canonical_parent.join(name);
                if let Ok(meta) = std::fs::symlink_metadata(&candidate) {
                    if meta.file_type().is_symlink() || !meta.file_type().is_file() {
                        return Err("Download destination must be a regular file".into());
                    }
                }
            } else {
                let mut existing_parent = parent;
                while !existing_parent.exists() {
                    existing_parent = existing_parent
                        .parent()
                        .ok_or_else(|| "Download path has no existing parent".to_string())?;
                }
                if !existing_parent
                    .canonicalize()
                    .map_err(|_| "Download parent directory is unavailable".to_string())?
                    .is_dir()
                {
                    return Err("Download parent is not a directory".into());
                }
            }
        }
    }
    Ok(())
}

async fn resolve_target(host: &str, port: u16) -> Result<SocketAddr, String> {
    lookup_host((host, port))
        .await
        .map_err(|_| "Could not resolve transfer host".to_string())?
        .next()
        .ok_or_else(|| "Could not resolve transfer host".to_string())
}

async fn read_upload(req: &TransferRequest) -> Result<Vec<u8>, String> {
    if let Some(content) = &req.content {
        return Ok(content.clone());
    }
    let mut data = Zeroizing::new(
        tokio::fs::read(&req.local_path)
            .await
            .map_err(|_| "Could not read upload source file".to_string())?,
    );
    if data.len() > MAX_TRANSFER_BYTES {
        data.zeroize();
        return Err("Upload source exceeds the 16 MiB limit".into());
    }
    Ok(std::mem::take(&mut *data))
}

fn save_download(path: &str, data: &[u8]) -> Result<(), String> {
    if data.len() > MAX_TRANSFER_BYTES {
        return Err("Download exceeds the 16 MiB limit".into());
    }
    // Revalidate immediately before writing, and replace the directory entry
    // atomically so an existing hard link cannot redirect the write elsewhere.
    validate_local_path(path, TransferDirection::Download)?;
    let destination = Path::new(path);
    let original_parent = destination
        .parent()
        .ok_or_else(|| "Download path has no parent directory".to_string())?;
    std::fs::create_dir_all(original_parent)
        .map_err(|_| "Could not create download parent directory".to_string())?;
    let parent = original_parent
        .canonicalize()
        .map_err(|_| "Download parent directory is unavailable".to_string())?;
    let name = destination
        .file_name()
        .ok_or_else(|| "Download path must name a file".to_string())?;
    let resolved_destination = parent.join(name);
    if std::fs::symlink_metadata(&resolved_destination)
        .is_ok_and(|meta| meta.file_type().is_symlink() || !meta.file_type().is_file())
    {
        return Err("Download destination must be a regular file".into());
    }
    let temporary = parent.join(format!(".mikomai-transfer-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| "Could not create temporary download file".to_string())?;
    if let Err(_) = file.write_all(data) {
        let _ = std::fs::remove_file(&temporary);
        return Err("Could not write downloaded file".into());
    }
    drop(file);
    if std::fs::rename(&temporary, &resolved_destination).is_err() {
        let _ = std::fs::remove_file(&temporary);
        return Err("Could not replace download destination".into());
    }
    Ok(())
}

async fn ftp_connect(
    target: SocketAddr,
    req: &TransferRequest,
) -> Result<(BufReader<TcpStream>, SocketAddr), String> {
    let stream = TcpStream::connect(target)
        .await
        .map_err(|_| "Could not connect to FTP server".to_string())?;
    let peer = stream
        .peer_addr()
        .map_err(|_| "Could not inspect FTP peer".to_string())?;
    let mut control = BufReader::new(stream);
    let (code, _) = ftp_reply(&mut control).await?;
    if code != 220 {
        return Err("FTP server rejected the connection".into());
    }
    ftp_command(
        &mut control,
        "USER",
        req.username.as_deref().unwrap_or("anonymous"),
    )
    .await?;
    let (code, _) = ftp_reply(&mut control).await?;
    if code == 331 {
        ftp_command(
            &mut control,
            "PASS",
            req.password.as_deref().unwrap_or("anonymous@"),
        )
        .await?;
        let (code, _) = ftp_reply(&mut control).await?;
        if code != 230 {
            return Err("FTP authentication failed".into());
        }
    } else if code != 230 {
        return Err("FTP authentication failed".into());
    }
    ftp_command(&mut control, "TYPE", "I").await?;
    let (code, _) = ftp_reply(&mut control).await?;
    if code != 200 {
        return Err("FTP server rejected binary transfer mode".into());
    }
    Ok((control, peer))
}

async fn ftp_passive(
    control: &mut BufReader<TcpStream>,
    peer: SocketAddr,
) -> Result<SocketAddr, String> {
    ftp_command(control, "EPSV", "").await?;
    let (code, response) = ftp_reply(control).await?;
    let port = if code == 229 {
        let start = response
            .find('(')
            .ok_or_else(|| "Malformed FTP passive response".to_string())?;
        let end = response[start..]
            .find(')')
            .map(|i| start + i)
            .ok_or_else(|| "Malformed FTP passive response".to_string())?;
        let fields: Vec<_> = response[start + 1..end]
            .split('|')
            .filter(|v| !v.is_empty())
            .collect();
        if fields.len() != 1 {
            return Err("Malformed FTP passive response".into());
        }
        fields[0]
            .parse::<u16>()
            .ok()
            .filter(|v| *v != 0)
            .ok_or_else(|| "Malformed FTP passive response".to_string())?
    } else {
        ftp_command(control, "PASV", "").await?;
        let (code, response) = ftp_reply(control).await?;
        if code != 227 {
            return Err("FTP server did not provide a passive data port".into());
        }
        let start = response
            .find('(')
            .ok_or_else(|| "Malformed FTP passive response".to_string())?;
        let end = response[start..]
            .find(')')
            .map(|i| start + i)
            .ok_or_else(|| "Malformed FTP passive response".to_string())?;
        let nums: Vec<u16> = response[start + 1..end]
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        if nums.len() != 6 || nums.iter().any(|v| *v > 255) {
            return Err("Malformed FTP passive response".into());
        }
        nums[4] * 256 + nums[5]
    };
    Ok(SocketAddr::new(peer.ip(), port))
}

async fn ftp_upload(req: &TransferRequest, target: SocketAddr) -> Result<(Vec<u8>, usize), String> {
    let data = Zeroizing::new(read_upload(req).await?);
    let (mut control, peer) = ftp_connect(target, req).await?;
    let data_addr = ftp_passive(&mut control, peer).await?;
    let mut data_stream = TcpStream::connect(data_addr)
        .await
        .map_err(|_| "Could not open FTP data connection".to_string())?;
    ftp_command(&mut control, "STOR", &req.remote_path).await?;
    let (code, _) = ftp_reply(&mut control).await?;
    if code != 125 && code != 150 {
        return Err("FTP server refused the upload".into());
    }
    data_stream
        .write_all(&data)
        .await
        .map_err(|_| "FTP upload failed while sending data".to_string())?;
    data_stream
        .shutdown()
        .await
        .map_err(|_| "FTP upload failed while closing data connection".to_string())?;
    let (code, _) = ftp_reply(&mut control).await?;
    if code != 226 && code != 250 {
        return Err("FTP server did not confirm the upload".into());
    }
    Ok((Vec::new(), data.len()))
}

async fn ftp_download(
    req: &TransferRequest,
    target: SocketAddr,
) -> Result<(Vec<u8>, usize), String> {
    let (mut control, peer) = ftp_connect(target, req).await?;
    let data_addr = ftp_passive(&mut control, peer).await?;
    let data_stream = TcpStream::connect(data_addr)
        .await
        .map_err(|_| "Could not open FTP data connection".to_string())?;
    ftp_command(&mut control, "RETR", &req.remote_path).await?;
    let (code, _) = ftp_reply(&mut control).await?;
    if code != 125 && code != 150 {
        return Err("FTP server refused the download".into());
    }
    let mut data = Zeroizing::new(Vec::new());
    data_stream
        .take((MAX_TRANSFER_BYTES + 1) as u64)
        .read_to_end(&mut data)
        .await
        .map_err(|_| "FTP download failed while receiving data".to_string())?;
    if data.len() > MAX_TRANSFER_BYTES {
        return Err("Download exceeds the 16 MiB limit".into());
    }
    let (code, _) = ftp_reply(&mut control).await?;
    if code != 226 && code != 250 {
        return Err("FTP server did not confirm the download".into());
    }
    let count = data.len();
    Ok((data.to_vec(), count))
}

async fn ftp_command(
    control: &mut BufReader<TcpStream>,
    command: &str,
    arg: &str,
) -> Result<(), String> {
    if arg.contains(['\r', '\n', '\0']) {
        return Err("FTP argument contains forbidden characters".into());
    }
    let mut line = Zeroizing::new(Vec::new());
    line.extend_from_slice(command.as_bytes());
    if !arg.is_empty() {
        line.push(b' ');
        line.extend_from_slice(arg.as_bytes());
    }
    line.extend_from_slice(b"\r\n");
    control
        .get_mut()
        .write_all(&line)
        .await
        .map_err(|_| "Could not send FTP command".to_string())
}

async fn ftp_reply(control: &mut BufReader<TcpStream>) -> Result<(u16, String), String> {
    let mut line = String::new();
    control
        .read_line(&mut line)
        .await
        .map_err(|_| "Could not read FTP response".to_string())?;
    let bytes = line.as_bytes();
    if bytes.len() < 3 || !bytes[..3].iter().all(u8::is_ascii_digit) {
        return Err("Malformed FTP response".into());
    }
    let code = std::str::from_utf8(&bytes[..3])
        .unwrap()
        .parse::<u16>()
        .map_err(|_| "Malformed FTP response".to_string())?;
    let multi = line.as_bytes().get(3) == Some(&b'-');
    let mut total = line.len();
    while multi {
        line.clear();
        let n = control
            .read_line(&mut line)
            .await
            .map_err(|_| "Could not read FTP response".to_string())?;
        if n == 0 {
            return Err("Truncated FTP response".into());
        }
        total += n;
        if total > 8192 {
            return Err("FTP response is too large".into());
        }
        if line.starts_with(&format!("{code} ")) {
            break;
        }
    }
    Ok((code, line.trim().to_owned()))
}

async fn tftp_upload(
    req: &TransferRequest,
    target: SocketAddr,
) -> Result<(Vec<u8>, usize), String> {
    let data = Zeroizing::new(read_upload(req).await?);
    let socket = UdpSocket::bind(if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })
    .await
    .map_err(|_| "Could not bind TFTP socket".to_string())?;
    let wrq = tftp_request(2, &req.remote_path, req.mode.as_deref().unwrap_or("octet"));
    socket
        .send_to(&wrq, target)
        .await
        .map_err(|_| "Could not send TFTP request".to_string())?;
    let mut packet = [0u8; 516];
    let mut retries = 0;
    let tid = loop {
        match tokio::time::timeout(
            Duration::from_secs(req.timeout_secs),
            socket.recv_from(&mut packet),
        )
        .await
        {
            Ok(Ok((n, addr))) if n >= 4 && addr.ip() == target.ip() => {
                if packet_opcode(&packet[..n]) == Some(5) {
                    return Err(parse_tftp_error(&packet[..n]));
                }
                if packet_opcode(&packet[..n]) != Some(4) || packet_block(&packet[..n]) != Some(0) {
                    return Err("Unexpected TFTP response to upload request".into());
                }
                break addr;
            }
            Ok(Ok(_)) => return Err("Malformed TFTP response to upload request".into()),
            Ok(Err(_)) => return Err("TFTP receive failed".into()),
            Err(_) => {
                retries += 1;
                if retries >= TFTP_RETRIES {
                    return Err("TFTP upload timed out waiting for acknowledgement".into());
                }
                socket
                    .send_to(&wrq, target)
                    .await
                    .map_err(|_| "Could not resend TFTP request".to_string())?;
            }
        }
    };
    let mut block: u16 = 1;
    let mut offset = 0;
    loop {
        let end = (offset + TFTP_BLOCK_SIZE).min(data.len());
        let chunk = &data[offset..end];
        let data_packet = Zeroizing::new(tftp_data(block, chunk));
        let mut retries = 0;
        loop {
            socket
                .send_to(&data_packet, tid)
                .await
                .map_err(|_| "Could not send TFTP data".to_string())?;
            match tokio::time::timeout(
                Duration::from_secs(req.timeout_secs),
                socket.recv_from(&mut packet),
            )
            .await
            {
                Ok(Ok((n, addr))) if addr == tid && n >= 4 => {
                    if packet_opcode(&packet[..n]) == Some(5) {
                        return Err(parse_tftp_error(&packet[..n]));
                    }
                    if packet_opcode(&packet[..n]) == Some(4)
                        && packet_block(&packet[..n]) == Some(block)
                    {
                        break;
                    }
                }
                Ok(Err(_)) => return Err("TFTP receive failed".into()),
                _ => {}
            }
            retries += 1;
            if retries >= TFTP_RETRIES {
                return Err("TFTP upload timed out waiting for acknowledgement".into());
            }
        }
        offset = end;
        if chunk.len() < TFTP_BLOCK_SIZE {
            break;
        }
        block = block.wrapping_add(1);
        if block == 0 {
            return Err("TFTP transfer exceeds supported block count".into());
        }
    }
    Ok((Vec::new(), data.len()))
}

async fn tftp_download(
    req: &TransferRequest,
    target: SocketAddr,
) -> Result<(Vec<u8>, usize), String> {
    let socket = UdpSocket::bind(if target.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })
    .await
    .map_err(|_| "Could not bind TFTP socket".to_string())?;
    let rrq = tftp_request(1, &req.remote_path, req.mode.as_deref().unwrap_or("octet"));
    socket
        .send_to(&rrq, target)
        .await
        .map_err(|_| "Could not send TFTP request".to_string())?;
    let mut bytes = Zeroizing::new(Vec::new());
    let mut packet = [0u8; 516];
    let mut tid = None;
    let mut expected: u16 = 1;
    loop {
        let mut retries = 0;
        let n = loop {
            match tokio::time::timeout(
                Duration::from_secs(req.timeout_secs),
                socket.recv_from(&mut packet),
            )
            .await
            {
                Ok(Ok((n, addr))) => {
                    if tid.is_none() && addr.ip() != target.ip() {
                        continue;
                    }
                    if tid.is_some_and(|expected_tid| expected_tid != addr) {
                        continue;
                    }
                    tid = Some(addr);
                    break n;
                }
                Ok(Err(_)) => return Err("TFTP receive failed".into()),
                Err(_) => {
                    retries += 1;
                    if retries >= TFTP_RETRIES {
                        return Err("TFTP download timed out waiting for data".into());
                    }
                    if expected == 1 {
                        let _ = socket.send_to(&rrq, target).await;
                    } else if let Some(addr) = tid {
                        let _ = socket
                            .send_to(&tftp_ack(expected.wrapping_sub(1)), addr)
                            .await;
                    }
                }
            }
        };
        if n < 4 {
            return Err("Malformed TFTP packet".into());
        }
        match packet_opcode(&packet[..n]) {
            Some(3) if packet_block(&packet[..n]) == Some(expected) => {
                let chunk = &packet[4..n];
                if bytes.len() + chunk.len() > MAX_TRANSFER_BYTES {
                    return Err("Download exceeds the 16 MiB limit".into());
                }
                bytes.extend_from_slice(chunk);
                socket
                    .send_to(&tftp_ack(expected), tid.unwrap())
                    .await
                    .map_err(|_| "Could not acknowledge TFTP data".to_string())?;
                expected = expected.wrapping_add(1);
                if chunk.len() < TFTP_BLOCK_SIZE {
                    break;
                }
                if expected == 0 {
                    return Err("TFTP transfer exceeds supported block count".into());
                }
            }
            Some(5) => return Err(parse_tftp_error(&packet[..n])),
            Some(3) if packet_block(&packet[..n]) == Some(expected.wrapping_sub(1)) => {
                socket
                    .send_to(&tftp_ack(expected.wrapping_sub(1)), tid.unwrap())
                    .await
                    .map_err(|_| "Could not acknowledge repeated TFTP data".to_string())?;
            }
            _ => return Err("Unexpected TFTP response".into()),
        }
    }
    let len = bytes.len();
    Ok((bytes.to_vec(), len))
}

fn tftp_request(op: u16, path: &str, mode: &str) -> Vec<u8> {
    let mut packet = op.to_be_bytes().to_vec();
    packet.extend_from_slice(path.as_bytes());
    packet.push(0);
    packet.extend_from_slice(mode.to_ascii_lowercase().as_bytes());
    packet.push(0);
    packet
}
fn tftp_data(block: u16, bytes: &[u8]) -> Vec<u8> {
    let mut packet = vec![0, 3];
    packet.extend_from_slice(&block.to_be_bytes());
    packet.extend_from_slice(bytes);
    packet
}
fn tftp_ack(block: u16) -> Vec<u8> {
    vec![0, 4, (block >> 8) as u8, block as u8]
}
fn packet_opcode(packet: &[u8]) -> Option<u16> {
    (packet.len() >= 2).then(|| u16::from_be_bytes([packet[0], packet[1]]))
}
fn packet_block(packet: &[u8]) -> Option<u16> {
    (packet.len() >= 4).then(|| u16::from_be_bytes([packet[2], packet[3]]))
}
fn parse_tftp_error(packet: &[u8]) -> String {
    if packet.len() < 4 {
        return "Malformed TFTP error packet".into();
    }
    let code = u16::from_be_bytes([packet[2], packet[3]]);
    let message = packet[4..]
        .split(|byte| *byte == 0)
        .next()
        .unwrap_or_default();
    format!(
        "TFTP server returned error {code}: {}",
        String::from_utf8_lossy(message)
    )
}
fn validate_serial_port(port: &str) -> Result<(), String> {
    let trimmed = port.trim();
    if trimmed.is_empty()
        || trimmed.len() > 256
        || trimmed.contains('\0')
        || trimmed.contains("..")
        || trimmed.chars().any(char::is_control)
    {
        return Err("Serial port path is invalid".into());
    }
    if trimmed
        .get(..3)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("COM"))
    {
        let n = &trimmed[3..];
        if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && !n.starts_with('0') {
            return Ok(());
        }
    }
    if let Some(name) = trimmed.strip_prefix("/dev/") {
        if !name.contains('/')
            && !name.contains('\\')
            && (name.starts_with("tty") || name.starts_with("cu."))
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Ok(());
        }
    }
    Err("Serial port must be COM<N>, /dev/tty*, or /dev/cu.*".into())
}

fn validate_console_message(message: &str) -> Result<(), String> {
    let trimmed = message.trim();
    if trimmed.is_empty() || message.len() > 8192 || message.contains('\0') {
        return Err("Serial command is empty or too long".into());
    }
    let body = message.trim_end_matches(['\r', '\n']);
    if body.contains(['\r', '\n'])
        || body.chars().any(|c| c.is_control() && c != '\t')
        || body.contains([';', '|', '&', '$', '(', ')', '`', '>', '<', '\\', '"', '\''])
    {
        return Err("Serial command contains forbidden characters".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn request(
        protocol: TransferProtocol,
        direction: TransferDirection,
        host: String,
        port: u16,
        local_path: String,
        content: Option<Vec<u8>>,
    ) -> TransferRequest {
        TransferRequest {
            host,
            port,
            protocol,
            direction,
            username: None,
            password: None,
            remote_path: "test.bin".into(),
            local_path,
            content,
            mode: None,
            timeout_secs: 3,
        }
    }

    #[test]
    fn serial_command_writes_carriage_return_and_rejects_command_chains() {
        #[derive(Clone)]
        struct Capture(Arc<Mutex<Vec<u8>>>);
        impl Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let captured = Arc::new(Mutex::new(Vec::new()));
        assert_eq!(
            write_console_message(&mut Capture(captured.clone()), "show version").unwrap(),
            13
        );
        assert_eq!(&*captured.lock().unwrap(), b"show version\r");
        assert!(validate_console_message("show version\nreload").is_err());
        assert!(validate_serial_port("/dev/ttyUSB0;evil").is_err());
    }

    #[tokio::test]
    async fn tftp_download_and_upload_transfer_payload_over_loopback() {
        let download_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let download_addr = download_socket.local_addr().unwrap();
        let download_server = tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let (_, client) = download_socket.recv_from(&mut buf).await.unwrap();
            assert_eq!(packet_opcode(&buf), Some(1));
            download_socket
                .send_to(&tftp_data(1, b"loopback payload"), client)
                .await
                .unwrap();
            let (n, _) = download_socket.recv_from(&mut buf).await.unwrap();
            assert_eq!(&buf[..n], &tftp_ack(1));
        });
        let tmp =
            std::env::temp_dir().join(format!("mikomai-tftp-download-{}", uuid::Uuid::new_v4()));
        let result = execute_file_transfer(request(
            TransferProtocol::Tftp,
            TransferDirection::Download,
            "127.0.0.1".into(),
            download_addr.port(),
            tmp.display().to_string(),
            None,
        ))
        .await
        .unwrap();
        assert_eq!(std::fs::read(&tmp).unwrap(), b"loopback payload");
        assert_eq!(result.bytes_transferred, 16);
        std::fs::remove_file(&tmp).unwrap();
        download_server.await.unwrap();

        let upload_socket = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let upload_addr = upload_socket.local_addr().unwrap();
        let upload_server = tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            let (_, client) = upload_socket.recv_from(&mut buf).await.unwrap();
            assert_eq!(packet_opcode(&buf), Some(2));
            upload_socket.send_to(&tftp_ack(0), client).await.unwrap();
            let (n, client) = upload_socket.recv_from(&mut buf).await.unwrap();
            assert_eq!(packet_opcode(&buf[..n]), Some(3));
            assert_eq!(&buf[4..n], b"upload bytes");
            upload_socket.send_to(&tftp_ack(1), client).await.unwrap();
        });
        let source =
            std::env::temp_dir().join(format!("mikomai-tftp-upload-{}", uuid::Uuid::new_v4()));
        std::fs::write(&source, b"upload bytes").unwrap();
        let result = execute_file_transfer(request(
            TransferProtocol::Tftp,
            TransferDirection::Upload,
            "127.0.0.1".into(),
            upload_addr.port(),
            source.display().to_string(),
            None,
        ))
        .await
        .unwrap();
        assert_eq!(result.bytes_transferred, 12);
        std::fs::remove_file(&source).unwrap();
        upload_server.await.unwrap();
    }

    #[tokio::test]
    async fn ftp_upload_sends_file_through_loopback_server_without_exposing_credentials() {
        use tokio::net::TcpListener;
        let control_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let control_addr = control_listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = control_listener.accept().await.unwrap();
            let mut control = BufReader::new(stream);
            control
                .get_mut()
                .write_all(b"220 local test FTP\r\n")
                .await
                .unwrap();
            let mut passive: Option<TcpListener> = None;
            let mut line = String::new();
            let mut uploaded = Vec::new();
            loop {
                line.clear();
                if control.read_line(&mut line).await.unwrap() == 0 {
                    break;
                }
                if line.starts_with("USER ") {
                    control
                        .get_mut()
                        .write_all(b"331 password required\r\n")
                        .await
                        .unwrap();
                } else if line.starts_with("PASS ") {
                    assert_eq!(line.trim_end(), "PASS never-in-result");
                    control
                        .get_mut()
                        .write_all(b"230 logged in\r\n")
                        .await
                        .unwrap();
                } else if line.starts_with("TYPE I") {
                    control
                        .get_mut()
                        .write_all(b"200 binary\r\n")
                        .await
                        .unwrap();
                } else if line.starts_with("EPSV") {
                    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                    let port = listener.local_addr().unwrap().port();
                    passive = Some(listener);
                    control
                        .get_mut()
                        .write_all(
                            format!("229 Entering Extended Passive Mode (|||{port}|)\r\n")
                                .as_bytes(),
                        )
                        .await
                        .unwrap();
                } else if line.starts_with("STOR test.bin") {
                    control
                        .get_mut()
                        .write_all(b"150 opening data\r\n")
                        .await
                        .unwrap();
                    let (mut data, _) = passive.take().unwrap().accept().await.unwrap();
                    data.read_to_end(&mut uploaded).await.unwrap();
                    control
                        .get_mut()
                        .write_all(b"226 transfer complete\r\n")
                        .await
                        .unwrap();
                    break;
                } else {
                    panic!("unexpected FTP command: {line:?}");
                }
            }
            uploaded
        });
        let source =
            std::env::temp_dir().join(format!("mikomai-ftp-upload-{}", uuid::Uuid::new_v4()));
        std::fs::write(&source, b"ftp loopback bytes").unwrap();
        let mut req = request(
            TransferProtocol::Ftp,
            TransferDirection::Upload,
            "127.0.0.1".into(),
            control_addr.port(),
            source.display().to_string(),
            None,
        );
        req.username = Some("test-user".into());
        req.password = Some("never-in-result".into());
        let result = execute_file_transfer(req).await.unwrap();
        let uploaded = server.await.unwrap();
        assert_eq!(uploaded, b"ftp loopback bytes");
        assert_eq!(result.bytes_transferred, uploaded.len());
        assert!(!result.output.contains("test-user"));
        assert!(!result.output.contains("never-in-result"));
        std::fs::remove_file(&source).unwrap();
    }

    #[tokio::test]
    async fn ftp_download_overwrites_regular_destination_with_binary_payload() {
        use tokio::net::TcpListener;
        let control_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let control_addr = control_listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (stream, _) = control_listener.accept().await.unwrap();
            let mut control = BufReader::new(stream);
            control
                .get_mut()
                .write_all(b"220 local test FTP\r\n")
                .await
                .unwrap();
            let mut passive: Option<TcpListener> = None;
            let mut line = String::new();
            loop {
                line.clear();
                if control.read_line(&mut line).await.unwrap() == 0 {
                    break;
                }
                if line.starts_with("USER ") {
                    control
                        .get_mut()
                        .write_all(b"230 logged in\r\n")
                        .await
                        .unwrap();
                } else if line.starts_with("TYPE I") {
                    control
                        .get_mut()
                        .write_all(b"200 binary\r\n")
                        .await
                        .unwrap();
                } else if line.starts_with("EPSV") {
                    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                    let port = listener.local_addr().unwrap().port();
                    passive = Some(listener);
                    control
                        .get_mut()
                        .write_all(
                            format!("229 Entering Extended Passive Mode (|||{port}|)\r\n")
                                .as_bytes(),
                        )
                        .await
                        .unwrap();
                } else if line.starts_with("RETR test.bin") {
                    control
                        .get_mut()
                        .write_all(b"150 opening data\r\n")
                        .await
                        .unwrap();
                    let (mut data, _) = passive.take().unwrap().accept().await.unwrap();
                    data.write_all(&[0, 1, 255, 10, 13, 0]).await.unwrap();
                    data.shutdown().await.unwrap();
                    control
                        .get_mut()
                        .write_all(b"226 transfer complete\r\n")
                        .await
                        .unwrap();
                    break;
                } else {
                    panic!("unexpected FTP command: {line:?}");
                }
            }
        });
        let destination =
            std::env::temp_dir().join(format!("mikomai-ftp-download-{}", uuid::Uuid::new_v4()));
        std::fs::write(&destination, b"old contents").unwrap();
        let mut req = request(
            TransferProtocol::Ftp,
            TransferDirection::Download,
            "127.0.0.1".into(),
            control_addr.port(),
            destination.display().to_string(),
            None,
        );
        req.username = Some("download-user".into());
        req.password = Some("download-secret".into());
        let debug = format!("{req:?}");
        assert!(!debug.contains("download-user"));
        assert!(!debug.contains("download-secret"));
        let result = execute_file_transfer(req).await.unwrap();
        server.await.unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), [0, 1, 255, 10, 13, 0]);
        assert_eq!(result.bytes_transferred, 6);
        assert!(!result.output.contains("download-user"));
        assert!(!result.output.contains("download-secret"));
        std::fs::remove_file(&destination).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn download_destination_symlink_is_rejected() {
        let root =
            std::env::temp_dir().join(format!("mikomai-transfer-path-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let victim = root.join("victim.bin");
        std::fs::write(&victim, b"keep me").unwrap();
        let link = root.join("approved.bin");
        std::os::unix::fs::symlink(&victim, &link).unwrap();
        assert!(validate_local_path(link.to_str().unwrap(), TransferDirection::Download).is_err());
        assert!(save_download(link.to_str().unwrap(), b"replace me").is_err());
        assert_eq!(std::fs::read(&victim).unwrap(), b"keep me");
        std::fs::remove_file(link).unwrap();
        std::fs::remove_file(victim).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn transfer_validation_blocks_remote_traversal_credentials_for_tftp_and_large_timeouts() {
        let mut req = request(
            TransferProtocol::Tftp,
            TransferDirection::Upload,
            "localhost".into(),
            69,
            "/does/not/need/to/exist".into(),
            Some(vec![]),
        );
        req.remote_path = "../startup-config".into();
        assert!(validate_transfer_request(&req).is_err());
        req.remote_path = "startup-config".into();
        req.username = Some("unexpected".into());
        assert!(validate_transfer_request(&req).is_err());
        req.username = None;
        req.mode = Some("unsupported".into());
        assert!(validate_transfer_request(&req).is_err());
        req.mode = None;
        req.timeout_secs = 121;
        assert!(validate_transfer_request(&req).is_err());
    }
}
