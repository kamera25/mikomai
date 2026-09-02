use super::extract::*;
use super::registry::McpTool;
use crate::network::CommandResult;
use std::collections::HashMap;
use tauri::Manager;

macro_rules! define_tool {
    ($struct_name:ident, $tool_name:expr, |$app:ident, $args:ident| $body:expr) => {
        pub struct $struct_name;
        impl McpTool for $struct_name {
            fn name(&self) -> &'static str {
                $tool_name
            }
            fn execute(
                &self,
                $app: tauri::AppHandle,
                $args: serde_json::Value,
            ) -> futures::future::BoxFuture<'static, Result<crate::network::CommandResult, String>>
            {
                Box::pin(async move { $body })
            }
        }
    };
}

define_tool!(PingTool, "self_network_ping", |app, args| {
    let host = get_str_arg(&args, &["host"]);
    let device = get_str_arg(&args, &["device"]);
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let ip = get_ip_arg(&args, &["ip"]);
    let size = get_usize_arg(&args, &["size"]);
    let count = get_u32_arg(&args, &["count"]);
    let df = get_bool_arg(&args, &["df"]);
    crate::mcp::ping::self_network_ping_with_params(
        app,
        crate::mcp::ping::PingParams {
            host,
            device,
            device_name,
            ip,
            size,
            count,
            df,
        },
    )
    .await
    .map(Into::into)
});

define_tool!(TracerouteTool, "self_network_traceroute", |app, args| {
    let host = get_str_arg(&args, &["host"]);
    let device = get_str_arg(&args, &["device", "deviceName", "device_name"]);
    let ip = get_ip_arg(&args, &["ip"]);
    crate::mcp::traceroute::self_network_traceroute_with_params(
        app,
        crate::mcp::traceroute::TracerouteParams { host, device, ip },
    )
    .await
    .map(Into::into)
});

define_tool!(
    TestConnectionTool,
    "self_network_test_connection",
    |app, args| {
        let host = get_str_arg(&args, &["host", "target"]);
        let device = get_str_arg(&args, &["device", "deviceName", "device_name"]);
        let ip = get_ip_arg(&args, &["ip"]);
        let computer_name = get_str_arg(&args, &["computer_name", "computerName"]);
        let port = get_u32_arg(&args, &["port", "remote_port", "remotePort"])
            .and_then(|p| crate::connections::Port::try_from(p).ok());
        let common_tcp_port = get_str_arg(&args, &["common_tcp_port", "commonTcpPort", "service"]);
        let timeout_ms = args
            .get("timeout_ms")
            .or(args.get("timeoutMs"))
            .and_then(|v| v.as_u64());

        crate::mcp::test_connection::self_network_test_connection_with_params(
            app.clone(),
            crate::mcp::test_connection::TestConnectionParams {
                host,
                device,
                ip,
                computer_name,
                port,
                common_tcp_port,
                timeout_ms,
            },
        )
        .await
        .map(Into::into)
    }
);

define_tool!(FetchConfigTool, "fetch_config", |app, args| {
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let device = get_str_arg(&args, &["device"]);
    let host = get_str_arg(&args, &["host"]);
    let user_msg = get_str_arg(&args, &["userMessage", "user_message"]);
    crate::mcp::fetch::fetch_config::fetch_config(
        app,
        device_name.clone(),
        device_name,
        device,
        host,
        user_msg.clone(),
        user_msg,
    )
    .await
});

define_tool!(FetchRoutingTool, "fetch_routing", |app, args| {
    let llama_state = app.state::<crate::llm::llm::LlamaState>();
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let device = get_str_arg(&args, &["device"]);
    let host = get_str_arg(&args, &["host"]);
    let user_msg = get_str_arg(&args, &["userMessage", "user_message"]);
    crate::mcp::fetch::fetch_routing::fetch_routing(
        app.clone(),
        llama_state,
        device_name.clone(),
        device_name,
        device,
        host,
        user_msg.clone(),
        user_msg,
    )
    .await
});

