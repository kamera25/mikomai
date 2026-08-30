import { describe, expect, it } from "vitest";
import { initialUIState, uiReducer } from "./UIContext";

describe("uiReducer", () => {
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
    const scheduled = uiReducer(initialUIState, { type: "SET_SCHEDULED_TASKS_OPEN", payload: true });
    const closed = uiReducer(scheduled, { type: "SET_SCHEDULED_TASKS_OPEN", payload: false });
    expect(closed.activePanel).toBe("chat");
  });
});
