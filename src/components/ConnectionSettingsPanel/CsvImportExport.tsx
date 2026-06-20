import React, { useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { message } from "@tauri-apps/plugin-dialog";
import Papa from "papaparse";
import { useTranslation } from "react-i18next";
import { Connection } from "../../types";

interface CsvImportExportProps {
  connections: Connection[];
  setConnections: React.Dispatch<React.SetStateAction<Connection[]>>;
  onConnectionsChanged?: () => void;
}

export const CsvImportExport: React.FC<CsvImportExportProps> = ({
  connections,
  setConnections,
  onConnectionsChanged,
}) => {
  const { t } = useTranslation();
  const fileInputRef = useRef<HTMLInputElement>(null);

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
      if (fileInputRef.current) {
        fileInputRef.current.value = "";
      }
    };

    reader.readAsText(file);
  };

  const handleExportCsv = () => {
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

    csvRows.push(headers.map(escapeCsv).join(","));

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

  return (
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
  );
};