define_tool!(FetchArpTool, "fetch_arp", |app, args| {
    let llama_state = app.state::<crate::llm::llm::LlamaState>();
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let device = get_str_arg(&args, &["device"]);
    let host = get_str_arg(&args, &["host"]);
    let user_msg = get_str_arg(&args, &["userMessage", "user_message"]);
    crate::mcp::fetch::fetch_arp::fetch_arp(
        app.clone(),
        llama_state,
        device_name.clone(),
        device_name,
        device,
        host,
        user_msg.clone(),
        user_msg,
    )
    .await
});

define_tool!(GetStateTool, "get_state", |app, args| {
    let device = get_str_arg(
        &args,
        &["device", "deviceName", "device_name", "host", "target"],
    );
    let resource = get_str_arg(
        &args,
        &["resource", "resourceType", "resource_type", "type"],
    );
    let user_msg = get_str_arg(&args, &["userMessage", "user_message"]);
    crate::mcp::fetch::get_state::get_state(
        app,
        device.clone(),
        device.clone(),
        device.clone(),
        device.clone(),
        device,
        resource.clone(),
        resource.clone(),
        resource,
        user_msg.clone(),
        user_msg,
    )
    .await
});

define_tool!(QueryNwDbTool, "query_nw_db", |app, args| {
    let query = get_str_arg(&args, &["query", "userMessage", "user_message"]).unwrap_or_default();
    let filter = get_str_arg(&args, &["filter"]);
    let rag_state = app.state::<crate::mcp::rag::RagState>();
    crate::mcp::rag::query_nw_db(query, filter, rag_state, app.clone())
        .await
        .map(Into::into)
});

define_tool!(QueryNetworkGraphTool, "query_network_graph", |app, args| {
    let query = get_str_arg(&args, &["query", "userMessage", "user_message"]).unwrap_or_default();
    let device_name = get_str_arg(&args, &["deviceName", "device_name", "device"]);
    let ip_address = get_str_arg(&args, &["ipAddress", "ip_address", "ip"]);
    let vlan = get_u32_arg(&args, &["vlan"]);
    let acl = get_str_arg(&args, &["acl"]);
    let state = app.state::<crate::graph::SurrealDbState>();
    crate::graph::query_network_graph(
        query,
        device_name.clone(),
        device_name,
        ip_address.clone(),
        ip_address,
        vlan,
        acl,
        app.clone(),
        state,
    )
    .await
    .map(|output| CommandResult {
        success: true,
        output,
        saved_path: None,
        is_cached: None,
        cache_time: None,
    })
});

define_tool!(SelfNetworkArpTool, "self_network_arp", |app, _args| {
    crate::mcp::arp::self_network_arp(app).await.map(Into::into)
});

define_tool!(SelfNetworkRouteTool, "self_network_route", |app, _args| {
    crate::mcp::route::self_network_route(app)
        .await
        .map(Into::into)
});

define_tool!(NetworkGetHostsTool, "network_get_hosts", |app, _args| {
    crate::mcp::hosts::network_get_hosts(app)
        .await
        .map(Into::into)
});

define_tool!(
    RequireHostRegisteredTool,
    "require_host_registered",
    |_app, _args| crate::mcp::hosts::require_host_registered().map(Into::into)
);

define_tool!(NetworkGetIpInfoTool, "network_get_ip_info", |_app, args| {
    let verbose = get_bool_arg(&args, &["verbose"]);
    crate::mcp::ip_info::network_get_ip_info(verbose)
        .await
        .map(Into::into)
});

define_tool!(
    NetworkListSerialPortsTool,
    "network_list_serial_ports",
    |_app, _args| crate::mcp::console::network_list_serial_ports().map(Into::into)
);

define_tool!(
    NetworkSendConsoleMessageTool,
    "network_send_console_message",
    |_app, args| {
        let port = get_str_arg(&args, &["port"]).unwrap_or_default();
        let baud_rate = get_u32_arg(&args, &["baud_rate", "baudRate"]);
        let message = get_str_arg(&args, &["message"]).unwrap_or_default();
        let timeout_ms = args
            .get("timeout_ms")
            .or(args.get("timeoutMs"))
            .and_then(|v| v.as_u64());
        crate::mcp::console::network_send_console_message(port, baud_rate, message, timeout_ms)
            .await
            .map(Into::into)
    }
);

