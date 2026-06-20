import React, { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import Papa from "papaparse";
import { useTranslation } from "react-i18next";
import { ServerIcon, TrashIcon } from "./Icons";
import "./ConnectionSettingsPanel.css";
import Select from "react-select";

interface ConnectionSettingsPanelProps {
  onClose: () => void;
  onConnectionsChanged?: () => void;
}

import { Connection, McpHost } from "../types";

const customSelectStyles = {
  control: (provided: any, state: any) => ({
    ...provided,
    backgroundColor: "var(--bg-secondary, white)",
    color: "var(--text-primary, #1e293b)",
    borderColor: state.isFocused ? "var(--accent-color, #3b82f6)" : "var(--border-color, #e2e8f0)",
    borderRadius: "6px",
    minHeight: "40px",
    fontSize: "0.95rem",
    boxShadow: state.isFocused ? "0 0 0 3px rgba(59, 130, 246, 0.1)" : "none",
    "&:hover": {
      borderColor: "var(--accent-color, #3b82f6)",
    },
  }),
  menu: (provided: any) => ({
    ...provided,
    backgroundColor: "var(--bg-secondary, white)",
    border: "1px solid var(--border-color, #e2e8f0)",
    borderRadius: "6px",
    zIndex: 9999,
  }),
  option: (provided: any, state: any) => ({
    ...provided,
    backgroundColor: state.isSelected
      ? "var(--accent-color, #3b82f6)"
      : state.isFocused
        ? "var(--bg-tertiary, #f1f5f9)"
        : "transparent",
    color: state.isSelected
      ? "white"
      : "var(--text-primary, #1e293b)",
    cursor: "pointer",
    fontSize: "0.95rem",
    "&:active": {
      backgroundColor: "var(--accent-hover, #2563eb)",
    },
  }),
  singleValue: (provided: any) => ({
    ...provided,
    color: "var(--text-primary, #1e293b)",
  }),
  input: (provided: any) => ({
    ...provided,
    color: "var(--text-primary, #1e293b)",
  }),
  placeholder: (provided: any) => ({
    ...provided,
    color: "var(--text-muted, #94a3b8)",
  }),
  dropdownIndicator: (provided: any) => ({
    ...provided,
    color: "var(--text-muted, #94a3b8)",
    "&:hover": {
      color: "var(--text-secondary, #475569)",
    },
  }),
  clearIndicator: (provided: any) => ({
    ...provided,
    color: "var(--text-muted, #94a3b8)",
  }),
  indicatorSeparator: (provided: any) => ({
    ...provided,
    backgroundColor: "var(--border-color, #e2e8f0)",
  }),
};

export const DEVICE_TYPES = [
  "a10",
  "accedian",
  "adtran_os",
  "adva_fsp150f2",
  "adva_fsp150f3",
  "alaxala_ax36s",
  "alaxala_ax26s",
  "alcatel_aos",
  "alcatel_sros",
  "allied_telesis_awplus",
  "apc_aos",
  "apresia_aeos",
  "arista_eos",
  "arris_cer",
  "aruba_os",
  "aruba_aoscx",
  "aruba_osswitch",
  "aruba_procurve",
  "asterfusion_asternos",
  "audiocode_72",
  "audiocode_66",
  "audiocode_shell",
  "avaya_ers",
  "avaya_vsp",
  "avara_aos",
  "aviat_wtm",
  "bintec_boss",
  "broadcom_icos",
  "brocade_fos",
  "brocade_fastiron",
  "brocade_netiron",
  "brocade_nos",
  "brocade_vdx",
  "brocade_vyos",
  "checkpoint_gaia",
  "calix_b6",
  "calix_exa",
  "casa_cmts",
  "cdot_cros",
  "centec_os",
  "ciena_saos",
  "ciena_saos10",
  "ciena_waveserver",
  "cisco_ap",
  "cisco_apic",
  "cisco_asa",
  "cisco_ftd",
  "cisco_ios",
  "cisco_nxos",
  "cisco_s200",
  "cisco_s300",
  "cisco_s500",
  "cisco_tp",
  "cisco_viptela",
  "cisco_wlc",
  "cisco_ioswlc",
  "cisco_xe",
  "cisco_xr",
  "cloudgenix_ion",
  "corelight_linux",
  "coriant",
  "cumulus_linux",
  "dell_dnos9",
  "dell_force10",
  "dell_os6",
  "dell_os9",
  "dell_os10",
  "dell_sonic",
  "dell_powerconnect",
  "dell_isilon",
  "dlink_ds",
  "digi_transport",
  "edgecore_sonic",
  "endace",
  "ekinops_ek360",
  "eltex",
  "eltex_esr",
  "enterasys",
  "ericsson_ipos",
  "ericsson_mltn63",
  "ericsson_mltn66",
  "extreme",
  "extreme_ers",
  "extreme_exos",
  "extreme_netiron",
  "extreme_nos",
  "extreme_slx",
  "extreme_tierra",
  "extreme_vdx",
  "extreme_vsp",
  "extreme_wing",
  "f5_ltm",
  "f5_tmsh",
  "f5_linux",
  "fiberstore_fsos",
  "fiberstore_fsosv2",
  "fiberstore_networkos",
  "flexvnf",
  "fortinet",
  "fsas_sir",
  "fujitsu_sir",
  "furukawa_fitelnet",
  "garderos_grs",
  "generic",
  "generic_termserver",
  "h3c_comware",
  "hillstone_stoneos",
  "hirschmann_hios",
  "hp_comware",
  "hp_procurve",
  "huawei",
  "huawei_smartaxmmi",
  "huawei_smartax",
  "huawei_olt",
  "huawei_ont",
  "huawei_vrp",
  "huawei_vrpv8",
  "infinera_packet",
  "ipinfusion_ocnos",
  "juniper",
  "juniper_junos",
  "juniper_screenos",
  "keymile",
  "keymile_nos",
  "lancom_lcossx4",
  "lancom_lcossx5",
  "linux",
  "mikrotik_routeros",
  "mikrotik_switchos",
  "mellanox",
  "mellanox_mlnxos",
  "moxa_nos",
  "mrv_lx",
  "mrv_optiswitch",
  "nec_ix",
  "netapp_cdot",
  "netgear_prosafe",
  "netscaler",
  "nokia_isam",
  "nokia_sros",
  "nokia_srl",
  "oneaccess_oneos",
  "opengear_linux",
  "ovs_linux",
  "paloalto_panos",
  "pluribus",
  "perle_iolan",
  "quanta_mesh",
  "rad_etx",
  "raisecom_roap",
  "raisecom_ros",
  "ruckus_fastiron",
  "ruijie_os",
  "iij_seilos",
  "silverpeak_vxoa",
  "sixwind_os",
  "smartoptics_dwdm",
  "sophos_fos",
  "supermicro_smis",
  "telcosystems_binos",
  "teldat_cit",
  "tplink_jetstream",
  "ubiquiti_edge",
  "ubiquiti_edgerouter",
  "ubiquiti_edgeswitch",
  "ubiquiti_unifiswitch",
  "vertiv_mph",
  "vyatta_vyos",
  "vyos",
  "watchguard_fireware",
  "zpe_nodegrid",
  "zte_zxros",
  "yamaha",
  "zyxel_os",
  "maipu",
];

export const DEVICE_TYPE_ALIASES: { [key: string]: string } = {
  a10: "A10 Networks",
  accedian: "Accedian",
  adtran_os: "Adtran OS",
  alaxala_ax36s: "ALAXALA AX36S",
  alaxala_ax26s: "ALAXALA AX26S",
  alcatel_aos: "Alcatel-Lucent AOS",
  alcatel_sros: "Alcatel-Lucent SROS",
  allied_telesis_awplus: "Allied Telesis AlliedWare Plus",
  apc_aos: "APC AOS",
  apresia_aeos: "APRESIA AEOS",
  arista_eos: "Arista EOS",
  arris_cer: "Arris CER",
  aruba_os: "ArubaOS",
  aruba_aoscx: "ArubaOS-CX",
  aruba_osswitch: "ArubaOS-Switch",
  aruba_procurve: "Aruba ProCurve",
  cisco_ios: "Cisco IOS",
  cisco_nxos: "Cisco NX-OS",
  cisco_xe: "Cisco IOS-XE",
  cisco_xr: "Cisco IOS-XR",
  cisco_asa: "Cisco ASA",
  cisco_ftd: "Cisco FTD",
  cisco_wlc: "Cisco WLC",
  cisco_apic: "Cisco APIC",
  dell_os6: "Dell OS6",
  dell_os9: "Dell OS9",
  dell_os10: "Dell OS10",
  dell_sonic: "Dell Enterprise SONiC",
  extreme_exos: "Extreme EXOS",
  fortinet: "Fortinet FortiOS",
  h3c_comware: "H3C Comware",
  hp_comware: "HP Comware",
  hp_procurve: "HP ProCurve",
  huawei: "Huawei VRP",
  huawei_vrpv8: "Huawei VRPv8",
  iij_seilos: "IIJ SEIL/OS",
  juniper_junos: "Juniper Junos",
  linux: "Linux",
  mikrotik_routeros: "MikroTik RouterOS",
  mikrotik_switchos: "MikroTik SwitchOS",
  nec_ix: "NEC UNIVERGE IX",
  nokia_sros: "Nokia SR OS",
  nokia_srl: "Nokia SR Linux",
  paloalto_panos: "Palo Alto PAN-OS",
  ruckus_fastiron: "Ruckus FastIron",
  ruijie_os: "Ruijie RGOS",
  yamaha: "Yamaha RT",
  zte_zxros: "ZTE ZXROS",
  zyxel_os: "ZyXEL ZyNOS",
};

export const getDeviceTypeAlias = (deviceType: string): string => {
  if (!deviceType) return "";
  if (DEVICE_TYPE_ALIASES[deviceType]) {
    return DEVICE_TYPE_ALIASES[deviceType];
  }
  return deviceType
    .split("_")
    .map((word) => {
      const wLower = word.toLowerCase();
      if (
        [
          "ios",
          "eos",
          "junos",
          "nxos",
          "sros",
          "srl",
          "asa",
          "apic",
          "wlc",
          "ftd",
          "wtm",
          "cer",
          "grs",
          "vxoa",
          "dwdm",
          "solt",
          "olt",
          "ont",
          "mmi",
          "vxoa",
          "cit",
        ].includes(wLower)
      ) {
        return word.toUpperCase();
      }
      return word.charAt(0).toUpperCase() + word.slice(1);
    })
    .join(" ");
};

const deviceTypeOptions = DEVICE_TYPES.map((dt) => ({
  value: dt,
  label: `${getDeviceTypeAlias(dt)} (${dt})`,
}));

export const ConnectionSettingsPanel: React.FC<ConnectionSettingsPanelProps> = ({
  onClose,
  onConnectionsChanged,
}) => {
  const { t } = useTranslation();
  const [connections, setConnections] = useState<Connection[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState("");
  const [isEditing, setIsEditing] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [passwordChanged, setPasswordChanged] = useState(false);
  const [enablePasswordChanged, setEnablePasswordChanged] = useState(false);
  const [errors, setErrors] = useState<{ [key: string]: string }>({});
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [mcpHosts, setMcpHosts] = useState<McpHost[]>([]);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const fetchMcpHosts = async () => {
      try {
        const hosts: McpHost[] = await invoke("get_mcp_hosts");
        setMcpHosts(hosts);
      } catch (e) {
        console.error("Failed to fetch MCP hosts:", e);
      }
    };
    fetchMcpHosts();
  }, []);

  useEffect(() => {
    const initConnections = async () => {
      try {
        const savedConnections: Connection[] = await invoke("load_connections");
        setConnections(savedConnections || []);
      } catch (e) {
        console.error("Failed to load connections:", e);
      } finally {
        setIsLoading(false);
      }
    };
    initConnections();
  }, []);

  const [formData, setFormData] = useState({
    hostname: "",
    ip: "",
    port: "",
    type: "SSH",
    username: "",
    password: "",
    enablePassword: "",
    passphrase: "",
    rememberPassword: true,
    agentForwarding: false,
    authMethod: "plain",
    privateKeyPath: "",
    consolePort: "COM1",
    baudRate: 9600,
    deviceType: "cisco_ios",
    vendorType: "",
  });

  const filteredConnections = connections.filter(
    (conn) =>
      conn.hostname.toLowerCase().includes(searchQuery.toLowerCase()) ||
      conn.ip.includes(searchQuery) ||
      conn.type.toLowerCase().includes(searchQuery.toLowerCase()) ||
      (conn.vendorType && conn.vendorType.toLowerCase().includes(searchQuery.toLowerCase())) ||
      (conn.deviceType && conn.deviceType.toLowerCase().includes(searchQuery.toLowerCase()))
  );

  const handleEdit = (conn: Connection) => {
    setEditingId(conn.id);
    setFormData({
      hostname: conn.hostname,
      ip: conn.ip,
      port: conn.port ? conn.port.toString() : "",
      type: conn.type.split(" ")[0] as any,
      username: conn.username || "root", // Mock data doesn't have these, using defaults
      password: "",
      enablePassword: "",
      passphrase: "",
      rememberPassword: true,
      agentForwarding: false,
      authMethod: "plain",
      privateKeyPath: "",
      consolePort: "COM1",
      baudRate: 9600,
      deviceType: conn.deviceType || "cisco_ios",
      vendorType: conn.vendorType || "",
    });
    setPasswordChanged(false);
    setEnablePasswordChanged(false);
    setErrors({});
    setIsEditing(true);
  };

  const handleAddHost = () => {
    setEditingId(null);
    setFormData({
      hostname: "",
      ip: "",
      port: "",
      type: "SSH",
      username: "",
      password: "",
      enablePassword: "",
      passphrase: "",
      rememberPassword: true,
      agentForwarding: false,
      authMethod: "plain",
      privateKeyPath: "",
      consolePort: "COM1",
      baudRate: 9600,
      deviceType: "cisco_ios",
      vendorType: "",
    });
    setPasswordChanged(false);
    setEnablePasswordChanged(false);
    setErrors({});
    setIsEditing(true);
  };

  const validate = () => {
    const newErrors: { [key: string]: string } = {};
    if (formData.type !== "Console" && !formData.ip.trim()) {
      newErrors.ip = t("connection_panel.err_ip_required");
    }
    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleMcpLookup = async () => {
    const mcpMatch = mcpHosts.find(
      (h) => h.hostname.toLowerCase() === formData.hostname.toLowerCase()
    );
    if (mcpMatch) {
      // Try to determine vendor from deviceType
      let vendor = "";
      if (mcpMatch.deviceType.toLowerCase().includes("cisco")) vendor = "Cisco";
      else if (mcpMatch.deviceType.toLowerCase().includes("juniper")) vendor = "Juniper";
      else if (mcpMatch.deviceType.toLowerCase().includes("arista")) vendor = "Arista";
      else if (mcpMatch.deviceType.toLowerCase().includes("yamaha")) vendor = "Yamaha";
      else if (mcpMatch.deviceType.toLowerCase().includes("linux")) vendor = "Linux";

      setFormData({
        ...formData,
        ip: mcpMatch.ip,
        type: mcpMatch.deviceType.split(" ")[0] as any,
        username: mcpMatch.username,
        deviceType: mcpMatch.deviceType,
        vendorType: vendor,
      });
      // Clear errors if we found it
      if (errors.ip) {
        const newErrors = { ...errors };
        delete newErrors.ip;
        setErrors(newErrors);
      }
      await message(t("connection_panel.msg_mcp_found", { hostname: mcpMatch.hostname }));
    } else {
      await message(t("connection_panel.msg_mcp_not_found", { hostname: formData.hostname }));
    }
  };

  const handleSave = async () => {
    if (validate()) {
      console.log("Saving connection:", formData);
      let updatedConnections = connections;

      if (editingId) {
        updatedConnections = connections.map((conn) =>
          conn.id === editingId
            ? {
                ...conn,
                hostname: formData.hostname || formData.ip,
                ip: formData.ip,
                port: formData.port ? parseInt(formData.port, 10) : undefined,
                type:
                  formData.type === "Console"
                    ? "Console (Serial)"
                    : `${formData.type} ${formData.authMethod === "key" ? "(Key)" : "(Password)"}`,
                username: formData.username,
                password: formData.password,
                enablePassword: formData.enablePassword,
                deviceType: formData.deviceType,
                vendorType: formData.vendorType,
                passwordChanged: passwordChanged,
                enablePasswordChanged: enablePasswordChanged,
              }
            : conn
        );
      } else {
        const newConnection: Connection = {
          id: Date.now().toString(),
          status: "offline",
          hostname: formData.hostname || formData.ip,
          ip: formData.ip,
          port: formData.port ? parseInt(formData.port, 10) : undefined,
          type:
            formData.type === "Console"
              ? "Console (Serial)"
              : `${formData.type} ${formData.authMethod === "key" ? "(Key)" : "(Password)"}`,
          lastConnected: "Never",
          username: formData.username,
          password: formData.password,
          enablePassword: formData.enablePassword,
          deviceType: formData.deviceType,
          vendorType: formData.vendorType,
          passwordChanged: true,
          enablePasswordChanged: true,
        };
        updatedConnections = [...connections, newConnection];
      }

      setConnections(updatedConnections);

      try {
        await invoke("save_connections", { connections: updatedConnections });
        onConnectionsChanged?.();
      } catch (e) {
        console.error("Failed to save connections:", e);
      }

      setIsEditing(false);
    }
  };

  const handleDeleteCurrent = async () => {
    if (!editingId) return;

    const connToDelete = connections.find((c) => c.id === editingId);
    if (!connToDelete) return;

    const updatedConnections = connections.filter((conn) => conn.id !== editingId);
    setConnections(updatedConnections);

    // Clean up selectedIds to avoid dangling state
    setSelectedIds((prev) => prev.filter((id) => id !== editingId));

    try {
      await invoke("save_connections", { connections: updatedConnections });
      onConnectionsChanged?.();
    } catch (e) {
      console.error("Failed to delete connection:", e);
    }

    setIsEditing(false);
    setEditingId(null);
  };

  const handleDeleteRow = async (id: string) => {
    const updatedConnections = connections.filter((conn) => conn.id !== id);
    setConnections(updatedConnections);
    setSelectedIds((prev) => prev.filter((i) => i !== id));

    try {
      await invoke("save_connections", { connections: updatedConnections });
      onConnectionsChanged?.();
    } catch (e) {
      console.error("Failed to delete connection:", e);
    }
  };

  const handleImportCsv = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = async (event) => {
      const text = event.target?.result as string;
      if (!text) return;

      Papa.parse(text, {
        header: true,
        skipEmptyLines: true,
        complete: async (results) => {
          interface CsvRow {
            id?: string;
            status?: string;
            hostname?: string;
            ip?: string;
            port?: string;
            type?: string;
            lastConnected?: string;
            username?: string;
            password?: string;
            enablePassword?: string;
            deviceType?: string;
            vendorType?: string;
          }
          const newConnections: Connection[] = [];

          (results.data as CsvRow[]).forEach((row, i) => {
            if (row.hostname && row.ip) {
              const newConn: Connection = {
                id: row.id || Date.now().toString() + i,
                status:
                  row.status === "online" || row.status === "offline" ? row.status : "offline",
                hostname: row.hostname,
                ip: row.ip,
                port: row.port ? parseInt(row.port, 10) : undefined,
                type: row.type || "SSH",
                lastConnected: row.lastConnected || "Never",
                username: row.username || "",
                password: row.password || "",
                enablePassword: row.enablePassword || "",
                deviceType: row.deviceType || "cisco_ios",
                vendorType: row.vendorType || "",
              };
              newConnections.push(newConn);
            }
          });

          if (newConnections.length > 0) {
            const connectionMap = new Map(connections.map((c) => [c.id, c]));
            for (const newConn of newConnections) {
              connectionMap.set(newConn.id, newConn);
            }
            const updatedConnections = Array.from(connectionMap.values());

            setConnections(updatedConnections);
            try {
              await invoke("save_connections", { connections: updatedConnections });
              onConnectionsChanged?.();
              await message(t("connection_panel.msg_csv_imported", { count: newConnections.length }));
            } catch (error) {
              console.error("Failed to save imported connections:", error);
            }
          }
        },
      });
      // Reset file input
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    };

    reader.readAsText(file);
  };

  const handleExportCsv = () => {
    // Basic CSV escaping function
    const escapeCsv = (val: string) => {
      if (val == null) return "";
      const str = String(val);
      if (str.includes(",") || str.includes('"') || str.includes("\n")) {
        return `"${str.replace(/"/g, '""')}"`;
      }
      return str;
    };

    const headers = [
      "id",
      "status",
      "hostname",
      "ip",
      "port",
      "type",
      "lastConnected",
      "deviceType",
      "vendorType",
      "username",
    ];
    const csvRows = [];

    // Add header row
    csvRows.push(headers.map(escapeCsv).join(","));

    // Add data rows
    for (const conn of connections) {
      const row = [
        conn.id,
        conn.status,
        conn.hostname,
        conn.ip,
        conn.port !== undefined ? conn.port.toString() : "",
        conn.type,
        conn.lastConnected,
        conn.deviceType || "",
        conn.vendorType || "",
        conn.username || "",
      ];
      csvRows.push(row.map(escapeCsv).join(","));
    }

    const csvContent = csvRows.join("\n");
    const blob = new Blob([new Uint8Array([0xef, 0xbb, 0xbf]), csvContent], {
      type: "text/csv;charset=utf-8;",
    });
    const url = URL.createObjectURL(blob);

    const link = document.createElement("a");
    link.href = url;
    link.setAttribute("download", "connections.csv");
    document.body.appendChild(link);
    link.click();
    document.body.removeChild(link);
    URL.revokeObjectURL(url);
  };

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => (prev.includes(id) ? prev.filter((i) => i !== id) : [...prev, id]));
  };

  const toggleSelectAll = () => {
    if (selectedIds.length === filteredConnections.length && filteredConnections.length > 0) {
      setSelectedIds([]);
    } else {
      setSelectedIds(filteredConnections.map((c) => c.id));
    }
  };

  const handleDeleteSelected = async () => {
    if (selectedIds.length === 0) return;

    const updatedConnections = connections.filter((conn) => !selectedIds.includes(conn.id));
    setConnections(updatedConnections);
    setSelectedIds([]);

    try {
      await invoke("save_connections", { connections: updatedConnections });
      onConnectionsChanged?.();
    } catch (e) {
      console.error("Failed to delete connections:", e);
    }
  };

  const renderForm = () => (
    <div className="connection-form-modal-overlay">
      <div className="connection-form-card">
        <header className="form-card-header">
          <h3>{editingId ? t("connection_panel.header_edit") : t("connection_panel.header_new")}</h3>
          <button className="close-card-btn" onClick={() => setIsEditing(false)}>
            &times;
          </button>
        </header>
        <div className="connection-form-content">
          <div className="form-section">
            <h3>{t("connection_panel.tab_basic")}</h3>
            <div className="form-grid">
              <div className="form-group">
                <label>{t("connection_panel.hostname_label")}</label>
                <input
                  type="text"
                  value={formData.hostname}
                  onChange={(e) => setFormData({ ...formData, hostname: e.target.value })}
                  placeholder={t("connection_panel.hostname_placeholder")}
                />
                <button
                  className="btn-mcp-lookup"
                  onClick={handleMcpLookup}
                  disabled={!formData.hostname}
                  title={t("connection_panel.mcp_fetch")}
                >
                  <svg
                    width="14"
                    height="14"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path>
                    <polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline>
                    <line x1="12" y1="22.08" x2="12" y2="12"></line>
                  </svg>
                  {t("connection_panel.mcp_fetch_btn")}
                </button>
              </div>
              <div className="form-group">
                <label>{t("connection_panel.connection_type")}</label>
                <select
                  value={formData.type}
                  onChange={(e) => setFormData({ ...formData, type: e.target.value })}
                >
                  <option value="SSH">SSH</option>
                  <option value="Telnet">Telnet</option>
                  <option value="Console">Console (Serial)</option>
                </select>
              </div>

              <div className="form-group">
                <label>{t("connection_panel.vendor_type")}</label>
                <input
                  type="text"
                  value={formData.vendorType}
                  onChange={(e) => setFormData({ ...formData, vendorType: e.target.value })}
                  placeholder={t("connection_panel.vendor_placeholder")}
                />
              </div>
              <div className="form-group">
                <label>{t("connection_panel.device_type")}</label>
                <Select
                  options={deviceTypeOptions}
                  value={deviceTypeOptions.find((opt) => opt.value === formData.deviceType)}
                  onChange={(selectedOption) =>
                    setFormData({ ...formData, deviceType: selectedOption ? selectedOption.value : "" })
                  }
                  styles={customSelectStyles}
                  placeholder="Select device type..."
                />
              </div>

              {formData.type !== "Console" ? (
                <>
                  <div className="form-group full-width">
                    <label>
                      {t("connection_panel.ip_label")} <span style={{ color: "#ef4444" }}>*</span>
                    </label>
                    <input
                      type="text"
                      className={errors.ip ? "error" : ""}
                      value={formData.ip}
                      onChange={(e) => setFormData({ ...formData, ip: e.target.value })}
                      placeholder="192.168.1.1 or router.local"
                    />
                    {errors.ip && <span className="error-message">{errors.ip}</span>}
                  </div>
                  <div className="form-group">
                    <label>{t("connection_panel.port_label")}</label>
                    <input
                      type="text"
                      value={formData.port}
                      onChange={(e) =>
                        setFormData({ ...formData, port: e.target.value.replace(/[^0-9]/g, "") })
                      }
                      placeholder={
                        formData.type === "SSH" ? "22" : formData.type === "Telnet" ? "23" : ""
                      }
                    />
                  </div>
                  <div className="form-group">
                    <label>{t("connection_panel.username_label")}</label>
                    <input
                      type="text"
                      value={formData.username}
                      onChange={(e) => setFormData({ ...formData, username: e.target.value })}
                    />
                  </div>
                  <div className="form-group">
                    <label>{t("connection_panel.password_label")}</label>
                    <input
                      type="password"
                      value={formData.password}
                      onChange={(e) => {
                        setFormData({ ...formData, password: e.target.value });
                        setPasswordChanged(true);
                      }}
                      placeholder={editingId && connections.find(c => c.id === editingId)?.hasPassword ? "••••••••" : ""}
                    />
                  </div>
                  <div className="form-group">
                    <label>{t("connection_panel.enable_password_label")}</label>
                    <input
                      type="password"
                      value={formData.enablePassword}
                      onChange={(e) => {
                        setFormData({ ...formData, enablePassword: e.target.value });
                        setEnablePasswordChanged(true);
                      }}
                      placeholder={editingId && connections.find(c => c.id === editingId)?.hasEnablePassword ? "••••••••" : ""}
                    />
                  </div>
                </>
              ) : (
                <>
                  <div className="form-group">
                    <label>{t("connection_panel.serial_port_label")}</label>
                    <input
                      type="text"
                      value={formData.consolePort}
                      onChange={(e) => setFormData({ ...formData, consolePort: e.target.value })}
                      placeholder="COM1 or /dev/ttyUSB0"
                    />
                  </div>
                  <div className="form-group">
                    <label>{t("connection_panel.baudrate_label")}</label>
                    <select
                      value={formData.baudRate}
                      onChange={(e) =>
                        setFormData({ ...formData, baudRate: parseInt(e.target.value) })
                      }
                    >
                      <option value="9600">9600</option>
                      <option value="19200">19200</option>
                      <option value="38400">38400</option>
                      <option value="57600">57600</option>
                      <option value="115200">115200</option>
                    </select>
                  </div>
                </>
              )}
            </div>
          </div>

          {formData.type === "SSH" && (
            <div className="form-section">
              <h3>{t("connection_panel.tab_ssh")}</h3>
              <div className="ssh-auth-grid">
                <div className="form-group">
                  <label>{t("connection_panel.passphrase_label")}</label>
                  <input
                    type="password"
                    value={formData.passphrase}
                    onChange={(e) => setFormData({ ...formData, passphrase: e.target.value })}
                  />
                </div>

                <div className="ssh-checkbox-group">
                  <label className="checkbox-item">
                    <input
                      type="checkbox"
                      checked={formData.rememberPassword}
                      onChange={(e) =>
                        setFormData({ ...formData, rememberPassword: e.target.checked })
                      }
                    />
                    {t("connection_panel.remember_password")}
                  </label>
                  <label className="checkbox-item">
                    <input
                      type="checkbox"
                      checked={formData.agentForwarding}
                      onChange={(e) =>
                        setFormData({ ...formData, agentForwarding: e.target.checked })
                      }
                    />
                    {t("connection_panel.agent_forwarding")}
                  </label>
                </div>

                <div className="auth-methods-list">
                  <div className="auth-method-item">
                    <input
                      type="radio"
                      name="authMethod"
                      checked={formData.authMethod === "plain"}
                      onChange={() => setFormData({ ...formData, authMethod: "plain" })}
                    />
                    <div className="auth-method-content">
                      <span className="auth-method-label">{t("connection_panel.auth_password")}</span>
                    </div>
                  </div>

                  <div className="auth-method-item">
                    <input
                      type="radio"
                      name="authMethod"
                      checked={formData.authMethod === "key"}
                      onChange={() => setFormData({ ...formData, authMethod: "key" })}
                    />
                    <div className="auth-method-content">
                      <span className="auth-method-label">{t("connection_panel.auth_key")}</span>
                      <div className="auth-method-details">
                        <button className="btn-file-select">{t("connection_panel.key_select_btn")}</button>
                        <input
                          type="text"
                          className="path-input"
                          placeholder={t("connection_panel.key_placeholder")}
                          value={formData.privateKeyPath}
                          onChange={(e) =>
                            setFormData({ ...formData, privateKeyPath: e.target.value })
                          }
                          disabled={formData.authMethod !== "key"}
                        />
                      </div>
                    </div>
                  </div>

                  <div className="auth-method-item">
                    <input
                      type="radio"
                      name="authMethod"
                      checked={formData.authMethod === "keyboard"}
                      onChange={() => setFormData({ ...formData, authMethod: "keyboard" })}
                    />
                    <div className="auth-method-content">
                      <span className="auth-method-label">
                        {t("connection_panel.auth_keyboard_interactive")}
                      </span>
                    </div>
                  </div>

                  <div className="auth-method-item">
                    <input
                      type="radio"
                      name="authMethod"
                      checked={formData.authMethod === "pageant"}
                      onChange={() => setFormData({ ...formData, authMethod: "pageant" })}
                    />
                    <div className="auth-method-content">
                      <span className="auth-method-label">{t("connection_panel.auth_pageant")}</span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
        <footer className="form-footer">
          {editingId && (
            <button
              className="btn-cancel"
              style={{
                marginRight: "auto",
                backgroundColor: "#fee2e2",
                color: "#dc2626",
                borderColor: "#f87171",
              }}
              onClick={handleDeleteCurrent}
            >
              {t("common.delete")}
            </button>
          )}
          <button className="btn-cancel" onClick={() => setIsEditing(false)}>
            {t("common.cancel")}
          </button>
          <button className="btn-save" onClick={handleSave}>
            {editingId ? t("common.save") : t("connection_panel.btn_add_host")}
          </button>
        </footer>
      </div>
    </div>
  );

  return (
    <div className="connection-settings-overlay">
      <div className="connection-settings-panel">
        <header className="connection-header-new">
          <div className="header-title-container">
            <h2>{t("connection_panel.header")}</h2>
          </div>
          <button className="panel-close-btn" onClick={onClose}>
            &times;
          </button>
        </header>

        <div className="connection-toolbar">
          <div className="toolbar-left">
            <span className="results-count">
              <strong>{filteredConnections.length}</strong> / <strong>{connections.length}</strong>{" "}
              {t("connection_panel.show_hosts")}
            </span>
            <div className="search-box-container">
              <svg
                className="search-icon"
                width="16"
                height="16"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <circle cx="11" cy="11" r="8"></circle>
                <line x1="21" y1="21" x2="16.65" y2="16.65"></line>
              </svg>
              <input
                type="text"
                placeholder={t("connection_panel.search_placeholder")}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          </div>
          <div className="toolbar-right">
            <button className="toolbar-btn">
              <svg
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M3 6h18M3 12h18M3 18h18"></path>
              </svg>
              {t("connection_panel.display_settings")}
              <svg
                width="12"
                height="12"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <polyline points="6 9 12 15 18 9"></polyline>
              </svg>
            </button>
            <div className="csv-actions">
              <input
                type="file"
                accept=".csv"
                ref={fileInputRef}
                style={{ display: "none" }}
                onChange={handleImportCsv}
              />
              <button className="toolbar-btn csv-btn" onClick={() => fileInputRef.current?.click()}>
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                  <polyline points="7 10 12 15 17 10"></polyline>
                  <line x1="12" y1="15" x2="12" y2="3"></line>
                </svg>
                {t("connection_panel.csv_import")}
              </button>
              <button className="toolbar-btn csv-btn" onClick={handleExportCsv}>
                <svg
                  width="14"
                  height="14"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                >
                  <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
                  <polyline points="17 8 12 3 7 8"></polyline>
                  <line x1="12" y1="3" x2="12" y2="15"></line>
                </svg>
                {t("connection_panel.csv_export")}
              </button>
            </div>
          </div>
        </div>

        <div className="connection-table-wrapper">
          <table className="connection-table">
            <thead>
              <tr>
                <th className="col-status">
                  <input
                    type="checkbox"
                    className="access-checkbox"
                    checked={
                      filteredConnections.length > 0 &&
                      selectedIds.length === filteredConnections.length
                    }
                    onChange={toggleSelectAll}
                  />
                </th>
                <th className="col-hostname">{t("connection_panel.th_hostname")}</th>
                <th className="col-ip">IP</th>
                <th className="col-vendor">{t("connection_panel.th_vendor")}</th>
                <th className="col-device-type">{t("connection_panel.th_device_type")}</th>
                <th className="col-type">{t("connection_panel.th_connection_type")}</th>
                <th className="col-last">{t("connection_panel.th_last_connected")}</th>
                <th className="col-actions">{t("connection_panel.th_actions")}</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr>
                  <td colSpan={8} style={{ textAlign: "center", padding: "30px", color: "var(--text-muted)" }}>
                    {t("connection_panel.loading_hosts")}
                  </td>
                </tr>
              ) : filteredConnections.length === 0 ? (
                <tr>
                  <td colSpan={8} style={{ textAlign: "center", padding: "30px", color: "var(--text-muted)" }}>
                    {searchQuery ? t("connection_panel.no_results_search") : t("connection_panel.no_hosts")}
                  </td>
                </tr>
              ) : (
                filteredConnections.map((conn) => (
                  <tr key={conn.id} className={selectedIds.includes(conn.id) ? "selected" : ""}>
                    <td className="col-status">
                      <input
                        type="checkbox"
                        className="access-checkbox"
                        checked={selectedIds.includes(conn.id)}
                        onChange={() => toggleSelect(conn.id)}
                      />
                    </td>
                    <td className="col-hostname">
                      <div className="hostname-cell">
                        <div className="device-icon">
                          <ServerIcon size={14} />
                        </div>
                        <span
                          className="hostname-text"
                          onClick={() => handleEdit(conn)}
                          role="button"
                          tabIndex={0}
                          onKeyDown={(e) => {
                            if (e.key === "Enter" || e.key === " ") {
                              e.preventDefault();
                              handleEdit(conn);
                            }
                          }}
                        >
                          {conn.hostname}
                        </span>
                        {mcpHosts.some((mh) => mh.hostname === conn.hostname) && (
                          <span className="mcp-badge" title={t("connection_panel.badge_mcp")}>
                            MCP
                          </span>
                        )}
                      </div>
                    </td>
                    <td className="col-ip">{conn.ip}</td>
                    <td className="col-vendor">{conn.vendorType || "-"}</td>
                    <td className="col-device-type">
                      {conn.deviceType ? getDeviceTypeAlias(conn.deviceType) : "-"}
                    </td>
                    <td className="col-type">
                      <div className="type-badge">{conn.type.split(" ")[0]}</div>
                      <span className="type-detail">
                        {conn.type.split(" ").slice(1).join(" ") || ""}
                      </span>
                    </td>
                    <td className="col-last">{conn.lastConnected}</td>
                    <td className="col-actions">
                      <button
                        className="row-delete-btn"
                        onClick={() => handleDeleteRow(conn.id)}
                        title={t("common.delete")}
                      >
                        <TrashIcon size={14} />
                      </button>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>

        <footer className="connection-panel-footer">
          <button className="add-device-btn" onClick={handleAddHost}>
            {t("connection_panel.btn_add_host")}
          </button>
          <button
            className="delete-selected-btn"
            onClick={handleDeleteSelected}
            disabled={selectedIds.length === 0}
            style={{
              opacity: selectedIds.length === 0 ? 0.5 : 1,
              cursor: selectedIds.length === 0 ? "not-allowed" : "pointer",
            }}
          >
            {t("connection_panel.btn_delete_selected")} {selectedIds.length > 0 && `(${selectedIds.length})`}
          </button>
        </footer>

        {isEditing && renderForm()}
      </div>
    </div>
  );
};
