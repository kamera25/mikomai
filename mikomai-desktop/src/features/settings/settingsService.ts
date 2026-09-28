import { ipc, COMMANDS } from "../../platform";
import type { SystemSettings } from "../../types";
export const settingsService = {
  load: () => ipc.command<SystemSettings>(COMMANDS.loadSettings),
  save: (settings: SystemSettings) => ipc.command<void>(COMMANDS.saveSettings, { settings }),
};
