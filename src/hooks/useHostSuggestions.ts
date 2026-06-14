import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Connection, McpHost } from "../types";

interface UseHostSuggestionsProps {
  recentIPs: string[];
  setRecentIPs: (ips: string[]) => void;
  activeSessionId: string;
  updateSessionRecentIps: (sessionId: string, ips: string[]) => void;
  saveAllSettings: (overrides: Partial<any>) => Promise<void>;
  input: string;
  setInput: (value: string) => void;
  textareaRef: React.RefObject<HTMLTextAreaElement | null>;
}

export function useHostSuggestions({
  recentIPs,
  setRecentIPs,
  activeSessionId,
  updateSessionRecentIps,
  saveAllSettings,
  input,
  setInput,
  textareaRef,
}: UseHostSuggestionsProps) {
  const [availableHosts, setAvailableHosts] = useState<{ hostname: string; ip: string }[]>([]);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [filteredSuggestions, setFilteredSuggestions] = useState<{ hostname: string; ip: string }[]>([]);
  const [suggestionIndex, setSuggestionIndex] = useState(0);
  const [cursorPos, setCursorPos] = useState(0);

  const fetchHosts = useCallback(async (hostToResolve?: string) => {
    try {
      const [connections, mcpHosts] = await Promise.all([
        invoke<Connection[]>("load_connections"),
        invoke<McpHost[]>("get_mcp_hosts"),
      ]);

      const hostMap = new Map<string, string>();
      if (connections) {
        connections.forEach((c) => {
          if (c.hostname && c.ip) hostMap.set(c.hostname, c.ip);
        });
      }
      if (mcpHosts) {
        mcpHosts.forEach((h) => {
          if (h.hostname && h.ip) hostMap.set(h.hostname, h.ip);
        });
      }

      // Active resolution for new IP addresses
      if (hostToResolve && /^(?:\d{1,3}\.){3}\d{1,3}$/.test(hostToResolve)) {
        const isKnown = Array.from(hostMap.values()).includes(hostToResolve);
        if (!isKnown) {
          try {
            const resolvedName = await invoke<string>("resolve_ip", { ip: hostToResolve });
            if (resolvedName) {
              hostMap.set(resolvedName, hostToResolve);
            }
          } catch (e) {
            // Silently fail if resolution fails
          }
        }
      }

      const hostsArray = Array.from(hostMap.entries()).map(([hostname, ip]) => ({
        hostname,
        ip,
      }));

      setAvailableHosts(hostsArray);
    } catch (e) {
      console.error("Failed to fetch hosts for suggestions:", e);
    }
  }, []);

  // Initial fetch for hosts
  useEffect(() => {
    fetchHosts();
  }, [fetchHosts]);

  // Trigger name resolution/host fetch when active IP/host changes
  useEffect(() => {
    if (recentIPs.length > 0) {
      fetchHosts(recentIPs[0]);
    }
  }, [recentIPs[0], fetchHosts]);

  const updateRecentHosts = useCallback((hosts: string[]) => {
    if (hosts.length === 0) return;

    const newRecent = [
      ...new Set([...hosts, ...recentIPs]),
    ].slice(0, 10);

    const isChanged = newRecent.length !== recentIPs.length || newRecent.some((val, idx) => val !== recentIPs[idx]);
    if (!isChanged) return;

    setRecentIPs(newRecent);

    if (activeSessionId) {
      updateSessionRecentIps(activeSessionId, newRecent);
    }

    saveAllSettings({ recentIps: newRecent }).catch((e) => {
      console.error("Failed to save recent hosts to settings:", e);
    });
  }, [recentIPs, activeSessionId, updateSessionRecentIps, saveAllSettings, setRecentIPs]);

  const handleSelectSuggestion = (hostObj: { hostname: string; ip: string }) => {
    const host = hostObj.hostname;
    const textBeforeCursor = input.slice(0, cursorPos);
    const atIndex = textBeforeCursor.lastIndexOf("@");
    const newValue = input.slice(0, atIndex) + host + " " + input.slice(cursorPos);
    setInput(newValue);
    setShowSuggestions(false);

    // Focus back to textarea and set cursor position
    setTimeout(() => {
      if (textareaRef.current) {
        textareaRef.current.focus();
        const newPos = atIndex + host.length + 1;
        textareaRef.current.setSelectionRange(newPos, newPos);
      }
    }, 0);
  };

  return {
    availableHosts,
    setAvailableHosts,
    showSuggestions,
    setShowSuggestions,
    filteredSuggestions,
    setFilteredSuggestions,
    suggestionIndex,
    setSuggestionIndex,
    cursorPos,
    setCursorPos,
    fetchHosts,
    updateRecentHosts,
    handleSelectSuggestion,
  };
}