fn resolve_device_config_from_args(
    app: &tauri::AppHandle,
    args: &serde_json::Value,
) -> Result<crate::network::NetmikoDeviceConfig, String> {
    // 1. Direct device object in args.device
    if let Some(device_val) = args.get("device") {
        if device_val.is_object() {
            if let Ok(config) =
                serde_json::from_value::<crate::network::NetmikoDeviceConfig>(device_val.clone())
            {
                if !config.host.trim().is_empty() {
                    return Ok(config);
                }
            }
        }
    }

    // 2. Direct device config in root args
    if let Ok(config) = serde_json::from_value::<crate::network::NetmikoDeviceConfig>(args.clone())
    {
        if !config.host.trim().is_empty() {
            return Ok(config);
        }
    }

    // 3. Resolve by device name / target / host string
    let device_name = get_str_arg(
        args,
        &["target", "deviceName", "device_name", "device", "host"],
    );
    let user_msg = get_str_arg(args, &["userMessage", "user_message"]);

    let resolved_name = crate::mcp::args::normalize_device_args(
        app,
        device_name.clone(),
        device_name.clone(),
        device_name.clone(),
        device_name,
        user_msg.clone(),
        user_msg,
    )?;

    if let Some((ip, user, password, enable_password, dtype)) =
        crate::connections::get_device_config(app, &resolved_name)
    {
        Ok(crate::network::NetmikoDeviceConfig {
            host: ip,
            username: user,
            password,
            enable_password,
            device_type: dtype,
            console_port: None,
            console_baud_rate: None,
            auth_method: None,
            private_key_path: None,
            passphrase: None,
            agent_forwarding: None,
        })
    } else {
        Ok(crate::network::NetmikoDeviceConfig {
            host: resolved_name,
            username: "admin".to_string(),
            password: None,
            enable_password: None,
            device_type: "cisco_ios".to_string(),
            console_port: None,
            console_baud_rate: None,
            auth_method: None,
            private_key_path: None,
            passphrase: None,
            agent_forwarding: None,
        })
    }
}

define_tool!(NetworkShowTool, "network_show", |app, args| {
    let device = resolve_device_config_from_args(&app, &args)?;
    let command = get_str_arg(&args, &["command"]).unwrap_or_default();
    // Route ARP and routing-table requests through the read-through graph
    // interface even when the Agent selected a generic show command.
    if is_arp_show_command(&command) {
        let llama_state = app.state::<crate::llm::llm::LlamaState>();
        return crate::mcp::fetch::fetch_arp::fetch_arp(
            app.clone(),
            llama_state,
            None,
            None,
            Some(device.host),
            None,
            None,
            None,
        )
        .await;
    }
    if is_route_show_command(&command) {
        let llama_state = app.state::<crate::llm::llm::LlamaState>();
        return crate::mcp::fetch::fetch_routing::fetch_routing(
            app.clone(),
            llama_state,
            None,
            None,
            Some(device.host),
            None,
            None,
            None,
        )
        .await;
    }
    crate::network::network_show(app, device, command)
        .await
        .map_err(|e| e.to_string())
});

fn is_arp_show_command(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    normalized.starts_with("show ") && normalized.split_whitespace().any(|word| word == "arp")
}

fn is_route_show_command(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    normalized.starts_with("show ")
        && normalized
            .split_whitespace()
            .any(|word| word == "route" || word == "routing")
}

define_tool!(NetworkConfigTool, "network_config", |app, args| {
    let device = resolve_device_config_from_args(&app, &args)?;
    let commands: Vec<String> = if let Some(cmds_val) = args.get("commands") {
        serde_json::from_value::<Vec<String>>(cmds_val.clone()).unwrap_or_default()
    } else if let Some(cmd) = get_str_arg(&args, &["command"]) {
        vec![cmd]
    } else {
        Vec::new()
    };

    crate::network::network_config(app, device, commands)
        .await
        .map_err(|e| e.to_string())
});

