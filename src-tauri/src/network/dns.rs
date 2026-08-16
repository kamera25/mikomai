use std::net::IpAddr;
use std::str::FromStr;

#[tauri::command]
pub async fn resolve_ip(ip: String) -> Result<String, String>
{
    tracing::info!("Resolving IP (using dns-lookup): {}", ip);

    // Parse the IP address
    let ip_addr = IpAddr::from_str(&ip).map_err(|e| format!("Invalid IP address: {}", e))?;

    // Perform the reverse lookup in a blocking task
    let hostname = tokio::task::spawn_blocking(move || dns_lookup::lookup_addr(&ip_addr))
        .await
        .map_err(|e| format!("Task joined failed: {}", e))?
        .map_err(|e| format!("DNS lookup failed: {}", e))?;

    if hostname.is_empty()
    {
        return Err("Not found".to_string());
    }

    Ok(hostname.trim_end_matches('.').to_string())
}
