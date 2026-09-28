import React from "react";
import { ServerIcon, TrashIcon } from "../Icons";
import { Connection, McpHost } from "../../types";
import { useTranslation } from "react-i18next";

interface ConnectionTableProps {
  connections: Connection[];
  filteredConnections: Connection[];
  selectedIds: string[];
  isLoading: boolean;
  searchQuery: string;
  mcpHosts: McpHost[];
  toggleSelect: (id: string) => void;
  toggleSelectAll: () => void;
  handleEdit: (conn: Connection) => void;
  handleDeleteRow: (id: string) => void;
  getDeviceTypeAlias: (deviceType: string) => string;
}

export const ConnectionTable: React.FC<ConnectionTableProps> = ({
  filteredConnections,
  selectedIds,
  isLoading,
  searchQuery,
  mcpHosts,
  toggleSelect,
  toggleSelectAll,
  handleEdit,
  handleDeleteRow,
  getDeviceTypeAlias,
}) => {
  const { t } = useTranslation();

  return (
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
                <td className="col-ip">{conn.ip || "-"}</td>
                <td className="col-vendor">{conn.vendorType || "-"}</td>
                <td className="col-device-type">
                  {conn.deviceType ? getDeviceTypeAlias(conn.deviceType) : "-"}
                </td>
                <td className="col-type">
                  <div className="type-badge">{(conn.type || "").split(" ")[0]}</div>
                  <span className="type-detail">
                    {(conn.type || "").split(" ").slice(1).join(" ") || ""}
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
  );
};
