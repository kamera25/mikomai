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

  it("enforces sidebar snag resistance at minimum width and maximum limits", () => {
    const { result } = renderHook(() => useResizablePane());

    act(() => {
      result.current.handleLeftMouseDown({ preventDefault: () => {} } as React.MouseEvent);
    });

    // clientX = 180 -> rawWidth = 180 - 60 = 120 (between threshold 80 and min 160) -> snags at min (160)
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 180 }));
    });
    expect(result.current.sidebarWidth).toBe(160);

    // clientX = 800 -> 800 - 60 = 740, max is 600
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 800 }));
    });
    expect(result.current.sidebarWidth).toBe(600);
  });

  it("collapses sidebar when pulled further past the collapse threshold", () => {
    const onSidebarCollapse = vi.fn();
    const { result } = renderHook(() =>
      useResizablePane({
        onSidebarCollapse,
      })
    );

    act(() => {
      result.current.handleLeftMouseDown({ preventDefault: () => {} } as React.MouseEvent);
    });

    // 1. Snag zone: clientX = 180 (rawWidth = 120 >= 80)
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 180 }));
    });
    expect(result.current.sidebarWidth).toBe(160);
    expect(onSidebarCollapse).not.toHaveBeenCalledWith(true);

    // 2. Pulled past snag threshold: clientX = 100 (rawWidth = 40 < 80) -> collapse!
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 100 }));
    });
    expect(onSidebarCollapse).toHaveBeenCalledWith(true);

    // 3. Drag back to right -> re-expands
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 250 }));
    });
    expect(onSidebarCollapse).toHaveBeenCalledWith(false);
    expect(result.current.sidebarWidth).toBe(190);

    // 4. Pull past threshold again and release mouse
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 90 }));
    });
    expect(onSidebarCollapse).toHaveBeenCalledWith(true);

    act(() => {
      window.dispatchEvent(new MouseEvent("mouseup"));
    });
    expect(result.current.isResizingLeft).toBe(false);
    // Width is restored to last valid width on mouseup
    expect(result.current.sidebarWidth).toBe(190);
  });

  it("handles diff panel resize, snag and collapse", () => {
    const onDiffCollapse = vi.fn();
    const { result } = renderHook(() =>
      useResizablePane({
        onDiffCollapse,
      })
    );

    act(() => {
      result.current.handleRightMouseDown({ preventDefault: () => {} } as React.MouseEvent);
    });

    // Mock innerWidth = 1000
    vi.stubGlobal("innerWidth", 1000);

    // clientX = 600 -> rawWidth = 1000 - 600 = 400
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 600 }));
    });
    expect(result.current.diffWidth).toBe(400);

    // clientX = 800 -> rawWidth = 200 (between threshold 140 and min 280) -> snags at 280
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 800 }));
    });
    expect(result.current.diffWidth).toBe(280);
    expect(onDiffCollapse).not.toHaveBeenCalledWith(true);

    // clientX = 900 -> rawWidth = 100 (< 140) -> collapses!
    act(() => {
      window.dispatchEvent(new MouseEvent("mousemove", { clientX: 900 }));
    });
    expect(onDiffCollapse).toHaveBeenCalledWith(true);

    act(() => {
      window.dispatchEvent(new MouseEvent("mouseup"));
    });
    expect(result.current.isResizingRight).toBe(false);
    expect(result.current.diffWidth).toBe(400);
  });
});
