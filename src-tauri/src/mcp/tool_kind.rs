use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    SelfNetworkPing,
    SelfNetworkTraceroute,
    SelfNetworkTestConnection,
    #[serde(rename = "self_network_test_net_connection")]
    SelfNetworkTestNetConnection,
    NetworkGetHosts,
    NetworkQueryNwDb,
    QueryNwDb,
    QueryRag,
    QueryNetworkGraph,
    SelfNetworkArp,
    SelfNetworkRoute,
    NetworkGetIpInfo,
    NetworkListSerialPorts,
    NetworkSendConsoleMessage,
    NetworkPacketAnalyze,
    NetworkPacketPrepare,
    NetworkPacketSafety,
    NetworkShow,
    NetworkConfig,
    FetchConfig,
    FetchRouting,
    FetchArp,
    GetState,
    RequireHostRegistered,
    SelfNetworkNwdiag,
    ValidateCiscoConfig,
    ConvertCiscoConfig,
    GetOperationPlan,
    AskUserChoice,
    AskInterfaceChoice,
    AskIpaddressChoice,
    NetworkFtpDownload,
    NetworkFtpUpload,
    NetworkTftpDownload,
    NetworkTftpUpload,
}

impl ToolKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SelfNetworkPing => "self_network_ping",
            Self::SelfNetworkTraceroute => "self_network_traceroute",
            Self::SelfNetworkTestConnection => "self_network_test_connection",
            Self::SelfNetworkTestNetConnection => "self_network_test_net_connection",
            Self::NetworkGetHosts => "network_get_hosts",
            Self::NetworkQueryNwDb => "network_query_nw_db",
            Self::QueryNwDb => "query_nw_db",
            Self::QueryRag => "query_rag",
            Self::QueryNetworkGraph => "query_network_graph",
            Self::SelfNetworkArp => "self_network_arp",
            Self::SelfNetworkRoute => "self_network_route",
            Self::NetworkGetIpInfo => "network_get_ip_info",
            Self::NetworkListSerialPorts => "network_list_serial_ports",
            Self::NetworkSendConsoleMessage => "network_send_console_message",
            Self::NetworkPacketAnalyze => "network_packet_analyze",
            Self::NetworkPacketPrepare => "network_packet_prepare",
            Self::NetworkPacketSafety => "network_packet_safety",
            Self::NetworkShow => "network_show",
            Self::NetworkConfig => "network_config",
            Self::FetchConfig => "fetch_config",
            Self::FetchRouting => "fetch_routing",
            Self::FetchArp => "fetch_arp",
            Self::GetState => "get_state",
            Self::RequireHostRegistered => "require_host_registered",
            Self::SelfNetworkNwdiag => "self_network_nwdiag",
            Self::ValidateCiscoConfig => "validate_cisco_config",
            Self::ConvertCiscoConfig => "convert_cisco_config",
            Self::GetOperationPlan => "get_operation_plan",
            Self::AskUserChoice => "ask_user_choice",
            Self::AskInterfaceChoice => "ask_interface_choice",
            Self::AskIpaddressChoice => "ask_ipaddress_choice",
            Self::NetworkFtpDownload => "network_ftp_download",
            Self::NetworkFtpUpload => "network_ftp_upload",
            Self::NetworkTftpDownload => "network_tftp_download",
            Self::NetworkTftpUpload => "network_tftp_upload",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::SelfNetworkPing => "Ping",
            Self::SelfNetworkTraceroute => "Traceroute",
            Self::SelfNetworkTestConnection | Self::SelfNetworkTestNetConnection => {
                "Test Connection"
            }
            Self::NetworkGetHosts => "Host List",
            Self::NetworkQueryNwDb | Self::QueryNwDb | Self::QueryRag => "NWDB検索",
            Self::QueryNetworkGraph => "ネットワークグラフ検索",
            Self::SelfNetworkArp => "ARP Table",
            Self::SelfNetworkRoute => "Route Table",
            Self::NetworkGetIpInfo => "IP Info",
            Self::NetworkListSerialPorts => "Serial Ports",
            Self::NetworkSendConsoleMessage => "Console Message",
            Self::NetworkPacketAnalyze => "Packet Analysis",
            Self::NetworkPacketPrepare => "DHCP Packet Preview",
            Self::NetworkPacketSafety => "Packet Safety Worker",
            Self::NetworkShow => "Show Command",
            Self::NetworkConfig => "Config Command",
            Self::FetchConfig => "Fetch Config",
            Self::FetchRouting => "Fetch Routing",
            Self::FetchArp => "Fetch ARP",
            Self::GetState => "State取得",
            Self::RequireHostRegistered => "ホスト登録要求",
            Self::SelfNetworkNwdiag => "ネットワーク図生成",
            Self::ValidateCiscoConfig => "Cisco設定検証",
            Self::ConvertCiscoConfig => "Cisco設定変換",
            Self::GetOperationPlan => "変更計画照会",
            Self::AskUserChoice => "ユーザ選択",
            Self::AskInterfaceChoice => "インターフェース選択",
            Self::AskIpaddressChoice => "IPアドレス選択",
            Self::NetworkFtpDownload => "FTPダウンロード",
            Self::NetworkFtpUpload => "FTPアップロード",
            Self::NetworkTftpDownload => "TFTPダウンロード",
            Self::NetworkTftpUpload => "TFTPアップロード",
        }
    }

    pub fn is_choice_tool(&self) -> bool {
        matches!(
            self,
            Self::AskUserChoice | Self::AskInterfaceChoice | Self::AskIpaddressChoice
        )
    }

    pub fn is_builder_tool(&self) -> bool {
        matches!(
            self,
            Self::AskUserChoice
                | Self::AskInterfaceChoice
                | Self::AskIpaddressChoice
                | Self::ValidateCiscoConfig
                | Self::ConvertCiscoConfig
                | Self::GetOperationPlan
                | Self::SelfNetworkNwdiag
        )
    }

    pub fn is_rag_tool(&self) -> bool {
        matches!(
            self,
            Self::QueryNwDb | Self::NetworkQueryNwDb | Self::QueryRag
        )
    }

    pub fn is_device_target_tool(&self) -> bool {
        matches!(
            self,
            Self::GetState | Self::FetchConfig | Self::FetchRouting | Self::FetchArp
        )
    }

    pub fn is_host_target_tool(&self) -> bool {
        matches!(
            self,
            Self::SelfNetworkPing
                | Self::SelfNetworkTraceroute
                | Self::SelfNetworkTestConnection
                | Self::SelfNetworkTestNetConnection
                | Self::NetworkFtpDownload
                | Self::NetworkFtpUpload
                | Self::NetworkTftpDownload
                | Self::NetworkTftpUpload
        )
    }

    pub fn is_heavy_network_tool(&self) -> bool {
        matches!(
            self,
            Self::GetState
                | Self::FetchConfig
                | Self::FetchRouting
                | Self::FetchArp
                | Self::NetworkShow
        )
    }

    /// Tools in this set may inspect the local system or network devices but
    /// must not change a device, transfer destination, or serial-console state.
    pub fn is_read_only(&self) -> bool {
        matches!(
            self,
            Self::SelfNetworkPing
                | Self::SelfNetworkTraceroute
                | Self::SelfNetworkTestConnection
                | Self::SelfNetworkTestNetConnection
                | Self::NetworkGetHosts
                | Self::NetworkQueryNwDb
                | Self::QueryNwDb
                | Self::QueryRag
                | Self::QueryNetworkGraph
                | Self::SelfNetworkArp
                | Self::SelfNetworkRoute
                | Self::NetworkGetIpInfo
                | Self::NetworkListSerialPorts
                | Self::NetworkPacketAnalyze
                | Self::NetworkPacketPrepare
                | Self::NetworkPacketSafety
                | Self::NetworkShow
                | Self::GetState
                | Self::FetchConfig
                | Self::FetchRouting
                | Self::FetchArp
                | Self::RequireHostRegistered
                | Self::SelfNetworkNwdiag
                | Self::ValidateCiscoConfig
                | Self::ConvertCiscoConfig
                | Self::AskUserChoice
                | Self::AskInterfaceChoice
                | Self::AskIpaddressChoice
        )
    }
}

