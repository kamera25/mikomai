import { invoke } from "@tauri-apps/api/core";
import React, { useEffect, useState } from "react";
import { RefreshIcon } from "./Icons";
import "./TaskAuditPanel.css";

interface TaskSummary {
  taskId: string;
  startedAt: string;
  goal: string;
  lastEventAt: string;
  eventCount: number;
  status: "finished" | "stopped";
}
interface AuditEvent {
  event_type: string;
  timestamp?: string;
  goal?: string;
  reason?: string;
  action_type?: string;
  tool?: string;
  target?: string;
  success?: boolean;
  observation?: { raw?: string; source?: { tool_name?: string; device?: string } };
}
interface TaskAudit {
  summary: TaskSummary;
  events: AuditEvent[];
}

const formatTime = (value: string) => new Date(value).toLocaleString();
const eventLabel = (event: AuditEvent) => {
  switch (event.event_type) {
    case "task_started": return "タスク開始";
    case "goal_set": return "目標設定";
    case "decision": return `判断: ${event.action_type ?? ""}`;
    case "action": return `実行: ${event.tool ?? ""}`;
    case "result": return `${event.success ? "成功" : "失敗"}: ${event.observation?.source?.tool_name ?? "ツール"}`;
    case "finished": return "完了";
    default: return event.event_type;
  }
};
const eventDetail = (event: AuditEvent) =>
  event.goal ?? event.reason ?? event.observation?.raw ?? [event.target, event.tool].filter(Boolean).join(" / ");

export const TaskAuditPanel: React.FC<{ onClose: () => void; onResume: (task: TaskSummary) => Promise<void> }> = ({ onClose, onResume }) => {
  const [tasks, setTasks] = useState<TaskSummary[]>([]);
  const [selected, setSelected] = useState<TaskAudit | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadTasks = async () => {
    setLoading(true);
    try {
      setTasks(await invoke<TaskSummary[]>("list_agent_tasks"));
      setError(null);
    } catch (reason) {
      setError(`監査履歴を読み込めませんでした: ${String(reason)}`);
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => { void loadTasks(); }, []);
  const selectTask = async (task: TaskSummary) => {
    try {
      setSelected(await invoke<TaskAudit>("get_agent_task_audit", { taskId: task.taskId }));
      setError(null);
    } catch (reason) {
      setError(`実行記録を読み込めませんでした: ${String(reason)}`);
    }
  };

  return <div className="task-audit-overlay">
    <section className="task-audit-panel" aria-label="エージェント実行履歴">
      <header className="task-audit-header">
        <div><h2>エージェント実行履歴</h2><p>保存済みの調査内容と実行結果を確認できます。</p></div>
        <div className="task-audit-header-actions"><button className="toolbar-btn" onClick={() => void loadTasks()}><RefreshIcon size={14} />更新</button><button className="close-card-btn" aria-label="閉じる" onClick={onClose}>×</button></div>
      </header>
      {error && <p className="task-audit-error">{error}</p>}
      <div className="task-audit-content">
        <aside className="task-audit-list">
          {loading ? <p>読み込み中...</p> : tasks.length === 0 ? <p>保存済みの実行履歴はありません。</p> : tasks.map((task) => <button key={task.taskId} className={`task-audit-item ${selected?.summary.taskId === task.taskId ? "selected" : ""}`} onClick={() => void selectTask(task)}>
            <span className={`task-audit-status ${task.status}`}>{task.status === "finished" ? "完了" : "中断"}</span>
            <strong>{task.goal}</strong><small>{formatTime(task.lastEventAt)} · {task.eventCount} 件</small>
          </button>)}
        </aside>
        <main className="task-audit-detail">
          {!selected ? <p className="task-audit-empty">左側から実行履歴を選択してください。</p> : <>
            <div className="task-audit-summary"><div><h3>{selected.summary.goal}</h3><p>開始: {formatTime(selected.summary.startedAt)}</p></div><button className="task-resume-button" onClick={() => void onResume(selected.summary)}>この調査を再開</button></div>
            <p className="task-audit-note">再開すると、過去の観測結果を引き継いだ新しい調査として実行します。元の記録は変更しません。</p>
            <ol className="task-audit-events">{selected.events.map((event, index) => <li key={`${event.event_type}-${index}`}><strong>{eventLabel(event)}</strong>{eventDetail(event) && <pre>{eventDetail(event)}</pre>}</li>)}</ol>
          </>}
        </main>
      </div>
    </section>
  </div>;
};
