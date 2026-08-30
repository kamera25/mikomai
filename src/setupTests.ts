import "@testing-library/jest-dom";
import { vi } from "vitest";
import jaTranslation from "./locales/ja/translation.json";

// Mock i18next and react-i18next
const translateMock = (key: string, options?: any) => {
  const parts = key.split(".");
  let current: any = jaTranslation;
  for (const part of parts) {
    if (current && current[part] !== undefined) {
      current = current[part];
    } else {
      return key;
    }
  }
  if (typeof current === "string") {
    let result = current;
    if (options) {
      Object.keys(options).forEach((optKey) => {
        result = result.replace(new RegExp(`{{${optKey}}}`, "g"), options[optKey]);
      });
    }
    return result;
  }
  return key;
};

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: translateMock,
    i18n: {
      changeLanguage: () => Promise.resolve(),
    },
  }),
  initReactI18next: {
    type: "3rdParty",
    init: () => {},
  },
}));

vi.mock("i18next", () => ({
  default: {
    t: translateMock,
    use: () => ({
      init: () => {},
    }),
  },
}));

// Mock Tauri API core and event
vi.mock("@tauri-apps/api/core", () => {
  return {
    invoke: vi.fn(async (cmd, _args) => {
      if (cmd === "load_settings") {
        return {
          repoPath: "",
          modelFilename: "",
          dbPath: "",
          consolePort: null,
          consoleBaudRate: 9600,
          ipVersion: "auto",
          autoSaveHistory: true,
          recentIPs: [],
        };
      }
      if (cmd === "load_connections") {
        return [];
      }
      if (cmd === "initialize_history") {
        return { history: [{ id: "test-session", type: "session", title: "新しいセッション", messages: [] }], activeSessionId: "test-session" };
      }
      if (cmd === "load_summaries") {
        return [];
      }
      if (cmd === "network_list_serial_ports") {
        return ["COM1", "COM2", "/dev/ttyUSB0"];
      }
      throw new Error(`Unknown command: ${cmd}`);
    }),
  };
});

vi.mock("@tauri-apps/api/event", () => {
  return {
    listen: vi.fn(async (_event, _callback) => {
      return () => {}; // return unlisten fn
    }),
    emit: vi.fn(async (_event, _payload) => {}),
  };
});
