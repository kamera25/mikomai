import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import './SettingsPanel.css';

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({ isOpen, onClose }) => {
  const [modelPath, setModelPath] = useState("Qwen/Qwen2.5-0.5B-Instruct-GGUF");
  const [modelFilename, setModelFilename] = useState("qwen2.5-0.5b-instruct-q4_k_m.gguf");
  const [downloadStatus, setDownloadStatus] = useState("");
  const [isLoading, setIsLoading] = useState(false);

  const handleDownloadAndLoad = async () => {
    try {
      setIsLoading(true);
      setDownloadStatus("Downloading model from HuggingFace... (this may take a while)");
      
      const downloadedPath = await invoke<string>("download_model", {
        repo: modelPath,
        filename: modelFilename
      });
      
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
            <h3>ローカルLLM (llama.cpp)</h3>
            <div className="form-control">
              <label>HuggingFace リポジトリ</label>
              <input type="text" value={modelPath} onChange={e => setModelPath(e.target.value)} placeholder="Qwen/Qwen2.5-0.5B-Instruct-GGUF" />
            </div>
            <div className="form-control">
              <label>ファイル名 (GGUF)</label>
              <input type="text" value={modelFilename} onChange={e => setModelFilename(e.target.value)} placeholder="qwen2.5-0.5b-instruct-q4_k_m.gguf" />
            </div>
            <div className="form-control">
              <button 
                className="btn btn-secondary" 
                onClick={handleDownloadAndLoad}
                disabled={isLoading}
              >
                {isLoading ? "ダウンロード中..." : "モデルをダウンロードして読み込む"}
              </button>
              {downloadStatus && <div className="status-text" style={{ marginTop: '12px', color: downloadStatus.startsWith('Error') ? 'var(--danger)' : 'var(--success)' }}>{downloadStatus}</div>}
            </div>
          </section>

          <section className="settings-group">
            <h3>ナレッジベース (LanceDB)</h3>
            <div className="form-control">
              <label>データベースディレクトリ</label>
              <div className="input-with-button">
                <input type="text" placeholder="/path/to/lancedb" defaultValue="./data/knowledge.lance" />
                <button className="btn btn-secondary">参照</button>
              </div>
            </div>
            <div className="form-control">
              <label>埋め込みモデル</label>
              <select>
                <option>all-MiniLM-L6-v2 (ローカル ONNX)</option>
                <option>bge-base-en-v1.5 (ローカル ONNX)</option>
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
