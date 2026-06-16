import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { Message } from "../../types";

interface UseMcpListenersProps {
  setMessages: React.Dispatch<React.SetStateAction<Message[]>>;
}

export function useMcpListeners({ setMessages }: UseMcpListenersProps) {
  useEffect(() => {
    let unlistenFn: (() => void) | undefined;

    const setupListener = async () => {
      let unlistenArp: (() => void) | undefined;
      let unlistenRoute: (() => void) | undefined;

      unlistenArp = await listen<{ deviceName: string; savedPath: string }>(
        "arp-yaml-saved",
        (event) => {
          const { deviceName, savedPath } = event.payload;
          setMessages((prev) =>
            prev.map((msg) => {
              const msgDevice = msg.args?.deviceName || msg.args?.device_name;
              if (
                msg.event_type === "ToolExecution" &&
                msg.tool_id === "fetch_arp" &&
                msgDevice === deviceName &&
                !msg.saved_path
              ) {
                return {
                  ...msg,
                  saved_path: savedPath,
                };
              }
              return msg;
            })
          );
        }
      );

      unlistenRoute = await listen<{ deviceName: string; savedPath: string }>(
        "route-yaml-saved",
        (event) => {
          const { deviceName, savedPath } = event.payload;
          setMessages((prev) =>
            prev.map((msg) => {
              const msgDevice = msg.args?.deviceName || msg.args?.device_name;
              if (
                msg.event_type === "ToolExecution" &&
                msg.tool_id === "fetch_routing" &&
                msgDevice === deviceName &&
                !msg.saved_path
              ) {
                return {
                  ...msg,
                  saved_path: savedPath,
                };
              }
              return msg;
            })
          );
        }
      );

      unlistenFn = () => {
        if (unlistenArp) unlistenArp();
        if (unlistenRoute) unlistenRoute();
      };
    };

    setupListener();

    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, [setMessages]);
}
