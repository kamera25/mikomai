import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import "./ConnectionSettingsPanel.css";
import { Connection, McpHost } from "../../types";
import { ConnectionTable } from "./ConnectionTable";
import { ConnectionForm } from "./ConnectionForm";
import { CsvImportExport } from "./CsvImportExport";

interface ConnectionSettingsPanelProps {
  onClose: () => void;
  onConnectionsChanged?: () => void;
}

export const getDeviceTypeAlias = (
  deviceType: string,
  aliases: { [key: string]: string }
): string => {
  if (!deviceType) return "";
  if (aliases[deviceType]) {
    return aliases[deviceType];
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
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [mcpHosts, setMcpHosts] = useState<McpHost[]>([]);
  const [deviceTypes, setDeviceTypes] = useState<string[]>([]);
  const [deviceTypeAliases, setDeviceTypeAliases] = useState<{ [key: string]: string }>({});

  useEffect(() => {
    const fetchDeviceTypes = async () => {
      try {
        const response: { deviceTypes: string[]; deviceTypeAliases: { [key: string]: string } } =
          await invoke("get_device_types");
        setDeviceTypes(response.deviceTypes || []);
        setDeviceTypeAliases(response.deviceTypeAliases || {});
      } catch (e) {
        console.error("Failed to fetch device types:", e);
      }
    };
    fetchDeviceTypes();
  }, []);

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
    setIsEditing(true);
  };

  const handleAddHost = () => {
    setEditingId(null);
    setIsEditing(true);
  };

  const handleSave = async (
    formData: any,
    isPasswordDirty: boolean,
    isEnablePasswordDirty: boolean
  ) => {
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
              passwordChanged: isPasswordDirty,
              enablePasswordChanged: isEnablePasswordDirty,
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
  };

  const handleDeleteCurrent = async () => {
    if (!editingId) return;

    const updatedConnections = connections.filter((conn) => conn.id !== editingId);
    setConnections(updatedConnections);
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

  const getAliasHelper = (dt: string) => getDeviceTypeAlias(dt, deviceTypeAliases);

  return (
    <div className="connection-settings-overlay">
      <div className="connection-settings-panel">
        <header className="connection-header-new">
          <div className="header-title-container">
            <h2>{t("connection_panel.header")}</h2>
          </div>
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
            <CsvImportExport
              connections={connections}
              setConnections={setConnections}
              onConnectionsChanged={onConnectionsChanged}
            />
          </div>
        </div>

        <ConnectionTable
          connections={connections}
          filteredConnections={filteredConnections}
          selectedIds={selectedIds}
          isLoading={isLoading}
          searchQuery={searchQuery}
          mcpHosts={mcpHosts}
          toggleSelect={toggleSelect}
          toggleSelectAll={toggleSelectAll}
          handleEdit={handleEdit}
          handleDeleteRow={handleDeleteRow}
          getDeviceTypeAlias={getAliasHelper}
        />

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

        {isEditing && (
          <ConnectionForm
            editingId={editingId}
            connections={connections}
            mcpHosts={mcpHosts}
            deviceTypes={deviceTypes}
            getDeviceTypeAlias={getAliasHelper}
            onClose={() => setIsEditing(false)}
            onSave={handleSave}
            onDeleteCurrent={handleDeleteCurrent}
          />
        )}
      </div>
    </div>
  );
};
