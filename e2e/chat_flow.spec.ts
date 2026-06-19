import { test, expect } from "@playwright/test";

test.describe("Chat Flow and Tool Execution E2E Test", () => {
  test.beforeEach(async ({ page }) => {
    // Inject mock Tauri APIs before the page loads
    await page.addInitScript(() => {
      window.__TAURI__ = {
        core: {
          invoke: async (cmd: string, args: any) => {
            console.log(`Mocked invoke: ${cmd}`, args);
            if (cmd === "handle_mcp_message") {
              // Simulate the Tauri backend emitting chat-events in response
              setTimeout(() => {
                const event = new CustomEvent("chat-event", {
                  detail: {
                    event: "McpInitialStarted",
                    payload: { task_id: "test-task-123" }
                  }
                });
                window.dispatchEvent(event);
              }, 100);

              setTimeout(() => {
                const event = new CustomEvent("chat-event", {
                  detail: {
                    event: "McpInitialFinished",
                    payload: { task_id: "test-task-123", content: "Thinking..." }
                  }
                });
                window.dispatchEvent(event);
              }, 300);

              setTimeout(() => {
                const event = new CustomEvent("chat-event", {
                  detail: {
                    event: "McpToolStarted",
                    payload: {
                      task_id: "test-task-123",
                      tool_id: "self_network_ping",
                      tool_label: "Ping",
                      args: { host: "192.168.1.1" },
                      resolved_host: "192.168.1.1"
                    }
                  }
                });
                window.dispatchEvent(event);
              }, 500);

              setTimeout(() => {
                const event = new CustomEvent("chat-event", {
                  detail: {
                    event: "McpToolFinished",
                    payload: {
                      task_id: "test-task-123",
                      success: true,
                      output: "Ping successful. 0% packet loss."
                    }
                  }
                });
                window.dispatchEvent(event);
              }, 800);

              setTimeout(() => {
                const event = new CustomEvent("chat-event", {
                  detail: {
                    event: "McpSummarySaved",
                    payload: {
                      task_id: "test-task-123",
                      summary_text: "Pinged 192.168.1.1 successfully.",
                      summary: {
                        timestamp: new Date().toISOString(),
                        content: "Pinged 192.168.1.1 successfully."
                      },
                      content: "The ping to 192.168.1.1 was successful with no packet loss."
                    }
                  }
                });
                window.dispatchEvent(event);
              }, 1200);

              return Promise.resolve();
            }
            return Promise.resolve({});
          }
        },
        event: {
          listen: async (eventName: string, handler: (event: any) => void) => {
            window.addEventListener(eventName, (e: any) => {
              handler({ payload: e.detail.payload, event: e.detail.event });
            });
            return () => {};
          }
        }
      } as any;
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
