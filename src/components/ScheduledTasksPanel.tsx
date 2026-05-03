import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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

export const ScheduledTasksPanel: React.FC<ScheduledTasksPanelProps> = ({ onClose }) => {
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [editingTask, setEditingTask] = useState<ScheduledTask | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [selectedTasks, setSelectedTasks] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadTasks();

    const unlisten = listen('task-executed', (event) => {
      console.log('Task executed:', event.payload);
      loadTasks(); // reload to get updated lastRun
    });

    return () => {
      unlisten.then(f => f());
    };
  }, []);

  const loadTasks = async () => {
    try {
      const loadedTasks = await invoke<ScheduledTask[]>('load_scheduled_tasks');
      setTasks(loadedTasks);
    } catch (error) {
      console.error('Failed to load tasks:', error);
    }
  };

  const filteredTasks = tasks.filter(task => 
    task.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    task.schedule.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleSaveTask = async () => {
    if (editingTask) {
      try {
        if (isCreating) {
          await invoke('add_scheduled_task', {
            name: editingTask.name,
            schedule: editingTask.schedule,
            prompt: editingTask.prompt
          });
        } else {
          await invoke('update_scheduled_task', { task: editingTask });
        }
        setEditingTask(null);
        setIsCreating(false);
        loadTasks();
      } catch (error) {
        console.error('Failed to save task:', error);
      }
    }
  };

  const handleDeleteSelected = async () => {
    try {
      for (const id of selectedTasks) {
        await invoke('delete_scheduled_task', { id });
      }
      setSelectedTasks(new Set());
      loadTasks();
    } catch (error) {
      console.error('Failed to delete tasks:', error);
    }
  };

  const handleToggleSelect = (id: string) => {
    const newSelected = new Set(selectedTasks);
    if (newSelected.has(id)) {
      newSelected.delete(id);
    } else {
      newSelected.add(id);
    }
    setSelectedTasks(newSelected);
  };

  const handleCreateNew = () => {
    setEditingTask({
      id: '',
      name: '新規タスク',
      status: 'running',
      schedule: '* * * * * *',
      lastRun: '-',
      prompt: 'プロンプトを入力してください。'
    });
    setIsCreating(true);
  };

  const handleExecuteNow = async (id: string) => {
    try {
      await invoke('execute_task', { id });
      loadTasks();
    } catch (error) {
      console.error('Failed to execute task:', error);
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
            <button className="toolbar-btn" onClick={loadTasks}>
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="23 4 23 10 17 10"></polyline><polyline points="1 20 1 14 7 14"></polyline><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path></svg>
              更新
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
                <th>実行タイミング(Cron)</th>
                <th>最終実行時刻</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {filteredTasks.map(task => (
                <tr key={task.id}>
                  <td className="col-checkbox">
                    <input
                      type="checkbox"
                      className="task-checkbox"
                      checked={selectedTasks.has(task.id)}
                      onChange={() => handleToggleSelect(task.id)}
                    />
                  </td>
                  <td>
                    <div className="task-name-cell">
                      <div className="task-icon">
                        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"></circle><polyline points="12 6 12 12 16 14"></polyline></svg>
                      </div>
                      <span 
                        className="task-name-text" 
                        onClick={() => {
                          setEditingTask({ ...task });
                          setIsCreating(false);
                        }}
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
                  <td>
                    <button
                      onClick={() => handleExecuteNow(task.id)}
                      style={{ padding: '4px 8px', fontSize: '12px', cursor: 'pointer' }}
                    >
                      手動実行
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {editingTask && (
          <div className="task-settings-modal-overlay">
            <div className="task-settings-card">
              <header className="settings-card-header">
                <h3>{isCreating ? '新規タスク作成' : 'タスク詳細設定'}</h3>
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
                  <label>実行タイミング (Cron)</label>
                  <input 
                    type="text" 
                    value={editingTask.schedule} 
                    onChange={(e) => setEditingTask({ ...editingTask, schedule: e.target.value })}
                  />
                </div>
                <div className="settings-form-group">
                  <label>状態</label>
                  <select
                    value={editingTask.status}
                    onChange={(e) => setEditingTask({ ...editingTask, status: e.target.value as any })}
                    style={{ width: '100%', padding: '8px', borderRadius: '4px', border: '1px solid #ccc', backgroundColor: '#1e1e1e', color: 'white' }}
                  >
                    <option value="running">実行中</option>
                    <option value="stopped">停止中</option>
                    <option value="disabled">無効化</option>
                  </select>
                </div>
                <div className="settings-form-group">
                  <label>プロンプト</label>
                  <textarea 
                    value={editingTask.prompt} 
                    readOnly={!isCreating}
                    onChange={(e) => isCreating && setEditingTask({ ...editingTask, prompt: e.target.value })}
                    className={!isCreating ? "readonly-prompt-area" : ""}
                    style={isCreating ? { width: '100%', minHeight: '80px', padding: '8px', borderRadius: '4px', border: '1px solid #ccc', backgroundColor: '#1e1e1e', color: 'white' } : {}}
                  />
                  {!isCreating && <p className="field-hint">※ プロンプトの内容はシステムによって管理されており、変更できません。</p>}
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
          <button className="add-task-btn" onClick={handleCreateNew}>新規タスク追加</button>
          <button
            className="delete-selected-btn"
            onClick={handleDeleteSelected}
            disabled={selectedTasks.size === 0}
            style={{
              padding: '12px 32px',
              backgroundColor: selectedTasks.size > 0 ? '#ef4444' : '#6b7280',
              color: 'white',
              border: 'none',
              borderRadius: '8px',
              fontWeight: 600,
              cursor: selectedTasks.size > 0 ? 'pointer' : 'not-allowed'
            }}
          >
            削除
          </button>
        </footer>
      </div>
    </div>
  );
};
