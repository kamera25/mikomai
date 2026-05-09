import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { SettingsPanel } from '../SettingsPanel.tsx';
import * as tauriApi from '@tauri-apps/api/core';
import * as tauriDialog from '@tauri-apps/plugin-dialog';

// Mock Tauri invoke and dialog functions
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(),
}));

describe('SettingsPanel', () => {
  const defaultProps = {
    isOpen: true,
    onClose: vi.fn(),
    historyLimit: 10,
    onHistoryLimitChange: vi.fn(),
    temperature: 0.7,
    onTemperatureChange: vi.fn(),
    repetitionPenalty: 1.1,
    onRepetitionPenaltyChange: vi.fn(),
    modelPath: '',
    onModelPathChange: vi.fn(),
    mcpTimeout: 30,
    onMcpTimeoutChange: vi.fn(),
    dbPath: '',
    onDbPathChange: vi.fn(),
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders correctly when isOpen is true', () => {
    render(<SettingsPanel {...defaultProps} />);
    expect(screen.getByText('設定')).toBeInTheDocument();
  });

  it('does not render when isOpen is false', () => {
    render(<SettingsPanel {...defaultProps} isOpen={false} />);
    expect(screen.queryByText('設定')).not.toBeInTheDocument();
  });

  it('calls onClose when close button is clicked', () => {
    render(<SettingsPanel {...defaultProps} />);
    const closeButton = screen.getByTitle('設定を閉じる');
    fireEvent.click(closeButton);
    expect(defaultProps.onClose).toHaveBeenCalled();
  });

  it('handles history limit change', () => {
    render(<SettingsPanel {...defaultProps} />);
    const slider = screen.getAllByRole('slider')[0]; // History limit
    fireEvent.change(slider, { target: { value: '15' } });
    expect(defaultProps.onHistoryLimitChange).toHaveBeenCalledWith(15);
  });

  it('handles temperature change', () => {
    render(<SettingsPanel {...defaultProps} />);
    const slider = screen.getAllByRole('slider')[1]; // Temperature
    fireEvent.change(slider, { target: { value: '0.8' } });
    expect(defaultProps.onTemperatureChange).toHaveBeenCalledWith(0.8);
  });

  it('handles repetition penalty change', () => {
    render(<SettingsPanel {...defaultProps} />);
    const slider = screen.getAllByRole('slider')[2]; // Repetition Penalty
    fireEvent.change(slider, { target: { value: '1.2' } });
    expect(defaultProps.onRepetitionPenaltyChange).toHaveBeenCalledWith(1.2);
  });

  it('handles mcp timeout change', () => {
    render(<SettingsPanel {...defaultProps} />);
    const slider = screen.getAllByRole('slider')[3]; // MCP Timeout
    fireEvent.change(slider, { target: { value: '60' } });
    expect(defaultProps.onMcpTimeoutChange).toHaveBeenCalledWith(60);
  });

  it('handles model download and load success', async () => {
    vi.mocked(tauriApi.invoke).mockImplementation((cmd, _args) => {
      if (cmd === 'download_model') {
        return Promise.resolve('/fake/download/path.gguf');
      }
      if (cmd === 'load_model') {
        return Promise.resolve('Model loaded successfully');
      }
      return Promise.reject(new Error('Unknown command'));
    });

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText('モデルをダウンロードして読み込む');
    fireEvent.click(button);

    expect(tauriApi.invoke).toHaveBeenCalledWith('download_model', {
      repo: 'bartowski/google_gemma-4-E4B-it-GGUF',
      filename: 'google_gemma-4-E4B-it-Q4_K_M.gguf'
    });

    await waitFor(() => {
      expect(defaultProps.onModelPathChange).toHaveBeenCalledWith('/fake/download/path.gguf');
    });

    expect(tauriApi.invoke).toHaveBeenCalledWith('load_model', {
      path: '/fake/download/path.gguf'
    });

    await waitFor(() => {
      expect(screen.getByText('Success: Model loaded successfully')).toBeInTheDocument();
    });
  });

  it('handles model download failure', async () => {
    vi.mocked(tauriApi.invoke).mockImplementation((cmd, _args) => {
      if (cmd === 'download_model') {
        return Promise.reject(new Error('Network error'));
      }
      return Promise.reject(new Error('Unknown command'));
    });

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText('モデルをダウンロードして読み込む');
    fireEvent.click(button);

    await waitFor(() => {
      expect(screen.getByText('Error: Network error')).toBeInTheDocument();
    });
  });

  it('handles selecting db directory', async () => {
    vi.mocked(tauriDialog.open).mockResolvedValue('/selected/db/path');

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText('参照');
    fireEvent.click(button);

    expect(tauriDialog.open).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      title: "データベースディレクトリを選択"
    });

    await waitFor(() => {
      expect(defaultProps.onDbPathChange).toHaveBeenCalledWith('/selected/db/path');
    });
  });

  it('handles input changes for repoPath, modelFilename, and dbPath', () => {
    render(<SettingsPanel {...defaultProps} />);

    // repoPath input
    const repoInput = screen.getByDisplayValue('bartowski/google_gemma-4-E4B-it-GGUF');
    fireEvent.change(repoInput, { target: { value: 'new/repo' } });
    expect(repoInput).toHaveValue('new/repo');

    // modelFilename input
    const filenameInput = screen.getByDisplayValue('google_gemma-4-E4B-it-Q4_K_M.gguf');
    fireEvent.change(filenameInput, { target: { value: 'new_model.gguf' } });
    expect(filenameInput).toHaveValue('new_model.gguf');

    // dbPath input
    const dbInput = screen.getByPlaceholderText('/path/to/lancedb');
    fireEvent.change(dbInput, { target: { value: '/new/db/path' } });
    expect(defaultProps.onDbPathChange).toHaveBeenCalledWith('/new/db/path');
  });

  it('handles db directory selection cancellation', async () => {
    vi.mocked(tauriDialog.open).mockResolvedValue(null);

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText('参照');
    fireEvent.click(button);

    // wait briefly to ensure the promise resolves
    await new Promise(resolve => setTimeout(resolve, 0));

    expect(defaultProps.onDbPathChange).not.toHaveBeenCalled();
  });

  it('handles db directory selection error', async () => {
    // mock console.error to prevent it from cluttering the test output
    const consoleSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    vi.mocked(tauriDialog.open).mockRejectedValue(new Error('Dialog failed'));

    render(<SettingsPanel {...defaultProps} />);
    const button = screen.getByText('参照');
    fireEvent.click(button);

    await waitFor(() => {
      expect(consoleSpy).toHaveBeenCalledWith("Failed to select directory:", expect.any(Error));
    });
    expect(defaultProps.onDbPathChange).not.toHaveBeenCalled();

    consoleSpy.mockRestore();
  });

  it('handles save and close button click', () => {
    render(<SettingsPanel {...defaultProps} />);
    const saveButton = screen.getByText('保存して終了');
    fireEvent.click(saveButton);
    expect(defaultProps.onClose).toHaveBeenCalled();
  });
});
