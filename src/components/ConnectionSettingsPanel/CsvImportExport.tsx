import React from "react";
import { invoke } from "@tauri-apps/api/core";
import { message, open, save } from "@tauri-apps/plugin-dialog";
import { useTranslation } from "react-i18next";
import { Connection } from "../../types";

interface CsvImportExportProps {
  setConnections: React.Dispatch<React.SetStateAction<Connection[]>>;
  onConnectionsChanged?: () => void;
}

export const CsvImportExport: React.FC<CsvImportExportProps> = ({
  setConnections,
  onConnectionsChanged,
}) => {
  const { t } = useTranslation();
  const handleImportCsv = async () => {
    const selected = await open({ multiple: false, filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (!selected || Array.isArray(selected)) return;
    const result = await invoke<{ connections: Connection[]; importedCount: number; warnings: { row: number; reason: string }[] }>("import_connections_csv", { path: selected });
    setConnections(result.connections);
    onConnectionsChanged?.();
    await message(t("connection_panel.msg_csv_imported", { count: result.importedCount }));
  };

  const handleExportCsv = async () => {
    const path = await save({ defaultPath: "connections.csv", filters: [{ name: "CSV", extensions: ["csv"] }] });
    if (path) await invoke("export_connections_csv", { path });
  };

  return (
    <div className="csv-actions">
      <button className="toolbar-btn csv-btn" onClick={() => void handleImportCsv()}>
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
