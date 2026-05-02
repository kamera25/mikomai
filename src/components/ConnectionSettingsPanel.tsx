import React, { useState } from 'react';
import './ConnectionSettingsPanel.css';

interface ConnectionSettingsPanelProps {
  onClose: () => void;
}

interface Connection {
  id: string;
  status: 'online' | 'offline';
  hostname: string;
  ip: string;
  type: string;
  lastConnected: string;
}

const mockConnections: Connection[] = [
  { id: '1', status: 'online', hostname: 'Core-Switch-01', ip: '192.168.1.1', type: 'SSH (Cisco IOS)', lastConnected: '2024-05-02 14:20' },
  { id: '2', status: 'offline', hostname: 'Edge-Router-02', ip: '192.168.2.1', type: 'SSH (Juniper JunOS)', lastConnected: '2024-04-30 09:15' },
  { id: '3', status: 'online', hostname: 'Dist-Switch-03', ip: '192.168.1.10', type: 'Telnet (Arista)', lastConnected: '2024-05-02 17:45' },
  { id: '4', status: 'online', hostname: 'Server-Farm-01', ip: '10.0.5.50', type: 'SSH (Ubuntu)', lastConnected: '2024-05-01 22:10' },
  { id: '5', status: 'offline', hostname: 'Backup-Router', ip: '172.16.0.1', type: 'SSH (Cisco XE)', lastConnected: 'Never' },
  { id: '6', status: 'online', hostname: 'Access-Point-04', ip: '192.168.5.25', type: 'SSH (Aruba)', lastConnected: '2024-05-02 10:30' },
];

