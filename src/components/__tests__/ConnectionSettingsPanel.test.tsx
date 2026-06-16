import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { ConnectionSettingsPanel } from '../ConnectionSettingsPanel.tsx';
import * as tauriApi from '@tauri-apps/api/core';
import * as tauriDialog from '@tauri-apps/plugin-dialog';

// Mock Tauri invoke and dialog functions
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  message: vi.fn(),
}));

describe('ConnectionSettingsPanel', () => {
  const defaultProps = {
    onClose: vi.fn(),
    onConnectionsChanged: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
    // Default mocks
    vi.mocked(tauriApi.invoke).mockImplementation(async (cmd, _args) => {
      if (cmd === 'get_mcp_hosts') {
        return [
          { hostname: 'Mcp-Host-01', ip: '192.168.10.10', deviceType: 'cisco_ios', username: 'admin' }
        ];
      }
      if (cmd === 'load_connections') {
        return [];
      }
      if (cmd === 'save_connections') {
        return null;
      }
      return null;
    });
  });

  it('renders correctly', async () => {
    render(<ConnectionSettingsPanel {...defaultProps} />);
    expect(screen.getByText('接続設定')).toBeInTheDocument();
  });

  it('handles MCP lookup success and calls message dialog', async () => {
    render(<ConnectionSettingsPanel {...defaultProps} />);
    
    // Wait for initial loads (mcp hosts and connections) to finish
    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith('get_mcp_hosts');
    });

    // Click Add Host button to open the form
    const addBtn = screen.getByText('ホスト追加');
    fireEvent.click(addBtn);

    // Enter hostname
    const hostnameInput = screen.getByPlaceholderText('例: Core-Switch-01');
    fireEvent.change(hostnameInput, { target: { value: 'Mcp-Host-01' } });

    // Click MCP Lookup
    const lookupBtn = screen.getByText('MCPから取得');
    fireEvent.click(lookupBtn);

    await waitFor(() => {
      expect(tauriDialog.message).toHaveBeenCalledWith(
        expect.stringContaining('MCPから「Mcp-Host-01」の情報を取得しました。')
      );
    });
  });

  it('handles MCP lookup failure and calls message dialog', async () => {
    render(<ConnectionSettingsPanel {...defaultProps} />);
    
    // Wait for initial loads (mcp hosts and connections) to finish
    await waitFor(() => {
      expect(tauriApi.invoke).toHaveBeenCalledWith('get_mcp_hosts');
    });

    // Click Add Host button to open the form
    const addBtn = screen.getByText('ホスト追加');
    fireEvent.click(addBtn);

    // Enter hostname not in MCP registry
    const hostnameInput = screen.getByPlaceholderText('例: Core-Switch-01');
    fireEvent.change(hostnameInput, { target: { value: 'Unknown-Host' } });

    // Click MCP Lookup
    const lookupBtn = screen.getByText('MCPから取得');
    fireEvent.click(lookupBtn);

    await waitFor(() => {
      expect(tauriDialog.message).toHaveBeenCalledWith(
        expect.stringContaining('MCPレジストリに「Unknown-Host」は見つかりませんでした。')
      );
    });
  });
});
