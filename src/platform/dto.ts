/**
 * IPC DTOs mirrored from `src-tauri/src/commands`. Keep transport shapes here
 * so feature code does not duplicate Rust-facing object contracts.
 */
export type TaskStatus =
  | "pending"
  | "running"
  | "awaiting_approval"
  | "awaiting_input"
  | "completed"
  | "failed"
  | "unknown";

export interface TaskSnapshot {
  taskId: string;
  goal: string;
  status: TaskStatus;
}

export interface OperationDto {
  id: string;
  toolId: string;
  target: string | null;
  planHash: string;
}

export interface EvidenceDto {
  id: string;
  content: string;
  source: { target: string | null; tool: string | null; request: string | null };
  provenance: { origin: "Tool" | "Parser" | "Inference" | "Human"; confidence: number | null };
}
