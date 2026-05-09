import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act, waitFor } from '@testing-library/react';
import { useHistory } from '../useHistory';
import * as tauriApi from '@tauri-apps/api/core';
import { HistoryItem } from '../../types';

// Mock Tauri invoke
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('useHistory', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe('toggleFolder', () => {
    it('should toggle a top-level folder', async () => {
      const mockHistory: HistoryItem[] = [
        {
          id: 'folder-1',
          type: 'folder',
          name: 'Folder 1',
          isOpen: true,
          items: []
        }
      ];

      vi.mocked(tauriApi.invoke).mockResolvedValue(mockHistory);

      const { result } = renderHook(() => useHistory());

      await waitFor(() => {
        expect(result.current.isLoaded).toBe(true);
      });

      act(() => {
        result.current.toggleFolder('folder-1');
      });

      expect(result.current.history[0].type).toBe('folder');
      // @ts-ignore
      expect(result.current.history[0].isOpen).toBe(false);

      act(() => {
        result.current.toggleFolder('folder-1');
      });

      expect(result.current.history[0].type).toBe('folder');
      // @ts-ignore
      expect(result.current.history[0].isOpen).toBe(true);
    });

    it('should toggle a nested folder', async () => {
      const mockHistory: HistoryItem[] = [
        {
          id: 'folder-1',
          type: 'folder',
          name: 'Folder 1',
          isOpen: true,
          items: [
            {
              id: 'folder-2',
              type: 'folder',
              name: 'Folder 2',
              isOpen: false,
              items: []
            }
          ]
        }
      ];

      vi.mocked(tauriApi.invoke).mockResolvedValue(mockHistory);

      const { result } = renderHook(() => useHistory());

      await waitFor(() => {
        expect(result.current.isLoaded).toBe(true);
      });

      act(() => {
        result.current.toggleFolder('folder-2');
      });

      expect(result.current.history[0].type).toBe('folder');
      // @ts-ignore
      const nestedFolder = result.current.history[0].items[0];
      expect(nestedFolder.isOpen).toBe(true);

      // Top level folder should remain unchanged
      // @ts-ignore
      expect(result.current.history[0].isOpen).toBe(true);
    });

    it('should leave non-matching folders and other items unchanged', async () => {
      const mockHistory: HistoryItem[] = [
        {
          id: 'folder-1',
          type: 'folder',
          name: 'Folder 1',
          isOpen: true,
          items: []
        },
        {
          id: 'session-1',
          type: 'session',
          title: 'Session 1',
          messages: []
        }
      ];

      vi.mocked(tauriApi.invoke).mockResolvedValue(mockHistory);

      const { result } = renderHook(() => useHistory());

      await waitFor(() => {
        expect(result.current.isLoaded).toBe(true);
      });

      act(() => {
        result.current.toggleFolder('non-existent-folder');
      });

      expect(result.current.history).toEqual(mockHistory);
    });
  });
});
