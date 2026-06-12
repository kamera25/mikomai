import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

import './SettingsPanel.css';

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
  historyLimit: number;
  onHistoryLimitChange: (limit: number) => void;
  temperature: number;
  onTemperatureChange: (temp: number) => void;
  repetitionPenalty: number;
  onRepetitionPenaltyChange: (penalty: number) => void;
  modelPath: string | null;
  onModelPathChange: (path: string) => void;
  mcpTimeout: number;
  onMcpTimeoutChange: (timeout: number) => void;
  dbPath: string;
  onDbPathChange: (path: string) => void;
  ipVersion: string;
  onIpVersionChange: (version: string) => void;
  consolePort: string | null;
  onConsolePortChange: (port: string) => void;
  consoleBaudRate: number;
  onConsoleBaudRateChange: (rate: number) => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({ 
  isOpen, 
  onClose,
  historyLimit,
  onHistoryLimitChange,
  temperature,
  onTemperatureChange,
  repetitionPenalty,
  onRepetitionPenaltyChange,
  modelPath: _savedModelPath,
  onModelPathChange,
  mcpTimeout,
  onMcpTimeoutChange,
  dbPath,
  onDbPathChange,
  ipVersion,
  onIpVersionChange,
  consolePort,
  onConsolePortChange,
  consoleBaudRate,
  onConsoleBaudRateChange
}) => {
  const [repoPath, setRepoPath] = useState("bartowski/google_gemma-4-E4B-it-GGUF");
  const [modelFilename, setModelFilename] = useState("google_gemma-4-E4B-it-Q4_K_M.gguf");
  const [availablePorts, setAvailablePorts] = useState<string[]>([]);

  useEffect(() => {
    const fetchPorts = async () => {
      try {
        const result: any = await invoke("network_list_serial_ports");
        if (result && result.success && result.output) {
          const ports: string[] = [];
          const lines = result.output.split("\n");
          for (const line of lines) {
            if (line.trim().startsWith("- ")) {
              const parts = line.trim().substring(2).split(":");
              if (parts[0]) {
                ports.push(parts[0].trim());
              }
            }
          }
          setAvailablePorts(ports);
        }
      } catch (e) {
        console.error("Failed to fetch serial ports for settings:", e);
      }
    };
    fetchPorts();
  }, []);
  const [downloadStatus, setDownloadStatus] = useState("");
  const [isLoading, setIsLoading] = useState(false);


  const handleDownloadAndLoad = async () => {
    try {
      setIsLoading(true);
      setDownloadStatus("モデルのダウンロードを開始します（時間がかかる場合があります）");
      
      const downloadedPath = await invoke<string>("download_model", {
        repo: repoPath,
        filename: modelFilename
      });
      
      onModelPathChange(downloadedPath);
      setDownloadStatus(`Model downloaded to: ${downloadedPath}. Loading into memory...`);
      
      const loadResult = await invoke<string>("load_model", {
        path: downloadedPath
      });
      
      setDownloadStatus(`Success: ${loadResult}`);
    } catch (e: any) {
      setDownloadStatus(`Error: ${e.toString()}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleOpenModelDir = async () => {
    try {
      await invoke("open_model_dir", { modelPath: _savedModelPath });
    } catch (e: any) {
      setDownloadStatus(`Error: ${e.toString()}`);
    }
  };

  const handleSelectDbDir = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "データベースディレクトリを選択"
      });
      if (selected) {
        onDbPathChange(selected as string);
      }
    } catch (e) {
      console.error("Failed to select directory:", e);
    }
  };

  if (!isOpen) return null;

  return (
    <div className="settings-full-overlay">
      <div className="settings-panel">
        <header className="settings-header">
          <div className="settings-header-content">
            <div>
              <h2>設定</h2>
              <p>ローカルLLM、ナレッジベースを設定します。</p>
            </div>
            <button className="close-button" onClick={onClose} title="設定を閉じる">&times;</button>
          </div>
        </header>

        <div className="settings-body">
          <section className="settings-group">
            <h3>対話設定</h3>
            <div className="form-control">
              <label>過去の履歴保持数 (要約)</label>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <input 
                  type="range" 
                  min="0" 
                  max="20" 
                  value={historyLimit} 
                  onChange={(e) => onHistoryLimitChange(parseInt(e.target.value))}
                  style={{ flexGrow: 1 }}
                />
                <span style={{ minWidth: '32px', fontWeight: 'bold', color: 'var(--accent-color)' }}>{historyLimit}</span>
              </div>
              <p className="help-text" style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>
                AIが回答を生成する際に参照する過去の会話の要約数です。大きくすると文脈をより理解しますが、メモリ消費量が増える可能性があります。
              </p>
            </div>
            <div className="form-control">
              <label>Temperature (温度)</label>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <input 
                  type="range" 
                  min="0" 
                  max="2.0" 
                  step="0.1"
                  value={temperature} 
                  onChange={(e) => onTemperatureChange(parseFloat(e.target.value))}
                  style={{ flexGrow: 1 }}
                />
                <span style={{ minWidth: '32px', fontWeight: 'bold', color: 'var(--accent-color)' }}>{temperature.toFixed(1)}</span>
              </div>
              <p className="help-text" style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>
                回答のランダム性を制御します。0に設定すると最も決定的（同じ入力に対して同じ回答）になります。
              </p>
            </div>
            <div className="form-control">
              <label>Repetition Penalty (繰り返し抑制)</label>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <input 
                  type="range" 
                  min="1.0" 
                  max="2.0" 
                  step="0.05"
                  value={repetitionPenalty} 
                  onChange={(e) => onRepetitionPenaltyChange(parseFloat(e.target.value))}
                  style={{ flexGrow: 1 }}
                />
                <span style={{ minWidth: '32px', fontWeight: 'bold', color: 'var(--accent-color)' }}>{repetitionPenalty.toFixed(2)}</span>
              </div>
              <p className="help-text" style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>
                同じ言葉の繰り返しを抑制します。1.0で無効、値を大きくするほど繰り返しが少なくなります。
              </p>
            </div>
            <div className="form-control">
              <label>MCP 実行タイムアウト (秒)</label>
              <div style={{ display: 'flex', alignItems: 'center', gap: '12px' }}>
                <input
                  type="range"
                  min="5"
                  max="120"
                  step="5"
                  value={mcpTimeout}
                  onChange={(e) => onMcpTimeoutChange(parseInt(e.target.value))}
                  style={{ flexGrow: 1 }}
                />
                <span style={{ minWidth: '32px', fontWeight: 'bold', color: 'var(--accent-color)' }}>{mcpTimeout}</span>
              </div>
              <p className="help-text" style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>
                ツール (MCP) 実行の最大待機時間です。時間を超えると処理を中断します。
              </p>
            </div>
            <div className="form-control">
              <label htmlFor="ip-version-select">利用するインターネットプロトコルの指定</label>
              <select 
                id="ip-version-select"
                value={ipVersion} 
                onChange={(e) => onIpVersionChange(e.target.value)}
              >
                <option value="auto">自動</option>
                <option value="ipv4">IPv4のみ</option>
                <option value="ipv6">IPv6のみ</option>
              </select>
              <p className="help-text" style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>
                ホスト名解決および接続時に使用するIPプロトコルの優先設定です。
              </p>
            </div>
            <div className="form-control">
              <label htmlFor="console-port-select">コンソールポート (MCP用)</label>
              <div style={{ display: 'flex', gap: '8px' }}>
                <select
                  id="console-port-select"
                  value={availablePorts.includes(consolePort || '') ? (consolePort || '') : (consolePort ? 'custom' : '')}
                  onChange={(e) => {
                    const val = e.target.value;
                    if (val === 'custom') {
                      onConsolePortChange('/dev/ttyUSB0');
                    } else if (val === '') {
                      onConsolePortChange('');
                    } else {
                      onConsolePortChange(val);
                    }
                  }}
                  style={{ flexGrow: 1 }}
                >
                  <option value="">None (使用しない)</option>
                  {availablePorts.map(p => (
                    <option key={p} value={p}>{p}</option>
                  ))}
                  <option value="custom">手動入力...</option>
                </select>
                {(!availablePorts.includes(consolePort || '') && consolePort) || consolePort === 'custom' ? (
                  <input
                    type="text"
                    value={consolePort === 'custom' ? '' : (consolePort || '')}
                    onChange={(e) => onConsolePortChange(e.target.value)}
                    placeholder="例: /dev/ttyUSB0, COM1"
                    style={{ width: '200px' }}
                  />
                ) : null}
              </div>
              <p className="help-text" style={{ fontSize: '0.8rem', color: '#64748b', marginTop: '4px' }}>
                指定されている場合、MCPの呼び出し(fetch_arp, config, routing等)は内部でこのシリアルコンソール経由で実行されます。
              </p>
            </div>
            {(consolePort && consolePort !== '') && (
              <div className="form-control">
                <label htmlFor="console-baudrate-select">コンソールボーレート</label>
                <select
                  id="console-baudrate-select"
                  value={consoleBaudRate}
                  onChange={(e) => onConsoleBaudRateChange(parseInt(e.target.value))}
                >
                  <option value="9600">9600 bps</option>
                  <option value="19200">19200 bps</option>
                  <option value="38400">38400 bps</option>
                  <option value="57600">57600 bps</option>
                  <option value="115200">115200 bps</option>
                </select>
              </div>
            )}
          </section>

          <section className="settings-group">
            <h3>ローカルLLM (llama.cpp)</h3>
            <div className="form-control">
              <label>HuggingFace リポジトリ</label>
              <input type="text" value={repoPath} onChange={e => setRepoPath(e.target.value)} placeholder="bartowski/google_gemma-4-E2B-it-GGUF" />
            </div>
            <div className="form-control">
              <label>ファイル名 (GGUF)</label>
              <input type="text" value={modelFilename} onChange={e => setModelFilename(e.target.value)} placeholder="google_gemma-4-E2B-it-Q4_K_M.gguf" />
            </div>
            <div className="form-control">
              <div style={{ display: 'flex', gap: '8px' }}>
                <button 
                  className="btn btn-secondary" 
                  onClick={handleDownloadAndLoad}
                  disabled={isLoading}
                  style={{ flexGrow: 1 }}
                >
                  {isLoading ? "ダウンロード中..." : "モデルをダウンロードして読み込む"}
                </button>
                <button 
                  className="btn btn-secondary" 
                  onClick={handleOpenModelDir}
                  title="モデルが格納されているフォルダを開きます"
                >
                  フォルダを開く
                </button>
              </div>
              {downloadStatus && <div className="status-text" style={{ marginTop: '12px', color: downloadStatus.startsWith('Error') ? 'var(--danger)' : 'var(--success)' }}>{downloadStatus}</div>}
            </div>
          </section>

          <section className="settings-group">
            <h3>ナレッジベース (LanceDB)</h3>
            <div className="form-control">
              <label>データベースディレクトリ</label>
              <div className="input-with-button">
                <input 
                  type="text" 
                  placeholder="/path/to/lancedb" 
                  value={dbPath} 
                  onChange={e => onDbPathChange(e.target.value)} 
                />

                <button 
                  className="btn btn-secondary" 
                  onClick={handleSelectDbDir}
                >
                  参照
                </button>
              </div>
            </div>
            <div className="form-control">
              <label>埋め込みモデル</label>
              <select>
                <option>MultilingualE5Large (ローカル)</option>
              </select>
            </div>
          </section>

        </div>

        <footer className="settings-footer">
          <div className="settings-footer-content">
            <button className="btn btn-primary" onClick={onClose}>保存して終了</button>
          </div>
        </footer>
      </div>
    </div>
  );
};
