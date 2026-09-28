import type { UIState } from "../contexts/UIContext";

export type PanelState = UIState["activePanel"];

/** Pure transition for the Root mediator's workspace state machine. */
export function transitionWorkspace(state: UIState, panel: PanelState): UIState {
  if (state.activePanel === panel) return state;
  return {
    ...state,
    activePanel: panel,
    isSettingsOpen: panel === "settings",
    isConnectionOpen: panel === "connections",
    isScheduledTasksOpen: panel === "scheduledTasks",
    isTaskAuditOpen: panel === "taskAudit",
    isSidebarOpen: panel === "chat" ? state.isSidebarOpen : false,
    isConfigDiffOpen: panel === "chat" ? state.isConfigDiffOpen : false,
  };
}
