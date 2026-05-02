import React from 'react';
import './SettingsPanel.css';

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
}

export const SettingsPanel: React.FC<SettingsPanelProps> = ({ isOpen, onClose }) => {
  if (!isOpen) return null;

  return (
    <div className="settings-overlay">
      <div className="settings-panel">
        <div className="settings-header">
          <h2>Agent Configuration</h2>
          <button className="close-button" onClick={onClose}>&times;</button>
        </div>

        <div className="settings-body">
          <div className="settings-group">
            <h3>Local LLM (llama.cpp)</h3>
            <div className="form-control">
              <label>Model Path</label>
              <div className="input-with-button">
                <input type="text" placeholder="/path/to/model.gguf" defaultValue="~/.cache/huggingface/hub/models--Qwen--Qwen2.5-7B-Instruct-GGUF" />
                <button className="btn btn-secondary">Browse</button>
              </div>
              <small>Model will be downloaded automatically if not found.</small>
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
