import { describe, expect, it, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CsvImportExport } from "./CsvImportExport";
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  save: vi.fn(),
  message: vi.fn(),
}));

describe("CsvImportExport", () => {
  const setConnections = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("imports through the backend and refreshes the connection list", async () => {
    vi.mocked(open).mockResolvedValue("/tmp/connections.csv");
    vi.mocked(invoke).mockResolvedValue({ connections: [{ id: "1", hostname: "r1" }], importedCount: 1, warnings: [] });
    render(<CsvImportExport setConnections={setConnections} />);

    fireEvent.click(screen.getByText("CSVインポート"));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("import_connections_csv", { path: "/tmp/connections.csv" }));
    expect(setConnections).toHaveBeenCalledWith([{ id: "1", hostname: "r1" }]);
  });

  it("exports through the backend without reading connection data in the browser", async () => {
    vi.mocked(save).mockResolvedValue("/tmp/export.csv");
    vi.mocked(invoke).mockResolvedValue(undefined);
    render(<CsvImportExport setConnections={setConnections} />);

    fireEvent.click(screen.getByText("CSVエクスポート"));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("export_connections_csv", { path: "/tmp/export.csv" }));
  });
});
