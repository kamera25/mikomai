import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useMemo, useState } from "react";
import { ClockIcon, SearchIcon, UpdateIcon } from "./Icons";
import "./ScheduledTasksPanel.css";

interface ScheduledTasksPanelProps {
  onClose: () => void;
}
type WatchStatus = "enabled" | "disabled";
type Step =
  | { id: string; call: "get_state"; args: { device: string; resource: "cpu" } }
  | { when: { left: { ref: string }; operator: "gt"; right: number }; then: unknown[] };
interface Watch {
  id: string;
  name: string;
  status: WatchStatus;
  ir: { schedule: { every: string }; steps: Step[] };
  lastRunAt?: string | null;
  lastError?: string | null;
}
interface WatchForm {
  id?: string;
  name: string;
  device: string;
  intervalSeconds: number;
  threshold: number;
}
interface RegisteredDevice {
  hostname: string;
  ip?: string;
}

const emptyForm = (): WatchForm => ({
  name: "CPU 使用率監視",
  device: "",
  intervalSeconds: 60,
  threshold: 80,
});
function toForm(watch: Watch): WatchForm {
  const call = watch.ir.steps.find(
    (step): step is Extract<Step, { call: "get_state" }> => "call" in step
  );
  const condition = watch.ir.steps.find(
    (step): step is Extract<Step, { when: unknown }> => "when" in step
  );
  return {
    id: watch.id,
    name: watch.name,
    device: call?.args.device ?? "",
    intervalSeconds: Number.parseInt(watch.ir.schedule.every, 10) || 60,
    threshold: condition?.when.right ?? 80,
  };
}
function toRequest(form: WatchForm) {
  const device = form.device.trim();
  return {
    name: form.name.trim(),
    ir: {
      version: 1,
      schedule: { every: `${form.intervalSeconds}s` },
      steps: [
        { id: "cpu", call: "get_state", args: { device, resource: "cpu" } },
        {
          when: { left: { ref: "cpu.usage" }, operator: "gt", right: form.threshold },
          then: [
            {
              call: "notify",
              args: { message: `${device} の CPU 使用率が ${form.threshold}% を超えました` },
            },
          ],
        },
      ],
    },
  };
}
const formatTime = (value?: string | null) => (value ? new Date(value).toLocaleString() : "未実行");

