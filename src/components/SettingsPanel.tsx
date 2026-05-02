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
    <div className="settings-overlay">
      <div className="settings-panel">
        <div className="settings-header">
          <h2>mikomai Configuration</h2>
          <button className="close-button" onClick={onClose}>&times;</button>
        </div>

        <div className="settings-body">
          <div className="settings-group">
            <h3>Local LLM (llama.cpp)</h3>
            <div className="form-control">
              <label>HuggingFace Repo</label>
              <input type="text" value={modelPath} onChange={e => setModelPath(e.target.value)} placeholder="Qwen/Qwen2.5-0.5B-Instruct-GGUF" />
            </div>
            <div className="form-control">
              <label>Filename (GGUF)</label>
              <input type="text" value={modelFilename} onChange={e => setModelFilename(e.target.value)} placeholder="qwen2.5-0.5b-instruct-q4_k_m.gguf" />
            </div>
            <div className="form-control">
              <button 
                className="btn btn-secondary" 
                onClick={handleDownloadAndLoad}
                disabled={isLoading}
              >
                {isLoading ? "Downloading..." : "Download & Load Model"}
              </button>
              {downloadStatus && <small className="status-text" style={{ marginTop: '8px', color: downloadStatus.startsWith('Error') ? 'var(--danger)' : 'var(--success)' }}>{downloadStatus}</small>}
            </div>
          </div>

          <div className="settings-group">
            <h3>Knowledge Base (LanceDB)</h3>
            <div className="form-control">
              <label>Database Directory</label>
              <div className="input-with-button">
                <input type="text" placeholder="/path/to/lancedb" defaultValue="./data/knowledge.lance" />
                <button className="btn btn-secondary">Browse</button>
              </div>
            </div>
            <div className="form-control">
              <label>Embedding Model</label>
              <select>
                <option>all-MiniLM-L6-v2 (Local ONNX)</option>
                <option>bge-base-en-v1.5 (Local ONNX)</option>
              </select>
            </div>
          </div>

          <div className="settings-group">
            <h3>Network Credentials</h3>
            <div className="form-control">
              <label>Default Username</label>
              <input type="text" placeholder="admin" />
            </div>
            <div className="form-control">
              <label>Default Password</label>
              <input type="password" placeholder="••••••••" />
            </div>
            <small className="warning-text">Credentials are stored locally in the secure enclave.</small>
          </div>
        </div>

        <div className="settings-footer">
          <button className="btn btn-primary" onClick={onClose}>Save Settings</button>
        </div>
      </div>
    </div>
  );
};
