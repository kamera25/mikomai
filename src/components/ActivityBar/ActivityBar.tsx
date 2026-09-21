import React, { Fragment } from "react";
import { useTranslation } from "react-i18next";
import { MessageIcon, NetworkTopologyIcon, ClockIcon, GearIcon, BookIcon } from "../Icons";
import { useGuiEvent, type Panel } from "../../gui/events";
import "./ActivityBar.css";

interface ActivityBarProps {
  activePanel: Panel;
}

export const ActivityBar: React.FC<ActivityBarProps> = React.memo(({ activePanel }) => {
  const { t } = useTranslation();
  const emit = useGuiEvent();
  const items = [
    { panel: "chat", title: t("activity_bar.chat"), icon: <MessageIcon size={20} /> },
    {
      panel: "connections",
      title: t("activity_bar.connection_settings"),
      icon: <NetworkTopologyIcon size={20} />,
    },
    {
      panel: "scheduledTasks",
      title: t("activity_bar.scheduled_tasks"),
      icon: <ClockIcon size={20} />,
    },
    { panel: "taskAudit", title: t("activity_bar.task_audit"), icon: <BookIcon size={20} /> },
    { panel: "settings", title: t("activity_bar.settings"), icon: <GearIcon size={20} /> },
  ] as const;

  return (
    <nav className="activity-bar">
      {items.map((item) => (
        <Fragment key={item.panel}>
          {item.panel === "taskAudit" && <div className="spacer" />}
          <div
            className={`activity-item ${item.panel === activePanel ? "active" : ""} ${item.panel === "settings" ? "bottom" : ""}`}
            title={item.title}
            role="button"
            tabIndex={0}
            onClick={() => emit({ type: "navigate", panel: item.panel })}
            onKeyDown={(event) => {
              if (event.key === "Enter" || event.key === " ") {
                event.preventDefault();
                emit({ type: "navigate", panel: item.panel });
              }
            }}
          >
            {item.icon}
          </div>
        </Fragment>
      ))}
    </nav>
  );
});

ActivityBar.displayName = "ActivityBar";
