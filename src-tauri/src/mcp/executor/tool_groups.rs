use std::collections::HashMap;
use super::registry::McpTool;

fn add<T: McpTool + 'static>(registry: &mut HashMap<String, Box<dyn McpTool>>, tool: T) {
    registry.insert(tool.name().to_string(), Box::new(tool));
}

pub fn register_read_tools(registry: &mut HashMap<String, Box<dyn McpTool>>) {
    add(registry, super::tools::PingTool);
    add(registry, super::tools::TracerouteTool);
    add(registry, super::tools::TestConnectionTool);
    add(registry, super::tools::FetchConfigTool);
    add(registry, super::tools::FetchRoutingTool);
    add(registry, super::tools::FetchArpTool);
    add(registry, super::tools::GetStateTool);
    add(registry, super::tools::QueryNwDbTool);
    add(registry, super::tools::QueryNetworkGraphTool);
    add(registry, super::tools::SelfNetworkArpTool);
    add(registry, super::tools::SelfNetworkRouteTool);
    add(registry, super::tools::NetworkGetHostsTool);
    add(registry, super::tools::RequireHostRegisteredTool);
    add(registry, super::tools::NetworkGetIpInfoTool);
    add(registry, super::tools::NetworkListSerialPortsTool);
    add(registry, super::tools::NetworkShowTool);
}

pub fn register_change_tools(registry: &mut HashMap<String, Box<dyn McpTool>>) {
    add(registry, super::tools::NetworkSendConsoleMessageTool);
    add(registry, super::tools::NetworkPacketAnalyzeTool);
    add(registry, super::tools::NetworkPacketPrepareTool);
    add(registry, super::tools::NetworkPacketSafetyTool);
    add(registry, super::tools::NetworkConfigTool);
    add(registry, super::tools::NwDiagTool);
    add(registry, super::tools::GetOperationPlanTool);
    add(registry, super::tools::ValidateCiscoConfigTool);
    add(registry, super::tools::ConvertCiscoConfigTool);
    add(registry, super::tools::AskUserChoiceTool);
    add(registry, super::tools::AskInterfaceChoiceTool);
    add(registry, super::tools::AskIpAddressChoiceTool);
}

pub fn register_file_transfer_tools(registry: &mut HashMap<String, Box<dyn McpTool>>) {
    add(registry, super::tools::FtpDownloadTool);
    add(registry, super::tools::FtpUploadTool);
    add(registry, super::tools::TftpDownloadTool);
    add(registry, super::tools::TftpUploadTool);
}