define_tool!(NwDiagTool, "self_network_nwdiag", |app, args| {
    let schema = get_str_arg(&args, &["schema"]).unwrap_or_default();
    crate::mcp::nwdiag::self_network_nwdiag(app, schema).await
});

define_tool!(GetOperationPlanTool, "get_operation_plan", |app, args| {
    let id = get_str_arg(&args, &["id", "plan_id", "planId"]).ok_or("plan id is required")?;
    let id = uuid::Uuid::parse_str(&id).map_err(|_| "invalid plan id")?;
    let plan = app.state::<crate::operations::OperationStore>().get(id)?;
    Ok(CommandResult {
        success: true,
        output: serde_json::to_string_pretty(&plan)
            .map_err(|error| format!("failed to serialize change plan: {error}"))?,
        saved_path: None,
        is_cached: None,
        cache_time: None,
    })
});

define_tool!(
    ValidateCiscoConfigTool,
    "validate_cisco_config",
    |app, args| {
        let id: Option<String> = args
            .get("id")
            .and_then(|v| v.as_str().map(|s| s.to_string()));
        let config: String = args
            .get("config")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .ok_or("config is required")?;
        let target = get_str_arg(&args, &["device_name", "deviceName", "target", "host"]);
        let target_device = target.map(|name| (name.clone(), name));
        crate::mcp::config_helper::validate_cisco_config_impl(Some(app), id, config, target_device)
            .await
    }
);

define_tool!(
    ConvertCiscoConfigTool,
    "convert_cisco_config",
    |_app, args| {
        let config = get_str_arg(&args, &["config"]).unwrap_or_default();
        let target_vendor =
            get_str_arg(&args, &["target_vendor", "targetVendor"]).unwrap_or_default();
        crate::mcp::config_helper::convert_cisco_config(config, target_vendor).await
    }
);

define_tool!(AskUserChoiceTool, "ask_user_choice", |app, args| {
    let id = get_str_arg(&args, &["task_id"]);
    let title = get_str_arg(&args, &["title"]).unwrap_or_default();
    let message = get_str_arg(&args, &["message"]).unwrap_or_default();

    let options: Vec<String> = if let Some(opt_val) = args.get("options") {
        if let Some(arr) = opt_val.as_array() {
            arr.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        } else {
            Vec::new()
        }
    } else {
        Vec::new()
    };

    match crate::mcp::config_helper::ask_user_choice(app.clone(), id, title, message, options).await
    {
        Ok(res) => Ok(CommandResult {
            success: true,
            output: res,
            saved_path: None,
            is_cached: None,
            cache_time: None,
        }),
        Err(e) => Err(e),
    }
});

define_tool!(
    AskInterfaceChoiceTool,
    "ask_interface_choice",
    |app, args| {
        let id = get_str_arg(&args, &["task_id"]);
        let vendor = get_str_arg(&args, &["vendor"]).unwrap_or_default();
        let message = get_str_arg(&args, &["message"]);

        match crate::mcp::config_helper::ask_interface_choice(app.clone(), id, vendor, message)
            .await
        {
            Ok(res) => Ok(CommandResult {
                success: true,
                output: res,
                saved_path: None,
                is_cached: None,
                cache_time: None,
            }),
            Err(e) => Err(e),
        }
    }
);

define_tool!(
    AskIpAddressChoiceTool,
    "ask_ipaddress_choice",
    |app, args| {
        let id = get_str_arg(&args, &["task_id"]);
        let title = get_str_arg(&args, &["title"]).unwrap_or_default();
        let message = get_str_arg(&args, &["message"]).unwrap_or_default();
        let subnet = get_str_arg(&args, &["subnet"]).unwrap_or_default();
        let ip_address = get_str_arg(&args, &["ip_address", "ipAddress"]);

        match crate::mcp::config_helper::ask_ipaddress_choice(
            app.clone(),
            id,
            title,
            message,
            subnet,
            ip_address,
        )
        .await
        {
            Ok(res) => Ok(CommandResult {
                success: true,
                output: res,
                saved_path: None,
                is_cached: None,
                cache_time: None,
            }),
            Err(e) => Err(e),
        }
    }
);

