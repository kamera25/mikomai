use crate::mcp::protocol::McpToolResult;
use serde::{Deserialize, Serialize};
use serialport::SerialPortType;
use std::io::{Read, Write};
use std::time::Duration;

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Debug)]
pub struct SerialPortInfo
{
    pub port_name: String,
    pub port_type: String,
}

pub type ConsoleResult = McpToolResult;

#[tauri::command]
pub fn network_list_serial_ports() -> Result<ConsoleResult, String>
{
    let ports = serialport::available_ports().map_err(|e| e.to_string())?;

    if ports.is_empty()
    {
        return Ok(ConsoleResult {
            success: true,
            output: "No serial ports found.".to_string(),
        });
    }

    let mut output = String::from("Available serial ports:\n\n");
    for p in ports
    {
        let port_type = match p.port_type
        {
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
mod tests
{
    use super::*;

    #[test]
    fn test_serial_port_info_serialization()
    {
        let info = SerialPortInfo {
            port_name: "COM1".to_string(),
            port_type: "USB".to_string(),
        };
        let serialized = serde_json::to_string(&info).unwrap();
        assert!(serialized.contains(r#""port_name":"COM1""#));
        assert!(serialized.contains(r#""port_type":"USB""#));
    }

    #[test]
    fn test_console_result_serialization()
    {
        let result = ConsoleResult {
            success: true,
            output: "console output".to_string(),
        };
        let serialized = serde_json::to_string(&result).unwrap();
        assert_eq!(serialized, r#"{"success":true,"output":"console output"}"#);
    }
}

#[tauri::command]
pub async fn network_send_console_message(
    port: String,
    baud_rate: Option<u32>,
    message: String,
    timeout_ms: Option<u64>,
) -> Result<ConsoleResult, String>
{
    let baud = baud_rate.unwrap_or(9600);
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(1000));

    let mut serial_port = serialport::new(&port, baud)
        .timeout(timeout)
        .open()
        .map_err(|e| format!("Failed to open port {}: {}", port, e))?;

    // Send message (add newline if not present)
    let mut msg = message.clone();
    if !msg.ends_with('\n') && !msg.ends_with('\r')
    {
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
    loop
    {
        match serial_port.read(&mut buffer)
        {
            Ok(bytes_read) if bytes_read > 0 =>
            {
                output.push_str(&String::from_utf8_lossy(&buffer[..bytes_read]));
                // If we read a full buffer, there might be more, but let's limit for now
                if bytes_read < 4096
                {
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
