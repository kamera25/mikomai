import { renderHook, act } from "@testing-library/react";
import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { useResizablePane } from "../useResizablePane";

describe("useResizablePane", () => {
  beforeEach(() => {
    vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
      cb(performance.now());
      return 0;
    });
    vi.stubGlobal("cancelAnimationFrame", () => {});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("initializes with default pane widths", () => {
    const { result } = renderHook(() => useResizablePane());
    expect(result.current.sidebarWidth).toBe(280);
    expect(result.current.diffWidth).toBe(450);
    expect(result.current.isResizingLeft).toBe(false);
    expect(result.current.isResizingRight).toBe(false);
  });

  it("customizes initial widths", () => {
    const { result } = renderHook(() =>
      useResizablePane({ initialSidebarWidth: 320, initialDiffWidth: 500 })
    );
    expect(result.current.sidebarWidth).toBe(320);
    expect(result.current.diffWidth).toBe(500);
  });

  it("handles left sidebar resize mouse events", () => {
    const { result } = renderHook(() => useResizablePane());

    act(() => {
      const mockEvent = { preventDefault: () => {} } as React.MouseEvent;
      result.current.handleLeftMouseDown(mockEvent);
    });

    expect(result.current.isResizingLeft).toBe(true);

    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 300 }));
    });

    // 300 - 60 = 240
    expect(result.current.sidebarWidth).toBe(240);

    act(() => {
      window.dispatchEvent(new MouseEvent("mouseup"));
    });

    expect(result.current.isResizingLeft).toBe(false);
  });

  it("enforces sidebar minimum and maximum limits", () => {
    const { result } = renderHook(() => useResizablePane());

    act(() => {
      result.current.handleLeftMouseDown({ preventDefault: () => {} } as React.MouseEvent);
    });

    // clientX = 100 -> 100 - 60 = 40, min is 160
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 100 }));
    });
    expect(result.current.sidebarWidth).toBe(160);

    // clientX = 800 -> 800 - 60 = 740, max is 600
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 800 }));
    });
    expect(result.current.sidebarWidth).toBe(600);
  });
});
