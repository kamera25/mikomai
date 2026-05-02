import React, { useState } from 'react';
import './ScheduledTasksPanel.css';

interface ScheduledTasksPanelProps {
  onClose: () => void;
}

interface ScheduledTask {
  id: string;
  name: string;
  status: 'running' | 'stopped' | 'disabled';
  schedule: string;
  lastRun: string;
  prompt: string;
}

const initialMockTasks: ScheduledTask[] = [
  { id: '1', name: 'バックアップ取得 (Core-Switch)', status: 'running', schedule: '毎日 03:00', lastRun: '2024-05-02 03:00', prompt: 'Core-Switchに対して running-config のバックアップを取得し、指定のサーバーに保存してください。' },
  { id: '2', name: 'インターフェース状態監視', status: 'running', schedule: '5分おき', lastRun: '2024-05-02 19:55', prompt: '全インターフェースのステータスを確認し、Downしているものがあれば通知してください。' },
  { id: '3', name: '構成不整合チェック', status: 'stopped', schedule: '毎週月曜 09:00', lastRun: '2024-04-29 09:00', prompt: '現在の構成と標準テンプレートを比較し、差分をレポートしてください。' },
  { id: '4', name: 'セキュリティログ転送', status: 'running', schedule: 'リアルタイム', lastRun: '2024-05-02 19:58', prompt: '拒否されたパケットのログをリアルタイムでセキュリティ分析チームに転送してください。' },
  { id: '5', name: '旧型番デバイス棚卸', status: 'disabled', schedule: '毎月1日 00:00', lastRun: '2024-05-01 00:00', prompt: 'ネットワーク内のデバイス型番をスキャンし、サポート終了が近いものをリストアップしてください。' },
];

export const ScheduledTasksPanel: React.FC<ScheduledTasksPanelProps> = ({ onClose }) => {
  const [tasks, setTasks] = useState<ScheduledTask[]>(initialMockTasks);
  const [searchQuery, setSearchQuery] = useState('');
  const [editingTask, setEditingTask] = useState<ScheduledTask | null>(null);

  const filteredTasks = tasks.filter(task => 
    task.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    task.schedule.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleSaveTask = () => {
    if (editingTask) {
      setTasks(prev => prev.map(t => t.id === editingTask.id ? editingTask : t));
      setEditingTask(null);
    }
  };

  const getStatusLabel = (status: string) => {
    switch (status) {
      case 'running': return '実行中';
      case 'stopped': return '停止中';
      case 'disabled': return '無効化';
      default: return status;
    }
  };

  return (
    <div className="scheduled-tasks-overlay">
      <div className="scheduled-tasks-panel">
        <header className="scheduled-header">
          <div className="header-title-container">
            <h2>定期実行設定</h2>
          </div>
          <button className="panel-close-btn" onClick={onClose}>&times;</button>
        </header>

        <div className="scheduled-toolbar">
          <div className="toolbar-left">
            <span className="results-count">
              <strong>{filteredTasks.length}</strong> / <strong>{tasks.length}</strong> 件のタスクを表示
            </span> 
            <div className="search-box-container">
              <svg className="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
              <input 
                type="text" 
                placeholder="タスクを検索..." 
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          </div>
          <div className="toolbar-right">
            <button className="toolbar-btn">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 6h18M3 12h18M3 18h18"></path></svg>
              表示設定
            </button>
          </div>
        </div>

        <div className="scheduled-table-wrapper">
          <table className="scheduled-table">
            <thead>
              <tr>
                <th className="col-checkbox">-</th>
                <th>タスク名称</th>
                <th>状態</th>
                <th>実行タイミング</th>
                <th>最終実行時刻</th>
              </tr>
            </thead>
            <tbody>
              {filteredTasks.map(task => (
                <tr key={task.id}>
                  <td className="col-checkbox">
                    <input type="checkbox" className="task-checkbox" />
                  </td>
                  <td>
                    <div className="task-name-cell">
                      <div className="task-icon">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"></circle><polyline points="12 6 12 12 16 14"></polyline></svg>
                      </div>
                      <span 
                        className="task-name-text" 
                        onClick={() => setEditingTask({ ...task })}
                      >
                        {task.name}
                      </span>
                    </div>
                  </td>
                  <td>
                    <span className={`status-badge ${task.status}`}>
                      {getStatusLabel(task.status)}
                    </span>
                  </td>
                  <td>{task.schedule}</td>
                  <td>{task.lastRun}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {editingTask && (
          <div className="task-settings-modal-overlay">
            <div className="task-settings-card">
              <header className="settings-card-header">
                <h3>タスク詳細設定</h3>
                <button className="close-card-btn" onClick={() => setEditingTask(null)}>&times;</button>
              </header>
              <div className="settings-card-body">
                <div className="settings-form-group">
                  <label>タスク名称</label>
                  <input 
                    type="text" 
                    value={editingTask.name} 
                    onChange={(e) => setEditingTask({ ...editingTask, name: e.target.value })}
                  />
                </div>
                <div className="settings-form-group">
                  <label>実行タイミング</label>
                  <input 
                    type="text" 
                    value={editingTask.schedule} 
                    onChange={(e) => setEditingTask({ ...editingTask, schedule: e.target.value })}
                  />
                </div>
                <div className="settings-form-group">
                  <label>プロンプト (表示専用)</label>
                  <textarea 
                    value={editingTask.prompt} 
                    readOnly 
                    className="readonly-prompt-area"
                  />
                  <p className="field-hint">※ プロンプトの内容はシステムによって管理されており、変更できません。</p>
                </div>
              </div>
              <footer className="settings-card-footer">
                <button className="settings-cancel-btn" onClick={() => setEditingTask(null)}>キャンセル</button>
                <button className="settings-save-btn" onClick={handleSaveTask}>保存</button>
              </footer>
            </div>
          </div>
        )}

        <footer className="scheduled-panel-footer">
          <button className="add-task-btn">新規タスク追加</button>
          <button className="delete-selected-btn" style={{ padding: '12px 32px', backgroundColor: '#ef4444', color: 'white', border: 'none', borderRadius: '8px', fontWeight: 600, cursor: 'pointer' }}>削除</button>
        </footer>
      </div>
    </div>
  );
};
