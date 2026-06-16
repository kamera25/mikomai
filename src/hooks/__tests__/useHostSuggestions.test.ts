import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useHostSuggestions } from '../useHostSuggestions';
import * as tauriApi from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('useHostSuggestions', () => {
  const defaultProps = {
    recentIPs: [],
    setRecentIPs: vi.fn(),
    activeSessionId: 'session-1',
    updateSessionRecentIps: vi.fn(),
    saveAllSettings: vi.fn().mockResolvedValue(undefined),
    input: 'test @',
    setInput: vi.fn(),
    textareaRef: { current: null },
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should fetch hosts on mount', async () => {
    vi.mocked(tauriApi.invoke).mockImplementation(async (cmd) => {
      if (cmd === 'load_connections') {
        return [{ hostname: 'router-1', ip: '10.0.0.1' }];
      }
      if (cmd === 'get_mcp_hosts') {
        return [{ hostname: 'switch-1', ip: '10.0.0.2' }];
      }
      return [];
    });

    const { result } = renderHook(() => useHostSuggestions(defaultProps));

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    expect(result.current.availableHosts).toContainEqual({ hostname: 'router-1', ip: '10.0.0.1' });
    expect(result.current.availableHosts).toContainEqual({ hostname: 'switch-1', ip: '10.0.0.2' });
  });

  it('should add new host to recentIPs', () => {
    const setRecentIPs = vi.fn();
    const { result } = renderHook(() =>
      useHostSuggestions({
        ...defaultProps,
        recentIPs: ['192.168.1.1'],
        setRecentIPs,
      })
    );

    act(() => {
      result.current.updateRecentHosts(['10.0.0.1']);
    });

    expect(setRecentIPs).toHaveBeenCalledWith(['10.0.0.1', '192.168.1.1']);
  });
});