export const ConnectionSettingsPanel: React.FC<ConnectionSettingsPanelProps> = ({ onClose }) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [isEditing, setIsEditing] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [errors, setErrors] = useState<{ [key: string]: string }>({});

  const [formData, setFormData] = useState({
    hostname: '',
    ip: '',
    type: 'SSH',
    username: '',
    password: '',
    passphrase: '',
    rememberPassword: true,
    agentForwarding: false,
    authMethod: 'plain',
    privateKeyPath: '',
    consolePort: 'COM1',
    baudRate: 9600
  });

  const filteredConnections = mockConnections.filter(conn => 
    conn.hostname.toLowerCase().includes(searchQuery.toLowerCase()) ||
    conn.ip.includes(searchQuery) ||
    conn.type.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleEdit = (conn: Connection) => {
    setEditingId(conn.id);
    setFormData({
      hostname: conn.hostname,
      ip: conn.ip,
      type: conn.type.split(' ')[0] as any,
      username: 'root', // Mock data doesn't have these, using defaults
      password: '',
      passphrase: '',
      rememberPassword: true,
      agentForwarding: false,
      authMethod: 'plain',
      privateKeyPath: '',
      consolePort: 'COM1',
      baudRate: 9600
    });
    setErrors({});
    setIsEditing(true);
  };

  const handleAddHost = () => {
    setEditingId(null);
    setFormData({
      hostname: '',
      ip: '',
      type: 'SSH',
      username: '',
      password: '',
      passphrase: '',
      rememberPassword: true,
      agentForwarding: false,
      authMethod: 'plain',
      privateKeyPath: '',
      consolePort: 'COM1',
      baudRate: 9600
    });
    setErrors({});
    setIsEditing(true);
  };

  const validate = () => {
    const newErrors: { [key: string]: string } = {};
    if (formData.type !== 'Console' && !formData.ip.trim()) {
      newErrors.ip = 'IPアドレスまたはホスト名を入力してください';
    }
    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleSave = () => {
    if (validate()) {
      console.log('Saving connection:', formData);
      // In a real app, we would update the state or call an API here
      setIsEditing(false);
    }
  };

  const renderForm = () => (
    <div className="connection-form-container">
      <div className="connection-form-content">
        <div className="form-section">
          <h3>基本設定</h3>
          <div className="form-grid">
            <div className="form-group">
              <label>接続ホスト名 (表示用)</label>
              <input 
                type="text" 
                value={formData.hostname} 
                onChange={(e) => setFormData({...formData, hostname: e.target.value})}
                placeholder="例: Core-Switch-01"
              />
            </div>
            <div className="form-group">
              <label>接続方式</label>
              <select 
                value={formData.type} 
                onChange={(e) => setFormData({...formData, type: e.target.value})}
              >
                <option value="SSH">SSH</option>
                <option value="Telnet">Telnet</option>
                <option value="Console">Console (Serial)</option>
              </select>
            </div>
            
            {formData.type !== 'Console' ? (
              <>
                <div className="form-group full-width">
                  <label>IPアドレス / ホスト名 <span style={{color: '#ef4444'}}>*</span></label>
                  <input 
                    type="text" 
                    className={errors.ip ? 'error' : ''}
                    value={formData.ip} 
                    onChange={(e) => setFormData({...formData, ip: e.target.value})}
                    placeholder="192.168.1.1 or router.local"
                  />
                  {errors.ip && <span className="error-message">{errors.ip}</span>}
                </div>
                <div className="form-group">
                  <label>ユーザ名</label>
                  <input 
                    type="text" 
                    value={formData.username} 
                    onChange={(e) => setFormData({...formData, username: e.target.value})}
                  />
                </div>
                <div className="form-group">
                  <label>パスワード</label>
                  <input 
                    type="password" 
                    value={formData.password} 
                    onChange={(e) => setFormData({...formData, password: e.target.value})}
                  />
                </div>
              </>
            ) : (
              <>
                <div className="form-group">
                  <label>シリアルポート</label>
                  <input 
                    type="text" 
                    value={formData.consolePort} 
                    onChange={(e) => setFormData({...formData, consolePort: e.target.value})}
                    placeholder="COM1 or /dev/ttyUSB0"
                  />
                </div>
                <div className="form-group">
                  <label>ボーレート</label>
                  <select 
                    value={formData.baudRate} 
                    onChange={(e) => setFormData({...formData, baudRate: parseInt(e.target.value)})}
                  >
                    <option value="9600">9600</option>
                    <option value="19200">19200</option>
                    <option value="38400">38400</option>
                    <option value="57600">57600</option>
                    <option value="115200">115200</option>
                  </select>
                </div>
              </>
            )}
          </div>
        </div>

        {formData.type === 'SSH' && (
          <div className="form-section">
            <h3>SSH認証設定</h3>
            <div className="ssh-auth-grid">
              <div className="form-group">
                <label>パスフレーズ</label>
                <input 
                  type="password" 
                  value={formData.passphrase} 
                  onChange={(e) => setFormData({...formData, passphrase: e.target.value})}
                />
              </div>

              <div className="ssh-checkbox-group">
                <label className="checkbox-item">
                  <input 
                    type="checkbox" 
                    checked={formData.rememberPassword} 
                    onChange={(e) => setFormData({...formData, rememberPassword: e.target.checked})}
                  />
                  パスワードをメモリ上に記憶する
                </label>
                <label className="checkbox-item">
                  <input 
                    type="checkbox" 
                    checked={formData.agentForwarding} 
                    onChange={(e) => setFormData({...formData, agentForwarding: e.target.checked})}
                  />
                  エージェント転送する
                </label>
              </div>

              <div className="auth-methods-list">
                <div className="auth-method-item">
                  <input 
                    type="radio" 
                    name="authMethod" 
                    checked={formData.authMethod === 'plain'} 
                    onChange={() => setFormData({...formData, authMethod: 'plain'})}
                  />
                  <div className="auth-method-content">
                    <span className="auth-method-label">プレインパスワードを使う</span>
                  </div>
                </div>

                <div className="auth-method-item">
                  <input 
                    type="radio" 
                    name="authMethod" 
                    checked={formData.authMethod === 'key'} 
                    onChange={() => setFormData({...formData, authMethod: 'key'})}
                  />
                  <div className="auth-method-content">
                    <span className="auth-method-label">RSA/DSA/ECDSA/ED25519鍵を使う</span>
                    <div className="auth-method-details">
                      <button className="btn-file-select">秘密鍵(K):</button>
                      <input 
                        type="text" 
                        className="path-input" 
                        placeholder="鍵ファイルのパス" 
                        value={formData.privateKeyPath}
                        onChange={(e) => setFormData({...formData, privateKeyPath: e.target.value})}
                        disabled={formData.authMethod !== 'key'}
                      />
                    </div>
                  </div>
                </div>

                <div className="auth-method-item">
                  <input 
                    type="radio" 
                    name="authMethod" 
                    checked={formData.authMethod === 'keyboard'} 
                    onChange={() => setFormData({...formData, authMethod: 'keyboard'})}
                  />
                  <div className="auth-method-content">
                    <span className="auth-method-label">キーボードインタラクティブ認証を使う</span>
                  </div>
                </div>

                <div className="auth-method-item">
                  <input 
                    type="radio" 
                    name="authMethod" 
                    checked={formData.authMethod === 'pageant'} 
                    onChange={() => setFormData({...formData, authMethod: 'pageant'})}
                  />
                  <div className="auth-method-content">
                    <span className="auth-method-label">Pageantを使う</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        )}
      </div>
      <footer className="form-footer">
        <button className="btn-cancel" onClick={() => setIsEditing(false)}>キャンセル</button>
        <button className="btn-save" onClick={handleSave}>{editingId ? '変更を保存' : 'ホストを登録'}</button>
      </footer>
    </div>
  );

  return (
    <div className="connection-settings-overlay">
      <div className="connection-settings-panel">
        <header className="connection-header-new">
          <div className="header-title-container">
            <h2>{isEditing ? (editingId ? '接続の編集' : '新規ホスト追加') : '接続設定'}</h2>
          </div>
          <button className="panel-close-btn" onClick={onClose}>&times;</button>
        </header>

        {isEditing ? renderForm() : (
          <>
            <div className="connection-toolbar">
              <div className="toolbar-left">
                <span className="results-count">
                  <strong>{filteredConnections.length}</strong> / <strong>{mockConnections.length}</strong> ホストを表示
                </span>
                <div className="search-box-container">
                  <svg className="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
                  <input 
                    type="text" 
                    placeholder="ホストを検索…" 
                    value={searchQuery}
                    onChange={(e) => setSearchQuery(e.target.value)}
                  />
                </div>
              </div>
              <div className="toolbar-right">
                <button className="toolbar-btn">
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 6h18M3 12h18M3 18h18"></path></svg>
                  表示設定
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
                </button>
                <div className="csv-actions">
                  <button className="toolbar-btn csv-btn">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
                    CSVインポート
                  </button>
                  <button className="toolbar-btn csv-btn">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="17 8 12 3 7 8"></polyline><line x1="12" y1="3" x2="12" y2="15"></line></svg>
                    CSVエクスポート
                  </button>
                </div>
              </div>
            </div>

            <div className="connection-table-wrapper">
              <table className="connection-table">
                <thead>
                  <tr>
                    <th className="col-status">-</th>
                    <th className="col-hostname">接続ホスト名</th>
                    <th className="col-ip">IP</th>
                    <th className="col-type">接続方式</th>
                    <th className="col-last">最後の接続時刻</th>
                    
                  </tr>
                </thead>
                <tbody>
                  {filteredConnections.map(conn => (
                    <tr key={conn.id}>
                      <td className="col-status">
                        <input type="checkbox" className="access-checkbox" defaultChecked={conn.status === 'online'} />
                      </td>
                      <td className="col-hostname">
                        <div className="hostname-cell">
                          <div className="device-icon">
                            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect><rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect><line x1="6" y1="6" x2="6.01" y2="6"></line><line x1="6" y1="18" x2="6.01" y2="18"></line></svg>
                          </div>
                                                    <span className="hostname-text" onClick={() => handleEdit(conn)}>{conn.hostname}</span>
                        </div>
                      </td>
                      <td className="col-ip">{conn.ip}</td>
                      <td className="col-type">
                        <div className="type-badge">
                          {conn.type.split(' ')[0]}
                        </div>
                        <span className="type-detail">{conn.type.split(' ').slice(1).join(' ') || ''}</span>
                      </td>
                      <td className="col-last">{conn.lastConnected}</td>

                    </tr>
                  ))}
                </tbody>
              </table>
            </div>

            <footer className="connection-panel-footer">
              <button className="add-device-btn" onClick={handleAddHost}>ホスト追加</button>
              <button className="delete-selected-btn">削除</button>
            </footer>
          </>
        )}
      </div>
    </div>
  );
};
