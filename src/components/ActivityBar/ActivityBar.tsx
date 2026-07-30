import React from "react";
import { useTranslation } from "react-i18next";
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

export const ActivityBar: React.FC<ActivityBarProps> = React.memo(({
  setIsConnectionOpen,
  isConnectionOpen,
  setIsScheduledTasksOpen,
  isScheduledTasksOpen,
  setIsSettingsOpen,
  isSettingsOpen,
}) => {
  const { t } = useTranslation();

  return (
    <nav className="activity-bar">
      <div
        className={`activity-item ${!isSettingsOpen && !isConnectionOpen && !isScheduledTasksOpen ? "active" : ""}`}
        title={t("activity_bar.chat")}
        onClick={() => {
          setIsSettingsOpen(false);
          setIsConnectionOpen(false);
          setIsScheduledTasksOpen(false);
        }}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsSettingsOpen(false);
            setIsConnectionOpen(false);
            setIsScheduledTasksOpen(false);
          }
        }}
      >
        <MessageIcon size={24} />
      </div>
      <div
        className={`activity-item ${isConnectionOpen ? "active" : ""}`}
        title={t("activity_bar.connection_settings")}
        onClick={() => {
          setIsSettingsOpen(false);
          setIsScheduledTasksOpen(false);
          setIsConnectionOpen(true);
        }}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsSettingsOpen(false);
            setIsScheduledTasksOpen(false);
            setIsConnectionOpen(true);
          }
        }}
      >
        <NetworkTopologyIcon size={24} />
      </div>
      <div
        className={`activity-item ${isScheduledTasksOpen ? "active" : ""}`}
        title={t("activity_bar.scheduled_tasks")}
        onClick={() => {
          setIsSettingsOpen(false);
          setIsConnectionOpen(false);
          setIsScheduledTasksOpen(true);
        }}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsSettingsOpen(false);
            setIsConnectionOpen(false);
            setIsScheduledTasksOpen(true);
          }
        }}
      >
        <ClockIcon size={24} />
      </div>
      <div className="spacer"></div>
      <div
        className={`activity-item bottom ${isSettingsOpen ? "active" : ""}`}
        title={t("activity_bar.settings")}
        onClick={() => {
          setIsConnectionOpen(false);
          setIsScheduledTasksOpen(false);
          setIsSettingsOpen(true);
        }}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsConnectionOpen(false);
            setIsScheduledTasksOpen(false);
            setIsSettingsOpen(true);
          }
        }}
      >
        <GearIcon size={24} />
      </div>
    </nav>
  );
});

ActivityBar.displayName = "ActivityBar";

