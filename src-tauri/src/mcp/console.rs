use crate::mcp::protocol::McpToolResult;
use serde::{Deserialize, Serialize};
use serialport::SerialPortType;
use std::io::{Read, Write};
use std::time::Duration;

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
pub struct SerialPortInfo {
    pub port_name: String,
    pub port_type: String,
}

pub type ConsoleResult = McpToolResult;

#[tauri::command]
pub fn network_list_serial_ports() -> Result<ConsoleResult, String> {
    let ports = serialport::available_ports().map_err(|e| e.to_string())?;

    if ports.is_empty() {
        return Ok(ConsoleResult {
            success: true,
            output: "No serial ports found.".to_string(),
        });
    }

    let mut output = String::from("Available serial ports:\n\n");
    for p in ports {
        let port_type = match p.port_type {
            SerialPortType::UsbPort(info) => format!(
                "USB ({})",
                info.product.unwrap_or_else(|| "Unknown".to_string())
            ),
            SerialPortType::PciPort => "PCI".to_string(),
            SerialPortType::BluetoothPort => "Bluetooth".to_string(),
            SerialPortType::Unknown => "Unknown".to_string(),
        };
        output.push_str(&format!("- {}: {}\n", p.port_name, port_type));
    }

    Ok(ConsoleResult {
        success: true,
        output,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serial_port_info_serialization() {
        let info = SerialPortInfo {
            port_name: "COM1".to_string(),
            port_type: "USB".to_string(),
        };
        let serialized = serde_json::to_string(&info).unwrap();
        assert!(serialized.contains(r#""port_name":"COM1""#));
        assert!(serialized.contains(r#""port_type":"USB""#));
    }

    #[test]
    fn test_console_result_serialization() {
        let result = ConsoleResult {
            success: true,
            output: "console output".to_string(),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert!(serialized.contains(r#""success":true"#));
    }

    #[test]
    fn test_validate_serial_port_valid() {
        assert!(validate_serial_port("COM1").is_ok());
        assert!(validate_serial_port("COM25").is_ok());
        assert!(validate_serial_port("/dev/ttyUSB0").is_ok());
        assert!(validate_serial_port("/dev/ttyACM1").is_ok());
        assert!(validate_serial_port("/dev/tty.usbserial-1420").is_ok());
        assert!(validate_serial_port("/dev/cu.usbmodem1101").is_ok());
    }

    #[test]
    fn test_validate_serial_port_invalid() {
        assert!(validate_serial_port("").is_err());
        assert!(validate_serial_port("../../../dev/ttyUSB0").is_err());
        assert!(validate_serial_port("/dev/ttyUSB0; rm -rf /").is_err());
        assert!(validate_serial_port("/dev/null").is_err());
        assert!(validate_serial_port("/dev/urandom").is_err());
        assert!(validate_serial_port("/dev/ptmx").is_err());
        assert!(validate_serial_port("/dev/tty/foo").is_err());
        assert!(validate_serial_port("COM0").is_err());
        assert!(validate_serial_port("LPT1").is_err());
    }

    #[test]
    fn test_validate_console_message_valid() {
        assert!(validate_console_message("show ip route").is_ok());
        assert!(validate_console_message("show running-config\r\n").is_ok());
        assert!(validate_console_message("interface GigabitEthernet0/1").is_ok());
        assert!(validate_console_message("ip address 192.168.1.1 255.255.255.0").is_ok());
    }

    #[test]
    fn test_validate_console_message_invalid() {
        assert!(validate_console_message("").is_err());
        assert!(validate_console_message("   ").is_err());
        // CRLF injection (newline in middle of message)
        assert!(validate_console_message("show ip\r\nreload\r\n").is_err());
        assert!(validate_console_message("show ip\nreload").is_err());
        // Shell metacharacters
        assert!(validate_console_message("show ip; cat /etc/passwd").is_err());
        assert!(validate_console_message("show ip | cat").is_err());
        assert!(validate_console_message("show ip && echo evil").is_err());
        assert!(validate_console_message("show ip `id`").is_err());
        assert!(validate_console_message("show ip $(id)").is_err());
        assert!(validate_console_message("show ip > /dev/null").is_err());
        // Null byte
        assert!(validate_console_message("show\0ip").is_err());
    }
}

pub fn validate_serial_port(port: &str) -> Result<(), String> {
    let trimmed = port.trim();
    if trimmed.is_empty() {
        return Err("Serial port cannot be empty".to_string());
    }
    if trimmed.len() > 256 {
        return Err("Serial port path exceeds maximum length".to_string());
    }
    if trimmed.contains('\0') {
        return Err("Serial port contains null bytes".to_string());
    }
    if trimmed.contains("..") {
        return Err("Path traversal detected in serial port".to_string());
    }
    for c in trimmed.chars() {
        if c.is_control() {
            return Err("Serial port contains forbidden control character".to_string());
        }
    }

    // Windows pattern: COM[1-9][0-9]*
    if trimmed.to_uppercase().starts_with("COM") {
        let num_part = &trimmed[3..];
        if !num_part.is_empty()
            && num_part.chars().all(|c| c.is_ascii_digit())
            && !num_part.starts_with('0')
        {
            return Ok(());
        }
        return Err(format!("Invalid Windows serial port name: '{}'", port));
    }

    // Unix / macOS pattern: must start with /dev/
    if trimmed.starts_with("/dev/") {
        let dev_name = &trimmed[5..];

        // Do not allow subdirectories within /dev
        if dev_name.contains('/') || dev_name.contains('\\') {
            return Err(format!("Invalid device path in /dev: '{}'", port));
        }

        // Block known dangerous pseudo-devices
        let blocked_devices = [
            "null", "zero", "urandom", "random", "ptmx", "stdin", "stdout", "stderr",
            "kmem", "mem", "port", "core",
        ];
        if blocked_devices.contains(&dev_name) {
            return Err(format!("Forbidden device path: '{}'", port));
        }

        // Must start with valid serial prefixes (tty, cu.)
        let is_valid_prefix = dev_name.starts_with("tty") || dev_name.starts_with("cu.");
        let is_valid_chars = dev_name.chars().all(|c| c.is_alphanumeric() || c == '.' || c == '-' || c == '_');

        if is_valid_prefix && is_valid_chars {
            return Ok(());
        }
        return Err(format!("Invalid Unix serial device name: '{}'", port));
    }

    Err(format!(
        "Invalid serial port format: '{}'. Expected COM<N> or /dev/tty* / /dev/cu.*",
        port
    ))
}

pub fn validate_console_message(message: &str) -> Result<(), String> {
    if message.contains('\0') {
        return Err("Message contains null bytes".to_string());
    }

    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Err("Message cannot be empty".to_string());
    }
    if message.len() > 8192 {
        return Err("Message exceeds maximum length (8192)".to_string());
    }

    // Check for CRLF injection: mid-string newlines are forbidden.
    // Only trailing newlines (at the very end) are tolerated.
    let trimmed_end = message.trim_end_matches(|c| c == '\r' || c == '\n');
    if trimmed_end.contains('\r') || trimmed_end.contains('\n') {
        return Err("Message contains mid-string newlines (command injection risk)".to_string());
    }

    // Block dangerous shell metacharacters and control characters
    let disallowed_chars = [
        ';', '|', '&', '$', '(', ')', '`', '>', '<', '\\', '"', '\'',
    ];
    for c in trimmed_end.chars() {
        if disallowed_chars.contains(&c) {
            return Err(format!("Message contains forbidden character: '{}'", c));
        }
        if c.is_control() && c != '\t' {
            return Err("Message contains forbidden control character".to_string());
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn network_send_console_message(
    port: String,
    baud_rate: Option<u32>,
    message: String,
    timeout_ms: Option<u64>,
) -> Result<ConsoleResult, String> {
    // Validate inputs
    validate_serial_port(&port)?;
    validate_console_message(&message)?;

    let baud = baud_rate.unwrap_or(9600);
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(1000));

    let mut serial_port = serialport::new(&port, baud)
        .timeout(timeout)
        .open()
        .map_err(|e| format!("Failed to open port {}: {}", port, e))?;

    // Send message (add newline if not present)
    let mut msg = message.clone();
    if !msg.ends_with('\n') && !msg.ends_with('\r') {
        msg.push('\r');
    }

    serial_port
        .write_all(msg.as_bytes())
        .map_err(|e| format!("Failed to write to port: {}", e))?;

    // Wait a bit for the device to respond
    tokio::time::sleep(Duration::from_millis(500)).await;

    // Read response
    let mut buffer = vec![0u8; 4096];
    let mut output = String::new();

    // Loop to read all available data until timeout or buffer full
    loop {
        match serial_port.read(&mut buffer) {
            Ok(bytes_read) if bytes_read > 0 => {
                output.push_str(&String::from_utf8_lossy(&buffer[..bytes_read]));
                // If we read a full buffer, there might be more, but let's limit for now
                if bytes_read < 4096 {
                    break;
                }
            }
            Ok(_) => break, // bytes_read == 0
            Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(e) => return Err(format!("Failed to read from port: {}", e)),
        }
    }

    Ok(ConsoleResult {
        success: true,
        output,
    })
}
