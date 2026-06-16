import React from "react";
import { MessageIcon, NetworkTopologyIcon, ClockIcon, GearIcon } from "../Icons";
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
        <MessageIcon size={24} />
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
        <NetworkTopologyIcon size={24} />
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
        <ClockIcon size={24} />
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
        <GearIcon size={24} />
      </div>
    </nav>
  );
};
