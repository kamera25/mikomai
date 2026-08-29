import React, { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useTranslation } from "react-i18next";
import "./ScheduledTasksPanel.css";
import { ClockIcon, SearchIcon, UpdateIcon } from "./Icons";

interface ScheduledTasksPanelProps {
  onClose: () => void;
}

type ScheduleType = "weekly" | "daily" | "hourly" | "minutely" | "secondly" | "custom";

const ScheduleInput: React.FC<{ value: string; onChange: (val: string) => void }> = ({
  value,
  onChange,
}) => {
  const { t } = useTranslation();
  const [type, setType] = useState<ScheduleType>("custom");
  const [dayOfWeek, setDayOfWeek] = useState("月曜");
  const [time, setTime] = useState("00:00");
  const [minute, setMinute] = useState("0");
  const [second, setSecond] = useState("0");
  const [customText, setCustomText] = useState("");

  useEffect(() => {
    if (value.startsWith("毎週") && value.includes(" ")) {
      setType("weekly");
      const parts = value.split(" ");
      setDayOfWeek(parts[0].replace("毎週", ""));
      setTime(parts[1]);
    } else if (value.startsWith("毎日 ")) {
      setType("daily");
      setTime(value.replace("毎日 ", ""));
    } else if (value.startsWith("毎時 ") && value.endsWith("分")) {
      setType("hourly");
      setMinute(value.replace("毎時 ", "").replace("分", ""));
    } else if (value.startsWith("毎分 ") && value.endsWith("秒")) {
      setType("minutely");
      setSecond(value.replace("毎分 ", "").replace("秒", ""));
    } else if (value === "毎秒") {
      setType("secondly");
    } else {
      setType("custom");
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

    let newValue = "";
    if (newType === "weekly") {
      newValue = `毎週${newDay} ${newTime}`;
    } else if (newType === "daily") {
      newValue = `毎日 ${newTime}`;
    } else if (newType === "hourly") {
      newValue = `毎時 ${newMin}分`;
    } else if (newType === "minutely") {
      newValue = `毎分 ${newSec}秒`;
    } else if (newType === "secondly") {
      newValue = `毎秒`;
    } else {
      newValue = newCustom;
    }
    onChange(newValue);
  };

  return (
    <div className="schedule-input-container">
      <select
        value={type}
        onChange={(e) =>
          handleChange(e.target.value as ScheduleType, dayOfWeek, time, minute, second, customText)
        }
        className="schedule-type-select"
      >
        <option value="weekly">{t("scheduled_tasks.weekly")}</option>
        <option value="daily">{t("scheduled_tasks.daily")}</option>
        <option value="hourly">{t("scheduled_tasks.hourly")}</option>
        <option value="minutely">{t("scheduled_tasks.minutely")}</option>
        <option value="secondly">{t("scheduled_tasks.secondly")}</option>
        <option value="custom">{t("scheduled_tasks.custom")}</option>
      </select>

      {type === "weekly" && (
        <>
          <select
            value={dayOfWeek}
            onChange={(e) => handleChange(type, e.target.value, time, minute, second, customText)}
            className="schedule-day-select"
          >
            <option value="月曜">{t("scheduled_tasks.monday")}</option>
            <option value="火曜">{t("scheduled_tasks.tuesday")}</option>
            <option value="水曜">{t("scheduled_tasks.wednesday")}</option>
            <option value="木曜">{t("scheduled_tasks.thursday")}</option>
            <option value="金曜">{t("scheduled_tasks.friday")}</option>
            <option value="土曜">{t("scheduled_tasks.saturday")}</option>
            <option value="日曜">{t("scheduled_tasks.sunday")}</option>
          </select>
          <input
            type="time"
            value={time}
            onChange={(e) =>
              handleChange(type, dayOfWeek, e.target.value, minute, second, customText)
            }
          />
        </>
      )}

      {type === "daily" && (
        <input
          type="time"
          value={time}
          onChange={(e) =>
            handleChange(type, dayOfWeek, e.target.value, minute, second, customText)
          }
        />
      )}

      {type === "hourly" && (
        <div className="schedule-flex-input">
          <input
            type="number"
            min="0"
            max="59"
            value={minute}
            onChange={(e) =>
              handleChange(type, dayOfWeek, time, e.target.value, second, customText)
            }
          />
          <span>{t("scheduled_tasks.minute")}</span>
        </div>
      )}

      {type === "minutely" && (
        <div className="schedule-flex-input">
          <input
            type="number"
            min="0"
            max="59"
            value={second}
            onChange={(e) =>
              handleChange(type, dayOfWeek, time, minute, e.target.value, customText)
            }
          />
          <span>{t("scheduled_tasks.second")}</span>
        </div>
      )}

      {type === "custom" && (
        <input
          type="text"
          value={customText}
          onChange={(e) => handleChange(type, dayOfWeek, time, minute, second, e.target.value)}
          placeholder={t("scheduled_tasks.cron_placeholder")}
        />
      )}
    </div>
  );
};

interface ScheduledTask {
  id: string;
  name: string;
  status: "running" | "stopped" | "disabled";
  schedule: string;
  lastRun: string;
  prompt: string;
}

export const ScheduledTasksPanel: React.FC<ScheduledTasksPanelProps> = ({ onClose: _onClose }) => {
  const { t } = useTranslation();
  const [tasks, setTasks] = useState<ScheduledTask[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [editingTask, setEditingTask] = useState<ScheduledTask | null>(null);
  const [isCreating, setIsCreating] = useState(false);
  const [selectedTasks, setSelectedTasks] = useState<Set<string>>(new Set());

  useEffect(() => {
    loadTasks();

    const unlisten = listen("task-executed", (event) => {
      console.log("Task executed:", event.payload);
      loadTasks(); // reload to get updated lastRun
    });

    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const loadTasks = async () => {
    try {
      const loadedTasks = await invoke<ScheduledTask[]>("load_scheduled_tasks");
      setTasks(loadedTasks);
    } catch (error) {
      console.error("Failed to load tasks:", error);
    }
  };

  const filteredTasks = tasks.filter(
    (task) =>
      task.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      task.schedule.toLowerCase().includes(searchQuery.toLowerCase())
  );

  const handleSaveTask = async () => {
    if (editingTask) {
      try {
        if (isCreating) {
          await invoke("add_scheduled_task", {
            name: editingTask.name,
            schedule: editingTask.schedule,
            prompt: editingTask.prompt,
          });
        } else {
          await invoke("update_scheduled_task", { task: editingTask });
        }
        setEditingTask(null);
        setIsCreating(false);
        loadTasks();
      } catch (error) {
        console.error("Failed to save task:", error);
      }
    }
  };

  const handleDeleteSelected = async () => {
    try {
      for (const id of selectedTasks) {
        await invoke("delete_scheduled_task", { id });
      }
      setSelectedTasks(new Set());
      loadTasks();
    } catch (error) {
      console.error("Failed to delete tasks:", error);
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
      id: "",
      name: t("scheduled_tasks.default_new_task_name"),
      status: "running",
      schedule: "* * * * * *",
      lastRun: "-",
      prompt: t("scheduled_tasks.default_prompt"),
    });
    setIsCreating(true);
  };

  const handleExecuteNow = async (id: string) => {
    try {
      await invoke("execute_task", { id });
      loadTasks();
    } catch (error) {
      console.error("Failed to execute task:", error);
    }
  };

  const getStatusLabel = (status: string) => {
    switch (status) {
      case "running":
        return t("scheduled_tasks.state_running");
      case "stopped":
        return t("scheduled_tasks.state_stopped");
      case "disabled":
        return t("scheduled_tasks.state_disabled");
      default:
        return status;
    }
  };

  return (
    <div className="scheduled-tasks-overlay">
      <div className="scheduled-tasks-panel">
        <header className="scheduled-header">
          <div className="header-title-container">
            <h2>{t("scheduled_tasks.header")}</h2>
          </div>
        </header>

        <div className="scheduled-toolbar">
          <div className="toolbar-left">
            <span className="results-count">
              <strong>{filteredTasks.length}</strong> / <strong>{tasks.length}</strong>{" "}
              {t("scheduled_tasks.show_tasks")}
            </span>
            <div className="search-box-container">
              <SearchIcon className="search-icon" size={16} />
              <input
                type="text"
                placeholder={t("scheduled_tasks.search_placeholder")}
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
              />
            </div>
          </div>
          <div className="toolbar-right">
            <button className="toolbar-btn" onClick={loadTasks}>
              <UpdateIcon size={14} />
              {t("scheduled_tasks.btn_update")}
            </button>
          </div>
        </div>

        <div className="scheduled-table-wrapper">
          <table className="scheduled-table">
            <thead>
              <tr>
                <th className="col-checkbox">-</th>
                <th>{t("scheduled_tasks.th_name")}</th>
                <th>{t("scheduled_tasks.th_state")}</th>
                <th>{t("scheduled_tasks.th_cron")}</th>
                <th>{t("scheduled_tasks.th_last_run")}</th>
                <th>{t("scheduled_tasks.th_actions")}</th>
              </tr>
            </thead>
            <tbody>
              {filteredTasks.map((task) => (
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
                        <ClockIcon size={14} />
                      </div>
                      <span
                        className="task-name-text"
                        onClick={() => {
                          setEditingTask({ ...task });
                          setIsCreating(false);
                        }}
                        role="button"
                        tabIndex={0}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            setEditingTask({ ...task });
                            setIsCreating(false);
                          }
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
                      style={{ padding: "4px 8px", fontSize: "12px", cursor: "pointer" }}
                    >
                      {t("scheduled_tasks.btn_manual_run")}
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
                <h3>{isCreating ? t("scheduled_tasks.dialog_create_title") : t("scheduled_tasks.dialog_edit_title")}</h3>
                <button className="close-card-btn" onClick={() => setEditingTask(null)}>
                  &times;
                </button>
              </header>
              <div className="settings-card-body">
                <div className="settings-form-group">
                  <label>{t("scheduled_tasks.label_name")}</label>
                  <input
                    type="text"
                    value={editingTask.name}
                    onChange={(e) => setEditingTask({ ...editingTask, name: e.target.value })}
                  />
                </div>
                <div className="settings-form-group">
                  <label>{t("scheduled_tasks.label_cron")}</label>
                  <ScheduleInput
                    value={editingTask.schedule}
                    onChange={(val) => setEditingTask({ ...editingTask, schedule: val })}
                  />
                </div>
                <div className="settings-form-group">
                  <label>{t("scheduled_tasks.label_state")}</label>
                  <select
                    value={editingTask.status}
                    onChange={(e) =>
                      setEditingTask({ ...editingTask, status: e.target.value as any })
                    }
                    className="task-status-select"
                  >
                    <option value="running">{t("scheduled_tasks.state_running")}</option>
                    <option value="stopped">{t("scheduled_tasks.state_stopped")}</option>
                    <option value="disabled">{t("scheduled_tasks.state_disabled")}</option>
                  </select>
                </div>
                <div className="settings-form-group">
                  <label>{t("scheduled_tasks.label_prompt")} {isCreating ? "" : "(表示専用)"}</label>
                  <textarea
                    value={editingTask.prompt}
                    readOnly={!isCreating}
                    onChange={(e) => setEditingTask({ ...editingTask, prompt: e.target.value })}
                    className={!isCreating ? "readonly-prompt-area" : "task-prompt-textarea"}
                  />
                  {!isCreating && (
                    <p className="field-hint">
                      {t("scheduled_tasks.prompt_readonly_notice")}
                    </p>
                  )}
                </div>
              </div>
              <footer className="settings-card-footer">
                <button className="settings-cancel-btn" onClick={() => setEditingTask(null)}>
                  {t("scheduled_tasks.btn_cancel")}
                </button>
                <button className="settings-save-btn" onClick={handleSaveTask}>
                  {t("scheduled_tasks.btn_save")}
                </button>
              </footer>
            </div>
          </div>
        )}

        <footer className="scheduled-panel-footer">
          <button className="add-task-btn" onClick={handleCreateNew}>
            {t("scheduled_tasks.btn_add")}
          </button>
          <button
            className={`delete-selected-btn ${selectedTasks.size > 0 ? "active" : "disabled"}`}
            onClick={handleDeleteSelected}
            disabled={selectedTasks.size === 0}
          >
            {t("scheduled_tasks.btn_delete")}
          </button>
        </footer>
      </div>
    </div>
  );
};
