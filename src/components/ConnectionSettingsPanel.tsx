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

  const filteredConnections = mockConnections.filter(conn => 
    conn.hostname.toLowerCase().includes(searchQuery.toLowerCase()) ||
    conn.ip.includes(searchQuery) ||
    conn.type.toLowerCase().includes(searchQuery.toLowerCase())
  );

  return (
    <div className="connection-settings-overlay">
      <div className="connection-settings-panel">
        <header className="connection-header-new">
          <div className="header-title-container">
            <h2>接続設定</h2>
          </div>
          <button className="panel-close-btn" onClick={onClose}>&times;</button>
        </header>

        <div className="connection-toolbar">
          <div className="toolbar-left">
            <span className="results-count">
              <strong>{filteredConnections.length}</strong> of <strong>{mockConnections.length}</strong> Registered Devices
            </span>
            <div className="search-box-container">
              <svg className="search-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"></circle><line x1="21" y1="21" x2="16.65" y2="16.65"></line></svg>
              <input 
                type="text" 
                placeholder="Search Registered Devices" 
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          </div>
          <div className="toolbar-right">
            <button className="toolbar-btn">
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M3 6h18M3 12h18M3 18h18"></path></svg>
              Columns
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><polyline points="6 9 12 15 18 9"></polyline></svg>
            </button>
            <div className="csv-actions">
              <button className="toolbar-btn csv-btn">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="7 10 12 15 17 10"></polyline><line x1="12" y1="15" x2="12" y2="3"></line></svg>
                Import CSV
              </button>
              <button className="toolbar-btn csv-btn">
                <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><polyline points="17 8 12 3 7 8"></polyline><line x1="12" y1="3" x2="12" y2="15"></line></svg>
                Export CSV
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
                <th className="col-actions"></th>
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
                      <span className="hostname-text">{conn.hostname}</span>
                    </div>
                  </td>
                  <td className="col-ip">{conn.ip}</td>
                  <td className="col-type">
                    <div className="type-badge">
                      {conn.type.includes('SSH') ? 'SSH' : 'Telnet'}
                    </div>
                    <span className="type-detail">{conn.type.split(' ')[1] || ''}</span>
                  </td>
                  <td className="col-last">{conn.lastConnected}</td>
                  <td className="col-actions">
                    <button className="btn-action edit">
                      編集
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <footer className="connection-panel-footer">
          <button className="add-device-btn">ホスト追加</button>
          <button className="delete-selected-btn">削除</button>
        </footer>
      </div>
    </div>
  );
};
