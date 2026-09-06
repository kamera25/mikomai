export interface OperationPlan {
  id: string;
  planHash: string;
  target?: string;
  approvalStatus: "pending" | "approved" | "executing" | "executed" | "failed";
}

export interface CommandResult {
  success: boolean;
  output: string;
}

export type StepPhaseKey = "fetching_before" | "dry_running" | "deploying" | "verifying";
export type CommitPhase = "idle" | StepPhaseKey | "success" | "failed";

export interface LogStep {
  key: StepPhaseKey;
  title: string;
  status: "pending" | "running" | "success" | "failed";
  startTime?: number;
  endTime?: number;
  logs: string[];
}

export const STEP_DEFINITIONS: { key: StepPhaseKey; title: string }[] = [
  { key: "fetching_before", title: "現状のConfig取得" },
  { key: "dry_running", title: "自動Dry-run (Tab補完検証)" },
  { key: "deploying", title: "Config投入 & 適用" },
  { key: "verifying", title: "投入後Config取得 & Diff検証" },
];

export function isActiveCommitPhase(phase: string): phase is StepPhaseKey {
  return STEP_DEFINITIONS.some((step) => step.key === phase);
}
