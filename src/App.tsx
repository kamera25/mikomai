import { useState, useRef, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SettingsPanel } from "./components/SettingsPanel";
import { ApprovalModal } from "./components/ApprovalModal";
import "./App.css";

interface Message {
  role: "user" | "ai";
  content: string;
}

function App() {
  const [input, setInput] = useState("");
  const [messages, setMessages] = useState<Message[]>([]);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);
  const [isApprovalOpen, setIsApprovalOpen] = useState(false);
  
  // Pending Tool Call State
  const [pendingCommands, setPendingCommands] = useState<string[]>([]);
  const [pendingRationale, setPendingRationale] = useState<string>("");
  const [pendingDiff, setPendingDiff] = useState<string>("");
  
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Auto-resize textarea
  useEffect(() => {
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.height = `${Math.min(textareaRef.current.scrollHeight, 150)}px`;
    }
  }, [input]);

  const handleSend = async () => {
    if (!input.trim()) return;
    
    const userMessage = input.trim();
    setInput("");
    setMessages(prev => [...prev, { role: "user", content: userMessage }]);

    // Simulated LLM Tool Calling Logic
    setTimeout(async () => {
      const lowerInput = userMessage.toLowerCase();
      
      if (lowerInput.includes("configure") || lowerInput.includes("change") || lowerInput.includes("set")) {
        // AI decides to call `network_config`
        setPendingCommands([
          "conf t",
          "interface GigabitEthernet0/1",
          "description Connected via AI Agent",
          "end",
          "write memory"
        ]);
        setPendingRationale("Based on your request, I will configure the interface description. This requires writing to the device configuration.");
        setPendingDiff(" interface GigabitEthernet0/1\n- description Old\n+ description Connected via AI Agent");
        setIsApprovalOpen(true);
        
      } else if (lowerInput.includes("show") || lowerInput.includes("status") || lowerInput.includes("check")) {
        // AI decides to call `network_show`
        setMessages(prev => [...prev, { role: "ai", content: "Retrieving device status..." }]);
        
        try {
          const result: any = await invoke("network_show", {
            device: { host: "192.168.1.1", username: "admin", device_type: "cisco_ios" },
            command: "show ip int brief"
          });
          setMessages(prev => [...prev, { role: "ai", content: result.success ? `\`\`\`\n${result.output}\n\`\`\`` : `Error: ${result.output}` }]);
        } catch (e: any) {
          setMessages(prev => [...prev, { role: "ai", content: `Failed to execute: ${e.toString()}` }]);
        }
      } else {
        // General Chat
        setMessages(prev => [...prev, { role: "ai", content: "I understand. I can help you check network status or configure devices. Try asking me to 'show interfaces' or 'configure port 1'." }]);
      }
    }, 500);
  };

  const handleApproveWrite = async () => {
    setIsApprovalOpen(false);
    setMessages(prev => [...prev, { role: "ai", content: "Executing configuration changes..." }]);
    
    try {
      const result: any = await invoke("network_config", {
        device: { host: "192.168.1.1", username: "admin", device_type: "cisco_ios" },
        commands: pendingCommands
      });
      setMessages(prev => [...prev, { role: "ai", content: result.success ? `Changes applied successfully:\n\`\`\`\n${result.output}\n\`\`\`` : `Configuration failed: ${result.output}` }]);
    } catch (e: any) {
      setMessages(prev => [...prev, { role: "ai", content: `Failed to execute: ${e.toString()}` }]);
    }
  };

  return (
    <div className="app-container">
      {/* Sidebar */}
      <aside className="sidebar">
        <div className="sidebar-header">
          <div className="agent-icon">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect>
              <rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect>
              <line x1="6" y1="6" x2="6.01" y2="6"></line>
              <line x1="6" y1="18" x2="6.01" y2="18"></line>
            </svg>
          </div>
          <h2>mikomai</h2>
        </div>
        
        <div className="session-list">
          <div className="session-item active">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path></svg>
            New Session
          </div>
          <div className="session-item">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path></svg>
            Configure Router 1
          </div>
          <div className="session-item">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"></path></svg>
            Troubleshoot VLAN 20
          </div>
        </div>
      </aside>

      {/* Main Chat Area */}
      <main className="main-chat">
        {/* Top Header */}
        <header className="chat-header">
          <div className="model-selector" onClick={() => setIsSettingsOpen(true)}>
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><path d="M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z"></path><polyline points="3.27 6.96 12 12.01 20.73 6.96"></polyline><line x1="12" y1="22.08" x2="12" y2="12"></line></svg>
            <span>Qwen 2.5 (Local)</span>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"></path></svg>
          </div>
          
          <div className="status-badge" style={{ cursor: 'pointer' }} onClick={() => setIsApprovalOpen(true)}>
            <div className="status-dot"></div>
            <span>Ready (Test Approval)</span>
          </div>
        </header>

        {/* Chat History */}
        <div className="chat-history">
          {messages.length === 0 ? (
            <div className="empty-state">
              <div className="agent-icon" style={{ width: 64, height: 64, marginBottom: 24, borderRadius: 16 }}>
                 <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                  <rect x="2" y="2" width="20" height="8" rx="2" ry="2"></rect>
                  <rect x="2" y="14" width="20" height="8" rx="2" ry="2"></rect>
                  <line x1="6" y1="6" x2="6.01" y2="6"></line>
                  <line x1="6" y1="18" x2="6.01" y2="18"></line>
                </svg>
              </div>
              <h3>mikomai</h3>
              <p>I am connected to your local vector database and MCP servers. Ask me to retrieve manuals, check switch statuses, or propose configuration changes.</p>
            </div>
          ) : (
            messages.map((msg, idx) => (
              <div key={idx} className={`message ${msg.role}`}>
                <div className={`avatar ${msg.role}`}>
                  {msg.role === 'ai' ? '🤖' : '👤'}
                </div>
                <div className="message-bubble">
                  {msg.content.includes("```") ? (
                    <pre style={{ whiteSpace: 'pre-wrap' }}>{msg.content.replace(/```/g, '')}</pre>
                  ) : (
                    msg.content
                  )}
                </div>
              </div>
            ))
          )}
        </div>

        {/* Input Area */}
        <div className="input-area">
          <div className="input-container">
            <textarea
              ref={textareaRef}
              className="chat-input"
              placeholder="Ask mikomai..."
              value={input}
              onChange={(e) => setInput(e.target.value)}
              rows={1}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault();
                  handleSend();
                }
              }}
            />
            <button className="send-button" onClick={handleSend}>
              <svg viewBox="0 0 24 24">
                <path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z"></path>
              </svg>
            </button>
          </div>
        </div>
      </main>

      <SettingsPanel 
        isOpen={isSettingsOpen} 
        onClose={() => setIsSettingsOpen(false)} 
      />

      <ApprovalModal 
        isOpen={isApprovalOpen}
        onClose={() => setIsApprovalOpen(false)}
        onApprove={handleApproveWrite}
        commands={pendingCommands}
        rationale={pendingRationale}
        diffText={pendingDiff}
      />
    </div>
  );
}

export default App;
