import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act } from "@testing-library/react";
import { KeyringAccessModal } from "../KeyringAccessModal";
import { listen } from "@tauri-apps/api/event";

describe("KeyringAccessModal", () => {
  let eventListeners: Record<string, (event: unknown) => void> = {};

  beforeEach(() => {
    eventListeners = {};
    vi.mocked(listen).mockImplementation(async (event: string, callback: (payload: unknown) => void) => {
      eventListeners[event] = callback;
      return () => {
        delete eventListeners[event];
      };
    });
  });

  it("does not render when closed by default", () => {
    render(<KeyringAccessModal />);
    expect(screen.queryByTestId("keyring-access-modal")).not.toBeInTheDocument();
  });

  it("renders when forceOpen is true", () => {
    render(<KeyringAccessModal forceOpen={true} />);
    expect(screen.getByTestId("keyring-access-modal")).toBeInTheDocument();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("opens on keyring-access-start event and closes on keyring-access-end event", async () => {
    render(<KeyringAccessModal />);

    expect(screen.queryByTestId("keyring-access-modal")).not.toBeInTheDocument();

    // Trigger keyring-access-start
    await act(async () => {
      if (eventListeners["keyring-access-start"]) {
        eventListeners["keyring-access-start"]({});
      }
    });

    expect(screen.getByTestId("keyring-access-modal")).toBeInTheDocument();

    // Trigger keyring-access-end
    await act(async () => {
      if (eventListeners["keyring-access-end"]) {
        eventListeners["keyring-access-end"]({});
      }
    });

    expect(screen.queryByTestId("keyring-access-modal")).not.toBeInTheDocument();
  });

  it("cleans up event listeners on unmount", async () => {
    let unmountFn: () => void = () => {};
    await act(async () => {
      const { unmount } = render(<KeyringAccessModal />);
      unmountFn = unmount;
    });

    expect(eventListeners["keyring-access-start"]).toBeDefined();
    expect(eventListeners["keyring-access-end"]).toBeDefined();

    await act(async () => {
      unmountFn();
    });

    expect(eventListeners["keyring-access-start"]).toBeUndefined();
    expect(eventListeners["keyring-access-end"]).toBeUndefined();
  });
});