impl std::str::FromStr for ToolKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "self_network_ping" => Ok(Self::SelfNetworkPing),
            "self_network_traceroute" => Ok(Self::SelfNetworkTraceroute),
            "self_network_test_connection" => Ok(Self::SelfNetworkTestConnection),
            "self_network_test_net_connection" => Ok(Self::SelfNetworkTestNetConnection),
            "network_get_hosts" => Ok(Self::NetworkGetHosts),
            "network_query_nw_db" => Ok(Self::NetworkQueryNwDb),
            "query_nw_db" => Ok(Self::QueryNwDb),
            "query_rag" => Ok(Self::QueryRag),
            "query_network_graph" => Ok(Self::QueryNetworkGraph),
            "self_network_arp" => Ok(Self::SelfNetworkArp),
            "self_network_route" => Ok(Self::SelfNetworkRoute),
            "network_get_ip_info" => Ok(Self::NetworkGetIpInfo),
            "network_list_serial_ports" => Ok(Self::NetworkListSerialPorts),
            "network_send_console_message" => Ok(Self::NetworkSendConsoleMessage),
            "network_packet_analyze" => Ok(Self::NetworkPacketAnalyze),
            "network_packet_prepare" => Ok(Self::NetworkPacketPrepare),
            "network_packet_safety" => Ok(Self::NetworkPacketSafety),
            "network_show" => Ok(Self::NetworkShow),
            "network_config" => Ok(Self::NetworkConfig),
            "fetch_config" => Ok(Self::FetchConfig),
            "fetch_routing" => Ok(Self::FetchRouting),
            "fetch_arp" => Ok(Self::FetchArp),
            "get_state" => Ok(Self::GetState),
            "require_host_registered" => Ok(Self::RequireHostRegistered),
            "self_network_nwdiag" => Ok(Self::SelfNetworkNwdiag),
            "validate_cisco_config" => Ok(Self::ValidateCiscoConfig),
            "convert_cisco_config" => Ok(Self::ConvertCiscoConfig),
            "get_operation_plan" => Ok(Self::GetOperationPlan),
            "ask_user_choice" => Ok(Self::AskUserChoice),
            "ask_interface_choice" => Ok(Self::AskInterfaceChoice),
            "ask_ipaddress_choice" => Ok(Self::AskIpaddressChoice),
            "network_ftp_download" => Ok(Self::NetworkFtpDownload),
            "network_ftp_upload" => Ok(Self::NetworkFtpUpload),
            "network_tftp_download" => Ok(Self::NetworkTftpDownload),
            "network_tftp_upload" => Ok(Self::NetworkTftpUpload),
            _ => Err(format!("Unknown tool: {}", s)),
        }
    }
}

impl std::fmt::Display for ToolKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