define_tool!(FtpDownloadTool, "network_ftp_download", |app, args| {
    let host = get_str_arg(&args, &["host"]);
    let device = get_str_arg(&args, &["device"]);
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let ip = get_ip_arg(&args, &["ip"]);
    let port =
        get_u32_arg(&args, &["port"]).and_then(|p| crate::connections::Port::try_from(p).ok());
    let username = get_str_arg(&args, &["username", "user"]);
    let password = get_str_arg(&args, &["password", "pass"]);
    let remote_file = get_str_arg(&args, &["remote_file", "remoteFile"]);
    let filename = get_str_arg(&args, &["filename", "file"]);
    let local_path = get_str_arg(&args, &["local_path", "localPath"]);
    let timeout_secs =
        get_u32_arg(&args, &["timeout_secs", "timeoutSecs", "timeout"]).map(|t| t as u64);

    crate::mcp::ftp::network_ftp_download_with_params(
        app,
        crate::mcp::ftp::FtpDownloadParams {
            host,
            device,
            device_name,
            ip,
            port,
            username,
            password,
            remote_file,
            filename,
            local_path,
            timeout_secs,
        },
    )
    .await
    .map(Into::into)
});

define_tool!(FtpUploadTool, "network_ftp_upload", |app, args| {
    let host = get_str_arg(&args, &["host"]);
    let device = get_str_arg(&args, &["device"]);
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let ip = get_ip_arg(&args, &["ip"]);
    let port =
        get_u32_arg(&args, &["port"]).and_then(|p| crate::connections::Port::try_from(p).ok());
    let username = get_str_arg(&args, &["username", "user"]);
    let password = get_str_arg(&args, &["password", "pass"]);
    let local_file = get_str_arg(&args, &["local_file", "localFile"]);
    let remote_file = get_str_arg(&args, &["remote_file", "remoteFile"]);
    let filename = get_str_arg(&args, &["filename", "file"]);
    let content = get_str_arg(&args, &["content"]);
    let timeout_secs =
        get_u32_arg(&args, &["timeout_secs", "timeoutSecs", "timeout"]).map(|t| t as u64);

    crate::mcp::ftp::network_ftp_upload_with_params(
        app,
        crate::mcp::ftp::FtpUploadParams {
            host,
            device,
            device_name,
            ip,
            port,
            username,
            password,
            local_file,
            remote_file,
            filename,
            content,
            timeout_secs,
        },
    )
    .await
    .map(Into::into)
});

define_tool!(TftpDownloadTool, "network_tftp_download", |app, args| {
    let host = get_str_arg(&args, &["host"]);
    let device = get_str_arg(&args, &["device"]);
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let ip = get_ip_arg(&args, &["ip"]);
    let port =
        get_u32_arg(&args, &["port"]).and_then(|p| crate::connections::Port::try_from(p).ok());
    let remote_file = get_str_arg(&args, &["remote_file", "remoteFile"]);
    let filename = get_str_arg(&args, &["filename", "file"]);
    let local_path = get_str_arg(&args, &["local_path", "localPath"]);
    let mode = get_str_arg(&args, &["mode"]);
    let timeout_secs =
        get_u32_arg(&args, &["timeout_secs", "timeoutSecs", "timeout"]).map(|t| t as u64);

    crate::mcp::tftp::network_tftp_download_with_params(
        app,
        crate::mcp::tftp::TftpDownloadParams {
            host,
            device,
            device_name,
            ip,
            port,
            remote_file,
            filename,
            local_path,
            mode,
            timeout_secs,
        },
    )
    .await
    .map(Into::into)
});

