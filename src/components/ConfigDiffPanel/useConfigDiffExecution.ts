import { useEffect, useRef, useState } from "react";
import { ipc } from "../../platform";
import { operationService } from "../../features/operations/operationService";
import { ConfigDiffData } from "../../contexts/UIContext";
import { isActiveCommitPhase, STEP_DEFINITIONS, type CommitPhase, type CommandResult, type LogStep, type OperationPlan, type StepPhaseKey } from "./configDiffModel";

export interface ConfigDiffExecutionOptions { id: string | null; isOpen: boolean; proposedDiffData: ConfigDiffData | null; }

export function useConfigDiffExecution({ id, isOpen, proposedDiffData }: ConfigDiffExecutionOptions) {
  const [phase, setPhase] = useState<CommitPhase>("idle");
  const [statusMessage, setStatusMessage] = useState<string>("");
  const [commitLogs, setCommitLogs] = useState<string[]>([]);
  const [verifiedDiffData, setVerifiedDiffData] = useState<ConfigDiffData | null>(null);
  const [activeTab, setActiveTab] = useState<"diff" | "logs">("diff");
  const [forceCommitReq, setForceCommitReq] = useState<{ forceId: string; errors: any[]; message: string } | null>(null);
  const [operationPlan, setOperationPlan] = useState<OperationPlan | null>(null);
  const logsEndRef = useRef<HTMLDivElement>(null);

  const [steps, setSteps] = useState<LogStep[]>([]);
  const [collapsedSteps, setCollapsedSteps] = useState<Record<string, boolean>>({});
  const [currentTime, setCurrentTime] = useState<number>(Date.now());

  // Timer tick for active step duration
  useEffect(() => {
    const timer = setInterval(() => setCurrentTime(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);

  // Toggle collapsing individual steps
  const toggleStepCollapse = (stepKey: string) => {
    setCollapsedSteps((prev) => ({
      ...prev,
      [stepKey]: !prev[stepKey],
    }));
  };

  // Reset state when id changes or panel opens
  useEffect(() => {
    setPhase("idle");
    setStatusMessage("");
    setCommitLogs([]);
    setVerifiedDiffData(null);
    setActiveTab("diff");
    setForceCommitReq(null);
    setOperationPlan(null);
    setSteps([]);
    setCollapsedSteps({});
  }, [id, proposedDiffData]);

  // Listen to Tauri events from Rust backend
  useEffect(() => {
    if (!isOpen) return;

    const unlistenStatus = ipc.subscribe<any>("commit-status", ({ id: eventId, phase: newPhase, message }) => {
      if (id && eventId && eventId !== id) return;

      if (newPhase) {
        setPhase(newPhase);

        if (isActiveCommitPhase(newPhase)) {
          setSteps((prev) => {
            const now = Date.now();
            const exists = prev.some((s) => s.key === newPhase);
            let updated = prev.map((s) => {
              if (s.status === "running") {
                return { ...s, status: "success" as const, endTime: now };
              }
              return s;
            });

            if (!exists) {
              const def = STEP_DEFINITIONS.find((d) => d.key === newPhase);
              updated.push({
                key: newPhase as StepPhaseKey,
                title: def ? def.title : newPhase,
                status: "running",
                startTime: now,
                logs: message ? [`[STATUS] ${message}`] : [],
              });
            } else {
              updated = updated.map((s) =>
                s.key === newPhase
                  ? { ...s, status: "running", startTime: s.startTime || now, endTime: undefined }
                  : s
              );
            }
            return updated;
          });
        } else if (newPhase === "success" || newPhase === "failed") {
          setSteps((prev) =>
            prev.map((s) =>
              s.status === "running"
                ? { ...s, status: newPhase === "success" ? "success" : "failed", endTime: Date.now() }
                : s
            )
          );
        }
      }

      if (message) {
        setStatusMessage(message);
        setCommitLogs((prev) => [...prev, `[STATUS] ${message}`]);
      }
      if (newPhase === "deploying" || newPhase === "fetching_before" || newPhase === "dry_running") {
        setActiveTab("logs");
      }
    });

    const unlistenLog = ipc.subscribe<any>("commit-log", ({ line }) => {
      if (line !== undefined && line !== null) {
        setCommitLogs((prev) => [...prev, line]);
        setSteps((prev) => {
          if (prev.length === 0) return prev;
          const lastIdx = prev.length - 1;
          const updated = [...prev];
          const currentStep = updated[lastIdx];
          const isErrorLine = line.includes("Error") || line.includes("FAILED") || line.startsWith("[DRY-RUN ERROR]");

          updated[lastIdx] = {
            ...currentStep,
            status: isErrorLine ? "failed" : currentStep.status,
            logs: [...currentStep.logs, line],
          };
          return updated;
        });
      }
    });

    const unlistenForceCommit = ipc.subscribe<any>("request-force-commit", ({ id: eventId, forceId, errors, message }) => {
      if (id && eventId && eventId !== id) return;
      setForceCommitReq({ forceId, errors: errors || [], message });
      setActiveTab("logs");
    });

    const unlistenDiffResult = ipc.subscribe<any>("commit-diff-result", ({ id: eventId, fileName, additions, deletions, diffLines, hostname, ip, status, message }) => {
      if (id && eventId && eventId !== id) return;

      const formattedLines = (diffLines || []).map((l: any) => ({
        type: l.type,
        oldLine: l.old_line !== undefined ? l.old_line : l.oldLine,
        newLine: l.new_line !== undefined ? l.new_line : l.newLine,
        content: l.content,
      }));

      setVerifiedDiffData({
        fileName: fileName || "running-config",
        additions: additions || 0,
        deletions: deletions || 0,
        diffLines: formattedLines,
        hostname,
        ip,
      });

      setPhase(status === "success" ? "success" : "failed");
      if (message) setStatusMessage(message);
      setActiveTab("diff");
    });

    return () => {
      unlistenStatus.then((fn) => fn());
      unlistenLog.then((fn) => fn());
      unlistenForceCommit.then((fn) => fn());
      unlistenDiffResult.then((fn) => fn());
    };
  }, [isOpen, id]);

  // Auto scroll logs
  useEffect(() => {
    if (activeTab === "logs" && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [commitLogs, activeTab]);

  const handleCommit = async () => {
    const diffData = verifiedDiffData || proposedDiffData;
    const deviceName = diffData?.hostname || diffData?.ip;
    const commands = (diffData?.diffLines || [])
      .filter((line) => line.type !== "normal")
      .map((line) => line.content.trim())
      .filter(Boolean);
    if (!deviceName) {
      setPhase("failed");
      setStatusMessage("対象機器が特定できません。登録済み機器を指定してから変更してください。");
      return;
    }
    try {
      setPhase("dry_running");
      setStatusMessage("変更計画を作成して承認中...");
      setCommitLogs(["[SYSTEM] 変更計画を作成しました。内容はこの差分と同一です。"]);
      setActiveTab("logs");
      const plan = await operationService.createPlan<OperationPlan>({
        deviceName,
        commands,
        rationale: `画面に表示した ${commands.length} 行の設定差分を ${deviceName} に適用する`,
      });
      setOperationPlan(plan);
      await operationService.approve<OperationPlan>(plan.id, plan.planHash);
      setOperationPlan((current) => current && { ...current, approvalStatus: "approved" });
      setCommitLogs((logs) => [...logs, `[SYSTEM] 変更計画 ${plan.id} を承認しました。`, "[SYSTEM] Dry-run を実行します。"]);
      const result = await operationService.execute<CommandResult>(plan.id, plan.planHash);
      setOperationPlan((current) =>
        current && { ...current, approvalStatus: result.success ? "executed" : "failed" }
      );
      setCommitLogs((logs) => [...logs, result.output]);
      setPhase(result.success ? "success" : "failed");
      setStatusMessage(result.success ? "承認済みの変更計画を適用しました。" : "変更計画の実行に失敗しました。");
      // Wake the conversion worker without granting it permission to perform
      // a second, legacy configuration write.
      await ipc.command("submit_user_choice", { id, choice: "operation_submitted" });
    } catch (e) {
      console.error("Failed to execute approved operation plan:", e);
      setPhase("failed");
      setStatusMessage(`変更計画の実行エラー: ${String(e)}`);
    }
  };

  const handleForceCommitChoice = async (choice: "commit_force" | "cancel") => {
    if (!forceCommitReq) return;
    const targetForceId = forceCommitReq.forceId;
    setForceCommitReq(null);
    try {
      await ipc.command("submit_user_choice", { id: targetForceId, choice });
    } catch (e) {
      console.error("Failed to submit force commit choice:", e);
    }
  };


  return { phase, statusMessage, commitLogs, verifiedDiffData, activeTab, setActiveTab, forceCommitReq, operationPlan, logsEndRef, steps, collapsedSteps, currentTime, toggleStepCollapse, handleCommit, handleForceCommitChoice };
}
