import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { SettingsProvider, useSettingsContext } from "./SettingsContext";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation((cmd) => {
    if (cmd === "load_settings") {
      return Promise.resolve({
        historyLimit: 5,
        temperature: 0.7,
      });
    }
    return Promise.resolve();
  }),
}));

const TestComponent = () => {
  const { historyLimit, temperature } = useSettingsContext();
  return (
    <div>
      <span data-testid="historyLimit">{historyLimit}</span>
      <span data-testid="temperature">{temperature}</span>
    </div>
  );
};

describe("SettingsContext", () => {
  it("should provide settings from Context", async () => {
    render(
      <SettingsProvider>
        <TestComponent />
      </SettingsProvider>
    );

    // Wait for the settings to load (useEffect runs on mount)
    const historyLimitEl = await screen.findByTestId("historyLimit");
    expect(historyLimitEl.textContent).toBe("5");
    
    const temperatureEl = await screen.findByTestId("temperature");
    expect(temperatureEl.textContent).toBe("0.7");
  });
});