define_tool!(TftpUploadTool, "network_tftp_upload", |app, args| {
    let host = get_str_arg(&args, &["host"]);
    let device = get_str_arg(&args, &["device"]);
    let device_name = get_str_arg(&args, &["deviceName", "device_name"]);
    let ip = get_ip_arg(&args, &["ip"]);
    let port =
        get_u32_arg(&args, &["port"]).and_then(|p| crate::connections::Port::try_from(p).ok());
    let local_file = get_str_arg(&args, &["local_file", "localFile"]);
    let remote_file = get_str_arg(&args, &["remote_file", "remoteFile"]);
    let filename = get_str_arg(&args, &["filename", "file"]);
    let content = get_str_arg(&args, &["content"]);
    let mode = get_str_arg(&args, &["mode"]);
    let timeout_secs =
        get_u32_arg(&args, &["timeout_secs", "timeoutSecs", "timeout"]).map(|t| t as u64);

    crate::mcp::tftp::network_tftp_upload_with_params(
        app,
        crate::mcp::tftp::TftpUploadParams {
            host,
            device,
            device_name,
            ip,
            port,
            local_file,
            remote_file,
            filename,
            content,
            mode,
            timeout_secs,
        },
    )
    .await
    .map(Into::into)
});

// Delegate alias tool to avoid duplicate implementations
struct DelegatingAliasTool {
    name: &'static str,
    target_tool_name: &'static str,
}

impl McpTool for DelegatingAliasTool {
    fn name(&self) -> &'static str {
        self.name
    }
    fn execute(
        &self,
        app: tauri::AppHandle,
        args: serde_json::Value,
    ) -> futures::future::BoxFuture<'static, Result<CommandResult, String>> {
        let target = self.target_tool_name;
        Box::pin(async move {
            let registry = super::registry::get_tool_registry();
            if let Some(tool) = registry.get(target) {
                tool.execute(app, args).await
            } else {
                Err(format!("Alias target tool not found: {}", target))
            }
        })
    }
}

pub fn init_tool_registry() -> HashMap<String, Box<dyn McpTool>> {
    let mut registry: HashMap<String, Box<dyn McpTool>> = HashMap::new();

    // Macro helper to register unique tools
    macro_rules! reg {
        ($tool:expr) => {
            let t = $tool;
            registry.insert(t.name().to_string(), Box::new(t));
        };
    }

    reg!(PingTool);
    reg!(TracerouteTool);
    reg!(TestConnectionTool);
    reg!(FetchConfigTool);
    reg!(FetchRoutingTool);
    reg!(FetchArpTool);
    reg!(GetStateTool);
    reg!(QueryNwDbTool);
    reg!(QueryNetworkGraphTool);
    reg!(SelfNetworkArpTool);
    reg!(SelfNetworkRouteTool);
    reg!(NetworkGetHostsTool);
    reg!(RequireHostRegisteredTool);
    reg!(NetworkGetIpInfoTool);
    reg!(NetworkListSerialPortsTool);
    reg!(NetworkSendConsoleMessageTool);
    reg!(NetworkShowTool);
    reg!(NetworkConfigTool);
    reg!(NwDiagTool);
    reg!(GetOperationPlanTool);
    reg!(ValidateCiscoConfigTool);
    reg!(ConvertCiscoConfigTool);
    reg!(AskUserChoiceTool);
    reg!(AskInterfaceChoiceTool);
    reg!(AskIpAddressChoiceTool);
    reg!(FtpDownloadTool);
    reg!(FtpUploadTool);
    reg!(TftpDownloadTool);
    reg!(TftpUploadTool);

    // Aliases (Eliminates duplication)
    macro_rules! alias {
        ($alias_name:expr, $target_name:expr) => {
            registry.insert(
                $alias_name.to_string(),
                Box::new(DelegatingAliasTool {
                    name: $alias_name,
                    target_tool_name: $target_name,
                }),
            );
        };
    }

    alias!(
        "self_network_test_net_connection",
        "self_network_test_connection"
    );
    alias!("network_query_nw_db", "query_nw_db");
    alias!("query_rag", "query_nw_db");

    registry
}
