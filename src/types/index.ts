export interface BaseMessage {
  role: "user" | "ai";
  content: string;
  timestamp?: string; // ISO string
  isToolLoading?: boolean;
  isHidden?: boolean;
  task_id?: string;
}

export interface UserMessage extends BaseMessage {
  role: "user";
  event_type: "UserInput";
  status?: "Pending";
  action_name?: undefined;
  tool_id?: undefined;
  summary_text?: undefined;
  raw_data?: undefined;
  args?: undefined;
  saved_path?: undefined;
  is_cached?: undefined;
  cache_time?: undefined;
}

export interface ToolExecutionMessage extends BaseMessage {
  role: "ai";
  event_type: "ToolExecution";
  status: "Running" | "Success" | "Failed";
  action_name: string;
  tool_id: string;
  summary_text: string;
  raw_data: string | null;
  args?: Record<string, unknown> | null;
  saved_path?: string;
  is_cached?: boolean;
  cache_time?: string;
  waitingForApproval?: boolean;
}

export interface AgentResponseMessage extends BaseMessage {
  role: "ai";
  event_type: "AgentResponse";
  status?: undefined;
  action_name?: undefined;
  tool_id?: undefined;
  summary_text?: undefined;
  raw_data?: undefined;
  args?: undefined;
  saved_path?: undefined;
  is_cached?: undefined;
  cache_time?: undefined;
}

export interface SystemMessage extends BaseMessage {
  role: "ai";
  event_type: "SystemMessage";
  status?: undefined;
  action_name?: undefined;
  tool_id?: undefined;
  summary_text?: undefined;
  raw_data?: undefined;
  args?: undefined;
  saved_path?: undefined;
  is_cached?: undefined;
  cache_time?: undefined;
}

export type Message =
  | UserMessage
  | ToolExecutionMessage
  | AgentResponseMessage
  | SystemMessage;

export type ModelState = "NotLoaded" | "Loading" | "Loaded" | { Error: string };

export interface TauriCommandResult {
  success: boolean;
  output?: string;
  saved_path?: string;
  is_cached?: boolean;
  cache_time?: string;
  error?: string;
}

export interface SummaryItem {
  timestamp: string;
  content: string;
}

export interface Connection {
  id: string;
  status: "online" | "offline";
  hostname: string;
  ip: string;
  port?: number;
  type: string;
  lastConnected: string;
  username?: string;
  password?: string;
  enablePassword?: string;
  deviceType?: string;
  vendorType?: string;
  hasPassword?: boolean;
  hasEnablePassword?: boolean;
  passwordChanged?: boolean;
  enablePasswordChanged?: boolean;
}

export interface McpHost {
  hostname: string;
  ip: string;
  port?: number;
  deviceType: string;
  username: string;
}

export interface AskInitialPayload {
  prompt: string;
}

export interface AnalyzePayload {
  userMessage: string;
  toolLabel: string;
  output: string;
  isRag: boolean;
  historyBlock?: string | null;
}

export interface ToolStartedPayload {
  taskId: string;
  toolId: string;
  toolLabel: string;
  args: any;
  resolvedHost?: string;
}

export interface ToolFinishedPayload {
  taskId: string;
  success: boolean;
  output: string;
  savedPath?: string;
  isCached?: boolean;
  cacheTime?: string;
}

export interface AnalysisStartedPayload {
  taskId: string;
  analysisTaskId: string;
}

export interface InitialStartedPayload {
  taskId: string;
}

export interface InitialFinishedPayload {
  taskId: string;
  content: string;
}

export interface SummarySavedPayload {
  taskId: string;
  summaryText: string;
  summary: SummaryItem;
  content: string;
}

export type ChatEvent =
  | { type: "arpYamlSaved"; payload: { deviceName: string; savedPath: string } }
  | { type: "routeYamlSaved"; payload: { deviceName: string; savedPath: string } }
  | { type: "mcpToolStarted"; payload: ToolStartedPayload }
  | { type: "mcpToolFinished"; payload: ToolFinishedPayload }
  | { type: "mcpAnalysisStarted"; payload: AnalysisStartedPayload }
  | { type: "llmChunk"; payload: string }
  | { type: "agentSelected"; payload: string }
  | { type: "mcpInitialStarted"; payload: InitialStartedPayload }
  | { type: "mcpInitialFinished"; payload: InitialFinishedPayload }
  | { type: "mcpSummarySaved"; payload: SummarySavedPayload };

export interface ChatSession {
  id: string;
  type: "session";
  title: string;
  messages: Message[];
  recentIps?: string[];
}

export interface Folder {
  id: string;
  type: "folder";
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
  preloadPlotter?: boolean;
  preloadBuilder?: boolean;
  preloadSummarization?: boolean;
}
