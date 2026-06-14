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
  tool_id?: string;
  summary_text?: string;
  raw_data?: string | null;
  args?: any;
  saved_path?: string;
  is_cached?: boolean;
  cache_time?: string;
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
  port?: number;
  type: string;
  lastConnected: string;
  username?: string;
  password?: string;
  deviceType?: string;
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
  recentIps?: string[];
}

export interface Folder {
  id: string;
  type: 'folder';
  name: string;
  items: HistoryItem[];
  isOpen: boolean;
}

export type HistoryItem = Folder | ChatSession;

export interface SystemSettings {
  historyLimit?: number;
  temperature?: number;
  repetitionPenalty?: number;
  modelPath?: string | null;
  recentIps?: string[];
  mcpTimeout?: number;
  cacheExpiryMinutes?: number;
  dbPath?: string;
  ipVersion?: string;
  consolePort?: string | null;
  consoleBaudRate?: number;
  preloadInvestigate?: boolean;
  preloadKnowledge?: boolean;
  preloadAnalysis?: boolean;
  preloadRag?: boolean;
}

