import { describe, expect, it } from "vitest";
import { initialUIState } from "../contexts/UIContext";
import { transitionWorkspace } from "./stateMachine";

describe("Root workspace state machine", () => {
  it("keeps only one workspace panel active", () => {
    const settings = transitionWorkspace(initialUIState, "settings");
    const connections = transitionWorkspace(settings, "connections");
    expect(settings.activePanel).toBe("settings");
    expect(settings.isSidebarOpen).toBe(false);
    expect(connections.activePanel).toBe("connections");
    expect(connections.isSettingsOpen).toBe(false);
    expect(connections.isConnectionOpen).toBe(true);
  });

  it("restores chat without changing the sidebar choice", () => {
    const collapsed = { ...initialUIState, isSidebarOpen: false };
    const settings = transitionWorkspace(collapsed, "settings");
    const chat = transitionWorkspace(settings, "chat");
    expect(chat.activePanel).toBe("chat");
    expect(chat.isSidebarOpen).toBe(false);
  });
});
