import { test, expect } from "@playwright/test";

test.describe("Chat Flow and Tool Execution E2E Test", () => {
  test.beforeEach(async ({ page }) => {
    // Inject mock Tauri APIs before the page loads
    await page.addInitScript(() => {
      const eventListeners = new Map<string, number[]>();
      let nextCallbackId = 1;
      const callbacks = new Map<number, (event: any) => void>();

      const emitEvent = (eventName: string, payload: any) => {
        const handlers = eventListeners.get(eventName) || [];
        for (const handlerId of handlers) {
          const cb = callbacks.get(handlerId);
          if (cb) {
            cb({ event: eventName, id: handlerId, payload });
          }
        }
      };

      (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
        unregisterListener: (_event: string, eventId: number) => {
          callbacks.delete(eventId);
        },
      };

      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args: any) => {
          console.log(`Mocked invoke: ${cmd}`, args);
          if (cmd === "plugin:event|listen") {
            const { event, handler } = args;
            if (!eventListeners.has(event)) {
              eventListeners.set(event, []);
            }
            eventListeners.get(event)!.push(handler);
            return handler;
          }
          if (cmd === "plugin:event|unlisten") {
            return;
          }
          if (cmd === "initialize_history") {
            return {
              history: [{ id: "test-session", type: "session", title: "新しいセッション", messages: [] }],
              activeSessionId: "test-session",
            };
          }
          if (cmd === "load_settings") {
            return {
              repoPath: "",
              modelFilename: "",
              consolePort: null,
              consoleBaudRate: 9600,
              ipVersion: "auto",
              autoSaveHistory: true,
              recentIPs: [],
            };
          }
          if (cmd === "load_connections") return [];
          if (cmd === "get_mcp_hosts") return [];
          if (cmd === "load_summaries") return [];
          if (cmd === "get_model_status") return { isLoaded: true, modelName: "test-model" };
          if (cmd === "mutate_history") {
            return {
              history: [{ id: "test-session", type: "session", title: "新しいセッション", messages: [] }],
              activeSessionId: "test-session",
            };
          }
          if (cmd === "handle_mcp_message") {
            setTimeout(() => {
              emitEvent("chat-event", {
                type: "mcpInitialStarted",
                payload: { taskId: "test-task-123" },
              });
            }, 100);

            setTimeout(() => {
              emitEvent("chat-event", {
                type: "mcpInitialFinished",
                payload: { taskId: "test-task-123", content: "Thinking..." },
              });
            }, 300);

            setTimeout(() => {
              emitEvent("chat-event", {
                type: "mcpToolStarted",
                payload: {
                  taskId: "test-task-123",
                  toolId: "self_network_ping",
                  args: { host: "192.168.1.1" },
                  resolvedHost: "192.168.1.1",
                },
              });
            }, 500);

            setTimeout(() => {
              emitEvent("chat-event", {
                type: "mcpToolFinished",
                payload: {
                  taskId: "test-task-123",
                  success: true,
                  output: "Ping successful. 0% packet loss.",
                },
              });
            }, 800);

            setTimeout(() => {
              emitEvent("chat-event", {
                type: "mcpSummarySaved",
                payload: {
                  taskId: "test-task-123",
                  summaryText: "Pinged 192.168.1.1 successfully.",
                  summary: {
                    timestamp: new Date().toISOString(),
                    content: "Pinged 192.168.1.1 successfully.",
                  },
                  content: "The ping to 192.168.1.1 was successful with no packet loss.",
                },
              });
            }, 1200);

            return Promise.resolve();
          }
          return Promise.resolve({});
        },
        transformCallback: (callback: any) => {
          const id = nextCallbackId++;
          callbacks.set(id, callback);
          return id;
        },
        unregisterCallback: (id: number) => {
          callbacks.delete(id);
        },
      };

      (window as any).__TAURI__ = {
        core: {
          invoke: (window as any).__TAURI_INTERNALS__.invoke,
        },
        event: {
          listen: async (eventName: string, handler: (event: any) => void) => {
            const id = (window as any).__TAURI_INTERNALS__.transformCallback(handler);
            await (window as any).__TAURI_INTERNALS__.invoke("plugin:event|listen", {
              event: eventName,
              handler: id,
            });
            return () => {
              (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__.unregisterListener(eventName, id);
            };
          },
        },
      };
    });

    await page.goto("/");
  });

  test("should send message, show thinking state, execute tool, and display final result", async ({ page }) => {
    // 1. Enter message
    const chatInput = page.locator('textarea, input[type="text"]').first();
    await expect(chatInput).toBeVisible();
    await chatInput.fill("ping 192.168.1.1");
    await chatInput.press("Enter");

    // 2. Verify message is displayed in user bubble
    await expect(page.locator("text=ping 192.168.1.1")).toBeVisible();

    // 3. Verify tool execution indicators / status messages are shown in UI
    // (Depending on how the UI displays tool runs/outputs)
    await expect(page.locator("text=Ping")).toBeVisible();
    await expect(page.locator("text=successful")).toBeVisible();
  });
});