export const ScheduledTasksPanel: React.FC<ScheduledTasksPanelProps> = ({ onClose }) => {
  const [watches, setWatches] = useState<Watch[]>([]);
  const [devices, setDevices] = useState<RegisteredDevice[]>([]);
  const [query, setQuery] = useState("");
  const [form, setForm] = useState<WatchForm | null>(null);
  const [error, setError] = useState<string | null>(null);
  const loadWatches = async () => {
    try {
      const [loadedWatches, loadedDevices] = await Promise.all([
        invoke<Watch[]>("list_watches"),
        invoke<RegisteredDevice[]>("load_connections"),
      ]);
      setWatches(loadedWatches);
      setDevices(loadedDevices);
      setError(null);
    } catch (reason) {
      setError(`Watch の読み込みに失敗しました: ${String(reason)}`);
    }
  };
  useEffect(() => {
    void loadWatches();
    const unlisten = listen("watch-executed", () => void loadWatches());
    return () => void unlisten.then((dispose) => dispose());
  }, []);
  const filtered = useMemo(
    () =>
      watches.filter((watch) =>
        `${watch.name} ${toForm(watch).device}`.toLowerCase().includes(query.toLowerCase())
      ),
    [query, watches]
  );
  const save = async () => {
    if (!form) return;
    if (!devices.some((device) => device.hostname === form.device)) {
      setError("対象機器は登録済み機器から選択してください。");
      return;
    }
    try {
      const request = toRequest(form);
      if (form.id) await invoke("update_watch", { id: form.id, request });
      else await invoke("create_watch", { request });
      setForm(null);
      await loadWatches();
    } catch (reason) {
      setError(`Watch を保存できませんでした: ${String(reason)}`);
    }
  };
  const setEnabled = async (watch: Watch) => {
    try {
      await invoke(watch.status === "enabled" ? "disable_watch" : "enable_watch", { id: watch.id });
      await loadWatches();
    } catch (reason) {
      setError(`状態を変更できませんでした: ${String(reason)}`);
    }
  };
  const run = async (id: string) => {
    try {
      await invoke("execute_watch_now", { id });
      await loadWatches();
    } catch (reason) {
      setError(`Watch を実行できませんでした: ${String(reason)}`);
    }
  };
  const remove = async (id: string) => {
    if (!window.confirm("この Watch を削除しますか？")) return;
    try {
      await invoke("delete_watch", { id });
      await loadWatches();
    } catch (reason) {
      setError(`Watch を削除できませんでした: ${String(reason)}`);
    }
  };
  return (
    <div className="scheduled-tasks-overlay">
      <div className="scheduled-tasks-panel">
        <header className="scheduled-header">
          <div>
            <h2>定期実行</h2>
          </div>
          <button className="close-card-btn" aria-label="閉じる" onClick={onClose}>
            ×
          </button>
        </header>
        <div className="scheduled-toolbar">
          <div className="toolbar-left">
            <span className="results-count">
              <strong>{filtered.length}</strong> / <strong>{watches.length}</strong> 件の 定期実行
            </span>
            <div className="search-box-container">
              <SearchIcon className="search-icon" size={16} />
              <input
                placeholder="名前または対象機器を検索..."
                value={query}
                onChange={(event) => setQuery(event.target.value)}
              />
            </div>
          </div>
          <button className="toolbar-btn" onClick={() => void loadWatches()}>
            <UpdateIcon size={14} />
            更新
          </button>
        </div>
        {error && (
          <div className="watch-error" role="alert">
            {error}
          </div>
        )}
        <div className="scheduled-table-wrapper">
          <table className="scheduled-table">
            <thead>
              <tr>
                <th>名称</th>
                <th>対象機器</th>
                <th>間隔</th>
                <th>閾値</th>
                <th>状態</th>
                <th>最終実行</th>
                <th>操作</th>
              </tr>
            </thead>
            <tbody>
              {filtered.map((watch) => {
                const values = toForm(watch);
                return (
                  <tr key={watch.id}>
                    <td>
                      <div className="task-name-cell">
                        <div className="task-icon">
                          <ClockIcon size={14} />
                        </div>
                        <button className="watch-name-button" onClick={() => setForm(values)}>
                          {watch.name}
                        </button>
                      </div>
                    </td>
                    <td>{values.device}</td>
                    <td>{values.intervalSeconds} 秒</td>
                    <td>{values.threshold}% 超過</td>
                    <td>
                      <span className={`status-badge ${watch.status}`}>
                        {watch.status === "enabled" ? "有効" : "無効"}
                      </span>
                    </td>
                    <td title={watch.lastError ?? undefined}>
                      {watch.lastError ? `エラー: ${watch.lastError}` : formatTime(watch.lastRunAt)}
                    </td>
                    <td className="watch-actions">
                      <button onClick={() => void run(watch.id)}>今すぐ実行</button>
                      <button onClick={() => void setEnabled(watch)}>
                        {watch.status === "enabled" ? "無効化" : "有効化"}
                      </button>
                      <button className="watch-delete" onClick={() => void remove(watch.id)}>
                        削除
                      </button>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
        <footer className="scheduled-panel-footer">
          <button className="add-task-btn" onClick={() => setForm(emptyForm())}>
            CPU Watch を追加
          </button>
        </footer>
        {form && (
          <div className="task-settings-modal-overlay">
            <div className="task-settings-card">
              <header className="settings-card-header">
                <h3>{form.id ? "CPU Watch を編集" : "CPU Watch を追加"}</h3>
                <button className="close-card-btn" onClick={() => setForm(null)}>
                  ×
                </button>
              </header>
              <div className="settings-card-body">
                <p className="field-hint">
                  実行 IR は固定です: CPU を取得し、指定した閾値を超えたときだけ通知します。
                </p>
                <label className="settings-form-group">
                  名称
                  <input
                    value={form.name}
                    onChange={(event) => setForm({ ...form, name: event.target.value })}
                  />
                </label>
                <label className="settings-form-group">
                  対象機器
                  <select
                    value={form.device}
                    onChange={(event) => setForm({ ...form, device: event.target.value })}
                  >
                    <option value="">登録済み機器を選択...</option>
                    {devices.map((device) => (
                      <option key={device.hostname} value={device.hostname}>
                        {device.hostname}
                        {device.ip ? ` (${device.ip})` : ""}
                      </option>
                    ))}
                  </select>
                  {form.device && !devices.some((device) => device.hostname === form.device) && (
                    <p className="field-hint watch-error-hint">
                      現在の対象機器は登録されていません。登録済み機器を選び直してください。
                    </p>
                  )}
                </label>
                <label className="settings-form-group">
                  実行間隔（秒）
                  <input
                    type="number"
                    min="60"
                    step="60"
                    value={form.intervalSeconds}
                    onChange={(event) =>
                      setForm({ ...form, intervalSeconds: Number(event.target.value) })
                    }
                  />
                </label>
                <label className="settings-form-group">
                  CPU 閾値（%）
                  <input
                    type="number"
                    min="0"
                    max="100"
                    step="0.1"
                    value={form.threshold}
                    onChange={(event) =>
                      setForm({ ...form, threshold: Number(event.target.value) })
                    }
                  />
                </label>
              </div>
              <footer className="settings-card-footer">
                <button className="settings-cancel-btn" onClick={() => setForm(null)}>
                  キャンセル
                </button>
                <button className="settings-save-btn" onClick={() => void save()}>
                  保存
                </button>
              </footer>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
