import '@testing-library/jest-dom';
import { vi } from 'vitest';

// Mock Tauri API core and event
vi.mock('@tauri-apps/api/core', () => {
  return {
    invoke: vi.fn(async (cmd, _args) => {
      if (cmd === 'load_settings') {
        return {
          repoPath: '',
          modelFilename: '',
          dbPath: '',
          consolePort: null,
          consoleBaudRate: 9600,
          ipVersion: 'auto',
          autoSaveHistory: true,
          recentIPs: []
        };
      }
      if (cmd === 'load_connections') {
        return [];
      }
      if (cmd === 'load_history') {
        return [];
      }
      if (cmd === 'load_summaries') {
        return [];
      }
      if (cmd === 'network_list_serial_ports') {
        return ['COM1', 'COM2', '/dev/ttyUSB0'];
      }
      throw new Error(`Unknown command: ${cmd}`);
    })
  };
});

vi.mock('@tauri-apps/api/event', () => {
  return {
    listen: vi.fn(async (_event, _callback) => {
      return () => {}; // return unlisten fn
    }),
    emit: vi.fn(async (_event, _payload) => {}),
  };
});
