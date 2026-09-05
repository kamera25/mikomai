import React from "react";
import { useTranslation } from "react-i18next";
import { MessageIcon, NetworkTopologyIcon, ClockIcon, GearIcon, BookIcon } from "../Icons";
import "./ActivityBar.css";

interface ActivityBarProps {
  setIsConnectionOpen: React.Dispatch<React.SetStateAction<boolean>>;
  isConnectionOpen: boolean;
  setIsScheduledTasksOpen: React.Dispatch<React.SetStateAction<boolean>>;
  isScheduledTasksOpen: boolean;
  setIsTaskAuditOpen: React.Dispatch<React.SetStateAction<boolean>>;
  isTaskAuditOpen: boolean;
  setIsSettingsOpen: React.Dispatch<React.SetStateAction<boolean>>;
  isSettingsOpen: boolean;
}

export const ActivityBar: React.FC<ActivityBarProps> = React.memo(({
  setIsConnectionOpen,
  isConnectionOpen,
  setIsScheduledTasksOpen,
  isScheduledTasksOpen,
  setIsTaskAuditOpen,
  isTaskAuditOpen,
  setIsSettingsOpen,
  isSettingsOpen,
}) => {
  const { t } = useTranslation();

  return (
    <nav className="activity-bar">
      <div
        className={`activity-item ${!isSettingsOpen && !isConnectionOpen && !isScheduledTasksOpen && !isTaskAuditOpen ? "active" : ""}`}
        title={t("activity_bar.chat")}
        onClick={() => {
          setIsSettingsOpen(false);
          setIsConnectionOpen(false);
          setIsScheduledTasksOpen(false);
          setIsTaskAuditOpen(false);
        }}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsSettingsOpen(false);
            setIsConnectionOpen(false);
            setIsScheduledTasksOpen(false);
            setIsTaskAuditOpen(false);
          }
        }}
      >
        <MessageIcon size={20} />
      </div>
      <div
        className={`activity-item ${isConnectionOpen ? "active" : ""}`}
        title={t("activity_bar.connection_settings")}
        onClick={() => {
          setIsSettingsOpen(false);
          setIsScheduledTasksOpen(false);
          setIsTaskAuditOpen(false);
          setIsConnectionOpen(true);
        }}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsSettingsOpen(false);
            setIsScheduledTasksOpen(false);
            setIsTaskAuditOpen(false);
            setIsConnectionOpen(true);
          }
        }}
      >
        <NetworkTopologyIcon size={20} />
      </div>
      <div
        className={`activity-item ${isScheduledTasksOpen ? "active" : ""}`}
        title={t("activity_bar.scheduled_tasks")}
        onClick={() => {
          setIsSettingsOpen(false);
          setIsConnectionOpen(false);
          setIsTaskAuditOpen(false);
          setIsScheduledTasksOpen(true);
        }}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsSettingsOpen(false);
            setIsConnectionOpen(false);
            setIsTaskAuditOpen(false);
            setIsScheduledTasksOpen(true);
          }
        }}
      >
        <ClockIcon size={20} />
      </div>
      <div className="spacer"></div>
      <div
        className={`activity-item ${isTaskAuditOpen ? "active" : ""}`}
        title={t("activity_bar.task_audit")}
        onClick={() => {
          setIsSettingsOpen(false); setIsConnectionOpen(false); setIsScheduledTasksOpen(false); setIsTaskAuditOpen(true);
        }}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault(); setIsSettingsOpen(false); setIsConnectionOpen(false); setIsScheduledTasksOpen(false); setIsTaskAuditOpen(true);
          }
        }}
      >
        <BookIcon size={20} />
      </div>
      <div
        className={`activity-item bottom ${isSettingsOpen ? "active" : ""}`}
        title={t("activity_bar.settings")}
        onClick={() => {
          setIsConnectionOpen(false);
          setIsScheduledTasksOpen(false);
          setIsTaskAuditOpen(false);
          setIsSettingsOpen(true);
        }}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            setIsConnectionOpen(false);
            setIsScheduledTasksOpen(false);
            setIsTaskAuditOpen(false);
            setIsSettingsOpen(true);
          }
        }}
      >
        <GearIcon size={20} />
      </div>
    </nav>
  );
});

ActivityBar.displayName = "ActivityBar";
