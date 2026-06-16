import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { useSettingsContext } from '../contexts/SettingsContext';

import './SettingsPanel.css';

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({ 
  isOpen, 
  onClose
}) => {
  const {
    historyLimit,
    setHistoryLimit,
    temperature,
    setTemperature,
    repetitionPenalty,
    setRepetitionPenalty,
    modelPath: _savedModelPath,
    setModelPath,
    mcpTimeout,
    setMcpTimeout,
    cacheExpiryMinutes,
    setCacheExpiryMinutes,
    dbPath,
    setDbPath,
    ipVersion,
    setIpVersion,
    consolePort,
    setConsolePort,
    consoleBaudRate,
    setConsoleBaudRate,
    preloadInvestigate,
    setPreloadInvestigate,
    preloadKnowledge,
    setPreloadKnowledge,
    preloadAnalysis,
    setPreloadAnalysis,
    preloadRag,
    setPreloadRag,
    saveAllSettings,
  } = useSettingsContext();

  const [repoPath, setRepoPath] = useState("bartowski/google_gemma-4-E4B-it-GGUF");
  const [modelFilename, setModelFilename] = useState("google_gemma-4-E4B-it-Q4_K_M.gguf");
  const [availablePorts, setAvailablePorts] = useState<string[]>([]);

  // Update handlers
  const handleHistoryLimitChange = (val: number) => {
    setHistoryLimit(val);
    saveAllSettings({ historyLimit: val });
  };

  const handleTemperatureChange = (val: number) => {
    setTemperature(val);
    saveAllSettings({ temperature: val });
  };

  const handleRepetitionPenaltyChange = (val: number) => {
    setRepetitionPenalty(val);
    saveAllSettings({ repetitionPenalty: val });
  };

  const handleModelPathChange = (val: string) => {
    setModelPath(val);
    saveAllSettings({ modelPath: val });
  };

  const handleMcpTimeoutChange = (val: number) => {
    setMcpTimeout(val);
    saveAllSettings({ mcpTimeout: val });
  };

  const handleCacheExpiryMinutesChange = (val: number) => {
    setCacheExpiryMinutes(val);
    saveAllSettings({ cacheExpiryMinutes: val });
  };

  const handleDbPathChange = (val: string) => {
    setDbPath(val);
    saveAllSettings({ dbPath: val });
  };

  const handleIpVersionChange = (val: string) => {
    setIpVersion(val);
    saveAllSettings({ ipVersion: val });
  };

  const handleConsolePortChange = (val: string) => {
    setConsolePort(val);
    saveAllSettings({ consolePort: val });
  };

  const handleConsoleBaudRateChange = (val: number) => {
    setConsoleBaudRate(val);
    saveAllSettings({ consoleBaudRate: val });
  };

  const handlePreloadInvestigateChange = (val: boolean) => {
    setPreloadInvestigate(val);
    saveAllSettings({ preloadInvestigate: val });
  };

  const handlePreloadKnowledgeChange = (val: boolean) => {
    setPreloadKnowledge(val);
    saveAllSettings({ preloadKnowledge: val });
  };

  const handlePreloadAnalysisChange = (val: boolean) => {
    setPreloadAnalysis(val);
    saveAllSettings({ preloadAnalysis: val });
  };

  const handlePreloadRagChange = (val: boolean) => {
    setPreloadRag(val);
    saveAllSettings({ preloadRag: val });
  };


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
      
      handleModelPathChange(downloadedPath);
      setDownloadStatus(`Model downloaded to: ${downloadedPath}. Loading into memory...`);
      
      const loadResult = await invoke<string>("load_model", {
        path: downloadedPath
      });
      
      setDownloadStatus(`Success: ${loadResult}`);
    } catch (e: unknown) {
      setDownloadStatus(`Error: ${e instanceof Error ? e.message : String(e)}`);
    } finally {
      setIsLoading(false);
    }
  };

  const handleOpenModelDir = async () => {
    try {
      await invoke("open_model_dir", { modelPath: _savedModelPath });
    } catch (e: unknown) {
      setDownloadStatus(`Error: ${e instanceof Error ? e.message : String(e)}`);
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
        handleDbPathChange(selected as string);
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
              <div className="form-range-container">
                <input 
                  type="range" 
                  min="0" 
                  max="20" 
                  value={historyLimit} 
                  onChange={(e) => handleHistoryLimitChange(parseInt(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{historyLimit}</span>
              </div>
              <p className="help-text form-help-text">
                AIが回答を生成する際に参照する過去の会話の要約数です。大きくすると文脈をより理解しますが、メモリ消費量が増える可能性があります。
              </p>
            </div>
            <div className="form-control">
              <label>Temperature (温度)</label>
              <div className="form-range-container">
                <input 
                  type="range" 
                  min="0" 
                  max="2.0" 
                  step="0.1"
                  value={temperature} 
                  onChange={(e) => handleTemperatureChange(parseFloat(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{temperature.toFixed(1)}</span>
              </div>
              <p className="help-text form-help-text">
                回答のランダム性を制御します。0に設定すると最も決定的（同じ入力に対して同じ回答）になります。
              </p>
            </div>
            <div className="form-control">
              <label>Repetition Penalty (繰り返し抑制)</label>
              <div className="form-range-container">
                <input 
                  type="range" 
                  min="1.0" 
                  max="2.0" 
                  step="0.05"
                  value={repetitionPenalty} 
                  onChange={(e) => handleRepetitionPenaltyChange(parseFloat(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{repetitionPenalty.toFixed(2)}</span>
              </div>
              <p className="help-text form-help-text">
                同じ言葉の繰り返しを抑制します。1.0で無効、値を大きくするほど繰り返しが少なくなります。
              </p>
            </div>
            <div className="form-control">
              <label>MCP 実行タイムアウト (秒)</label>
              <div className="form-range-container">
                <input
                  type="range"
                  min="5"
                  max="120"
                  step="5"
                  value={mcpTimeout}
                  onChange={(e) => handleMcpTimeoutChange(parseInt(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{mcpTimeout}</span>
              </div>
              <p className="help-text form-help-text">
                ツール (MCP) 実行の最大待機時間です。時間を超えると処理を中断します。
              </p>
            </div>
            <div className="form-control">
              <label>キャッシュ有効時間 (分)</label>
              <div className="form-range-container">
                <input
                  type="range"
                  min="0"
                  max="60"
                  step="1"
                  value={cacheExpiryMinutes}
                  onChange={(e) => handleCacheExpiryMinutesChange(parseInt(e.target.value))}
                  className="form-range-input"
                />
                <span className="form-range-value">{cacheExpiryMinutes}</span>
              </div>
              <p className="help-text form-help-text">
                前回データ取得からの経過時間（分）です。この時間内であれば、実機に接続せず保存済みのYAMLデータから応答します。0にするとキャッシュを使用しません。
              </p>
            </div>
            <div className="form-control">
              <label htmlFor="ip-version-select">利用するインターネットプロトコルの指定</label>
              <select 
                id="ip-version-select"
                value={ipVersion} 
                onChange={(e) => handleIpVersionChange(e.target.value)}
              >
                <option value="auto">自動</option>
                <option value="ipv4">IPv4のみ</option>
                <option value="ipv6">IPv6のみ</option>
              </select>
              <p className="help-text form-help-text">
                ホスト名解決および接続時に使用するIPプロトコルの優先設定です。
              </p>
            </div>
            <div className="form-control">
              <label htmlFor="console-port-select">コンソールポート (MCP用)</label>
              <div className="download-actions">
                <select
                  id="console-port-select"
                  value={availablePorts.includes(consolePort || '') ? (consolePort || '') : (consolePort ? 'custom' : '')}
                  onChange={(e) => {
                    const val = e.target.value;
                    if (val === 'custom') {
                      handleConsolePortChange('/dev/ttyUSB0');
                    } else if (val === '') {
                      handleConsolePortChange('');
                    } else {
                      handleConsolePortChange(val);
                    }
                  }}
                  className="download-btn-stretch"
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
                    onChange={(e) => handleConsolePortChange(e.target.value)}
                    placeholder="例: /dev/ttyUSB0, COM1"
                    className="custom-port-input"
                  />
                ) : null}
              </div>
              <p className="help-text form-help-text">
                指定されている場合、MCPの呼び出し(fetch_arp, config, routing等)は内部でこのシリアルコンソール経由で実行されます。
              </p>
            </div>
            {(consolePort && consolePort !== '') && (
              <div className="form-control">
                <label htmlFor="console-baudrate-select">コンソールボーレート</label>
                <select
                  id="console-baudrate-select"
                  value={consoleBaudRate}
                  onChange={(e) => handleConsoleBaudRateChange(parseInt(e.target.value))}
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
              <label>起動時・モデル読込時のKVキャッシュ プリロード設定</label>
              <p className="help-text form-help-text margin-bottom">
                チェックを外したワーカーは、初回呼出時（遅延読込）にキャッシュを生成するため、起動時のメモリ消費量と起動時間を削減できます。
              </p>
              <div className="preload-grid">
                <label className="preload-label">
                  <input type="checkbox" checked={preloadInvestigate} onChange={(e) => handlePreloadInvestigateChange(e.target.checked)} />
                  Investigator (調査員)
                </label>
                <label className="preload-label">
                  <input type="checkbox" checked={preloadKnowledge} onChange={(e) => handlePreloadKnowledgeChange(e.target.checked)} />
                  Knowledge (知識専門家)
                </label>
                <label className="preload-label">
                  <input type="checkbox" checked={preloadAnalysis} onChange={(e) => handlePreloadAnalysisChange(e.target.checked)} />
                  Analyst (分析官)
                </label>
                <label className="preload-label">
                  <input type="checkbox" checked={preloadRag} onChange={(e) => handlePreloadRagChange(e.target.checked)} />
                  RAG Worker (RAG回答員)
                </label>
              </div>
            </div>
            <div className="form-control">
              <div className="download-actions">
                <button 
                  className="btn btn-secondary download-btn-stretch" 
                  onClick={handleDownloadAndLoad}
                  disabled={isLoading}
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
              {downloadStatus && <div className="status-text download-status-container" style={{ color: downloadStatus.startsWith('Error') ? 'var(--danger)' : 'var(--success)' }}>{downloadStatus}</div>}
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
                  onChange={e => handleDbPathChange(e.target.value)} 
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
