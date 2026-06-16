import { Message, SummaryItem } from "../../types";

export interface UseMcpProps {
  messages: Message[];
  setMessages: React.Dispatch<React.SetStateAction<Message[]>>;
  summaries: SummaryItem[];
  setSummaries: React.Dispatch<React.SetStateAction<SummaryItem[]>>;
  historyLimit: number;
  mcpTimeout?: number;
  updateRecentHosts?: (hosts: string[]) => void;
  recentIPs?: string[];
}
