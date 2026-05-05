export interface Message {
  role: "user" | "ai";
  content: string;
  timestamp?: string; // ISO string
  isToolLoading?: boolean;
  isHidden?: boolean;
  task_id?: string;
  event_type?: "UserInput" | "ToolExecution" | "AgentResponse" | "SystemMessage";
  status?: "Running" | "Success" | "Failed";
  action_name?: string;
  summary_text?: string;
  raw_data?: string | null;
}

export interface SummaryItem {
  timestamp: string;
  content: string;
}

export interface Connection {
  id: string;
  status: 'online' | 'offline';
  hostname: string;
  ip: string;
  type: string;
  lastConnected: string;
}

export interface McpHost {
  hostname: string;
  ip: string;
  deviceType: string;
  username: string;
}

export interface ChatSession {
  id: string;
  type: 'session';
  title: string;
  messages: Message[];
}

export interface Folder {
  id: string;
  type: 'folder';
  name: string;
  items: HistoryItem[];
  isOpen: boolean;
}

export type HistoryItem = Folder | ChatSession;
