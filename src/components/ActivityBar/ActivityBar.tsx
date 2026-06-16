import React from "react";
import "./ActivityBar.css";

interface ActivityBarProps {
  setIsConnectionOpen: React.Dispatch<React.SetStateAction<boolean>>;
  isConnectionOpen: boolean;
  setIsScheduledTasksOpen: React.Dispatch<React.SetStateAction<boolean>>;
  isScheduledTasksOpen: boolean;
  setIsSettingsOpen: React.Dispatch<React.SetStateAction<boolean>>;
  isSettingsOpen: boolean;
}

export const ActivityBar: React.FC<ActivityBarProps> = ({
  setIsConnectionOpen,
  isConnectionOpen,
  setIsScheduledTasksOpen,
  isScheduledTasksOpen,
  setIsSettingsOpen,
  isSettingsOpen,
}) => {
  return (
    <nav className="activity-bar">
      <div
        className={`activity-item ${!isSettingsOpen && !isConnectionOpen && !isScheduledTasksOpen ? "active" : ""}`}
        title="チャット"
        onClick={() => {
          setIsSettingsOpen(false);
          setIsConnectionOpen(false);
          setIsScheduledTasksOpen(false);
        }}
      >
        <svg
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path>
        </svg>
      </div>
      <div
        className={`activity-item ${isConnectionOpen ? "active" : ""}`}
        title="接続設定"
        onClick={() => {
          setIsSettingsOpen(false);
          setIsScheduledTasksOpen(false);
          setIsConnectionOpen(true);
        }}
      >
        <svg
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <rect x="16" y="16" width="6" height="6" rx="1"></rect>
          <rect x="2" y="16" width="6" height="6" rx="1"></rect>
          <rect x="9" y="2" width="6" height="6" rx="1"></rect>
          <path d="M5 16v-3a1 1 0 0 1 1-1h12a1 1 0 0 1 1 1v3"></path>
          <line x1="12" y1="12" x2="12" y2="8"></line>
        </svg>
      </div>
      <div
        className={`activity-item ${isScheduledTasksOpen ? "active" : ""}`}
        title="スケジュールタスク"
        onClick={() => {
          setIsSettingsOpen(false);
          setIsConnectionOpen(false);
          setIsScheduledTasksOpen(true);
        }}
      >
        <svg
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <circle cx="12" cy="12" r="10"></circle>
          <polyline points="12 6 12 12 16 14"></polyline>
        </svg>
      </div>
      <div className="spacer"></div>
      <div
        className={`activity-item bottom ${isSettingsOpen ? "active" : ""}`}
        title="設定"
        onClick={() => {
          setIsConnectionOpen(false);
          setIsScheduledTasksOpen(false);
          setIsSettingsOpen(true);
        }}
      >
        <svg
          width="24"
          height="24"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <circle cx="12" cy="12" r="3"></circle>
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path>
        </svg>
      </div>
    </nav>
  );
};
