import { describe, expect, it } from "vitest";
import { initialUIState, uiReducer } from "./UIContext";

describe("uiReducer", () => {
  it("moves between named states through NAVIGATE", () => {
    const settings = uiReducer(initialUIState, { type: "NAVIGATE", panel: "settings" });
    const audit = uiReducer(settings, { type: "NAVIGATE", panel: "taskAudit" });
    const chat = uiReducer(audit, { type: "NAVIGATE", panel: "chat" });
    expect(settings.isSettingsOpen).toBe(true);
    expect(settings.isSidebarOpen).toBe(false);
    expect(audit.isSettingsOpen).toBe(false);
    expect(audit.isTaskAuditOpen).toBe(true);
    expect(uiReducer(audit, { type: "SET_SIDEBAR_OPEN", payload: true })).toBe(audit);
    expect(uiReducer(audit, { type: "SET_CONFIG_DIFF_OPEN", payload: true })).toBe(audit);
    expect(chat.activePanel).toBe("chat");
    expect(chat.isTaskAuditOpen).toBe(false);
  });
  it("keeps workspace panels mutually exclusive", () => {
    const settings = uiReducer(initialUIState, { type: "SET_SETTINGS_OPEN", payload: true });
    const connections = uiReducer(settings, { type: "SET_CONNECTION_OPEN", payload: true });

    expect(settings.activePanel).toBe("settings");
    expect(connections.activePanel).toBe("connections");
    expect(connections.isSettingsOpen).toBe(false);
    expect(connections.isConnectionOpen).toBe(true);
    expect(connections.isSidebarOpen).toBe(false);
  });

  it("returns to chat when the active panel closes", () => {
    const scheduled = uiReducer(initialUIState, {
      type: "SET_SCHEDULED_TASKS_OPEN",
      payload: true,
    });
    const closed = uiReducer(scheduled, { type: "SET_SCHEDULED_TASKS_OPEN", payload: false });
    expect(closed.activePanel).toBe("chat");
  });
});
