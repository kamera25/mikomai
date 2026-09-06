import { useState, useEffect } from "react";
import { ipc } from "../platform";
import { useUIContext } from "../contexts/UIContext";

export function useConfigDiffEvents() {
  const { dispatch: uiDispatch } = useUIContext();
  const [diffCommitId, setDiffCommitId] = useState<string | null>(null);

  // Update ConfigDiffPanel with dynamic conversion diffs
  useEffect(() => {
    const unlisten = ipc.subscribe<any>("chat-event", (chatEvent) => {
      if (chatEvent.type === "mcpToolFinished") {
        const { toolId, success, output, args } = chatEvent.payload;
        if (success && toolId === "convert_cisco_config") {
          // Extract the converted config from markdown
          const regex = /```[a-z]*\n([\s\S]*?)```/;
          const match = output.match(regex);
          if (match && match[1]) {
            const converted = match[1].trim();
            const vendor = args?.target_vendor || args?.targetVendor || "juniper";
            const lines = converted.split("\n");
            const diffLines = lines.map((line: string, idx: number) => ({
              type: "insert" as const,
              oldLine: null,
              newLine: idx + 1,
              content: line,
            }));

            uiDispatch({
              type: "SET_CONFIG_DIFF_DATA",
              payload: {
                fileName: `${vendor}.conf`,
                additions: lines.length,
                deletions: 0,
                diffLines,
              },
            });
            uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: true });
          }
        }
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [uiDispatch]);

  // Listen to request-diff-commit from Rust
  useEffect(() => {
    const unlisten = ipc.subscribe<any>("request-diff-commit", ({ id, config, fileName, hostname, ip }) => {
      if (id) {
        setDiffCommitId(id);
      }
      if (config) {
        const lines = config.split("\n");
        const diffLines = lines.map((line: string, idx: number) => ({
          type: "insert" as const,
          oldLine: null,
          newLine: idx + 1,
          content: line,
        }));

        uiDispatch({
          type: "SET_CONFIG_DIFF_DATA",
          payload: {
            fileName: fileName || "cisco.conf",
            additions: lines.length,
            deletions: 0,
            diffLines,
            hostname,
            ip,
          },
        });
        uiDispatch({ type: "SET_CONFIG_DIFF_OPEN", payload: true });
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [uiDispatch]);

  return {
    diffCommitId,
    setDiffCommitId,
  };
}
