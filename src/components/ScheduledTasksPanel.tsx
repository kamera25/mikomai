import React, { useState, useEffect } from 'react';
import './ScheduledTasksPanel.css';

interface ScheduledTasksPanelProps {
  onClose: () => void;
}

type ScheduleType = 'weekly' | 'daily' | 'hourly' | 'minutely' | 'secondly' | 'custom';

const ScheduleInput: React.FC<{ value: string; onChange: (val: string) => void }> = ({ value, onChange }) => {
  const [type, setType] = useState<ScheduleType>('custom');
  const [dayOfWeek, setDayOfWeek] = useState('月曜');
  const [time, setTime] = useState('00:00');
  const [minute, setMinute] = useState('0');
  const [second, setSecond] = useState('0');
  const [customText, setCustomText] = useState('');

  useEffect(() => {
    if (value.startsWith('毎週') && value.includes(' ')) {
      setType('weekly');
      const parts = value.split(' ');
      setDayOfWeek(parts[0].replace('毎週', ''));
      setTime(parts[1]);
    } else if (value.startsWith('毎日 ')) {
      setType('daily');
      setTime(value.replace('毎日 ', ''));
    } else if (value.startsWith('毎時 ') && value.endsWith('分')) {
      setType('hourly');
      setMinute(value.replace('毎時 ', '').replace('分', ''));
    } else if (value.startsWith('毎分 ') && value.endsWith('秒')) {
      setType('minutely');
      setSecond(value.replace('毎分 ', '').replace('秒', ''));
    } else if (value === '毎秒') {
      setType('secondly');
    } else {
      setType('custom');
      setCustomText(value);
    }
  }, [value]);

  const handleChange = (
    newType: ScheduleType,
    newDay: string,
    newTime: string,
    newMin: string,
    newSec: string,
    newCustom: string
  ) => {
    setType(newType);
    setDayOfWeek(newDay);
    setTime(newTime);
    setMinute(newMin);
    setSecond(newSec);
    setCustomText(newCustom);

    let newValue = '';
    if (newType === 'weekly') {
      newValue = `毎週${newDay} ${newTime}`;
    } else if (newType === 'daily') {
      newValue = `毎日 ${newTime}`;
    } else if (newType === 'hourly') {
      newValue = `毎時 ${newMin}分`;
    } else if (newType === 'minutely') {
      newValue = `毎分 ${newSec}秒`;
    } else if (newType === 'secondly') {
      newValue = `毎秒`;
    } else {
      newValue = newCustom;
    }
    onChange(newValue);
  };

  return (
    <div className="schedule-input-container">
      <select value={type} onChange={(e) => handleChange(e.target.value as ScheduleType, dayOfWeek, time, minute, second, customText)} className="schedule-type-select">
        <option value="weekly">週次</option>
        <option value="daily">毎日</option>
        <option value="hourly">毎時</option>
        <option value="minutely">毎分</option>
        <option value="secondly">毎秒</option>
        <option value="custom">カスタム</option>
      </select>

      {type === 'weekly' && (
        <>
          <select value={dayOfWeek} onChange={(e) => handleChange(type, e.target.value, time, minute, second, customText)} className="schedule-day-select">
            <option value="月曜">月曜</option>
            <option value="火曜">火曜</option>
            <option value="水曜">水曜</option>
            <option value="木曜">木曜</option>
            <option value="金曜">金曜</option>
            <option value="土曜">土曜</option>
            <option value="日曜">日曜</option>
          </select>
          <input type="time" value={time} onChange={(e) => handleChange(type, dayOfWeek, e.target.value, minute, second, customText)} />
        </>
      )}

      {type === 'daily' && (
        <input type="time" value={time} onChange={(e) => handleChange(type, dayOfWeek, e.target.value, minute, second, customText)} />
      )}

      {type === 'hourly' && (
        <div className="schedule-flex-input">
          <input type="number" min="0" max="59" value={minute} onChange={(e) => handleChange(type, dayOfWeek, time, e.target.value, second, customText)} />
          <span>分</span>
        </div>
      )}

      {type === 'minutely' && (
        <div className="schedule-flex-input">
          <input type="number" min="0" max="59" value={second} onChange={(e) => handleChange(type, dayOfWeek, time, minute, e.target.value, customText)} />
          <span>秒</span>
        </div>
      )}

      {type === 'custom' && (
        <input type="text" value={customText} onChange={(e) => handleChange(type, dayOfWeek, time, minute, second, e.target.value)} placeholder="例: 毎月1日 00:00" />
      )}
    </div>
  );
};

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
  const [isNewTask, setIsNewTask] = useState(false);

  const filteredTasks = tasks.filter(task => 
    task.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
    task.schedule.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleSaveTask = () => {
    if (editingTask) {
      if (isNewTask) {
        setTasks(prev => [...prev, editingTask]);
      } else {
        setTasks(prev => prev.map(t => t.id === editingTask.id ? editingTask : t));
      }
      setEditingTask(null);
      setIsNewTask(false);
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
                        onClick={() => {
                          setEditingTask({ ...task });
                          setIsNewTask(false);
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
                <button className="close-card-btn" onClick={() => { setEditingTask(null); setIsNewTask(false); }}>&times;</button>
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
                  <ScheduleInput
                    value={editingTask.schedule} 
                    onChange={(val) => setEditingTask({ ...editingTask, schedule: val })}
                  />
                </div>
                <div className="settings-form-group">
                  <label>プロンプト {isNewTask ? '' : '(表示専用)'}</label>
                  <textarea 
                    value={editingTask.prompt} 
                    readOnly={!isNewTask}
                    onChange={(e) => setEditingTask({ ...editingTask, prompt: e.target.value })}
                    className={isNewTask ? "" : "readonly-prompt-area"}
                  />
                  {!isNewTask && (
                    <p className="field-hint">※ プロンプトの内容はシステムによって管理されており、変更できません。</p>
                  )}
                </div>
              </div>
              <footer className="settings-card-footer">
                <button className="settings-cancel-btn" onClick={() => { setEditingTask(null); setIsNewTask(false); }}>キャンセル</button>
                <button className="settings-save-btn" onClick={handleSaveTask}>保存</button>
              </footer>
            </div>
          </div>
        )}

        <footer className="scheduled-panel-footer">
          <button className="add-task-btn" onClick={() => {
            setIsNewTask(true);
            setEditingTask({
              id: Date.now().toString(),
              name: '',
              status: 'stopped',
              schedule: '毎日 00:00',
              lastRun: '-',
              prompt: ''
            });
          }}>新規タスク追加</button>
          <button className="delete-selected-btn" style={{ padding: '12px 32px', backgroundColor: '#ef4444', color: 'white', border: 'none', borderRadius: '8px', fontWeight: 600, cursor: 'pointer' }}>削除</button>
        </footer>
      </div>
    </div>
  );
};
